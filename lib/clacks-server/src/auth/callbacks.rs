//! Standard OIDC request/callback wiring and account admission.

use super::{
    error::AuthError,
    oidc,
    pending::{self, PendingFlow, Purpose},
};
use crate::{runtime::ServerState, store::logins::LoginReference};
use axum::{
    extract::{Path, Query, State},
    response::Redirect,
};
use axum_session_sqlx::SessionPgSession;
use openidconnect::{
    AuthenticationFlow, AuthorizationCode, CsrfToken, Nonce, PkceCodeChallenge, TokenResponse,
    core::CoreResponseType,
};
use serde::Deserialize;

/// Provider redirect parameters; their contents are never logged.
#[derive(Deserialize)]
pub(crate) struct Callback {
    /// Standard browser request correlation value.
    state: String,
    /// Code exchanged only with the configured provider.
    code: Option<String>,
    /// Provider refusal/error indication; untrusted text is not exposed.
    error: Option<String>,
}

/// Start ordinary configured-provider sign-in without account mutation.
#[tracing::instrument(skip_all, name = "account.login.start")]
pub(crate) async fn start(
    State(state): State<ServerState>,
    session: SessionPgSession,
    Path(key): Path<String>,
) -> Result<Redirect, AuthError> {
    begin(&state, &session, key, Purpose::SignIn).await
}

/// Start one standard provider request for its server-recorded Clacks purpose.
pub(crate) async fn begin(
    state: &ServerState,
    session: &SessionPgSession,
    key: String,
    purpose: Purpose,
) -> Result<Redirect, AuthError> {
    let provider = state
        .config
        .providers
        .get(&key)
        .ok_or(AuthError::InvalidCallback)?;
    let callback = state
        .config
        .callback(&key)
        .map_err(|_| AuthError::InvalidCallback)?;
    let client = oidc::discover(provider, &callback, &state.provider_http)
        .await
        .map_err(|_| AuthError::Provider)?;
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let mut request = client
        .authorize_url(
            AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .set_pkce_challenge(challenge);
    if purpose.requires_recent_authentication() {
        request = request.set_max_age(std::time::Duration::from_secs(300));
    }
    if matches!(&purpose, Purpose::CompleteLink { .. }) {
        request = request.add_prompt(openidconnect::core::CoreAuthPrompt::SelectAccount);
    }
    let (url, csrf, nonce) = request.url();
    pending::insert(
        session,
        PendingFlow {
            provider: key,
            state: csrf,
            nonce,
            verifier,
            created_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            purpose,
        },
        state.config.pending_seconds,
    );
    Ok(Redirect::to(url.as_str()))
}

/// Validate the protocol response before any Clacks account operation.
#[tracing::instrument(skip_all, name = "account.login.callback")]
pub(crate) async fn callback(
    State(state): State<ServerState>,
    session: SessionPgSession,
    Path(key): Path<String>,
    Query(query): Query<Callback>,
) -> Result<Redirect, AuthError> {
    let flow = pending::take(&session, &query.state, &key, state.config.pending_seconds)
        .map_err(|_| AuthError::InvalidCallback)?;
    if query.error.is_some() {
        return Err(AuthError::Provider);
    }
    let provider = state
        .config
        .providers
        .get(&key)
        .ok_or(AuthError::InvalidCallback)?;
    let callback = state
        .config
        .callback(&key)
        .map_err(|_| AuthError::InvalidCallback)?;
    let client = oidc::discover(provider, &callback, &state.provider_http)
        .await
        .map_err(|_| AuthError::Provider)?;
    let response = client
        .exchange_code(AuthorizationCode::new(
            query.code.ok_or(AuthError::InvalidCallback)?,
        ))
        .map_err(|_| AuthError::Provider)?
        .set_pkce_verifier(flow.verifier)
        .request_async(&state.provider_http)
        .await
        .map_err(|_| AuthError::Provider)?;
    let token = response.id_token().ok_or(AuthError::InvalidCallback)?;
    let recent = flow.purpose.requires_recent_authentication();
    let verifier = client
        .id_token_verifier()
        .set_auth_time_verifier_fn(move |auth_time| {
            if !recent {
                return Ok(());
            }
            let now = time::OffsetDateTime::now_utc().unix_timestamp();
            match auth_time {
                Some(value)
                    if value.timestamp() <= now && now.saturating_sub(value.timestamp()) <= 300 =>
                {
                    Ok(())
                }
                _ => Err("provider did not establish recent authentication".into()),
            }
        });
    let claims = token
        .claims(&verifier, &flow.nonce)
        .map_err(|_| AuthError::InvalidCallback)?;
    let identity = oidc::VerifiedIdentity {
        issuer: claims.issuer().as_str().to_owned(),
        subject: claims.subject().as_str().to_owned(),
        display_name: None,
    };
    match flow.purpose {
        Purpose::SignIn => {
            let reference = state
                .store
                .provision_login(identity, state.lifetime())
                .await
                .map_err(AuthError::from_store)?;
            session.set("login_reference", reference.secret());
            session.renew();
            Ok(Redirect::to("/account/identities"))
        }
        Purpose::AuthorizeLink {
            origin,
            authorizer,
            new_provider,
        } => {
            super::changes::verify_authorizer(&state, &origin, &authorizer, &identity).await?;
            begin(
                &state,
                &session,
                new_provider,
                Purpose::CompleteLink { origin, authorizer },
            )
            .await
        }
        Purpose::CompleteLink { origin, authorizer } => {
            state
                .store
                .link_identity(&super::changes::action(origin, authorizer)?, identity)
                .await
                .map_err(AuthError::from_store)?;
            Ok(Redirect::to("/account/identities"))
        }
        Purpose::AuthorizeUnlink {
            origin,
            authorizer,
            target,
        } => {
            super::changes::verify_authorizer(&state, &origin, &authorizer, &identity).await?;
            let target = target.parse().map_err(|_| AuthError::InvalidCallback)?;
            let reference = origin.login_reference.clone();
            state
                .store
                .unlink_identity(&super::changes::action(origin, authorizer)?, target)
                .await
                .map_err(AuthError::from_store)?;
            let live = state
                .store
                .authenticate(&LoginReference::from_session(reference), state.lifetime())
                .await
                .map_err(AuthError::from_store)?
                .is_some();
            if live {
                Ok(Redirect::to("/account/identities"))
            } else {
                session.destroy();
                Ok(Redirect::to("/sign-in"))
            }
        }
    }
}

/// End this Clacks browser login without affecting provider or other-device logins.
pub(crate) async fn logout(
    State(state): State<ServerState>,
    session: SessionPgSession,
) -> Result<Redirect, AuthError> {
    if let Some(reference) = session.get::<String>("login_reference") {
        state
            .store
            .logout(&LoginReference::from_session(reference))
            .await
            .map_err(AuthError::from_store)?;
    }
    session.destroy();
    Ok(Redirect::to("/sign-in"))
}

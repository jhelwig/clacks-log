//! Standard OIDC request/callback wiring and account admission.

use super::{
    error::AuthError,
    oidc,
    pending::{self, Purpose},
};
use crate::{runtime::ServerState, store::logins::LoginReference};
use axum::{
    extract::{Path, Query, State},
    response::Redirect,
};
use axum_session_sqlx::SessionPgSession;
use openidconnect::{AuthorizationCode, TokenResponse};
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

/// Return provider callbacks to the account page or a safe sign-in error.
pub(crate) async fn callback(
    State(state): State<ServerState>,
    session: SessionPgSession,
    Path(key): Path<String>,
    Query(query): Query<Callback>,
) -> Redirect {
    match complete(state, session, key, query).await {
        Ok(redirect) => redirect,
        Err(error) => {
            let query = url::form_urlencoded::Serializer::new(String::new())
                .append_pair("error", &error.to_string())
                .finish();
            Redirect::to(&format!("/sign-in?{query}"))
        }
    }
}

/// Validate the protocol response before any Clacks account operation.
#[tracing::instrument(skip_all, name = "account.login.callback")]
async fn complete(
    state: ServerState,
    session: SessionPgSession,
    key: String,
    query: Callback,
) -> Result<Redirect, AuthError> {
    let provider = state
        .config
        .providers
        .get(&key)
        .ok_or(AuthError::InvalidCallback)?;
    let flow = pending::take(&session, &query.state, &key, state.config.pending_seconds).map_err(
        |_| {
            tracing::info!(
                provider = %key,
                "Authorization context missing or expired; restart required"
            );
            AuthError::InvalidCallback
        },
    )?;

    if query.error.is_some() {
        tracing::info!(provider = %key, "Identity provider declined authorization");
        return Err(AuthError::Provider);
    }

    let callback = state
        .config
        .callback(&key)
        .map_err(|_| AuthError::InvalidCallback)?;
    let client = oidc::discover(provider, &callback, &state.provider_http)
        .await
        .map_err(|error| {
            tracing::warn!(
                provider = %key,
                reason = %error,
                "Identity provider client setup failed"
            );
            AuthError::Provider
        })?;

    let response = client
        .exchange_code(AuthorizationCode::new(
            query.code.ok_or(AuthError::InvalidCallback)?,
        ))
        .map_err(|_| AuthError::Provider)?
        .set_pkce_verifier(flow.verifier)
        .request_async(&state.provider_http)
        .await
        .map_err(|_| {
            tracing::warn!(provider = %key, "Identity provider code exchange failed");
            AuthError::Provider
        })?;

    let token = response.id_token().ok_or(AuthError::InvalidCallback)?;
    let claims = token
        .claims(&client.id_token_verifier(), &flow.nonce)
        .map_err(|_| {
            tracing::info!(
                provider = %key,
                "Identity token did not satisfy authorization requirements"
            );
            AuthError::InvalidCallback
        })?;

    let identity = oidc::VerifiedIdentity {
        issuer: claims.issuer().as_str().to_owned(),
        subject: claims.subject().as_str().to_owned(),
        display_name: None,
    };

    match flow.purpose {
        Purpose::SignIn { enrollment } => {
            let reference = state
                .store
                .provision_login(identity, state.lifetime())
                .await
                .map_err(AuthError::from_store)?;
            session.set("login_reference", reference.secret());
            session.renew();
            let location = enrollment
                .as_deref()
                .map(crate::enrollment::approval::location)
                .unwrap_or_else(|| "/account/identities".into());
            Ok(Redirect::to(&location))
        }
        Purpose::AuthorizeLink {
            origin,
            authorizer,
            new_provider,
        } => {
            super::changes::verify_authorizer(&state, &origin, authorizer, &identity).await?;
            Ok(Redirect::to(
                oidc::authorization_url(
                    &state,
                    &session,
                    new_provider,
                    Purpose::CompleteLink { origin, authorizer },
                )
                .await?
                .as_str(),
            ))
        }
        Purpose::CompleteLink { origin, authorizer } => {
            state
                .store
                .link_identity(&super::changes::action(origin, authorizer), identity)
                .await
                .map_err(AuthError::from_store)?;
            Ok(Redirect::to("/account/identities"))
        }
        Purpose::AuthorizeUnlink {
            origin,
            authorizer,
            target,
        } => {
            super::changes::verify_authorizer(&state, &origin, authorizer, &identity).await?;
            let reference = origin.login_reference.clone();
            state
                .store
                .unlink_identity(&super::changes::action(origin, authorizer), target)
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

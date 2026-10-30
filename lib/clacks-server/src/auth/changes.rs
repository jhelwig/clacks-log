//! Identity actions bound to a current account and an identity that remains linked.

use super::{
    error::AuthError,
    oidc::VerifiedIdentity,
    pending::{OriginatingLogin, Purpose},
    principal::Principal,
};
use crate::{
    runtime::ServerState,
    store::{
        identities::{IdentityAction, IdentityView},
        logins::LoginReference,
    },
};
use axum_session_sqlx::SessionPgSession;
use serde::Deserialize;

/// Account-owner selection for a new provider identity.
#[derive(Deserialize)]
pub struct LinkForm {
    /// Configured provider key for the additional identity.
    pub provider: String,
    /// Existing identity selected to authorize this action.
    pub authorizing_identity: String,
}

/// Account-owner selection for removal through an identity that remains linked.
#[derive(Deserialize)]
pub struct UnlinkForm {
    /// Selected identity belonging to this account.
    pub target: String,
    /// Different existing identity selected for recent authentication.
    pub authorizing_identity: String,
}

/// Find a selected owned identity; another account's ID is never authority.
async fn owned(
    state: &ServerState,
    principal: &Principal,
    id: ulid::Ulid,
) -> Result<IdentityView, AuthError> {
    state
        .store
        .identities(principal)
        .await
        .map_err(AuthError::from_store)?
        .into_iter()
        .find(|identity| identity.identity_id == id)
        .ok_or(AuthError::InvalidCallback)
}

/// Select a configured provider for an exact recorded issuer.
fn provider_for(state: &ServerState, identity: &IdentityView) -> Result<String, AuthError> {
    state
        .config
        .providers
        .iter()
        .find(|(_, provider)| provider.issuer.as_str() == identity.issuer)
        .map(|(key, _)| key.clone())
        .ok_or(AuthError::Provider)
}

/// Retain the current account/login needed for this particular action.
fn origin(
    session: &SessionPgSession,
    principal: &Principal,
) -> Result<OriginatingLogin, AuthError> {
    Ok(OriginatingLogin {
        account_id: principal.account_id,
        login_reference: session
            .get("login_reference")
            .ok_or(AuthError::SignInRequired)?,
    })
}

/// Begin a specific link operation from an authenticated browser request.
pub async fn request_link(
    state: &ServerState,
    session: &SessionPgSession,
    principal: &Principal,
    form: LinkForm,
) -> Result<url::Url, AuthError> {
    let authorizer = owned(
        state,
        principal,
        form.authorizing_identity
            .parse()
            .map_err(|_| AuthError::InvalidCallback)?,
    )
    .await?;
    if !state.config.providers.contains_key(&form.provider) {
        return Err(AuthError::InvalidCallback);
    }
    let provider = provider_for(state, &authorizer)?;
    let purpose = Purpose::AuthorizeLink {
        origin: origin(session, principal)?,
        authorizer: authorizer.identity_id,
        new_provider: form.provider,
    };
    super::oidc::authorization_url(state, session, provider, purpose).await
}

/// Begin removal of one owned identity through a different remaining identity.
pub async fn request_unlink(
    state: &ServerState,
    session: &SessionPgSession,
    principal: &Principal,
    form: UnlinkForm,
) -> Result<url::Url, AuthError> {
    let target = owned(
        state,
        principal,
        form.target
            .parse()
            .map_err(|_| AuthError::InvalidCallback)?,
    )
    .await?;
    let authorizer = owned(
        state,
        principal,
        form.authorizing_identity
            .parse()
            .map_err(|_| AuthError::InvalidCallback)?,
    )
    .await?;
    if target.identity_id == authorizer.identity_id {
        return Err(AuthError::Conflict(
            "Choose an identity that will remain linked.".into(),
        ));
    }
    let provider = provider_for(state, &authorizer)?;
    let purpose = Purpose::AuthorizeUnlink {
        origin: origin(session, principal)?,
        authorizer: authorizer.identity_id,
        target: target.identity_id,
    };
    super::oidc::authorization_url(state, session, provider, purpose).await
}

/// Recheck a verified authorizing identity and the original live login.
pub(crate) async fn verify_authorizer(
    state: &ServerState,
    origin: &OriginatingLogin,
    authorizer: ulid::Ulid,
    verified: &VerifiedIdentity,
) -> Result<(), AuthError> {
    let principal = state
        .store
        .authenticate(
            &LoginReference::from_session(origin.login_reference.clone()),
            state.lifetime(),
        )
        .await
        .map_err(AuthError::from_store)?
        .ok_or(AuthError::SignInRequired)?;
    if principal.account_id != origin.account_id {
        return Err(AuthError::InvalidCallback);
    }
    let expected = owned(state, &principal, authorizer).await?;
    if expected.issuer != verified.issuer || expected.subject != verified.subject {
        return Err(AuthError::Conflict(
            "Authenticate using the selected existing identity.".into(),
        ));
    }
    Ok(())
}

/// Interpret server-retained context for the identity transaction boundary.
pub(crate) fn action(origin: OriginatingLogin, authorizer: ulid::Ulid) -> IdentityAction {
    IdentityAction {
        account_id: origin.account_id,
        originating_login: origin.login_reference,
        authorizing_identity: authorizer,
    }
}

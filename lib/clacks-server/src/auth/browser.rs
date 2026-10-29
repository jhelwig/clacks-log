//! Account login operations called by the browser's Leptos server actions.

use super::{error::AuthError, oidc, pending::Purpose};
use crate::{runtime::ServerState, store::logins::LoginReference};
use axum_session_sqlx::SessionPgSession;

/// Start normal sign-in for a configured provider, retaining its callback context.
#[tracing::instrument(skip_all, name = "account.login.start")]
pub async fn sign_in(
    state: &ServerState,
    session: &SessionPgSession,
    provider: String,
) -> Result<url::Url, AuthError> {
    oidc::authorization_url(state, session, provider, Purpose::SignIn).await
}

/// End only this browser's Clacks login and discard its middleware session.
pub async fn sign_out(state: &ServerState, session: &SessionPgSession) -> Result<(), AuthError> {
    if let Some(reference) = session.get::<String>("login_reference") {
        state
            .store
            .logout(&LoginReference::from_session(reference))
            .await
            .map_err(AuthError::from_store)?;
    }
    session.destroy();
    Ok(())
}

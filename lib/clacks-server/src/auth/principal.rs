//! The account and login identity authorized for a request.

use crate::{auth::error::AuthError, runtime::ServerState, store::logins::LoginReference};
use axum::{extract::FromRequestParts, http::request::Parts};
use axum_session_sqlx::SessionPgSession;
use clacks_model::ids::AccountId;
use ulid::Ulid;

/// Live authentication context; never contains provider or session secrets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Principal {
    /// Stable internal account owning this request's data.
    pub account_id: AccountId,
    /// Linked identity used to establish this login.
    pub identity_id: Ulid,
}

impl FromRequestParts<ServerState> for Principal {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &ServerState,
    ) -> Result<Self, Self::Rejection> {
        let session = SessionPgSession::from_request_parts(parts, state)
            .await
            .map_err(|_| AuthError::Unavailable)?;
        let reference = session
            .get::<String>("login_reference")
            .ok_or(AuthError::SignInRequired)?;
        state
            .store
            .authenticate(&LoginReference::from_session(reference), state.lifetime())
            .await
            .map_err(AuthError::from_store)?
            .ok_or(AuthError::SignInRequired)
    }
}

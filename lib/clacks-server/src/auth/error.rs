//! Safe HTTP authentication outcomes without secret-bearing provider diagnostics.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

/// User-visible account request failure; internal secrets never enter its text.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    /// A safe identity refusal the account owner can resolve.
    #[error("{0}")]
    Conflict(String),
    /// A live authoritative login is required.
    #[error("Sign in to continue.")]
    SignInRequired,
    /// Callback verification or action context failed.
    #[error("Authorization failed or expired. Restart the action.")]
    InvalidCallback,
    /// Local persistence is temporarily unavailable.
    #[error("Account storage is unavailable. Try again later.")]
    Unavailable,
    /// The selected provider could not complete authentication.
    #[error("The identity provider could not complete sign-in. Try again.")]
    Provider,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::SignInRequired => StatusCode::UNAUTHORIZED,
            Self::InvalidCallback => StatusCode::BAD_REQUEST,
            Self::Unavailable | Self::Provider => StatusCode::SERVICE_UNAVAILABLE,
        };
        (status, self.to_string()).into_response()
    }
}

impl AuthError {
    /// Explain domain refusals without exposing database or provider payloads.
    pub(crate) fn from_store(error: anyhow::Error) -> Self {
        match error.downcast_ref::<crate::store::identities::IdentityError>() {
            Some(error) => {
                tracing::info!(reason = %error, "Identity action refused");
                Self::Conflict(error.to_string())
            }
            None => {
                let code = error
                    .downcast_ref::<sqlx::Error>()
                    .and_then(sqlx::Error::as_database_error)
                    .and_then(|database| database.code());
                tracing::error!(
                    sqlstate = code.as_deref(),
                    "Account storage operation failed"
                );
                Self::Unavailable
            }
        }
    }
}

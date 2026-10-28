//! Safe HTTP authentication outcomes without secret-bearing provider diagnostics.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

/// User-visible account request failure; internal secrets never enter its text.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
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
            Self::SignInRequired => StatusCode::UNAUTHORIZED,
            Self::InvalidCallback => StatusCode::BAD_REQUEST,
            Self::Unavailable | Self::Provider => StatusCode::SERVICE_UNAVAILABLE,
        };
        (status, self.to_string()).into_response()
    }
}

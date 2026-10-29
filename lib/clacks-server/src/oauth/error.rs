//! Safe native authorization responses without credential or proof disclosure.

use axum::{
    Json,
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};

/// Native request authorization failure classified at the application boundary.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ClientAuthError {
    /// Absent, expired, revoked or otherwise invalid credential/proof.
    #[error("Client authorization is invalid. Authorize this client again if needed.")]
    Invalid,
    /// A live credential lacks the requested application scope.
    #[error("Client authorization does not include synchronization access.")]
    Scope,
    /// Authoritative authorization state could not be read.
    #[error("Client authorization is temporarily unavailable. Try again later.")]
    Unavailable,
}

impl IntoResponse for ClientAuthError {
    fn into_response(self) -> Response {
        let (status, code, challenge) = match self {
            Self::Invalid => (
                StatusCode::UNAUTHORIZED,
                "invalid_token",
                "DPoP error=\"invalid_token\"",
            ),
            Self::Scope => (
                StatusCode::FORBIDDEN,
                "insufficient_scope",
                "DPoP error=\"insufficient_scope\", scope=\"sync\"",
            ),
            Self::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "temporarily_unavailable",
                "DPoP",
            ),
        };
        let mut response = (
            status,
            Json(serde_json::json!({"error": code, "error_description": self.to_string()})),
        )
            .into_response();
        response
            .headers_mut()
            .insert("www-authenticate", HeaderValue::from_static(challenge));
        response
    }
}

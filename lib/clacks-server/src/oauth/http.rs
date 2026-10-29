//! Mounted issuer capabilities and the native client identity resource.

use super::error::ClientAuthError;
use crate::runtime::ServerState;
use axum::{
    Json, Router,
    extract::{Request, State},
    http::{HeaderMap, Method, Uri},
    response::Response,
    routing::{get, post},
};

/// Mount native endpoints separately from browser-only mutation-origin checking.
pub(crate) fn router() -> Router<ServerState> {
    Router::new()
        .route("/.well-known/oauth-authorization-server", get(metadata))
        .route("/oauth/token", post(issuer))
        .route("/oauth/revoke", post(issuer))
        .route("/api/client", get(identity))
}

/// Delegate token and revocation parsing/processing to the standard issuer service.
async fn issuer(State(state): State<ServerState>, request: Request) -> Response {
    state
        .oauth_http
        .handle(request)
        .await
        .map(axum::body::Body::new)
}

/// Advertise the supported subset of the library's public metadata.
async fn metadata(
    State(state): State<ServerState>,
) -> Result<Json<serde_json::Value>, ClientAuthError> {
    let mut value =
        serde_json::to_value(state.issuer.metadata()).map_err(|_| ClientAuthError::Unavailable)?;
    let object = value.as_object_mut().ok_or(ClientAuthError::Unavailable)?;
    object.remove("device_authorization_endpoint");
    object.remove("introspection_endpoint");
    object.insert(
        "grant_types_supported".into(),
        serde_json::json!(["authorization_code", "refresh_token"]),
    );
    object.insert(
        "token_endpoint_auth_methods_supported".into(),
        serde_json::json!(["none"]),
    );
    object.insert("scopes_supported".into(), serde_json::json!(["sync"]));
    Ok(Json(value))
}

/// Return authoritative account/actor identity without claiming an installed sync checkpoint.
async fn identity(
    State(state): State<ServerState>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
) -> Result<Json<clacks_sync::protocol::ClientIdentity>, ClientAuthError> {
    let client = super::resource::authorize(&state, &headers, &method, &uri).await?;
    Ok(Json(clacks_sync::protocol::ClientIdentity {
        account: client.account,
        actor: client.actor,
    }))
}

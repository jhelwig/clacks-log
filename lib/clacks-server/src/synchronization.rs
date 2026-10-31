//! Authenticated account WebSockets over the concrete native synchronization stores.

mod connection;

use crate::{
    oauth::{error::ClientAuthError, resource},
    runtime::ServerState,
};
use axum::{
    Router,
    extract::{State, WebSocketUpgrade},
    http::{HeaderMap, Method, Uri},
    response::Response,
    routing::get,
};

/// Mount the native account endpoint independently of browser-cookie mutation handling.
pub(crate) fn router() -> Router<ServerState> {
    Router::new().route("/api/sync", get(upgrade))
}

/// Authorize the public HTTPS upgrade once, then track its full connection lifetime.
async fn upgrade(
    State(state): State<ServerState>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    upgrade: WebSocketUpgrade,
) -> Result<Response, ClientAuthError> {
    let client = resource::authorize(&state, &headers, &method, &uri).await?;
    if state.sync_stop.is_cancelled() {
        return Err(ClientAuthError::Unavailable);
    }
    let token = state.sync_tasks.token();
    Ok(upgrade
        .max_message_size(8 * 1024 * 1024)
        .max_frame_size(8 * 1024 * 1024)
        .on_upgrade(move |socket| async move {
            let _token = token;
            connection::run(socket, state, client).await;
        }))
}

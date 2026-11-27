//! Authenticated account WebSockets over the concrete native synchronization stores.

/// Metric name for one completed server synchronization message dispatch.
pub(crate) const SYNC_DISPATCH_DURATION_SECONDS_METRIC: &str =
    "clacks_sync_dispatch_duration_seconds";

mod connection;
mod contracts;

use crate::{
    oauth::{error::ClientAuthError, resource},
    runtime::ServerState,
};
use axum::{
    Router,
    extract::{State, WebSocketUpgrade},
    http::{HeaderMap, Method, Uri},
    response::{IntoResponse, Response},
    routing::get,
};
use clacks_sync::protocol::contracts::SyncContract;
use opentelemetry::trace::{FutureExt, TraceContextExt};
use strum::IntoEnumIterator;

/// Mount the native account endpoint independently of browser-cookie mutation handling.
pub(crate) fn router() -> Router<ServerState> {
    Router::new()
        .route("/api/sync", get(upgrade))
        .route("/api/sync/capabilities", get(contracts::capabilities))
}

/// Authorize the public HTTPS upgrade once, then track its full connection lifetime.
async fn upgrade(
    State(state): State<ServerState>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    upgrade: WebSocketUpgrade,
) -> Response {
    let client = match resource::authorize(&state, &headers, &method, &uri).await {
        Ok(client) => client,
        Err(error) => return error.into_response(),
    };
    if state.sync_stop.is_cancelled() {
        return ClientAuthError::Unavailable.into_response();
    }

    let upgrade = upgrade.protocols(SyncContract::iter().map(SyncContract::id));
    let selected = upgrade
        .selected_protocol()
        .and_then(|value| value.to_str().ok())
        .and_then(SyncContract::from_id);
    let Some(contract) = selected else {
        tracing::info!(event="sync.contract.incompatible", account_id=%client.account, actor_id=%client.actor, "No accepted sync contract was offered");
        return (
            axum::http::StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({
                "error": "sync_contract_incompatible",
                "error_description": "No accepted synchronization contract was offered."
            })),
        )
            .into_response();
    };
    tracing::info!(event="sync.contract.selected", account_id=%client.account, actor_id=%client.actor, ?contract);

    let context = opentelemetry::Context::new().with_remote_span_context(
        opentelemetry::Context::current()
            .span()
            .span_context()
            .clone(),
    );
    let token = state.sync_tasks.token();

    upgrade
        .max_message_size(8 * 1024 * 1024)
        .max_frame_size(8 * 1024 * 1024)
        .on_upgrade(move |socket| async move {
            let _token = token;
            match contract {
                SyncContract::V1 => {
                    connection::run(socket, state, client)
                        .with_context(context)
                        .await
                }
            }
        })
}

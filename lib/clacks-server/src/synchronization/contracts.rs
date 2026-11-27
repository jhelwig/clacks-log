//! Public discovery of the deployment's implemented synchronization contracts.

use axum::{Json, http::header::CACHE_CONTROL, response::IntoResponse};
use clacks_sync::protocol::contracts::SyncContract;
use strum::IntoEnumIterator;

/// Public deployment capabilities, independent of account authorization.
pub(super) async fn capabilities() -> impl IntoResponse {
    let sync_contracts = SyncContract::iter()
        .map(SyncContract::id)
        .collect::<Vec<_>>();
    (
        [(CACHE_CONTROL, "no-store")],
        Json(serde_json::json!({ "sync_contracts": sync_contracts })),
    )
}

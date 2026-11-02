//! Safe persistence failure categories for owning request boundaries.

/// Classify concrete causes without exposing SQL, native values or provider error text.
pub(crate) fn error_class(error: &anyhow::Error) -> &'static str {
    match error.downcast_ref::<sqlx::Error>() {
        Some(sqlx::Error::PoolTimedOut) => "storage_pool_timeout",
        Some(sqlx::Error::PoolClosed) => "storage_pool_closed",
        Some(sqlx::Error::Io(_)) => "storage_transport",
        Some(sqlx::Error::Tls(_)) => "storage_tls",
        Some(sqlx::Error::Database(_)) => "storage_database",
        Some(sqlx::Error::ColumnDecode { .. } | sqlx::Error::Decode(_)) => "storage_decode",
        Some(_) => "storage_operation",
        None if error
            .downcast_ref::<clacks_sync::error::MetadataError>()
            .is_some() =>
        {
            "synchronization_metadata"
        }
        None if error.downcast_ref::<serde_json::Error>().is_some() => "stored_json_decode",
        None => "application_invariant",
    }
}

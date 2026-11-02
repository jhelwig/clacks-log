//! Safe classifications at desktop workflow handling boundaries.

/// Preserve typed operational distinctions without rendering arbitrary cause chains.
pub(crate) fn error_class(error: &anyhow::Error) -> &'static str {
    if let Some(error) = error.downcast_ref::<sqlx::Error>() {
        return match error {
            sqlx::Error::PoolTimedOut => "storage_pool_timeout",
            sqlx::Error::Database(_) => "storage_database",
            sqlx::Error::Io(_) => "storage_transport",
            sqlx::Error::Decode(_) | sqlx::Error::ColumnDecode { .. } => "storage_decode",
            _ => "storage_operation",
        };
    }
    if error.downcast_ref::<keyring_core::Error>().is_some() {
        return "credential_access";
    }
    if let Some(error) = error.downcast_ref::<reqwest_oauth::Error>() {
        return if error.is_timeout() {
            "http_timeout"
        } else if error.is_connect() {
            "http_connect"
        } else if error.is_decode() {
            "http_decode"
        } else {
            "http_request"
        };
    }
    if let Some(error) = error.downcast_ref::<tokio_tungstenite::tungstenite::Error>() {
        return socket_error_class(error);
    }
    if error.downcast_ref::<serde_json::Error>().is_some() {
        return "protocol_decode";
    }
    if let Some(error) = error.downcast_ref::<std::io::Error>() {
        return match error.kind() {
            std::io::ErrorKind::PermissionDenied => "file_permission",
            std::io::ErrorKind::NotFound => "file_missing",
            _ => "file_io",
        };
    }
    "application_validation"
}

/// Classify a native transport error without formatting headers, URLs or frames.
pub(crate) fn socket_error_class(error: &tokio_tungstenite::tungstenite::Error) -> &'static str {
    use tokio_tungstenite::tungstenite::Error;
    match error {
        Error::Http(_) => "upgrade_refused",
        Error::Io(_) => "socket_transport",
        Error::Tls(_) => "socket_tls",
        Error::Protocol(_) | Error::Utf8(_) => "socket_protocol",
        Error::ConnectionClosed | Error::AlreadyClosed => "socket_closed",
        _ => "socket_operation",
    }
}

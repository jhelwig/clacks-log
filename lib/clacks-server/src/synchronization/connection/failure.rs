//! Safe protocol diagnostics classified at the owning connection boundary.

use crate::{oauth::error::ClientAuthError, store::discovery::SourceError};
use clacks_sync::protocol::messages::FailureKind;

/// A rejected wire operation with a fixed explanation safe to return to its sender.
#[derive(Debug, thiserror::Error)]
#[error("{reason}")]
pub(super) struct InvalidMessage {
    /// Static diagnostic chosen at the request validation boundary.
    pub(super) reason: &'static str,
}

/// Select the peer-visible category and explanation without exposing arbitrary error chains.
pub(super) fn classify(error: &anyhow::Error) -> (FailureKind, String) {
    if let Some(invalid) = error.downcast_ref::<InvalidMessage>() {
        return (FailureKind::Invalid, invalid.to_string());
    }
    if matches!(
        error.downcast_ref::<ClientAuthError>(),
        Some(ClientAuthError::Invalid | ClientAuthError::Scope)
    ) {
        return (
            FailureKind::Unauthorized,
            "Client authorization is no longer valid. Reauthorize this client if needed.".into(),
        );
    }
    if let Some(source) = error.downcast_ref::<SourceError>() {
        return (FailureKind::Unavailable, source.to_string());
    }
    (
        FailureKind::Unavailable,
        "Server synchronization is unavailable. The client will reconnect; check server diagnostics.".into(),
    )
}

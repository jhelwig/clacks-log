//! Native enrollment workflow and protected credential persistence.

pub(crate) mod credentials;
pub(crate) mod flow;
#[cfg(all(test, any(target_os = "macos", windows)))]
mod tests;

use clacks_client_store::identity::EnrollmentBinding;
use clacks_sync::protocol::ClientIdentity;

/// Non-secret inputs for one explicitly requested browser authorization.
#[derive(Clone, Debug)]
pub(crate) struct EnrollmentRequest {
    /// User-selected deployment origin.
    pub server: String,
    /// User-visible installation label.
    pub label: String,
    /// Native key entry persisted before starting the browser flow.
    pub credential_reference: String,
    /// Existing account/writer that the user is recovering, when present.
    pub expected: Option<EnrollmentBinding>,
}

/// Progress safe to send to the UI without credentials or signing material.
#[derive(Clone, Debug)]
pub(crate) enum EnrollmentStatus {
    /// No enrollment operation is running.
    Idle,
    /// Standard authorization request ready for the operating-system browser.
    Authorizing {
        /// Non-secret browser URL; never emit it to diagnostics.
        authorization_url: String,
    },
    /// Verified credentials or workspace completion are being persisted.
    Saving,
    /// Durable completion succeeded; no synchronization receipt is implied.
    Ready {
        /// Workspace opened or activated by the completed workflow.
        destination: std::path::PathBuf,
    },
    /// Actionable workflow error, retained for explicit retry.
    Failed {
        /// Display-only diagnostic; never log the whole UI state.
        message: String,
    },
}

/// Verified identity and native credential reference for durable local completion.
#[derive(Clone, Debug)]
pub(crate) struct AuthorizedEnrollment {
    /// Account/writer returned by the protected identity endpoint.
    pub identity: ClientIdentity,
    /// Origin verified for this credential exchange.
    pub server: String,
    /// Native entry holding key and complete persistent refresh metadata.
    pub credential_reference: String,
}

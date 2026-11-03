//! Native enrollment workflow and protected credential persistence.

pub(crate) mod completion;
pub(crate) mod credentials;
mod failure;
pub(crate) mod flow;
#[cfg(all(test, any(target_os = "macos", windows)))]
mod tests;
pub(crate) mod worker;

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
    Idle {
        /// Saved attempt or installed binding origin, when present.
        server: Option<String>,
        /// Whether the initiating workspace already has an enrollment.
        enrolled: bool,
    },
    /// Standard authorization request ready for the operating-system browser.
    Authorizing {
        /// Non-secret browser URL; never emit it to diagnostics.
        authorization_url: String,
    },
    /// Verified credentials or workspace completion are being persisted.
    Saving,
    /// Durable completion succeeded; no synchronization receipt is implied.
    Ready,
    /// Actionable workflow error, retained for explicit retry.
    Failed {
        /// Whether a connection is already installed despite this failure.
        enrolled: bool,
        /// Display-only diagnostic; never log the whole UI state.
        message: String,
        /// Sanitized diagnostic details, excluding remote payloads and secrets.
        details: Option<String>,
    },
}

impl Default for EnrollmentStatus {
    fn default() -> Self {
        Self::Idle {
            server: None,
            enrolled: false,
        }
    }
}

/// Explicit non-secret requests admitted by the owning workspace session.
#[derive(Debug)]
pub(crate) enum EnrollmentCommand {
    /// Connect or reauthorize using existing internal progress when available.
    Connect {
        /// Selected deployment origin.
        server: String,
        /// User-visible installation label.
        label: String,
    },
    /// Stop the active listener/workflow without claiming remote rollback.
    Cancel,
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

#[cfg(test)]
mod completion_tests;

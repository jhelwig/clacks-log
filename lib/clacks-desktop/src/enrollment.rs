//! Native enrollment workflow and protected credential persistence.

pub(crate) mod completion;
pub(crate) mod credentials;
pub(crate) mod flow;
#[cfg(all(test, any(target_os = "macos", windows)))]
mod tests;
pub(crate) mod worker;
#[cfg(test)]
#[path = "enrollment/worker_tests.rs"]
mod worker_tests;

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
        /// Whether a durable attempt can be resumed.
        resume_available: bool,
        /// Saved destination proposed by the current attempt.
        destination: Option<std::path::PathBuf>,
        /// The attempt is known not to have installed a destination binding.
        destination_change_available: bool,
        /// Whether an unfinished pre-binding attempt may be explicitly abandoned.
        discard_available: bool,
    },
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
        /// A saved attempt remains available for explicit retry.
        resume_available: bool,
        /// Saved destination proposed by the current attempt.
        destination: Option<std::path::PathBuf>,
        /// The rejected destination may be corrected without another authorization.
        destination_change_available: bool,
        /// Only a pre-binding attempt can be abandoned without removing an installed key.
        discard_available: bool,
    },
}

impl Default for EnrollmentStatus {
    fn default() -> Self {
        Self::Idle {
            server: None,
            enrolled: false,
            resume_available: false,
            discard_available: false,
            destination: None,
            destination_change_available: false,
        }
    }
}

/// Explicit non-secret requests admitted by the owning workspace session.
#[derive(Debug)]
pub(crate) enum EnrollmentCommand {
    /// Start browser authorization after persisting a new native proof key.
    Begin {
        /// Selected deployment origin.
        server: String,
        /// User-visible installation label.
        label: String,
        /// Explicit destination override for first enrollment.
        destination: Option<std::path::PathBuf>,
    },
    /// Complete a saved attempt, or explicitly reopen authorization when only its key was saved.
    Resume {
        /// Label shown if a new browser approval is needed.
        label: String,
    },
    /// Explicitly restart browser approval when a saved credential is absent or rejected.
    AuthorizeAgain {
        /// User-visible label for the replacement approval.
        label: String,
    },
    /// Correct an uninstalled attempt's destination without replacing its saved credentials.
    SetDestination {
        /// Explicit directory, or the managed account directory when absent.
        destination: Option<std::path::PathBuf>,
    },
    /// Stop the active listener/workflow without claiming remote rollback.
    Cancel,
    /// Remove an explicitly abandoned pre-binding attempt and its uninstalled native key.
    Discard,
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

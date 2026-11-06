//! Session-only bulk accounting and safe contact-level explanations.
use super::super::failure::{LookupFailure, LookupFailureKind};
use clacks_model::{
    ids::QsoId,
    scalar::{RecordedCallsign, UtcSecond},
};
use std::sync::Arc;

/// Identification captured from the saved contact, never a replacement for its facts.
#[derive(Clone, Debug)]
pub(crate) struct BulkContact {
    /// Original selected identity, including when its parent is no longer available.
    pub id: QsoId,
    /// Recorded callsign if the parent could be read.
    pub callsign: Option<RecordedCallsign>,
    /// Recorded UTC start if the parent could be read.
    pub start: Option<UtcSecond>,
}

/// Mutually exclusive contact outcomes; their total equals the admitted selection.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct BulkCounts {
    /// Committed new snapshots or material changes.
    pub new_updated: u64,
    /// No material change, including newer metadata-only commits.
    pub unchanged: u64,
    /// Provider explicitly returned no station record.
    pub not_found: u64,
    /// Lookup or local application failed.
    pub failed: u64,
    /// Selection members skipped by cancellation, an early stop or ineligibility.
    pub unprocessed: u64,
}

impl BulkCounts {
    /// Number of selection members with a settled outcome, including skipped members.
    pub fn settled(&self) -> u64 {
        self.new_updated + self.unchanged + self.not_found + self.failed + self.unprocessed
    }
}

/// Secret-safe failure with the boundary needed to choose corrective guidance.
#[derive(Clone, Debug, thiserror::Error)]
pub(crate) enum BulkFailure {
    /// Existing typed provider or lookup failure.
    #[error("{failure}")]
    Lookup {
        /// Safe diagnosis from the shared lookup boundary.
        failure: LookupFailure,
    },
    /// The currently selected protected material was rejected.
    #[error("Directory credentials were rejected")]
    Authentication,
    /// Directory selection/material is absent or unusable.
    #[error("{message}")]
    Configuration {
        /// Safe current configuration explanation.
        message: String,
    },
    /// Local application or reading failed, independently of provider success.
    #[error("Local directory snapshot storage failed: {message}")]
    Storage {
        /// Safe local error, excluding provider URLs and payloads.
        message: String,
    },
}

impl BulkFailure {
    /// Explain the next step without claiming an unobserved cause or automatic recovery.
    pub fn guidance(&self) -> &'static str {
        match self {
            Self::Authentication => {
                "Check or update the account's directory credentials, then explicitly retry."
            }
            Self::Configuration { .. } => {
                "Check the account directory configuration and directory-storage diagnostics."
            }
            Self::Storage { .. } => {
                "Check local workspace storage and diagnostics before retrying."
            }
            Self::Lookup { failure } => match failure.kind {
                LookupFailureKind::Transport => {
                    "Check connectivity and retry later. This may be temporary."
                }
                LookupFailureKind::RateLimited => {
                    "The provider is limiting requests. Retry later, respecting its quiet period."
                }
                LookupFailureKind::Subscription => {
                    "Check directory access or subscription on the selected provider account."
                }
                LookupFailureKind::Refusal => {
                    "Check provider account and service status. Whether a later retry will succeed is unknown."
                }
                LookupFailureKind::Parse
                | LookupFailureKind::InvalidResponse
                | LookupFailureKind::InvalidFacts => {
                    "Retry later or investigate the provider response problem if it persists."
                }
                LookupFailureKind::Configuration => {
                    "Check account directory configuration or retry directory storage."
                }
                LookupFailureKind::Internal => "Check workspace diagnostics before retrying.",
            },
        }
    }
}

/// Why one selected contact has no applicable completed lookup outcome.
#[derive(Clone, Debug, thiserror::Error)]
pub(crate) enum UnprocessedReason {
    /// Parent was absent or deleted at capture/application.
    #[error("Contact is no longer available")]
    ParentUnavailable,
    /// A different station is now recorded on the contact.
    #[error("The contact's directory lookup key changed")]
    LookupKeyChanged,
    /// User or workspace shutdown cancelled this consumer.
    #[error("Lookup was cancelled")]
    Cancelled,
    /// The run stopped after a shared failure.
    #[error("Lookup stopped: {failure}")]
    Stopped {
        /// Shared stopping cause, retained without secret sources.
        failure: BulkFailure,
    },
}

/// Only failure and unprocessed contacts need retained individual report rows.
#[derive(Clone, Debug)]
pub(crate) enum BulkDetailOutcome {
    /// An actual failed contact operation.
    Failed {
        /// Safe cause and corrective guidance.
        failure: BulkFailure,
    },
    /// Contact did not receive a completed applicable lookup.
    Unprocessed {
        /// Explicit skip reason.
        reason: UnprocessedReason,
    },
}

/// Captured contact identification beside its non-success diagnosis.
#[derive(Clone, Debug)]
pub(crate) struct BulkDetail {
    /// Original saved identity/caption.
    pub contact: BulkContact,
    /// Failure or skipped-contact explanation.
    pub outcome: BulkDetailOutcome,
}

/// Terminal batch state, distinct from individual contact outcomes.
#[derive(Clone, Debug)]
pub(crate) enum BulkStop {
    /// All admitted members were considered.
    Completed,
    /// User or workspace requested cancellation.
    Cancelled,
    /// A shared problem stopped the remaining members.
    Stopped {
        /// Actual cause, not an automatic retry instruction.
        reason: BulkFailure,
    },
    /// Target preparation failed before any bulk snapshot writes.
    PreparationFailed {
        /// Safe read/admission diagnosis.
        message: String,
    },
}

/// Final report owned by the session UI until explicitly dismissed or replaced.
#[derive(Clone, Debug)]
pub(crate) struct BulkReport {
    /// Count captured when the selection was admitted.
    pub total: u64,
    /// Exactly one classification for each selected identity.
    pub counts: BulkCounts,
    /// Only failures and skipped contacts, in captured selection order.
    pub details: Vec<BulkDetail>,
    /// Whole-run completion or stopping state.
    pub stop: BulkStop,
}

/// Compact latest progress; full detail lists are published only once at settlement.
#[derive(Clone, Debug)]
pub(crate) enum BulkUpdate {
    /// Selection is being resolved, before provider waits or writes.
    Preparing {
        /// Original selected count, including unavailable parents.
        total: u64,
    },
    /// Complete outcomes already settled by this run.
    Running {
        /// Original count, independent of later filter/selection edits.
        total: u64,
        /// Compact outcome totals with no growing detail clone.
        counts: BulkCounts,
    },
    /// Final authoritative result retained cheaply across redraws.
    Finished(Arc<BulkReport>),
}

//! Diagnostics for invalid metadata and unavailable ordering evidence.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Reason an ordering comparison cannot establish a relationship.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnknownReason {
    /// The referenced baseline interpretation is unavailable.
    MissingBaseline,
    /// A timeline relationship is not retained.
    MissingLineage,
    /// The original record value is unavailable.
    MissingOriginal,
    /// Required ordering detail has been collected.
    CollectedContext,
}

/// Invalid synchronization metadata with caller-visible diagnostic context.
#[derive(Debug, Error)]
pub enum MetadataError {
    /// Two inputs belong to different account or record domains.
    #[error("synchronization scope mismatch: {detail}")]
    Scope {
        /// Compared domains or references.
        detail: String,
    },
    /// Two representations claim incompatible values for one identity.
    #[error("conflicting metadata: {detail}")]
    Conflict {
        /// Conflicting identity and observed facts.
        detail: String,
    },
    /// A monotonically increasing counter has no next value.
    #[error("counter exhausted: {counter}")]
    Overflow {
        /// Counter that cannot advance.
        counter: String,
    },
    /// A metadata invariant was violated.
    #[error("invalid metadata: {detail}")]
    Invalid {
        /// Invariant and observed values.
        detail: String,
    },
    /// A completion claim lacks required evidence.
    #[error("incomplete coverage: {detail}")]
    Incomplete {
        /// Missing record, version or interpretation.
        detail: String,
    },
}

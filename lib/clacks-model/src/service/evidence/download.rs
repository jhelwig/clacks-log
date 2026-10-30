//! Maintained destination acquisition coordination and coverage-qualified progress.

use super::observation::QueryIdentity;
use crate::{
    ids::{AcquisitionId, DestinationId, DownloadStateId, ObservationId},
    scalar::{UtcSecond, UtcTimestamp},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Completed change watermark scoped to exact query/filter/field semantics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamProgress {
    /// Query semantics whose requested coverage completed.
    pub query: QueryIdentity,
    /// Provider change watermark, never a pagination cursor.
    pub watermark: Option<UtcSecond>,
}

/// One coordination row per remote destination, shared by scheduled and manual requests.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadControl {
    /// Stable identity derived from the destination, prohibiting alternate coordination rows.
    pub id: DownloadStateId,
    /// Remote account/logbook governed by this row.
    pub destination: DestinationId,
    /// Currently authorized acquisition; no implicit expiration or lease.
    pub active: Option<AcquisitionId>,
    /// Independently completed stream/filter/field watermarks.
    pub progress: Vec<StreamProgress>,
}

impl DownloadControl {
    /// Construct empty coordination using the destination's stable ID bytes.
    pub fn new(destination: DestinationId) -> anyhow::Result<Self> {
        Ok(Self {
            id: destination.to_string().parse()?,
            destination,
            active: None,
            progress: Vec::new(),
        })
    }
    /// Validate deterministic identity and progress scope before accepting stored state.
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.id.to_string() == self.destination.to_string(),
            "download control identity must equal destination identity"
        );
        for (n, p) in self.progress.iter().enumerate() {
            anyhow::ensure!(
                p.query.destination == self.destination,
                "progress crosses destination"
            );
            anyhow::ensure!(
                !self
                    .progress
                    .iter()
                    .take(n)
                    .any(|other| same_stream(&p.query, &other.query)),
                "duplicate query progress"
            );
        }
        Ok(())
    }
}

/// Whether a request remains active or closed with a concrete outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AcquisitionPhase {
    /// Response/page processing may continue.
    Active,
    /// All requested input and coverage completed at the supplied time.
    Completed {
        /// Explicit caller-supplied completion time.
        at: UtcTimestamp,
    },
    /// Processing closed without a completed-coverage claim.
    Failed {
        /// Explicit caller-supplied failure/cancellation time.
        at: UtcTimestamp,
        /// Retained diagnostic for the incomplete acquisition.
        reason: String,
    },
}

/// Recoverable query coordination with one coherent acquisition outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquisitionState {
    /// Stable request identity.
    pub id: AcquisitionId,
    /// Exact public query parameters.
    pub query: QueryIdentity,
    /// Active processing or a closed outcome with its required timestamp.
    pub phase: AcquisitionPhase,
    /// Diagnostic start time supplied by the caller.
    pub started_at: UtcTimestamp,
    /// Semantic occurrences retained for this response, including reused evidence.
    pub retained_inputs: BTreeSet<ObservationId>,
}

impl AcquisitionState {
    /// Return the explicit close time, or absence while processing remains active.
    pub fn finished_at(&self) -> Option<UtcTimestamp> {
        match self.phase {
            AcquisitionPhase::Active => None,
            AcquisitionPhase::Completed { at } | AcquisitionPhase::Failed { at, .. } => Some(at),
        }
    }

    /// Report whether the entire requested input and its coverage completed.
    pub fn complete(&self) -> bool {
        matches!(self.phase, AcquisitionPhase::Completed { .. })
    }
}

/// Compare watermark domains while excluding the since lower bound itself.
pub fn same_stream(a: &QueryIdentity, b: &QueryIdentity) -> bool {
    a.destination == b.destination
        && a.stream == b.stream
        && a.selected_fields == b.selected_fields
        && a.own_call == b.own_call
}

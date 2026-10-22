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

/// Whether a request is still open or was explicitly closed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AcquisitionPhase {
    /// Response/page processing may continue.
    Active,
    /// All requested input is represented durably and coverage was completed.
    Completed,
    /// Explicit failure/cancellation, with no completed-coverage claim.
    Failed(String),
}

/// Recoverable query coordination; original provenance is also embedded in semantic observations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquisitionState {
    /// Stable request identity.
    pub id: AcquisitionId,
    /// Exact public query parameters.
    pub query: QueryIdentity,
    /// Current explicit phase.
    pub phase: AcquisitionPhase,
    /// Diagnostic start time supplied by the caller.
    pub started_at: UtcTimestamp,
    /// Explicit close time, when closed.
    pub finished_at: Option<UtcTimestamp>,
    /// Whether all requested pages/windows completed.
    pub complete: bool,
    /// Semantic occurrences retained for this response, including reused unchanged evidence.
    pub retained_inputs: BTreeSet<ObservationId>,
}

/// Compare watermark domains while excluding the since lower bound itself.
pub fn same_stream(a: &QueryIdentity, b: &QueryIdentity) -> bool {
    a.destination == b.destination
        && a.stream == b.stream
        && a.selected_fields == b.selected_fields
        && a.own_call == b.own_call
}

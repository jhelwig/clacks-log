//! Frozen service submissions used by actual attempts and later coverage comparisons.

use crate::ids::DestinationId;
use crate::service::{destination::Service, lotw::LotwSubmission, qrz::QrzSubmission};
use serde::{Deserialize, Serialize};

/// Complete prepared semantic values for one service destination.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubmissionProjection {
    /// Shared normalized signing input for LoTW.
    Lotw(LotwSubmission),
    /// Ordinary QRZ ADIF field set.
    Qrz(QrzSubmission),
}

impl SubmissionProjection {
    /// Return the collection whose values this submission covers.
    pub fn destination(&self) -> DestinationId {
        match self {
            Self::Lotw(v) => v.destination,
            Self::Qrz(v) => v.destination,
        }
    }

    /// Return the provider without interpreting the mode or destination configuration.
    pub fn service(&self) -> Service {
        match self {
            Self::Lotw(_) => Service::Lotw,
            Self::Qrz(_) => Service::Qrz,
        }
    }
}

/// Retained contact dimensions used to evaluate confirmation after later edits.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmationBasis {
    /// Original band-only label when service interpretation is unavailable.
    #[serde(default)]
    pub uninterpreted_band: Option<String>,
    /// Original effective receive label when service interpretation is unavailable.
    #[serde(default)]
    pub uninterpreted_receive_band: Option<String>,
    /// Historical own full callsign, unknown only for incomplete provider evidence.
    pub own_call: Option<crate::scalar::RecordedCallsign>,
    /// Historical worked full callsign.
    pub worked_call: Option<crate::scalar::RecordedCallsign>,
    /// Historical exact start when available.
    pub start: Option<crate::scalar::UtcSecond>,
    /// Historical primary service band.
    pub band: Option<String>,
    /// Historical effective receive band.
    pub receive_band: Option<String>,
    /// Exact source frequency retained when no service band is recognizable.
    pub frequency: Option<crate::scalar::Hertz>,
    /// Exact receive source frequency when receive band cannot be interpreted.
    pub receive_frequency: Option<crate::scalar::Hertz>,
    /// Concrete mode and available parent/group interpretation.
    pub mode: Option<super::observation::ModeReport>,
}

/// Phase whose occurrence is supported by the retained evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DispatchPhase {
    /// Durable intent; bytes may not have been sent.
    Intent,
    /// Interrupted boundary leaves sending uncertain.
    PossibleSend,
    /// Available transport evidence establishes dispatch.
    Dispatched,
}

/// Frozen semantic values for one real attempted dispatch, independent of later edits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmissionAttempt {
    /// Stable dispatch identity.
    pub id: crate::ids::AttemptId,
    /// Associated account-owned contact.
    pub qso: crate::ids::QsoId,
    /// Actual or possible sent values and destination.
    pub projection: SubmissionProjection,
    /// Dispatch boundary originally recorded.
    pub phase: DispatchPhase,
    /// Time of retained intent/dispatch evidence.
    pub at: crate::scalar::UtcTimestamp,
    /// Optional request batch membership.
    pub batch: Option<crate::ids::SubmissionBatchId>,
}

/// Scope established by a result, preserving upload versus individual processing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResultScope {
    /// Upload-level outcome for exactly these submitted members.
    UploadBatch {
        /// Request correlation identity.
        batch: crate::ids::SubmissionBatchId,
        /// Attempts actually included in the request.
        members: std::collections::BTreeSet<crate::ids::AttemptId>,
    },
    /// Outcome explicitly applying to one attempt.
    IndividualAttempt(crate::ids::AttemptId),
}

/// Outcome facts; an uncertain response is not proof of rejection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubmissionOutcome {
    /// Explicit success at the accompanying scope.
    Success,
    /// Explicit provider rejection with diagnostic detail.
    Rejected(String),
    /// Missing/unusable response or interrupted phase.
    Uncertain(String),
}

/// Independent outcome evidence, including later clarification of an earlier uncertainty.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmissionResult {
    /// Actual member whose returned external identity is established, when batch scope alone is ambiguous.
    #[serde(default)]
    pub external_attempt: Option<crate::ids::AttemptId>,
    /// Stable identity of this outcome fact.
    pub id: crate::ids::SubmissionResultId,
    /// Remote collection returned by this request.
    pub destination: DestinationId,
    /// Actual member-level or upload-level correlation.
    pub scope: ResultScope,
    /// Explicit scope-qualified outcome.
    pub outcome: SubmissionOutcome,
    /// Phase observed by transport/provider handling.
    pub phase: DispatchPhase,
    /// Operational diagnostic time, not causal order.
    pub at: crate::scalar::UtcTimestamp,
    /// Credential-free provider/transport explanation.
    pub diagnostic: Option<String>,
    /// Actual returned record ID when the provider establishes one.
    pub external: Option<crate::service::destination::ExternalRecordKey>,
}

impl ResultScope {
    /// Enumerate the exact attempts whose values this result describes.
    pub fn attempts(&self) -> Vec<crate::ids::AttemptId> {
        match self {
            Self::IndividualAttempt(id) => vec![*id],
            Self::UploadBatch { members, .. } => members.iter().copied().collect(),
        }
    }
}

impl SubmissionResult {
    /// Identify the request member whose remote ID was returned, without guessing across a batch.
    ///
    /// A single-member scope is unambiguous. A larger batch needs explicit member
    /// correlation; absent correlation leaves the remote ID unassociated.
    pub fn identified_attempt(&self) -> anyhow::Result<Option<crate::ids::AttemptId>> {
        let members = self.scope.attempts();
        if let Some(id) = self.external_attempt {
            anyhow::ensure!(
                self.external.is_some() && members.contains(&id),
                "result {} external identity has invalid member correlation {id}",
                self.id
            );
            return Ok(Some(id));
        }
        Ok(if self.external.is_some() && members.len() == 1 {
            members.first().copied()
        } else {
            None
        })
    }
}

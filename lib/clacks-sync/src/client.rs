//! Durable client working values, frozen proposals and client-owned review inputs.

pub mod preparation;
mod reconciliation;
pub mod returned;
mod state;

use reconciliation::{review_reason, witness_evidence};

use crate::{
    account::{EvidenceView, InstalledCheckpoint},
    clock::{ClockActor, RecordVector},
    compare::{ChangePattern, RecordRelation, classify_record_changes, compare_records},
    error::{MetadataError, UnknownReason},
    record::{AcceptedRecord, Observation, RecordKey, RecordState, VersionStamp},
};
use clacks_model::ids::ClientActorId;
use serde::{Deserialize, Serialize};

/// Original values and actual evidence against which local work began.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OriginalBasis {
    /// A previously incorporated accepted version.
    Accepted(AcceptedRecord),
    /// Known local creation, optionally accompanied by certified shared absence.
    NeverCreated {
        /// Account-qualified native identity.
        key: RecordKey,
        /// Complete view establishing shared absence, when already received.
        checkpoint: Option<InstalledCheckpoint>,
    },
    /// Frozen own values known incorporated, without inventing their acceptance position.
    SubsumedOwn {
        /// Account-qualified native identity.
        key: RecordKey,
        /// Own submitted native state.
        state: RecordState,
        /// Own prepared record context.
        vector: RecordVector,
        /// Actual accepted context proving incorporation.
        witness: VersionStamp,
    },
    /// Recovery lacks a usable original.
    Unavailable {
        /// Account-qualified native identity.
        key: RecordKey,
        /// Missing comparison evidence.
        reason: UnknownReason,
    },
}

impl OriginalBasis {
    /// Identify the original's record without inferring a version from current coverage.
    pub fn key(&self) -> RecordKey {
        match self {
            Self::Accepted(v) => v.key,
            Self::NeverCreated { key, .. }
            | Self::SubsumedOwn { key, .. }
            | Self::Unavailable { key, .. } => *key,
        }
    }

    /// Read retained original values; absence and missing originals have no saved state.
    pub fn state(&self) -> Option<&RecordState> {
        match self {
            Self::Accepted(v) => Some(&v.state),
            Self::SubsumedOwn { state, .. } => Some(state),
            _ => None,
        }
    }

    /// Read retained record context for outgoing preparation and compaction witnesses.
    pub fn vector(&self) -> RecordVector {
        match self {
            Self::Accepted(v) => v.stamp.vector().clone(),
            Self::SubsumedOwn {
                vector, witness, ..
            } => vector.join(witness.vector()),
            _ => RecordVector::default(),
        }
    }
}

/// Unpublished local work and at most one unsettled frozen version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingRecord {
    /// Account-qualified native identity.
    pub key: RecordKey,
    /// Retained original values and context.
    pub original: OriginalBasis,
    /// Current local saved working state.
    pub working: RecordState,
    /// Frozen transmitted version, independent of subsequent edits.
    pub prepared: Option<PreparedChange>,
    /// Self-contained original comparison evidence retained before baseline collection.
    pub witness: Option<crate::compaction::BasisWitness>,
}

/// Client rows saved together when shared state or local work changes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ClientRecordFields")]
pub struct ClientRecord {
    /// Account-qualified native identity.
    pub(crate) key: RecordKey,
    /// Latest incorporated shared observation.
    pub(crate) latest: Observation,
    /// Incomparable accepted envelopes awaiting context reconciliation, not edit history.
    pub(crate) retained: Vec<AcceptedRecord>,
    /// Durable unpublished working state, when present.
    pub(crate) pending: Option<PendingRecord>,
}

/// Unvalidated input for a complete client working-state replacement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientRecordFields {
    /// Account-qualified native identity.
    pub key: RecordKey,
    /// Latest incorporated shared observation.
    pub latest: Observation,
    /// Incomparable accepted envelopes awaiting context reconciliation, not edit history.
    pub retained: Vec<AcceptedRecord>,
    /// Durable unpublished working state, when present.
    pub pending: Option<PendingRecord>,
}

/// Durable counter reservation for one account, record and client actor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalCounter {
    /// Writer whose counters are reserved.
    pub actor: ClientActorId,
    /// Greatest counter already reserved or observed.
    pub high_water: u64,
}

/// Shared state required by a conditional write.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersionCondition {
    /// Certified absence, with the completed comparison view retained.
    Absent {
        /// Identity required to be absent.
        key: RecordKey,
        /// Absence certification.
        checkpoint: InstalledCheckpoint,
    },
    /// Exact accepted version required to remain current.
    Is {
        /// Identity whose version is compared.
        key: RecordKey,
        /// Compared accepted context.
        stamp: VersionStamp,
    },
}

impl VersionCondition {
    /// Check the counter domain of the accepted or absent comparison basis.
    pub fn validate(&self) -> Result<(), crate::error::MetadataError> {
        match self {
            Self::Absent { checkpoint, .. } => checkpoint.validate(),
            Self::Is { .. } => Ok(()),
        }
    }

    /// Identify the guarded record for transaction dependency checks.
    pub fn key(&self) -> RecordKey {
        match self {
            Self::Absent { key, .. } | Self::Is { key, .. } => *key,
        }
    }

    /// Build an exact write condition from a validated shared observation.
    pub fn from_observation(observation: &Observation) -> anyhow::Result<Self> {
        observation.validate()?;
        match observation {
            Observation::Accepted(v) => Ok(Self::Is {
                key: v.key,
                stamp: v.stamp.clone(),
            }),
            Observation::Absent { key, checkpoint } => Ok(Self::Absent {
                key: *key,
                checkpoint: checkpoint.clone(),
            }),
            Observation::Unavailable { key, .. } => Err(MetadataError::Incomplete {
                detail: format!("no conditional write basis for {key:?}"),
            }
            .into()),
        }
    }

    /// Check the exact compared identity/context against a current store observation.
    pub fn matches(&self, observation: &Observation) -> bool {
        match (self, observation) {
            (Self::Is { key, stamp }, Observation::Accepted(v)) => {
                *key == v.key && *stamp == v.stamp
            }
            (
                Self::Absent { key, checkpoint },
                Observation::Absent {
                    key: other,
                    checkpoint: current,
                },
            ) => key == other && checkpoint.account == current.account,
            _ => false,
        }
    }
}

/// Frozen outgoing record values and their allocated client version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "PreparedChangeFields")]
pub struct PreparedChange {
    /// Account-qualified native identity.
    pub(crate) key: RecordKey,
    /// Native state frozen for this version.
    pub(crate) state: RecordState,
    /// Original context plus the reserved writer counter.
    pub(crate) vector: RecordVector,
    /// Client submitting this new proposal.
    pub(crate) actor: ClientActorId,
    /// Shared version against which the values were approved.
    pub(crate) expected: VersionCondition,
    /// Compared native reference conditions.
    pub(crate) dependencies: Vec<VersionCondition>,
}

/// Unvalidated values for restoring or preparing a frozen outgoing proposal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedChangeFields {
    /// Account-qualified native identity.
    pub key: RecordKey,
    /// Native state frozen for this version.
    pub state: RecordState,
    /// Original context plus the reserved writer counter.
    pub vector: RecordVector,
    /// Client submitting this new proposal.
    pub actor: ClientActorId,
    /// Shared version against which the values were approved.
    pub expected: VersionCondition,
    /// Compared native reference conditions.
    pub dependencies: Vec<VersionCondition>,
}

/// Why a preserved local record needs an explicit user choice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewReason {
    /// A pending deletion would leave active shared records referring to its target.
    Referenced {
        /// Bounded examples of the surviving shared references.
        referrers: Vec<RecordKey>,
        /// Full referrer count at the shared acceptance check.
        total: u64,
    },
    /// Both sides changed the original to differing values.
    BothDifferent,
    /// The original value is unavailable.
    MissingOriginal,
    /// Differing versions lack interpretable ordering.
    OrderingUnknown(UnknownReason),
    /// A selected native reference is gone.
    MissingReference(RecordKey),
    /// Automatic admission under the old actor is no longer allowed.
    ExcludedActor(ClientActorId),
}

/// Values and evidence presented to the client's resolution workflow.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewInput {
    /// Actual original values/evidence, when retained.
    pub original: OriginalBasis,
    /// Saved local working values.
    pub local: RecordState,
    /// Compared current shared state.
    pub shared: Observation,
    /// Explanation for requiring a choice.
    pub reason: ReviewReason,
    /// Accepted alternatives retained independently of the genuine common original.
    pub retained: Vec<AcceptedRecord>,
}

/// Client rows and any review input to save together in the integrating transaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientUpdate {
    /// Updated accepted and pending rows.
    pub record: ClientRecord,
    /// Explicit resolution request, when necessary.
    pub review: Option<ReviewInput>,
}

/// Preparation outcome without treating ordinary review or retry as an error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Preparation {
    /// New counter and frozen values to persist atomically before transmission.
    Ready {
        /// Updated durable client rows.
        record: ClientRecord,
        /// Counter reservation to save with the proposal.
        counter: LocalCounter,
        /// Frozen outgoing proposal.
        change: PreparedChange,
    },
    /// The same unsettled version must be retransmitted.
    Retry {
        /// Retained client rows.
        record: ClientRecord,
        /// Identical previously reserved version.
        change: PreparedChange,
    },
    /// A user choice is required before preparation.
    Review {
        /// Preserved client rows.
        record: ClientRecord,
        /// Values and explanation to present.
        input: ReviewInput,
    },
    /// No unpublished native change remains.
    Unchanged {
        /// Current client rows.
        record: ClientRecord,
    },
}

/// Incorporate shared accepted state while preserving originals and later local edits.
///
/// Persist the returned accepted/pending rows together. A review is client-owned;
/// it does not prevent unrelated shared records from being installed.
#[tracing::instrument(
    skip_all,
    name = "sync.client.receive",
    level = "debug",
    fields(
        account_id = %current.key.account,
        record_id = %current.key.id,
    ),
)]
pub fn receive_accepted(
    current: &ClientRecord,
    incoming: &AcceptedRecord,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<ClientUpdate> {
    current.validate()?;
    let sealed = witness_evidence(current, evidence)?;
    let sealed_view = sealed
        .as_ref()
        .map(|(lineage, baselines)| EvidenceView { lineage, baselines });
    let evidence = sealed_view.as_ref().unwrap_or(evidence);
    incoming.validate()?;
    if current.key != incoming.key {
        return Err(MetadataError::Scope {
            detail: format!("incoming key differs from {:?}", current.key),
        }
        .into());
    }
    if let Observation::Accepted(old) = &current.latest {
        crate::service::check_integrity(current.key, &old.state, &incoming.state)?;
    }
    let relation = if let Observation::Accepted(old) = &current.latest {
        Some(compare_records(incoming, old, evidence)?)
    } else {
        None
    };
    if matches!(relation, Some(RecordRelation::Before)) {
        return Ok(ClientUpdate {
            record: current.clone(),
            review: None,
        });
    }
    let mut next = current.clone();
    let mut retained = Vec::new();
    for old in &current.retained {
        if !matches!(
            compare_records(old, incoming, evidence)?,
            RecordRelation::Before | RecordRelation::SameVersion
        ) {
            retained.push(old.clone());
        }
    }
    if matches!(
        relation,
        Some(RecordRelation::Divergent | RecordRelation::Unknown(_))
    ) && let Observation::Accepted(old) = &current.latest
        && !retained.iter().any(|r| r.stamp == old.stamp)
    {
        retained.push(old.clone());
    }
    next.retained = retained;
    next.latest = Observation::Accepted(incoming.clone());
    if next.pending.is_none()
        && matches!(
            relation,
            Some(RecordRelation::Divergent | RecordRelation::Unknown(_))
        )
        && let Observation::Accepted(old) = &current.latest
        && !old.state.same_values(&incoming.state)
    {
        next.pending = Some(PendingRecord {
            witness: None,
            key: current.key,
            original: OriginalBasis::Unavailable {
                key: current.key,
                reason: UnknownReason::MissingOriginal,
            },
            working: old.state.clone(),
            prepared: None,
        });
    }
    if let Some(p) = &mut next.pending {
        if let Some(prepared) = &p.prepared {
            let dot = prepared
                .vector
                .component(ClockActor::Client(prepared.actor));
            let compatible = evidence
                .baseline(current.key.account, incoming.stamp.baseline())?
                .is_some();
            if dot > 0
                && compatible
                && incoming
                    .stamp
                    .vector()
                    .component(ClockActor::Client(prepared.actor))
                    >= dot
            {
                p.original = OriginalBasis::SubsumedOwn {
                    key: p.key,
                    state: prepared.state.clone(),
                    vector: prepared.vector.clone(),
                    witness: incoming.stamp.clone(),
                };
                p.prepared = None;
                p.witness = None;
            } else if !prepared.expected.matches(&next.latest) {
                p.prepared = None;
            }
        }
        let reason = review_reason(p, &next.latest, evidence)?;
        if let Some(reason) = reason {
            tracing::debug!(
                event = "client.review_required",
                account_id = %current.key.account,
                record_id = %current.key.id,
                review_reason = ?reason,
            );
            let review = ReviewInput {
                retained: next.retained.clone(),
                original: p.original.clone(),
                local: p.working.clone(),
                shared: next.latest.clone(),
                reason,
            };
            return Ok(ClientUpdate {
                record: next,
                review: Some(review),
            });
        }
        let original = p.original.state();
        let pattern = classify_record_changes(original, Some(&p.working), Some(&incoming.state));
        if matches!(
            pattern,
            ChangePattern::Unchanged | ChangePattern::SharedOnly | ChangePattern::EqualResults
        ) {
            next.pending = None;
        }
    }
    Ok(ClientUpdate {
        record: next,
        review: None,
    })
}

/// Preserve local and frozen values after a terminal admission/reference rejection.
///
/// Save the returned review alongside the unchanged pending evidence. Reauthorization
/// does not silently change the author of the excluded proposal.
pub fn receive_rejection(
    current: &ClientRecord,
    reason: &crate::acceptance::RejectionReason,
) -> anyhow::Result<ClientUpdate> {
    current.validate()?;
    let Some(p) = &current.pending else {
        return Err(MetadataError::Invalid {
            detail: format!("rejection for {:?} has no pending values", current.key),
        }
        .into());
    };
    let reason = match reason {
        crate::acceptance::RejectionReason::Referenced {
            key,
            referrers,
            total,
        } => {
            anyhow::ensure!(
                *key == current.key && referrers.iter().all(|value| value.account == key.account),
                "blocked deletion crosses record/account"
            );
            ReviewReason::Referenced {
                referrers: referrers.clone(),
                total: *total,
            }
        }
        crate::acceptance::RejectionReason::ServiceCondition { .. } => {
            return Ok(ClientUpdate {
                record: current.clone(),
                review: None,
            });
        }
        crate::acceptance::RejectionReason::ActorClosed { actor, .. }
        | crate::acceptance::RejectionReason::ActorUnknown { actor } => {
            ReviewReason::ExcludedActor(*actor)
        }
        crate::acceptance::RejectionReason::MissingReference { key } => {
            ReviewReason::MissingReference(*key)
        }
    };
    tracing::debug!(
        event = "client.review_required",
        account_id = %current.key.account,
        record_id = %current.key.id,
        review_reason = ?reason,
    );
    Ok(ClientUpdate {
        record: current.clone(),
        review: Some(ReviewInput {
            retained: current.retained.clone(),
            original: p.original.clone(),
            local: p.working.clone(),
            shared: current.latest.clone(),
            reason,
        }),
    })
}

/// Incorporate an explicit stale response and release only the matching rejected frozen proposal.
///
/// A lost response must keep its frozen change for retry. This transition is used only after
/// the server explicitly reports that this exact proposal's comparison failed.
pub fn receive_stale(
    current: &ClientRecord,
    change: &PreparedChange,
    observed: &Observation,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<ClientUpdate> {
    current.validate()?;
    observed.validate()?;
    anyhow::ensure!(
        change.key() == current.key
            && observed.key().account == current.key.account
            && (observed.key() == current.key
                || change
                    .dependencies()
                    .iter()
                    .any(|condition| condition.key() == observed.key())),
        "stale response does not belong to this proposal"
    );
    let mut update = if observed.key() == current.key {
        match observed {
            Observation::Accepted(value) => receive_accepted(current, value, evidence)?,
            _ => {
                let mut record = current.clone();
                if let Observation::Accepted(value) = record.latest()
                    && !record.retained.contains(value)
                {
                    record.retained.push(value.clone());
                }
                record.latest = observed.clone();
                let review = if let Some(pending) = &record.pending {
                    review_reason(pending, observed, evidence)?.map(|reason| ReviewInput {
                        original: pending.original.clone(),
                        local: pending.working.clone(),
                        shared: observed.clone(),
                        reason,
                        retained: record.retained.clone(),
                    })
                } else {
                    None
                };
                ClientUpdate { record, review }
            }
        }
    } else {
        ClientUpdate {
            record: current.clone(),
            review: None,
        }
    };
    if let Some(pending) = &mut update.record.pending
        && pending.prepared.as_ref() == Some(change)
    {
        pending.prepared = None;
    }
    if observed.key() != current.key
        && !matches!(observed, Observation::Accepted(value) if matches!(value.state(), RecordState::Live(_)))
    {
        return receive_rejection(
            &update.record,
            &crate::acceptance::RejectionReason::MissingReference {
                key: observed.key(),
            },
        );
    }
    Ok(update)
}

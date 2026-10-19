//! Durable client working values, frozen proposals and client-owned review inputs.

use crate::{
    account::{EvidenceView, InstalledCheckpoint},
    clock::{ClockActor, RecordVector},
    compare::{ChangePattern, RecordRelation, classify_changes, compare_records},
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
    /// Certified absence before local creation.
    NeverCreated {
        /// Account-qualified native identity.
        key: RecordKey,
        /// Complete view establishing absence.
        checkpoint: InstalledCheckpoint,
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
            Self::Accepted(v) => v.stamp.vector.clone(),
            Self::SubsumedOwn { vector, .. } => vector.clone(),
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
}

/// Client rows saved together when shared state or local work changes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientRecord {
    /// Account-qualified native identity.
    pub key: RecordKey,
    /// Latest incorporated shared observation.
    pub latest: Observation,
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
pub struct PreparedChange {
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

/// Validate account/key consistency and native values in a client row set.
fn validate_client(current: &ClientRecord) -> anyhow::Result<()> {
    current.latest.validate()?;
    if current.latest.key() != current.key {
        return Err(MetadataError::Scope {
            detail: format!("client latest key differs from {:?}", current.key),
        }
        .into());
    }
    if let Some(p) = &current.pending {
        if p.key != current.key || p.original.key() != current.key {
            return Err(MetadataError::Scope {
                detail: format!("pending/original key differs from {:?}", current.key),
            }
            .into());
        }
        p.working.validate(current.key)?;
        if let Some(state) = p.original.state() {
            state.validate(current.key)?;
        }
        if let Some(prepared) = &p.prepared {
            if prepared.key != p.key || prepared.expected.key() != p.key {
                return Err(MetadataError::Scope {
                    detail: format!("prepared key differs from {:?}", p.key),
                }
                .into());
            }
            prepared.state.validate(p.key)?;
        }
    }
    Ok(())
}

/// Determine the review reason using whole-record values and retained ordering.
fn review_reason(
    pending: &PendingRecord,
    shared: &Observation,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<Option<ReviewReason>> {
    let shared_state = match shared {
        Observation::Accepted(v) => Some(&v.state),
        Observation::Absent { .. } => None,
        Observation::Unavailable { reason, .. } => {
            return Ok(Some(ReviewReason::OrderingUnknown(*reason)));
        }
    };
    if shared_state == Some(&pending.working) {
        return Ok(None);
    }
    if let OriginalBasis::Unavailable { .. } = &pending.original {
        return Ok(Some(ReviewReason::MissingOriginal));
    }
    if let (OriginalBasis::Accepted(original), Observation::Accepted(current)) =
        (&pending.original, shared)
    {
        if let RecordRelation::Unknown(reason) = compare_records(original, current, evidence)? {
            return Ok(Some(ReviewReason::OrderingUnknown(reason)));
        }
    }
    let local = Some(&pending.working);
    Ok(
        (classify_changes(&pending.original.state(), &local, &shared_state)
            == ChangePattern::BothDifferent)
            .then_some(ReviewReason::BothDifferent),
    )
}

/// Incorporate shared accepted state while preserving originals and later local edits.
///
/// Persist the returned accepted/pending rows together. A review is client-owned;
/// it does not prevent unrelated shared records from being installed.
pub fn receive_accepted(
    current: &ClientRecord,
    incoming: &AcceptedRecord,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<ClientUpdate> {
    validate_client(current)?;
    incoming.validate()?;
    if current.key != incoming.key {
        return Err(MetadataError::Scope {
            detail: format!("incoming key differs from {:?}", current.key),
        }
        .into());
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
    next.latest = Observation::Accepted(incoming.clone());
    if next.pending.is_none()
        && matches!(
            relation,
            Some(RecordRelation::Divergent | RecordRelation::Unknown(_))
        )
    {
        if let Observation::Accepted(old) = &current.latest {
            if old.state != incoming.state {
                next.pending = Some(PendingRecord {
                    key: current.key,
                    original: OriginalBasis::Unavailable {
                        key: current.key,
                        reason: UnknownReason::MissingOriginal,
                    },
                    working: old.state.clone(),
                    prepared: None,
                });
            }
        }
    }
    if let Some(p) = &mut next.pending {
        if let Some(prepared) = &p.prepared {
            let dot = prepared
                .vector
                .component(ClockActor::Client(prepared.actor));
            let compatible = evidence
                .baseline(current.key.account, incoming.stamp.baseline)?
                .is_some();
            if dot > 0
                && compatible
                && incoming
                    .stamp
                    .vector
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
            } else if !prepared.expected.matches(&next.latest) {
                p.prepared = None;
            }
        }
        let reason = review_reason(p, &next.latest, evidence)?;
        if let Some(reason) = reason {
            let review = ReviewInput {
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
        let pattern = classify_changes(&original, &Some(&p.working), &Some(&incoming.state));
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

/// Reserve one coalesced outgoing version or return preserved retry/review state.
///
/// Save a ready counter and row set atomically before sending. The supplied counter
/// belongs to this record and actor; counter exhaustion returns a concrete error.
pub fn prepare_change(
    current: &ClientRecord,
    actor: ClientActorId,
    counter: &LocalCounter,
    dependencies: &[VersionCondition],
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<Preparation> {
    validate_client(current)?;
    if counter.actor != actor {
        return Err(MetadataError::Scope {
            detail: format!("counter actor {} differs from {actor}", counter.actor),
        }
        .into());
    }
    let Some(p) = &current.pending else {
        return Ok(Preparation::Unchanged {
            record: current.clone(),
        });
    };
    if let Some(change) = &p.prepared {
        if change.actor != actor {
            return Ok(Preparation::Review {
                record: current.clone(),
                input: ReviewInput {
                    original: p.original.clone(),
                    local: p.working.clone(),
                    shared: current.latest.clone(),
                    reason: ReviewReason::ExcludedActor(change.actor),
                },
            });
        }
        return Ok(Preparation::Retry {
            record: current.clone(),
            change: change.clone(),
        });
    }
    if let Some(reason) = review_reason(p, &current.latest, evidence)? {
        return Ok(Preparation::Review {
            record: current.clone(),
            input: ReviewInput {
                original: p.original.clone(),
                local: p.working.clone(),
                shared: current.latest.clone(),
                reason,
            },
        });
    }
    let shared = match &current.latest {
        Observation::Accepted(v) => Some(&v.state),
        _ => None,
    };
    let pattern = classify_changes(&p.original.state(), &Some(&p.working), &shared);
    if matches!(
        pattern,
        ChangePattern::Unchanged | ChangePattern::SharedOnly | ChangePattern::EqualResults
    ) {
        let mut record = current.clone();
        record.pending = None;
        return Ok(Preparation::Unchanged { record });
    }
    let context = match &current.latest {
        Observation::Accepted(v) => p.original.vector().join(&v.stamp.vector),
        _ => p.original.vector(),
    };
    let (vector, high_water) = context.increment(ClockActor::Client(actor), counter.high_water)?;
    for dep in dependencies {
        if dep.key().account != current.key.account {
            return Err(MetadataError::Scope {
                detail: format!("dependency {:?} crosses account", dep.key()),
            }
            .into());
        }
    }
    let change = PreparedChange {
        key: current.key,
        state: p.working.clone(),
        vector,
        actor,
        expected: VersionCondition::from_observation(&current.latest)?,
        dependencies: dependencies.to_vec(),
    };
    let mut record = current.clone();
    if let Some(p) = &mut record.pending {
        p.prepared = Some(change.clone());
    }
    Ok(Preparation::Ready {
        record,
        counter: LocalCounter { actor, high_water },
        change,
    })
}

/// Validate an explicit user-selected result against the compared shared version.
///
/// This creates local pending values; it does not publish or silently restore references.
pub fn resolve_review(
    review: &ReviewInput,
    chosen: RecordState,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<ClientRecord> {
    let key = review.shared.key();
    review.shared.validate()?;
    chosen.validate(key)?;
    if key != review.original.key() {
        return Err(MetadataError::Scope {
            detail: format!("review original differs from {key:?}"),
        }
        .into());
    }
    let original = match &review.shared {
        Observation::Accepted(v) => {
            evidence.baseline(key.account, v.stamp.baseline)?;
            OriginalBasis::Accepted(v.clone())
        }
        Observation::Absent { key, checkpoint } => OriginalBasis::NeverCreated {
            key: *key,
            checkpoint: checkpoint.clone(),
        },
        Observation::Unavailable { .. } => {
            return Err(MetadataError::Incomplete {
                detail: format!("cannot resolve against unavailable shared state for {key:?}"),
            }
            .into());
        }
    };
    Ok(ClientRecord {
        key,
        latest: review.shared.clone(),
        pending: Some(PendingRecord {
            key,
            original,
            working: chosen,
            prepared: None,
        }),
    })
}

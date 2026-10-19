//! Conditional server acceptance with explicit real-store transaction preconditions.

use crate::{
    account::{AccountMetadata, EvidenceView},
    client::{PreparedChange, VersionCondition},
    clock::{ClockActor, RecordVector, VectorRelation},
    error::MetadataError,
    membership::MembershipSnapshot,
    record::{
        AcceptedRecord, Observation, Position, RecordId, RecordKey, RecordState, RecordValue,
        VersionStamp,
    },
};
use clacks_model::ids::ClientActorId;
use serde::{Deserialize, Serialize};

/// Consistent rows loaded within the real acceptance transaction.
pub struct AcceptanceInput<'a> {
    /// Current account head and epoch.
    pub account: &'a AccountMetadata,
    /// Current admission state.
    pub membership: &'a MembershipSnapshot,
    /// Compared target observation.
    pub current: &'a Observation,
    /// Compared native references.
    pub references: &'a [Observation],
}

/// Transfer-journal activity, separate from the envelope's original acceptance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalChange {
    /// Installation/change position in the current timeline.
    pub installed_at: Position,
    /// Changed native identity.
    pub key: RecordKey,
    /// Envelope version installed at that position.
    pub stamp: VersionStamp,
}

/// New rows and conditions that an adapter must commit atomically.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptanceCommit {
    /// Head required to remain current.
    pub expected_head: Position,
    /// Admission epoch required to remain current.
    pub expected_membership_generation: u64,
    /// Compared target version.
    pub expected: VersionCondition,
    /// Compared native references.
    pub dependencies: Vec<VersionCondition>,
    /// Next account head and complete installed coverage.
    pub next_account: AccountMetadata,
    /// Validated accepted native state and context.
    pub accepted: AcceptedRecord,
    /// Whether native values changed, independently of causal metadata.
    pub native_changed: bool,
    /// Journal row to commit with the envelope and head.
    pub journal: JournalChange,
}

/// Internal server-authored change, including initial shared-definition seeds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerChange {
    /// Native identity to create or update.
    pub key: RecordKey,
    /// Approved native values or deletion.
    pub state: RecordState,
    /// Compared target condition.
    pub expected: VersionCondition,
    /// Compared native reference conditions.
    pub dependencies: Vec<VersionCondition>,
}

/// Normal admission/reference failure requiring caller action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectionReason {
    /// The actor's automatic admission lifetime ended.
    ActorClosed {
        /// Excluded actor.
        actor: ClientActorId,
        /// Durable closure position.
        closed_at: Position,
    },
    /// This account has no current enrollment for the actor.
    ActorUnknown {
        /// Unrecognized admission identity.
        actor: ClientActorId,
    },
    /// A selected tag or equipment record no longer exists.
    MissingReference {
        /// Referenced identity requiring explicit removal or replacement.
        key: RecordKey,
    },
}

/// Ordinary conditional-acceptance outcomes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AcceptanceDecision {
    /// Commit all returned rows under their stated preconditions.
    Commit(Box<AcceptanceCommit>),
    /// This client version is already incorporated.
    Covered(AcceptedRecord),
    /// The target or a reference changed; reconcile against this observation.
    Stale(Observation),
    /// Admission or a native reference requires caller action.
    Rejected(RejectionReason),
}

/// Check current admission before either new proposals or retained-state recovery.
pub(crate) fn admission(
    actor: ClientActorId,
    membership: &MembershipSnapshot,
) -> Option<RejectionReason> {
    match membership.clients.iter().find(|c| c.actor == actor) {
        None => Some(RejectionReason::ActorUnknown { actor }),
        Some(c) => c
            .closed_at
            .map(|closed_at| RejectionReason::ActorClosed { actor, closed_at }),
    }
}

/// Plan a new client version; retries already covered by context never mutate again.
pub fn plan_acceptance(
    change: &PreparedChange,
    input: &AcceptanceInput<'_>,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<AcceptanceDecision> {
    input.membership.validate(input.account)?;
    if let Some(reason) = admission(change.actor, input.membership) {
        return Ok(AcceptanceDecision::Rejected(reason));
    }
    validate_input(change.key, &change.state, &change.expected, input, evidence)?;
    let own = ClockActor::Client(change.actor);
    let count = change.vector.component(own);
    if count == 0 {
        return Err(MetadataError::Invalid {
            detail: format!("proposal {:?} has no client contribution", change.key),
        }
        .into());
    }
    if let Observation::Accepted(current) = input.current
        && current.stamp.vector.component(own) >= count
    {
        return Ok(AcceptanceDecision::Covered(current.clone()));
    }
    if let VersionCondition::Is { stamp, .. } = &change.expected
        && (!matches!(
            stamp.vector.compare_components(&change.vector),
            VectorRelation::Before
        ) || count <= stamp.vector.component(own))
    {
        return Err(MetadataError::Invalid {
            detail: format!(
                "proposal {:?} does not extend its compared context",
                change.key
            ),
        }
        .into());
    }
    plan_commit(
        change.key,
        &change.state,
        &change.expected,
        &change.dependencies,
        &change.vector,
        input,
        evidence,
    )
}

/// Plan internal server-origin values without inventing a client enrollment or dot.
pub fn plan_server_acceptance(
    change: &ServerChange,
    input: &AcceptanceInput<'_>,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<AcceptanceDecision> {
    validate_input(change.key, &change.state, &change.expected, input, evidence)?;
    let context = match input.current {
        Observation::Accepted(v) => v.stamp.vector.clone(),
        _ => RecordVector::default(),
    };
    plan_commit(
        change.key,
        &change.state,
        &change.expected,
        &change.dependencies,
        &context,
        input,
        evidence,
    )
}

/// Validate the shared domains and retained interpretation of a conditional change.
fn validate_input(
    key: RecordKey,
    state: &RecordState,
    expected: &VersionCondition,
    input: &AcceptanceInput<'_>,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<()> {
    input.membership.validate(input.account)?;
    input.current.validate()?;
    state.validate(key)?;
    if key.account != input.account.account || key != input.current.key() || key != expected.key() {
        return Err(MetadataError::Scope {
            detail: format!("acceptance target/condition differs from {key:?}"),
        }
        .into());
    }
    if evidence
        .baseline(key.account, input.account.baseline)?
        .is_none()
    {
        return Err(MetadataError::Incomplete {
            detail: format!(
                "account {} baseline {} unavailable",
                key.account, input.account.baseline
            ),
        }
        .into());
    }
    for reference in input.references {
        reference.validate()?;
        if reference.key().account != key.account {
            return Err(MetadataError::Scope {
                detail: format!("reference {:?} crosses account", reference.key()),
            }
            .into());
        }
    }
    Ok(())
}

/// Derive mandatory native reference identities without treating unknown groups as invalid.
fn required_references(key: RecordKey, state: &RecordState) -> Vec<RecordKey> {
    match state {
        RecordState::Live(RecordValue::Qso(q)) => q
            .tags
            .iter()
            .map(|id| RecordKey {
                account: key.account,
                id: RecordId::Tag(*id),
            })
            .collect(),
        RecordState::Live(RecordValue::Profile(p)) => p
            .default_equipment
            .iter()
            .map(|id| RecordKey {
                account: key.account,
                id: RecordId::Equipment(*id),
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Produce a conditional atomic mutation after target and dependency checks.
fn plan_commit(
    key: RecordKey,
    state: &RecordState,
    expected: &VersionCondition,
    dependencies: &[VersionCondition],
    context: &RecordVector,
    input: &AcceptanceInput<'_>,
    _evidence: &EvidenceView<'_>,
) -> anyhow::Result<AcceptanceDecision> {
    if !expected.matches(input.current) {
        return Ok(AcceptanceDecision::Stale(input.current.clone()));
    }
    for dep in dependencies {
        if dep.key().account != key.account {
            return Err(MetadataError::Scope {
                detail: format!("dependency {:?} crosses account", dep.key()),
            }
            .into());
        }
        let found = input.references.iter().find(|r| r.key() == dep.key());
        match found {
            Some(row) if dep.matches(row) => {}
            Some(row) => return Ok(AcceptanceDecision::Stale(row.clone())),
            None => {
                return Ok(AcceptanceDecision::Rejected(
                    RejectionReason::MissingReference { key: dep.key() },
                ));
            }
        }
    }
    for required in required_references(key, state) {
        let supplied = input.references.iter().any(|row| {
            row.key() == required
                && matches!(
                    row,
                    Observation::Accepted(AcceptedRecord {
                        state: RecordState::Live(_),
                        ..
                    })
                )
        });
        if !supplied
            || !dependencies
                .iter()
                .any(|d| d.key() == required && matches!(d, VersionCondition::Is { .. }))
        {
            return Ok(AcceptanceDecision::Rejected(
                RejectionReason::MissingReference { key: required },
            ));
        }
    }
    let next_account = input.account.advance()?;
    let server = ClockActor::Server(next_account.head.timeline);
    if context.component(server) >= next_account.head.revision {
        return Err(MetadataError::Conflict {
            detail: format!("record {key:?} server context is ahead of account head"),
        }
        .into());
    }
    let vector = context.join(&RecordVector::from_entries([(
        server,
        next_account.head.revision,
    )])?);
    let accepted = AcceptedRecord::new(
        key,
        state.clone(),
        VersionStamp {
            accepted_at: next_account.head,
            baseline: input.account.baseline,
            vector,
        },
    )?;
    let native_changed = match input.current {
        Observation::Accepted(old) => old.state != *state,
        _ => true,
    };
    let journal = JournalChange {
        installed_at: next_account.head,
        key,
        stamp: accepted.stamp.clone(),
    };
    Ok(AcceptanceDecision::Commit(Box::new(AcceptanceCommit {
        expected_head: input.account.head,
        expected_membership_generation: input.membership.generation,
        expected: expected.clone(),
        dependencies: dependencies.to_vec(),
        next_account,
        accepted,
        native_changed,
        journal,
    })))
}

/// Commit a value-policy-approved recovery result with both interpretable contexts.
pub(crate) fn plan_reconciled(
    incoming: &AcceptedRecord,
    state: &RecordState,
    input: &AcceptanceInput<'_>,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<AcceptanceDecision> {
    let expected = VersionCondition::from_observation(input.current)?;
    validate_input(incoming.key, state, &expected, input, evidence)?;
    if evidence
        .baseline(incoming.key.account, incoming.stamp.baseline)?
        .is_none()
    {
        return Err(MetadataError::Incomplete {
            detail: "recovery context baseline unavailable".into(),
        }
        .into());
    }
    let context = match input.current {
        Observation::Accepted(v) => v.stamp.vector.join(&incoming.stamp.vector),
        _ => incoming.stamp.vector.clone(),
    };
    plan_commit(
        incoming.key,
        state,
        &expected,
        &[],
        &context,
        input,
        evidence,
    )
}

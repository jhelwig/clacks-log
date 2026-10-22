//! Exceptional rollback boundaries and hub-and-spoke accepted-state recovery.

use crate::{
    acceptance::{
        AcceptanceCommit, AcceptanceDecision, AcceptanceInput, JournalChange, admission,
        plan_reconciled,
    },
    account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
    client::{OriginalBasis, ReviewInput, ReviewReason, VersionCondition},
    compare::{ChangePattern, RecordRelation, classify_changes, compare_records},
    error::{MetadataError, UnknownReason},
    lineage::{BoundaryKind, Coverage, TimelineBoundary},
    membership::MembershipSnapshot,
    record::{AcceptedRecord, Observation, Position},
};
use clacks_model::ids::{AccountId, ClientActorId, TimelineId};

/// Advertised server position compared with a client's durable prior knowledge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryObservation {
    /// Account being synchronized.
    pub account: AccountId,
    /// Current server advertisement.
    pub advertised: Position,
    /// Complete prefixes remembered locally.
    pub remembered: Coverage,
}

/// Revision/ancestry contradiction check, not a universal rollback detector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HistoryCheck {
    /// Available witnesses establish no contradiction.
    NoContradiction,
    /// The advertised timeline is behind a previously observed cut.
    Regression {
        /// Contradictory current advertisement.
        advertised: Position,
        /// Durable evidence of later receipt.
        remembered: Coverage,
    },
    /// The advertised history cannot be interpreted.
    Unknown(UnknownReason),
}

/// Conditional history-boundary installation and corresponding account update.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryBoundaryPlan {
    /// Required current account position.
    pub expected_head: Position,
    /// Required admission epoch.
    pub expected_membership_generation: u64,
    /// Account state to save with the boundary.
    pub next_account: AccountMetadata,
    /// Immutable ancestry relationship to install.
    pub boundary: TimelineBoundary,
}

/// Client-side recovery outcome with explicit server transaction guards.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecoveredDecision {
    /// Service input remains staged until support/order/index conditions can be established.
    DeferredService {
        /// Missing dependency or unsafe binding diagnostic; never a manual QSO edit conflict.
        detail: String,
    },
    /// Install an already accepted version, retaining its original acceptance.
    Install {
        /// Namespace-index conditions required alongside the original envelope.
        external_guards: Vec<crate::service::ExternalBindingGuard>,
        /// Required current account head.
        expected_head: Position,
        /// Required admission epoch.
        expected_membership_generation: u64,
        /// Currently authorized carrier, independently of the original author.
        returning_actor: ClientActorId,
        /// Compared target condition.
        expected: VersionCondition,
        /// Native reference conditions required by the recovered payload.
        dependencies: Vec<VersionCondition>,
        /// Original accepted envelope to retain unchanged.
        envelope: AcceptedRecord,
        /// New-timeline installation activity.
        journal: Box<JournalChange>,
        /// Account head to save atomically with the envelope and journal.
        next_account: AccountMetadata,
    },
    /// Current context already represents the returning version.
    Covered,
    /// Value policy approves a new result covering both branches.
    ///
    /// This is new acceptance of reconciled context, not relabeling a retained envelope.
    Reconciled(Box<AcceptanceCommit>),
    /// Differing versions need an explicit client-owned choice.
    Review(ReviewInput),
}

/// Detect a witnessed same-history regression without claiming detection of every restore.
pub fn check_history(
    observation: &HistoryObservation,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<HistoryCheck> {
    if observation.account != evidence.lineage.account() {
        return Err(MetadataError::Scope {
            detail: "history observation crosses account".into(),
        }
        .into());
    }
    if observation
        .remembered
        .entries()
        .any(|(timeline, revision)| {
            timeline == observation.advertised.timeline
                && revision > observation.advertised.revision
        })
    {
        return Ok(HistoryCheck::Regression {
            advertised: observation.advertised,
            remembered: observation.remembered.clone(),
        });
    }
    if evidence
        .lineage
        .coverage_at(observation.advertised)
        .is_err()
    {
        return Ok(HistoryCheck::Unknown(UnknownReason::MissingLineage));
    }
    Ok(HistoryCheck::NoContradiction)
}

/// Establish a fresh exceptional timeline rooted only in actually restored state.
///
/// Ordinary starts retain their timeline and do not call this transition.
pub fn plan_recovery_fork(
    new_timeline: TimelineId,
    account: &AccountMetadata,
    membership: &MembershipSnapshot,
    known_restored: &Coverage,
) -> anyhow::Result<RecoveryBoundaryPlan> {
    membership.validate(account)?;
    if new_timeline == account.head.timeline
        || account
            .installed
            .entries()
            .any(|(id, _)| id == new_timeline)
    {
        return Err(MetadataError::Conflict {
            detail: format!("recovery timeline {new_timeline} already exists"),
        }
        .into());
    }
    if !known_restored.covers(account.head)
        || known_restored
            .entries()
            .any(|(timeline, revision)| !account.installed.covers(Position { timeline, revision }))
    {
        return Err(MetadataError::Incomplete {
            detail: "fork origin is not the restored installed state".into(),
        }
        .into());
    }
    let at = Position {
        timeline: new_timeline,
        revision: 0,
    };
    let mut next_account = account.clone();
    next_account.head = at;
    next_account.installed = known_restored.join(&Coverage::from(at));
    Ok(RecoveryBoundaryPlan {
        expected_head: account.head,
        expected_membership_generation: membership.generation,
        next_account,
        boundary: TimelineBoundary {
            at,
            includes: known_restored.clone(),
            kind: BoundaryKind::ForkOrigin,
        },
    })
}

/// Retain values and explained uncertainty in a recovery review.
fn review(
    incoming: &AcceptedRecord,
    current: &Observation,
    original: Option<&OriginalBasis>,
    reason: ReviewReason,
) -> RecoveredDecision {
    RecoveredDecision::Review(ReviewInput {
        retained: vec![incoming.clone()],
        original: original.cloned().unwrap_or(OriginalBasis::Unavailable {
            key: incoming.key,
            reason: UnknownReason::MissingOriginal,
        }),
        local: incoming.state.clone(),
        shared: current.clone(),
        reason,
    })
}

/// Plan return of previously server-distributed state through the authorized client.
///
/// The server rechecks all emitted conditions and current carrier admission. The
/// original writer's revocation does not revoke an envelope already accepted.
pub fn plan_recovered_install(
    incoming: &AcceptedRecord,
    original: Option<&OriginalBasis>,
    returning_actor: ClientActorId,
    input: &AcceptanceInput<'_>,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<RecoveredDecision> {
    let (current, account, membership) = (input.current, input.account, input.membership);
    membership.validate(account)?;
    incoming.validate()?;
    current.validate()?;
    if let Observation::Accepted(old) = current {
        crate::service::check_integrity(incoming.key, &old.state, &incoming.state)?;
    }
    let mut reference_keys = std::collections::BTreeSet::new();
    for reference in input.references {
        reference.validate()?;
        if reference.key().account != account.account || !reference_keys.insert(reference.key()) {
            return Err(MetadataError::Scope {
                detail: format!(
                    "duplicate/cross-account recovery reference {:?}",
                    reference.key()
                ),
            }
            .into());
        }
    }
    if incoming.key.account != account.account
        || current.key() != incoming.key
        || original.is_some_and(|o| o.key() != incoming.key)
    {
        return Err(MetadataError::Scope {
            detail: format!("recovery domain differs from {:?}", incoming.key),
        }
        .into());
    }
    if admission(returning_actor, membership).is_some() {
        return Ok(review(
            incoming,
            current,
            original,
            ReviewReason::ExcludedActor(returning_actor),
        ));
    }
    let external_guards =
        match crate::service::dependency::admission(&incoming.state, input, evidence)? {
            Ok(guards) => guards,
            Err(reason) => {
                return Ok(RecoveredDecision::DeferredService {
                    detail: format!("{reason:?}"),
                });
            }
        };
    let relation = match current {
        Observation::Accepted(old) => compare_records(incoming, old, evidence)?,
        Observation::Absent { checkpoint, .. } => {
            if evidence
                .baseline(account.account, checkpoint.baseline)?
                .is_none()
                || evidence
                    .baseline(account.account, incoming.stamp.baseline)?
                    .is_none()
            {
                RecordRelation::Unknown(UnknownReason::MissingBaseline)
            } else if crate::compaction::expanded(checkpoint, evidence)?
                .covers(incoming.stamp.accepted_at)
            {
                return Err(MetadataError::Conflict {
                    detail: format!(
                        "certified absence contradicts retained accepted {:?}",
                        incoming.key
                    ),
                }
                .into());
            } else {
                RecordRelation::After
            }
        }
        Observation::Unavailable { reason, .. } => RecordRelation::Unknown(*reason),
    };
    match relation {
        RecordRelation::SameVersion | RecordRelation::Before => {
            return Ok(RecoveredDecision::Covered);
        }
        RecordRelation::After => {}
        RecordRelation::Divergent | RecordRelation::Unknown(_) => {
            if crate::service::payload(incoming).is_some_and(|s| s.payload.immutable())
                && matches!(relation, RecordRelation::Unknown(_))
            {
                return Ok(RecoveredDecision::DeferredService {
                    detail: format!("original service context cannot be interpreted: {relation:?}"),
                });
            }
            if crate::service::payload(incoming).is_some_and(|s| {
                !matches!(
                    s.payload,
                    clacks_model::service::evidence::ServicePayload::Association(_)
                )
            }) && matches!(current,Observation::Accepted(old) if old.state!=incoming.state)
            {
                return Ok(RecoveredDecision::DeferredService {
                    detail: format!(
                        "service state has unordered alternatives; retain evidence and refresh: {relation:?}"
                    ),
                });
            }
            let shared_state = match current {
                Observation::Accepted(v) => Some(&v.state),
                _ => None,
            };
            let pattern = original
                .map(|o| classify_changes(&o.state(), &Some(&incoming.state), &shared_state));
            let equal = shared_state == Some(&incoming.state);
            if equal
                || (!matches!(relation, RecordRelation::Unknown(_))
                    && matches!(
                        pattern,
                        Some(
                            ChangePattern::LocalOnly
                                | ChangePattern::SharedOnly
                                | ChangePattern::Unchanged
                                | ChangePattern::EqualResults
                        )
                    ))
            {
                let state = if matches!(
                    pattern,
                    Some(ChangePattern::SharedOnly | ChangePattern::Unchanged)
                ) {
                    match current {
                        Observation::Accepted(v) => &v.state,
                        _ => &incoming.state,
                    }
                } else {
                    &incoming.state
                };
                match plan_reconciled(
                    incoming,
                    state,
                    &AcceptanceInput {
                        bindings: input.bindings,
                        account,
                        membership,
                        current,
                        references: input.references,
                    },
                    evidence,
                )? {
                    AcceptanceDecision::Commit(commit) => {
                        return Ok(RecoveredDecision::Reconciled(commit));
                    }
                    AcceptanceDecision::Rejected(
                        crate::acceptance::RejectionReason::MissingReference { key },
                    ) => {
                        return Ok(review(
                            incoming,
                            current,
                            original,
                            ReviewReason::MissingReference(key),
                        ));
                    }
                    _ => {
                        return Err(MetadataError::Conflict {
                            detail: "recovery reconciliation target changed".into(),
                        }
                        .into());
                    }
                }
            }
            let reason = match relation {
                RecordRelation::Unknown(r) => ReviewReason::OrderingUnknown(r),
                _ if original.is_none()
                    || matches!(original, Some(OriginalBasis::Unavailable { .. })) =>
                {
                    ReviewReason::MissingOriginal
                }
                _ => ReviewReason::BothDifferent,
            };
            return Ok(review(incoming, current, original, reason));
        }
    }
    let mut dependencies = Vec::new();
    for key in crate::acceptance::required_references(incoming.key, &incoming.state) {
        let Some(row) = input.references.iter().find(|r| {
            r.key() == key
                && matches!(
                    r,
                    Observation::Accepted(AcceptedRecord {
                        state: crate::record::RecordState::Live(_),
                        ..
                    })
                )
        }) else {
            return Ok(review(
                incoming,
                current,
                original,
                ReviewReason::MissingReference(key),
            ));
        };
        dependencies.push(VersionCondition::from_observation(row)?);
    }
    let next_account = account.advance()?;
    let journal = JournalChange {
        installed_at: next_account.head,
        key: incoming.key,
        stamp: incoming.stamp.clone(),
    };
    Ok(RecoveredDecision::Install {
        external_guards,
        expected_head: account.head,
        expected_membership_generation: membership.generation,
        returning_actor,
        expected: VersionCondition::from_observation(current)?,
        dependencies,
        envelope: incoming.clone(),
        journal: Box::new(journal),
        next_account,
    })
}

/// Establish recovered ancestry only from an actually installed complete checkpoint.
///
/// Reload the checkpoint after commit; a source advertisement or proposed update is
/// insufficient. Save the returned boundary and account row atomically.
pub fn plan_recovered_coverage(
    account: &AccountMetadata,
    completed: &InstalledCheckpoint,
    membership: &MembershipSnapshot,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<RecoveryBoundaryPlan> {
    membership.validate(account)?;
    if completed.account != account.account {
        return Err(MetadataError::Scope {
            detail: "recovered checkpoint crosses account".into(),
        }
        .into());
    }
    if evidence
        .baseline(account.account, completed.baseline)?
        .is_none()
    {
        return Err(MetadataError::Incomplete {
            detail: "recovered checkpoint baseline unavailable".into(),
        }
        .into());
    }
    if completed
        .coverage
        .entries()
        .any(|(timeline, revision)| !account.installed.covers(Position { timeline, revision }))
    {
        return Err(MetadataError::Incomplete {
            detail: "recovered prefix is not durably installed".into(),
        }
        .into());
    }
    let older = Coverage::from_entries(
        completed
            .coverage
            .entries()
            .filter(|(timeline, _)| *timeline != account.head.timeline),
    )?;
    let next_account = account.advance()?;
    Ok(RecoveryBoundaryPlan {
        expected_head: account.head,
        expected_membership_generation: membership.generation,
        boundary: TimelineBoundary {
            at: next_account.head,
            includes: older,
            kind: BoundaryKind::RecoveredCoverage,
        },
        next_account,
    })
}

//! Membership-bounded common coverage, local basis witnesses and safe projection.

use crate::{
    account::{AccountMetadata, Baseline, EvidenceView, InstalledCheckpoint},
    client::{OriginalBasis, PendingRecord, VersionCondition},
    clock::{ClockActor, RecordVector},
    compare::{RecordRelation, compare_contexts, compare_records},
    error::MetadataError,
    lineage::{Coverage, Lineage, PositionRelation},
    membership::MembershipSnapshot,
    record::{AcceptedRecord, Position, RecordKey, VersionStamp},
};
use clacks_model::ids::{BaselineId, ClientActorId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Self-contained retained original values and their established comparison relationship.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BasisWitness {
    /// Native identity to which this relationship applies.
    pub key: RecordKey,
    /// Actual original values and context, never relabeled to the checkpoint.
    pub original: OriginalBasis,
    /// Actual observed accepted version.
    pub observed: VersionStamp,
    /// Complete installed prefixes at observation.
    pub checkpoint: InstalledCheckpoint,
    /// Established original-to-observed relationship.
    pub relation: RecordRelation,
    /// Position-dependent ancestry needed to interpret the witness.
    pub lineage: Lineage,
    /// Baseline meanings needed by the original, observed version and checkpoint.
    pub baselines: Vec<Baseline>,
}

/// Rows and surviving context references from one consistent compaction view.
pub struct CompactionInput<'a> {
    /// Current account head and baseline.
    pub account: &'a AccountMetadata,
    /// Current active and closed identities.
    pub membership: &'a MembershipSnapshot,
    /// Current accepted heads, including deletions.
    pub records: &'a [AcceptedRecord],
    /// Contexts still requiring server-interpretable residual components.
    pub surviving: &'a [VersionStamp],
}

/// Atomic baseline installation and projected rows, preceding physical cleanup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaselinePlan {
    /// Required current account head.
    pub expected_head: Position,
    /// Required current admission generation.
    pub expected_membership_generation: u64,
    /// Exact current versions of every touched envelope.
    pub expected_records: Vec<VersionCondition>,
    /// Common history interpretation to make durable first.
    pub baseline: Baseline,
    /// Projected current envelopes with unchanged native state/acceptance positions.
    pub projected: Vec<AcceptedRecord>,
    /// Closed identities no longer required by projected or surviving contexts.
    pub removable_clients: BTreeSet<ClientActorId>,
    /// Revised account head and baseline to save atomically with the projections.
    pub next_account: AccountMetadata,
}

/// Expand certified checkpoint coordinates using retained ancestry and its baseline.
pub(crate) fn expanded(
    checkpoint: &InstalledCheckpoint,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<Coverage> {
    let baseline = evidence
        .baseline(checkpoint.account, checkpoint.baseline)?
        .ok_or_else(|| MetadataError::Incomplete {
            detail: format!("checkpoint baseline {} unavailable", checkpoint.baseline),
        })?;
    let mut result = checkpoint.coverage.join(&baseline.coverage);
    for (timeline, revision) in checkpoint.coverage.entries() {
        result = result.join(
            &evidence
                .lineage
                .coverage_at(Position { timeline, revision })?,
        );
    }
    Ok(result)
}

/// Select common complete prefixes bounded by server and all authorized clients.
///
/// Inactivity never removes a participant. An initializing client without a durable
/// acknowledgment limits advancement to the already installed common baseline.
pub fn common_coverage(
    server: &InstalledCheckpoint,
    membership: &MembershipSnapshot,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<Coverage> {
    if membership.account != server.account {
        return Err(MetadataError::Scope {
            detail: "common coverage membership crosses account".into(),
        }
        .into());
    }
    let baseline = evidence
        .baseline(server.account, server.baseline)?
        .ok_or_else(|| MetadataError::Incomplete {
            detail: format!("server baseline {} unavailable", server.baseline),
        })?;
    let mut common: BTreeMap<_, _> = expanded(server, evidence)?.entries().collect();
    let mut unique = BTreeSet::new();
    for client in &membership.clients {
        if !unique.insert(client.actor) {
            return Err(MetadataError::Conflict {
                detail: format!("duplicate client {}", client.actor),
            }
            .into());
        }
        if client.closed_at.is_some() {
            continue;
        }
        let covered = match &client.acknowledged {
            Some(ack) => {
                if ack.account != server.account {
                    return Err(MetadataError::Scope {
                        detail: format!("client {} acknowledgment crosses account", client.actor),
                    }
                    .into());
                }
                expanded(ack, evidence)?
            }
            None => baseline.coverage.clone(),
        };
        let entries: BTreeMap<_, _> = covered.entries().collect();
        common.retain(|id, n| {
            if let Some(other) = entries.get(id) {
                *n = (*n).min(*other);
                true
            } else {
                false
            }
        });
    }
    Ok(Coverage::from_entries(common)?)
}

/// Retain original values and a proven relationship before releasing older detail.
///
/// Missing or divergent evidence rejects sealing; preserve the old context instead.
pub fn seal_basis(
    pending: &PendingRecord,
    observed: &AcceptedRecord,
    checkpoint: &InstalledCheckpoint,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<BasisWitness> {
    observed.validate()?;
    pending.working.validate(pending.key)?;
    if pending.key != observed.key
        || pending.original.key() != pending.key
        || checkpoint.account != pending.key.account
    {
        return Err(MetadataError::Scope {
            detail: format!("basis witness domain differs from {:?}", pending.key),
        }
        .into());
    }
    if !expanded(checkpoint, evidence)?.covers(observed.stamp.accepted_at) {
        return Err(MetadataError::Incomplete {
            detail: format!(
                "witness checkpoint does not cover observed {:?}",
                observed.stamp.accepted_at
            ),
        }
        .into());
    }
    let relation = match &pending.original {
        OriginalBasis::Accepted(original) => compare_records(original, observed, evidence)?,
        OriginalBasis::SubsumedOwn { witness, .. } => {
            compare_contexts(pending.key, witness, &observed.stamp, evidence)?
        }
        OriginalBasis::NeverCreated {
            checkpoint: original,
            ..
        } => {
            let source = expanded(original, evidence)?;
            if source.entries().all(|(timeline, revision)| {
                checkpoint.coverage.covers(Position { timeline, revision })
            }) {
                RecordRelation::Before
            } else {
                return Err(MetadataError::Incomplete {
                    detail: "absence basis is not covered by witness checkpoint".into(),
                }
                .into());
            }
        }
        OriginalBasis::Unavailable { .. } => {
            return Err(MetadataError::Incomplete {
                detail: "cannot seal an unavailable original".into(),
            }
            .into());
        }
    };
    if !matches!(
        relation,
        RecordRelation::SameVersion | RecordRelation::Before
    ) {
        return Err(MetadataError::Incomplete {
            detail: format!("cannot seal original relationship {relation:?}"),
        }
        .into());
    }
    Ok(BasisWitness {
        key: pending.key,
        original: pending.original.clone(),
        observed: observed.stamp.clone(),
        checkpoint: checkpoint.clone(),
        relation,
        lineage: evidence.lineage.clone(),
        baselines: evidence.baselines.iter().filter(|b| b.id==checkpoint.baseline || b.id==observed.stamp.baseline || matches!(&pending.original,OriginalBasis::Accepted(a) if a.stamp.baseline==b.id) || matches!(&pending.original,OriginalBasis::SubsumedOwn{witness,..} if witness.baseline==b.id)).cloned().collect(),
    })
}

/// Plan a common baseline and remove only provably collectible closed components.
///
/// Commit the baseline, account head and projected rows under all returned guards
/// before physical cleanup. Missing proof retains metadata; no payload is collected.
pub fn plan_baseline(
    id: BaselineId,
    input: &CompactionInput<'_>,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<BaselinePlan> {
    input.membership.validate(input.account)?;
    let server = InstalledCheckpoint {
        account: input.account.account,
        baseline: input.account.baseline,
        coverage: input.account.installed.clone(),
    };
    let coverage = common_coverage(&server, input.membership, evidence)?;
    let baseline = Baseline {
        id,
        account: input.account.account,
        coverage: coverage.clone(),
        lineage: evidence.lineage.clone(),
        membership_generation: input.membership.generation,
    };
    if let Some(old) = evidence.baseline(input.account.account, id)?
        && old != &baseline
    {
        return Err(MetadataError::Conflict {
            detail: format!("baseline {id} already has a different interpretation"),
        }
        .into());
    }
    let mut removable: BTreeSet<_> = input
        .membership
        .clients
        .iter()
        .filter(|c| c.closed_at.is_some_and(|p| coverage.covers(p)))
        .map(|c| c.actor)
        .collect();
    for surviving in input.surviving {
        if evidence
            .baseline(input.account.account, surviving.baseline)?
            .is_none()
        {
            removable.clear();
            break;
        }
        removable.retain(|actor| surviving.vector.component(ClockActor::Client(*actor)) == 0);
    }
    let mut projected = Vec::new();
    let mut expected_records = Vec::new();
    let mut identities = BTreeSet::new();
    for record in input.records {
        record.validate()?;
        if record.key.account != input.account.account || !identities.insert(record.key) {
            return Err(MetadataError::Scope {
                detail: format!(
                    "duplicate or cross-account compaction record {:?}",
                    record.key
                ),
            }
            .into());
        }
        let interpretation = evidence.baseline(record.key.account, record.stamp.baseline)?;
        let represents_prefix = coverage.entries().all(|(timeline, revision)| {
            matches!(
                evidence
                    .lineage
                    .compare(Position { timeline, revision }, record.stamp.accepted_at),
                PositionRelation::Before | PositionRelation::Equal | PositionRelation::After
            )
        });
        if interpretation.is_none() || !represents_prefix {
            for (actor, _) in record.stamp.vector.entries() {
                if let ClockActor::Client(actor) = actor {
                    removable.remove(&actor);
                }
            }
        }
    }
    for record in input.records {
        let interpretation = evidence.baseline(record.key.account, record.stamp.baseline)?;
        let represents_prefix = coverage.entries().all(|(timeline, revision)| {
            !matches!(
                evidence
                    .lineage
                    .compare(Position { timeline, revision }, record.stamp.accepted_at),
                PositionRelation::Unknown(_) | PositionRelation::Divergent
            )
        });
        if interpretation.is_none() || !represents_prefix {
            continue;
        }
        let mut next = record.clone();
        next.stamp.vector =
            RecordVector::from_entries(record.stamp.vector.entries().filter(
                |(actor, _)| !matches!(actor,ClockActor::Client(c) if removable.contains(c)),
            ))?;
        next.stamp.baseline = id;
        expected_records.push(VersionCondition::Is {
            key: record.key,
            stamp: record.stamp.clone(),
        });
        projected.push(next);
    }
    let mut next_account = input.account.advance()?;
    next_account.baseline = id;
    Ok(BaselinePlan {
        expected_head: input.account.head,
        expected_membership_generation: input.membership.generation,
        expected_records,
        baseline,
        projected,
        removable_clients: removable,
        next_account,
    })
}

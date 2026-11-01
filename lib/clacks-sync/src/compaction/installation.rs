//! Equivalent baseline projections bounded by authorized durable receipt.

use super::expanded;
use crate::{
    account::{AccountMetadata, Baseline, EvidenceView, InstalledCheckpoint},
    clock::{ClockActor, RecordVector},
    error::MetadataError,
    lineage::{Coverage, PositionRelation},
    membership::MembershipSnapshot,
    record::{Position, VersionStamp},
    transfer::RecordRequirement,
};
use clacks_model::ids::BaselineId;
use std::collections::BTreeSet;

/// Native metadata changes to persist together under the account's write lock.
pub struct BaselineInstallation {
    /// Fresh immutable interpretation of the common durable prefix.
    pub baseline: Baseline,
    /// Next account state, including this metadata installation's journal position.
    pub account: AccountMetadata,
    /// Equivalent current representations; native payloads remain unchanged.
    pub conversions: Vec<ContextConversion>,
}

/// Original and equivalent projected context for one current accepted head.
pub struct ContextConversion {
    /// Existing accepted descriptor; its historical references remain valid.
    pub original: RecordRequirement,
    /// Same logical version with eligible closed components removed.
    pub projected: RecordRequirement,
}

/// Find the certified prefix shared by the server and every authorized client.
///
/// A client without its first acknowledgment bounds collection at the existing
/// baseline. Missing interpretation or regressed common coverage is an error;
/// callers must keep the account's ordering evidence in that case.
pub fn common_coverage(
    account: &AccountMetadata,
    membership: &MembershipSnapshot,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<Coverage> {
    membership.validate(account)?;
    let previous = evidence
        .baseline(account.account, account.baseline)?
        .ok_or_else(|| MetadataError::Incomplete {
            detail: "current baseline unavailable for collection".into(),
        })?;

    let checkpoint = InstalledCheckpoint {
        account: account.account,
        baseline: account.baseline,
        coverage: account.installed.clone(),
    };

    let mut common = expanded(&checkpoint, evidence)?;

    for client in membership
        .clients
        .iter()
        .filter(|client| client.closed_at.is_none())
    {
        let coverage = match &client.acknowledged {
            Some(ack) => expanded(ack, evidence)?,
            None => previous.coverage.clone(),
        };
        common = Coverage::from_entries(common.entries().filter_map(|(timeline, revision)| {
            coverage
                .entries()
                .find(|(id, _)| *id == timeline)
                .map(|(_, received)| (timeline, revision.min(received)))
        }))?;
    }

    anyhow::ensure!(
        previous
            .coverage
            .entries()
            .all(|(timeline, revision)| common.covers(Position { timeline, revision })),
        MetadataError::Incomplete {
            detail: "common receipt does not preserve the installed baseline".into()
        }
    );

    Ok(common)
}

/// Plan only proved, useful context conversions for current accepted heads.
///
/// Read all inputs under the same account transaction and persist the result
/// there. A no-work result neither advances revisions nor creates a baseline.
/// Unknown or incomparable record contexts remain in their original form.
pub fn plan_installation(
    baseline_id: BaselineId,
    account: &AccountMetadata,
    membership: &MembershipSnapshot,
    current: &[RecordRequirement],
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<Option<BaselineInstallation>> {
    let coverage = common_coverage(account, membership, evidence)?;

    let closed: BTreeSet<_> = membership
        .clients
        .iter()
        .filter_map(|client| {
            client
                .closed_at
                .filter(|position| coverage.covers(*position))
                .map(|_| client.actor)
        })
        .collect();

    let mut conversions = Vec::new();

    for original in current {
        anyhow::ensure!(
            original.key.account == account.account,
            "projection crosses account"
        );

        let stamp = &original.stamp;
        if evidence
            .baseline(account.account, stamp.baseline())?
            .is_none()
        {
            continue;
        }

        let ordered = coverage.covers(stamp.accepted_at())
            || coverage.entries().all(|(timeline, revision)| {
                let position = Position { timeline, revision };
                stamp.vector().component(ClockActor::Server(timeline)) >= revision
                    || matches!(
                        evidence.lineage.compare(position, stamp.accepted_at()),
                        PositionRelation::Before | PositionRelation::Equal
                    )
            });

        if !ordered
            || !stamp
                .vector()
                .entries()
                .any(|(actor, _)| matches!(actor, ClockActor::Client(id) if closed.contains(&id)))
        {
            continue;
        }

        let vector =
            RecordVector::from_entries(stamp.vector().entries().filter(
                |(actor, _)| !matches!(actor, ClockActor::Client(id) if closed.contains(id)),
            ))?;
        conversions.push(ContextConversion {
            original: original.clone(),
            projected: RecordRequirement {
                key: original.key,
                deleted: original.deleted,
                stamp: VersionStamp::new(stamp.accepted_at(), baseline_id, vector)?,
            },
        });
    }

    tracing::debug!(
        event = "sync.baseline.projection",
        account_id = %account.account,
        eligible_actors = closed.len(),
        contexts = current.len(),
        conversions = conversions.len(),
        "Compared current contexts against common durable receipt"
    );

    if conversions.is_empty() {
        return Ok(None);
    }

    anyhow::ensure!(
        evidence.baseline(account.account, baseline_id)?.is_none(),
        "projection baseline identity already exists"
    );

    let baseline = Baseline {
        id: baseline_id,
        account: account.account,
        coverage,
        lineage: evidence.lineage.clone(),
        membership_generation: membership.generation,
    };
    let mut next = account.advance()?;
    next.baseline = baseline.id;

    Ok(Some(BaselineInstallation {
        baseline,
        account: next,
        conversions,
    }))
}

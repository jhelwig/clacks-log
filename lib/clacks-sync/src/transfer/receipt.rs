//! Connection-owned discovery obligations and verified complete coverage.

use super::{
    RecordRequirement, WindowMode,
    discovery::{DiscoveryEnd, DiscoveryPage, DiscoveryStart},
};
use crate::{
    account::{Baseline, EvidenceView, InstalledCheckpoint},
    lineage::TimelineBoundary,
    record::AcceptedRecord,
    service::{VersionReference, operation::CoupledRequirement},
};
use clacks_model::ids::OperationId;
use std::collections::BTreeSet;

/// One live receiver's obligations; reconnect starts a fresh receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReceiptState {
    /// Fixed complete target advertised by this source.
    pub source: InstalledCheckpoint,
    /// Full enumeration or journal interval being verified.
    pub mode: WindowMode,
    /// Sequence required for the next page.
    pub next_page: u64,
    /// Exact requirements registered by preceding pages.
    pub records: Vec<RecordRequirement>,
    /// Required ancestry interpretations.
    pub boundaries: Vec<TimelineBoundary>,
    /// Required baseline meanings.
    pub baselines: Vec<Baseline>,
    /// Related effects which must be represented together.
    pub units: Vec<CoupledRequirement>,
    /// Exact incomplete inputs; their payloads live in semantic staging.
    pub staged: Vec<VersionReference>,
    /// Coupled effects committed by this receiver.
    pub completed_units: BTreeSet<OperationId>,
}

impl ReceiptState {
    /// Start an uncredited discovery target; this does not install data or coverage.
    pub fn begin(start: &DiscoveryStart) -> anyhow::Result<Self> {
        start.source.validate()?;
        if let WindowMode::Incremental { after } = &start.mode {
            after.validate()?;
        }
        Ok(Self {
            source: start.source.clone(),
            mode: start.mode.clone(),
            next_page: 0,
            records: vec![],
            boundaries: vec![],
            baselines: vec![],
            units: vec![],
            staged: vec![],
            completed_units: BTreeSet::new(),
        })
    }
}

/// Register an ordered page; publish this state only after its data commits.
#[tracing::instrument(skip_all, name="sync.receipt.register", level="debug", fields(account_id=%current.source.account, page=page.sequence))]
pub fn register_page(current: &ReceiptState, page: &DiscoveryPage) -> anyhow::Result<ReceiptState> {
    anyhow::ensure!(
        page.sequence == current.next_page,
        "page outside receipt sequence"
    );
    let mut next = current.clone();
    next.records.extend(page.requirements.clone());
    for row in page.accepted.iter().chain(&page.support) {
        next.records.push(super::RecordRequirement {
            key: row.key(),
            stamp: row.stamp.clone(),
            deleted: matches!(row.state(), crate::record::RecordState::Deleted),
        });
    }
    for unit in &page.units {
        if let Some(old) = next.units.iter().find(|old| old.id == unit.id) {
            anyhow::ensure!(old == unit, "coupled requirement changed");
        } else {
            next.units.push(unit.clone());
        }
        next.records
            .extend(unit.members.iter().map(|member| super::RecordRequirement {
                key: member.key,
                stamp: member.stamp.clone(),
                deleted: member.deleted,
            }));
    }
    for baseline in &page.baselines {
        if let Some(old) = next.baselines.iter().find(|old| old.id == baseline.id) {
            anyhow::ensure!(old == baseline, "baseline interpretation changed");
        } else {
            next.baselines.push(baseline.clone());
        }
    }
    for boundary in &page.boundaries {
        boundary.at.validate()?;
        boundary.includes.validate()?;
        if !next.boundaries.contains(boundary) {
            next.boundaries.push(boundary.clone());
        }
    }
    let mut unique = Vec::new();
    for row in next.records {
        anyhow::ensure!(
            row.key.account == next.source.account,
            "receipt requirement crosses account"
        );
        if let Some(old) = unique
            .iter()
            .find(|old: &&RecordRequirement| old.key == row.key && old.stamp == row.stamp)
        {
            anyhow::ensure!(
                old.deleted == row.deleted,
                "contradictory deletion for accepted context"
            );
        } else {
            unique.push(row);
        }
    }
    next.records = unique;
    for baseline in &next.baselines {
        baseline.validate()?;
        anyhow::ensure!(
            baseline.account == next.source.account,
            "receipt baseline crosses account"
        );
    }
    let mut operations = BTreeSet::new();
    for unit in &next.units {
        anyhow::ensure!(
            !unit.members.is_empty() && operations.insert(unit.id),
            "empty or repeated coupled unit"
        );
        let mut keys = BTreeSet::new();
        for member in &unit.members {
            anyhow::ensure!(
                member.key.account == next.source.account && keys.insert(member.key),
                "invalid coupled member"
            );
        }
    }
    next.next_page = crate::error::next_counter(current.next_page, "discovery page")?;
    Ok(next)
}

/// Verify source exhaustion and committed values before the adapter saves coverage.
#[tracing::instrument(skip_all, name="sync.receipt.finish.plan", level="debug", fields(account_id=%current.source.account))]
pub fn finish_receipt(
    current: &ReceiptState,
    end: &DiscoveryEnd,
    installed: &[AcceptedRecord],
    previous: Option<&InstalledCheckpoint>,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<InstalledCheckpoint> {
    anyhow::ensure!(
        end.source == current.source && end.pages == current.next_page,
        "discovery end does not match received source"
    );
    let account = current.source.account;
    anyhow::ensure!(
        evidence.lineage.account() == account,
        "completion crosses account"
    );
    anyhow::ensure!(
        current.staged.is_empty(),
        crate::error::MetadataError::Incomplete {
            detail: "receipt still has staged input".into()
        }
    );
    anyhow::ensure!(
        evidence
            .baseline(account, current.source.baseline)?
            .is_some(),
        crate::error::MetadataError::Incomplete {
            detail: "source baseline is not installed".into()
        }
    );
    for baseline in &current.baselines {
        anyhow::ensure!(
            evidence.baseline(account, baseline.id)? == Some(baseline),
            "required baseline interpretation is not installed"
        );
    }
    for boundary in &current.boundaries {
        anyhow::ensure!(
            evidence.lineage.boundaries().contains(boundary),
            "required ancestry is not installed"
        );
    }
    for requirement in &current.records {
        let mut represented = false;
        for value in installed {
            if super::satisfies(requirement, value, evidence)? {
                represented = true;
                break;
            }
        }
        anyhow::ensure!(
            represented,
            crate::error::MetadataError::Incomplete {
                detail: format!(
                    "required accepted version {:?} is not represented",
                    requirement.key
                )
            }
        );
    }
    anyhow::ensure!(
        current
            .units
            .iter()
            .all(|unit| current.completed_units.contains(&unit.id)),
        crate::error::MetadataError::Incomplete {
            detail: "coupled effects are not committed".into()
        }
    );
    let coverage = match previous {
        Some(previous) => {
            previous.validate()?;
            anyhow::ensure!(
                previous.account == account,
                "previous checkpoint crosses account"
            );
            if let WindowMode::Incremental { after } = &current.mode {
                anyhow::ensure!(
                    after.entries().all(|(timeline, revision)| previous
                        .coverage
                        .covers(crate::record::Position { timeline, revision })),
                    "incremental starting prefixes are not installed"
                );
            }
            previous.coverage.join(&current.source.coverage)
        }
        None => {
            anyhow::ensure!(
                matches!(current.mode, WindowMode::Full),
                "initial receipt must be full"
            );
            current.source.coverage.clone()
        }
    };
    Ok(InstalledCheckpoint {
        account,
        baseline: current.source.baseline,
        coverage,
    })
}

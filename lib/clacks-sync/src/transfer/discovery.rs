//! Shared discovery targets and native page contents, independent of database cursors.

use super::{RecordRequirement, WindowMode};
use crate::{
    account::{Baseline, EvidenceView, InstalledCheckpoint},
    lineage::{Coverage, TimelineBoundary},
    record::AcceptedRecord,
    service::operation::CoupledRequirement,
};
use serde::{Deserialize, Serialize};

/// Fixed complete coverage being discovered through one live exchange.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveryStart {
    /// Complete target; individual newer records cannot enlarge it.
    pub source: InstalledCheckpoint,
    /// Full enumeration or journal-backed difference.
    pub mode: WindowMode,
}

/// Ordered native obligations and inputs committed together by a receiver.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveryPage {
    /// Zero-based sequence within this exchange.
    pub sequence: u64,
    /// Exact accepted contexts required represented before completion.
    pub requirements: Vec<RecordRequirement>,
    /// Current accepted heads, which may be newer than the fixed target.
    pub accepted: Vec<AcceptedRecord>,
    /// Exact historical or retained facts; receipt must not make these current heads.
    pub support: Vec<AcceptedRecord>,
    /// Baseline meanings required to interpret the supplied contexts.
    pub baselines: Vec<Baseline>,
    /// Position-dependent ancestry required to interpret supplied contexts.
    pub boundaries: Vec<TimelineBoundary>,
    /// Effects which must be durably represented together.
    pub units: Vec<CoupledRequirement>,
}

/// Source assertion that discovery exhausted this target after all preceding pages.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveryEnd {
    /// Original fixed target.
    pub source: InstalledCheckpoint,
    /// Number of pages emitted in order, including an empty traversal's zero.
    pub pages: u64,
}

/// Select a journal difference only when the retained interval and interpretation cover it.
///
/// Storage supplies the real retained boundary and target from one read transaction.
/// A missing eligible journal selects fuller-state reconciliation without expiring a client.
#[tracing::instrument(skip_all, name = "sync.discovery.select", level = "debug", fields(account_id = %source.account))]
pub fn select_mode(
    source: &InstalledCheckpoint,
    after: Option<&InstalledCheckpoint>,
    retained_from: Option<&Coverage>,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<WindowMode> {
    source.validate()?;
    let (Some(after), Some(retained)) = (after, retained_from) else {
        return Ok(WindowMode::Full);
    };
    anyhow::ensure!(after.account == source.account, "discovery crosses account");
    after.validate()?;
    let Ok(known) = crate::compaction::expanded(after, evidence) else {
        return Ok(WindowMode::Full);
    };
    let target = crate::compaction::expanded(source, evidence)?;
    if retained
        .entries()
        .all(|(timeline, revision)| known.covers(crate::record::Position { timeline, revision }))
        && known.entries().all(|(timeline, revision)| {
            target.covers(crate::record::Position { timeline, revision })
        })
    {
        Ok(WindowMode::Incremental { after: known })
    } else {
        Ok(WindowMode::Full)
    }
}

/// Expand the target through its genuine retained baseline and timeline ancestry.
pub fn target_coverage(
    source: &InstalledCheckpoint,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<Coverage> {
    crate::compaction::expanded(source, evidence)
}

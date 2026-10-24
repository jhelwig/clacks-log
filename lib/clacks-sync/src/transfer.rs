//! Immutable complete windows, bounded installation and conditional checkpoint updates.

use crate::service::operation::CoupledRequirement;
use crate::{
    account::{Baseline, EvidenceView, InstalledCheckpoint},
    client::{ClientRecord, ClientUpdate, receive_accepted},
    compare::{RecordRelation, compare_contexts},
    error::MetadataError,
    lineage::{Coverage, TimelineBoundary},
    record::{AcceptedRecord, Observation, RecordKey, RecordState, VersionStamp},
};
use clacks_model::ids::{AccountId, OperationId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Version or deletion that a complete source view requires represented.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordRequirement {
    /// Required native identity.
    pub key: RecordKey,
    /// Required accepted context.
    pub stamp: VersionStamp,
    /// Whether this version records deletion.
    pub deleted: bool,
}

/// Exact full enumeration or journal-supported incremental transfer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowMode {
    /// Source certifies all current identities and deletion descriptors.
    Full,
    /// Source certifies all changes beyond the given completed prefixes.
    Incremental {
        /// Installed prefixes from which journal discovery begins.
        after: Coverage,
    },
}

/// Immutable requirements from one consistent source checkpoint and mode.
///
/// Store once under `(source, mode)`. A different requirement set under that same
/// identity is invalid; adapters must reject replacement rather than rewrite it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowManifest {
    /// Owning account.
    pub account: AccountId,
    /// Complete source view to transfer.
    pub source: InstalledCheckpoint,
    /// Discovery completeness contract.
    pub mode: WindowMode,
    /// Sorted unique required versions/deletions.
    pub records: Vec<RecordRequirement>,
    /// Ancestry to install before completion.
    pub boundaries: Vec<TimelineBoundary>,
    /// Baseline interpretations to install before completion.
    pub baselines: Vec<Baseline>,
    /// Atomic units whose effects must be represented together.
    pub units: Vec<CoupledRequirement>,
}

impl WindowManifest {
    /// Validate a consistent source manifest, deduplicating only identical descriptors.
    pub fn new(
        account: AccountId,
        source: InstalledCheckpoint,
        mode: WindowMode,
        records: Vec<RecordRequirement>,
        boundaries: Vec<TimelineBoundary>,
        baselines: Vec<Baseline>,
        units: Vec<CoupledRequirement>,
    ) -> anyhow::Result<Self> {
        source.validate()?;
        if let WindowMode::Incremental { after } = &mode {
            after.validate()?;
        }
        if source.account != account {
            return Err(MetadataError::Scope {
                detail: format!(
                    "manifest source belongs to {} instead of {account}",
                    source.account
                ),
            }
            .into());
        }
        let mut unique = BTreeMap::new();
        for record in records {
            record.stamp.validate()?;
            if record.key.account != account {
                return Err(MetadataError::Scope {
                    detail: format!("manifest record {:?} crosses account", record.key),
                }
                .into());
            }
            if record.stamp.accepted_at.revision == 0 {
                return Err(MetadataError::Invalid {
                    detail: format!("zero accepted position for {:?}", record.key),
                }
                .into());
            }
            if unique
                .insert(record.key, record.clone())
                .is_some_and(|old| old != record)
            {
                return Err(MetadataError::Conflict {
                    detail: format!("conflicting window requirement {:?}", record.key),
                }
                .into());
            }
        }
        for b in &baselines {
            b.validate()?;
            if b.account != account || b.lineage.account() != account {
                return Err(MetadataError::Scope {
                    detail: format!("manifest baseline {} crosses account", b.id),
                }
                .into());
            }
        }
        for boundary in &boundaries {
            boundary.at.validate()?;
            boundary.includes.validate()?;
        }
        let mut unit_ids = BTreeSet::new();
        for unit in &units {
            if unit.members.is_empty() || !unit_ids.insert(unit.id) {
                anyhow::bail!("empty or duplicate atomic unit {}", unit.id);
            }
            let mut member_keys = BTreeSet::new();
            for member in &unit.members {
                member.stamp.validate()?;
                anyhow::ensure!(
                    member.key.account == account
                        && unique.contains_key(&member.key)
                        && member_keys.insert(member.key),
                    "invalid or undeclared atomic member {:?}",
                    member.key
                );
            }
        }
        Ok(Self {
            account,
            source,
            mode,
            records: unique.into_values().collect(),
            boundaries,
            baselines,
            units,
        })
    }

    /// Reject replacement of an already stored manifest under the same source/mode identity.
    ///
    /// Call when persisting a source manifest that already exists. Atomic-unit descriptors
    /// are part of its immutable contents even when native record requirements agree.
    pub fn validate_replacement(&self, proposed: &Self) -> anyhow::Result<()> {
        self.validate()?;
        proposed.validate()?;
        if self.source == proposed.source && self.mode == proposed.mode {
            anyhow::ensure!(
                self == proposed,
                "immutable source manifest contents changed"
            );
        }
        Ok(())
    }

    /// Recheck deserialized composite rows before using them in a transition.
    fn validate(&self) -> anyhow::Result<()> {
        Self::new(
            self.account,
            self.source.clone(),
            self.mode.clone(),
            self.records.clone(),
            self.boundaries.clone(),
            self.baselines.clone(),
            self.units.clone(),
        )?;
        Ok(())
    }
}

/// Durable progress for a separately stored immutable manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowProgress {
    /// Immutable source view identity.
    pub source: InstalledCheckpoint,
    /// Immutable enumeration mode.
    pub mode: WindowMode,
    /// Checked conditional progress revision.
    pub generation: u64,
    /// Requirement keys durably installed so far.
    pub installed: BTreeSet<RecordKey>,
    /// Durably received inputs that cannot yet be applied as complete units.
    pub staged: Vec<AcceptedRecord>,
    /// Atomic-unit IDs whose accepted effects are durably represented.
    pub completed_units: BTreeSet<OperationId>,
}

/// Bounded row updates to save atomically with appended progress keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallBatch {
    /// Manifest source identity.
    pub source: InstalledCheckpoint,
    /// Manifest mode identity.
    pub mode: WindowMode,
    /// Progress revision required by this batch.
    pub expected_generation: u64,
    /// Accepted rows selected for installation.
    pub accepted: Vec<AcceptedRecord>,
    /// Associated client working/original updates.
    pub client_updates: Vec<ClientUpdate>,
    /// Keys to append, without copying the existing installed set.
    pub installed_keys: Vec<RecordKey>,
    /// Progress revision after this transaction.
    pub next_generation: u64,
    /// Complete replacement for durable staged inputs in this generation.
    pub staged: Vec<AcceptedRecord>,
    /// Complete replacement for represented unit IDs in this generation.
    pub completed_units: BTreeSet<OperationId>,
}

/// Conditional checkpoint row update; this is not proof of a durable commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckpointUpdate {
    /// Installed checkpoint required to remain current.
    pub expected: InstalledCheckpoint,
    /// Checkpoint to save after complete verification.
    pub next: InstalledCheckpoint,
    /// Independently completed source prefixes.
    pub completed_source: InstalledCheckpoint,
    /// Required durable progress revision.
    pub expected_progress_generation: u64,
}

/// Verify the progress row belongs to this immutable window.
fn validate_progress(manifest: &WindowManifest, progress: &WindowProgress) -> anyhow::Result<()> {
    crate::error::validate_counter(progress.generation, "window progress")?;
    manifest.validate()?;
    if manifest.source != progress.source || manifest.mode != progress.mode {
        return Err(MetadataError::Conflict {
            detail: "progress belongs to a replaced window".into(),
        }
        .into());
    }
    if progress
        .installed
        .iter()
        .any(|key| !manifest.records.iter().any(|r| r.key == *key))
    {
        return Err(MetadataError::Conflict {
            detail: "progress credits an undeclared key".into(),
        }
        .into());
    }
    anyhow::ensure!(
        progress
            .completed_units
            .iter()
            .all(|id| manifest.units.iter().any(|u| u.id == *id)),
        "progress credits undeclared atomic unit"
    );
    let mut staged = BTreeSet::new();
    for row in &progress.staged {
        row.validate()?;
        anyhow::ensure!(
            staged.insert(row.key) && manifest.records.iter().any(|r| r.key == row.key),
            "duplicate or undeclared staged row"
        );
    }
    Ok(())
}

/// Check a concrete installed version against a context-only requirement.
fn satisfies(
    required: &RecordRequirement,
    actual: &AcceptedRecord,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<bool> {
    actual.validate()?;
    if required.key != actual.key {
        return Ok(false);
    }
    match compare_contexts(required.key, &required.stamp, &actual.stamp, evidence)? {
        RecordRelation::SameVersion => {
            Ok(required.deleted == matches!(actual.state, RecordState::Deleted))
        }
        RecordRelation::Before => Ok(true),
        _ => Ok(false),
    }
}

/// Plan a bounded installation from rows loaded in the same actual write transaction.
///
/// Atomic installation includes accepted values, client comparison evidence and
/// progress keys. A local conflict is retained without blocking shared receipt.
#[tracing::instrument(
    skip_all,
    name = "sync.transfer.batch.plan",
    level = "debug",
    fields(
        account_id = %manifest.account,
    ),
)]
pub fn plan_batch(
    manifest: &WindowManifest,
    progress: &WindowProgress,
    incoming: &[AcceptedRecord],
    local: &[ClientRecord],
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<InstallBatch> {
    validate_progress(manifest, progress)?;
    let next_generation = crate::error::next_counter(progress.generation, "window progress")?;
    let mut candidates = progress.staged.clone();
    for row in incoming {
        let required = manifest
            .records
            .iter()
            .find(|r| r.key == row.key)
            .ok_or_else(|| anyhow::anyhow!("undeclared incoming key {:?}", row.key))?;
        let exact_staging = required.stamp == row.stamp
            && required.deleted == matches!(row.state, RecordState::Deleted)
            && (crate::service::payload(row).is_some()
                || manifest
                    .units
                    .iter()
                    .any(|u| u.members.iter().any(|m| m.key == row.key)));
        anyhow::ensure!(
            exact_staging || satisfies(required, row, evidence)?,
            "incoming version does not satisfy source requirement {:?}",
            row.key
        );
        if let Some(old) = candidates.iter().find(|r| r.key == row.key) {
            crate::service::check_integrity(row.key, &old.state, &row.state)?;
            anyhow::ensure!(old == row, "staged variants differ for {:?}", row.key);
        } else {
            candidates.push(row.clone());
        }
    }
    for row in &candidates {
        if let Some(existing) = local.iter().find(|r| r.key == row.key)
            && let Observation::Accepted(old) = &existing.latest
        {
            crate::service::check_integrity(row.key, &old.state, &row.state)?;
        }
    }
    let mut available = candidates.clone();
    for old in local.iter().filter_map(|r| {
        if let Observation::Accepted(v) = &r.latest {
            Some(v)
        } else {
            None
        }
    }) {
        if let Some(index) = available.iter().position(|r| r.key == old.key) {
            let incoming = available
                .get(index)
                .ok_or_else(|| anyhow::anyhow!("selected row missing"))?;
            if matches!(
                compare_contexts(old.key, &incoming.stamp, &old.stamp, evidence)?,
                RecordRelation::Before | RecordRelation::SameVersion
            ) {
                available.remove(index);
                available.push(old.clone());
            }
        } else {
            available.push(old.clone());
        }
    }
    let view = crate::service::ServiceView {
        account: manifest.account,
        records: &available,
        bindings: &[],
        evidence,
    };
    let mut blocked = BTreeSet::new();
    for row in &candidates {
        if evidence
            .baseline(manifest.account, row.stamp.baseline)?
            .is_none()
        {
            blocked.insert(row.key);
        }
        let represented = available.iter().find(|r| r.key == row.key).unwrap_or(row);
        if let Some(s) = crate::service::payload(represented)
            && matches!(
                crate::service::dependency::check(s, &view)?,
                crate::service::dependency::DependencyStatus::Incomplete(_)
            )
        {
            blocked.insert(row.key);
        }
    }
    loop {
        let before = blocked.len();
        for unit in &manifest.units {
            let mut complete = true;
            for member in &unit.members {
                let required = RecordRequirement {
                    key: member.key,
                    stamp: member.stamp.clone(),
                    deleted: member.deleted,
                };
                let Some(actual) = available.iter().find(|r| r.key == member.key) else {
                    complete = false;
                    continue;
                };
                if blocked.contains(&member.key) || !satisfies(&required, actual, evidence)? {
                    complete = false;
                }
            }
            if !complete {
                for member in &unit.members {
                    let relation = available
                        .iter()
                        .find(|r| r.key == member.key)
                        .map(|r| compare_contexts(member.key, &member.stamp, &r.stamp, evidence))
                        .transpose()?;
                    let durable = local
                        .iter()
                        .filter_map(|r| {
                            if let Observation::Accepted(v) = &r.latest {
                                Some(v)
                            } else {
                                None
                            }
                        })
                        .find(|r| r.key == member.key)
                        .map(|r| {
                            satisfies(
                                &RecordRequirement {
                                    key: member.key,
                                    stamp: member.stamp.clone(),
                                    deleted: member.deleted,
                                },
                                r,
                                evidence,
                            )
                        })
                        .transpose()?
                        .unwrap_or(false);
                    if !durable && !matches!(relation, Some(RecordRelation::Before)) {
                        blocked.insert(member.key);
                    }
                }
            }
        }
        for row in &candidates {
            if let Some(s) = crate::service::payload(row) {
                let deps = crate::service::dependency::required_keys(manifest.account, s)
                    .into_iter()
                    .chain(s.support.required.iter().map(|r| r.reference().key));
                if deps.into_iter().any(|key| blocked.contains(&key)) {
                    blocked.insert(row.key);
                }
            }
        }
        if blocked.len() == before {
            break;
        }
    }
    let staged = candidates
        .iter()
        .filter(|r| blocked.contains(&r.key))
        .cloned()
        .collect();
    let mut completed_units = progress.completed_units.clone();
    for unit in &manifest.units {
        if unit.members.iter().all(|r| !blocked.contains(&r.key)) {
            completed_units.insert(unit.id);
        }
    }
    let ready = candidates
        .iter()
        .filter(|r| !blocked.contains(&r.key))
        .cloned()
        .collect::<Vec<_>>();
    let mut accepted = Vec::new();
    let mut client_updates = Vec::new();
    let mut keys = BTreeSet::new();
    for row in &ready {
        let required = manifest
            .records
            .iter()
            .find(|r| r.key == row.key)
            .ok_or_else(|| MetadataError::Conflict {
                detail: format!("undeclared incoming key {:?}", row.key),
            })?;
        if !satisfies(required, row, evidence)? {
            return Err(MetadataError::Incomplete {
                detail: format!(
                    "incoming {:?} does not represent its required version/deletion",
                    row.key
                ),
            }
            .into());
        }
        if !keys.insert(row.key) {
            if accepted
                .iter()
                .any(|old: &AcceptedRecord| old.key == row.key && old != row)
            {
                return Err(MetadataError::Conflict {
                    detail: format!("conflicting duplicate batch row {:?}", row.key),
                }
                .into());
            }
            continue;
        }
        let update = if let Some(existing) = local.iter().find(|r| r.key == row.key) {
            receive_accepted(existing, row, evidence)?
        } else {
            ClientUpdate {
                record: ClientRecord {
                    retained: Vec::new(),
                    key: row.key,
                    latest: Observation::Accepted(row.clone()),
                    pending: None,
                },
                review: None,
            }
        };
        let Observation::Accepted(chosen) = &update.record.latest else {
            return Err(MetadataError::Incomplete {
                detail: format!("no accepted result for {:?}", row.key),
            }
            .into());
        };
        if !satisfies(required, chosen, evidence)? {
            return Err(MetadataError::Incomplete {
                detail: format!("installed result does not cover {:?}", row.key),
            }
            .into());
        }
        accepted.push(chosen.clone());
        client_updates.push(update);
    }
    let plan = InstallBatch {
        source: manifest.source.clone(),
        mode: manifest.mode.clone(),
        expected_generation: progress.generation,
        accepted,
        client_updates,
        installed_keys: keys.into_iter().collect(),
        next_generation,
        staged,
        completed_units,
    };
    let readiness = if plan.staged.is_empty() {
        "ready"
    } else {
        "staged"
    };
    metrics::counter!("clacks_sync_transfer_batch_plans_total", "result" => readiness).increment(1);
    tracing::debug!(
        event = "transfer.batch.planned",
        readiness,
        input_count = incoming.len(),
        proposed_record_count = plan.accepted.len(),
        staged_record_count = plan.staged.len(),
        proposed_install_key_count = plan.installed_keys.len()
    );
    Ok(plan)
}

/// Verify complete installed rows and interpretation before proposing checkpoint advance.
///
/// Supply committed rows from one actual read/write transaction. Only a subsequent
/// reloaded committed checkpoint may be acknowledged to another participant.
#[tracing::instrument(
    skip_all,
    name = "sync.transfer.window.finish",
    level = "debug",
    fields(
        account_id = %manifest.account,
    ),
)]
pub fn finish_window(
    manifest: &WindowManifest,
    progress: &WindowProgress,
    installed: &[AcceptedRecord],
    previous: &InstalledCheckpoint,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<CheckpointUpdate> {
    previous.validate()?;
    if previous.account != manifest.account {
        anyhow::bail!("window completion account mismatch");
    }
    if let WindowMode::Incremental { after } = &manifest.mode
        && after.entries().any(|(timeline, revision)| {
            !previous
                .coverage
                .covers(crate::record::Position { timeline, revision })
        })
    {
        return Err(MetadataError::Incomplete {
            detail: "incremental starting prefixes are not installed".into(),
        }
        .into());
    }
    verify_complete(manifest, progress, installed, evidence)?;
    Ok(CheckpointUpdate {
        expected: previous.clone(),
        next: InstalledCheckpoint {
            account: manifest.account,
            baseline: manifest.source.baseline,
            coverage: previous.coverage.join(&manifest.source.coverage),
        },
        completed_source: manifest.source.clone(),
        expected_progress_generation: progress.generation,
    })
}

/// Verify a first complete full view without inventing an earlier installed checkpoint.
///
/// Storage adapters must check that no completed receipt exists and commit this
/// verified result with the window progress before acknowledging it.
pub fn finish_initial_window(
    manifest: &WindowManifest,
    progress: &WindowProgress,
    installed: &[AcceptedRecord],
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<InstalledCheckpoint> {
    anyhow::ensure!(
        matches!(manifest.mode, WindowMode::Full),
        "initial installation requires a full window"
    );
    verify_complete(manifest, progress, installed, evidence)?;
    Ok(manifest.source.clone())
}

/// Check required values, complete units and installed interpretation for either completion path.
fn verify_complete(
    manifest: &WindowManifest,
    progress: &WindowProgress,
    installed: &[AcceptedRecord],
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<()> {
    validate_progress(manifest, progress)?;
    if evidence.lineage.account() != manifest.account {
        return Err(MetadataError::Scope {
            detail: "window completion account mismatch".into(),
        }
        .into());
    }
    if evidence
        .baseline(manifest.account, manifest.source.baseline)?
        .is_none()
    {
        return Err(MetadataError::Incomplete {
            detail: "source baseline is not installed".into(),
        }
        .into());
    }
    for baseline in &manifest.baselines {
        if evidence.baseline(manifest.account, baseline.id)? != Some(baseline) {
            return Err(MetadataError::Incomplete {
                detail: format!("baseline {} interpretation not installed", baseline.id),
            }
            .into());
        }
    }
    for boundary in &manifest.boundaries {
        if !evidence.lineage.boundaries().contains(boundary) {
            return Err(MetadataError::Incomplete {
                detail: format!("boundary {:?} is not installed", boundary.at),
            }
            .into());
        }
    }
    for requirement in &manifest.records {
        let actual = installed
            .iter()
            .find(|row| row.key == requirement.key)
            .ok_or_else(|| MetadataError::Incomplete {
                detail: format!("missing required record {:?}", requirement.key),
            })?;
        if !satisfies(requirement, actual, evidence)? {
            return Err(MetadataError::Incomplete {
                detail: format!(
                    "required version/deletion {:?} not represented",
                    requirement.key
                ),
            }
            .into());
        }
    }
    anyhow::ensure!(
        progress.staged.is_empty(),
        "window still has staged atomic inputs"
    );
    for unit in &manifest.units {
        anyhow::ensure!(
            progress.completed_units.contains(&unit.id),
            "atomic unit {} not committed",
            unit.id
        );
        for member in &unit.members {
            let required = RecordRequirement {
                key: member.key,
                stamp: member.stamp.clone(),
                deleted: member.deleted,
            };
            let actual = installed
                .iter()
                .find(|r| r.key == member.key)
                .ok_or_else(|| anyhow::anyhow!("atomic member missing"))?;
            anyhow::ensure!(
                satisfies(&required, actual, evidence)?,
                "atomic member not represented"
            );
        }
    }
    Ok(())
}

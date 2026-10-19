//! Immutable complete windows, bounded installation and conditional checkpoint updates.

use crate::{
    account::{Baseline, EvidenceView, InstalledCheckpoint},
    client::{ClientRecord, ClientUpdate, receive_accepted},
    compare::{RecordRelation, compare_contexts},
    error::MetadataError,
    lineage::{Coverage, TimelineBoundary},
    record::{AcceptedRecord, Observation, RecordKey, RecordState, VersionStamp},
};
use clacks_model::ids::AccountId;
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
    ) -> anyhow::Result<Self> {
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
            if b.account != account || b.lineage.account() != account {
                return Err(MetadataError::Scope {
                    detail: format!("manifest baseline {} crosses account", b.id),
                }
                .into());
            }
        }
        Ok(Self {
            account,
            source,
            mode,
            records: unique.into_values().collect(),
            boundaries,
            baselines,
        })
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
pub fn plan_batch(
    manifest: &WindowManifest,
    progress: &WindowProgress,
    incoming: &[AcceptedRecord],
    local: &[ClientRecord],
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<InstallBatch> {
    validate_progress(manifest, progress)?;
    let next_generation =
        progress
            .generation
            .checked_add(1)
            .ok_or_else(|| MetadataError::Overflow {
                counter: "window progress".into(),
            })?;
    let mut accepted = Vec::new();
    let mut client_updates = Vec::new();
    let mut keys = BTreeSet::new();
    for row in incoming {
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
    Ok(InstallBatch {
        source: manifest.source.clone(),
        mode: manifest.mode.clone(),
        expected_generation: progress.generation,
        accepted,
        client_updates,
        installed_keys: keys.into_iter().collect(),
        next_generation,
    })
}

/// Verify complete installed rows and interpretation before proposing checkpoint advance.
///
/// Supply committed rows from one actual read/write transaction. Only a subsequent
/// reloaded committed checkpoint may be acknowledged to another participant.
pub fn finish_window(
    manifest: &WindowManifest,
    progress: &WindowProgress,
    installed: &[AcceptedRecord],
    previous: &InstalledCheckpoint,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<CheckpointUpdate> {
    validate_progress(manifest, progress)?;
    if previous.account != manifest.account || evidence.lineage.account() != manifest.account {
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

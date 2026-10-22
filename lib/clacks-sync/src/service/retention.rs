//! Semantic retention obligations independent of synchronization-journal collection.

use super::{VersionReference, operation::CoupledRequirement};
use crate::record::{AcceptedRecord, RecordKey, VersionStamp};
use clacks_model::service::{
    destination::Destination,
    evidence::{ServicePayload, download::AcquisitionPhase},
};
use std::collections::BTreeSet;

/// Required retained meaning and eligible disposable acquisition coordination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetentionRequirements {
    /// Stable semantic identities that must remain available, without keeping every old QSO body.
    pub semantic_keys: BTreeSet<RecordKey>,
    /// Accepted fact/effect requirements, satisfiable by documented equivalent summaries.
    pub support: Vec<VersionReference>,
    /// Context interpretations that must survive directly or through valid baselines.
    pub interpretation: Vec<VersionStamp>,
    /// Frozen public namespace descriptions retained without live service credentials.
    pub destinations: Vec<Destination>,
    /// Atomic descriptors still needed by surviving facts/effects.
    pub units: Vec<CoupledRequirement>,
    /// Closed acquisition rows no surviving semantic/support/unit requirement needs.
    pub removable_staging: BTreeSet<RecordKey>,
}

/// Identify durable service facts and dependencies before proposing physical collection.
///
/// Supply the complete relevant retained row/unit view. This does not select a
/// time-based expiry, discard distinct attempts, or choose a common sync boundary.
#[tracing::instrument(
    skip_all,
    name = "sync.retention.requirements.calculate",
    level = "debug"
)]
pub fn requirements(
    records: &[AcceptedRecord],
    units: &[CoupledRequirement],
) -> anyhow::Result<RetentionRequirements> {
    let mut result = RetentionRequirements {
        semantic_keys: BTreeSet::new(),
        support: Vec::new(),
        interpretation: Vec::new(),
        destinations: Vec::new(),
        units: Vec::new(),
        removable_staging: BTreeSet::new(),
    };
    let mut identities = BTreeSet::new();
    let mut account = None;
    for row in records {
        row.validate()?;
        anyhow::ensure!(
            identities.insert(row.key),
            "duplicate retention record {:?}",
            row.key
        );
        if let Some(a) = account {
            anyhow::ensure!(a == row.key.account, "retention crosses account");
        } else {
            account = Some(row.key.account);
        }
        if let Some(s) = super::payload(row) {
            match &s.payload {
                ServicePayload::Acquisition(a) if a.phase != AcquisitionPhase::Active => {
                    result.removable_staging.insert(row.key);
                }
                _ => {
                    result.semantic_keys.insert(row.key);
                }
            }
            if let Some(d) = &s.support.destination
                && !result.destinations.contains(d)
            {
                result.destinations.push(d.clone());
            }
            if let ServicePayload::Destination(d) = &s.payload
                && !result.destinations.contains(d)
            {
                result.destinations.push(d.clone());
            }
            for req in &s.support.required {
                let r = req.reference();
                result.semantic_keys.insert(r.key);
                if !result.support.contains(r) {
                    result.support.push(r.clone());
                }
                if !result.interpretation.contains(&r.stamp) {
                    result.interpretation.push(r.stamp.clone());
                }
            }
            if let Some(stamp) = &s.support.acquisition
                && !result.interpretation.contains(stamp)
            {
                result.interpretation.push(stamp.clone());
            }
        }
    }
    let mut unit_ids = BTreeSet::new();
    for unit in units {
        anyhow::ensure!(
            unit_ids.insert(unit.id) && !unit.members.is_empty(),
            "duplicate or empty retained unit {}",
            unit.id
        );
        for member in &unit.members {
            anyhow::ensure!(
                account.is_none_or(|a| a == member.key.account),
                "retained unit crosses account"
            );
        }
        let needed = unit.members.iter().any(|m| {
            records.iter().any(|row| {
                row.key == m.key
                    && (super::payload(row).is_none() || result.semantic_keys.contains(&row.key))
                    && (row.stamp.accepted_at == m.stamp.accepted_at
                        || super::payload(row)
                            .is_some_and(|s| s.support.operation == Some(unit.id)))
            })
        });
        if needed {
            result.units.push(unit.clone());
            for member in &unit.members {
                result.semantic_keys.insert(member.key);
                if !result.support.contains(member) {
                    result.support.push(member.clone());
                }
                if !result.interpretation.contains(&member.stamp) {
                    result.interpretation.push(member.stamp.clone());
                }
            }
        }
    }
    result
        .removable_staging
        .retain(|key| !result.semantic_keys.contains(key));
    tracing::debug!(
        event = "service.retention.calculated",
        required_record_count = result.semantic_keys.len(),
        removable_staging_count = result.removable_staging.len()
    );
    Ok(result)
}

/// Reject a physical collection proposal that loses required stable semantic identities.
///
/// Run alongside normal baseline and equivalent-interpretation checks; this only
/// validates key retention and does not certify that a replacement summary is valid.
pub fn validate_collection(
    required: &RetentionRequirements,
    retained: &[RecordKey],
) -> anyhow::Result<()> {
    for key in &required.semantic_keys {
        anyhow::ensure!(
            retained.contains(key),
            "collection would remove required semantic record {key:?}"
        );
    }
    Ok(())
}

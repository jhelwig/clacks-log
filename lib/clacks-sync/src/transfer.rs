//! Bounded native discovery requirements and client installation results.

pub mod batch;
pub mod discovery;
pub mod readiness;
pub mod receipt;

use crate::{
    account::EvidenceView,
    client::ClientUpdate,
    compare::{RecordRelation, compare_contexts},
    lineage::Coverage,
    record::{AcceptedRecord, RecordKey, RecordState, VersionStamp},
};
use clacks_model::ids::OperationId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

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

/// Accepted data and working-record changes to commit together.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallBatch {
    /// Received accepted facts, including exact historical support.
    pub accepted: Vec<AcceptedRecord>,
    /// Client working and comparison updates for current accepted values.
    pub client_updates: Vec<ClientUpdate>,
    /// Incomplete candidates to retain independently of this exchange.
    pub staged: Vec<AcceptedRecord>,
    /// Coupled effects represented by this batch.
    pub completed_units: BTreeSet<OperationId>,
}

/// Check a concrete installed version against a context-only requirement.
pub(crate) fn satisfies(
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

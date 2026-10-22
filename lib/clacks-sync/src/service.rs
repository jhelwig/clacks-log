//! Account-scoped service carriers, retained support and real-store binding guards.

pub mod association;
pub mod combination;
pub mod dependency;
pub mod download;
pub mod operation;
pub mod recovery;
pub mod retention;

use crate::{
    account::EvidenceView,
    record::{AcceptedRecord, RecordKey, RecordState, RecordValue, VersionStamp},
};
use clacks_model::{
    ids::{AccountId, AssociationId, OperationId, QsoId},
    service::{
        destination::{Destination, ExternalRecordKey},
        evidence::ServicePayload,
        evidence::processing::AssociationState,
    },
};
use serde::{Deserialize, Serialize};

/// An accepted version required by retained interpretation or a coupled effect.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionReference {
    /// Account-qualified identity.
    pub key: RecordKey,
    /// Original acceptance context, interpretable through retained baselines.
    pub stamp: VersionStamp,
    /// Whether the required version is a deletion.
    pub deleted: bool,
}

impl From<&AcceptedRecord> for VersionReference {
    fn from(v: &AcceptedRecord) -> Self {
        Self {
            key: v.key,
            stamp: v.stamp.clone(),
            deleted: matches!(v.state, RecordState::Deleted),
        }
    }
}

/// Required supporting fact versus an effect representable by later accepted state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SupportRequirement {
    /// Immutable semantic fact must remain available.
    ExactFact(VersionReference),
    /// This accepted effect or an established successor/deletion suffices.
    CoveredEffect(VersionReference),
}

impl SupportRequirement {
    /// Borrow the referenced version for transport/retention interpretation.
    pub fn reference(&self) -> &VersionReference {
        match self {
            Self::ExactFact(v) | Self::CoveredEffect(v) => v,
        }
    }
}

/// Self-contained interpretation saved atomically beside domain evidence.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceSupport {
    /// Required accepted facts and effect references.
    pub required: Vec<SupportRequirement>,
    /// Atomic operation membership, when applicable.
    pub operation: Option<OperationId>,
    /// Original accepted query-start context, not response installation order.
    pub acquisition: Option<VersionStamp>,
    /// Frozen public namespace description; live credentials are unnecessary.
    pub destination: Option<Destination>,
}

/// Synchronized service domain value with its original interpretation dependencies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceRecord {
    /// Domain content without clock dependencies.
    pub payload: ServicePayload,
    /// Retained synchronization support, separate from temporary write guards.
    pub support: ServiceSupport,
}

impl ServiceRecord {
    /// Validate carrier structure and account isolation without claiming its support is installed.
    pub fn validate(&self, account: AccountId) -> anyhow::Result<()> {
        self.payload.validate()?;
        if self
            .support
            .required
            .iter()
            .any(|r| r.reference().key.account != account)
            || self
                .support
                .destination
                .as_ref()
                .is_some_and(|d| self.payload.destination().is_some_and(|id| d.id != id))
        {
            return Err(dependency::SyncServiceError::Scope {
                account,
                detail: format!(
                    "service {:?} has cross-account/namespace support",
                    self.payload.id()
                ),
            }
            .into());
        }
        Ok(())
    }
}

/// Maintained association version observed through the actual unique binding index.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingVersion {
    /// Existing association record.
    pub association: AssociationId,
    /// Local contact currently occupying the remote identity.
    pub qso: QsoId,
    /// Compared accepted binding context.
    pub stamp: VersionStamp,
    /// Active or retained historical/deletion state.
    pub state: AssociationState,
}

/// Explicit index observation; an omitted row is not certified absence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalBindingObservation {
    /// Account whose unique index was read in the transaction.
    pub account: AccountId,
    /// Exact namespace-qualified index key.
    pub external: ExternalRecordKey,
    /// Existing occupant, or explicit transactionally observed absence.
    pub binding: Option<BindingVersion>,
}

/// Unique-index condition the actual transaction must recheck, including phantom inserts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalBindingGuard {
    /// Owning account.
    pub account: AccountId,
    /// Unique index lookup key.
    pub external: ExternalRecordKey,
    /// Exact occupant or observed absence required at commit.
    pub expected: Option<BindingVersion>,
}

/// Relevant accepted rows and namespace observations from one real account read view.
pub struct ServiceView<'a> {
    /// Account isolation domain.
    pub account: AccountId,
    /// Durably retained records supporting this operation.
    pub records: &'a [AcceptedRecord],
    /// Explicit unique-index reads.
    pub bindings: &'a [ExternalBindingObservation],
    /// Existing lineage and baseline interpretation.
    pub evidence: &'a EvidenceView<'a>,
}

/// Borrow a service value from a validated accepted native envelope.
pub(crate) fn payload(record: &AcceptedRecord) -> Option<&ServiceRecord> {
    if let RecordState::Live(RecordValue::Service(v)) = &record.state {
        Some(v)
    } else {
        None
    }
}

/// Reject rewriting immutable semantic content before version-order shortcuts can hide it.
pub(crate) fn check_integrity(
    key: RecordKey,
    old: &RecordState,
    new: &RecordState,
) -> anyhow::Result<()> {
    if let RecordState::Live(RecordValue::Service(a)) = old
        && a.payload.immutable()
    {
        let compatible = if let RecordState::Live(RecordValue::Service(b)) = new {
            a.payload == b.payload
                && a.support.operation == b.support.operation
                && a.support.destination == b.support.destination
                && a.support.acquisition.as_ref().map(|s| s.accepted_at)
                    == b.support.acquisition.as_ref().map(|s| s.accepted_at)
                && a.support.required.len() == b.support.required.len()
                && a.support.required.iter().all(|r| {
                    b.support.required.iter().any(|v| {
                        std::mem::discriminant(r) == std::mem::discriminant(v)
                            && r.reference().key == v.reference().key
                            && r.reference().stamp.accepted_at == v.reference().stamp.accepted_at
                            && r.reference().deleted == v.reference().deleted
                    })
                })
        } else {
            false
        };
        if !compatible {
            return Err(dependency::SyncServiceError::Integrity {
                key,
                detail: "immutable service identity has different content/support".into(),
            }
            .into());
        }
    }
    Ok(())
}

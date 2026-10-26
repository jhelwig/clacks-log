//! Metadata and validation for synchronizing external-service evidence.

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
        if let Some(acquisition) = &self.support.acquisition {
            acquisition.validate()?;
        }
        for requirement in &self.support.required {
            requirement.reference().stamp.validate()?;
        }
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

/// Compare retained support identities independently of equivalent clock representations.
pub(crate) fn same_support_values(a: &ServiceSupport, b: &ServiceSupport) -> bool {
    a.operation == b.operation
        && a.destination == b.destination
        && a.acquisition.as_ref().map(|s| s.accepted_at)
            == b.acquisition.as_ref().map(|s| s.accepted_at)
        && a.required.len() == b.required.len()
        && a.required.iter().all(|r| {
            b.required.iter().any(|s| {
                std::mem::discriminant(r) == std::mem::discriminant(s)
                    && r.reference().key == s.reference().key
                    && r.reference().stamp.accepted_at == s.reference().stamp.accepted_at
                    && r.reference().deleted == s.reference().deleted
            })
        })
}

/// Establish equivalent embedded projections, reporting unavailable interpretation separately.
pub(crate) fn compare_support(
    a: &ServiceRecord,
    b: &ServiceRecord,
    key: RecordKey,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<crate::compare::RecordRelation> {
    use crate::compare::{RecordRelation, compare_contexts};
    if a == b {
        return Ok(RecordRelation::SameVersion);
    }
    if a.payload != b.payload || !same_support_values(&a.support, &b.support) {
        return Ok(RecordRelation::Divergent);
    }
    for r in &a.support.required {
        let Some(s) = b.support.required.iter().find(|s| {
            std::mem::discriminant(r) == std::mem::discriminant(*s)
                && r.reference().key == s.reference().key
                && r.reference().stamp.accepted_at == s.reference().stamp.accepted_at
        }) else {
            return Ok(RecordRelation::Divergent);
        };
        if r.reference().stamp == s.reference().stamp {
            continue;
        }
        let relation = compare_contexts(
            r.reference().key,
            &r.reference().stamp,
            &s.reference().stamp,
            evidence,
        )?;
        if relation != RecordRelation::SameVersion {
            return Ok(relation);
        }
    }
    if let (Some(a), Some(b)) = (&a.support.acquisition, &b.support.acquisition) {
        if a == b {
            return Ok(RecordRelation::SameVersion);
        }
        return compare_contexts(key, a, b, evidence);
    }
    Ok(RecordRelation::SameVersion)
}

/// Complete remote-collection registry read under the account-head transaction guard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DestinationRegistryObservation {
    /// Owning account, isolated from other accounts' provider collections.
    pub account: AccountId,
    /// Account head at which the complete registry was observed.
    pub head: crate::record::Position,
    /// All retained public namespace descriptors, including credential-removed destinations.
    pub destinations: Vec<Destination>,
}

/// Check collective namespace identity before admitting any newly installed service evidence.
pub(crate) fn check_namespace(
    record: &ServiceRecord,
    input: &crate::acceptance::AcceptanceInput<'_>,
) -> anyhow::Result<Result<(), crate::acceptance::RejectionReason>> {
    let Some(id) = record.payload.destination() else {
        return Ok(Ok(()));
    };
    let Some(registry) = input.destination_registry else {
        return Ok(Err(crate::acceptance::RejectionReason::ServiceCondition {
            detail: format!(
                "account {} destination {id} requires a complete destination-registry observation",
                input.account.account
            ),
        }));
    };
    anyhow::ensure!(
        registry.account == input.account.account && registry.head == input.account.head,
        "destination registry account/head changed"
    );
    clacks_model::service::destination::validate_destinations(&registry.destinations)?;
    let proposed = if let ServicePayload::Destination(d) = &record.payload {
        Some(d)
    } else {
        record.support.destination.as_ref()
    }
    .or_else(|| registry.destinations.iter().find(|d| d.id == id));
    let Some(proposed) = proposed else {
        return Ok(Err(crate::acceptance::RejectionReason::ServiceCondition {
            detail: format!("destination {id} lacks its public namespace descriptor"),
        }));
    };
    let mut all = registry.destinations.clone();
    if let Some(old) = all.iter_mut().find(|d| d.id == proposed.id) {
        use clacks_model::service::destination::RemoteCollection;
        let namespace_update = matches!(&record.payload, ServicePayload::Destination(_));
        let compatible = match (&old.collection, &proposed.collection) {
            (RemoteCollection::Lotw { account: a }, RemoteCollection::Lotw { account: b }) => {
                a.trim().eq_ignore_ascii_case(b.trim())
            }
            (RemoteCollection::Qrz { book_id: a }, RemoteCollection::Qrz { book_id: b }) => {
                a == b || a.is_none() || (!namespace_update && b.is_none())
            }
            _ => false,
        };
        if !compatible {
            return Err(dependency::SyncServiceError::Integrity {
                key: RecordKey {
                    account: input.account.account,
                    id: crate::record::RecordId::Service(record.payload.id()),
                },
                detail: format!("destination {id} changes its retained remote collection"),
            }
            .into());
        }
        if namespace_update || matches!(&old.collection, RemoteCollection::Qrz { book_id: None }) {
            *old = proposed.clone();
        }
    } else {
        all.push(proposed.clone());
    }
    if let Err(error) = clacks_model::service::destination::validate_destinations(&all) {
        return Ok(Err(crate::acceptance::RejectionReason::ServiceCondition {
            detail: format!(
                "account {} destination {id} namespace collision: {error}",
                input.account.account
            ),
        }));
    }
    Ok(Ok(()))
}

/// Retain an admitted namespace refinement for later members of the same transaction.
pub(crate) fn retain_destination(destinations: &mut Vec<Destination>, row: &AcceptedRecord) {
    let Some(record) = payload(row) else {
        return;
    };
    let proposed = match &record.payload {
        ServicePayload::Destination(d) => Some(d),
        _ => record.support.destination.as_ref(),
    };
    let Some(proposed) = proposed else {
        return;
    };
    if let Some(old) = destinations.iter_mut().find(|d| d.id == proposed.id) {
        if matches!(
            &old.collection,
            clacks_model::service::destination::RemoteCollection::Qrz { book_id: None }
        ) && matches!(
            &proposed.collection,
            clacks_model::service::destination::RemoteCollection::Qrz { book_id: Some(_) }
        ) {
            *old = proposed.clone();
        }
    } else {
        destinations.push(proposed.clone());
    }
}

/// Identify candidates that establish a known namespace before unrelated members are admitted.
///
/// This only supplies planning priority; ordinary admission must still validate the
/// proposed identity and all source guards before retaining it.
fn refines_destination(destinations: &[Destination], state: &RecordState) -> bool {
    let RecordState::Live(RecordValue::Service(record)) = state else {
        return false;
    };
    let proposed = match &record.payload {
        ServicePayload::Destination(d) => Some(d),
        _ => record.support.destination.as_ref(),
    };
    proposed.is_some_and(|proposed| {
        matches!(
            &proposed.collection,
            clacks_model::service::destination::RemoteCollection::Qrz { book_id: Some(_) }
        ) && destinations.iter().any(|old| {
            old.id == proposed.id
                && matches!(
                    &old.collection,
                    clacks_model::service::destination::RemoteCollection::Qrz { book_id: None }
                )
        })
    })
}

/// Prioritize namespace refinements and their pending prerequisites before unrelated candidates.
pub(crate) fn namespace_prerequisites(
    destinations: &[Destination],
    candidates: &[(RecordKey, &RecordState, Vec<RecordKey>)],
) -> std::collections::BTreeSet<RecordKey> {
    let mut required: std::collections::BTreeSet<_> = candidates
        .iter()
        .filter(|(_, state, _)| refines_destination(destinations, state))
        .map(|(key, _, _)| *key)
        .collect();
    loop {
        let before = required.len();
        for (key, _, dependencies) in candidates {
            if required.contains(key) {
                required.extend(dependencies.iter().copied());
            }
        }
        if required.len() == before {
            return required;
        }
    }
}

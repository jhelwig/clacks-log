use crate::support;
use clacks_model::{
    ids::{AssertionId, AttemptId, QsoId, SubmissionResultId},
    scalar::UtcTimestamp,
    service::{
        destination::Service,
        evidence::{
            ImportProvenance, ImportedAssertion, ImportedBaseline, ImportedClaim, ServicePayload,
            submission::{
                ConfirmationBasis, DispatchPhase, ResultScope, SubmissionOutcome, SubmissionResult,
            },
        },
    },
};
use clacks_sync::{
    account::{AccountMetadata, EvidenceView},
    record::{RecordId, RecordKey, RecordState, RecordValue},
    service::{
        ServiceRecord, ServiceSupport, ServiceView,
        combination::combine,
        dependency::{DependencyStatus, check},
    },
};

fn assertion() -> ServicePayload {
    ServicePayload::Imported(Box::new(ImportedAssertion {
        id: AssertionId::new(),
        qso: QsoId::new(),
        service: Service::Lotw,
        destination: None,
        claim: ImportedClaim::Uploaded,
        date: None,
        provenance: ImportProvenance {
            source: "file".into(),
            record: "1".into(),
            mapping: "ADIF".into(),
        },
        baseline: ImportedBaseline {
            native_end: None,
            service: Service::Lotw,
            fields: Default::default(),
            own_location: None,
            confirmation: ConfirmationBasis::default(),
            coverage: Default::default(),
        },
    }))
}

#[test]
fn immutable_content_is_checked_even_when_context_covers_old_version() -> anyhow::Result<()> {
    let payload = assertion();
    let mut old = support::accepted(200, "fixture");
    old.key = RecordKey {
        account: support::id(1),
        id: RecordId::Service(payload.id()),
    };
    old.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
        payload,
        support: ServiceSupport::default(),
    })));
    let mut new = support::accepted(201, "fixture");
    new.key = old.key;
    new.state = old.state.clone();
    if let RecordState::Live(RecordValue::Service(s)) = &mut new.state
        && let ServicePayload::Imported(a) = &mut s.payload
    {
        a.claim = ImportedClaim::Confirmed;
    }
    let (_, b) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let bases = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &bases,
    };
    let view = ServiceView {
        account: support::id(1),
        records: &[],
        bindings: &[],
        evidence: &ev,
    };
    assert!(combine(&old, &new, &view).is_err());
    Ok(())
}

#[test]
fn outcome_without_attempt_is_incomplete_not_success() -> anyhow::Result<()> {
    let destination = clacks_model::service::destination::Destination {
        id: support::id(1),
        collection: clacks_model::service::destination::RemoteCollection::Qrz {
            book_id: Some("42".parse().unwrap()),
        },
        description: "book".into(),
    };
    let result = SubmissionResult {
        external_attempt: None,
        id: SubmissionResultId::new(),
        destination: destination.id,
        scope: ResultScope::IndividualAttempt(AttemptId::new()),
        outcome: SubmissionOutcome::Success,
        phase: DispatchPhase::Dispatched,
        at: UtcTimestamp::new(0, 0)?,
        diagnostic: None,
        external: None,
    };
    let record = ServiceRecord {
        payload: ServicePayload::Result(result),
        support: ServiceSupport {
            destination: Some(destination),
            ..Default::default()
        },
    };
    let (_, b) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let bases = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &bases,
    };
    let view = ServiceView {
        account: support::id(1),
        records: &[],
        bindings: &[],
        evidence: &ev,
    };
    assert!(matches!(
        check(&record, &view)?,
        DependencyStatus::Incomplete(_)
    ));
    Ok(())
}

#[test]
fn generic_acceptance_cannot_rewrite_immutable_service_facts() -> anyhow::Result<()> {
    use clacks_sync::{
        acceptance::{AcceptanceInput, ServerChange, plan_server_acceptance},
        client::VersionCondition,
        membership::MembershipSnapshot,
        record::Observation,
    };
    let payload = assertion();
    let mut old = support::accepted(200, "native fixture");
    old.key.id = RecordId::Service(payload.id());
    old.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
        payload,
        support: ServiceSupport::default(),
    })));
    let (mut account, b) =
        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    account.head.revision = 200;
    let bases = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &bases,
    };
    let members = MembershipSnapshot {
        account: account.account,
        generation: 0,
        clients: vec![],
    };
    let current = Observation::Accepted(old.clone());
    let mut state = old.state.clone();
    if let RecordState::Live(RecordValue::Service(v)) = &mut state
        && let ServicePayload::Imported(a) = &mut v.payload
    {
        a.provenance.record = "different".into();
    }
    let change = ServerChange {
        key: old.key,
        state,
        expected: VersionCondition::from_observation(&current)?,
        dependencies: vec![],
    };
    assert!(
        plan_server_acceptance(
            &change,
            &AcceptanceInput {
                destination_registry: None,
                account: &account,
                membership: &members,
                current: &current,
                references: &[],
                bindings: &[]
            },
            &ev
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn common_admission_rejects_a_second_namespace_for_the_same_remote_collection() -> anyhow::Result<()>
{
    use clacks_model::service::destination::{Destination, RemoteCollection};
    use clacks_sync::{
        acceptance::{AcceptanceDecision, AcceptanceInput, ServerChange, plan_server_acceptance},
        account::InstalledCheckpoint,
        client::VersionCondition,
        membership::MembershipSnapshot,
        record::Observation,
        service::DestinationRegistryObservation,
    };
    let (mut account, b) =
        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    account.head.revision = 100;
    let bases = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &bases,
    };
    let membership = MembershipSnapshot {
        account: account.account,
        generation: 0,
        clients: vec![],
    };
    let old = Destination {
        id: support::id(41),
        collection: RemoteCollection::Qrz {
            book_id: Some("42".parse().unwrap()),
        },
        description: "first".into(),
    };
    let new = Destination {
        id: support::id(42),
        description: "alias".into(),
        ..old.clone()
    };
    let registry = DestinationRegistryObservation {
        account: account.account,
        head: account.head,
        destinations: vec![old],
    };
    let key = RecordKey {
        account: account.account,
        id: RecordId::Service(
            clacks_model::service::evidence::ServiceRecordId::Destination(new.id),
        ),
    };
    let current = Observation::Absent {
        key,
        checkpoint: InstalledCheckpoint {
            account: account.account,
            baseline: b.id,
            coverage: account.installed.clone(),
        },
    };
    let change = ServerChange {
        key,
        state: RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
            payload: ServicePayload::Destination(new),
            support: Default::default(),
        }))),
        expected: VersionCondition::from_observation(&current)?,
        dependencies: vec![],
    };
    assert!(matches!(
        plan_server_acceptance(
            &change,
            &AcceptanceInput {
                destination_registry: Some(&registry),
                account: &account,
                membership: &membership,
                current: &current,
                references: &[],
                bindings: &[]
            },
            &ev
        )?,
        AcceptanceDecision::Rejected(_)
    ));
    let mut authorized = membership.clone();
    authorized
        .clients
        .push(clacks_sync::membership::ClientMembership {
            actor: support::id(4),
            created_at: account.head,
            closed_at: None,
            acknowledged: None,
        });
    let mut incoming = support::accepted(190, "fixture");
    incoming.key = key;
    incoming.state = change.state.clone();
    assert!(matches!(
        clacks_sync::recovery::plan_recovered_install(
            &incoming,
            None,
            support::id(4),
            &AcceptanceInput {
                destination_registry: Some(&registry),
                account: &account,
                membership: &authorized,
                current: &current,
                references: &[],
                bindings: &[]
            },
            &ev
        )?,
        clacks_sync::recovery::RecoveredDecision::DeferredService { .. }
    ));
    Ok(())
}

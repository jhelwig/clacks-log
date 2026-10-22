mod support;
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
    ServicePayload::Imported(ImportedAssertion {
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
            service: Service::Lotw,
            fields: Default::default(),
            own_location: None,
            confirmation: ConfirmationBasis::default(),
            coverage: Default::default(),
        },
    })
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
        collection: clacks_model::service::destination::RemoteCollection::Qrz { book_id: Some(42) },
        description: "book".into(),
    };
    let result = SubmissionResult {
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

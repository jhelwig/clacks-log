mod support;
use clacks_model::service::{
    destination::{ExternalRecordKey, RemoteRecordId},
    evidence::processing::{AssociationState, ExternalAssociation},
};
use clacks_sync::{
    account::AccountMetadata,
    record::{Observation, RecordId},
    service::{
        ExternalBindingObservation,
        association::{BindingInput, plan_binding},
        operation::ServiceDecision,
    },
};

#[test]
fn simultaneous_binding_plans_carry_identical_absence_guard() -> anyhow::Result<()> {
    let a = support::accepted(200, "q1");
    let mut b = support::accepted(200, "q2");
    let RecordId::Qso(q1) = a.key.id else {
        panic!()
    };
    let q2 = support::id(99);
    b.key.id = RecordId::Qso(q2);
    if let clacks_sync::record::RecordState::Live(clacks_sync::record::RecordValue::Qso(q)) =
        &mut b.state
    {
        q.id = q2;
    }
    let key = ExternalRecordKey {
        destination: support::id(42),
        record: RemoteRecordId::Qrz(7),
    };
    let index = ExternalBindingObservation {
        account: a.key.account,
        external: key.clone(),
        binding: None,
    };
    let a1 = ExternalAssociation {
        id: support::id(11),
        external: key.clone(),
        qso: q1,
        state: AssociationState::Active,
    };
    let a2 = ExternalAssociation {
        id: support::id(12),
        external: key,
        qso: q2,
        state: AssociationState::Active,
    };
    let ServiceDecision::Ready(p1) = plan_binding(&BindingInput {
        account: a.key.account,
        association: &a1,
        index: &index,
        target: &Observation::Accepted(a),
    })?
    else {
        panic!()
    };
    let ServiceDecision::Ready(p2) = plan_binding(&BindingInput {
        account: b.key.account,
        association: &a2,
        index: &index,
        target: &Observation::Accepted(b),
    })?
    else {
        panic!()
    };
    assert_eq!(p1.guard, p2.guard);
    Ok(())
}

#[test]
fn import_qso_and_claim_form_one_guarded_operation() -> anyhow::Result<()> {
    use clacks_model::{
        ids::OperationId,
        service::{
            destination::Service,
            evidence::submission::ConfirmationBasis,
            evidence::{
                ImportProvenance, ImportedAssertion, ImportedBaseline, ImportedClaim,
                ServiceRecordId,
            },
        },
    };
    use clacks_sync::{
        acceptance::ServerChange,
        account::{EvidenceView, InstalledCheckpoint},
        client::VersionCondition,
        membership::MembershipSnapshot,
        record::{RecordKey, RecordState, RecordValue},
        service::operation::{
            NativeChange, ServiceOperation, ServiceOperationInput, plan_operation,
        },
    };
    let (account, base) =
        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let bases = [base.clone()];
    let evidence = EvidenceView {
        lineage: &base.lineage,
        baselines: &bases,
    };
    let membership = MembershipSnapshot {
        account: account.account,
        generation: 0,
        clients: vec![],
    };
    let q = support::qso(support::id(100));
    let qkey = RecordKey {
        account: account.account,
        id: RecordId::Qso(q.id),
    };
    let claim = ImportedAssertion {
        id: support::id(101),
        qso: q.id,
        service: Service::Lotw,
        destination: None,
        claim: ImportedClaim::Uploaded,
        date: None,
        provenance: ImportProvenance {
            source: "file".into(),
            record: "1".into(),
            mapping: "standard".into(),
        },
        baseline: ImportedBaseline {
            native_end: None,
            service: Service::Lotw,
            fields: Default::default(),
            own_location: None,
            confirmation: ConfirmationBasis::default(),
            coverage: Default::default(),
        },
    };
    let checkpoint = InstalledCheckpoint {
        account: account.account,
        baseline: base.id,
        coverage: account.installed.clone(),
    };
    let absent = Observation::Absent {
        key: qkey,
        checkpoint: checkpoint.clone(),
    };
    let reads = [
        absent.clone(),
        Observation::Absent {
            key: RecordKey {
                account: account.account,
                id: RecordId::Service(ServiceRecordId::Assertion(claim.id)),
            },
            checkpoint,
        },
    ];
    let op = ServiceOperation::ApplyImport {
        qso_change: NativeChange::Server(ServerChange {
            key: qkey,
            state: RecordState::Live(RecordValue::Qso(Box::new(q))),
            expected: VersionCondition::from_observation(&absent)?,
            dependencies: vec![],
        }),
        assertions: vec![claim],
        selected_tag_effects: vec![],
        association: None,
    };
    let ServiceDecision::Ready(commit) = plan_operation(&ServiceOperationInput {
        id: OperationId::new(),
        account: &account,
        membership: &membership,
        reads: &reads,
        bindings: &[],
        destinations: &[],
        evidence: &evidence,
        operation: &op,
        matching: None,
    })?
    else {
        panic!("not ready")
    };
    assert_eq!(commit.accepted.len(), 2);
    assert_eq!(commit.unit.members.len(), 2);
    assert_eq!(commit.expected_head.revision, 0);
    assert_eq!(commit.next_account.head.revision, 2);
    Ok(())
}

fn remote(
    q: &clacks_model::contact::Qso,
) -> clacks_model::service::evidence::observation::RemoteObservation {
    use clacks_model::service::evidence::observation::*;
    RemoteObservation {
        id: support::id(501),
        destination: support::id(42),
        identity: RemoteIdentity::Complete(ExternalRecordKey {
            destination: support::id(42),
            record: RemoteRecordId::Qrz(7),
        }),
        facts: RemoteFacts {
            contact: ContactReport {
                own_call: Reported::Value(q.own_station.callsign.clone()),
                worked_call: Reported::Value(q.contacted_station.callsign.clone()),
                start: Reported::Value(ObservedStart::Second(q.start)),
                band: Reported::Value("20m".into()),
                mode: Reported::Value(ModeReport {
                    concrete: "FT4".into(),
                    parent: Some("MFSK".into()),
                    group: None,
                }),
                ..Default::default()
            },
            ..Default::default()
        },
        ordering: Default::default(),
        acquisition: AcquisitionProvenance {
            id: support::id(502),
            query: QueryIdentity {
                destination: support::id(42),
                stream: QueryStream::Qrz,
                selected_fields: Default::default(),
                own_call: None,
                since: None,
            },
            observed_at: clacks_model::scalar::UtcTimestamp::new(1000, 0).unwrap(),
        },
    }
}

#[test]
fn candidates_respect_identity_hierarchy_and_time_coverage() -> anyhow::Result<()> {
    use clacks_model::{
        recognition::RecognitionCatalog,
        scalar::{RecordedCallsign, UtcSecond},
        service::{
            evidence::observation::{ObservedStart, Reported},
            lotw::LotwConfiguration,
        },
    };
    use clacks_sync::service::association::{CandidateInput, classify_candidates};
    let row = support::accepted(200, "fixture");
    let RecordId::Qso(qid) = row.key.id else {
        panic!()
    };
    let q = support::qso(qid);
    let mut obs = remote(&q);
    let rows = [row];
    let cfg = LotwConfiguration::bundled()?;
    let classify = |obs: &_| {
        classify_candidates(&CandidateInput {
            observation: obs,
            account: support::id(1),
            account_qsos: &rows,
            recognition: RecognitionCatalog::adif_318().unwrap(),
            lotw_configuration: &cfg,
        })
    };
    assert_eq!(classify(&obs)?.confident, vec![qid]);
    for (offset, count) in [(1, 1), (1800, 1), (1801, 0), (-1800, 1), (-1801, 0)] {
        obs.facts.contact.start = Reported::Value(ObservedStart::Second(
            UtcSecond::from_unix_seconds(1000 + offset),
        ));
        let c = classify(&obs)?;
        assert!(c.confident.is_empty());
        assert_eq!(c.possible.len(), count);
    }
    obs.facts.contact.start =
        Reported::Value(ObservedStart::Minute(UtcSecond::from_unix_seconds(960)));
    assert_eq!(classify(&obs)?.confident, vec![qid]);
    if let Reported::Value(m) = &mut obs.facts.contact.mode {
        m.concrete = "MFSK".into();
        m.parent = None;
    }
    assert_eq!(classify(&obs)?.possible.len(), 1);
    obs.facts.contact.own_call = Reported::NotReported;
    assert_eq!(classify(&obs)?.possible.len(), 1);
    obs.facts.contact.worked_call = Reported::Value(RecordedCallsign::new("K1ABC/P")?);
    assert!(classify(&obs)?.possible.is_empty());
    Ok(())
}

#[test]
fn occupied_historical_binding_never_automatically_reactivates() -> anyhow::Result<()> {
    use clacks_sync::service::BindingVersion;
    let row = support::accepted(200, "fixture");
    let RecordId::Qso(qso) = row.key.id else {
        panic!()
    };
    let key = ExternalRecordKey {
        destination: support::id(42),
        record: RemoteRecordId::Qrz(7),
    };
    let a = ExternalAssociation {
        id: support::id(11),
        external: key.clone(),
        qso,
        state: AssociationState::Active,
    };
    for (id, state) in [
        (support::id(12), AssociationState::Active),
        (a.id, AssociationState::Superseded),
        (a.id, AssociationState::DeletedQso),
    ] {
        let index = ExternalBindingObservation {
            account: row.key.account,
            external: key.clone(),
            binding: Some(BindingVersion {
                association: id,
                qso,
                state,
                stamp: row.stamp.clone(),
            }),
        };
        assert!(matches!(
            plan_binding(&BindingInput {
                account: row.key.account,
                association: &a,
                index: &index,
                target: &Observation::Accepted(row.clone())
            })?,
            ServiceDecision::Review(_)
        ));
    }
    let foreign = ExternalBindingObservation {
        account: support::id(99),
        external: key,
        binding: None,
    };
    assert!(
        plan_binding(&BindingInput {
            account: row.key.account,
            association: &a,
            index: &foreign,
            target: &Observation::Accepted(row)
        })
        .is_err()
    );
    Ok(())
}

#[test]
fn result_after_contact_deletion_retains_frozen_attempt_and_job_outcome() -> anyhow::Result<()> {
    use clacks_model::{
        scalar::UtcTimestamp,
        service::{
            destination::{Destination, RemoteCollection},
            evidence::{ServicePayload, ServiceRecordId, submission::*},
            qrz::QrzSubmission,
        },
    };
    use clacks_sync::{
        account::{EvidenceView, InstalledCheckpoint},
        membership::MembershipSnapshot,
        record::{RecordKey, RecordState, RecordValue},
        service::{ServiceRecord, ServiceSupport, operation::*},
    };
    let (mut account, base) =
        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    account.head.revision = 200;
    let bases = [base.clone()];
    let evidence = EvidenceView {
        lineage: &base.lineage,
        baselines: &bases,
    };
    let membership = MembershipSnapshot {
        account: account.account,
        generation: 0,
        clients: vec![],
    };
    let destination = Destination {
        id: support::id(42),
        collection: RemoteCollection::Qrz { book_id: Some(7) },
        description: "book".into(),
    };
    let attempt = SubmissionAttempt {
        id: support::id(510),
        qso: support::id(10),
        projection: SubmissionProjection::Qrz(QrzSubmission {
            destination: destination.id,
            fields: [("COMMENT".into(), "old frozen value".into())]
                .into_iter()
                .collect(),
        }),
        phase: DispatchPhase::Intent,
        at: UtcTimestamp::new(0, 0)?,
        batch: None,
    };
    let mut a = support::accepted(199, "fixture");
    a.key.id = RecordId::Service(ServiceRecordId::Attempt(attempt.id));
    a.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
        payload: ServicePayload::Attempt(attempt.clone()),
        support: ServiceSupport {
            destination: Some(destination.clone()),
            ..Default::default()
        },
    })));
    let result = SubmissionResult {
        external_attempt: None,
        id: support::id(511),
        destination: destination.id,
        scope: ResultScope::IndividualAttempt(attempt.id),
        outcome: SubmissionOutcome::Uncertain("response lost".into()),
        phase: DispatchPhase::PossibleSend,
        at: UtcTimestamp::new(1, 0)?,
        diagnostic: None,
        external: None,
    };
    let mut deleted = support::accepted(200, "fixture");
    deleted.state = RecordState::Deleted;
    let reads = [
        Observation::Accepted(deleted),
        Observation::Accepted(a),
        Observation::Absent {
            key: RecordKey {
                account: account.account,
                id: RecordId::Service(ServiceRecordId::Result(result.id)),
            },
            checkpoint: InstalledCheckpoint {
                account: account.account,
                baseline: base.id,
                coverage: account.installed.clone(),
            },
        },
    ];
    let operation = ServiceOperation::RecordSubmissionResult {
        result: result.clone(),
        batch_members: vec![attempt.id],
        association: None,
    };
    let ServiceDecision::Ready(commit) = plan_operation(&ServiceOperationInput {
        id: support::id(512),
        account: &account,
        membership: &membership,
        reads: &reads,
        bindings: &[],
        destinations: &[destination],
        evidence: &evidence,
        operation: &operation,
        matching: None,
    })?
    else {
        panic!()
    };
    assert_eq!(commit.accepted.len(), 1);
    assert_eq!(commit.job_updates[0].outcome, result.outcome);
    assert!(
        commit
            .accepted
            .iter()
            .all(|r| !matches!(r.state, RecordState::Live(RecordValue::Qso(_))))
    );
    Ok(())
}

#[test]
fn generic_binding_admission_rejects_historical_reactivation() -> anyhow::Result<()> {
    use clacks_model::service::{
        destination::{Destination, RemoteCollection},
        evidence::{ServicePayload, ServiceRecordId},
    };
    use clacks_sync::{
        acceptance::{AcceptanceDecision, AcceptanceInput, ServerChange, plan_server_acceptance},
        account::EvidenceView,
        client::VersionCondition,
        membership::MembershipSnapshot,
        record::{RecordKey, RecordState, RecordValue},
        service::{BindingVersion, ServiceRecord, ServiceSupport},
    };
    let q = support::accepted(200, "fixture");
    let RecordId::Qso(qso) = q.key.id else {
        panic!()
    };
    let destination = Destination {
        id: support::id(42),
        collection: RemoteCollection::Qrz { book_id: Some(7) },
        description: "book".into(),
    };
    let a = ExternalAssociation {
        id: support::id(11),
        external: ExternalRecordKey {
            destination: destination.id,
            record: RemoteRecordId::Qrz(7),
        },
        qso,
        state: AssociationState::Superseded,
    };
    let mut old = q.clone();
    old.key = RecordKey {
        account: q.key.account,
        id: RecordId::Service(ServiceRecordId::Association(a.id)),
    };
    old.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
        payload: ServicePayload::Association(a.clone()),
        support: ServiceSupport {
            destination: Some(destination.clone()),
            ..Default::default()
        },
    })));
    let current = Observation::Accepted(old.clone());
    let mut new = old.state.clone();
    if let RecordState::Live(RecordValue::Service(s)) = &mut new
        && let ServicePayload::Association(a) = &mut s.payload
    {
        a.state = AssociationState::Active;
    }
    let bindings = [ExternalBindingObservation {
        account: q.key.account,
        external: a.external.clone(),
        binding: Some(BindingVersion {
            association: a.id,
            qso,
            stamp: old.stamp.clone(),
            state: a.state,
        }),
    }];
    let (mut account, b) =
        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    account.head.revision = 200;
    let bases = [b.clone()];
    let evidence = EvidenceView {
        lineage: &b.lineage,
        baselines: &bases,
    };
    let membership = MembershipSnapshot {
        account: account.account,
        generation: 0,
        clients: vec![],
    };
    let change = ServerChange {
        key: old.key,
        state: new,
        expected: VersionCondition::from_observation(&current)?,
        dependencies: vec![],
    };
    let registry = clacks_sync::service::DestinationRegistryObservation {
        account: account.account,
        head: account.head,
        destinations: vec![destination],
    };
    assert!(matches!(
        plan_server_acceptance(
            &change,
            &AcceptanceInput {
                destination_registry: Some(&registry),
                account: &account,
                membership: &membership,
                current: &current,
                references: &[Observation::Accepted(q)],
                bindings: &bindings
            },
            &evidence
        )?,
        AcceptanceDecision::Rejected(_)
    ));
    Ok(())
}

fn absent_service(payload: &clacks_model::service::evidence::ServicePayload) -> Observation {
    use clacks_sync::{account::InstalledCheckpoint, record::RecordKey};
    let (account, b) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    Observation::Absent {
        key: RecordKey {
            account: account.account,
            id: RecordId::Service(payload.id()),
        },
        checkpoint: InstalledCheckpoint {
            account: account.account,
            baseline: b.id,
            coverage: account.installed,
        },
    }
}

fn run_operation(
    op: &clacks_sync::service::operation::ServiceOperation,
    reads: &[Observation],
    bindings: &[ExternalBindingObservation],
) -> anyhow::Result<ServiceDecision<clacks_sync::service::operation::CoupledCommit>> {
    use clacks_model::service::{
        destination::{Destination, RemoteCollection},
        lotw::LotwConfiguration,
    };
    use clacks_sync::{
        account::EvidenceView,
        membership::MembershipSnapshot,
        service::operation::{MatchContext, ServiceOperationInput, plan_operation},
    };
    let (mut account, b) =
        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    account.head.revision = 200;
    let bases = [b.clone()];
    let evidence = EvidenceView {
        lineage: &b.lineage,
        baselines: &bases,
    };
    let membership = MembershipSnapshot {
        account: account.account,
        generation: 0,
        clients: vec![],
    };
    let cfg = LotwConfiguration::bundled()?;
    plan_operation(&ServiceOperationInput {
        id: support::id(700),
        account: &account,
        membership: &membership,
        reads,
        bindings,
        destinations: &[Destination {
            id: support::id(42),
            collection: RemoteCollection::Qrz { book_id: Some(7) },
            description: "book".into(),
        }],
        evidence: &evidence,
        operation: op,
        matching: Some(MatchContext {
            recognition: clacks_model::recognition::RecognitionCatalog::adif_318()?,
            lotw: &cfg,
        }),
    })
}

#[test]
fn completed_review_commits_binding_and_effect_with_exact_observation() -> anyhow::Result<()> {
    use clacks_model::service::{
        destination::{Destination, RemoteCollection},
        evidence::{
            ServicePayload,
            processing::{ProcessingDecision, ProcessingDisposition},
        },
    };
    use clacks_sync::{
        record::{RecordState, RecordValue},
        service::{ServiceRecord, ServiceSupport, operation::ServiceOperation},
    };
    let q = support::accepted(200, "fixture");
    let RecordId::Qso(qso) = q.key.id else {
        panic!()
    };
    let obs = remote(&support::qso(qso));
    let mut row = q.clone();
    row.key.id = RecordId::Service(ServicePayload::Observation(Box::new(obs.clone())).id());
    row.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
        payload: ServicePayload::Observation(Box::new(obs.clone())),
        support: ServiceSupport {
            destination: Some(Destination {
                id: support::id(42),
                collection: RemoteCollection::Qrz { book_id: Some(7) },
                description: "book".into(),
            }),
            ..Default::default()
        },
    })));
    let key = ExternalRecordKey {
        destination: support::id(42),
        record: RemoteRecordId::Qrz(7),
    };
    let association = ExternalAssociation {
        id: support::id(601),
        external: key.clone(),
        qso,
        state: AssociationState::Active,
    };
    let disposition = ProcessingDisposition {
        id: support::id(602),
        observation: obs.id,
        decision: ProcessingDecision::Associated,
        association: Some(association.id),
    };
    let reads = [
        Observation::Accepted(q),
        Observation::Accepted(row),
        absent_service(&ServicePayload::Association(association.clone())),
        absent_service(&ServicePayload::Disposition(disposition.clone())),
    ];
    let bindings = [ExternalBindingObservation {
        account: support::id(1),
        external: key,
        binding: None,
    }];
    let op = ServiceOperation::CompleteObservation {
        observation: obs.id,
        disposition: disposition.clone(),
        association: Some(association),
        qso_change: None,
        selected: None,
    };
    let ServiceDecision::Ready(commit) = run_operation(&op, &reads, &bindings)? else {
        panic!("not ready")
    };
    assert_eq!(commit.accepted.len(), 2);
    let completed = commit
        .accepted
        .iter()
        .filter_map(|r| {
            if let RecordState::Live(RecordValue::Service(s)) = &r.state {
                Some(s)
            } else {
                None
            }
        })
        .find(|s| matches!(s.payload, ServicePayload::Disposition(_)))
        .unwrap();
    assert!(
        completed
            .support
            .required
            .iter()
            .any(|r| r.reference().key.id == RecordId::Qso(qso))
    );
    let mut changed = op;
    if let ServiceOperation::CompleteObservation { observation, .. } = &mut changed {
        *observation = support::id(999);
    }
    assert!(run_operation(&changed, &reads, &bindings).is_err());
    Ok(())
}

#[test]
fn contact_deletion_is_coupled_to_all_known_bindings_and_suppression() -> anyhow::Result<()> {
    use clacks_model::service::{
        destination::{Destination, RemoteCollection},
        evidence::{ServicePayload, processing::DeletionSuppression},
    };
    use clacks_sync::{
        acceptance::ServerChange,
        client::VersionCondition,
        record::{RecordState, RecordValue},
        service::{
            BindingVersion, ServiceRecord, ServiceSupport,
            operation::{NativeChange, ServiceOperation},
        },
    };
    let q = support::accepted(200, "fixture");
    let RecordId::Qso(qso) = q.key.id else {
        panic!()
    };
    let external = ExternalRecordKey {
        destination: support::id(42),
        record: RemoteRecordId::Qrz(7),
    };
    let mut association = ExternalAssociation {
        id: support::id(801),
        external: external.clone(),
        qso,
        state: AssociationState::Active,
    };
    let mut a = q.clone();
    a.key.id = RecordId::Service(ServicePayload::Association(association.clone()).id());
    a.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
        payload: ServicePayload::Association(association.clone()),
        support: ServiceSupport {
            destination: Some(Destination {
                id: support::id(42),
                collection: RemoteCollection::Qrz { book_id: Some(7) },
                description: "book".into(),
            }),
            ..Default::default()
        },
    })));
    let binding = ExternalBindingObservation {
        account: q.key.account,
        external: external.clone(),
        binding: Some(BindingVersion {
            association: association.id,
            qso,
            stamp: a.stamp.clone(),
            state: AssociationState::Active,
        }),
    };
    association.state = AssociationState::DeletedQso;
    let suppression = DeletionSuppression {
        id: support::id(802),
        qso,
        external,
    };
    let current = Observation::Accepted(q);
    let op = ServiceOperation::DeleteQso {
        qso_change: NativeChange::Server(ServerChange {
            key: current.key(),
            state: RecordState::Deleted,
            expected: VersionCondition::from_observation(&current)?,
            dependencies: vec![],
        }),
        associations: vec![association],
        suppression: vec![suppression.clone()],
    };
    let reads = [
        current,
        Observation::Accepted(a),
        absent_service(&ServicePayload::Suppression(suppression)),
    ];
    let ServiceDecision::Ready(commit) =
        run_operation(&op, &reads, std::slice::from_ref(&binding))?
    else {
        panic!()
    };
    assert_eq!(commit.accepted.len(), 3);
    assert_eq!(commit.external_guards.len(), 2);
    let mut incomplete = op;
    if let ServiceOperation::DeleteQso { suppression, .. } = &mut incomplete {
        suppression.clear();
    }
    assert!(run_operation(&incomplete, &reads, &[binding]).is_err());
    Ok(())
}

#[test]
fn returned_external_identity_cannot_bind_an_unsubmitted_contact() -> anyhow::Result<()> {
    use clacks_model::{
        scalar::UtcTimestamp,
        service::{
            destination::{Destination, RemoteCollection},
            evidence::{ServicePayload, submission::*},
            qrz::QrzSubmission,
        },
    };
    use clacks_sync::{
        record::{RecordState, RecordValue},
        service::{ServiceRecord, ServiceSupport, operation::ServiceOperation},
    };
    let destination = Destination {
        id: support::id(42),
        collection: RemoteCollection::Qrz { book_id: Some(7) },
        description: "book".into(),
    };
    let attempt = SubmissionAttempt {
        id: support::id(901),
        qso: support::id(10),
        projection: SubmissionProjection::Qrz(QrzSubmission {
            destination: destination.id,
            fields: Default::default(),
        }),
        phase: DispatchPhase::Dispatched,
        at: UtcTimestamp::new(0, 0)?,
        batch: None,
    };
    let external = ExternalRecordKey {
        destination: destination.id,
        record: RemoteRecordId::Qrz(7),
    };
    let result = SubmissionResult {
        external_attempt: None,
        id: support::id(902),
        destination: destination.id,
        scope: ResultScope::IndividualAttempt(attempt.id),
        outcome: SubmissionOutcome::Success,
        phase: DispatchPhase::Dispatched,
        at: UtcTimestamp::new(1, 0)?,
        diagnostic: None,
        external: Some(external.clone()),
    };
    let association = ExternalAssociation {
        id: support::id(903),
        external: external.clone(),
        qso: support::id(99),
        state: AssociationState::Active,
    };
    let mut target = support::accepted(200, "fixture");
    target.key.id = RecordId::Qso(association.qso);
    if let RecordState::Live(RecordValue::Qso(q)) = &mut target.state {
        q.id = association.qso;
    }
    let mut a = support::accepted(199, "fixture");
    a.key.id = RecordId::Service(ServicePayload::Attempt(attempt.clone()).id());
    a.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
        payload: ServicePayload::Attempt(attempt.clone()),
        support: ServiceSupport {
            destination: Some(destination),
            ..Default::default()
        },
    })));
    let reads = [
        Observation::Accepted(a),
        Observation::Accepted(target),
        absent_service(&ServicePayload::Result(result.clone())),
        absent_service(&ServicePayload::Association(association.clone())),
    ];
    let op = ServiceOperation::RecordSubmissionResult {
        result,
        batch_members: vec![attempt.id],
        association: Some(association),
    };
    assert!(
        run_operation(
            &op,
            &reads,
            &[ExternalBindingObservation {
                account: support::id(1),
                external,
                binding: None
            }]
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn client_import_can_reference_a_tag_created_in_the_same_operation() -> anyhow::Result<()> {
    use clacks_model::tag::LogbookTag;
    use clacks_sync::{
        acceptance::ServerChange,
        account::{EvidenceView, InstalledCheckpoint},
        client::{PreparedChange, VersionCondition},
        clock::{ClockActor, RecordVector},
        membership::{ClientMembership, MembershipSnapshot},
        record::{RecordKey, RecordState, RecordValue},
        service::operation::*,
    };
    let (a, b) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let bs = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &bs,
    };
    let actor = support::id(4);
    let m = MembershipSnapshot {
        account: a.account,
        generation: 0,
        clients: vec![ClientMembership {
            actor,
            created_at: a.head,
            closed_at: None,
            acknowledged: None,
        }],
    };
    let tag = LogbookTag::new(support::id(8), "Imported".into());
    let mut q = support::qso(support::id(100));
    q.tags.insert(tag.id);
    let absent = |id| Observation::Absent {
        key: RecordKey {
            account: a.account,
            id,
        },
        checkpoint: InstalledCheckpoint {
            account: a.account,
            baseline: b.id,
            coverage: a.installed.clone(),
        },
    };
    let qr = absent(RecordId::Qso(q.id));
    let tr = absent(RecordId::Tag(tag.id));
    let qc = PreparedChange {
        key: qr.key(),
        state: RecordState::Live(RecordValue::Qso(Box::new(q))),
        vector: RecordVector::default()
            .increment(ClockActor::Client(actor), 0)?
            .0,
        actor,
        expected: VersionCondition::from_observation(&qr)?,
        dependencies: vec![VersionCondition::from_observation(&tr)?],
    };
    let tc = ServerChange {
        key: tr.key(),
        state: RecordState::Live(RecordValue::Tag(Box::new(tag))),
        expected: VersionCondition::from_observation(&tr)?,
        dependencies: vec![],
    };
    let op = ServiceOperation::ApplyImport {
        qso_change: NativeChange::Client(qc),
        assertions: vec![],
        selected_tag_effects: vec![NativeChange::Server(tc)],
        association: None,
    };
    let original_tag_guard = VersionCondition::from_observation(&tr)?;
    let qso_key = qr.key();
    let result = plan_operation(&ServiceOperationInput {
        id: support::id(500),
        account: &a,
        membership: &m,
        reads: &[qr, tr],
        bindings: &[],
        destinations: &[],
        evidence: &ev,
        operation: &op,
        matching: None,
    })?;
    let ServiceDecision::Ready(commit) = result else {
        panic!("same-operation dependency should be fulfilled internally, got {result:?}");
    };
    assert!(commit.conditions.contains(&original_tag_guard));
    let qso = commit
        .accepted
        .iter()
        .find(|row| row.key == qso_key)
        .unwrap();
    assert_eq!(qso.stamp.vector.component(ClockActor::Client(actor)), 1);
    Ok(())
}

#[test]
fn qrz_destination_can_establish_its_previously_unknown_book_id() -> anyhow::Result<()> {
    use clacks_model::service::{
        destination::{Destination, RemoteCollection},
        evidence::ServicePayload,
    };
    use clacks_sync::{
        acceptance::{AcceptanceDecision, AcceptanceInput, ServerChange, plan_server_acceptance},
        account::EvidenceView,
        client::VersionCondition,
        membership::MembershipSnapshot,
        record::{RecordState, RecordValue},
        service::{DestinationRegistryObservation, ServiceRecord, ServiceSupport},
    };
    let (mut account, b) =
        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    account.head.revision = 200;
    let old = Destination {
        id: support::id(42),
        collection: RemoteCollection::Qrz { book_id: None },
        description: "book".into(),
    };
    let new = Destination {
        collection: RemoteCollection::Qrz { book_id: Some(7) },
        ..old.clone()
    };
    let mut row = support::accepted(200, "fixture");
    row.key.id = RecordId::Service(ServicePayload::Destination(old.clone()).id());
    row.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
        payload: ServicePayload::Destination(old.clone()),
        support: ServiceSupport::default(),
    })));
    let current = Observation::Accepted(row);
    let bases = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &bases,
    };
    let m = MembershipSnapshot {
        account: account.account,
        generation: 0,
        clients: vec![],
    };
    let registry = DestinationRegistryObservation {
        account: account.account,
        head: account.head,
        destinations: vec![old],
    };
    let change = ServerChange {
        key: current.key(),
        state: RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
            payload: ServicePayload::Destination(new.clone()),
            support: ServiceSupport::default(),
        }))),
        expected: VersionCondition::from_observation(&current)?,
        dependencies: vec![],
    };
    let result = plan_server_acceptance(
        &change,
        &AcceptanceInput {
            destination_registry: Some(&registry),
            account: &account,
            membership: &m,
            current: &current,
            references: &[],
            bindings: &[],
        },
        &ev,
    );
    let AcceptanceDecision::Commit(commit) = result? else {
        panic!("expected identity refinement");
    };
    let account = commit.next_account;
    let current = Observation::Accepted(commit.accepted);
    let registry = DestinationRegistryObservation {
        account: account.account,
        head: account.head,
        destinations: vec![new.clone()],
    };
    for book_id in [None, Some(8)] {
        let changed = Destination {
            collection: RemoteCollection::Qrz { book_id },
            ..new.clone()
        };
        let change = ServerChange {
            key: current.key(),
            state: RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
                payload: ServicePayload::Destination(changed),
                support: ServiceSupport::default(),
            }))),
            expected: VersionCondition::from_observation(&current)?,
            dependencies: vec![],
        };
        assert!(
            plan_server_acceptance(
                &change,
                &AcceptanceInput {
                    destination_registry: Some(&registry),
                    account: &account,
                    membership: &m,
                    current: &current,
                    references: &[],
                    bindings: &[]
                },
                &ev
            )
            .is_err()
        );
    }
    let observation = remote(&support::qso(support::id(10)));
    let payload = ServicePayload::Observation(Box::new(observation));
    let current = absent_service(&payload);
    let change = ServerChange {
        key: current.key(),
        state: RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
            payload,
            support: ServiceSupport {
                destination: registry.destinations.first().map(|d| Destination {
                    collection: RemoteCollection::Qrz { book_id: None },
                    ..d.clone()
                }),
                ..Default::default()
            },
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
                membership: &m,
                current: &current,
                references: &[],
                bindings: &[]
            },
            &ev
        )?,
        AcceptanceDecision::Commit(_)
    ));
    Ok(())
}

#[test]
fn adding_downloaded_contact_requires_a_selected_tag() -> anyhow::Result<()> {
    use clacks_model::service::{
        destination::{Destination, RemoteCollection},
        evidence::{
            ServicePayload,
            processing::{ProcessingDecision, ProcessingDisposition},
        },
    };
    use clacks_sync::{
        acceptance::ServerChange,
        account::InstalledCheckpoint,
        client::VersionCondition,
        record::{RecordKey, RecordState, RecordValue},
        service::{
            ServiceRecord, ServiceSupport,
            operation::{NativeChange, ServiceOperation},
        },
    };
    let q = support::qso(support::id(10));
    assert!(q.tags.is_empty());
    let obs = remote(&q);
    let mut row = support::accepted(200, "fixture");
    row.key.id = RecordId::Service(ServicePayload::Observation(Box::new(obs.clone())).id());
    row.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
        payload: ServicePayload::Observation(Box::new(obs.clone())),
        support: ServiceSupport {
            destination: Some(Destination {
                id: support::id(42),
                collection: RemoteCollection::Qrz { book_id: Some(7) },
                description: "book".into(),
            }),
            ..Default::default()
        },
    })));
    let association = ExternalAssociation {
        id: support::id(601),
        external: ExternalRecordKey {
            destination: support::id(42),
            record: RemoteRecordId::Qrz(7),
        },
        qso: q.id,
        state: AssociationState::Active,
    };
    let disposition = ProcessingDisposition {
        id: support::id(602),
        observation: obs.id,
        decision: ProcessingDecision::Added,
        association: Some(association.id),
    };
    let (a, b) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let absent = Observation::Absent {
        key: RecordKey {
            account: a.account,
            id: RecordId::Qso(q.id),
        },
        checkpoint: InstalledCheckpoint {
            account: a.account,
            baseline: b.id,
            coverage: a.installed.clone(),
        },
    };
    let change = ServerChange {
        key: absent.key(),
        state: RecordState::Live(RecordValue::Qso(Box::new(q))),
        expected: VersionCondition::from_observation(&absent)?,
        dependencies: vec![],
    };
    let reads = [
        absent,
        Observation::Accepted(row),
        absent_service(&ServicePayload::Association(association.clone())),
        absent_service(&ServicePayload::Disposition(disposition.clone())),
    ];
    let bindings = [ExternalBindingObservation {
        account: a.account,
        external: association.external.clone(),
        binding: None,
    }];
    let op = ServiceOperation::CompleteObservation {
        observation: obs.id,
        disposition,
        association: Some(association),
        qso_change: Some(NativeChange::Server(change)),
        selected: None,
    };
    assert!(run_operation(&op, &reads, &bindings).is_err());
    let mut tagged = op;
    let tag_id = support::id(88);
    if let ServiceOperation::CompleteObservation {
        qso_change: Some(NativeChange::Server(change)),
        ..
    } = &mut tagged
        && let RecordState::Live(RecordValue::Qso(qso)) = &mut change.state
    {
        qso.tags.insert(tag_id);
    }
    let mut tag = support::accepted(199, "tag");
    tag.key.id = RecordId::Tag(tag_id);
    tag.state = RecordState::Live(RecordValue::Tag(Box::new(
        clacks_model::tag::LogbookTag::new(tag_id, "Downloaded".into()),
    )));
    let mut reads = reads.to_vec();
    reads.push(Observation::Accepted(tag));
    assert!(matches!(
        run_operation(&tagged, &reads, &bindings)?,
        ServiceDecision::Ready(_)
    ));
    Ok(())
}

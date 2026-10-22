mod support;
#[test]
fn recovered_same_id_mutation_is_checked_before_covered_shortcut() -> anyhow::Result<()> {
    use clacks_model::service::{
        destination::{Destination, RemoteCollection},
        evidence::{ServicePayload, ServiceRecordId, submission::*},
    };
    use clacks_sync::{
        acceptance::AcceptanceInput,
        account::{AccountMetadata, EvidenceView},
        membership::MembershipSnapshot,
        record::{Observation, RecordId, RecordState, RecordValue},
        recovery::plan_recovered_install,
        service::{ServiceRecord, ServiceSupport},
    };
    let destination = Destination {
        id: support::id(42),
        collection: RemoteCollection::Qrz { book_id: Some(7) },
        description: "book".into(),
    };
    let result = SubmissionResult {
        id: support::id(101),
        destination: destination.id,
        scope: ResultScope::IndividualAttempt(support::id(102)),
        outcome: SubmissionOutcome::Success,
        phase: DispatchPhase::Dispatched,
        at: clacks_model::scalar::UtcTimestamp::new(0, 0)?,
        diagnostic: None,
        external: None,
    };
    let mut incoming = support::accepted(190, "fixture");
    incoming.key.id = RecordId::Service(ServiceRecordId::Result(result.id));
    incoming.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
        payload: ServicePayload::Result(result),
        support: ServiceSupport {
            destination: Some(destination),
            ..Default::default()
        },
    })));
    let mut newer = incoming.clone();
    newer.stamp = support::accepted(200, "fixture").stamp;
    if let RecordState::Live(RecordValue::Service(s)) = &mut newer.state
        && let ServicePayload::Result(r) = &mut s.payload
    {
        r.outcome = SubmissionOutcome::Rejected("different".into());
    }
    let (mut account, b) =
        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    account.head.revision = 200;
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
    let current = Observation::Accepted(newer);
    assert!(
        plan_recovered_install(
            &incoming,
            None,
            support::id(9),
            &AcceptanceInput {
                account: &account,
                membership: &membership,
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
fn restored_server_recovers_original_success_without_covering_a_new_projection()
-> anyhow::Result<()> {
    use clacks_model::{
        scalar::UtcTimestamp,
        service::{
            applicability::{PublicationCoverage, PublicationCoverageInput, publication_coverage},
            destination::{Destination, RemoteCollection, Service},
            evidence::{ImportedBaseline, ServicePayload, submission::*},
            qrz::QrzSubmission,
        },
    };
    use clacks_sync::{
        account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
        lineage::Coverage,
        membership::{ClientMembership, MembershipSnapshot},
        record::{Observation, RecordId, RecordKey, RecordState, RecordValue},
        recovery::plan_recovery_fork,
        service::{
            ServiceRecord, ServiceSupport, SupportRequirement, VersionReference,
            operation::{CoupledRequirement, ServiceDecision},
            recovery::{RecoveredOperationInput, plan_recovered_operation},
        },
    };
    let (mut restored, b) =
        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    restored.head.revision = 160;
    restored.installed = Coverage::from(restored.head);
    let membership = MembershipSnapshot {
        account: restored.account,
        generation: 0,
        clients: vec![ClientMembership {
            actor: support::id(4),
            created_at: restored.head,
            closed_at: None,
            acknowledged: None,
        }],
    };
    let fork = plan_recovery_fork(support::id(5), &restored, &membership, &restored.installed)?;
    let account = fork.next_account;
    let mut lineage = b.lineage.clone();
    lineage.add_boundary(fork.boundary)?;
    let bases = [b.clone()];
    let ev = EvidenceView {
        lineage: &lineage,
        baselines: &bases,
    };
    let checkpoint = InstalledCheckpoint {
        account: account.account,
        baseline: b.id,
        coverage: account.installed.clone(),
    };
    let destination = Destination {
        id: support::id(42),
        collection: RemoteCollection::Qrz { book_id: Some(7) },
        description: "book".into(),
    };
    let old = SubmissionProjection::Qrz(QrzSubmission {
        destination: destination.id,
        fields: [("COMMENT".into(), "P1".into())].into_iter().collect(),
    });
    let attempt = SubmissionAttempt {
        id: support::id(101),
        qso: support::id(10),
        projection: old,
        phase: DispatchPhase::Dispatched,
        at: UtcTimestamp::new(0, 0)?,
        batch: None,
    };
    let result = SubmissionResult {
        id: support::id(102),
        destination: destination.id,
        scope: ResultScope::IndividualAttempt(attempt.id),
        outcome: SubmissionOutcome::Success,
        phase: DispatchPhase::Dispatched,
        at: UtcTimestamp::new(1, 0)?,
        diagnostic: None,
        external: None,
    };
    let carrier = |payload: ServicePayload, rev| {
        let mut row = support::accepted(rev, "fixture");
        row.key.id = RecordId::Service(payload.id());
        row.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
            payload,
            support: ServiceSupport {
                destination: Some(destination.clone()),
                ..Default::default()
            },
        })));
        row
    };
    let a = carrier(ServicePayload::Attempt(attempt.clone()), 180);
    let mut r = carrier(ServicePayload::Result(result.clone()), 190);
    if let RecordState::Live(RecordValue::Service(s)) = &mut r.state {
        s.support
            .required
            .push(SupportRequirement::ExactFact(VersionReference::from(&a)));
    }
    let incoming = [r.clone(), a.clone()];
    let unit = CoupledRequirement {
        id: support::id(103),
        members: incoming.iter().map(VersionReference::from).collect(),
    };
    let reads: Vec<_> = incoming
        .iter()
        .map(|row| Observation::Absent {
            key: RecordKey {
                account: account.account,
                id: row.key.id,
            },
            checkpoint: checkpoint.clone(),
        })
        .collect();
    let input = RecoveredOperationInput {
        unit: &unit,
        incoming: &incoming,
        returning_actor: support::id(4),
        account: &account,
        membership: &membership,
        reads: &reads,
        bindings: &[],
        originals: &[],
        evidence: &ev,
    };
    let ServiceDecision::Ready(commit) = plan_recovered_operation(&input)? else {
        panic!()
    };
    assert_eq!(commit.accepted.len(), 2);
    assert!(commit.accepted.contains(&r));
    assert!(commit.accepted.contains(&a));
    assert!(!commit.next_account.installed.covers(r.stamp.accepted_at));
    assert!(commit.job_updates.is_empty());
    let reads: Vec<_> = commit
        .accepted
        .iter()
        .cloned()
        .map(Observation::Accepted)
        .collect();
    let retry = RecoveredOperationInput {
        reads: &reads,
        account: &commit.next_account,
        ..input
    };
    assert!(matches!(
        plan_recovered_operation(&retry)?,
        ServiceDecision::Unchanged
    ));
    let changed = SubmissionProjection::Qrz(QrzSubmission {
        destination: destination.id,
        fields: [("COMMENT".into(), "P2".into())].into_iter().collect(),
    });
    let baseline = ImportedBaseline {
        service: Service::Qrz,
        fields: Default::default(),
        own_location: None,
        confirmation: Default::default(),
        coverage: Default::default(),
    };
    assert_eq!(
        publication_coverage(&PublicationCoverageInput {
            qso: attempt.qso,
            current: &changed,
            source_current: &baseline,
            assertions: &[],
            attempts: &[attempt],
            results: &[result]
        })?,
        PublicationCoverage::Pending
    );
    let mut excluded = membership.clone();
    excluded.clients.clear();
    assert!(matches!(
        plan_recovered_operation(&RecoveredOperationInput {
            membership: &excluded,
            ..retry
        })?,
        ServiceDecision::Review(_)
    ));
    Ok(())
}

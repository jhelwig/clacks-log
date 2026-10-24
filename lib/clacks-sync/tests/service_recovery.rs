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
        collection: RemoteCollection::Qrz {
            book_id: Some("7".parse().unwrap()),
        },
        description: "book".into(),
    };
    let result = SubmissionResult {
        external_attempt: None,
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
                destination_registry: None,
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
    let captured = support::telemetry::capture(|| {
        use clacks_model::{
            scalar::UtcTimestamp,
            service::{
                applicability::{
                    PublicationCoverage, PublicationCoverageInput, publication_coverage,
                },
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
            collection: RemoteCollection::Qrz {
                book_id: Some("7".parse().unwrap()),
            },
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
            external_attempt: None,
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
        let registry = clacks_sync::service::DestinationRegistryObservation {
            account: account.account,
            head: account.head,
            destinations: vec![destination.clone()],
        };
        let input = RecoveredOperationInput {
            destination_registry: &registry,
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
        let mut cyclic_attempt = a.clone();
        if let RecordState::Live(RecordValue::Service(service)) = &mut cyclic_attempt.state {
            service
                .support
                .required
                .push(SupportRequirement::ExactFact(VersionReference::from(&r)));
        }
        let mut older_attempt = a.clone();
        older_attempt.stamp = support::accepted(170, "fixture").stamp;
        for (reason, incoming, reads) in [
            ("missing_target_read", incoming.to_vec(), vec![]),
            ("missing_unit_member", vec![], reads.clone()),
            (
                "unit_member_not_covered",
                vec![],
                vec![
                    Observation::Accepted(r.clone()),
                    Observation::Accepted(older_attempt),
                ],
            ),
            ("deferred_service", vec![r.clone()], reads.clone()),
            (
                "dependency_cycle",
                vec![r.clone(), cyclic_attempt],
                reads.clone(),
            ),
        ] {
            let held = support::telemetry::capture(|| {
                plan_recovered_operation(&RecoveredOperationInput {
                    incoming: &incoming,
                    reads: &reads,
                    ..input
                })
            });
            assert!(
                matches!(held.result, Ok(ServiceDecision::Incomplete(_))),
                "{reason}: {:?}",
                held.result
            );
            let event = held.event("service.recovery.incomplete");
            assert_eq!(event.fields.get("reason").map(String::as_str), Some(reason));
            assert_eq!(event.parent, held.span("sync.service.recovery.plan").id);
        }
        let ready = support::telemetry::capture_filtered(
            || plan_recovered_operation(&input),
            |metadata| *metadata.level() <= tracing::Level::INFO,
        );
        let event = ready.event("service.recovery.planned");
        let span = ready.span("sync.service.recovery.plan");
        assert_eq!(event.fields.get("level").map(String::as_str), Some("INFO"));
        assert_eq!(span.fields.get("level").map(String::as_str), Some("INFO"));
        assert_eq!(
            span.fields.get("account_id"),
            Some(&account.account.to_string())
        );
        assert_eq!(span.fields.get("operation_id"), Some(&unit.id.to_string()));
        assert_eq!(
            span.fields.get("actor_id"),
            Some(&input.returning_actor.to_string())
        );
        assert_eq!(event.parent, span.id);
        ready.event_count("service.recovery.planned", "proposed_record", 2.);
        let ServiceDecision::Ready(commit) = ready.result? else {
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
        let retry_registry = clacks_sync::service::DestinationRegistryObservation {
            head: commit.next_account.head,
            ..registry.clone()
        };
        let retry = RecoveredOperationInput {
            destination_registry: &retry_registry,
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
            native_end: None,
            service: Service::Qrz,
            fields: Default::default(),
            own_location: None,
            confirmation: Default::default(),
            coverage: Default::default(),
        };
        assert_eq!(
            publication_coverage(&PublicationCoverageInput {
                qso: attempt.qso,
                current: Some(&changed),
                service: changed.service(),
                destination: Some(changed.destination()),
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
    });
    let events: Vec<_> = captured
        .observations
        .events
        .iter()
        .filter_map(|event| event.fields.get("event"))
        .filter(|name| name.starts_with("service.recovery."))
        .map(String::as_str)
        .collect();
    assert_eq!(
        events,
        vec![
            "service.recovery.covered",
            "service.recovery.review_required"
        ]
    );
    assert!(
        captured
            .metrics
            .iter()
            .all(|m| !m.name.contains("upload") && !m.name.contains("dispatch"))
    );
    captured.result
}

#[test]
fn covered_active_association_is_not_reinstalled_after_contact_deletion() -> anyhow::Result<()> {
    use clacks_model::service::{
        destination::{Destination, ExternalRecordKey, RemoteCollection, RemoteRecordId},
        evidence::{
            ServicePayload,
            processing::{AssociationState, ExternalAssociation},
        },
    };
    use clacks_sync::{
        acceptance::AcceptanceInput,
        account::{AccountMetadata, EvidenceView},
        membership::{ClientMembership, MembershipSnapshot},
        record::{Observation, RecordId, RecordState, RecordValue},
        recovery::{RecoveredDecision, plan_recovered_install},
        service::{
            BindingVersion, DestinationRegistryObservation, ExternalBindingObservation,
            ServiceRecord, ServiceSupport,
        },
    };
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
        clients: vec![ClientMembership {
            actor: support::id(4),
            created_at: account.head,
            closed_at: None,
            acknowledged: None,
        }],
    };
    let destination = Destination {
        id: support::id(42),
        collection: RemoteCollection::Qrz {
            book_id: Some("7".parse().unwrap()),
        },
        description: "book".into(),
    };
    let a = ExternalAssociation {
        id: support::id(101),
        external: ExternalRecordKey {
            destination: destination.id,
            record: RemoteRecordId::Qrz("7".parse().unwrap()),
        },
        qso: support::id(10),
        state: AssociationState::Active,
    };
    let mut old = support::accepted(190, "fixture");
    old.key.id = RecordId::Service(ServicePayload::Association(a.clone()).id());
    old.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
        payload: ServicePayload::Association(a.clone()),
        support: ServiceSupport {
            destination: Some(destination.clone()),
            ..Default::default()
        },
    })));
    let mut new = old.clone();
    new.stamp = support::accepted(200, "fixture").stamp;
    if let RecordState::Live(RecordValue::Service(s)) = &mut new.state
        && let ServicePayload::Association(a) = &mut s.payload
    {
        a.state = AssociationState::DeletedQso;
    }
    let mut deleted = support::accepted(200, "fixture");
    deleted.state = RecordState::Deleted;
    let bindings = [ExternalBindingObservation {
        account: account.account,
        external: a.external.clone(),
        binding: Some(BindingVersion {
            association: a.id,
            qso: a.qso,
            state: AssociationState::DeletedQso,
            stamp: new.stamp.clone(),
        }),
    }];
    let registry = DestinationRegistryObservation {
        account: account.account,
        head: account.head,
        destinations: vec![destination],
    };
    let current = Observation::Accepted(new);
    let refs = [Observation::Accepted(deleted)];
    assert!(matches!(
        plan_recovered_install(
            &old,
            None,
            support::id(4),
            &AcceptanceInput {
                destination_registry: Some(&registry),
                account: &account,
                membership: &membership,
                current: &current,
                references: &refs,
                bindings: &bindings
            },
            &ev
        )?,
        RecoveredDecision::Covered
    ));
    Ok(())
}

#[test]
fn recovery_uses_refined_qrz_namespace_for_later_members() -> anyhow::Result<()> {
    use clacks_model::service::{
        destination::{Destination, RemoteCollection},
        evidence::ServicePayload,
    };
    use clacks_sync::{
        account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
        lineage::Coverage,
        membership::{ClientMembership, MembershipSnapshot},
        record::{Observation, RecordId, RecordState, RecordValue},
        recovery::plan_recovery_fork,
        service::{
            DestinationRegistryObservation, ServiceRecord, ServiceSupport, VersionReference,
            operation::{CoupledRequirement, ServiceDecision},
            recovery::{RecoveredOperationInput, plan_recovered_operation},
        },
    };
    let (mut restored, baseline) =
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
    let mut lineage = baseline.lineage.clone();
    lineage.add_boundary(fork.boundary)?;
    let baselines = [baseline.clone()];
    let evidence = EvidenceView {
        lineage: &lineage,
        baselines: &baselines,
    };
    let unknown = Destination {
        id: support::id(42),
        collection: RemoteCollection::Qrz { book_id: None },
        description: "first".into(),
    };
    let first = Destination {
        collection: RemoteCollection::Qrz {
            book_id: Some("7".parse().unwrap()),
        },
        ..unknown.clone()
    };
    let second = Destination {
        id: support::id(43),
        collection: RemoteCollection::Qrz {
            book_id: Some("8".parse().unwrap()),
        },
        description: "second".into(),
    };
    let carrier = |destination: Destination, revision| {
        let mut row = support::accepted(revision, "destination");
        row.key.id = RecordId::Service(ServicePayload::Destination(destination.clone()).id());
        row.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
            payload: ServicePayload::Destination(destination),
            support: ServiceSupport::default(),
        })));
        row
    };
    let original = carrier(unknown.clone(), 150);
    let incoming = [carrier(first, 180), carrier(second, 190)];
    let unit = CoupledRequirement {
        id: support::id(99),
        members: incoming.iter().map(VersionReference::from).collect(),
    };
    let reads = [
        Observation::Accepted(original),
        Observation::Absent {
            key: incoming.last().unwrap().key,
            checkpoint: InstalledCheckpoint {
                account: account.account,
                baseline: baseline.id,
                coverage: account.installed.clone(),
            },
        },
    ];
    let registry = DestinationRegistryObservation {
        account: account.account,
        head: account.head,
        destinations: vec![unknown],
    };
    let mut incoming = incoming;
    for _ in 0..2 {
        let result = plan_recovered_operation(&RecoveredOperationInput {
            destination_registry: &registry,
            unit: &unit,
            incoming: &incoming,
            returning_actor: support::id(4),
            account: &account,
            membership: &membership,
            reads: &reads,
            bindings: &[],
            originals: &[],
            evidence: &evidence,
        })?;
        assert!(
            matches!(result, ServiceDecision::Ready(_)),
            "refined namespace must be visible to remaining recovered records: {result:?}"
        );
        incoming.reverse();
    }
    let qso = support::accepted(175, "recovered contact");
    let external = clacks_model::service::destination::ExternalRecordKey {
        destination: support::id(42),
        record: clacks_model::service::destination::RemoteRecordId::Qrz("7".parse().unwrap()),
    };
    let association = clacks_model::service::evidence::processing::ExternalAssociation {
        id: support::id(101),
        external: external.clone(),
        qso: support::id(10),
        state: clacks_model::service::evidence::processing::AssociationState::Active,
    };
    let mut binding = support::accepted(180, "binding");
    binding.key.id = RecordId::Service(ServicePayload::Association(association.clone()).id());
    binding.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
        payload: ServicePayload::Association(association),
        support: ServiceSupport {
            destination: Some(Destination {
                id: support::id(42),
                collection: RemoteCollection::Qrz {
                    book_id: Some("7".parse().unwrap()),
                },
                description: "first".into(),
            }),
            required: vec![clacks_sync::service::SupportRequirement::CoveredEffect(
                VersionReference::from(&qso),
            )],
            ..Default::default()
        },
    })));
    let incoming = [incoming.last().unwrap().clone(), qso, binding];
    let unit = CoupledRequirement {
        id: support::id(102),
        members: incoming.iter().map(VersionReference::from).collect(),
    };
    let reads: Vec<_> = incoming
        .iter()
        .map(|row| Observation::Absent {
            key: row.key,
            checkpoint: InstalledCheckpoint {
                account: account.account,
                baseline: baseline.id,
                coverage: account.installed.clone(),
            },
        })
        .collect();
    let bindings = [clacks_sync::service::ExternalBindingObservation {
        account: account.account,
        external,
        binding: None,
    }];
    let result = plan_recovered_operation(&RecoveredOperationInput {
        destination_registry: &registry,
        unit: &unit,
        incoming: &incoming,
        returning_actor: support::id(4),
        account: &account,
        membership: &membership,
        reads: &reads,
        bindings: &bindings,
        originals: &[],
        evidence: &evidence,
    })?;
    assert!(
        matches!(result, ServiceDecision::Ready(_)),
        "inline refinement prerequisites must precede unrelated namespaces: {result:?}"
    );
    Ok(())
}

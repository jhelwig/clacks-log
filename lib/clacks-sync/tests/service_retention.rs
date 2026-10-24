mod support;
#[test]
fn equal_attempts_remain_distinct_and_closed_poll_staging_can_be_collected() -> anyhow::Result<()> {
    let captured = support::telemetry::capture(|| {
        use clacks_model::{
            scalar::UtcTimestamp,
            service::{
                destination::{Destination, RemoteCollection},
                evidence::{
                    ServicePayload,
                    download::{AcquisitionPhase, AcquisitionState},
                    observation::{QueryIdentity, QueryStream},
                    submission::*,
                },
                qrz::QrzSubmission,
            },
        };
        use clacks_sync::{
            record::{RecordId, RecordState, RecordValue},
            service::{ServiceRecord, ServiceSupport, retention::requirements},
        };
        let destination = Destination {
            id: support::id(42),
            collection: RemoteCollection::Qrz {
                book_id: Some("7".parse().unwrap()),
            },
            description: "book".into(),
        };
        let attempt = SubmissionAttempt {
            id: support::id(101),
            qso: support::id(10),
            projection: SubmissionProjection::Qrz(QrzSubmission {
                destination: destination.id,
                fields: Default::default(),
            }),
            phase: DispatchPhase::Intent,
            at: UtcTimestamp::new(0, 0)?,
            batch: None,
        };
        let mut second = attempt.clone();
        second.id = support::id(102);
        let carrier = |payload: ServicePayload| {
            let mut row = support::accepted(200, "fixture");
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
        let a = carrier(ServicePayload::Attempt(attempt));
        let b = carrier(ServicePayload::Attempt(second));
        let poll = carrier(ServicePayload::Acquisition(AcquisitionState {
            id: support::id(103),
            query: QueryIdentity {
                destination: destination.id,
                stream: QueryStream::Qrz,
                selected_fields: Default::default(),
                own_call: None,
                since: None,
            },
            phase: AcquisitionPhase::Completed,
            started_at: UtcTimestamp::new(0, 0)?,
            finished_at: Some(UtcTimestamp::new(1, 0)?),
            complete: true,
            retained_inputs: Default::default(),
        }));
        let r = requirements(&[a.clone(), b.clone(), poll.clone()], &[])?;
        assert!(r.semantic_keys.contains(&a.key));
        assert!(r.semantic_keys.contains(&b.key));
        assert!(r.removable_staging.contains(&poll.key));
        assert!(!r.removable_staging.contains(&a.key));
        Ok(())
    });
    captured.event_count("service.retention.calculated", "required_record", 2.);
    captured.event_count("service.retention.calculated", "removable_staging", 1.);
    captured.result
}

#[test]
fn compaction_projects_embedded_contexts_without_changing_evidence_identity() -> anyhow::Result<()>
{
    use clacks_model::service::{
        destination::{Destination, RemoteCollection},
        evidence::{
            ServicePayload,
            download::{AcquisitionPhase, AcquisitionState},
            observation::{QueryIdentity, QueryStream},
        },
    };
    use clacks_sync::{
        account::{AccountMetadata, EvidenceView},
        clock::{ClockActor, RecordVector},
        compaction::{CompactionInput, plan_baseline},
        lineage::Coverage,
        membership::{ClientMembership, MembershipSnapshot},
        record::{RecordId, RecordState, RecordValue},
        service::{ServiceRecord, ServiceSupport},
    };
    let (mut account, b) =
        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    account.head.revision = 200;
    account.installed = Coverage::from(account.head);
    let actor = support::id(4);
    let membership = MembershipSnapshot {
        account: account.account,
        generation: 0,
        clients: vec![ClientMembership {
            actor,
            created_at: support::accepted(1, "fixture").stamp.accepted_at,
            closed_at: Some(support::accepted(150, "fixture").stamp.accepted_at),
            acknowledged: None,
        }],
    };
    let bases = [b.clone()];
    let evidence = EvidenceView {
        lineage: &b.lineage,
        baselines: &bases,
    };
    let mut stamp = support::accepted(100, "fixture").stamp;
    stamp.vector = stamp.vector.join(&RecordVector::from_entries([(
        ClockActor::Client(actor),
        1,
    )])?);
    let destination = Destination {
        id: support::id(42),
        collection: RemoteCollection::Qrz {
            book_id: Some("7".parse().unwrap()),
        },
        description: "book".into(),
    };
    let payload = ServicePayload::Acquisition(AcquisitionState {
        id: support::id(101),
        query: QueryIdentity {
            destination: destination.id,
            stream: QueryStream::Qrz,
            selected_fields: Default::default(),
            own_call: None,
            since: None,
        },
        phase: AcquisitionPhase::Completed,
        started_at: clacks_model::scalar::UtcTimestamp::new(0, 0)?,
        finished_at: Some(clacks_model::scalar::UtcTimestamp::new(1, 0)?),
        complete: true,
        retained_inputs: Default::default(),
    });
    let mut row = support::accepted(200, "fixture");
    row.key.id = RecordId::Service(payload.id());
    row.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
        payload: payload.clone(),
        support: ServiceSupport {
            acquisition: Some(stamp.clone()),
            destination: Some(destination),
            ..Default::default()
        },
    })));
    let plan = plan_baseline(
        support::id(10),
        &CompactionInput {
            account: &account,
            membership: &membership,
            records: &[row],
            surviving: &[],
        },
        &evidence,
    )?;
    assert!(plan.removable_clients.contains(&actor));
    let projected = plan.projected.first().unwrap();
    let RecordState::Live(RecordValue::Service(s)) = &projected.state else {
        panic!()
    };
    assert_eq!(s.payload, payload);
    let p = s.support.acquisition.as_ref().unwrap();
    assert_eq!(p.accepted_at, stamp.accepted_at);
    assert_eq!(p.baseline, plan.baseline.id);
    assert_eq!(p.vector.component(ClockActor::Client(actor)), 0);
    Ok(())
}

#[test]
fn compacted_service_support_is_equivalent_during_receipt_both_directions() -> anyhow::Result<()> {
    use clacks_model::service::{
        destination::{Destination, RemoteCollection},
        evidence::{
            ServicePayload,
            download::{AcquisitionPhase, AcquisitionState},
            observation::{QueryIdentity, QueryStream},
        },
    };
    use clacks_sync::{
        account::{AccountMetadata, EvidenceView},
        client::{ClientRecord, receive_accepted},
        compare::{RecordRelation, compare_records},
        record::{Observation, RecordId, RecordState, RecordValue},
        service::{ServiceRecord, ServiceSupport},
    };
    let (_, b) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let mut projected_baseline = b.clone();
    projected_baseline.id = support::id(10);
    let bases = [b.clone(), projected_baseline.clone()];
    let evidence = EvidenceView {
        lineage: &b.lineage,
        baselines: &bases,
    };
    let destination = Destination {
        id: support::id(42),
        collection: RemoteCollection::Qrz {
            book_id: Some("7".parse().unwrap()),
        },
        description: "book".into(),
    };
    let payload = ServicePayload::Acquisition(AcquisitionState {
        id: support::id(101),
        query: QueryIdentity {
            destination: destination.id,
            stream: QueryStream::Qrz,
            selected_fields: Default::default(),
            own_call: None,
            since: None,
        },
        phase: AcquisitionPhase::Completed,
        started_at: clacks_model::scalar::UtcTimestamp::new(0, 0)?,
        finished_at: Some(clacks_model::scalar::UtcTimestamp::new(1, 0)?),
        complete: true,
        retained_inputs: Default::default(),
    });
    let mut old = support::accepted(200, "fixture");
    old.key.id = RecordId::Service(payload.id());
    old.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
        payload,
        support: ServiceSupport {
            acquisition: Some(support::accepted(100, "fixture").stamp),
            destination: Some(destination),
            ..Default::default()
        },
    })));
    let mut projected = old.clone();
    projected.stamp.baseline = projected_baseline.id;
    if let RecordState::Live(RecordValue::Service(s)) = &mut projected.state {
        s.support.acquisition.as_mut().unwrap().baseline = projected_baseline.id;
    }
    for (a, b) in [(&old, &projected), (&projected, &old)] {
        assert_eq!(
            compare_records(a, b, &evidence)?,
            RecordRelation::SameVersion
        );
        let current = ClientRecord {
            key: a.key,
            latest: Observation::Accepted(a.clone()),
            retained: vec![],
            pending: None,
        };
        let received = receive_accepted(&current, b, &evidence)?;
        assert!(received.review.is_none());
        assert!(received.record.pending.is_none());
    }
    Ok(())
}

use crate::support;
use clacks_sync::{
    account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
    lineage::Coverage,
    record::{RecordId, RecordState, RecordValue},
    service::{VersionReference, operation::CoupledRequirement},
    transfer::{RecordRequirement, WindowMode, batch::plan_batch},
};

#[test]
fn split_atomic_unit_stages_without_partial_install() -> anyhow::Result<()> {
    let captured = support::telemetry::capture(|| {
        let (_, b) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
        let bases = [b.clone()];
        let ev = EvidenceView {
            lineage: &b.lineage,
            baselines: &bases,
        };
        let q = support::accepted(60, "fixture");
        let mut tag = q.clone();
        support::edit_accepted(&mut tag, |tag| {
            tag.key.id = RecordId::Tag(support::id(11));

            tag.state = RecordState::Live(RecordValue::Tag(Box::new(
                clacks_model::tag::LogbookTag::new(support::id(11), "tag".into()),
            )));
        });
        let source = InstalledCheckpoint {
            account: support::id(1),
            baseline: b.id,
            coverage: Coverage::from(q.stamp.accepted_at()),
        };
        let unit = CoupledRequirement {
            id: support::id(100),
            members: vec![VersionReference::from(&q), VersionReference::from(&tag)],
        };
        let manifest = clacks_sync::transfer::receipt::ReceiptState {
            source: source.clone(),
            mode: WindowMode::Full,
            next_page: 0,
            records: [&q, &tag]
                .into_iter()
                .map(|r| RecordRequirement {
                    key: r.key(),
                    stamp: r.stamp.clone(),
                    deleted: false,
                })
                .collect(),
            boundaries: vec![],
            baselines: bases.to_vec(),
            units: vec![unit],
            staged: vec![],
            completed_units: Default::default(),
        };
        let mut staged = vec![];
        let first = plan_batch(&manifest, &staged, &[], std::slice::from_ref(&q), &[], &ev)?;
        assert!(first.accepted.is_empty());
        assert_eq!(first.staged.len(), 1);
        assert!(first.completed_units.is_empty());
        staged = first.staged;
        let second = plan_batch(&manifest, &staged, &[], &[tag], &[], &ev)?;
        assert_eq!(second.accepted.len(), 2);
        assert!(second.staged.is_empty());
        assert_eq!(second.completed_units.len(), 1);
        Ok(())
    });
    let rows: Vec<_> = captured
        .observations
        .events
        .iter()
        .filter(|s| {
            s.fields
                .get("event")
                .is_some_and(|v| v == "transfer.batch.planned")
        })
        .map(|s| {
            (
                s.fields.get("readiness").unwrap().as_str(),
                s.fields.get("proposed_record_count").unwrap().as_str(),
                s.fields.get("staged_record_count").unwrap().as_str(),
            )
        })
        .collect();
    assert_eq!(rows, vec![("staged", "0", "1"), ("ready", "2", "0")]);

    captured.result
}

#[test]
fn independent_successor_does_not_wait_for_an_older_units_missing_effect() -> anyhow::Result<()> {
    let (_, b) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let bases = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &bases,
    };
    let old = support::accepted(40, "old effect");
    let next = support::accepted(60, "later independent edit");
    let mut tag = next.clone();
    support::edit_accepted(&mut tag, |tag| {
        tag.key.id = RecordId::Tag(support::id(11));

        tag.state = RecordState::Live(RecordValue::Tag(Box::new(
            clacks_model::tag::LogbookTag::new(support::id(11), "tag".into()),
        )));
    });
    let source = InstalledCheckpoint {
        account: support::id(1),
        baseline: b.id,
        coverage: Coverage::from(next.stamp.accepted_at()),
    };
    let unit = CoupledRequirement {
        id: support::id(100),
        members: vec![VersionReference::from(&old), VersionReference::from(&tag)],
    };
    let manifest = clacks_sync::transfer::receipt::ReceiptState {
        source: source.clone(),
        mode: WindowMode::Full,
        next_page: 0,
        records: [&next, &tag]
            .into_iter()
            .map(|r| RecordRequirement {
                key: r.key(),
                stamp: r.stamp.clone(),
                deleted: false,
            })
            .collect(),
        boundaries: vec![],
        baselines: bases.to_vec(),
        units: vec![unit],
        staged: vec![],
        completed_units: Default::default(),
    };
    let staged = vec![];
    let first = plan_batch(&manifest, &staged, &[], &[next], &[], &ev)?;
    assert_eq!(first.accepted.len(), 1);
    assert!(first.completed_units.is_empty());
    Ok(())
}

#[test]
fn completed_disposition_waits_for_observation_binding_and_contact_effects() -> anyhow::Result<()> {
    use clacks_model::{
        scalar::UtcTimestamp,
        service::{
            destination::{Destination, ExternalRecordKey, RemoteCollection, RemoteRecordId},
            evidence::{ServicePayload, observation::*, processing::*},
        },
    };
    use clacks_sync::{
        record::{AcceptedRecord, RecordKey},
        service::{ServiceRecord, ServiceSupport, SupportRequirement},
    };
    let q = support::accepted(60, "fixture");
    let RecordId::Qso(qso) = q.key().id else {
        panic!()
    };
    let external = ExternalRecordKey {
        destination: support::id(42),
        record: RemoteRecordId::Qrz("7".parse().unwrap()),
    };
    let destination = Destination {
        id: external.destination,
        collection: RemoteCollection::Qrz {
            book_id: Some("7".parse().unwrap()),
        },
        description: "book".into(),
    };
    let obs = RemoteObservation {
        id: support::id(501),
        destination: destination.id,
        identity: RemoteIdentity::Complete(external.clone()),
        facts: Default::default(),
        ordering: Default::default(),
        acquisition: AcquisitionProvenance {
            id: support::id(502),
            query: QueryIdentity {
                destination: destination.id,
                stream: QueryStream::Qrz,
                selected_fields: Default::default(),
                own_call: None,
                since: None,
            },
            observed_at: UtcTimestamp::new(0, 0)?,
        },
    };
    let carrier = |payload: ServicePayload| -> AcceptedRecord {
        let mut r = q.clone();

        support::edit_accepted(&mut r, |r| {
            r.key = RecordKey {
                account: q.key().account,
                id: RecordId::Service(payload.id()),
            };
            r.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
                payload,
                support: ServiceSupport {
                    destination: Some(destination.clone()),
                    ..Default::default()
                },
            })));
        });
        r
    };
    let o = carrier(ServicePayload::Observation(Box::new(obs.clone())));
    let a = carrier(ServicePayload::Association(ExternalAssociation {
        id: support::id(503),
        external,
        qso,
        state: AssociationState::Active,
    }));
    let mut d = carrier(ServicePayload::Disposition(ProcessingDisposition {
        id: support::id(504),
        observation: obs.id,
        decision: ProcessingDecision::Associated,
        association: Some(support::id(503)),
    }));
    support::edit_accepted(&mut d, |d| {
        if let RecordState::Live(RecordValue::Service(s)) = &mut d.state {
            s.support.required = vec![
                SupportRequirement::ExactFact(VersionReference::from(&o)),
                SupportRequirement::CoveredEffect(VersionReference::from(&a)),
                SupportRequirement::CoveredEffect(VersionReference::from(&q)),
            ];
        }
    });
    let (_, b) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let bases = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &bases,
    };
    let source = InstalledCheckpoint {
        account: q.key().account,
        baseline: b.id,
        coverage: Coverage::from(q.stamp.accepted_at()),
    };
    let unit = CoupledRequirement {
        id: support::id(505),
        members: [&q, &a, &d]
            .into_iter()
            .map(VersionReference::from)
            .collect(),
    };
    let manifest = clacks_sync::transfer::receipt::ReceiptState {
        source: source.clone(),
        mode: WindowMode::Full,
        next_page: 0,
        records: [&q, &a, &d, &o]
            .into_iter()
            .map(|r| RecordRequirement {
                key: r.key(),
                stamp: r.stamp.clone(),
                deleted: false,
            })
            .collect(),
        boundaries: vec![],
        baselines: bases.to_vec(),
        units: vec![unit],
        staged: vec![],
        completed_units: Default::default(),
    };
    let mut staged = vec![];
    for batch in [vec![d], vec![q, a]] {
        let next = plan_batch(&manifest, &staged, &[], &batch, &[], &ev)?;
        assert!(next.accepted.is_empty());
        assert!(next.completed_units.is_empty());
        staged = next.staged;
    }
    let done = plan_batch(&manifest, &staged, &[], &[o], &[], &ev)?;
    assert_eq!(done.accepted.len(), 4);
    assert!(done.staged.is_empty());
    assert_eq!(done.completed_units.len(), 1);
    Ok(())
}

#[test]
fn old_binding_unit_is_covered_by_accepted_deletion_without_reactivation() -> anyhow::Result<()> {
    use clacks_model::service::{
        destination::{Destination, ExternalRecordKey, RemoteCollection, RemoteRecordId},
        evidence::{
            ServicePayload,
            processing::{AssociationState, ExternalAssociation},
        },
    };
    use clacks_sync::{
        record::Observation,
        service::{ServiceRecord, ServiceSupport},
    };
    let old_q = support::accepted(190, "old");
    let RecordId::Qso(qso) = old_q.key().id else {
        panic!()
    };
    let destination = Destination {
        id: support::id(42),
        collection: RemoteCollection::Qrz {
            book_id: Some("7".parse().unwrap()),
        },
        description: "book".into(),
    };
    let payload = ServicePayload::Association(ExternalAssociation {
        id: support::id(101),
        external: ExternalRecordKey {
            destination: destination.id,
            record: RemoteRecordId::Qrz("7".parse().unwrap()),
        },
        qso,
        state: AssociationState::Active,
    });
    let mut old_a = old_q.clone();
    support::edit_accepted(&mut old_a, |old_a| {
        old_a.key.id = RecordId::Service(payload.id());

        old_a.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
            payload,
            support: ServiceSupport {
                destination: Some(destination),
                ..Default::default()
            },
        })));
    });
    let mut deleted_q = support::accepted(200, "deleted");

    support::edit_accepted(&mut deleted_q, |deleted_q| {
        deleted_q.state = RecordState::Deleted;
    });
    let mut deleted_a = old_a.clone();
    deleted_a.stamp = deleted_q.stamp.clone();
    support::edit_accepted(&mut deleted_a, |deleted_a| {
        if let RecordState::Live(RecordValue::Service(s)) = &mut deleted_a.state
            && let ServicePayload::Association(a) = &mut s.payload
        {
            a.state = AssociationState::DeletedQso;
        }
    });
    let (_, b) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let bases = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &bases,
    };
    let source = InstalledCheckpoint {
        account: old_q.key().account,
        baseline: b.id,
        coverage: Coverage::from(old_q.stamp.accepted_at()),
    };
    let unit = CoupledRequirement {
        id: support::id(102),
        members: [&old_q, &old_a]
            .into_iter()
            .map(VersionReference::from)
            .collect(),
    };
    let manifest = clacks_sync::transfer::receipt::ReceiptState {
        source: source.clone(),
        mode: WindowMode::Full,
        next_page: 0,
        records: [&old_q, &old_a]
            .into_iter()
            .map(|r| RecordRequirement {
                key: r.key(),
                stamp: r.stamp.clone(),
                deleted: false,
            })
            .collect(),
        boundaries: vec![],
        baselines: bases.to_vec(),
        units: vec![unit],
        staged: vec![],
        completed_units: Default::default(),
    };
    let staged = vec![];
    let local: Vec<_> = [deleted_q, deleted_a.clone()]
        .into_iter()
        .map(|r| {
            clacks_sync::client::ClientRecordFields {
                key: r.key(),
                latest: Observation::Accepted(r),
                retained: vec![],
                pending: None,
            }
            .try_into()
            .expect("coherent client fixture")
        })
        .collect();
    let batch = plan_batch(&manifest, &staged, &[], &[old_a.clone()], &local, &ev)?;
    assert!(batch.staged.is_empty());
    assert_eq!(batch.completed_units.len(), 1);
    assert_eq!(batch.accepted, vec![old_a]);
    assert_eq!(
        batch.client_updates.first().unwrap().record.latest(),
        &Observation::Accepted(deleted_a)
    );
    Ok(())
}

#[test]
fn covered_support_uses_accepted_successor_without_weakening_exact_facts() -> anyhow::Result<()> {
    use clacks_sync::service::{SupportRequirement, dependency::represents};
    let (_, baseline) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let baselines = [baseline.clone()];
    let evidence = EvidenceView {
        lineage: &baseline.lineage,
        baselines: &baselines,
    };

    let original = support::accepted(60, "original");
    let mut successor = support::accepted(70, "correction");
    support::edit_accepted(&mut successor, |fields| fields.state = RecordState::Deleted);

    let covered = SupportRequirement::CoveredEffect(VersionReference::from(&original));
    let exact = SupportRequirement::ExactFact(VersionReference::from(&original));
    assert!(represents(
        &covered,
        &RecordRequirement::from(&successor),
        &evidence
    )?);
    assert!(!represents(
        &exact,
        &RecordRequirement::from(&successor),
        &evidence
    )?);
    assert!(represents(
        &exact,
        &RecordRequirement::from(&original),
        &evidence
    )?);

    let missing = EvidenceView {
        lineage: &baseline.lineage,
        baselines: &[],
    };
    assert!(
        !represents(&covered, &RecordRequirement::from(&successor), &missing)?,
        "unknown ordering cannot cover an effect"
    );
    assert_eq!(covered.reference(), &VersionReference::from(&original));
    Ok(())
}

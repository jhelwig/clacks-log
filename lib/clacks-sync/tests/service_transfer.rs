mod support;
use clacks_sync::{
    account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
    lineage::Coverage,
    record::{RecordId, RecordState, RecordValue},
    service::{VersionReference, operation::CoupledRequirement},
    transfer::{
        RecordRequirement, WindowManifest, WindowMode, WindowProgress, finish_window, plan_batch,
    },
};

#[test]
fn split_atomic_unit_stages_without_partial_install_and_survives_reload() -> anyhow::Result<()> {
    let (_, b) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let bases = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &bases,
    };
    let q = support::accepted(60, "fixture");
    let mut tag = q.clone();
    tag.key.id = RecordId::Tag(support::id(11));
    tag.state = RecordState::Live(RecordValue::Tag(Box::new(
        clacks_model::tag::LogbookTag::new(support::id(11), "tag".into()),
    )));
    let source = InstalledCheckpoint {
        account: support::id(1),
        baseline: b.id,
        coverage: Coverage::from(q.stamp.accepted_at),
    };
    let unit = CoupledRequirement {
        id: support::id(100),
        members: vec![VersionReference::from(&q), VersionReference::from(&tag)],
    };
    let manifest = WindowManifest::new(
        source.account,
        source.clone(),
        WindowMode::Full,
        [&q, &tag]
            .into_iter()
            .map(|r| RecordRequirement {
                key: r.key,
                stamp: r.stamp.clone(),
                deleted: false,
            })
            .collect(),
        vec![],
        bases.to_vec(),
        vec![unit],
    )?;
    let mut progress = WindowProgress {
        source: source.clone(),
        mode: WindowMode::Full,
        generation: 0,
        installed: Default::default(),
        staged: vec![],
        completed_units: Default::default(),
    };
    let first = plan_batch(&manifest, &progress, std::slice::from_ref(&q), &[], &ev)?;
    assert!(first.accepted.is_empty());
    assert_eq!(first.staged.len(), 1);
    assert!(first.completed_units.is_empty());
    progress.staged = first.staged;
    progress.generation = first.next_generation;
    progress = serde_json::from_str(&serde_json::to_string(&progress)?)?;
    assert!(finish_window(&manifest, &progress, &[], &source, &ev).is_err());
    let second = plan_batch(&manifest, &progress, &[tag], &[], &ev)?;
    assert_eq!(second.accepted.len(), 2);
    assert!(second.staged.is_empty());
    assert_eq!(second.completed_units.len(), 1);
    Ok(())
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
    tag.key.id = RecordId::Tag(support::id(11));
    tag.state = RecordState::Live(RecordValue::Tag(Box::new(
        clacks_model::tag::LogbookTag::new(support::id(11), "tag".into()),
    )));
    let source = InstalledCheckpoint {
        account: support::id(1),
        baseline: b.id,
        coverage: Coverage::from(next.stamp.accepted_at),
    };
    let unit = CoupledRequirement {
        id: support::id(100),
        members: vec![VersionReference::from(&old), VersionReference::from(&tag)],
    };
    let manifest = WindowManifest::new(
        source.account,
        source.clone(),
        WindowMode::Full,
        [&next, &tag]
            .into_iter()
            .map(|r| RecordRequirement {
                key: r.key,
                stamp: r.stamp.clone(),
                deleted: false,
            })
            .collect(),
        vec![],
        bases.to_vec(),
        vec![unit],
    )?;
    let progress = WindowProgress {
        source,
        mode: WindowMode::Full,
        generation: 0,
        installed: Default::default(),
        staged: vec![],
        completed_units: Default::default(),
    };
    let first = plan_batch(&manifest, &progress, &[next], &[], &ev)?;
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
    let RecordId::Qso(qso) = q.key.id else {
        panic!()
    };
    let external = ExternalRecordKey {
        destination: support::id(42),
        record: RemoteRecordId::Qrz(7),
    };
    let destination = Destination {
        id: external.destination,
        collection: RemoteCollection::Qrz { book_id: Some(7) },
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
        r.key = RecordKey {
            account: q.key.account,
            id: RecordId::Service(payload.id()),
        };
        r.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
            payload,
            support: ServiceSupport {
                destination: Some(destination.clone()),
                ..Default::default()
            },
        })));
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
    if let RecordState::Live(RecordValue::Service(s)) = &mut d.state {
        s.support.required = vec![
            SupportRequirement::ExactFact(VersionReference::from(&o)),
            SupportRequirement::CoveredEffect(VersionReference::from(&a)),
            SupportRequirement::CoveredEffect(VersionReference::from(&q)),
        ];
    }
    let (_, b) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let bases = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &bases,
    };
    let source = InstalledCheckpoint {
        account: q.key.account,
        baseline: b.id,
        coverage: Coverage::from(q.stamp.accepted_at),
    };
    let unit = CoupledRequirement {
        id: support::id(505),
        members: [&q, &a, &d]
            .into_iter()
            .map(VersionReference::from)
            .collect(),
    };
    let manifest = WindowManifest::new(
        source.account,
        source.clone(),
        WindowMode::Full,
        [&q, &a, &d, &o]
            .into_iter()
            .map(|r| RecordRequirement {
                key: r.key,
                stamp: r.stamp.clone(),
                deleted: false,
            })
            .collect(),
        vec![],
        bases.to_vec(),
        vec![unit],
    )?;
    let mut progress = WindowProgress {
        source,
        mode: WindowMode::Full,
        generation: 0,
        installed: Default::default(),
        staged: vec![],
        completed_units: Default::default(),
    };
    for batch in [vec![d], vec![q, a]] {
        let next = plan_batch(&manifest, &progress, &batch, &[], &ev)?;
        assert!(next.accepted.is_empty());
        assert!(next.completed_units.is_empty());
        progress.staged = next.staged;
        progress.generation = next.next_generation;
    }
    let done = plan_batch(&manifest, &progress, &[o], &[], &ev)?;
    assert_eq!(done.accepted.len(), 4);
    assert!(done.staged.is_empty());
    assert_eq!(done.completed_units.len(), 1);
    let mut changed = manifest.clone();
    changed.units.clear();
    assert!(manifest.validate_replacement(&changed).is_err());
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
        client::ClientRecord,
        record::Observation,
        service::{ServiceRecord, ServiceSupport},
    };
    let old_q = support::accepted(190, "old");
    let RecordId::Qso(qso) = old_q.key.id else {
        panic!()
    };
    let destination = Destination {
        id: support::id(42),
        collection: RemoteCollection::Qrz { book_id: Some(7) },
        description: "book".into(),
    };
    let payload = ServicePayload::Association(ExternalAssociation {
        id: support::id(101),
        external: ExternalRecordKey {
            destination: destination.id,
            record: RemoteRecordId::Qrz(7),
        },
        qso,
        state: AssociationState::Active,
    });
    let mut old_a = old_q.clone();
    old_a.key.id = RecordId::Service(payload.id());
    old_a.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
        payload,
        support: ServiceSupport {
            destination: Some(destination),
            ..Default::default()
        },
    })));
    let mut deleted_q = support::accepted(200, "deleted");
    deleted_q.state = RecordState::Deleted;
    let mut deleted_a = old_a.clone();
    deleted_a.stamp = deleted_q.stamp.clone();
    if let RecordState::Live(RecordValue::Service(s)) = &mut deleted_a.state
        && let ServicePayload::Association(a) = &mut s.payload
    {
        a.state = AssociationState::DeletedQso;
    }
    let (_, b) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let bases = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &bases,
    };
    let source = InstalledCheckpoint {
        account: old_q.key.account,
        baseline: b.id,
        coverage: Coverage::from(old_q.stamp.accepted_at),
    };
    let unit = CoupledRequirement {
        id: support::id(102),
        members: [&old_q, &old_a]
            .into_iter()
            .map(VersionReference::from)
            .collect(),
    };
    let manifest = WindowManifest::new(
        source.account,
        source.clone(),
        WindowMode::Full,
        [&old_q, &old_a]
            .into_iter()
            .map(|r| RecordRequirement {
                key: r.key,
                stamp: r.stamp.clone(),
                deleted: false,
            })
            .collect(),
        vec![],
        bases.to_vec(),
        vec![unit],
    )?;
    let progress = WindowProgress {
        source,
        mode: WindowMode::Full,
        generation: 0,
        installed: Default::default(),
        staged: vec![],
        completed_units: Default::default(),
    };
    let local: Vec<_> = [deleted_q, deleted_a.clone()]
        .into_iter()
        .map(|r| ClientRecord {
            key: r.key,
            latest: Observation::Accepted(r),
            retained: vec![],
            pending: None,
        })
        .collect();
    let batch = plan_batch(&manifest, &progress, &[old_a], &local, &ev)?;
    assert!(batch.staged.is_empty());
    assert_eq!(batch.completed_units.len(), 1);
    assert_eq!(batch.accepted, vec![deleted_a]);
    Ok(())
}

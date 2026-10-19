mod support;
use clacks_sync::{
    account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
    client::{ClientRecord, OriginalBasis, PendingRecord},
    lineage::Coverage,
    record::{Observation, RecordId, RecordKey, RecordState, RecordValue},
    transfer::{
        RecordRequirement, WindowManifest, WindowMode, WindowProgress, finish_window, plan_batch,
    },
};
use std::collections::BTreeSet;
use support::{accepted, id};

#[test]
fn interrupted_window_preserves_coverage_and_pending_conflict() {
    let (_, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    let defs = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    let q = accepted(60, "shared");
    let mut h = q.clone();
    h.key = RecordKey {
        account: id(1),
        id: RecordId::Tag(id(11)),
    };
    h.state = RecordState::Live(RecordValue::Tag(Box::new(
        clacks_model::tag::LogbookTag::new(id(11), "H".into()),
    )));
    let mut p = q.clone();
    p.key = RecordKey {
        account: id(1),
        id: RecordId::Profile(id(12)),
    };
    p.state = RecordState::Live(RecordValue::Profile(Box::new(
        clacks_model::profile::SavedLocationProfile {
            id: id(12),
            name: "P".into(),
            station: support::qso(id(10)).own_station,
            default_equipment: None,
        },
    )));
    let source = InstalledCheckpoint {
        account: id(1),
        baseline: id(3),
        coverage: Coverage::from_entries([(id(2), 60)]).unwrap(),
    };
    let manifest = WindowManifest::new(
        id(1),
        source.clone(),
        WindowMode::Full,
        [&q, &h, &p]
            .into_iter()
            .map(|v| RecordRequirement {
                key: v.key,
                stamp: v.stamp.clone(),
                deleted: false,
            })
            .collect(),
        vec![],
        defs.to_vec(),
    )
    .unwrap();
    let mut progress = WindowProgress {
        source,
        mode: WindowMode::Full,
        generation: 0,
        installed: BTreeSet::new(),
    };
    let previous = InstalledCheckpoint {
        coverage: Coverage::from_entries([(id(2), 40)]).unwrap(),
        ..progress.source.clone()
    };
    let original = accepted(20, "old");
    let local = [ClientRecord {
        retained: Vec::new(),
        key: q.key,
        latest: Observation::Accepted(original.clone()),
        pending: Some(PendingRecord {
            witness: None,
            key: q.key,
            original: OriginalBasis::Accepted(original),
            working: accepted(20, "local").state,
            prepared: None,
        }),
    }];
    let batch = plan_batch(&manifest, &progress, std::slice::from_ref(&q), &local, &ev).unwrap();
    assert!(batch.client_updates.first().unwrap().review.is_some());
    progress.installed.extend(batch.installed_keys);
    progress.generation = batch.next_generation;
    let progress: WindowProgress =
        serde_json::from_str(&serde_json::to_string(&progress).unwrap()).unwrap();
    assert!(
        finish_window(
            &manifest,
            &progress,
            std::slice::from_ref(&q),
            &previous,
            &ev
        )
        .is_err()
    );
    assert!(previous.coverage.covers(clacks_sync::record::Position {
        timeline: id(2),
        revision: 40
    }));
    let batch = plan_batch(&manifest, &progress, &[p.clone(), h.clone()], &[], &ev).unwrap();
    let mut progress = progress;
    progress.installed.extend(batch.installed_keys);
    progress.generation = batch.next_generation;
    let done = finish_window(&manifest, &progress, &[q.clone(), h, p], &previous, &ev).unwrap();
    assert_eq!(done.next.coverage, manifest.source.coverage);
    let repeat = plan_batch(
        &manifest,
        &progress,
        &[q],
        &batch
            .client_updates
            .into_iter()
            .map(|u| u.record)
            .collect::<Vec<_>>(),
        &ev,
    )
    .unwrap();
    assert_eq!(repeat.expected_generation, 2);
}

#[test]
fn tombstones_scope_baselines_and_replaced_windows_are_checked() {
    let (_, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    let defs = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    let mut deleted = accepted(60, "gone");
    deleted.state = RecordState::Deleted;
    let source = InstalledCheckpoint {
        account: id(1),
        baseline: id(3),
        coverage: Coverage::from_entries([(id(2), 60)]).unwrap(),
    };
    let r = RecordRequirement {
        key: deleted.key,
        stamp: deleted.stamp.clone(),
        deleted: true,
    };
    let manifest = WindowManifest::new(
        id(1),
        source.clone(),
        WindowMode::Full,
        vec![r.clone(), r.clone()],
        vec![],
        defs.to_vec(),
    )
    .unwrap();
    assert_eq!(manifest.records.len(), 1);
    let progress = WindowProgress {
        source: source.clone(),
        mode: WindowMode::Full,
        generation: 0,
        installed: BTreeSet::new(),
    };
    assert!(finish_window(&manifest, &progress, &[], &source, &ev).is_err());
    assert!(finish_window(&manifest, &progress, &[accepted(60, "live")], &source, &ev).is_err());
    assert!(
        plan_batch(
            &manifest,
            &progress,
            &[deleted.clone()],
            &[],
            &EvidenceView {
                lineage: &b.lineage,
                baselines: &[]
            }
        )
        .is_err()
    );
    let mut bad = r.clone();
    bad.key.account = id(9);
    assert!(
        WindowManifest::new(
            id(1),
            source.clone(),
            WindowMode::Full,
            vec![bad],
            vec![],
            defs.to_vec()
        )
        .is_err()
    );
    let newer = InstalledCheckpoint {
        coverage: Coverage::from_entries([(id(2), 61)]).unwrap(),
        ..source.clone()
    };
    let replacement = WindowManifest::new(
        id(1),
        newer,
        WindowMode::Full,
        vec![r],
        vec![],
        defs.to_vec(),
    )
    .unwrap();
    assert!(plan_batch(&replacement, &progress, &[deleted], &[], &ev).is_err());
}

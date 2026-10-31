use crate::support;
use clacks_sync::{
    account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
    lineage::Coverage,
    record::{Observation, RecordState},
    transfer::{
        RecordRequirement, WindowMode,
        discovery::{DiscoveryEnd, DiscoveryPage, DiscoveryStart},
        receipt::{ReceiptState, finish_receipt, register_page},
    },
};
use support::{accepted, id};

#[test]
fn receipt_requires_complete_obligations_and_matching_end() {
    let (_, baseline) = AccountMetadata::initialize(id(1), id(2), id(3));
    let baselines = [baseline.clone()];
    let evidence = EvidenceView {
        lineage: &baseline.lineage,
        baselines: &baselines,
    };
    let source = InstalledCheckpoint {
        account: id(1),
        baseline: baseline.id,
        coverage: Coverage::from_entries([(id(2), 100)]).unwrap(),
    };
    let first = accepted(92, "first");
    let later = accepted(102, "later");
    let receipt = ReceiptState::begin(&DiscoveryStart {
        source: source.clone(),
        mode: WindowMode::Full,
    })
    .unwrap();
    let page = DiscoveryPage {
        sequence: 0,
        requirements: vec![RecordRequirement {
            key: first.key(),
            stamp: first.stamp.clone(),
            deleted: false,
        }],
        accepted: vec![later.clone()],
        support: vec![],
        baselines: baselines.to_vec(),
        boundaries: vec![],
        units: vec![],
    };
    let receipt = register_page(&receipt, &page).unwrap();
    let end = DiscoveryEnd {
        source: source.clone(),
        pages: 1,
    };
    assert!(finish_receipt(&receipt, &end, &[], None, &evidence).is_err());
    assert_eq!(
        finish_receipt(
            &receipt,
            &end,
            std::slice::from_ref(&later),
            None,
            &evidence
        )
        .unwrap(),
        source
    );
    assert!(register_page(&receipt, &page).is_err());
    let mut wrong_end = end.clone();
    wrong_end.pages = 0;
    assert!(
        finish_receipt(
            &receipt,
            &wrong_end,
            std::slice::from_ref(&later),
            None,
            &evidence
        )
        .is_err()
    );
    wrong_end = end;
    wrong_end.source.coverage = Coverage::from_entries([(id(2), 102)]).unwrap();
    assert!(
        finish_receipt(
            &receipt,
            &wrong_end,
            std::slice::from_ref(&later),
            None,
            &evidence
        )
        .is_err()
    );
    let mut unrelated = later;
    support::edit_stamp(&mut unrelated.stamp, |stamp| {
        stamp.accepted_at.timeline = id(77);
        stamp.vector = clacks_sync::clock::RecordVector::from_entries([(
            clacks_sync::clock::ClockActor::Server(id(77)),
            102,
        )])
        .unwrap();
        Ok(())
    });
    assert!(
        finish_receipt(
            &receipt,
            &DiscoveryEnd {
                source: source.clone(),
                pages: 1
            },
            &[unrelated],
            None,
            &evidence
        )
        .is_err()
    );
    let previous = InstalledCheckpoint {
        coverage: Coverage::from_entries([(id(2), 80)]).unwrap(),
        ..source.clone()
    };
    let retained = Coverage::from_entries([(id(2), 90)]).unwrap();
    assert_eq!(
        clacks_sync::transfer::discovery::select_mode(
            &source,
            Some(&previous),
            Some(&retained),
            &evidence
        )
        .unwrap(),
        WindowMode::Full
    );
}

#[test]
fn completion_requires_deletion_and_interpretation_in_the_same_account() {
    let (_, baseline) = AccountMetadata::initialize(id(1), id(2), id(3));
    let baselines = [baseline.clone()];
    let evidence = EvidenceView {
        lineage: &baseline.lineage,
        baselines: &baselines,
    };
    let mut deletion = accepted(60, "deleted");
    support::edit_accepted(&mut deletion, |fields| fields.state = RecordState::Deleted);
    let source = InstalledCheckpoint {
        account: id(1),
        baseline: baseline.id,
        coverage: deletion.stamp.accepted_at().into(),
    };
    let start = DiscoveryStart {
        source: source.clone(),
        mode: WindowMode::Full,
    };
    let receipt = ReceiptState::begin(&start).unwrap();
    let mut page = DiscoveryPage {
        sequence: 0,
        requirements: vec![],
        accepted: vec![deletion.clone()],
        support: vec![],
        baselines: baselines.to_vec(),
        boundaries: vec![],
        units: vec![],
    };
    let receipt = register_page(&receipt, &page).unwrap();
    let end = DiscoveryEnd { source, pages: 1 };
    assert!(finish_receipt(&receipt, &end, &[accepted(60, "live")], None, &evidence).is_err());
    assert!(
        finish_receipt(
            &receipt,
            &end,
            std::slice::from_ref(&deletion),
            None,
            &EvidenceView {
                lineage: &baseline.lineage,
                baselines: &[]
            }
        )
        .is_err()
    );
    assert!(finish_receipt(&receipt, &end, &[deletion], None, &evidence).is_ok());
    page.requirements.push(RecordRequirement {
        key: clacks_sync::record::RecordKey {
            account: id(99),
            id: page.accepted.first().unwrap().key().id,
        },
        stamp: accepted(60, "x").stamp,
        deleted: false,
    });
    assert!(register_page(&ReceiptState::begin(&start).unwrap(), &page).is_err());
}

#[test]
fn supporting_version_cannot_replace_the_sources_current_head() {
    use clacks_sync::{
        clock::{ClockActor, RecordVector},
        lineage::{BoundaryKind, TimelineBoundary},
        record::Position,
        transfer::{
            discovery::{DiscoveryPage, DiscoveryStart},
            receipt::{ReceiptState, register_page},
        },
    };
    let (_, baseline) = AccountMetadata::initialize(id(1), id(2), id(3));
    let definitions = [baseline.clone()];
    let mut lineage = baseline.lineage.clone();
    lineage
        .add_boundary(TimelineBoundary {
            at: Position {
                timeline: id(5),
                revision: 0,
            },
            includes: Coverage::from_entries([(id(2), 0)]).unwrap(),
            kind: BoundaryKind::ForkOrigin,
        })
        .unwrap();
    let evidence = EvidenceView {
        lineage: &lineage,
        baselines: &definitions,
    };
    let head = accepted(102, "current head");
    let mut supporting = accepted(92, "historical supporting value");
    crate::support::edit_stamp(&mut supporting.stamp, |fields| {
        fields.accepted_at = Position {
            timeline: id(5),
            revision: 1,
        };
        fields.vector = RecordVector::from_entries([(ClockActor::Server(id(5)), 1)]).unwrap();
        Ok(())
    });
    let receipt = ReceiptState::begin(&DiscoveryStart {
        source: InstalledCheckpoint {
            account: id(1),
            baseline: id(3),
            coverage: Coverage::from_entries([(id(2), 100)]).unwrap(),
        },
        mode: WindowMode::Full,
    })
    .unwrap();
    let page = DiscoveryPage {
        sequence: 0,
        requirements: vec![],
        accepted: vec![head.clone()],
        support: vec![supporting.clone()],
        baselines: definitions.to_vec(),
        boundaries: lineage.boundaries().to_vec(),
        units: vec![],
    };
    let registered = register_page(&receipt, &page).unwrap();
    let planned = clacks_sync::transfer::batch::plan_batch(
        &registered,
        &[],
        &page.support,
        &page.accepted,
        &[],
        &evidence,
    )
    .unwrap();
    let updates = planned.client_updates;
    assert_eq!(updates.len(), 1);
    let update = updates.first().unwrap();
    assert_eq!(update.record.latest(), &Observation::Accepted(head));
    assert!(update.record.pending().is_none());
    assert!(update.record.retained().is_empty());
    assert!(update.review.is_none());
}

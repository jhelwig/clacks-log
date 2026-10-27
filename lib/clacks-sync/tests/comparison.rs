use crate::support;
use clacks_sync::{
    account::{AccountMetadata, EvidenceView},
    clock::{ClockActor, RecordVector},
    compare::{RecordRelation, compare_records},
    error::UnknownReason,
    lineage::{BoundaryKind, Coverage, TimelineBoundary},
    record::Position,
};
use support::{accepted, id};

#[test]
fn record_context_can_exceed_global_recovery_prefix() {
    let (_, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    let defs = [b.clone()];
    let mut l = b.lineage;
    l.add_boundary(TimelineBoundary {
        at: Position {
            timeline: id(4),
            revision: 0,
        },
        includes: Coverage::from_entries([(id(2), 160)]).unwrap(),
        kind: BoundaryKind::ForkOrigin,
    })
    .unwrap();
    let a = accepted(180, "a");
    let mut c = accepted(181, "b");
    c.stamp.accepted_at = Position {
        timeline: id(4),
        revision: 5,
    };
    c.stamp.vector = RecordVector::from_entries([
        (ClockActor::Server(id(2)), 180),
        (ClockActor::Server(id(4)), 5),
    ])
    .unwrap();
    let ev = EvidenceView {
        lineage: &l,
        baselines: &defs,
    };
    assert_eq!(
        compare_records(&a, &c, &ev).unwrap(),
        RecordRelation::Before
    );
    assert!(
        !l.coverage_at(c.stamp.accepted_at)
            .unwrap()
            .covers(a.stamp.accepted_at)
    );
    let missing = EvidenceView {
        lineage: &l,
        baselines: &[],
    };
    assert_eq!(
        compare_records(&a, &c, &missing).unwrap(),
        RecordRelation::Unknown(UnknownReason::MissingBaseline)
    );
}

#[test]
fn conflicting_identity_and_baseline_definitions_are_errors() {
    let (_, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    let mut bad = b.clone();
    bad.membership_generation = 1;
    let defs = [b.clone(), bad];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    assert!(compare_records(&accepted(1, "a"), &accepted(2, "b"), &ev).is_err());
    let defs = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    assert!(compare_records(&accepted(1, "a"), &accepted(1, "b"), &ev).is_err());
    let mut cross = accepted(2, "b");
    cross.key.account = id(9);
    assert!(compare_records(&accepted(1, "a"), &cross, &ev).is_err());
}

#[test]
fn complete_incomparable_contexts_diverge_and_opposite_proof_errors() {
    let (_, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    let defs = [b.clone()];
    let mut l = b.lineage;
    l.add_boundary(TimelineBoundary {
        at: Position {
            timeline: id(4),
            revision: 0,
        },
        includes: Coverage::from_entries([(id(2), 0)]).unwrap(),
        kind: BoundaryKind::ForkOrigin,
    })
    .unwrap();
    let a = accepted(180, "a");
    let mut c = accepted(181, "b");
    c.stamp.accepted_at = Position {
        timeline: id(4),
        revision: 5,
    };
    c.stamp.vector = RecordVector::from_entries([(ClockActor::Server(id(4)), 5)]).unwrap();
    let ev = EvidenceView {
        lineage: &l,
        baselines: &defs,
    };
    assert_eq!(
        compare_records(&a, &c, &ev).unwrap(),
        RecordRelation::Divergent
    );
    let mut contradictory = a.clone();
    contradictory.stamp.vector = contradictory
        .stamp
        .vector
        .join(&RecordVector::from_entries([(ClockActor::Server(id(2)), 200)]).unwrap());
    assert!(compare_records(&contradictory, &accepted(200, "c"), &ev).is_err());
}

#[test]
fn explicit_server_succession_survives_retired_component_projection() {
    let (_, old) = AccountMetadata::initialize(id(1), id(2), id(3));
    let mut new = old.clone();
    new.id = id(20);
    new.coverage = Coverage::from_entries([(id(2), 300)]).unwrap();
    let defs = [old.clone(), new];
    let ev = EvidenceView {
        lineage: &old.lineage,
        baselines: &defs,
    };
    let mut a = accepted(400, "old");
    a.stamp.vector = a
        .stamp
        .vector
        .join(&RecordVector::from_entries([(ClockActor::Client(id(4)), 1)]).unwrap());
    let mut b = accepted(600, "new");
    b.stamp.baseline = id(20);
    assert_eq!(
        compare_records(&a, &b, &ev).unwrap(),
        RecordRelation::Before
    );
}

#[test]
fn same_acceptance_compares_across_projection_above_common_prefix() {
    let (_, old) = AccountMetadata::initialize(id(1), id(2), id(3));
    let mut new = old.clone();
    new.id = id(20);
    new.coverage = Coverage::from_entries([(id(2), 300)]).unwrap();
    let defs = [old.clone(), new];
    let ev = EvidenceView {
        lineage: &old.lineage,
        baselines: &defs,
    };
    let mut a = accepted(600, "same");
    a.stamp.vector = a
        .stamp
        .vector
        .join(&RecordVector::from_entries([(ClockActor::Client(id(4)), 3)]).unwrap());
    let mut b = accepted(600, "same");
    b.stamp.baseline = id(20);
    assert_eq!(
        compare_records(&a, &b, &ev).unwrap(),
        RecordRelation::SameVersion
    );
}

#[test]
fn projection_does_not_excuse_disagreeing_positive_components() {
    let (_, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    let mut new = b.clone();
    new.id = id(20);
    new.coverage = Coverage::from_entries([(id(2), 700)]).unwrap();
    let defs = [b.clone(), new];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    let mut old = accepted(600, "same");
    old.stamp.vector = old
        .stamp
        .vector
        .join(&RecordVector::from_entries([(ClockActor::Client(id(4)), 3)]).unwrap());
    let mut projected = old.clone();
    projected.stamp.baseline = id(20);
    projected.stamp.vector = projected
        .stamp
        .vector
        .join(&RecordVector::from_entries([(ClockActor::Client(id(4)), 4)]).unwrap());
    assert!(compare_records(&old, &projected, &ev).is_err());
}

#[test]
fn receipt_and_preparation_apply_whole_record_policy() {
    use clacks_sync::{
        client::{
            ClientRecord, LocalCounter, OriginalBasis, PendingRecord, Preparation, ReviewReason,
            VersionCondition, prepare_change, receive_accepted,
        },
        record::{Observation, RecordState, RecordValue},
    };
    let (_, baseline) = AccountMetadata::initialize(id(1), id(2), id(3));
    let baselines = [baseline.clone()];
    let evidence = EvidenceView {
        lineage: &baseline.lineage,
        baselines: &baselines,
    };
    let original = accepted(1, "original");
    let mut tagged = original.state.clone();
    let RecordState::Live(RecordValue::Qso(qso)) = &mut tagged else {
        panic!("qso")
    };
    qso.tags.insert(id(20));
    let mut other_tag = original.state.clone();
    let RecordState::Live(RecordValue::Qso(qso)) = &mut other_tag else {
        panic!("qso")
    };
    qso.tags.insert(id(21));
    let edited = accepted(1, "shared edit").state;
    struct Case {
        local: RecordState,
        shared: RecordState,
        review: bool,
        pending: bool,
    }
    for Case {
        local,
        shared,
        review,
        pending,
    } in [
        Case {
            local: original.state.clone(),
            shared: original.state.clone(),
            review: false,
            pending: false,
        },
        Case {
            local: tagged.clone(),
            shared: original.state.clone(),
            review: false,
            pending: true,
        },
        Case {
            local: original.state.clone(),
            shared: edited.clone(),
            review: false,
            pending: false,
        },
        Case {
            local: tagged.clone(),
            shared: tagged.clone(),
            review: false,
            pending: false,
        },
        Case {
            local: tagged.clone(),
            shared: edited.clone(),
            review: true,
            pending: true,
        },
        Case {
            local: tagged.clone(),
            shared: other_tag,
            review: true,
            pending: true,
        },
        Case {
            local: RecordState::Deleted,
            shared: original.state.clone(),
            review: false,
            pending: true,
        },
        Case {
            local: RecordState::Deleted,
            shared: edited,
            review: true,
            pending: true,
        },
        Case {
            local: tagged,
            shared: RecordState::Deleted,
            review: true,
            pending: true,
        },
    ] {
        let current = ClientRecord {
            key: original.key,
            latest: Observation::Accepted(original.clone()),
            retained: vec![],
            pending: Some(PendingRecord {
                key: original.key,
                original: OriginalBasis::Accepted(original.clone()),
                working: local.clone(),
                prepared: None,
                witness: None,
            }),
        };
        let mut incoming = accepted(2, "shared edit");
        incoming.state = shared;
        let receipt = receive_accepted(&current, &incoming, &evidence).unwrap();
        assert_eq!(
            receipt.record.latest,
            Observation::Accepted(incoming.clone())
        );
        assert_eq!(receipt.record.pending.is_some(), pending);
        assert_eq!(
            receipt.review.as_ref().map(|r| &r.reason),
            review.then_some(&ReviewReason::BothDifferent)
        );
        if let Some(pending) = &receipt.record.pending {
            assert_eq!(pending.working, local);
            assert_eq!(pending.original, OriginalBasis::Accepted(original.clone()));
        }
        // Preparation must apply the same policy even before a receipt update was saved.
        let preparing = ClientRecord {
            latest: Observation::Accepted(incoming.clone()),
            ..current
        };
        let result = prepare_change(
            &preparing,
            id(4),
            &LocalCounter {
                actor: id(4),
                high_water: 0,
            },
            &[],
            &evidence,
        )
        .unwrap();
        if review {
            let Preparation::Review { record, input } = result else {
                panic!("whole-record review")
            };
            assert_eq!(input.reason, ReviewReason::BothDifferent);
            assert_eq!(input.local, local);
            assert_eq!(input.shared, Observation::Accepted(incoming));
            assert_eq!(record.pending.unwrap().working, local);
        } else if pending {
            let Preparation::Ready { change, .. } = result else {
                panic!("local proposal")
            };
            assert_eq!(change.state, local);
            assert_eq!(
                change.expected,
                VersionCondition::Is {
                    key: incoming.key,
                    stamp: incoming.stamp
                }
            );
        } else {
            let Preparation::Unchanged { record } = result else {
                panic!("settled values")
            };
            assert!(record.pending.is_none());
        }
    }
}

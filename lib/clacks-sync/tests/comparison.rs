mod support;
use clacks_sync::{
    account::{AccountMetadata, EvidenceView},
    clock::{ClockActor, RecordVector},
    compare::{ChangePattern, RecordRelation, classify_changes, compare_records},
    error::UnknownReason,
    lineage::{BoundaryKind, Coverage, TimelineBoundary},
    record::{Position, RecordState, RecordValue},
};
use support::{accepted, id};

#[test]
fn whole_record_truth_table_includes_deletion_and_disjoint_fields() {
    for (a, b, c, want) in [
        (0, 0, 0, ChangePattern::Unchanged),
        (0, 1, 0, ChangePattern::LocalOnly),
        (0, 0, 1, ChangePattern::SharedOnly),
        (0, 1, 1, ChangePattern::EqualResults),
        (0, 1, 2, ChangePattern::BothDifferent),
    ] {
        assert_eq!(classify_changes(&a, &b, &c), want);
    }
    let a = accepted(1, "a").state;
    let mut b = a.clone();
    let mut c = a.clone();
    if let RecordState::Live(RecordValue::Qso(q)) = &mut b {
        q.tags.insert(id(20));
    }
    if let RecordState::Live(RecordValue::Qso(q)) = &mut c {
        q.comment = Some("c".into());
    }
    assert_eq!(classify_changes(&a, &b, &c), ChangePattern::BothDifferent);
    if let RecordState::Live(RecordValue::Qso(q)) = &mut c {
        q.comment = Some("a".into());
        q.tags.insert(id(21));
    }
    assert_eq!(classify_changes(&a, &b, &c), ChangePattern::BothDifferent);
    assert_eq!(
        classify_changes(&a, &RecordState::Deleted, &a),
        ChangePattern::LocalOnly
    );
}

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

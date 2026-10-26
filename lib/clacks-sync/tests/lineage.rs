mod support;
use clacks_sync::account::AccountMetadata;
use clacks_sync::error::UnknownReason;
use clacks_sync::lineage::{BoundaryKind, Coverage, Lineage, PositionRelation, TimelineBoundary};
use clacks_sync::record::Position;
use support::id;

#[test]
fn recovered_coverage_is_not_retroactive() {
    let (t1, t2) = (id(1), id(2));
    let mut l = Lineage::new(id(3), t1);
    l.add_boundary(TimelineBoundary {
        at: Position {
            timeline: t2,
            revision: 0,
        },
        includes: Coverage::from_entries([(t1, 160)]).unwrap(),
        kind: BoundaryKind::ForkOrigin,
    })
    .unwrap();
    l.add_boundary(TimelineBoundary {
        at: Position {
            timeline: t2,
            revision: 41,
        },
        includes: Coverage::from_entries([(t1, 200)]).unwrap(),
        kind: BoundaryKind::RecoveredCoverage,
    })
    .unwrap();
    let round: Lineage = serde_json::from_str(&serde_json::to_string(&l).unwrap()).unwrap();
    assert!(
        !round
            .coverage_at(Position {
                timeline: t2,
                revision: 40
            })
            .unwrap()
            .covers(Position {
                timeline: t1,
                revision: 199
            })
    );
    assert!(
        round
            .coverage_at(Position {
                timeline: t2,
                revision: 41
            })
            .unwrap()
            .covers(Position {
                timeline: t1,
                revision: 199
            })
    );
    assert!(
        !round
            .coverage_at(Position {
                timeline: t2,
                revision: 41
            })
            .unwrap()
            .covers(Position {
                timeline: t1,
                revision: 201
            })
    );
    assert_eq!(
        Lineage::new(id(3), t1).compare(
            Position {
                timeline: t1,
                revision: 199
            },
            Position {
                timeline: t2,
                revision: 41
            }
        ),
        PositionRelation::Unknown(UnknownReason::MissingLineage)
    );
}

#[test]
fn boundary_rewrites_cycles_and_safe_reverse_cuts() {
    let (t1, t2) = (id(1), id(2));
    let mut l = Lineage::new(id(3), t1);
    let origin = TimelineBoundary {
        at: Position {
            timeline: t2,
            revision: 0,
        },
        includes: Coverage::from_entries([(t1, 10)]).unwrap(),
        kind: BoundaryKind::ForkOrigin,
    };
    l.add_boundary(origin.clone()).unwrap();
    l.add_boundary(origin.clone()).unwrap();
    let mut conflicting = origin;
    conflicting.includes = Coverage::from_entries([(t1, 11)]).unwrap();
    assert!(l.add_boundary(conflicting).is_err());
    assert!(
        l.add_boundary(TimelineBoundary {
            at: Position {
                timeline: t1,
                revision: 5
            },
            includes: Coverage::from_entries([(t2, 0)]).unwrap(),
            kind: BoundaryKind::RecoveredCoverage
        })
        .is_err()
    );
    l.add_boundary(TimelineBoundary {
        at: Position {
            timeline: t1,
            revision: 20,
        },
        includes: Coverage::from_entries([(t2, 5)]).unwrap(),
        kind: BoundaryKind::RecoveredCoverage,
    })
    .unwrap();
    assert_eq!(
        l.compare(
            Position {
                timeline: t2,
                revision: 5
            },
            Position {
                timeline: t1,
                revision: 20
            }
        ),
        PositionRelation::Before
    );
}

#[test]
fn root_and_deep_ancestry_are_traversable() {
    let (a, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    assert_eq!(a.head.revision, 0);
    assert_eq!(b.account, a.account);
    assert!(b.coverage.covers(a.head));
    let mut l = b.lineage;
    for n in 3..160 {
        l.add_boundary(TimelineBoundary {
            at: Position {
                timeline: id(n),
                revision: 0,
            },
            includes: Coverage::from_entries([(id(n - 1), 0)]).unwrap(),
            kind: BoundaryKind::ForkOrigin,
        })
        .unwrap();
    }
    assert!(
        l.coverage_at(Position {
            timeline: id(159),
            revision: 4
        })
        .unwrap()
        .covers(Position {
            timeline: id(2),
            revision: 0
        })
    );
}

#[test]
fn evidence_cannot_cross_account_boundaries() {
    let (_, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    let defs = [b.clone()];
    let evidence = clacks_sync::account::EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    assert!(evidence.baseline(id(9), id(3)).is_err());
}

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
    support::edit_stamp(&mut c.stamp, |stamp| {
        stamp.accepted_at = Position {
            timeline: id(4),
            revision: 5,
        };
        stamp.vector = RecordVector::from_entries([
            (ClockActor::Server(id(2)), 180),
            (ClockActor::Server(id(4)), 5),
        ])
        .unwrap();
        Ok(())
    });
    let ev = EvidenceView {
        lineage: &l,
        baselines: &defs,
    };
    assert_eq!(
        compare_records(&a, &c, &ev).unwrap(),
        RecordRelation::Before
    );
    assert!(
        !l.coverage_at(c.stamp.accepted_at())
            .unwrap()
            .covers(a.stamp.accepted_at())
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
    support::edit_accepted(&mut cross, |cross| {
        cross.key.account = id(9);
    });
    assert!(compare_records(&accepted(1, "a"), &cross, &ev).is_err());
}

#[test]
fn complete_incomparable_contexts_diverge() {
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
    support::edit_stamp(&mut c.stamp, |stamp| {
        stamp.accepted_at = Position {
            timeline: id(4),
            revision: 5,
        };
        stamp.vector = RecordVector::from_entries([(ClockActor::Server(id(4)), 5)]).unwrap();
        Ok(())
    });
    let ev = EvidenceView {
        lineage: &l,
        baselines: &defs,
    };
    assert_eq!(
        compare_records(&a, &c, &ev).unwrap(),
        RecordRelation::Divergent
    );
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
    support::edit_stamp(&mut a.stamp, |stamp| {
        stamp.vector = stamp
            .vector
            .join(&RecordVector::from_entries([(ClockActor::Client(id(4)), 1)]).unwrap());
        Ok(())
    });
    let mut b = accepted(600, "new");
    support::edit_stamp(&mut b.stamp, |stamp| {
        stamp.baseline = id(20);
        Ok(())
    });
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
    support::edit_stamp(&mut a.stamp, |stamp| {
        stamp.vector = stamp
            .vector
            .join(&RecordVector::from_entries([(ClockActor::Client(id(4)), 3)]).unwrap());
        Ok(())
    });
    let mut b = accepted(600, "same");
    support::edit_stamp(&mut b.stamp, |stamp| {
        stamp.baseline = id(20);
        Ok(())
    });
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
    support::edit_stamp(&mut old.stamp, |stamp| {
        stamp.vector = stamp
            .vector
            .join(&RecordVector::from_entries([(ClockActor::Client(id(4)), 3)]).unwrap());
        Ok(())
    });
    let mut projected = old.clone();
    support::edit_stamp(&mut projected.stamp, |stamp| {
        stamp.baseline = id(20);
        Ok(())
    });
    support::edit_stamp(&mut projected.stamp, |stamp| {
        stamp.vector = stamp
            .vector
            .join(&RecordVector::from_entries([(ClockActor::Client(id(4)), 4)]).unwrap());
        Ok(())
    });
    assert!(compare_records(&old, &projected, &ev).is_err());
}

#[test]
fn receipt_and_preparation_apply_whole_record_policy() {
    use clacks_sync::{
        client::{
            LocalCounter, OriginalBasis, PendingRecord, Preparation, ReviewReason,
            VersionCondition, preparation::prepare_change, receive_accepted,
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
    let mut tagged = original.state().clone();
    let RecordState::Live(RecordValue::Qso(qso)) = &mut tagged else {
        panic!("qso")
    };
    support::edit(qso, |qso| {
        qso.tags.insert(id(20));
        Ok(())
    });
    let mut other_tag = original.state().clone();
    let RecordState::Live(RecordValue::Qso(qso)) = &mut other_tag else {
        panic!("qso")
    };
    support::edit(qso, |qso| {
        qso.tags.insert(id(21));
        Ok(())
    });
    let edited = accepted(1, "shared edit").state().clone();
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
            local: original.state().clone(),
            shared: original.state().clone(),
            review: false,
            pending: false,
        },
        Case {
            local: tagged.clone(),
            shared: original.state().clone(),
            review: false,
            pending: true,
        },
        Case {
            local: original.state().clone(),
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
            shared: original.state().clone(),
            review: false,
            pending: true,
        },
        Case {
            local: RecordState::Deleted,
            shared: edited.clone(),
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
        let current = clacks_sync::client::ClientRecordFields {
            key: original.key(),
            latest: Observation::Accepted(original.clone()),
            retained: vec![],
            pending: Some(PendingRecord {
                key: original.key(),
                original: OriginalBasis::Accepted(original.clone()),
                working: local.clone(),
                prepared: None,
                witness: None,
            }),
        }
        .try_into()
        .expect("coherent client fixture");
        let mut incoming = accepted(2, "shared edit");

        support::edit_accepted(&mut incoming, |incoming| {
            incoming.state = shared;
        });
        let receipt = receive_accepted(&current, &incoming, &evidence).unwrap();
        assert_eq!(
            receipt.record.latest(),
            &Observation::Accepted(incoming.clone())
        );
        assert_eq!(receipt.record.pending().is_some(), pending);
        assert_eq!(
            receipt.review.as_ref().map(|r| &r.reason),
            review.then_some(&ReviewReason::BothDifferent)
        );
        if let Some(pending) = &receipt.record.pending() {
            assert_eq!(pending.working, local);
            assert_eq!(pending.original, OriginalBasis::Accepted(original.clone()));
        }
        // Preparation must apply the same policy even before a receipt update was saved.
        let preparing = clacks_sync::client::ClientRecordFields {
            latest: Observation::Accepted(incoming.clone()),
            ..current.into_fields()
        }
        .try_into()
        .expect("coherent client fixture");
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
            assert_eq!(record.pending().unwrap().working, local);
        } else if pending {
            let Preparation::Ready { change, .. } = result else {
                panic!("local proposal")
            };
            assert_eq!(change.state(), &local);
            assert_eq!(
                change.expected(),
                &VersionCondition::Is {
                    key: incoming.key(),
                    stamp: incoming.stamp
                }
            );
        } else {
            let Preparation::Unchanged { record } = result else {
                panic!("settled values")
            };
            assert!(record.pending().is_none());
        }
    }
}

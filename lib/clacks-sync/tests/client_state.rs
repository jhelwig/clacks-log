use crate::support;
use clacks_sync::{
    account::{AccountMetadata, EvidenceView},
    client::{
        ClientRecord, LocalCounter, OriginalBasis, PendingRecord, Preparation,
        preparation::prepare_change, receive_accepted,
    },
    clock::{ClockActor, RecordVector},
    record::{Observation, RecordState},
};
use support::{accepted, id};

fn pending() -> ClientRecord {
    let a = accepted(200, "A");
    clacks_sync::client::ClientRecordFields {
        retained: Vec::new(),
        key: a.key(),
        latest: Observation::Accepted(a.clone()),
        pending: Some(PendingRecord {
            witness: None,
            key: a.key(),
            original: OriginalBasis::Accepted(a),
            working: accepted(200, "B").state().clone(),
            prepared: None,
        }),
    }
    .try_into()
    .expect("coherent client fixture")
}

#[test]
fn lost_response_keeps_later_work() {
    let captured = support::telemetry::capture(|| {
        let (_, b) = AccountMetadata::initialize(id(1), id(2), id(3));
        let defs = [b.clone()];
        let ev = EvidenceView {
            lineage: &b.lineage,
            baselines: &defs,
        };
        let Preparation::Ready {
            mut record,
            change,
            counter,
        } = prepare_change(
            &pending(),
            id(4),
            &LocalCounter {
                actor: id(4),
                high_water: 0,
            },
            &[],
            &ev,
        )
        .unwrap()
        else {
            panic!("ready")
        };
        assert_eq!(change.vector().component(ClockActor::Client(id(4))), 1);
        support::edit_client(&mut record, |fields| {
            fields.pending.as_mut().unwrap().working = accepted(200, "C").state().clone();
        });
        let record: ClientRecord =
            serde_json::from_str(&serde_json::to_string(&record).unwrap()).unwrap();
        let Preparation::Retry { change: retry, .. } =
            prepare_change(&record, id(4), &counter, &[], &ev).unwrap()
        else {
            panic!("retry")
        };
        assert_eq!(retry, change);
        let mut shared = accepted(201, "B");
        support::edit_stamp(&mut shared.stamp, |stamp| {
            stamp.vector = stamp
                .vector
                .join(&RecordVector::from_entries([(ClockActor::Client(id(4)), 1)]).unwrap());
            Ok(())
        });
        let update = receive_accepted(&record, &shared, &ev).unwrap();
        assert!(update.review.is_none());
        let p = update.record.pending().unwrap();
        assert_eq!(p.working, accepted(200, "C").state().clone());
        assert!(p.prepared.is_none());
        let OriginalBasis::SubsumedOwn { state, witness, .. } = &p.original else {
            panic!("witnessed own version")
        };
        assert_eq!(state, accepted(200, "B").state());
        assert_eq!(witness, &shared.stamp);
        let mut d = accepted(202, "D");
        support::edit_stamp(&mut d.stamp, |stamp| {
            stamp.vector = stamp.vector.join(change.vector());
            Ok(())
        });
        assert!(
            receive_accepted(&update.record, &d, &ev)
                .unwrap()
                .review
                .is_some()
        );
    });
    let retry = captured.event("client.retry_prepared");
    assert!(
        captured
            .observations
            .spans
            .iter()
            .any(|span| { span.name == "sync.client.prepare" && span.id == retry.parent })
    );
    assert_eq!(
        captured
            .event("client.review_required")
            .fields
            .get("review_reason")
            .map(String::as_str),
        Some("BothDifferent")
    );
    assert!(
        captured
            .observations
            .events
            .iter()
            .all(|e| e.fields.get("level").is_none_or(|l| l != "ERROR"))
    );
    captured.result
}

#[test]
fn original_survives_without_own_acceptance_and_deletion_is_a_value() {
    let (_, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    let defs = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    let mut c = pending();
    support::edit_client(&mut c, |fields| {
        fields.pending.as_mut().unwrap().working = RecordState::Deleted;
    });
    let result = receive_accepted(&c, &accepted(201, "D"), &ev).unwrap();
    assert!(result.review.is_some());
    assert_eq!(
        result.record.pending().unwrap().working,
        RecordState::Deleted
    );
    let mut c = pending();
    support::edit_client(&mut c, |fields| {
        fields.pending.as_mut().unwrap().original = OriginalBasis::Unavailable {
            key: fields.key,
            reason: clacks_sync::error::UnknownReason::MissingOriginal,
        };
    });
    assert!(
        receive_accepted(&c, &accepted(201, "D"), &ev)
            .unwrap()
            .review
            .is_some()
    );
    assert!(
        prepare_change(
            &pending(),
            id(4),
            &LocalCounter {
                actor: id(4),
                high_water: u64::MAX
            },
            &[],
            &ev
        )
        .is_err()
    );
}

#[test]
fn equal_work_is_settled_but_old_prepared_actor_is_not_relabelled() {
    let (_, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    let defs = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    let Preparation::Ready { record, .. } = prepare_change(
        &pending(),
        id(4),
        &LocalCounter {
            actor: id(4),
            high_water: 0,
        },
        &[],
        &ev,
    )
    .unwrap() else {
        panic!("ready")
    };
    let Preparation::Review { record: kept, .. } = prepare_change(
        &record,
        id(5),
        &LocalCounter {
            actor: id(5),
            high_water: 0,
        },
        &[],
        &ev,
    )
    .unwrap() else {
        panic!("excluded actor review")
    };
    assert_eq!(kept.pending(), record.pending());
    let update = receive_accepted(&pending(), &accepted(201, "B"), &ev).unwrap();
    assert!(update.review.is_none());
    assert!(update.record.pending().is_none());
}

#[test]
fn superseded_own_version_keeps_its_values_without_fabricating_position() {
    let (_, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    let defs = [b.clone()];
    let ev = EvidenceView {
        lineage: &b.lineage,
        baselines: &defs,
    };
    let Preparation::Ready {
        mut record, change, ..
    } = prepare_change(
        &pending(),
        id(4),
        &LocalCounter {
            actor: id(4),
            high_water: 8,
        },
        &[],
        &ev,
    )
    .unwrap()
    else {
        panic!("ready")
    };
    support::edit_client(&mut record, |fields| {
        fields.pending.as_mut().unwrap().working = accepted(200, "C").state().clone();
    });
    let mut d = accepted(202, "D");
    support::edit_stamp(&mut d.stamp, |stamp| {
        stamp.vector = stamp.vector.join(change.vector());
        Ok(())
    });
    let update = receive_accepted(&record, &d, &ev).unwrap();
    let review = update.review.unwrap();
    let OriginalBasis::SubsumedOwn { state, witness, .. } = &review.original else {
        panic!("own basis")
    };
    assert_eq!(state, accepted(200, "B").state());
    assert_eq!(witness.accepted_at().revision, 202);
    let captured = support::telemetry::capture_filtered(
        || {
            clacks_sync::client::preparation::resolve_review(
                &review,
                accepted(200, "C").state().clone(),
                &ev,
            )
        },
        |metadata| *metadata.level() <= tracing::Level::INFO,
    );
    let event = captured.event("client.resolution_prepared");
    assert_eq!(event.fields.get("level").map(String::as_str), Some("INFO"));
    assert_eq!(
        captured
            .span("sync.client.resolve")
            .fields
            .get("level")
            .map(String::as_str),
        Some("INFO")
    );
    let key = review.shared.key();
    assert_eq!(
        event.fields.get("account_id"),
        Some(&key.account.to_string())
    );
    assert_eq!(event.fields.get("record_id"), Some(&key.id.to_string()));
    assert_eq!(event.parent, captured.span("sync.client.resolve").id);
    let resolved = captured.result.unwrap();
    let Preparation::Ready { change, .. } = prepare_change(
        &resolved,
        id(4),
        &LocalCounter {
            actor: id(4),
            high_water: 0,
        },
        &[],
        &ev,
    )
    .unwrap() else {
        panic!("resolved ready")
    };
    assert_eq!(change.vector().component(ClockActor::Client(id(4))), 10);
}

#[test]
fn a_subsumed_own_witness_does_not_order_an_unrelated_branch() {
    use clacks_sync::{
        lineage::{BoundaryKind, Coverage, TimelineBoundary},
        record::Position,
    };
    let (_, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    let defs = [b.clone()];
    let mut lineage = b.lineage.clone();
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
    let ev = EvidenceView {
        lineage: &lineage,
        baselines: &defs,
    };
    let mut current = pending();
    let mut other = accepted(5, "A");
    support::edit_stamp(&mut other.stamp, |stamp| {
        stamp.accepted_at = Position {
            timeline: id(5),
            revision: 5,
        };
        stamp.vector = RecordVector::from_entries([(ClockActor::Server(id(5)), 5)]).unwrap();
        Ok(())
    });
    support::edit_client(&mut current, |fields| {
        fields.latest = Observation::Accepted(other);
    });
    support::edit_client(&mut current, |fields| {
        let p = fields.pending.as_mut().unwrap();
        p.original = OriginalBasis::SubsumedOwn {
            key: p.key,
            state: accepted(10, "A").state().clone(),
            vector: accepted(10, "A").stamp.vector().clone(),
            witness: accepted(10, "A").stamp,
        };
    });
    assert!(matches!(
        prepare_change(
            &current,
            id(4),
            &LocalCounter {
                actor: id(4),
                high_water: 0
            },
            &[],
            &ev
        )
        .unwrap(),
        Preparation::Review { .. }
    ));
}

#[test]
fn incomparable_receipt_retains_context_and_joins_equal_values_without_review() {
    use clacks_sync::{
        lineage::{BoundaryKind, Coverage, TimelineBoundary},
        record::Position,
    };
    let (_, b) = AccountMetadata::initialize(id(1), id(2), id(3));
    let defs = [b.clone()];
    let mut lineage = b.lineage.clone();
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
    let ev = EvidenceView {
        lineage: &lineage,
        baselines: &defs,
    };
    let old = accepted(10, "same");
    let mut incoming = accepted(5, "same");
    support::edit_stamp(&mut incoming.stamp, |stamp| {
        stamp.accepted_at = Position {
            timeline: id(5),
            revision: 5,
        };
        stamp.vector = RecordVector::from_entries([(ClockActor::Server(id(5)), 5)]).unwrap();
        Ok(())
    });
    let current = clacks_sync::client::ClientRecordFields {
        key: old.key(),
        latest: Observation::Accepted(old.clone()),
        pending: None,
        retained: vec![],
    }
    .try_into()
    .expect("coherent client fixture");
    let update = receive_accepted(&current, &incoming, &ev).unwrap();
    assert!(update.review.is_none());
    assert_eq!(update.record.retained(), vec![old]);
    let Preparation::Ready { change, .. } = prepare_change(
        &update.record,
        id(4),
        &LocalCounter {
            actor: id(4),
            high_water: 0,
        },
        &[],
        &ev,
    )
    .unwrap() else {
        panic!("context reconciliation")
    };
    assert_eq!(change.vector().component(ClockActor::Server(id(2))), 10);
    let mut p = pending();
    let original = p.pending().unwrap().original.clone();
    support::edit_client(&mut p, |fields| {
        fields.latest = current.latest().clone();
    });
    let update = receive_accepted(&p, &incoming, &ev).unwrap();
    assert_eq!(update.record.pending().unwrap().original, original);
    assert!(!update.record.retained().is_empty());
}

#[test]
fn explicit_stale_response_releases_only_the_rejected_frozen_change() {
    let (_, baseline) = AccountMetadata::initialize(id(1), id(2), id(3));
    let definitions = [baseline.clone()];
    let evidence = EvidenceView {
        lineage: &baseline.lineage,
        baselines: &definitions,
    };
    let Preparation::Ready {
        record,
        change,
        counter,
    } = prepare_change(
        &pending(),
        id(4),
        &LocalCounter {
            actor: id(4),
            high_water: 0,
        },
        &[],
        &evidence,
    )
    .unwrap()
    else {
        panic!("ready")
    };
    // Without an explicit response, the exact frozen proposal must still be retried.
    assert!(matches!(
        prepare_change(&record, id(4), &counter, &[], &evidence).unwrap(),
        Preparation::Retry { .. }
    ));
    let absent = Observation::Absent {
        key: record.key(),
        checkpoint: clacks_sync::account::InstalledCheckpoint {
            account: id(1),
            baseline: id(3),
            coverage: clacks_sync::lineage::Coverage::from_entries([(id(2), 201)]).unwrap(),
        },
    };
    let missing = clacks_sync::client::receive_stale(&record, &change, &absent, &evidence).unwrap();
    assert!(matches!(
        missing.review.as_ref().map(|value| &value.reason),
        Some(clacks_sync::client::ReviewReason::BothDifferent)
    ));
    assert_eq!(
        missing.record.pending().unwrap().original,
        record.pending().unwrap().original
    );
    let update =
        clacks_sync::client::receive_stale(&record, &change, record.latest(), &evidence).unwrap();
    assert!(update.record.pending().unwrap().prepared.is_none());
    assert_eq!(
        update.record.pending().unwrap().working,
        record.pending().unwrap().working
    );
    let Preparation::Ready {
        change: replacement,
        ..
    } = prepare_change(&update.record, id(4), &counter, &[], &evidence).unwrap()
    else {
        panic!("new preparation")
    };
    assert_eq!(replacement.vector().component(ClockActor::Client(id(4))), 2);
}

#[test]
fn returned_review_preserves_later_local_work_and_the_shared_reason() {
    let current = pending();
    let shared = Observation::Accepted(accepted(201, "shared"));
    let input = clacks_sync::client::ReviewInput {
        original: OriginalBasis::Unavailable {
            key: current.key(),
            reason: clacks_sync::error::UnknownReason::MissingOriginal,
        },
        local: accepted(200, "older returned value").state().clone(),
        shared: shared.clone(),
        reason: clacks_sync::client::ReviewReason::Referenced {
            referrers: vec![],
            total: 1,
        },
        retained: vec![accepted(200, "older returned value")],
    };
    let update = clacks_sync::client::returned::receive_review(&current, &input).unwrap();
    assert_eq!(update.record.latest(), &shared);
    assert_eq!(update.record.working_state(), current.working_state());
    assert_eq!(
        update.record.pending().unwrap().original,
        current.pending().unwrap().original
    );
    let review = update.review.unwrap();
    assert_eq!(review.reason, input.reason);
    assert_eq!(review.local, current.pending().unwrap().working);
    assert!(
        review
            .retained
            .contains(&accepted(200, "older returned value"))
    );
}

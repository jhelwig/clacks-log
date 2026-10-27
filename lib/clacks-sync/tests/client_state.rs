use crate::support;
use clacks_sync::{
    account::{AccountMetadata, EvidenceView},
    client::{
        ClientRecord, LocalCounter, OriginalBasis, PendingRecord, Preparation, prepare_change,
        receive_accepted,
    },
    clock::{ClockActor, RecordVector},
    record::{Observation, RecordState},
};
use support::{accepted, id};

fn pending() -> ClientRecord {
    let a = accepted(200, "A");
    ClientRecord {
        retained: Vec::new(),
        key: a.key,
        latest: Observation::Accepted(a.clone()),
        pending: Some(PendingRecord {
            witness: None,
            key: a.key,
            original: OriginalBasis::Accepted(a),
            working: accepted(200, "B").state,
            prepared: None,
        }),
    }
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
        assert_eq!(change.vector.component(ClockActor::Client(id(4))), 1);
        record.pending.as_mut().unwrap().working = accepted(200, "C").state;
        let record: ClientRecord =
            serde_json::from_str(&serde_json::to_string(&record).unwrap()).unwrap();
        let Preparation::Retry { change: retry, .. } =
            prepare_change(&record, id(4), &counter, &[], &ev).unwrap()
        else {
            panic!("retry")
        };
        assert_eq!(retry, change);
        let mut shared = accepted(201, "B");
        shared.stamp.vector = shared
            .stamp
            .vector
            .join(&RecordVector::from_entries([(ClockActor::Client(id(4)), 1)]).unwrap());
        let update = receive_accepted(&record, &shared, &ev).unwrap();
        assert!(update.review.is_none());
        let p = update.record.pending.as_ref().unwrap();
        assert_eq!(p.working, accepted(200, "C").state);
        assert!(p.prepared.is_none());
        let OriginalBasis::SubsumedOwn { state, witness, .. } = &p.original else {
            panic!("witnessed own version")
        };
        assert_eq!(state, &accepted(200, "B").state);
        assert_eq!(witness, &shared.stamp);
        let mut d = accepted(202, "D");
        d.stamp.vector = d.stamp.vector.join(&change.vector);
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
    c.pending.as_mut().unwrap().working = RecordState::Deleted;
    let result = receive_accepted(&c, &accepted(201, "D"), &ev).unwrap();
    assert!(result.review.is_some());
    assert_eq!(result.record.pending.unwrap().working, RecordState::Deleted);
    let mut c = pending();
    c.pending.as_mut().unwrap().original = OriginalBasis::Unavailable {
        key: c.key,
        reason: clacks_sync::error::UnknownReason::MissingOriginal,
    };
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
    assert_eq!(kept.pending, record.pending);
    let update = receive_accepted(&pending(), &accepted(201, "B"), &ev).unwrap();
    assert!(update.review.is_none());
    assert!(update.record.pending.is_none());
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
    record.pending.as_mut().unwrap().working = accepted(200, "C").state;
    let mut d = accepted(202, "D");
    d.stamp.vector = d.stamp.vector.join(&change.vector);
    let update = receive_accepted(&record, &d, &ev).unwrap();
    let review = update.review.unwrap();
    let OriginalBasis::SubsumedOwn { state, witness, .. } = &review.original else {
        panic!("own basis")
    };
    assert_eq!(state, &accepted(200, "B").state);
    assert_eq!(witness.accepted_at.revision, 202);
    let captured = support::telemetry::capture_filtered(
        || clacks_sync::client::resolve_review(&review, accepted(200, "C").state, &ev),
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
    assert_eq!(change.vector.component(ClockActor::Client(id(4))), 10);
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
    other.stamp.accepted_at = Position {
        timeline: id(5),
        revision: 5,
    };
    other.stamp.vector = RecordVector::from_entries([(ClockActor::Server(id(5)), 5)]).unwrap();
    current.latest = Observation::Accepted(other);
    let p = current.pending.as_mut().unwrap();
    p.original = OriginalBasis::SubsumedOwn {
        key: p.key,
        state: accepted(10, "A").state,
        vector: accepted(10, "A").stamp.vector,
        witness: accepted(10, "A").stamp,
    };
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
    incoming.stamp.accepted_at = Position {
        timeline: id(5),
        revision: 5,
    };
    incoming.stamp.vector = RecordVector::from_entries([(ClockActor::Server(id(5)), 5)]).unwrap();
    let current = ClientRecord {
        key: old.key,
        latest: Observation::Accepted(old.clone()),
        pending: None,
        retained: vec![],
    };
    let update = receive_accepted(&current, &incoming, &ev).unwrap();
    assert!(update.review.is_none());
    assert_eq!(update.record.retained, vec![old]);
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
    assert_eq!(change.vector.component(ClockActor::Server(id(2))), 10);
    let mut p = pending();
    let original = p.pending.as_ref().unwrap().original.clone();
    p.latest = current.latest;
    let update = receive_accepted(&p, &incoming, &ev).unwrap();
    assert_eq!(update.record.pending.as_ref().unwrap().original, original);
    assert!(!update.record.retained.is_empty());
}

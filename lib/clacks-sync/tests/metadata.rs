mod support;

use clacks_model::ids::{AccountId, ClientActorId};
use clacks_sync::clock::{ClockActor, RecordVector, VectorRelation};
use clacks_sync::record::{
    AcceptedRecord, Position, RecordId, RecordKey, RecordState, RecordValue, VersionStamp,
};
use support::{id, qso};

#[test]
fn vectors_detect_concurrency_join_and_exhaustion() {
    let a = ClockActor::Client(id::<ClientActorId>(1));
    let b = ClockActor::Client(id::<ClientActorId>(2));
    let x = RecordVector::from_entries([(a, 4), (b, 0)]).unwrap();
    let y = RecordVector::from_entries([(b, 2)]).unwrap();
    assert_eq!(x.compare_components(&y), VectorRelation::Concurrent);
    assert_eq!(x.join(&y).component(a), 4);
    assert_eq!(x.join(&y).component(b), 2);
    assert_eq!(x.increment(a, 8).unwrap().1, 9);
    assert!(x.increment(a, u64::MAX).is_err());
    assert!(RecordVector::from_entries([(a, 1), (a, 2)]).is_err());
    let round: RecordVector = serde_json::from_str(&serde_json::to_string(&x).unwrap()).unwrap();
    assert_eq!(x, round);
    assert_eq!(x, RecordVector::from_entries([(a, 4)]).unwrap());
}

#[test]
fn accepted_envelopes_validate_native_state_and_identity() {
    let key = RecordKey {
        account: id::<AccountId>(1),
        id: RecordId::Qso(id(2)),
    };
    let timeline = id(3);
    let stamp = VersionStamp {
        accepted_at: Position {
            timeline,
            revision: 1,
        },
        baseline: id(4),
        vector: RecordVector::from_entries([(ClockActor::Server(timeline), 1)]).unwrap(),
    };
    let mut q = qso(id(2));
    assert!(
        AcceptedRecord::new(
            key,
            RecordState::Live(RecordValue::Qso(Box::new(q.clone()))),
            stamp.clone()
        )
        .is_ok()
    );
    q.end = Some(clacks_model::scalar::UtcSecond::from_unix_seconds(0));
    let err = AcceptedRecord::new(
        key,
        RecordState::Live(RecordValue::Qso(Box::new(q))),
        stamp.clone(),
    )
    .unwrap_err();
    assert!(
        err.downcast_ref::<clacks_model::contact::RecordError>()
            .is_some()
    );
    assert!(
        AcceptedRecord::new(
            key,
            RecordState::Live(RecordValue::Qso(Box::new(qso(id(9))))),
            stamp
        )
        .is_err()
    );
}

#[test]
fn scopes_roles_and_deletions_remain_distinct() {
    let key = RecordKey {
        account: id(1),
        id: RecordId::Qso(id(2)),
    };
    assert_ne!(
        key,
        RecordKey {
            account: id(9),
            ..key
        }
    );
    let timeline = id(3);
    let vector = RecordVector::from_entries([
        (ClockActor::Server(timeline), 7),
        (ClockActor::Client(id(3)), 2),
    ])
    .unwrap();
    assert_eq!(vector.component(ClockActor::Client(id(3))), 2);
    let record = AcceptedRecord::new(
        key,
        RecordState::Deleted,
        VersionStamp {
            accepted_at: Position {
                timeline,
                revision: 7,
            },
            baseline: id(4),
            vector,
        },
    )
    .unwrap();
    assert_eq!(
        serde_json::from_str::<AcceptedRecord>(&serde_json::to_string(&record).unwrap()).unwrap(),
        record
    );
    assert!(serde_json::from_str::<RecordVector>(r#"[[{"Client":"00000000000000000000000001"},1],[{"Client":"00000000000000000000000001"},2]]"#).is_err());
}

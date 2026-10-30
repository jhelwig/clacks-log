use crate::support;

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
    let q = qso(id(2));
    assert!(
        AcceptedRecord::new(
            key,
            RecordState::Live(RecordValue::Qso(Box::new(q.clone()))),
            stamp.clone()
        )
        .is_ok()
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
fn vector_ingress_rejects_conflicting_duplicate_components() {
    assert!(serde_json::from_str::<RecordVector>(r#"[[{"Client":"00000000000000000000000001"},1],[{"Client":"00000000000000000000000001"},2]]"#).is_err());
}

#[test]
fn record_ids_display_their_native_identifier() {
    use clacks_model::service::evidence::ServiceRecordId;
    let expected = id::<clacks_model::ids::QsoId>(7).to_string();
    for record in [
        RecordId::Qso(id(7)),
        RecordId::Tag(id(7)),
        RecordId::Profile(id(7)),
        RecordId::Equipment(id(7)),
        RecordId::Group(id(7)),
        RecordId::OutputProfile(id(7)),
        RecordId::Service(ServiceRecordId::Attempt(id(7))),
    ] {
        assert_eq!(record.to_string(), expected);
    }
}

pub mod service;

use clacks_model::{
    contact::{BandFrequency, OwnOperatingContext, Qso},
    ids::QsoId,
    recognition::mode::{Mode, ModeIdentity},
    scalar::{Hertz, RecordedCallsign, UtcSecond},
};
use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};

/// Build deterministic native fixture identities without a global clock or external generator.
pub fn id<T: std::str::FromStr>(value: u128) -> T
where
    T::Err: std::fmt::Debug,
{
    ulid::Ulid::from(value).to_string().parse().unwrap()
}

/// Build a complete recorded contact whose fields can be changed explicitly by each scenario.
pub fn qso(value: QsoId) -> Qso {
    Qso::try_from(clacks_model::contact::input::QsoFields {
        id: value,
        start: UtcSecond::from_unix_seconds(1000),
        end: None,
        own_station: clacks_model::contact::StationSnapshot {
            callsign: RecordedCallsign::new("W7HEX/P").unwrap(),
            location: None,
            references: Default::default(),
        },
        contacted_station: clacks_model::contact::StationSnapshot {
            callsign: RecordedCallsign::new("K1ABC").unwrap(),
            location: None,
            references: Default::default(),
        },
        own_context: (OwnOperatingContext::default()),
        contacted_operator: None,
        primary: BandFrequency::from_recorded_pair(Some(Hertz::new(14_074_000).unwrap()), None)
            .unwrap(),
        receive_override: None,
        mode: Mode::from_recorded_identity(ModeIdentity::Parent { label: "CW".into() }).unwrap(),
        sent_report: None,
        received_report: None,
        comment: None,
        tags: Default::default(),
    })
    .expect("valid contact fixture")
}

/// Qualify a native value with the fixture's local account.
pub fn key(value: &RecordValue) -> RecordKey {
    RecordKey {
        account: id(1),
        id: value.id(),
    }
}

/// Wrap a fixture contact as a complete live native state.
pub fn contact(value: Qso) -> RecordState {
    RecordState::Live(RecordValue::Qso(Box::new(value)))
}

/// Build a contact key in the fixture's local account.
pub fn qso_key(value: QsoId) -> RecordKey {
    RecordKey {
        account: id(1),
        id: RecordId::Qso(value),
    }
}

/// Build a validated accepted envelope for a supplied server cut in focused transfer fixtures.
pub fn accepted(revision: u64, state: RecordState) -> clacks_sync::record::AcceptedRecord {
    use clacks_sync::{
        clock::{ClockActor, RecordVector},
        record::{AcceptedRecord, Position, VersionStamp},
    };
    let RecordState::Live(value) = &state else {
        panic!("fixture requires a live value")
    };
    AcceptedRecord::new(
        key(value),
        state,
        VersionStamp::new(
            Position {
                timeline: id(2),
                revision,
            },
            id(3),
            RecordVector::from_entries([(ClockActor::Server(id(2)), revision)]).unwrap(),
        )
        .expect("valid accepted fixture context"),
    )
    .unwrap()
}

/// Apply a valid fixture correction through checked contact construction.
pub fn edit(
    qso: &mut Qso,
    change: impl FnOnce(
        &mut clacks_model::contact::input::QsoFields,
    ) -> Result<(), Box<dyn std::error::Error>>,
) {
    let mut fields = qso.clone().into_fields();
    change(&mut fields).expect("valid fixture input");
    *qso = Qso::try_from(fields).expect("valid contact fixture edit");
}

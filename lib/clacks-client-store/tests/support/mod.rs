#![allow(dead_code)]

use clacks_model::{
    contact::{BandFrequency, OwnOperatingContext, Qso, StationSnapshot},
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
    Qso {
        id: value,
        start: UtcSecond::from_unix_seconds(1000),
        end: None,
        own_station: StationSnapshot {
            callsign: RecordedCallsign::new("W7HEX/P").unwrap(),
            location: None,
            references: Default::default(),
        },
        contacted_station: StationSnapshot {
            callsign: RecordedCallsign::new("K1ABC").unwrap(),
            location: None,
            references: Default::default(),
        },
        own_context: OwnOperatingContext::default(),
        contacted_operator: None,
        primary: BandFrequency::from_recorded_pair(Some(Hertz::new(14_074_000).unwrap()), None)
            .unwrap(),
        receive_override: None,
        mode: Mode::from_recorded_identity(ModeIdentity::Parent { label: "CW".into() }).unwrap(),
        sent_report: None,
        received_report: None,
        comment: None,
        tags: Default::default(),
    }
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

#![allow(dead_code)]
use clacks_model::contact::{BandFrequency, OwnOperatingContext, Qso, StationSnapshot};
use clacks_model::ids::QsoId;
use clacks_model::recognition::RecognitionCatalog;
use clacks_model::recognition::mode::{Mode, ModeIdentity};
use clacks_model::scalar::{Hertz, RecordedCallsign, UtcSecond};
use std::collections::BTreeSet;

pub fn id<T: std::str::FromStr>(value: u128) -> T
where
    T::Err: std::fmt::Debug,
{
    ulid::Ulid::from(value).to_string().parse().unwrap()
}

pub fn qso(id: QsoId) -> Qso {
    Qso {
        id,
        start: UtcSecond::from_unix_seconds(1000),
        end: None,
        own_station: StationSnapshot {
            callsign: RecordedCallsign::new("W7HEX").unwrap(),
            location: None,
            references: BTreeSet::new(),
        },
        contacted_station: StationSnapshot {
            callsign: RecordedCallsign::new("K1ABC").unwrap(),
            location: None,
            references: BTreeSet::new(),
        },
        own_context: OwnOperatingContext::default(),
        contacted_operator: None,
        primary: BandFrequency::new(
            Some(Hertz::new(14_074_000).unwrap()),
            None,
            RecognitionCatalog::adif_318().expect("bundled recognition fixture must be valid"),
        )
        .unwrap(),
        receive_override: None,
        mode: Mode::from_recorded_identity(ModeIdentity::Specialization {
            label: "FT4".into(),
            parent: Some("MFSK".into()),
        })
        .unwrap(),
        sent_report: None,
        received_report: None,
        comment: None,
        tags: BTreeSet::new(),
    }
}

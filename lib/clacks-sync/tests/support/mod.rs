#![allow(dead_code)]

pub mod telemetry;

use clacks_model::contact::{BandFrequency, OwnOperatingContext, Qso};
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
    Qso::try_from(clacks_model::contact::input::QsoFields {
        id,
        start: UtcSecond::from_unix_seconds(1000),
        end: None,
        own_station: clacks_model::contact::StationSnapshot {
            callsign: RecordedCallsign::new("W7HEX").unwrap(),
            location: None,
            references: BTreeSet::new(),
        },
        contacted_station: clacks_model::contact::StationSnapshot {
            callsign: RecordedCallsign::new("K1ABC").unwrap(),
            location: None,
            references: BTreeSet::new(),
        },
        own_context: (OwnOperatingContext::default()),
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
    })
    .expect("valid contact fixture")
}

pub fn accepted(revision: u64, comment: &str) -> clacks_sync::record::AcceptedRecord {
    use clacks_sync::{
        clock::{ClockActor, RecordVector},
        record::{
            AcceptedRecord, Position, RecordId, RecordKey, RecordState, RecordValue, VersionStamp,
        },
    };
    let timeline = id(2);
    let mut q = qso(id(10));
    edit(&mut q, |q| {
        q.comment = Some(comment.into());
        Ok(())
    });
    AcceptedRecord::new(
        RecordKey {
            account: id(1),
            id: RecordId::Qso(q.id()),
        },
        RecordState::Live(RecordValue::Qso(Box::new(q))),
        VersionStamp {
            accepted_at: Position { timeline, revision },
            baseline: id(3),
            vector: RecordVector::from_entries([(ClockActor::Server(timeline), revision)]).unwrap(),
        },
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

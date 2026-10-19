#![allow(dead_code)]

use clacks_model::*;
use std::collections::BTreeSet;

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
            RecognitionCatalog::adif_318(),
        )
        .unwrap(),
        receive_override: None,
        mode: Mode::Known(KnownMode::Mfsk(Some(MfskSubmode::Ft4))),
        sent_report: None,
        received_report: None,
        comment: None,
        tags: BTreeSet::new(),
    }
}

pub fn reference(group: GroupId, identifier: Option<&str>) -> ReferenceAssociation {
    ReferenceAssociation {
        group_id: group,
        identifier: identifier.map(str::to_owned),
    }
}

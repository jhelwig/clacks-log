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

pub fn definitions() -> DefinitionSet {
    DefinitionSet::new(serde_json::from_str(include_str!("../fixtures/definitions.json")).unwrap())
        .unwrap()
}

pub fn catalog_case() -> (
    Qso,
    ReferenceAssociation,
    DefinitionSet,
    CatalogSet,
    CatalogId,
) {
    let mut contact = qso(QsoId::from_u128(1));
    let association = reference(GroupId::from_u128(1), Some("US-1234"));
    contact
        .contacted_station
        .references
        .insert(association.clone());
    let id = CatalogId::from_u128(1);
    let catalog = ReferenceCatalog {
        id,
        provenance: SourceInformation {
            dataset_name: "parks".into(),
            publisher: Some("Example publisher".into()),
            source_url: Some("https://example.org/parks.csv".into()),
            obtained_at: UtcTimestamp::new(42, 0).unwrap(),
            upstream_version: None,
        },
        entries: [(
            "US-1234".into(),
            CatalogEntry {
                metadata: [
                    ("name".into(), serde_json::json!("Example park")),
                    ("status".into(), serde_json::json!("active")),
                    ("latitude".into(), serde_json::json!(40.0)),
                ]
                .into(),
            },
        )]
        .into(),
    };
    (
        contact,
        association,
        definitions(),
        [(id, catalog)].into(),
        id,
    )
}

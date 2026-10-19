#![allow(dead_code)]

use clacks_model::catalog::{CatalogEntry, CatalogSet, ReferenceCatalog, SourceInformation};
use clacks_model::contact::{BandFrequency, OwnOperatingContext, Qso, StationSnapshot};
use clacks_model::export::rules::ValueExpression;
use clacks_model::export::{OutputProfile, OutputPurpose, ProfileSet};
use clacks_model::group::DefinitionSet;
use clacks_model::ids::{CatalogId, GroupId, OutputProfileId, QsoId};
use clacks_model::recognition::RecognitionCatalog;
use clacks_model::recognition::mode::{Mode, ModeIdentity, ModeInterpretation};
use clacks_model::reference::ReferenceAssociation;
use clacks_model::scalar::{Hertz, RecordedCallsign, UtcSecond, UtcTimestamp};
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

pub fn profiles() -> ProfileSet {
    [
        (
            OutputProfileId::from_u128(1),
            OutputProfile {
                id: OutputProfileId::from_u128(1),
                name: "ordinary".into(),
                purpose: OutputPurpose::OrdinaryInterchange,
                contextual_fields: ["CONTEST_ID".into()].into(),
            },
        ),
        (
            OutputProfileId::from_u128(2),
            OutputProfile {
                id: OutputProfileId::from_u128(2),
                name: "submission fixture".into(),
                purpose: OutputPurpose::Targeted,
                contextual_fields: BTreeSet::new(),
            },
        ),
    ]
    .into()
}

pub fn contest_definitions() -> DefinitionSet {
    let defs = definitions();
    let mut b = defs.get(GroupId::from_u128(2)).unwrap().clone();
    b.id = GroupId::from_u128(3);
    b.name = "Other contest".into();
    b.export_mappings.ordinary_interchange[0].value = ValueExpression::Literal("CONTEST-B".into());
    DefinitionSet::new(defs.iter().cloned().chain([b]).collect()).unwrap()
}

pub fn legacy_view_round_trip(
    json: &str,
    catalog: &RecognitionCatalog,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut q: Qso = serde_json::from_str(json)?;
    let recorded_identity = match q.mode.interpret(catalog) {
        ModeInterpretation::Known(_) => q.mode.identity(),
        ModeInterpretation::Other(identity) => identity,
    };
    q.mode = Mode::from_recorded_identity(recorded_identity)?;
    q.comment = Some("edited by older recognition view".into());
    Ok(serde_json::to_string(&q)?)
}

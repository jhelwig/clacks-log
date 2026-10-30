#![allow(dead_code)]

pub mod telemetry;

use clacks_model::catalog::{CatalogEntry, CatalogSet, ReferenceCatalog, SourceInformation};
use clacks_model::contact::{BandFrequency, OwnOperatingContext, Qso};
use clacks_model::export::rules::ValueExpression;
use clacks_model::export::{OutputProfile, OutputPurpose, ProfileSet};
use clacks_model::group::DefinitionSet;
use clacks_model::ids::{CatalogId, GroupId, OutputProfileId, QsoId};
use clacks_model::recognition::RecognitionCatalog;
use clacks_model::recognition::mode::{Mode, ModeIdentity};
use clacks_model::reference::ReferenceAssociation;
use clacks_model::scalar::{Hertz, RecordedCallsign, UtcSecond, UtcTimestamp};
use std::collections::BTreeSet;

/// Construct a deterministic fixture ID through the native string parser.
pub fn id<T>(value: u128) -> T
where
    T: std::str::FromStr,
    T::Err: std::fmt::Debug,
{
    ulid::Ulid::from(value)
        .to_string()
        .parse()
        .expect("fixture ULID must parse as a native ID")
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

pub fn reference(group: GroupId, identifier: Option<&str>) -> ReferenceAssociation {
    ReferenceAssociation::new(group, identifier.map(str::to_owned))
        .expect("valid reference fixture")
}

pub fn definitions() -> DefinitionSet {
    DefinitionSet::new(serde_json::from_str(include_str!("../fixtures/definitions.json")).unwrap())
        .unwrap()
}

pub struct CatalogCase {
    pub association: ReferenceAssociation,
    pub definitions: DefinitionSet,
    pub catalogs: CatalogSet,
    pub id: CatalogId,
}

pub fn catalog_case() -> CatalogCase {
    let association = reference(id::<GroupId>(1), Some("US-1234"));
    let id = id::<CatalogId>(1);
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
    CatalogCase {
        association,
        definitions: definitions(),
        catalogs: [(id, catalog)].into(),
        id,
    }
}

pub fn profiles() -> ProfileSet {
    [
        (
            id::<OutputProfileId>(1),
            OutputProfile {
                id: id::<OutputProfileId>(1),
                name: "ordinary".into(),
                purpose: OutputPurpose::OrdinaryInterchange,
                contextual_fields: ["CONTEST_ID".into()].into(),
            },
        ),
        (
            id::<OutputProfileId>(2),
            OutputProfile {
                id: id::<OutputProfileId>(2),
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
    let mut b = defs.get(id::<GroupId>(2)).unwrap().clone().into_fields();
    b.id = id::<GroupId>(3);
    b.name = "Other contest".into();
    b.export_mappings.ordinary_interchange[0].value = ValueExpression::Literal("CONTEST-B".into());
    DefinitionSet::new(
        defs.iter()
            .cloned()
            .chain([b.try_into().unwrap()])
            .collect(),
    )
    .unwrap()
}

/// Apply a valid fixture correction through the saved-contact input boundary.
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

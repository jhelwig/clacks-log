use clacks_geography::{
    artifact::{Field, ParentScope},
    assistance::{change_position, populate},
    lookup::{Candidate, Coverage, FieldLookup, LookupResult},
};
use clacks_model::{
    location::{CqZone, ItuZone, RecordedLocationFields},
    position::{Coordinates, GridLength, Locator, Position},
};
use std::collections::BTreeSet;

#[test]
fn population_is_independent_and_preserves_protected_absence() {
    let position = Position::ExplicitPoint {
        coordinates: Coordinates::from_microdegrees(500_000, 500_000).unwrap(),
        grid_length: GridLength::new(6).unwrap(),
    };
    let result = LookupResult {
        release_id: "authored-release".into(),
        dxcc: FieldLookup {
            coverage: Coverage::Complete,
            candidates: vec![candidate("291".to_owned())],
        },
        primary_subdivision: FieldLookup {
            coverage: Coverage::Complete,
            candidates: vec![candidate("WA".to_owned())],
        },
        secondary_subdivision: FieldLookup {
            coverage: Coverage::Complete,
            candidates: vec![
                candidate("WA,King".to_owned()),
                candidate("WA,Pierce".to_owned()),
            ],
        },
        cq_zone: FieldLookup {
            coverage: Coverage::None,
            candidates: vec![],
        },
        itu_zone: FieldLookup {
            coverage: Coverage::Partial,
            candidates: vec![candidate(ItuZone::new(6).unwrap())],
        },
    };
    let mut fields = RecordedLocationFields {
        position: Some(position),
        ..Default::default()
    };
    let protected = BTreeSet::from([Field::Dxcc]);
    let changed = populate(&mut fields, &protected, &result);
    assert_eq!(fields.primary_subdivision.as_deref(), Some("WA"));
    assert_eq!(fields.secondary_subdivision, None);
    assert_eq!(fields.itu_zone, None);
    assert_eq!(fields.dxcc, None);
    assert_eq!(changed, BTreeSet::from([Field::PrimarySubdivision]));
    fields.primary_subdivision = Some("my correction".into());
    let changed = populate(&mut fields, &BTreeSet::new(), &result);
    assert_eq!(changed, BTreeSet::from([Field::Dxcc]));
    assert_eq!(fields.primary_subdivision.as_deref(), Some("my correction"));
}

/// A native candidate for testing application rules rather than geometric predicates.
fn candidate<T>(value: T) -> Candidate<T> {
    Candidate {
        value,
        parent_scope: ParentScope::Global,
        display_label: "Authored candidate".into(),
        source_identities: vec![],
        derivation_keys: vec![],
    }
}

#[test]
fn position_change_expires_only_location_corrections() {
    let coordinates = Coordinates::from_microdegrees(500_000, 500_000).unwrap();
    let point = Position::ExplicitPoint {
        coordinates,
        grid_length: GridLength::new(6).unwrap(),
    };
    let mut fields = RecordedLocationFields {
        position: Some(point),
        dxcc: Some("291".into()),
        primary_subdivision: Some("WA".into()),
        secondary_subdivision: Some("WA,King".into()),
        cq_zone: Some(CqZone::new(3).unwrap()),
        itu_zone: Some(ItuZone::new(6).unwrap()),
    };
    let facts = fields.clone();
    let protection = BTreeSet::from([
        Field::Dxcc,
        Field::PrimarySubdivision,
        Field::SecondarySubdivision,
        Field::CqZone,
        Field::ItuZone,
    ]);
    let mut protected = protection.clone();
    assert!(!change_position(
        &mut fields,
        &mut protected,
        Some(Position::ExplicitPoint {
            coordinates,
            grid_length: GridLength::new(8).unwrap()
        })
    ));
    assert_eq!(fields.dxcc, facts.dxcc);
    assert_eq!(fields.primary_subdivision, facts.primary_subdivision);
    assert_eq!(fields.secondary_subdivision, facts.secondary_subdivision);
    assert_eq!(fields.cq_zone, facts.cq_zone);
    assert_eq!(fields.itu_zone, facts.itu_zone);
    assert_eq!(protected, protection);
    assert!(change_position(
        &mut fields,
        &mut protected,
        Some(Position::GridArea(Locator::parse("JJ00mm").unwrap()))
    ));
    assert_eq!(
        fields,
        RecordedLocationFields {
            position: Some(Position::GridArea(Locator::parse("JJ00mm").unwrap())),
            ..Default::default()
        }
    );
    assert!(protected.is_empty());
    fields.dxcc = Some("manual".into());
    protected.insert(Field::Dxcc);
    assert!(change_position(&mut fields, &mut protected, None));
    assert_eq!(fields.position, None);
    assert_eq!(fields.dxcc, None);
    assert!(protected.is_empty());
}

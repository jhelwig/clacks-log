mod support;

use clacks_model::catalog::{CatalogFinding, lookup_reference};
use clacks_model::effective::{
    EligibleStationFacts, InformationSource, StationFacts, directory_key_matches, select_location,
    select_name,
};
use clacks_model::ids::{GroupId, QsoId};
use clacks_model::location::RecordedLocation;
fn facts(state: &str) -> StationFacts {
    StationFacts {
        name: Some("Directory name".into()),
        location: Some(RecordedLocation {
            primary_subdivision: Some(state.into()),
            secondary_subdivision: Some("home county".into()),
            ..Default::default()
        }),
    }
}

#[test]
fn whole_location_never_patches_lower_source() {
    let mut station = support::qso(QsoId::from_u128(1)).contacted_station;
    station.location = Some(RecordedLocation {
        primary_subdivision: Some("WY".into()),
        ..Default::default()
    });
    let directory = facts("CT");
    let eligible = EligibleStationFacts {
        directory: Some(&directory),
        ..Default::default()
    };
    let value = select_location(&station, &eligible, &support::definitions()).unwrap();
    assert_eq!(value.source, InformationSource::Recorded);
    assert_eq!(value.value.unwrap().secondary_subdivision, None);
    assert_eq!(
        select_name(Some("On-air name"), &eligible).unwrap().value,
        "On-air name"
    );
    station.location = Some(RecordedLocation::default());
    assert_eq!(
        select_location(&station, &eligible, &support::definitions())
            .unwrap()
            .source,
        InformationSource::Directory
    );
}

#[test]
fn location_reference_blocks_home_location() {
    let mut station = support::qso(QsoId::from_u128(1)).contacted_station;
    station
        .references
        .insert(support::reference(GroupId::from_u128(1), Some("US-1234")));
    let home = facts("CT");
    let eligible = EligibleStationFacts {
        lotw: Some(&home),
        qrz: Some(&home),
        directory: Some(&home),
    };
    let selected = select_location(&station, &eligible, &support::definitions()).unwrap();
    assert_eq!(selected.source, InformationSource::Recorded);
    assert_eq!(selected.value, None);
    station.references.clear();
    station
        .references
        .insert(support::reference(GroupId::from_u128(999), Some("unknown")));
    assert_eq!(
        select_location(&station, &eligible, &support::definitions())
            .unwrap()
            .source,
        InformationSource::Lotw
    );
    let qr = EligibleStationFacts {
        lotw: None,
        ..eligible
    };
    assert_eq!(
        select_location(&station, &qr, &support::definitions())
            .unwrap()
            .source,
        InformationSource::Qrz
    );
}

#[test]
fn catalog_point_is_not_station_point() {
    let (q, association, defs, catalogs, _) = support::catalog_case();
    assert!(matches!(
        lookup_reference(&association, &defs, &catalogs),
        CatalogFinding::Found { .. }
    ));
    let chosen = select_location(
        &q.contacted_station,
        &EligibleStationFacts::default(),
        &defs,
    )
    .unwrap();
    assert_eq!(chosen.value, None);
    assert!(q.contacted_station.location.is_none());
}

#[test]
fn old_directory_key_cannot_supply_fallback() {
    let station = support::qso(QsoId::from_u128(1)).contacted_station;
    let old = facts("CT");
    for (current_key, expected) in [("K1ABC", true), ("W7HEX", false)] {
        let eligible = EligibleStationFacts {
            directory: directory_key_matches(current_key, "K1ABC").then_some(&old),
            ..Default::default()
        };
        assert_eq!(
            select_location(&station, &eligible, &support::definitions()).is_some(),
            expected
        );
        assert_eq!(select_name(None, &eligible).is_some(), expected);
    }
}

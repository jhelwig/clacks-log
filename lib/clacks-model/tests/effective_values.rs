use crate::support;

use clacks_model::effective::{
    EligibleStationFacts, InformationSource, StationFacts, select_location, select_name,
};
use clacks_model::ids::{GroupId, QsoId};
fn facts(state: &str) -> StationFacts {
    StationFacts {
        name: Some("Directory name".into()),
        location: Some(
            (clacks_model::location::RecordedLocationFields {
                primary_subdivision: Some(state.into()),
                secondary_subdivision: Some("home county".into()),
                ..Default::default()
            })
            .try_into()
            .expect("valid native fixture"),
        ),
    }
}

#[test]
fn whole_location_never_patches_lower_source() {
    let mut station = support::qso(support::id::<QsoId>(1))
        .contacted_station()
        .clone();
    station.location = Some(
        (clacks_model::location::RecordedLocationFields {
            primary_subdivision: Some("WY".into()),
            ..Default::default()
        })
        .try_into()
        .expect("valid native fixture"),
    );
    let directory = facts("CT");
    let eligible = EligibleStationFacts {
        directory: Some(&directory),
        ..Default::default()
    };
    let value = select_location(&station, &eligible, &support::definitions()).unwrap();
    assert_eq!(value.source, InformationSource::Recorded);
    assert_eq!(value.value.unwrap().secondary_subdivision(), &None);
    assert_eq!(
        select_name(Some("On-air name"), &eligible).unwrap().value,
        "On-air name"
    );
    station.location = None;
    assert_eq!(
        select_location(&station, &eligible, &support::definitions())
            .unwrap()
            .source,
        InformationSource::Directory
    );
}

#[test]
fn location_reference_blocks_home_location() {
    let mut station = support::qso(support::id::<QsoId>(1))
        .contacted_station()
        .clone();
    station.references.insert(support::reference(
        support::id::<GroupId>(1),
        Some("US-1234"),
    ));
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
    station.references.insert(support::reference(
        support::id::<GroupId>(999),
        Some("unknown"),
    ));
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
fn only_supplied_eligible_observations_provide_fallback() {
    let station = support::qso(support::id::<QsoId>(1))
        .contacted_station()
        .clone();
    let directory = facts("CT");
    for observation in [None, Some(&directory)] {
        let eligible = EligibleStationFacts {
            directory: observation,
            ..Default::default()
        };
        let expected = observation.is_some();
        assert_eq!(
            select_location(&station, &eligible, &support::definitions()).is_some(),
            expected
        );
        assert_eq!(select_name(None, &eligible).is_some(), expected);
    }
}

mod support;

use clacks_model::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn unrelated_edit_survives_old_recognition_round_trip() -> TestResult {
    let old = RecognitionCatalog::from_json(include_str!("fixtures/recognition-old.json"))?;
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/recorded-values.json"))?;
    let mut identities: Vec<ModeIdentity> = serde_json::from_value(fixture["modes"].clone())?;
    identities.push(ModeIdentity::Specialization {
        label: "FT4".into(),
        parent: Some("MFSK".into()),
    });
    for identity in identities {
        let mut q = support::qso(QsoId::from_u128(1));
        q.mode = Mode::from_recorded_identity(identity.clone(), RecognitionCatalog::adif_318())?;
        let mut json = serde_json::to_value(&q)?;
        json["primary"]["band"] = fixture["bands"][0].clone();
        let after: serde_json::Value = serde_json::from_str(&support::legacy_view_round_trip(
            &serde_json::to_string(&json)?,
            &old,
        )?)?;
        assert_eq!(after["mode"], json["mode"]);
        assert_eq!(after["primary"], json["primary"]);
        let received: Qso = serde_json::from_value(after)?;
        assert_eq!(received.mode.identity(), identity);
        assert_eq!(
            received.comment.as_deref(),
            Some("edited by older recognition view")
        );
        received.validate()?;
    }
    Ok(())
}

#[test]
fn source_mutation_cannot_rewrite_recorded_snapshots() -> TestResult {
    let mut q = support::qso(QsoId::from_u128(1));
    let mut station = q.own_station.clone();
    station
        .references
        .insert(support::reference(GroupId::from_u128(1), Some("US-1234")));
    let mut p = SavedLocationProfile {
        id: ProfileId::from_u128(1),
        name: "field".into(),
        station,
        default_equipment: None,
    };
    let c = CurrentOperatingValues {
        station: q.own_station.clone(),
        operator: None,
        transmit_power: None,
        equipment: None,
    };
    let loaded = load_profile(&c, &p, None)?;
    q.own_station = loaded.station.clone();
    let before = q.clone();
    p.station.references.clear();
    p.station.callsign = RecordedCallsign::new("W7HEX/P")?;
    assert_eq!(q, before);
    assert_eq!(loaded.station, q.own_station);
    assert_ne!(loaded.station, p.station);
    Ok(())
}

#[test]
fn empty_optional_aggregates_are_not_location_evidence() {
    let mut q = support::qso(QsoId::from_u128(1));
    q.contacted_station.location = Some(RecordedLocation::default());
    let fallback = StationFacts {
        name: None,
        location: Some(RecordedLocation {
            primary_subdivision: Some("CT".into()),
            ..Default::default()
        }),
    };
    let eligible = EligibleStationFacts {
        directory: Some(&fallback),
        ..Default::default()
    };
    assert_eq!(
        select_location(&q.contacted_station, &eligible, &support::definitions())
            .unwrap()
            .source,
        InformationSource::Directory
    );
    q.normalize_optional_aggregates();
    assert_eq!(q.contacted_station.location, None);
}

#[test]
fn whole_record_comparison_includes_disjoint_tag_edits() {
    let base = support::qso(QsoId::from_u128(1));
    let mut local = base.clone();
    let mut remote = base.clone();
    local.tags.insert(TagId::from_u128(1));
    remote.comment = Some("remote edit".into());
    assert!(!local.same_recorded_values(&base));
    assert!(!remote.same_recorded_values(&base));
    assert!(!local.same_recorded_values(&remote));
    let grid = Locator::parse("FN31PR").unwrap();
    let mut area = base.clone();
    let mut point = base;
    area.own_station.location = Some(RecordedLocation {
        position: Some(Position::GridArea(grid.clone())),
        ..Default::default()
    });
    point.own_station.location = Some(RecordedLocation {
        position: Some(Position::ExplicitPoint {
            coordinates: grid.center(),
            grid_length: grid.length(),
        }),
        ..Default::default()
    });
    assert!(!area.same_recorded_values(&point));
}

#[test]
fn invalid_deserialized_values_are_rejected_before_save() -> TestResult {
    let q = support::qso(QsoId::from_u128(1));
    let mut json = serde_json::to_value(&q)?;
    json["primary"] = serde_json::json!({"frequency":null,"band":null});
    assert!(serde_json::from_value::<Qso>(json)?.validate().is_err());
    let mut json = serde_json::to_value(&q)?;
    json["own_context"]["transmit_power"] = serde_json::json!(0);
    assert!(serde_json::from_value::<Qso>(json).is_err());
    let mut json = serde_json::to_value(&q)?;
    json["own_station"]["callsign"] = serde_json::json!(" ");
    assert!(serde_json::from_value::<Qso>(json).is_err());
    Ok(())
}

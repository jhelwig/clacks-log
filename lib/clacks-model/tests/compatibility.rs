mod support;

use clacks_model::contact::Qso;
use clacks_model::effective::{
    EligibleStationFacts, InformationSource, StationFacts, select_location,
};
use clacks_model::ids::QsoId;
use clacks_model::location::RecordedLocation;
use clacks_model::recognition::RecognitionCatalog;
use clacks_model::recognition::mode::{ModeIdentity, ModeInterpretation};
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn unrelated_edit_preserves_incoming_identities_with_old_recognition() -> TestResult {
    let old = RecognitionCatalog::from_json(include_str!("fixtures/recognition-old.json"))?;
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/recorded-values.json"))?;
    let mut identities: Vec<ModeIdentity> = serde_json::from_value(fixture["modes"].clone())?;
    identities.push(ModeIdentity::Specialization {
        label: "FT4".into(),
        parent: Some("MFSK".into()),
    });
    for identity in identities {
        let json = serde_json::json!({
            "id": "00000000000000000000000001",
            "start": 1000,
            "own_station": {"callsign": "W7HEX", "references": []},
            "contacted_station": {"callsign": "K1ABC", "references": []},
            "own_context": {},
            "primary": {"frequency": 14074000, "band": "future-band"},
            "mode": identity,
            "tags": []
        });
        let mut received: Qso = serde_json::from_value(json.clone())?;
        assert!(matches!(
            received.mode.interpret(&old),
            ModeInterpretation::Other(_)
        ));
        received.comment = Some("edited by older recognition view".into());
        let after = serde_json::to_value(&received)?;
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
fn empty_optional_aggregates_are_not_location_evidence() {
    let mut q = support::qso(support::id::<QsoId>(1));
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
fn invalid_deserialized_values_are_rejected_before_save() -> TestResult {
    let q = support::qso(support::id::<QsoId>(1));
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

mod support;

use clacks_model::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn band_and_frequency_authority_does_not_feed_back() -> TestResult {
    let c = RecognitionCatalog::adif_318();
    assert!(BandFrequency::new(None, None, c).is_err());
    let mut p = BandFrequency::new(
        Some(Hertz::new(14_074_000)?),
        Some(Band::Known(KnownBand::Meters40)),
        c,
    )?;
    assert_eq!(p.band().unwrap().label(), "20m");
    assert_eq!(p.frequency().unwrap().get(), 14_074_000);
    p.set_band(Band::Known(KnownBand::Meters40));
    assert!(p.frequency().is_none());
    p.set_frequency(Hertz::new(7_074_000)?, c);
    assert_eq!(p.band().unwrap().label(), "40m");
    assert_eq!(p.frequency().unwrap().get(), 7_074_000);
    p.set_frequency(Hertz::new(1)?, c);
    assert!(p.band().is_none());
    assert_eq!(p.frequency().unwrap().get(), 1);
    Ok(())
}

#[test]
fn a_present_receive_pair_is_a_whole_override() -> TestResult {
    let c = RecognitionCatalog::adif_318();
    let primary = BandFrequency::new(Some(Hertz::new(14_074_000)?), None, c)?;
    let receive = BandFrequency::new(None, Some(Band::Known(KnownBand::Meters20)), c)?;
    assert_eq!(primary.effective_receive(None), &primary);
    assert!(
        primary
            .effective_receive(Some(&receive))
            .frequency()
            .is_none()
    );
    let freq = BandFrequency::new(Some(Hertz::new(7_074_000)?), None, c)?;
    assert_eq!(
        primary
            .effective_receive(Some(&freq))
            .frequency()
            .unwrap()
            .get(),
        7_074_000
    );
    Ok(())
}

#[test]
fn validate_contact_times_and_required_station_identity() -> TestResult {
    let mut q = support::qso(QsoId::from_u128(1));
    q.validate()?;
    for end in [1000, 86401] {
        q.end = Some(UtcSecond::from_unix_seconds(end));
        q.validate()?;
    }
    q.end = Some(UtcSecond::from_unix_seconds(999));
    assert!(q.validate().is_err());
    let mut json = serde_json::to_value(support::qso(q.id))?;
    json["own_station"]
        .as_object_mut()
        .unwrap()
        .remove("callsign");
    assert!(serde_json::from_value::<Qso>(json).is_err());
    assert!(RecordedCallsign::new(" ").is_err());
    Ok(())
}

#[test]
fn whole_record_equality_uses_sets_and_preserved_values() -> TestResult {
    let mut a = support::qso(QsoId::from_u128(1));
    a.tags.extend([TagId::from_u128(2), TagId::from_u128(3)]);
    a.own_station
        .references
        .insert(support::reference(GroupId::from_u128(1), Some("US-1234")));
    let mut b = a.clone();
    b.tags.insert(TagId::from_u128(2));
    assert!(a.same_recorded_values(&b));
    b.comment = Some("changed".into());
    assert!(!a.same_recorded_values(&b));
    b = a.clone();
    b.id = QsoId::from_u128(2);
    assert!(!a.same_recorded_values(&b));
    a.mode = Mode::from_recorded_identity(
        ModeIdentity::Specialization {
            label: "Future".into(),
            parent: None,
        },
        RecognitionCatalog::adif_318(),
    )?;
    b = a.clone();
    b.comment = Some("unrelated".into());
    assert_eq!(
        serde_json::to_value(&a.mode)?,
        serde_json::to_value(&b.mode)?
    );
    a.contacted_operator = Some(OperatorInformation {
        callsign: None,
        recorded_name: Some("Alice".into()),
    });
    a.validate()?;
    assert!(a.contacted_operator.as_ref().unwrap().callsign.is_none());
    a.own_station.location = Some(RecordedLocation::default());
    a.own_context.operator = Some(OperatorInformation::default());
    a.normalize_optional_aggregates();
    assert!(a.own_station.location.is_none());
    assert!(a.own_context.operator.is_none());
    Ok(())
}

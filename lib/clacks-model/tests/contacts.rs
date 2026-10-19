mod support;

use clacks_model::contact::{BandFrequency, OperatorInformation, Qso};
use clacks_model::ids::{GroupId, QsoId, TagId};
use clacks_model::location::RecordedLocation;
use clacks_model::recognition::RecognitionCatalog;
use clacks_model::recognition::band::Band;
use clacks_model::recognition::mode::{Mode, ModeIdentity};
use clacks_model::scalar::{Hertz, RecordedCallsign, UtcSecond};
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn band_and_frequency_authority_does_not_feed_back() -> TestResult {
    let c = RecognitionCatalog::adif_318();
    assert!(BandFrequency::new(None, None, c).is_err());
    let mut p = BandFrequency::new(
        Some(Hertz::new(14_074_000)?),
        Some(Band::from_recorded_label("40m").unwrap()),
        c,
    )?;
    assert_eq!(p.band().unwrap().label(), "20m");
    assert_eq!(p.frequency().unwrap().get(), 14_074_000);
    p.set_band(Band::from_recorded_label("40m").unwrap());
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
    let receive = BandFrequency::new(None, Some(Band::from_recorded_label("20m").unwrap()), c)?;
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
    let mut q = support::qso(support::id::<QsoId>(1));
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
    let mut a = support::qso(support::id::<QsoId>(1));
    a.tags
        .extend([support::id::<TagId>(2), support::id::<TagId>(3)]);
    a.own_station.references.insert(support::reference(
        support::id::<GroupId>(1),
        Some("US-1234"),
    ));
    let mut b = a.clone();
    b.tags.insert(support::id::<TagId>(2));
    assert_eq!(a, b);
    b.comment = Some("changed".into());
    assert_ne!(a, b);
    b = a.clone();
    b.id = support::id::<QsoId>(2);
    assert_ne!(a, b);
    a.mode = Mode::from_recorded_identity(ModeIdentity::Specialization {
        label: "Future".into(),
        parent: None,
    })?;
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

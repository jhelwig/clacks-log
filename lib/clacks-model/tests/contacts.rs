use crate::support;

use clacks_model::contact::{BandFrequency, Qso};
use clacks_model::ids::QsoId;
use clacks_model::recognition::RecognitionCatalog;
use clacks_model::recognition::band::Band;
use clacks_model::scalar::{Hertz, RecordedCallsign, UtcSecond};
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn band_and_frequency_authority_does_not_feed_back() -> TestResult {
    let c = RecognitionCatalog::adif_318()?;
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
    let c = RecognitionCatalog::adif_318()?;
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

    for end in [1000, 86401] {
        support::edit(&mut q, |q| {
            q.end = Some(UtcSecond::from_unix_seconds(end));

            Ok(())
        });
    }
    let mut fields = q.clone().into_fields();
    fields.end = Some(UtcSecond::from_unix_seconds(999));
    assert!(Qso::try_from(fields).is_err());
    let mut json = serde_json::to_value(support::qso(q.id()))?;
    json["own_station"]
        .as_object_mut()
        .unwrap()
        .remove("callsign");
    assert!(serde_json::from_value::<Qso>(json).is_err());
    assert!(RecordedCallsign::new(" ").is_err());
    Ok(())
}

#[test]
fn recorded_operator_names_do_not_require_operator_callsigns() -> TestResult {
    let mut contact = support::qso(support::id::<QsoId>(1));
    support::edit(&mut contact, |contact| {
        contact.own_context.operator = Some(
            (clacks_model::contact::OperatorInformationFields {
                callsign: None,
                recorded_name: Some("Our operator".into()),
            })
            .try_into()
            .expect("valid native fixture"),
        );

        Ok(())
    });
    support::edit(&mut contact, |contact| {
        contact.contacted_operator = Some(
            (clacks_model::contact::OperatorInformationFields {
                callsign: None,
                recorded_name: Some("Alice".into()),
            })
            .try_into()
            .expect("valid native fixture"),
        );

        Ok(())
    });

    Ok(())
}

#[test]
fn restored_contacts_reject_invalid_time_and_empty_radio_pair() -> TestResult {
    let contact = support::qso(support::id::<QsoId>(1));
    let mut encoded = serde_json::to_value(&contact)?;
    encoded["end"] = serde_json::json!(999);
    assert!(serde_json::from_value::<Qso>(encoded).is_err());

    let mut fields = contact.into_fields();
    fields.comment = Some("   ".into());
    let error = Qso::try_from(fields).unwrap_err();
    assert!(matches!(
        error.downcast_ref::<clacks_model::contact::RecordError>(),
        Some(clacks_model::contact::RecordError::InvalidOptionalText { .. })
    ));

    let empty = serde_json::json!({ "frequency": null, "band": null });
    assert!(serde_json::from_value::<BandFrequency>(empty).is_err());
    Ok(())
}

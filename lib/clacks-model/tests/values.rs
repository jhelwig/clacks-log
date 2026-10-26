mod support;

use clacks_model::ids::QsoId;
use clacks_model::scalar::{
    Hertz, Milliwatts, RecordedCallsign, UtcSecond, UtcTimestamp, optional_text,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn callsign_and_text_preserve_recorded_meaning() -> TestResult {
    assert_eq!(RecordedCallsign::new(" ok2cqr/p ")?.as_str(), "OK2CQR/P");
    assert!(RecordedCallsign::new("  ").is_err());
    assert_eq!(optional_text("  Zoë  "), Some("  Zoë  ".to_owned()));
    assert_eq!(optional_text(" \t\n"), None);
    assert!(serde_json::from_str::<RecordedCallsign>("\" \" ").is_err());
    Ok(())
}

#[test]
fn quantities_and_operational_time_are_validated() -> TestResult {
    assert_eq!(Milliwatts::new(1)?.get(), 1);
    assert!(Hertz::new(0).is_err());
    assert!(Milliwatts::new(0).is_err());
    assert!(UtcTimestamp::new(42, 1_000_000_000).is_err());
    assert!(serde_json::from_str::<Hertz>("0").is_err());
    assert!(serde_json::from_str::<Milliwatts>("0").is_err());
    assert_eq!(UtcSecond::from_unix_seconds(-1).unix_seconds(), -1);
    let t = UtcTimestamp::new(-1, 999_999_999)?;
    assert_eq!(
        serde_json::to_value(t)?,
        serde_json::json!({"seconds": -1, "nanoseconds": 999_999_999})
    );
    let decoded: UtcTimestamp = serde_json::from_str(r#"{"seconds":-1,"nanoseconds":999999999}"#)?;
    assert_eq!(decoded.unix_seconds(), -1);
    assert_eq!(decoded.subsecond_nanoseconds(), 999_999_999);
    assert!(
        serde_json::from_str::<UtcTimestamp>(r#"{"seconds":42,"nanoseconds":1000000000}"#).is_err()
    );
    Ok(())
}

#[test]
fn native_ids_use_string_values_in_persisted_records() -> TestResult {
    let encoded = serde_json::json!("00000000000000000000000001");
    let contact = support::qso(support::id::<QsoId>(2));
    let mut incoming = serde_json::to_value(&contact)?;
    incoming["id"] = encoded.clone();

    let restored: clacks_model::contact::Qso = serde_json::from_value(incoming)?;
    assert_eq!(restored.id.to_string(), "00000000000000000000000001");
    assert_eq!(serde_json::to_value(&restored)?["id"], encoded);

    assert_eq!(
        serde_json::to_value(&contact)?["id"],
        serde_json::json!("00000000000000000000000002")
    );
    Ok(())
}

#[test]
fn service_record_ids_display_their_native_identifier() {
    use clacks_model::service::evidence::ServiceRecordId;
    let expected = support::id::<QsoId>(7).to_string();
    for id in [
        ServiceRecordId::Download(support::id(7)),
        ServiceRecordId::Acquisition(support::id(7)),
        ServiceRecordId::Destination(support::id(7)),
        ServiceRecordId::Assertion(support::id(7)),
        ServiceRecordId::Attempt(support::id(7)),
        ServiceRecordId::Result(support::id(7)),
        ServiceRecordId::Observation(support::id(7)),
        ServiceRecordId::Association(support::id(7)),
        ServiceRecordId::Disposition(support::id(7)),
        ServiceRecordId::Suppression(support::id(7)),
    ] {
        assert_eq!(id.to_string(), expected);
    }
}

#[test]
fn quantities_fit_sqlite_integer() -> TestResult {
    for value in [1, 9_223_372_036_854_775_807] {
        assert_eq!(Hertz::new(value)?.get(), value);
        assert_eq!(Milliwatts::new(value)?.get(), value);
    }
    for value in [0, 9_223_372_036_854_775_808, u64::MAX] {
        assert!(Hertz::new(value).is_err(), "accepted frequency {value}");
        assert!(Milliwatts::new(value).is_err(), "accepted power {value}");
        assert!(serde_json::from_str::<Hertz>(&value.to_string()).is_err());
        assert!(serde_json::from_str::<Milliwatts>(&value.to_string()).is_err());
    }
    Ok(())
}

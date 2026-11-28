use clacks_model::{
    position::Coordinates,
    recognition::{
        RecognitionCatalog,
        mode::{Mode, ModeIdentity},
    },
    scalar::{Hertz, Milliwatts, UtcSecond},
    service::adif,
};

#[test]
fn exact_quantities_and_calendar_limits() -> anyhow::Result<()> {
    assert_eq!(adif::frequency(Hertz::new(14_074_001)?), "14.074001");
    assert_eq!(adif::power(Milliwatts::new(1_001)?), "1.001");
    assert!(adif::contact_time(UtcSecond::from_unix_seconds(i64::MAX)).is_err());
    assert_eq!(
        adif::contact_time(UtcSecond::from_unix_seconds(0))?,
        ("19700101".into(), "000000".into())
    );
    assert_eq!(
        adif::contact_time(UtcSecond::from_unix_seconds(-1))?,
        ("19691231".into(), "235959".into())
    );
    assert_eq!(
        adif::contact_time(UtcSecond::from_unix_seconds(253_402_300_799))?,
        ("99991231".into(), "235959".into())
    );
    Ok(())
}

#[test]
fn coordinates_round_at_output_precision() -> anyhow::Result<()> {
    assert_eq!(
        adif::coordinates(Coordinates::from_microdegrees(-1, 180_000_000)?),
        ("S000 00.000".into(), "E180 00.000".into())
    );
    assert_eq!(
        adif::coordinates(Coordinates::from_microdegrees(12_999_999, 0)?).0,
        "N013 00.000"
    );
    assert_eq!(
        adif::coordinates(Coordinates::from_microdegrees(1, 0)?),
        adif::coordinates(Coordinates::from_microdegrees(2, 0)?)
    );
    Ok(())
}

#[test]
fn mode_specialization_and_text_loss_are_explicit() -> anyhow::Result<()> {
    let m = Mode::from_recorded_identity(ModeIdentity::Specialization {
        label: "FT4".into(),
        parent: Some("MFSK".into()),
    })?;
    let fields = adif::mode(&m, RecognitionCatalog::adif_318()?);
    assert_eq!(fields.get("MODE").map(String::as_str), Some("MFSK"));
    assert_eq!(fields.get("SUBMODE").map(String::as_str), Some("FT4"));
    let converted = adif::text("NAME", "étude");
    assert_eq!(converted.value, Some("etude".into()));
    assert!(!converted.warnings.is_empty());
    assert_eq!(adif::text("NAME", "\u{e000}").value, None);
    Ok(())
}

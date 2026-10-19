use clacks_model::error::ValueError;
use clacks_model::recognition::RecognitionCatalog;
use clacks_model::recognition::band::Band;
use clacks_model::recognition::mode::{Mode, ModeIdentity, ModeInterpretation};
use clacks_model::scalar::Hertz;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn older_recognition_cannot_rewrite_a_specialization() -> TestResult {
    let id = ModeIdentity::Specialization {
        label: "FT4".into(),
        parent: Some("MFSK".into()),
    };
    let full = RecognitionCatalog::adif_318();
    let old = RecognitionCatalog::from_json(include_str!("fixtures/recognition-old.json"))?;
    let a = Mode::from_recorded_identity(id.clone())?;
    let b: Mode = serde_json::from_value(serde_json::to_value(&a)?)?;
    assert!(matches!(a.interpret(&old), ModeInterpretation::Other(_)));
    assert!(matches!(b.interpret(full), ModeInterpretation::Known(_)));
    assert_eq!(a, b);
    assert_eq!(serde_json::to_value(&a)?, serde_json::to_value(&b)?);
    assert_eq!(a.identity(), id);
    assert!(matches!(a.interpret(full), ModeInterpretation::Known(_)));
    Ok(())
}

#[test]
fn every_source_row_is_recognized_from_data() -> TestResult {
    let full = RecognitionCatalog::adif_318();
    assert_eq!(full.bands.len(), 33);
    assert_eq!(full.modes.len(), 91);
    assert_eq!(full.submodes.len(), 198);
    for row in &full.modes {
        let mode = Mode::from_recorded_identity(ModeIdentity::Parent {
            label: row.code.clone(),
        })?;
        match mode.interpret(full) {
            ModeInterpretation::Known(recognized) => {
                assert_eq!(recognized.mode, row);
                assert!(recognized.submode.is_none());
            }
            other => panic!("{}: {other:?}", row.code),
        }
        assert_eq!(
            mode.identity(),
            ModeIdentity::Parent {
                label: row.code.clone()
            }
        );
    }
    for row in &full.submodes {
        let id = ModeIdentity::Specialization {
            label: row.code.clone(),
            parent: Some(row.parent.clone()),
        };
        let mode = Mode::from_recorded_identity(id.clone())?;
        match mode.interpret(full) {
            ModeInterpretation::Known(recognized) => {
                assert_eq!(recognized.mode.code, row.parent);
                assert_eq!(recognized.submode, Some(row));
            }
            other => panic!("{}: {other:?}", row.code),
        }
        assert_eq!(mode.identity(), id);
    }
    for row in &full.bands {
        assert_eq!(
            Band::from_recorded_label(&row.code)?.interpret(full),
            Some(row)
        );
        assert_eq!(
            Band::for_frequency(Hertz::new(row.lower_hz)?, full)
                .unwrap()
                .label(),
            row.code
        );
        assert_eq!(
            Band::for_frequency(Hertz::new(row.upper_hz.unwrap())?, full)
                .unwrap()
                .label(),
            row.code
        );
    }
    Ok(())
}

#[test]
fn unfamiliar_context_and_deprecated_labels_survive() -> TestResult {
    let full = RecognitionCatalog::adif_318();
    for id in [
        ModeIdentity::Parent {
            label: "MFSK".into(),
        },
        ModeIdentity::Specialization {
            label: "unfamiliar".into(),
            parent: None,
        },
        ModeIdentity::Specialization {
            label: "FT4".into(),
            parent: Some("unfamiliar parent".into()),
        },
        ModeIdentity::Parent {
            label: "PSK31".into(),
        },
    ] {
        let m = Mode::from_recorded_identity(id.clone())?;
        assert_eq!(
            serde_json::from_str::<Mode>(&serde_json::to_string(&m)?)?.identity(),
            id
        );
    }
    let b = Band::from_recorded_label("20M")?;
    assert_eq!(
        serde_json::from_str::<Band>(&serde_json::to_string(&b)?)?.label(),
        "20M"
    );
    assert_eq!(
        Band::for_frequency(Hertz::new(14_074_000)?, full)
            .unwrap()
            .label(),
        "20m"
    );
    assert!(Band::for_frequency(Hertz::new(1)?, full).is_none());
    assert!(Mode::from_recorded_identity(ModeIdentity::Parent { label: " ".into() }).is_err());
    let mut open = full.clone();
    open.bands.last_mut().unwrap().upper_hz = None;
    assert_eq!(
        Band::for_frequency(Hertz::new(u64::MAX)?, &open)
            .unwrap()
            .label(),
        "submm"
    );
    assert!(RecognitionCatalog::from_json("{}").is_err());
    Ok(())
}

#[test]
fn supplied_catalog_rows_extend_recognition_without_code_changes() -> TestResult {
    let base = RecognitionCatalog::adif_318();
    let mut data = serde_json::to_value(base)?;
    data["modes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"code":"EXPERIMENTAL-FAMILY","deprecated":false}));
    data["submodes"].as_array_mut().unwrap().push(serde_json::json!({"code":"EXPERIMENTAL-SUBMODE","parent":"EXPERIMENTAL-FAMILY","deprecated":false}));
    data["bands"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"code":"experimental-band","lower_hz":100,"upper_hz":200}));
    let extended = RecognitionCatalog::from_json(&serde_json::to_string(&data)?)?;
    for identity in [
        ModeIdentity::Parent {
            label: "EXPERIMENTAL-FAMILY".into(),
        },
        ModeIdentity::Specialization {
            label: "EXPERIMENTAL-SUBMODE".into(),
            parent: Some("supplied context".into()),
        },
    ] {
        let value = Mode::from_recorded_identity(identity.clone())?;
        let before = serde_json::to_value(&value)?;
        match value.interpret(&extended) {
            ModeInterpretation::Known(recognized) => {
                assert_eq!(recognized.mode.code, "EXPERIMENTAL-FAMILY");
                match &identity {
                    ModeIdentity::Parent { .. } => assert!(recognized.submode.is_none()),
                    ModeIdentity::Specialization { .. } => {
                        assert_eq!(recognized.submode.unwrap().code, "EXPERIMENTAL-SUBMODE")
                    }
                }
            }
            other => panic!("new catalog row not recognized: {other:?}"),
        }
        assert!(matches!(
            value.interpret(base),
            ModeInterpretation::Other(_)
        ));
        assert_eq!(value.identity(), identity);
        assert_eq!(serde_json::to_value(&value)?, before);
    }
    let band = Band::from_recorded_label("experimental-band")?;
    assert!(
        band.interpret(&extended).is_some(),
        "new band catalog row not recognized"
    );
    assert!(band.interpret(base).is_none());
    assert_eq!(
        Band::for_frequency(Hertz::new(150)?, &extended)
            .unwrap()
            .label(),
        "experimental-band"
    );
    assert_eq!(
        serde_json::to_value(&band)?,
        serde_json::json!("experimental-band")
    );
    Ok(())
}

#[test]
fn supplied_band_rows_are_not_limited_to_compiled_labels() -> TestResult {
    let base = RecognitionCatalog::adif_318();
    let mut data = serde_json::to_value(base)?;
    data["bands"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"code":"experimental-band","lower_hz":100,"upper_hz":200}));
    let extended = RecognitionCatalog::from_json(&serde_json::to_string(&data)?)?;
    let band = Band::from_recorded_label("experimental-band")?;
    assert!(band.interpret(&extended).is_some());
    Ok(())
}

#[test]
fn catalog_rejects_labels_that_collide_during_recognition() -> TestResult {
    let valid = serde_json::json!({
        "release": "case-collision-regression",
        "bands": [{"code": "X", "lower_hz": 100, "upper_hz": 200}],
        "modes": [{"code": "A", "deprecated": false}, {"code": "B", "deprecated": false}],
        "submodes": [{"code": "X", "parent": "A", "deprecated": false}]
    });
    for (table, conflicting_row) in [
        (
            "bands",
            serde_json::json!({"code": "x", "lower_hz": 300, "upper_hz": 400}),
        ),
        (
            "modes",
            serde_json::json!({"code": "a", "deprecated": true}),
        ),
        (
            "submodes",
            serde_json::json!({"code": "x", "parent": "B", "deprecated": true}),
        ),
    ] {
        let mut data = valid.clone();
        data[table].as_array_mut().unwrap().push(conflicting_row);
        for reverse in [false, true] {
            if reverse {
                data[table].as_array_mut().unwrap().reverse();
            }
            assert!(
                matches!(
                    RecognitionCatalog::from_json(&serde_json::to_string(&data)?),
                    Err(ValueError::InvalidRecognitionCatalog { .. })
                ),
                "{table} accepted ambiguous labels (reversed: {reverse})"
            );
        }
    }
    Ok(())
}

use clacks_model::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn older_recognition_cannot_rewrite_a_specialization() -> TestResult {
    let id = ModeIdentity::Specialization {
        label: "FT4".into(),
        parent: Some("MFSK".into()),
    };
    let full = RecognitionCatalog::adif_318();
    let old = RecognitionCatalog::from_json(include_str!("fixtures/recognition-old.json"))?;
    let a = Mode::from_recorded_identity(id.clone(), &old)?;
    let b = Mode::from_recorded_identity(id.clone(), full)?;
    assert!(matches!(a, Mode::Other(_)));
    assert!(matches!(
        b,
        Mode::Known(KnownMode::Mfsk(Some(MfskSubmode::Ft4)))
    ));
    assert_eq!(a, b);
    assert_eq!(serde_json::to_value(&a)?, serde_json::to_value(&b)?);
    assert_eq!(a.identity(), id);
    assert!(matches!(a.interpret(full), ModeInterpretation::Known(_)));
    Ok(())
}

#[test]
fn every_source_row_has_typed_coverage() -> TestResult {
    let full = RecognitionCatalog::adif_318();
    assert_eq!(full.bands.len(), 33);
    assert_eq!(full.modes.len(), 91);
    assert!(full.submodes.len() >= 188);
    for row in &full.modes {
        let mode = Mode::from_recorded_identity(
            ModeIdentity::Parent {
                label: row.code.clone(),
            },
            full,
        )?;
        assert!(matches!(mode, Mode::Known(_)), "{}", row.code);
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
        let mode = Mode::from_recorded_identity(id.clone(), full)?;
        assert!(matches!(mode, Mode::Known(_)), "{}", row.code);
        assert_eq!(mode.identity(), id);
    }
    for row in &full.bands {
        assert!(matches!(
            Band::from_recorded_label(&row.code, full)?,
            Band::Known(_)
        ));
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
        let m = Mode::from_recorded_identity(id.clone(), full)?;
        assert_eq!(
            serde_json::from_str::<Mode>(&serde_json::to_string(&m)?)?.identity(),
            id
        );
    }
    let b = Band::from_recorded_label("20M", full)?;
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
    assert!(
        Mode::from_recorded_identity(ModeIdentity::Parent { label: " ".into() }, full).is_err()
    );
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

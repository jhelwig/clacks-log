use clacks_model::location::{CqZone, ItuZone, RecordedLocation};
use clacks_model::position::{Coordinates, GridLength, Locator, LookupGeometry, Position};
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn preserve_area_point_and_supplied_length() -> TestResult {
    for text in [
        "FN",
        "FN31",
        "FN31PR",
        "FN31PR12",
        "FN31PR12AB",
        "FN31PR12AB34",
    ] {
        let grid = Locator::parse(&text.to_lowercase())?;
        assert_eq!(grid.to_string(), text);
        assert_eq!(grid.center().locator(grid.length()), grid);
        let area = Position::GridArea(grid.clone());
        let point = Position::ExplicitPoint {
            coordinates: grid.center(),
            grid_length: grid.length(),
        };
        assert_ne!(area, point);
        assert!(matches!(area.lookup_geometry(), LookupGeometry::Area(_)));
        assert!(matches!(point.lookup_geometry(), LookupGeometry::Point(_)));
    }
    let c = Locator::parse("FN31PR")?.center();
    assert_eq!(c.latitude_microdegrees(), 41_729_167);
    assert_eq!(c.longitude_microdegrees(), -72_708_333);
    assert!(
        Position::GridArea(Locator::parse("FN")?)
            .validate_operating_entry()
            .is_err()
    );
    assert!(
        Position::GridArea(Locator::parse("FN31")?)
            .validate_operating_entry()
            .is_ok()
    );
    Ok(())
}

#[test]
fn reject_before_rounding_and_round_half_away_from_zero() -> TestResult {
    for v in [
        "90.0000001",
        "-90.0000000000000001",
        "9999999999999999999999999",
    ] {
        assert!(Coordinates::from_decimal_degrees(v, "0").is_err());
    }
    for (v, expected) in [
        ("-0.0000005", -1),
        ("0.0000005", 1),
        ("-0", 0),
        ("+.5", 500_000),
        ("1.23456749999999", 1_234_567),
        ("1.2345675", 1_234_568),
    ] {
        assert_eq!(
            Coordinates::from_decimal_degrees(v, "0")?.latitude_microdegrees(),
            expected
        );
    }
    for v in ["NaN", "1e-2", ".", "--1", "1.2.3", "1x"] {
        assert!(Coordinates::from_decimal_degrees(v, "0").is_err());
    }
    Ok(())
}

#[test]
fn extrema_and_invalid_cells() -> TestResult {
    for (lat, lon, cell) in [
        (90_000_000, 180_000_000, "AR09AX09AX09"),
        (-90_000_000, -180_000_000, "AA00AA00AA00"),
    ] {
        let c = Coordinates::from_microdegrees(lat, lon)?;
        assert_eq!(c.locator(GridLength::new(12)?).to_string(), cell);
        assert_eq!(c.latitude_microdegrees(), lat);
        assert_eq!(c.longitude_microdegrees(), lon);
    }
    for text in [
        "",
        "F",
        "SN",
        "FN3A",
        "FN31YY",
        "FN31PR1A",
        "FN31PR12YY",
        "FN31PR12AB1234",
        "éé",
    ] {
        assert!(Locator::parse(text).is_err(), "{text}");
    }
    assert!(Coordinates::from_microdegrees(90_000_001, 0).is_err());
    assert!(GridLength::new(3).is_err());
    assert!(CqZone::new(41).is_err());
    assert!(ItuZone::new(0).is_err());
    assert!(serde_json::from_str::<Locator>("\"SN\"").is_err());
    assert!(serde_json::from_str::<Coordinates>(r#"{"latitude":90000001,"longitude":0}"#).is_err());
    Ok(())
}

#[test]
fn display_precision_does_not_change_point_identity() -> TestResult {
    let coordinates = Coordinates::from_microdegrees(41_123_456, -72_123_456)?;
    let a = Position::ExplicitPoint {
        coordinates,
        grid_length: GridLength::new(4)?,
    };
    let b = Position::ExplicitPoint {
        coordinates,
        grid_length: GridLength::new(12)?,
    };
    assert!(a.same_geographic_position(&b));
    for _ in 0..100 {
        assert_eq!(b.display_coordinates(), coordinates);
        assert_eq!(b.display_locator().length(), GridLength::new(12)?);
    }
    assert!(!a.same_geographic_position(&Position::GridArea(a.display_locator())));
    assert!(!RecordedLocation::default().has_values());
    Ok(())
}

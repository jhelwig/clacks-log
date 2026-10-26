//! Fixed coordinate controls for the selected datum operations and input units.

use super::*;

#[test]
fn selected_translations_accept_degrees_and_return_wgs84_coordinates() {
    // NAD83/GRS80 to WGS84, zero input height, with the recipe's explicit
    // mainland, Alaska and Hawaii translation parameters. Controls are fixed
    // evaluations of those operations, not source-boundary accuracy claims.
    for (point, delta, expected) in [
        (
            Coord {
                x: -122.33,
                y: 47.60,
            },
            [0., 0., 0.],
            Coord {
                x: -122.33,
                y: 47.599_999_999_060_79,
            },
        ),
        (
            Coord {
                x: -166.53,
                y: 53.87,
            },
            [-2., 0., 4.],
            Coord {
                x: -166.530_007_082_285_9,
                y: 53.870_007_074_855_586,
            },
        ),
        (
            Coord {
                x: -157.86,
                y: 21.31,
            },
            [1., 1., -1.],
            Coord {
                x: -157.860_005_295_157_32,
                y: 21.309_995_862_388_51,
            },
        ),
    ] {
        let actual = Projection::translation(delta)
            .unwrap()
            .apply(point)
            .unwrap();
        assert!((actual.x - expected.x).abs() < 1e-9, "longitude {actual:?}");
        assert!((actual.y - expected.y).abs() < 1e-9, "latitude {actual:?}");
    }
}

#[test]
fn missing_corrupt_grids_and_invalid_points_fail_without_fallback() {
    assert!(Projection::grid(b"invalid", &"0".repeat(64)).is_err());
    let projection = Projection::translation([-2., 0., 4.]).unwrap();
    assert!(projection.apply(Coord { x: 0., y: 91. }).is_err());
    assert!(projection.apply(Coord { x: f64::NAN, y: 0. }).is_err());
}

#[test]
fn verified_caribbean_grid_uses_longitude_latitude_and_rejects_outside_points() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/us_noaa_pvhpgn.tif"
    ));
    let hash = "a6db010f956526a011ba719c45fe32e5aff987eeb1cb03f2b8338208a77c619d";
    let projection = Projection::grid(bytes, hash).unwrap();

    // Fixed controls from the pinned NOAA grid's latitude/longitude offsets,
    // in arc-seconds. Cover an interior grid node, an interpolated point and
    // both opposite corners without implementing TIFF decoding/interpolation.
    for (point, expected) in [
        (
            Coord { x: -66., y: 18.5 },
            Coord {
                x: -66.000_002_126_388_85,
                y: 18.500_008_755_833_324,
            },
        ),
        (
            Coord {
                x: -66.10,
                y: 18.46,
            },
            Coord {
                x: -66.100_002_868_842_21,
                y: 18.460_008_699_522_266,
            },
        ),
        (
            Coord { x: -68., y: 20.5 },
            Coord {
                x: -68.000_002_135_555_52,
                y: 20.500_013_506_667_067,
            },
        ),
        (
            Coord { x: -62., y: 17. },
            Coord {
                x: -62.000_006_247_778_01,
                y: 16.999_998_805_277_8,
            },
        ),
    ] {
        let actual = projection.apply(point).unwrap();
        assert!((actual.x - expected.x).abs() < 1e-9, "longitude {actual:?}");
        assert!((actual.y - expected.y).abs() < 1e-9, "latitude {actual:?}");
    }
    assert!(projection.apply(Coord { x: 0., y: 0. }).is_err());

    let mut corrupt = bytes.to_vec();
    if let Some(first) = corrupt.first_mut() {
        *first = 0;
    }
    assert!(Projection::grid(&corrupt, hash).is_err());
}

#[test]
fn segment_subdivision_is_direction_independent_and_bounded() {
    let projection = Projection::translation([-2., 0., 4.]).unwrap();
    let a = Coord { x: -170., y: 52. };
    let b = Coord { x: -120., y: 75. };
    let forward = projection.segment(a, b, 1_000_000).unwrap();
    let mut reversed = projection.segment(b, a, 1_000_000).unwrap();
    reversed.reverse();
    assert_eq!(forward, reversed);
    assert!(forward.len() > 2);
    assert!(projection.segment(a, b, 2).is_err());
    assert_eq!(forward.first(), Some(&projection.apply(a).unwrap()));
    assert_eq!(forward.last(), Some(&projection.apply(b).unwrap()));
}

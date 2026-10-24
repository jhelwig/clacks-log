//! Independent numerical and coordinate-format acceptance cases.

#![allow(clippy::indexing_slicing)]

use super::*;

/// Evaluate the published geocentric translation directly using ellipsoid equations.
fn translated(point: Coord<f64>, delta: [f64; 3]) -> Coord<f64> {
    let [dx, dy, dz] = delta;
    let (lambda, phi) = (point.x.to_radians(), point.y.to_radians());
    let a = 6_378_137.;
    let f = 1. / 298.257222101;
    let e2 = f * (2. - f);
    let n = a / (1. - e2 * phi.sin().powi(2)).sqrt();
    let x = n * phi.cos() * lambda.cos() + dx;
    let y = n * phi.cos() * lambda.sin() + dy;
    let z = n * (1. - e2) * phi.sin() + dz;
    let target_f = 1. / 298.257223563;
    let target_e2 = target_f * (2. - target_f);
    let p = x.hypot(y);
    let mut latitude = z.atan2(p * (1. - target_e2));
    for _ in 0..16 {
        let radius = a / (1. - target_e2 * latitude.sin().powi(2)).sqrt();
        latitude = (z + target_e2 * radius * latitude.sin()).atan2(p);
    }
    Coord {
        x: y.atan2(x).to_degrees(),
        y: latitude.to_degrees(),
    }
}

#[test]
fn geocentric_operations_match_independent_equations_in_degrees() {
    for (point, delta) in [
        (
            Coord {
                x: -122.33,
                y: 47.60,
            },
            [0., 0., 0.],
        ),
        (
            Coord {
                x: -166.53,
                y: 53.87,
            },
            [-2., 0., 4.],
        ),
        (
            Coord {
                x: -157.86,
                y: 21.31,
            },
            [1., 1., -1.],
        ),
    ] {
        let projection = Projection::translation(delta).unwrap();
        let actual = projection.apply(point).unwrap();
        let expected = translated(point, delta);
        assert!(
            (actual.x - expected.x).abs() < 1e-9,
            "longitude {actual:?} != {expected:?}"
        );
        assert!(
            (actual.y - expected.y).abs() < 1e-9,
            "latitude {actual:?} != {expected:?}"
        );
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
fn published_grid_is_loaded_only_from_verified_bytes() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/us_noaa_pvhpgn.tif"
    ));
    let hash = "a6db010f956526a011ba719c45fe32e5aff987eeb1cb03f2b8338208a77c619d";
    let projection = Projection::grid(bytes, hash).unwrap();
    let actual = projection
        .apply(Coord {
            x: -66.10,
            y: 18.46,
        })
        .unwrap();
    assert!(actual.x.is_finite() && actual.y.is_finite());
    assert!(projection.apply(Coord { x: 0., y: 0. }).is_err());
    let again = Projection::grid(bytes, hash).unwrap();
    assert_eq!(
        again
            .apply(Coord {
                x: -66.10,
                y: 18.46
            })
            .unwrap(),
        actual
    );
    let mut corrupt = bytes.to_vec();
    corrupt[0] = 0;
    assert!(Projection::grid(&corrupt, hash).is_err());
}

#[test]
fn grid_samples_and_bilinear_values_match_published_metadata() {
    use std::io::Cursor;
    use tiff::{
        decoder::{Decoder, DecodingResult},
        tags::Tag,
    };
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/us_noaa_pvhpgn.tif"
    ));
    let mut decoder = Decoder::new(Cursor::new(bytes)).unwrap();
    let (width, height) = decoder.dimensions().unwrap();
    let scale = decoder.get_tag_f64_vec(Tag::ModelPixelScaleTag).unwrap();
    let tie = decoder.get_tag_f64_vec(Tag::ModelTiepointTag).unwrap();
    let keys = decoder.get_tag_u16_vec(Tag::GeoKeyDirectoryTag).unwrap();
    let metadata = decoder.get_tag_ascii_string(Tag::Unknown(42112)).unwrap();
    assert_eq!((width, height), (25, 15));
    assert_eq!(scale, vec![0.25, 0.25, 0.]);
    assert_eq!(tie, vec![0., 0., 0., -68., 20.5, 0.]);
    assert!(keys.as_chunks::<4>().0.contains(&[1025, 0, 1, 2]));
    assert!(
        metadata.contains("latitude_offset")
            && metadata.contains("longitude_offset")
            && metadata.contains("east")
            && metadata.matches("arc-second").count() == 2
    );
    let mut image = DecodingResult::F32(vec![]);
    let layout = decoder.read_image_to_buffer(&mut image).unwrap();
    let DecodingResult::F32(samples) = image else {
        panic!("not float samples")
    };
    assert_eq!(samples.len() * 4, layout.complete_len);
    assert_eq!(samples.len(), 750);
    let projection = Projection::grid(
        bytes,
        "a6db010f956526a011ba719c45fe32e5aff987eeb1cb03f2b8338208a77c619d",
    )
    .unwrap();
    for point in [
        Coord { x: -66., y: 18.5 },
        Coord {
            x: -66.10,
            y: 18.46,
        },
        Coord { x: -68., y: 20.5 },
        Coord { x: -62., y: 17. },
    ] {
        let column: f64 = (point.x + 68.) / 0.25;
        let row: f64 = (20.5 - point.y) / 0.25;
        let c = (column.floor() as usize).min(23);
        let r = (row.floor() as usize).min(13);
        let u = column - c as f64;
        let v = row - r as f64;
        let interpolate = |channel: usize| {
            let sample = |r: usize, c: usize| f64::from(samples[channel * 375 + r * 25 + c]);
            sample(r, c) * (1. - u) * (1. - v)
                + sample(r, c + 1) * u * (1. - v)
                + sample(r + 1, c) * (1. - u) * v
                + sample(r + 1, c + 1) * u * v
        };
        let expected = Coord {
            x: point.x + interpolate(1) / 3600.,
            y: point.y + interpolate(0) / 3600.,
        };
        let actual = projection.apply(point).unwrap();
        assert!(
            (actual.x - expected.x).abs() < 1e-9,
            "{actual:?} != {expected:?}"
        );
        assert!(
            (actual.y - expected.y).abs() < 1e-9,
            "{actual:?} != {expected:?}"
        );
    }
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

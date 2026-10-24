//! Synthetic polygon normalization expectations.

use super::*;
use geo::{Area, Contains};
use geo_types::{MultiPolygon, Point, Polygon, line_string};

#[test]
fn split_dateline_preserves_holes_and_positive_area() {
    let polygon = Polygon::new(
        line_string![(x:179.,y:0.),(x:181.,y:0.),(x:181.,y:2.),(x:179.,y:2.),(x:179.,y:0.)],
        vec![
            line_string![(x:179.25,y:0.5),(x:179.75,y:0.5),(x:179.75,y:1.5),(x:179.25,y:1.5),(x:179.25,y:0.5)],
        ],
    );
    let result = normalize(&MultiPolygon(vec![polygon])).unwrap();
    assert!((result.unsigned_area() - 3.5).abs() < 1e-6);
    assert!(!result.contains(&Point::new(179.5, 1.)));
    assert!(result.contains(&Point::new(-179.5, 1.)));
    assert_eq!(result.0.len(), 2);
}

#[test]
fn wrapped_vertices_and_explicit_longitudes_have_identical_interiors() {
    let wrap = Polygon::new(
        line_string![(x:179.,y:0.),(x:-179.,y:0.),(x:-179.,y:2.),(x:179.,y:2.),(x:179.,y:0.)],
        vec![],
    );
    let explicit = Polygon::new(
        line_string![(x:179.,y:0.),(x:181.,y:0.),(x:181.,y:2.),(x:179.,y:2.),(x:179.,y:0.)],
        vec![],
    );
    assert_eq!(
        normalize(&MultiPolygon(vec![wrap])).unwrap(),
        normalize(&MultiPolygon(vec![explicit])).unwrap()
    );
}

#[test]
fn ambiguous_and_invalid_rings_fail_without_repair() {
    for ring in [
        line_string![(x:0.,y:0.),(x:180.,y:0.),(x:0.,y:1.),(x:0.,y:0.)],
        line_string![(x:-120.,y:0.),(x:0.,y:1.),(x:120.,y:0.),(x:-120.,y:0.)],
        line_string![(x:0.,y:0.),(x:2.,y:2.),(x:0.,y:2.),(x:2.,y:0.),(x:0.,y:0.)],
    ] {
        assert!(normalize(&MultiPolygon(vec![Polygon::new(ring, vec![])])).is_err());
    }
}

#[test]
fn ordinary_straight_edges_and_tiny_polygons_are_unchanged() {
    let original = MultiPolygon(vec![Polygon::new(
        line_string![(x:1.,y:1.),(x:1.000000001,y:1.),(x:1.000000001,y:1.000000001),(x:1.,y:1.000000001),(x:1.,y:1.)],
        vec![],
    )]);
    assert_eq!(normalize(&original).unwrap(), original);
}

#[test]
fn pinned_cq19_clipping_produces_valid_domain_and_is_idempotent() {
    use geo::CoordsIter;
    let input: geojson::Geometry = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/cq-19-original.json"
    )))
    .unwrap();
    let geo_types::Geometry::LineString(line) =
        geo_types::Geometry::<f64>::try_from(input).unwrap()
    else {
        panic!()
    };
    let first = normalize(&MultiPolygon(vec![Polygon::new(line, vec![])])).unwrap();
    let offending: Vec<_> = first
        .coords_iter()
        .filter(|p| p.x.abs() > 180. || p.y.abs() > 90.)
        .collect();
    assert!(
        offending.is_empty(),
        "out-of-domain clipping output: {:?}",
        offending.first()
    );
    assert_eq!(normalize(&first).unwrap(), first);
}

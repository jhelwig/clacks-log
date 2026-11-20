//! Producer output must retain positive-area pieces meeting at an exact node.

use super::*;
use geo::Contains;
use geo_types::{Point, Polygon, Rect, polygon};

#[test]
fn touching_mask_holes_preserve_a_thin_disconnected_classified_piece() {
    let width = 2_f64.powi(-30);
    let county = MultiPolygon(vec![Rect::new((-2., -0.5), (width, 0.5)).to_polygon()]);
    let remote = Polygon::new(
        Rect::new((10., 10.), (14., 14.))
            .to_polygon()
            .exterior()
            .clone(),
        vec![
            Rect::new((11., 11.), (13., 13.))
                .to_polygon()
                .exterior()
                .clone(),
        ],
    );
    let local = Polygon::new(
        Rect::new((-3., -3.), (3., 3.))
            .to_polygon()
            .exterior()
            .clone(),
        vec![
            polygon![(x:0.,y:0.), (x:1.,y:1.), (x:-1.,y:1.), (x:0.,y:0.)]
                .exterior()
                .clone(),
            polygon![(x:0.,y:0.), (x:-1.,y:-1.), (x:1.,y:-1.), (x:0.,y:0.)]
                .exterior()
                .clone(),
        ],
    );
    let land = MultiPolygon(vec![remote, local]);
    clacks_geography::geometry::validate_topology(&land).unwrap();
    let result = overlay(&county, &land, ClipType::Intersection).unwrap();
    assert_eq!(result.0.len(), 2);
    assert!(result.contains(&Point::new(-1., 0.)));
    assert!(result.contains(&Point::new(width / 2., 0.)));
    assert!(!result.contains(&Point::new(0., 0.25)));
}

#[test]
fn a_point_touching_exclusion_remains_an_interior_ring() {
    let county = MultiPolygon(vec![Rect::new((0., 0.), (4., 4.)).to_polygon()]);
    let exclusion = MultiPolygon(vec![polygon![
        (x:2.,y:0.), (x:3.,y:1.), (x:2.,y:2.), (x:1.,y:1.), (x:2.,y:0.)
    ]]);
    let result = overlay(&county, &exclusion, ClipType::Difference).unwrap();
    assert_eq!(result.0.len(), 1);
    assert_eq!(result.0.first().unwrap().interiors().len(), 1);
    assert!(result.contains(&Point::new(0.5, 0.5)));
    assert!(!result.contains(&Point::new(2., 1.)));
}

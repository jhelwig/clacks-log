mod support;

use clacks_geography::{
    artifact::{CompiledFeature, ValueEntry, read::open_bundle, write::write_bundle},
    lookup::Coverage,
};
use clacks_model::position::{Coordinates, GridBounds, Locator, LookupGeometry, RationalDegree};
use geo_types::{MultiPolygon, polygon};

fn area(w: i64, s: i64, e: i64, n: i64) -> LookupGeometry {
    let d = |numerator| RationalDegree {
        numerator,
        denominator: 1,
    };
    LookupGeometry::Area(GridBounds {
        west: d(w),
        south: d(s),
        east: d(e),
        north: d(n),
    })
}
fn point(lon: i32, lat: i32) -> LookupGeometry {
    LookupGeometry::Point(Coordinates::from_microdegrees(lat, lon).unwrap())
}

#[test]
fn shared_boundary_points_are_ambiguous_but_touching_areas_are_not() {
    let mut input = support::contents();
    let mut other: ValueEntry = input.manifest.values[0].clone();
    other.key = "or".into();
    other.recordable_value = "OR".into();
    other.display_label = "Oregon".into();
    input.manifest.values.push(other);
    let layer = input
        .layers
        .iter_mut()
        .find(|l| !l.features.is_empty())
        .unwrap();
    layer.features.push(CompiledFeature {
        part_id: "or-1".into(),
        value_key: "or".into(),
        derivation_key: "census-wa".into(),
        geometry: MultiPolygon(vec![
            polygon![(x:2.,y:0.),(x:3.,y:0.),(x:3.,y:1.),(x:2.,y:1.),(x:2.,y:0.)],
        ]),
    });
    let dir = tempfile::tempdir().unwrap();
    write_bundle(dir.path(), &input).unwrap();
    let mut bundle = open_bundle(dir.path()).unwrap();
    let boundary = bundle
        .lookup(point(2_000_000, 500_000))
        .unwrap()
        .primary_subdivision;
    assert_eq!(boundary.coverage, Coverage::Complete);
    assert_eq!(boundary.candidates.len(), 2);
    let contact = bundle.lookup(area(0, 0, 2, 1)).unwrap().primary_subdivision;
    assert_eq!(contact.coverage, Coverage::Complete);
    assert_eq!(contact.candidates.len(), 1);
    let partial = bundle.lookup(area(0, 0, 4, 1)).unwrap().primary_subdivision;
    assert_eq!(partial.coverage, Coverage::Partial);
    assert_eq!(partial.candidates.len(), 2);
    assert_eq!(
        bundle
            .lookup(point(-1_000_000, 0))
            .unwrap()
            .primary_subdivision
            .coverage,
        Coverage::None
    );
}

#[test]
fn same_value_overlaps_union_and_holes_remain_uncovered() {
    let mut input = support::contents();
    let layer = input
        .layers
        .iter_mut()
        .find(|l| !l.features.is_empty())
        .unwrap();
    let mut duplicate = layer.features[0].clone();
    duplicate.part_id = "wa-2".into();
    layer.features.push(duplicate);
    let dir = tempfile::tempdir().unwrap();
    write_bundle(dir.path(), &input).unwrap();
    let mut bundle = open_bundle(dir.path()).unwrap();
    let result = bundle.lookup(area(0, 0, 2, 1)).unwrap().primary_subdivision;
    assert_eq!(result.coverage, Coverage::Complete);
    assert_eq!(result.candidates.len(), 1);
    let mut hole_input = support::contents();
    let feature = &mut hole_input
        .layers
        .iter_mut()
        .find(|l| !l.features.is_empty())
        .unwrap()
        .features[0];
    feature.geometry = MultiPolygon(vec![geo_types::Polygon::new(
        polygon![(x:0.,y:0.),(x:3.,y:0.),(x:3.,y:3.),(x:0.,y:3.),(x:0.,y:0.)]
            .exterior()
            .clone(),
        vec![
            polygon![(x:1.,y:1.),(x:1.,y:2.),(x:2.,y:2.),(x:2.,y:1.),(x:1.,y:1.)]
                .exterior()
                .clone(),
        ],
    )]);
    let dir = tempfile::tempdir().unwrap();
    write_bundle(dir.path(), &hole_input).unwrap();
    let mut bundle = open_bundle(dir.path()).unwrap();
    assert_eq!(
        bundle
            .lookup(area(0, 0, 3, 3))
            .unwrap()
            .primary_subdivision
            .coverage,
        Coverage::Partial
    );
    assert_eq!(
        bundle
            .lookup(point(1_500_000, 1_500_000))
            .unwrap()
            .primary_subdivision
            .coverage,
        Coverage::None
    );
}

#[test]
fn locators_use_whole_bounds_and_invalid_bounds_are_errors() {
    let dir = tempfile::tempdir().unwrap();
    write_bundle(dir.path(), &support::contents()).unwrap();
    let mut bundle = open_bundle(dir.path()).unwrap();
    for grid in [
        "JJ",
        "JJ00",
        "JJ00AA",
        "JJ00AA00",
        "JJ00AA00AA",
        "JJ00AA00AA00",
    ] {
        let result = bundle
            .lookup(LookupGeometry::Area(Locator::parse(grid).unwrap().bounds()))
            .unwrap();
        assert_eq!(result.primary_subdivision.candidates.len(), 1);
    }
    let mut bounds = Locator::parse("JJ00").unwrap().bounds();
    bounds.west.denominator = 0;
    assert!(bundle.lookup(LookupGeometry::Area(bounds)).is_err());
}

#[test]
fn narrow_positive_overlaps_and_gaps_are_not_reported_complete() {
    use clacks_geography::error::GeographyError;
    for width in [1e-6, 1e-8, 1e-10] {
        let mut input = support::contents();
        let layer = input
            .layers
            .iter_mut()
            .find(|l| !l.features.is_empty())
            .unwrap();
        layer.features[0].geometry = MultiPolygon(vec![
            polygon![(x:-179.,y:-80.),(x:179. + width,y:-80.),(x:179. + width,y:80.),(x:-179.,y:80.),(x:-179.,y:-80.)],
        ]);
        let dir = tempfile::tempdir().unwrap();
        write_bundle(dir.path(), &input).unwrap();
        let mut bundle = open_bundle(dir.path()).unwrap();
        let grid = Locator::parse("RJ90MA00AA00").unwrap();
        match bundle.lookup(LookupGeometry::Area(grid.bounds())) {
            Ok(result) => {
                assert_eq!(result.primary_subdivision.candidates.len(), 1);
                assert_eq!(result.primary_subdivision.coverage, Coverage::Partial);
            }
            Err(GeographyError::NumericalFailure { .. }) => (),
            other => panic!("unexpected thin overlap result {other:?}"),
        }
    }
}

#[test]
fn date_line_endpoints_and_pole_longitudes_share_candidates() {
    let mut input = support::contents();
    let layer = input
        .layers
        .iter_mut()
        .find(|l| !l.features.is_empty())
        .unwrap();
    layer.features[0].geometry = MultiPolygon(vec![
        polygon![(x:179.,y:89.),(x:180.,y:89.),(x:180.,y:90.),(x:179.,y:90.),(x:179.,y:89.)],
    ]);
    let dir = tempfile::tempdir().unwrap();
    write_bundle(dir.path(), &input).unwrap();
    let mut bundle = open_bundle(dir.path()).unwrap();
    let east = bundle.lookup(point(180_000_000, 89_500_000)).unwrap();
    let west = bundle.lookup(point(-180_000_000, 89_500_000)).unwrap();
    assert_eq!(east, west);
    assert_eq!(
        bundle
            .lookup(point(0, 90_000_000))
            .unwrap()
            .primary_subdivision
            .candidates
            .len(),
        1
    );
}

#[test]
fn a_narrow_gap_between_large_features_never_becomes_complete() {
    use clacks_geography::error::GeographyError;
    for width in [1e-6, 1e-8, 1e-10] {
        let grid = Locator::parse("RJ90MA00AA00").unwrap();
        let west = grid.bounds().west.numerator as f64 / grid.bounds().west.denominator as f64;
        let split = west + 0.00001;
        let mut input = support::contents();
        let layer = input
            .layers
            .iter_mut()
            .find(|l| !l.features.is_empty())
            .unwrap();
        layer.features[0].geometry = MultiPolygon(vec![
            polygon![(x:-179.,y:-80.),(x:split,y:-80.),(x:split,y:80.),(x:-179.,y:80.),(x:-179.,y:-80.)],
        ]);
        let mut second = layer.features[0].clone();
        second.part_id = "east".into();
        second.geometry = MultiPolygon(vec![
            polygon![(x:split+width,y:-80.),(x:180.,y:-80.),(x:180.,y:80.),(x:split+width,y:80.),(x:split+width,y:-80.)],
        ]);
        layer.features.push(second);
        let dir = tempfile::tempdir().unwrap();
        write_bundle(dir.path(), &input).unwrap();
        let mut bundle = open_bundle(dir.path()).unwrap();
        match bundle.lookup(LookupGeometry::Area(grid.bounds())) {
            Ok(result) => assert_eq!(
                result.primary_subdivision.coverage,
                Coverage::Partial,
                "lost gap {width}"
            ),
            Err(GeographyError::NumericalFailure { .. }) => (),
            other => panic!("unexpected gap result {other:?}"),
        }
    }
}

#[test]
fn equal_values_in_different_scopes_are_distinct_and_invalid_areas_fail() {
    use clacks_geography::artifact::{Field, ParentScope};
    let mut input = support::contents();
    let mut value = input.manifest.values[0].clone();
    value.key = "different-scope".into();
    value.parent_scope = ParentScope::Entity("150".into());
    input.manifest.values.push(value);
    let layer = input
        .layers
        .iter_mut()
        .find(|l| l.field == Field::PrimarySubdivision)
        .unwrap();
    let mut feature = layer.features[0].clone();
    feature.part_id = "second-scope".into();
    feature.value_key = "different-scope".into();
    layer.features.push(feature);
    let dir = tempfile::tempdir().unwrap();
    write_bundle(dir.path(), &input).unwrap();
    let mut bundle = open_bundle(dir.path()).unwrap();
    assert_eq!(
        bundle
            .lookup(point(500_000, 500_000))
            .unwrap()
            .primary_subdivision
            .candidates
            .len(),
        2
    );
    for invalid in [area(0, 2, 1, 1), area(-181, 0, 0, 1), area(0, 0, 1, 91)] {
        assert!(bundle.lookup(invalid).is_err());
    }
    for grid in [
        "AR",
        "AR09",
        "AR09AX",
        "AR09AX99",
        "AR09AX99AX",
        "AR09AX99AX99",
        "RR",
        "RR99",
        "RR99XX",
        "RR99XX99",
        "RR99XX99XX",
        "RR99XX99XX99",
    ] {
        assert_eq!(
            bundle
                .lookup(LookupGeometry::Area(Locator::parse(grid).unwrap().bounds()))
                .unwrap()
                .primary_subdivision
                .coverage,
            Coverage::None
        );
    }
}

#[test]
fn shared_non_binary_boundary_is_not_an_uncovered_overlay_sliver() {
    for boundary in [0.1234567890123456, 0.9876543210987654] {
        let mut input = support::contents();
        let mut other = input.manifest.values[0].clone();
        other.key = "other".into();
        other.recordable_value = "OR".into();
        other.display_label = "Oregon".into();
        input.manifest.values.push(other);
        let layer = input
            .layers
            .iter_mut()
            .find(|layer| !layer.features.is_empty())
            .unwrap();
        layer.features[0].geometry = MultiPolygon(vec![
            geo_types::Rect::new((-180., -90.), (boundary, 90.)).to_polygon(),
        ]);
        let mut feature = layer.features[0].clone();
        feature.part_id = "right".into();
        feature.value_key = "other".into();
        feature.geometry = MultiPolygon(vec![
            geo_types::Rect::new((boundary, -90.), (180., 90.)).to_polygon(),
        ]);
        layer.features.push(feature);
        let directory = tempfile::tempdir().unwrap();
        write_bundle(directory.path(), &input).unwrap();
        let mut bundle = open_bundle(directory.path()).unwrap();
        let result = bundle.lookup(area(0, 0, 1, 1)).unwrap();
        assert_eq!(result.primary_subdivision.coverage, Coverage::Complete);
        assert_eq!(result.primary_subdivision.candidates.len(), 2);
    }
}

#[test]
fn sub_lattice_holes_and_clipped_corner_gaps_are_never_reported_complete() {
    use geo_types::{Polygon, line_string};
    let delta = 1e-12;
    let shell = geo_types::Rect::new((-180., -90.), (180., 90.)).to_polygon();
    let hole = Polygon::new(
        shell.exterior().clone(),
        vec![line_string![(x:0.5,y:0.5),(x:0.5+delta,y:0.5),(x:0.5,y:0.5+delta),(x:0.5,y:0.5)]],
    );
    let corner = Polygon::new(
        line_string![(x:80.,y:-80.+delta),(x:180.,y:-80.+delta),(x:180.,y:90.),(x:-80.,y:90.),(x:-80.,y:80.+delta),(x:80.,y:-80.+delta)],
        vec![],
    );
    for geometry in [hole, corner] {
        let mut input = support::contents();
        input
            .layers
            .iter_mut()
            .find(|layer| !layer.features.is_empty())
            .unwrap()
            .features[0]
            .geometry = MultiPolygon(vec![geometry]);
        let directory = tempfile::tempdir().unwrap();
        write_bundle(directory.path(), &input).unwrap();
        let mut bundle = open_bundle(directory.path()).unwrap();
        match bundle.lookup(area(0, 0, 1, 1)) {
            Ok(result) => assert_eq!(result.primary_subdivision.coverage, Coverage::Partial),
            Err(error) => assert!(matches!(
                error,
                clacks_geography::error::GeographyError::NumericalFailure { .. }
            )),
        }
    }
}

#[test]
fn separated_near_collinear_nodes_do_not_erase_a_hole_or_notch() {
    use geo_types::{Polygon, line_string};
    let shell = geo_types::Rect::new((-1., -1.), (2., 2.)).to_polygon();
    let thin = line_string![(x:0.2,y:0.2),(x:0.8,y:0.8),(x:0.5,y:0.5+1e-13),(x:0.2,y:0.2)];
    let hole = Polygon::new(shell.exterior().clone(), vec![thin]);
    let notch = Polygon::new(
        line_string![(x:-1.,y:-1.),(x:2.,y:-1.),(x:2.,y:2.),(x:0.8,y:0.8),(x:0.5,y:0.5+1e-13),(x:0.2,y:0.2),(x:-1.,y:2.),(x:-1.,y:-1.)],
        vec![],
    );
    for geometry in [hole, notch] {
        let mut input = support::contents();
        input
            .layers
            .iter_mut()
            .find(|layer| !layer.features.is_empty())
            .unwrap()
            .features[0]
            .geometry = MultiPolygon(vec![geometry]);
        let directory = tempfile::tempdir().unwrap();
        write_bundle(directory.path(), &input).unwrap();
        let mut bundle = open_bundle(directory.path()).unwrap();
        match bundle.lookup(area(0, 0, 1, 1)) {
            Ok(result) => assert_eq!(result.primary_subdivision.coverage, Coverage::Partial),
            Err(error) => assert!(matches!(
                error,
                clacks_geography::error::GeographyError::NumericalFailure { .. }
            )),
        }
    }
}

//! Authored island classifications exercise original-component selection and exclusions.

use super::super::{compile_selected, tests::setup};
use crate::recipe::{ComponentMask, CoordinateMethod, MappingException};
use clacks_geography::artifact::{validate::open_bundle, write::write_bundle};
use clacks_model::position::{Coordinates, LookupGeometry};
use geo_types::{Geometry, MultiPolygon, Rect};
use std::sync::atomic::AtomicBool;

#[test]
fn component_mapping_and_parent_exclusion_produce_distinct_island_values() {
    let directory = tempfile::tempdir().unwrap();
    let (mut recipe, mut sources) = setup(directory.path());
    let operation = recipe.operations.first_mut().unwrap();
    operation.id = "authored-translation".into();
    operation.method = CoordinateMethod::Translation {
        x_m: 0.,
        y_m: 0.,
        z_m: 100_000.,
    };
    let original = sources.features.first_mut().unwrap();
    original.geometry = Geometry::MultiPolygon(MultiPolygon(vec![
        Rect::new((0., 0.), (2., 2.)).to_polygon(),
        Rect::new((4., 0.), (6., 2.)).to_polygon(),
    ]));
    let identity = original.identity.clone();
    let mut child = recipe.mappings.first().unwrap().clone();
    child.id = "island".into();
    child.recordable_value = "138".into();
    child.display_label = "Authored island".into();
    child.source_feature_selectors.clear();
    child.source_components = vec![ComponentMask {
        feature: identity.clone(),
        contains_point: [5., 1.],
    }];
    recipe.mappings.push(child);
    recipe.exceptions.push(MappingException {
        id: "separate-island".into(),
        affected_mapping_ids: vec!["parent".into()],
        exclusion_mapping_ids: vec!["island".into()],
        exclusion_geometry_path: String::new(),
        exclusion_sha256: String::new(),
        replacement: None,
        rationale: "Original second island is a separate entity".into(),
        citations: vec!["Independently authored two-island case".into()],
        notice_paths: vec![],
    });
    let compiled = compile_selected(&recipe, "test", &sources, &AtomicBool::new(false)).unwrap();
    let output = tempfile::tempdir().unwrap();
    write_bundle(output.path(), &compiled.contents).unwrap();
    let mut bundle = open_bundle(output.path()).unwrap();
    for (longitude, wanted) in [(1_000_000, "291"), (5_000_000, "138")] {
        let actual = bundle
            .lookup(LookupGeometry::Point(
                Coordinates::from_microdegrees(1_000_000, longitude).unwrap(),
            ))
            .unwrap();
        assert_eq!(actual.dxcc.candidates.len(), 1);
        assert_eq!(actual.dxcc.candidates.first().unwrap().value, wanted);
    }
    let derivation = compiled
        .contents
        .manifest
        .derivations
        .iter()
        .find(|d| d.mapping_ids.iter().any(|id| id == "island"))
        .unwrap();
    assert_eq!(derivation.source_identities, vec![identity]);
    assert_eq!(derivation.operation_ids, vec!["authored-translation"]);
    // This source interior lies below the transformed island: component mapping
    // must execute the prescribed operation rather than copy raw coordinates.
    let source_position = bundle
        .lookup(LookupGeometry::Point(
            Coordinates::from_microdegrees(100_000, 5_000_000).unwrap(),
        ))
        .unwrap();
    assert!(source_position.dxcc.candidates.is_empty());
}

#[test]
fn component_mapping_rejects_missing_nonfinite_and_ambiguous_original_anchors() {
    let directory = tempfile::tempdir().unwrap();
    let (base, originals) = setup(directory.path());
    for anchor in [[20., 20.], [f64::NAN, 1.], [1., 1.]] {
        let mut recipe = base.clone();
        let mut sources = crate::source::DecodedSources {
            features: originals.features.clone(),
            grids: originals.grids.clone(),
        };
        // Duplicate overlapping originals make the finite interior anchor ambiguous;
        // normalization must not silently collapse them before anchor selection.
        let feature = sources.features.first_mut().unwrap();
        let polygon = Rect::new((0., 0.), (10., 10.)).to_polygon();
        feature.geometry = Geometry::MultiPolygon(MultiPolygon(vec![polygon.clone(), polygon]));
        let mapping = recipe.mappings.first_mut().unwrap();
        mapping.source_feature_selectors.clear();
        mapping.source_components = vec![ComponentMask {
            feature: feature.identity.clone(),
            contains_point: anchor,
        }];
        let error = compile_selected(&recipe, "test", &sources, &AtomicBool::new(false))
            .err()
            .unwrap();
        assert!(
            format!("{error:#}").contains("component anchor"),
            "{error:#}"
        );
    }
}

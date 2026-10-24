//! Compilation tests through production bundle encoding and lookup.

#![allow(clippy::indexing_slicing)]

use super::*;
use crate::recipe::*;
use crate::source::{DecodedSources, SourceFeature};
use clacks_geography::artifact::{Field, ParentScope, SourceIdentity};
use geo_types::{Geometry, MultiPolygon, Rect, polygon};
use serde_json::json;
use sha2_rsa::{Digest, Sha256};
use std::{collections::BTreeMap, sync::atomic::AtomicBool};

fn setup(directory: &std::path::Path) -> (BuildRecipe, DecodedSources) {
    let mut recipe:BuildRecipe=serde_json::from_value(json!({"schema_version":1,"release_id":"test","producer_revision":"test-revision","sources":[{"id":"source","publisher":"Test author","upstream_version":"one","citation_urls":["authored test geometry"],"download_url":"https://example.invalid/source","archive_sha256":"0".repeat(64),"expected_coordinate_declaration":"WGS84","text_encoding":"UTF-8","archive_members":[],"notice_paths":[],"kind":"shapefile","limits":{"download_bytes":1024,"member_bytes":1024,"expanded_bytes":1024,"members":10}}],"selections":[],"operations":[{"id":"identity","source_ids":["source"],"method":"identity","assignment":"entire","description":"Already WGS84","citations":["authored"]}],"corrections":[],"mappings":[{"id":"parent","source_feature_selectors":[{"source_id":"source","feature_id":"one"}],"field":"dxcc","parent_scope":"global","recordable_value":"291","display_label":"United States","citations":["authored"]}],"exceptions":[],"omissions":[],"acceptance_cases":[],"notices":{},"scope":"Synthetic square","limitations":[]})).unwrap();
    recipe.directory = directory.to_owned();
    let feature = SourceFeature {
        identity: SourceIdentity {
            source_id: "source".into(),
            feature_id: "one".into(),
        },
        properties: BTreeMap::new(),
        geometry: Geometry::MultiPolygon(MultiPolygon(vec![
            Rect::new((0., 0.), (10., 10.)).to_polygon(),
        ])),
        coordinate_declaration: "WGS84".into(),
        feature_sha256: "1".repeat(64),
    };
    (
        recipe,
        DecodedSources {
            features: vec![feature],
            grids: BTreeMap::new(),
        },
    )
}

fn geometry_file(directory: &std::path::Path, name: &str, bounds: [f64; 4]) -> String {
    let [w, s, e, n] = bounds;
    let geometry = geojson::Geometry::from(&Rect::new((w, s), (e, n)).to_polygon());
    let bytes = serde_json::to_vec(&geometry).unwrap();
    std::fs::write(directory.join(name), &bytes).unwrap();
    hex::encode(Sha256::digest(&bytes))
}

#[test]
fn exclusions_and_replacements_compile_to_actual_query_geometry() {
    use clacks_geography::artifact::{read::open_bundle, write::write_bundle};
    let dir = tempfile::tempdir().unwrap();
    let (mut recipe, sources) = setup(dir.path());
    let digest = geometry_file(dir.path(), "hole.json", [2., 2., 4., 4.]);
    recipe.exceptions.push(MappingException {
        id: "exclusion".into(),
        affected_mapping_ids: vec!["parent".into()],
        exclusion_mapping_ids: vec![],
        exclusion_geometry_path: "hole.json".into(),
        exclusion_sha256: digest.clone(),
        replacement: None,
        rationale: "Synthetic omitted interior".into(),
        citations: vec!["authored".into()],
        notice_paths: vec![],
    });
    let compiled = compile_selected(&recipe, &sources, &AtomicBool::new(false)).unwrap();
    let layer = compiled
        .contents
        .layers
        .iter()
        .find(|l| l.field == Field::Dxcc)
        .unwrap();
    assert_eq!(layer.features.len(), 1);
    use geo::{Area, Contains};
    assert!((layer.features[0].geometry.unsigned_area() - 96.).abs() < 1e-6);
    assert!(
        !layer.features[0]
            .geometry
            .contains(&geo_types::Point::new(3., 3.))
    );
    let destination = tempfile::tempdir().unwrap();
    write_bundle(destination.path(), &compiled.contents).unwrap();
    open_bundle(destination.path()).unwrap();
    recipe.exceptions[0].replacement = Some(MappingReplacement {
        geometry_path: "hole.json".into(),
        sha256: digest,
        mapping: ValueMapping {
            id: "replacement".into(),
            source_feature_selectors: vec![],
            field: Field::Dxcc,
            parent_scope: ParentScope::Global,
            recordable_value: "236".into(),
            display_label: "Greece".into(),
            citations: vec!["authored".into()],
        },
    });
    let compiled = compile_selected(&recipe, &sources, &AtomicBool::new(false)).unwrap();
    assert_eq!(
        compiled
            .contents
            .layers
            .iter()
            .find(|l| l.field == Field::Dxcc)
            .unwrap()
            .features
            .len(),
        2
    );
}

#[test]
fn bad_mappings_and_order_dependent_exceptions_fail_closed() {
    let dir = tempfile::tempdir().unwrap();
    let (recipe, sources) = setup(dir.path());
    for mutation in ["unknown", "zone", "label", "exception", "duplicate"] {
        let mut recipe = recipe.clone();
        match mutation {
            "unknown" => {
                recipe.mappings[0].source_feature_selectors[0].feature_id = "missing".into()
            }
            "zone" => {
                recipe.mappings[0].field = Field::CqZone;
                recipe.mappings[0].recordable_value = "41".into();
            }
            "label" => {
                let mut mapping = recipe.mappings[0].clone();
                mapping.id = "other".into();
                mapping.display_label = "inconsistent".into();
                recipe.mappings.push(mapping);
            }
            "exception" => recipe.exceptions.push(MappingException {
                id: "bad".into(),
                affected_mapping_ids: vec!["missing".into()],
                exclusion_mapping_ids: vec![],
                exclusion_geometry_path: "missing.json".into(),
                exclusion_sha256: "0".repeat(64),
                replacement: None,
                rationale: "bad".into(),
                citations: vec!["test".into()],
                notice_paths: vec![],
            }),
            "duplicate" => recipe.mappings.push(recipe.mappings[0].clone()),
            _ => unreachable!(),
        }
        assert!(
            compile_selected(&recipe, &sources, &AtomicBool::new(false)).is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn original_guards_are_checked_before_a_complete_replacement_is_used() {
    let dir = tempfile::tempdir().unwrap();
    let (mut recipe, sources) = setup(dir.path());
    let digest = geometry_file(dir.path(), "replacement.json", [1., 1., 9., 9.]);
    recipe.corrections.push(GeometryCorrection {
        id: "corrected".into(),
        feature: sources.features[0].identity.clone(),
        expected_source_digest: "0".repeat(64),
        expected_feature_sha256: "1".repeat(64),
        replacement_geojson_path: "replacement.json".into(),
        replacement_sha256: digest,
        rationale: "Synthetic replacement".into(),
        supporting_citations: vec!["authored".into()],
        notice_paths: vec![],
    });
    let compiled = compile_selected(&recipe, &sources, &AtomicBool::new(false)).unwrap();
    let derivation = compiled.contents.manifest.derivations.first().unwrap();
    assert_eq!(derivation.correction_ids, vec!["corrected"]);
    assert_eq!(
        derivation.source_identities,
        vec![sources.features[0].identity.clone()]
    );
    for mutation in ["source", "feature", "absent", "multiple", "invalid"] {
        let mut recipe = recipe.clone();
        match mutation {
            "source" => recipe.corrections[0].expected_source_digest = "2".repeat(64),
            "feature" => recipe.corrections[0].expected_feature_sha256 = "2".repeat(64),
            "absent" => recipe.corrections[0].feature.feature_id = "missing".into(),
            "multiple" => recipe.corrections.push(recipe.corrections[0].clone()),
            "invalid" => recipe.corrections[0].replacement_sha256 = "2".repeat(64),
            _ => unreachable!(),
        }
        assert!(
            compile_selected(&recipe, &sources, &AtomicBool::new(false)).is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn acceptance_checks_recipe_digest_and_independent_expected_values() {
    use clacks_geography::artifact::{read::open_bundle, write::write_bundle};
    let directory = tempfile::tempdir().unwrap();
    let (mut recipe, sources) = setup(directory.path());
    recipe.acceptance_cases.push(AcceptanceCase {
        id: "inside".into(),
        query: AcceptanceGeometry::Point {
            latitude_microdegrees: 1_000_000,
            longitude_microdegrees: 1_000_000,
        },
        expected: [(
            Field::Dxcc,
            ExpectedField {
                coverage: ExpectedCoverage::Complete,
                candidates: vec![ExpectedCandidate {
                    parent_scope: ParentScope::Global,
                    value: "291".into(),
                }],
            },
        )]
        .into_iter()
        .collect(),
        citations: vec!["authored square".into()],
    });
    let compiled = compile_selected(&recipe, &sources, &AtomicBool::new(false)).unwrap();
    let bundle_dir = tempfile::tempdir().unwrap();
    write_bundle(bundle_dir.path(), &compiled.contents).unwrap();
    let mut bundle = open_bundle(bundle_dir.path()).unwrap();
    assert_eq!(
        verify_bundle(&mut bundle, &recipe).unwrap().checked_cases,
        1
    );
    recipe.acceptance_cases[0]
        .expected
        .get_mut(&Field::Dxcc)
        .unwrap()
        .candidates[0]
        .value = "236".into();
    assert!(verify_bundle(&mut bundle, &recipe).is_err());
}

#[test]
fn a_detailed_mapping_can_supply_an_exclusion_mask_for_a_coarse_mapping() {
    let dir = tempfile::tempdir().unwrap();
    let (mut recipe, mut sources) = setup(dir.path());
    let mut detailed = sources.features[0].clone();
    detailed.identity.feature_id = "detail".into();
    detailed.geometry = Geometry::MultiPolygon(MultiPolygon(vec![
        Rect::new((2., 2.), (4., 4.)).to_polygon(),
    ]));
    sources.features.push(detailed.clone());
    let mut mapping = recipe.mappings[0].clone();
    mapping.id = "detail".into();
    mapping.source_feature_selectors = vec![detailed.identity];
    mapping.recordable_value = "236".into();
    mapping.display_label = "Greece".into();
    recipe.mappings.push(mapping);
    recipe.exceptions.push(MappingException {
        id: "detailed-precedence".into(),
        affected_mapping_ids: vec!["parent".into()],
        exclusion_mapping_ids: vec!["detail".into()],
        exclusion_geometry_path: String::new(),
        exclusion_sha256: String::new(),
        replacement: None,
        rationale: "Detailed source overrides the coarse source".into(),
        citations: vec!["authored".into()],
        notice_paths: vec![],
    });
    let result = compile_selected(&recipe, &sources, &AtomicBool::new(false)).unwrap();
    use geo::Contains;
    let parent = result
        .contents
        .layers
        .iter()
        .flat_map(|layer| &layer.features)
        .find(|feature| feature.part_id.starts_with("parent:"))
        .unwrap();
    assert!(!parent.geometry.contains(&geo_types::Point::new(3., 3.)));
}

#[test]
fn accepted_open_source_rings_retain_explicit_closure_evidence() {
    let directory = tempfile::tempdir().unwrap();
    let (mut recipe, mut sources) = setup(directory.path());
    let mut line = Rect::new((0., 0.), (10., 10.))
        .to_polygon()
        .exterior()
        .clone();
    line.0.pop();
    sources.features[0].geometry = Geometry::LineString(line);
    recipe.sources[0].kind = SourceKind::Zones {
        assignment: "zones".into(),
        expected_features: 1,
        open_rings: ["one".into()].into_iter().collect(),
    };
    let result = compile_selected(&recipe, &sources, &AtomicBool::new(false)).unwrap();
    let id = "close-source-linework:source:one";
    assert!(
        result
            .contents
            .manifest
            .operations
            .iter()
            .any(|operation| operation.id == id)
    );
    assert!(
        result.contents.manifest.derivations[0]
            .operation_ids
            .iter()
            .any(|operation| operation == id)
    );
}

#[test]
fn retained_rule_parameters_are_structured_descriptive_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let (recipe, sources) = setup(directory.path());
    let result = compile_selected(&recipe, &sources, &AtomicBool::new(false)).unwrap();
    let operation = result.contents.manifest.operations.first().unwrap();
    assert_eq!(operation.description, "Already WGS84");
    assert_eq!(operation.definition["method"], "identity");
}

#[test]
fn decoded_iteration_and_output_paths_do_not_change_artifacts() {
    use clacks_geography::artifact::write::write_bundle;
    let directory = tempfile::tempdir().unwrap();
    let (mut recipe, mut sources) = setup(directory.path());
    let mut other = sources.features[0].clone();
    other.identity.feature_id = "two".into();
    other.geometry = Geometry::MultiPolygon(MultiPolygon(vec![
        Rect::new((20., 20.), (30., 30.)).to_polygon(),
    ]));
    let mut mapping = recipe.mappings[0].clone();
    mapping.id = "second".into();
    mapping.source_feature_selectors = vec![other.identity.clone()];
    recipe.mappings.push(mapping);
    sources.features.push(other);
    let first = compile_selected(&recipe, &sources, &AtomicBool::new(false)).unwrap();
    sources.features.reverse();
    let second = compile_selected(&recipe, &sources, &AtomicBool::new(false)).unwrap();
    let left = tempfile::tempdir().unwrap();
    let right = tempfile::tempdir().unwrap();
    let manifest = write_bundle(left.path(), &first.contents).unwrap();
    write_bundle(right.path(), &second.contents).unwrap();
    for path in
        std::iter::once("manifest.json").chain(manifest.files.iter().map(|file| file.path.as_str()))
    {
        assert_eq!(
            std::fs::read(left.path().join(path)).unwrap(),
            std::fs::read(right.path().join(path)).unwrap()
        );
    }
}

#[test]
fn narrow_reviewed_exclusion_remains_unclassified() {
    use clacks_geography::artifact::{read::open_bundle, write::write_bundle};
    use clacks_geography::lookup::Coverage;
    use clacks_model::position::{Coordinates, LookupGeometry};
    let directory = tempfile::tempdir().unwrap();
    let (mut recipe, sources) = setup(directory.path());
    let digest = geometry_file(
        directory.path(),
        "hole.json",
        [5. - 5e-11, 4., 5. + 5e-11, 6.],
    );
    recipe.exceptions.push(MappingException {
        id: "narrow-exclusion".into(),
        affected_mapping_ids: vec!["parent".into()],
        exclusion_mapping_ids: vec![],
        exclusion_geometry_path: "hole.json".into(),
        exclusion_sha256: digest,
        replacement: None,
        rationale: "Reviewed unsupported interior".into(),
        citations: vec!["Authored analytical rectangle".into()],
        notice_paths: vec![],
    });
    let compiled = compile_selected(&recipe, &sources, &AtomicBool::new(false)).unwrap();
    let output = tempfile::tempdir().unwrap();
    write_bundle(output.path(), &compiled.contents).unwrap();
    let mut bundle = open_bundle(output.path()).unwrap();
    let result = bundle
        .lookup(LookupGeometry::Point(
            Coordinates::from_microdegrees(5_000_000, 5_000_000).unwrap(),
        ))
        .unwrap();
    assert_eq!(result.dxcc.coverage, Coverage::None);
    assert!(result.dxcc.candidates.is_empty());
}

#[test]
fn shared_boundary_segmentation_does_not_create_a_coverage_gap() {
    use clacks_geography::artifact::{read::open_bundle, write::write_bundle};
    use clacks_geography::lookup::Coverage;
    use clacks_model::position::{Coordinates, GridLength, LookupGeometry};
    let directory = tempfile::tempdir().unwrap();
    let (mut recipe, mut sources) = setup(directory.path());
    recipe.operations[0].method = CoordinateMethod::Translation {
        x_m: -2.,
        y_m: 0.,
        z_m: 4.,
    };
    sources.features[0].geometry = Geometry::MultiPolygon(MultiPolygon(vec![geo_types::polygon![
        (x:-170., y:50.), (x:-150., y:60.), (x:-150., y:70.), (x:-170., y:50.)
    ]]));
    let mut other = sources.features[0].clone();
    other.identity.feature_id = "two".into();
    other.geometry = Geometry::MultiPolygon(MultiPolygon(vec![geo_types::polygon![
        (x:-170., y:50.), (x:-170., y:40.), (x:-150., y:60.), (x:-164., y:53.), (x:-170., y:50.)
    ]]));
    let mut mapping = recipe.mappings[0].clone();
    mapping.id = "two".into();
    mapping.source_feature_selectors = vec![other.identity.clone()];
    recipe.mappings.push(mapping);
    sources.features.push(other);
    let compiled = compile_selected(&recipe, &sources, &AtomicBool::new(false)).unwrap();
    let output = tempfile::tempdir().unwrap();
    write_bundle(output.path(), &compiled.contents).unwrap();
    let mut bundle = open_bundle(output.path()).unwrap();
    let locator = Coordinates::from_microdegrees(53_000_008, -164_000_008)
        .unwrap()
        .locator(GridLength::new(12).unwrap());
    let result = bundle
        .lookup(LookupGeometry::Area(locator.bounds()))
        .unwrap();
    assert_eq!(result.dxcc.coverage, Coverage::Complete);
    assert_eq!(result.dxcc.candidates.len(), 1);
    sources.features.reverse();
    recipe.mappings.reverse();
    let reversed = compile_selected(&recipe, &sources, &AtomicBool::new(false)).unwrap();
    let second = tempfile::tempdir().unwrap();
    write_bundle(second.path(), &reversed.contents).unwrap();
    assert_eq!(
        std::fs::read(output.path().join("layers/dxcc.fgb")).unwrap(),
        std::fs::read(second.path().join("layers/dxcc.fgb")).unwrap()
    );
}

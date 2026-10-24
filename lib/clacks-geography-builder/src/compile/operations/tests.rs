//! Source partition and exact-operation seam requirements.

use super::*;
use crate::recipe::*;
use crate::source::*;
use clacks_geography::artifact::SourceIdentity;
use geo_types::{Geometry, MultiPolygon, Rect};
use std::{collections::BTreeMap, sync::atomic::AtomicBool};

fn feature() -> SourceFeature {
    SourceFeature {
        identity: SourceIdentity {
            source_id: "source".into(),
            feature_id: "one".into(),
        },
        properties: BTreeMap::new(),
        geometry: Geometry::MultiPolygon(MultiPolygon(vec![
            Rect::new((-67., 18.), (-66., 19.)).to_polygon(),
        ])),
        coordinate_declaration: "NAD83".into(),
        feature_sha256: "0".repeat(64),
    }
}

fn operation(id: &str, assignment: Assignment, method: CoordinateMethod) -> CoordinateOperation {
    CoordinateOperation {
        id: id.into(),
        source_ids: vec!["source".into()],
        assignment,
        method,
        description: "explicit test operation".into(),
        citations: vec!["test".into()],
    }
}

#[test]
fn partitions_reject_missing_and_overlapping_assignments() {
    let sources = DecodedSources {
        features: vec![feature()],
        grids: BTreeMap::new(),
    };
    let cancelled = AtomicBool::new(false);
    for operations in [
        vec![],
        vec![operation(
            "one",
            Assignment::Bounds([-67., 18., -66.5, 19.]),
            CoordinateMethod::Identity,
        )],
        vec![
            operation("one", Assignment::Entire, CoordinateMethod::Identity),
            operation("two", Assignment::Entire, CoordinateMethod::Identity),
        ],
    ] {
        let prepared = prepare_operations(&operations, &sources).unwrap();
        assert!(transform_feature(&feature(), &prepared, &cancelled).is_err());
    }
}

#[test]
fn distinct_operations_preserve_exact_seam_and_report_displacement() {
    let sources = DecodedSources {
        features: vec![feature()],
        grids: BTreeMap::new(),
    };
    let operations = vec![
        operation(
            "shift",
            Assignment::Bounds([-67., 18., -66.5, 19.]),
            CoordinateMethod::Translation {
                x_m: -2.,
                y_m: 0.,
                z_m: 4.,
            },
        ),
        operation(
            "approximation",
            Assignment::Remainder {
                excluded_operation_ids: vec!["shift".into()],
            },
            CoordinateMethod::NoShift,
        ),
    ];
    let prepared = prepare_operations(&operations, &sources).unwrap();
    let result = transform_feature(&feature(), &prepared, &AtomicBool::new(false)).unwrap();
    assert_eq!(result.parts.len(), 2);
    assert!(!result.seams.is_empty());
    assert!(
        result
            .seams
            .iter()
            .any(|seam| seam.max_sampled_displacement_m > 0.01)
    );
}

#[test]
fn missing_required_grid_is_not_an_approximation() {
    let sources = DecodedSources {
        features: vec![feature()],
        grids: BTreeMap::new(),
    };
    let operations = vec![operation(
        "grid",
        Assignment::Entire,
        CoordinateMethod::Grid {
            source_id: "missing".into(),
        },
    )];
    assert!(prepare_operations(&operations, &sources).is_err());
}

#[test]
fn coordinate_parts_share_one_feature_vertex_budget() {
    let geometry = MultiPolygon(vec![Rect::new((-67., 18.), (-66., 19.)).to_polygon()]);
    let projection = Projection::translation([0., 0., 0.]).unwrap();
    let mut remaining = 7usize;
    transform_polygon(
        &geometry,
        &projection,
        &AtomicBool::new(false),
        &mut remaining,
    )
    .unwrap();
    assert_eq!(remaining, 2);
    assert!(
        transform_polygon(
            &geometry,
            &projection,
            &AtomicBool::new(false),
            &mut remaining
        )
        .is_err()
    );
}

#[test]
fn fractional_partition_boundary_retains_seam_correspondence() {
    let sources = DecodedSources {
        features: vec![feature()],
        grids: BTreeMap::new(),
    };
    let operations = vec![
        operation(
            "shift",
            Assignment::Bounds([-67., 18., -66.523456789, 19.]),
            CoordinateMethod::Translation {
                x_m: -2.,
                y_m: 0.,
                z_m: 4.,
            },
        ),
        operation(
            "approximation",
            Assignment::Remainder {
                excluded_operation_ids: vec!["shift".into()],
            },
            CoordinateMethod::NoShift,
        ),
    ];
    let prepared = prepare_operations(&operations, &sources).unwrap();
    let result = transform_feature(&feature(), &prepared, &AtomicBool::new(false)).unwrap();
    let [seam] = result.seams.as_slice() else {
        panic!("expected one seam, got {:?}", result.seams)
    };
    assert!(seam.sampled_vertices >= 2);
    assert!(seam.max_sampled_displacement_m > 0.01);
}

fn transform_feature(
    feature: &SourceFeature,
    prepared: &PreparedOperations<'_>,
    cancelled: &AtomicBool,
) -> Result<TransformedFeature> {
    transform_features(&[feature], prepared, cancelled)?
        .remove(&feature.identity)
        .context("transformed feature")
}

#[test]
fn diagonal_partition_intersections_retain_seam_correspondence() {
    use geo_types::polygon;
    let mut mask = feature();
    mask.identity.feature_id = "mask".into();
    mask.geometry = Geometry::MultiPolygon(MultiPolygon(vec![polygon![
        (x:-68.,y:17.), (x:-65.,y:17.), (x:-66.123456789,y:20.), (x:-68.,y:17.)
    ]]));
    let mask_identity = mask.identity.clone();
    let sources = DecodedSources {
        features: vec![feature(), mask],
        grids: BTreeMap::new(),
    };
    let operations = vec![
        operation(
            "shift",
            Assignment::SourceMask {
                source_ids: vec![],
                components: vec![],
                identities: vec![mask_identity],
                extent: None,
            },
            CoordinateMethod::Translation {
                x_m: -2.,
                y_m: 0.,
                z_m: 4.,
            },
        ),
        operation(
            "approximation",
            Assignment::Remainder {
                excluded_operation_ids: vec!["shift".into()],
            },
            CoordinateMethod::NoShift,
        ),
    ];
    let prepared = prepare_operations(&operations, &sources).unwrap();
    let result = transform_feature(&feature(), &prepared, &AtomicBool::new(false)).unwrap();
    let [seam] = result.seams.as_slice() else {
        panic!("expected one seam, got {:?}", result.seams)
    };
    assert!(seam.sampled_vertices >= 2);
    assert!(seam.max_sampled_displacement_m > 0.01);
}

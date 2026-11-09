//! Guarded representation of Egypt's exact source boundary as explicit rings.

use crate::{
    recipe::{BuildRecipe, GeometryCorrection},
    source::DecodedSources,
};
use anyhow::{Context, Result, ensure};
use geo::{Area, Relate, coordinate_position::CoordPos, dimensions::Dimensions};
use geo_types::{Geometry, LineString, MultiPolygon, Polygon};
use sha2_rsa::{Digest, Sha256};
use std::{collections::BTreeMap, fs};

/// Original semantic feature whose attached loop has been independently reviewed.
const ORIGINAL_EGYPT_SHA256: &str =
    "b1a8eae72bc99cc827137c71a0b3c1aad55ad74d0ce24081baa05521722917c4";

/// Turn only the known source loop into its explicit touching hole, preserving edges.
pub fn author(
    recipe: &mut BuildRecipe,
    sources: &DecodedSources,
    references: &std::path::Path,
) -> Result<()> {
    let source = sources
        .features
        .iter()
        .find(|feature| {
            feature.identity.source_id == "subunits" && feature.identity.feature_id == "EGY"
        })
        .context("Egypt original source")?;
    ensure!(
        source.feature_sha256 == ORIGINAL_EGYPT_SHA256,
        "Egypt original feature guard changed"
    );
    let Geometry::MultiPolygon(original) = &source.geometry else {
        anyhow::bail!("Egypt original is not multipart");
    };
    ensure!(
        original.0.len() == 8,
        "Egypt original member inventory changed"
    );
    let mut reviewed = original.clone();
    let main = reviewed.0.first_mut().context("Egypt main polygon")?;
    ensure!(
        main.exterior().0.len() == 1801 && main.interiors().len() == 2,
        "Egypt original ring inventory changed"
    );
    let coordinates = &main.exterior().0;
    let loop_coordinates = coordinates
        .get(765..=770)
        .context("Egypt reviewed touching loop")?
        .to_vec();
    let hole = LineString(loop_coordinates);
    let hole_polygon = Polygon::new(hole.clone(), vec![]);
    ensure!(
        main.signed_area() < 0. && hole_polygon.signed_area() > 0.,
        "Egypt reviewed shell/hole winding changed"
    );
    let exterior = LineString(
        coordinates
            .iter()
            .take(765)
            .chain(coordinates.iter().skip(770))
            .copied()
            .collect(),
    );
    let shell = Polygon::new(exterior.clone(), vec![]);
    let relation = shell.relate(&hole_polygon);
    ensure!(
        relation.is_contains()
            && relation.get(CoordPos::OnBoundary, CoordPos::OnBoundary)
                == Dimensions::ZeroDimensional,
        "Egypt reviewed hole is not contained with point-only shell contact"
    );
    let mut holes = main.interiors().to_vec();
    holes.push(hole);
    *main = Polygon::new(exterior, holes);

    ensure!(
        boundary_edges(original) == boundary_edges(&reviewed),
        "Egypt ring reorganization changed an original directed boundary edge"
    );
    ensure!(
        original.signed_area() == reviewed.signed_area()
            && original.unsigned_area() == reviewed.unsigned_area(),
        "Egypt ring reorganization changed source area"
    );
    clacks_geography::geometry::validate_topology(&reviewed)?;
    let bytes = serde_json::to_vec_pretty(&geojson::Geometry::from(&reviewed))?;
    let path = "shipping/egypt.json";
    fs::write(references.join("geography").join(path), &bytes)?;
    let pin = recipe
        .sources
        .iter()
        .find(|pin| pin.id == "subunits")
        .context("Natural Earth source pin")?;
    recipe.corrections.push(GeometryCorrection {
        id:"egypt-explicit-touching-hole".into(),
        feature:source.identity.clone(),
        expected_source_digest:pin.archive_sha256.clone(),
        expected_feature_sha256:ORIGINAL_EGYPT_SHA256.into(),
        replacement_geojson_path:path.into(),
        replacement_sha256:hex::encode(Sha256::digest(&bytes)),
        rationale:"The original Egypt exterior contains the identical node 35.62108710600006/23.139292914000066 at positions 765 and 770. Its six-point attached loop is counterclockwise within the clockwise shell and represents a touching hole. Move exactly that original loop into a third explicit hole; retain all other rings and seven other polygon members. The full directed boundary-edge multiset is bitwise identical, signed/unsigned source area is exactly unchanged, and the complete replacement satisfies required topology. No coordinate is moved, snapped, dropped or invented; no generic source repair is enabled.".into(),
        supporting_citations:vec![pin.download_url.clone(),"Pinned Natural Earth EGY semantic feature and exact source-ring/winding/containment inspection; reproducible by the maintained Rust authoring tool.".into()],
        notice_paths:vec!["notices/natural-earth.txt".into()],
    });
    Ok(())
}

/// Count exact directed edge bit patterns, including original zero-length edges.
fn boundary_edges(geometry: &MultiPolygon<f64>) -> BTreeMap<[u64; 4], usize> {
    let mut edges = BTreeMap::new();
    for polygon in &geometry.0 {
        for ring in std::iter::once(polygon.exterior()).chain(polygon.interiors()) {
            for edge in ring.lines() {
                *edges
                    .entry([
                        edge.start.x.to_bits(),
                        edge.start.y.to_bits(),
                        edge.end.x.to_bits(),
                        edge.end.y.to_bits(),
                    ])
                    .or_default() += 1;
            }
        }
    }
    edges
}

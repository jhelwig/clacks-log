//! Guarded source-specific ring repairs and explicit full-longitude bands.

use crate::{
    recipe::{BuildRecipe, GeometryCorrection, ValueMapping},
    source::DecodedSources,
};
use anyhow::{Context, Result, ensure};
use clacks_geography::artifact::{Field, ParentScope};
use geo_types::{Coord, Geometry, LineString, Polygon};
use sha2_rsa::{Digest, Sha256};

/// Preserve original linework except reviewed overshoots and explicit edge subdivision.
pub fn author(
    recipe: &mut BuildRecipe,
    sources: &DecodedSources,
    references: &std::path::Path,
) -> Result<()> {
    let pin = recipe
        .sources
        .iter()
        .find(|source| source.id == "itu")
        .context("ITU source")?;
    let digest = pin.archive_sha256.clone();
    let citation = pin.download_url.clone();
    recipe
        .mappings
        .retain(|mapping| mapping.field != Field::ItuZone);
    recipe.corrections.clear();
    recipe.limitations.push("ITU zones 74 and 75 explicitly cover full-longitude bands from 80 to 85 degrees south/north. Their guarded interpretation preserves those exact extents; ITU coverage beyond 85 degrees remains unknown. Retaining all 90 source records does not infer polar caps.".into());
    for source in sources
        .features
        .iter()
        .filter(|feature| feature.identity.source_id == "itu")
    {
        let zone = source.identity.feature_id.as_str();
        let Geometry::LineString(original) = &source.geometry else {
            anyhow::bail!("ITU source is not linework");
        };
        let mut line = original.clone();
        let rationale = match zone {
            "2" | "4" | "14" | "15" => {
                let expected = match zone {
                    "2" => (-141.00403, 79.99907),
                    "4" => (-90.00001, 48.05995),
                    "14" => (-58.3374, -16.5),
                    _ => (-58.3438, -16.51773),
                };
                let final_vertex = line.0.pop().context("empty ITU ring")?;
                ensure!(
                    (final_vertex.x, final_vertex.y) == expected,
                    "reviewed ITU {zone} final vertex drift"
                );
                Some(
                    "Remove the specifically reviewed final near-start overshoot and close directly to the original first vertex; preserve every other original coordinate.",
                )
            }
            "31" | "42" => {
                let spike = geo_types::Coord {
                    x: 80.60215,
                    y: 42.89507,
                };
                ensure!(
                    line.0.iter().filter(|point| **point == spike).count() == 1,
                    "reviewed shared ITU spike drift"
                );
                line.0.retain(|point| *point != spike);
                Some(
                    "Remove only the shared source spike at longitude 80.60215, latitude 42.89507. The path from 80.54704/42.88449 doubles back through 80.59896/42.89399 and crosses itself before 80.59714/42.89744. Both neighboring zone records receive the identical correction. All other coordinates retained.",
                )
            }
            "74" | "75" => {
                line = full_longitude_band(original, zone)?;
                Some(
                    "Explicit source interpretation: this original rectangle spans all longitudes between 80 and 85 degrees south (74) or north (75). Retain all original vertices and subdivide only its two horizontal +/-180 degree edges with collinear +/-60 degree vertices. The Cartesian footprint and latitude extent are unchanged; each replacement edge has unambiguous longitude continuity. No coverage beyond 85 degrees or polar cap is inferred.",
                )
            }
            _ => None,
        };
        if let Some(rationale) = rationale {
            let polygon = Polygon::new(line, vec![]);
            clacks_geography::geometry::validate_topology(&geo_types::MultiPolygon(vec![
                polygon.clone(),
            ]))?;
            let bytes = serde_json::to_vec_pretty(&geojson::Geometry::from(&polygon))?;
            let path = format!("shipping/itu-{zone}.json");
            std::fs::write(references.join("geography").join(&path), &bytes)?;
            recipe.corrections.push(GeometryCorrection {
                id: if matches!(zone, "74" | "75") {
                    format!("itu-{zone}-full-longitude-band")
                } else {
                    format!("itu-{zone}-reviewed-overshoot")
                },
                feature: source.identity.clone(),
                expected_source_digest: digest.clone(),
                expected_feature_sha256: source.feature_sha256.clone(),
                replacement_geojson_path: path,
                replacement_sha256: hex::encode(Sha256::digest(&bytes)),
                rationale: rationale.into(),
                supporting_citations: if matches!(zone, "74" | "75") {
                    vec![
                        citation.clone(),
                        "https://www.rfc-editor.org/rfc/rfc7946.html#section-3.1.1".into(),
                    ]
                } else {
                    vec![citation.clone()]
                },
                notice_paths: vec!["notices/ha8tks.txt".into()],
            });
        }
        recipe.mappings.push(ValueMapping {
            id: format!("itu-{zone}"),
            source_components: vec![],
            source_feature_selectors: vec![source.identity.clone()],
            field: Field::ItuZone,
            parent_scope: ParentScope::Global,
            recordable_value: zone.into(),
            display_label: format!("ITU zone {zone}"),
            citations: vec![citation.clone()],
        });
    }
    Ok(())
}

/// Author the two pinned world-width latitude bands without inferring polar caps.
fn full_longitude_band(original: &LineString<f64>, zone: &str) -> Result<LineString<f64>> {
    use geo::Area;
    let (south, north) = if zone == "74" {
        (-85., -80.)
    } else {
        (80., 85.)
    };
    ensure!(
        original.0.len() == 9
            && original
                .0
                .iter()
                .all(|point| point.x.abs() == 180. && (point.y == south || point.y == north)),
        "reviewed ITU{zone} full-band original drift"
    );
    ensure!(
        Polygon::new(original.clone(), vec![]).unsigned_area() == 1800.,
        "reviewed full-band Cartesian footprint drift"
    );
    let mut coordinates = vec![
        *original
            .0
            .first()
            .context("missing full-band first vertex")?,
    ];
    let mut full_width_edges = 0;
    for pair in original.0.windows(2) {
        let [start, end] = pair else {
            anyhow::bail!("missing full-band edge endpoint");
        };
        if (end.x - start.x).abs() == 360. {
            ensure!(start.y == end.y, "nonhorizontal full-band source edge");
            let intermediate = if start.x < end.x {
                [-60., 60.]
            } else {
                [60., -60.]
            };
            coordinates.extend(intermediate.map(|x| Coord { x, y: start.y }));
            full_width_edges += 1;
        }
        coordinates.push(*end);
    }
    ensure!(
        full_width_edges == 2,
        "reviewed full-band edge inventory drift"
    );
    let reviewed = LineString(coordinates);
    ensure!(
        Polygon::new(reviewed.clone(), vec![]).unsigned_area() == 1800.,
        "full-band subdivision changed its Cartesian footprint"
    );
    Ok(reviewed)
}

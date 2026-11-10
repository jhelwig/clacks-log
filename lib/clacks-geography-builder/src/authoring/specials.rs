//! Exact selected U.S. island components that are separate radio entities.

use crate::{
    recipe::{BuildRecipe, ComponentMask, MappingException},
    source::DecodedSources,
};
use anyhow::{Context, Result, ensure};
use clacks_geography::artifact::{Field, ParentScope};
use geo::Contains;
use geo_types::{Geometry, Point};

/// Independently identified island and its administrative context.
struct Island {
    /// Human-readable island name.
    name: &'static str,
    /// Census state/territory code.
    state: &'static str,
    /// Original Census county identity.
    county: &'static str,
    /// Original-source longitude/latitude interior anchor.
    anchor: [f64; 2],
    /// Separate recorded radio entity.
    dxcc: &'static str,
}

/// Split separate radio entities using original components and normal transforms.
pub fn author(recipe: &mut BuildRecipe, sources: &DecodedSources) -> Result<()> {
    let islands = [
        Island {
            name: "Kure",
            state: "HI",
            county: "15003",
            anchor: [-178.33, 28.418],
            dxcc: "138",
        },
        Island {
            name: "Desecheo",
            state: "PR",
            county: "72097",
            anchor: [-67.481, 18.386],
            dxcc: "43",
        },
        Island {
            name: "Swains",
            state: "AS",
            county: "60040",
            anchor: [-171.086, -11.056],
            dxcc: "515",
        },
    ];
    for island in islands {
        for field in [
            Field::Dxcc,
            Field::PrimarySubdivision,
            Field::SecondarySubdivision,
        ] {
            let base_id = match field {
                Field::Dxcc => format!("state-dxcc-{}", island.state),
                Field::PrimarySubdivision => format!("state-{}", island.state),
                _ => format!("county-{}", island.county),
            };
            let parent = match field {
                Field::Dxcc => ParentScope::Global,
                Field::PrimarySubdivision => ParentScope::Entity(island.dxcc.into()),
                _ => ParentScope::Subdivision {
                    entity: island.dxcc.into(),
                    primary: island.state.into(),
                },
            };
            if island.state == "AS" && field == Field::SecondarySubdivision {
                recipe
                    .mappings
                    .iter_mut()
                    .find(|mapping| mapping.id == base_id)
                    .context("Swains county")?
                    .parent_scope = parent;
                continue;
            }
            let mut child = recipe
                .mappings
                .iter()
                .find(|mapping| mapping.id == base_id)
                .context("island parent mapping")?
                .clone();
            let [original] = child.source_feature_selectors.as_slice() else {
                anyhow::bail!("island parent must identify one original feature");
            };
            child.source_components = vec![ComponentMask {
                feature: original.clone(),
                contains_point: island.anchor,
            }];
            child.source_feature_selectors.clear();
            child.id = format!("{base_id}-separate-{}", island.name);
            child.parent_scope = parent;
            if field == Field::Dxcc {
                child.recordable_value = island.dxcc.into();
                child.display_label = island.name.into();
            }
            child.citations.extend([
                "https://www.arrl.org/country-lists-prefixes".into(),
                "https://lotw.arrl.org/lotw-help/specialdocumentationrequirements/".into(),
                "Original Census component selected by island interior anchor; normal prescribed source-coordinate operations apply.".into(),
            ]);
            recipe.exceptions.push(MappingException {
                id:format!("separate-{}-{base_id}",island.name),
                affected_mapping_ids:vec![base_id],
                exclusion_mapping_ids:vec![child.id.clone()],
                exclusion_geometry_path:String::new(),
                exclusion_sha256:String::new(),
                replacement:None,
                rationale:format!("{} is a separate DXCC entity ({}). Subtract its exact selected and normally transformed Census component from the parent classification; child mapping supplies the separate classified value/scope.",island.name,island.dxcc),
                citations:vec!["https://www.arrl.org/country-lists-prefixes".into()],
                notice_paths:vec!["notices/census.txt".into()],
            });
            recipe.mappings.push(child);
        }
    }
    let mut invalid = Vec::new();
    for mapping in &recipe.mappings {
        for component in &mapping.source_components {
            let original = sources
                .features
                .iter()
                .find(|feature| feature.identity == component.feature)
                .context("component original source")?;
            let point = Point::from(component.contains_point);
            let matching = match &original.geometry {
                Geometry::Polygon(polygon) => usize::from(polygon.contains(&point)),
                Geometry::MultiPolygon(polygons) => polygons
                    .0
                    .iter()
                    .filter(|polygon| polygon.contains(&point))
                    .count(),
                _ => anyhow::bail!("component original is not polygon geometry"),
            };
            if matching != 1 {
                invalid.push(mapping.id.clone());
            }
        }
    }
    ensure!(
        invalid.is_empty(),
        "invalid original component anchors: {invalid:?}"
    );
    Ok(())
}

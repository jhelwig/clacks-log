//! Original-source polygon selection before the prescribed coordinate operations.

use super::operations::{self, PreparedOperations, TransformedFeature};
use crate::{
    recipe::BuildRecipe,
    source::{DecodedSources, check_cancelled},
};
use anyhow::{Context, Result, ensure};
use clacks_geography::artifact::SourceIdentity;
use geo::Contains;
use geo_types::{Geometry, MultiPolygon, Point};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::AtomicBool,
};

#[cfg(test)]
mod tests;

/// Mapping-specific transformed components, retaining the original source identities.
pub(super) type MappingComponents = BTreeMap<String, BTreeMap<SourceIdentity, TransformedFeature>>;

/// Select original components and reuse the complete production transformation path.
pub(super) fn transform_components(
    recipe: &BuildRecipe,
    sources: &DecodedSources,
    operations: &PreparedOperations<'_>,
    cancelled: &AtomicBool,
) -> Result<MappingComponents> {
    let mut result = BTreeMap::new();
    for mapping in &recipe.mappings {
        check_cancelled(cancelled)?;
        let mut selected = BTreeMap::new();
        let mut component_ids = BTreeSet::new();
        for component in &mapping.source_components {
            ensure!(
                !mapping
                    .source_feature_selectors
                    .contains(&component.feature),
                "mapping selects both full source and its component"
            );
            ensure!(
                !recipe
                    .corrections
                    .iter()
                    .any(|c| c.feature == component.feature),
                "component mapping cannot select a fully corrected source"
            );
            let matching: Vec<_> = sources
                .features
                .iter()
                .filter(|f| f.identity == component.feature)
                .collect();
            let [source] = matching.as_slice() else {
                anyhow::bail!("component mapping must identify exactly one selected feature")
            };
            let [x, y] = component.contains_point;
            ensure!(
                x.is_finite() && y.is_finite(),
                "non-finite component anchor"
            );
            // Match originals before normalization can split or merge components.
            let polygons = match &source.geometry {
                Geometry::Polygon(polygon) => std::slice::from_ref(polygon),
                Geometry::MultiPolygon(polygons) => polygons.0.as_slice(),
                _ => anyhow::bail!("component mapping requires original source polygons"),
            };
            let matching: Vec<_> = polygons
                .iter()
                .enumerate()
                .filter(|(_, polygon)| polygon.contains(&Point::new(x, y)))
                .collect();
            let [(index, polygon)] = matching.as_slice() else {
                anyhow::bail!(
                    "component anchor does not identify exactly one original source polygon"
                )
            };
            ensure!(
                component_ids.insert((&component.feature, *index)),
                "duplicate mapping component"
            );
            selected
                .entry(component.feature.clone())
                .or_insert_with(Vec::new)
                .push((*polygon).clone());
        }
        let selected: Vec<_> = selected
            .into_iter()
            .map(|(identity, polygons)| {
                let mut feature = sources
                    .features
                    .iter()
                    .find(|f| f.identity == identity)
                    .context("selected component source missing")?
                    .clone();
                feature.geometry = Geometry::MultiPolygon(MultiPolygon(polygons));
                Ok(feature)
            })
            .collect::<Result<_>>()?;
        if !selected.is_empty() {
            let references: Vec<_> = selected.iter().collect();
            result.insert(
                mapping.id.clone(),
                operations::transform_features(&references, operations, cancelled)?,
            );
        }
    }
    Ok(result)
}

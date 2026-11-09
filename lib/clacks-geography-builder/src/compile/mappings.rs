//! Exact recorded-value assignments and compiled geographic exceptions.

use super::{
    components::MappingComponents, corrections::read_geometry, operations::TransformedFeature,
    overlay::overlay,
};
use crate::{
    recipe::{BuildRecipe, ValueMapping},
    source::check_cancelled,
};
use anyhow::{Context, Result, ensure};
use clacks_geography::artifact::{
    CompiledFeature, Derivation, Field, LayerInput, SourceIdentity, ValueEntry,
};
use clipper2_rust::engine::ClipType;
use geo::{BooleanOps, Relate};
use geo_types::MultiPolygon;
use sha2_rsa::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::AtomicBool,
};

/// Classified geometry and its manifest-local value and derivation dictionaries.
pub(super) struct MappedContents {
    /// One mandatory layer per field, including empty roles.
    pub layers: Vec<LayerInput>,
    /// Deduplicated scoped values with consistent display labels.
    pub values: Vec<ValueEntry>,
    /// Source and rule evidence for every produced feature.
    pub derivations: Vec<Derivation>,
}

/// Compile exclusions and replacements into geometry without runtime precedence rules.
pub(super) fn compile_mappings(
    recipe: &BuildRecipe,
    features: &BTreeMap<SourceIdentity, TransformedFeature>,
    components: &MappingComponents,
    corrections: &BTreeMap<SourceIdentity, String>,
    cancelled: &AtomicBool,
) -> Result<MappedContents> {
    let mut ids = BTreeSet::new();
    let mut values: BTreeMap<String, ValueEntry> = BTreeMap::new();
    let mut derivations = Vec::new();
    let mut layers: BTreeMap<Field, Vec<CompiledFeature>> = Field::ALL
        .into_iter()
        .map(|field| (field, Vec::new()))
        .collect();
    for mapping in recipe.mappings.iter().chain(
        recipe
            .exceptions
            .iter()
            .filter_map(|e| e.replacement.as_ref().map(|r| &r.mapping)),
    ) {
        ensure!(
            !mapping.id.is_empty() && ids.insert(&mapping.id),
            "duplicate/empty mapping identity"
        );
        let value = value(mapping)?;
        if let Some(existing) = values.insert(value.key.clone(), value.clone()) {
            ensure!(existing == value, "inconsistent scoped value label");
        }
    }
    let base_ids: BTreeSet<_> = recipe.mappings.iter().map(|m| &m.id).collect();
    let mut exception_ids = BTreeSet::new();
    let mut exclusions = BTreeMap::new();
    for exception in &recipe.exceptions {
        ensure!(
            !exception.id.is_empty()
                && exception_ids.insert(&exception.id)
                && !exception.affected_mapping_ids.is_empty()
                && !exception.rationale.is_empty()
                && !exception.citations.is_empty(),
            "invalid/duplicate exception"
        );
        ensure!(
            exception
                .notice_paths
                .iter()
                .all(|path| recipe.notices.contains_key(path)),
            "exception lacks notice"
        );
        ensure!(
            exception
                .affected_mapping_ids
                .iter()
                .all(|id| base_ids.contains(id)),
            "exception targets unknown/non-base mapping (cycles are not supported)"
        );
        let geometry = if exception.exclusion_mapping_ids.is_empty() {
            read_geometry(
                &recipe.directory,
                &exception.exclusion_geometry_path,
                &exception.exclusion_sha256,
            )?
        } else {
            ensure!(
                exception.exclusion_geometry_path.is_empty()
                    && exception.exclusion_sha256.is_empty(),
                "exclusion has both file and mapped masks"
            );
            let mut polygons = Vec::new();
            for id in &exception.exclusion_mapping_ids {
                ensure!(
                    !exception.affected_mapping_ids.contains(id),
                    "exclusion depends on its own target"
                );
                let source = recipe
                    .mappings
                    .iter()
                    .find(|mapping| &mapping.id == id)
                    .context("exclusion mask references unknown mapping")?;
                ensure!(
                    !recipe
                        .exceptions
                        .iter()
                        .any(|rule| rule.affected_mapping_ids.contains(id)),
                    "exclusion masks must refer to unmodified mappings, not exception targets"
                );
                for (_, source) in selected_features(source, features, components)? {
                    polygons.extend(
                        source
                            .parts
                            .iter()
                            .flat_map(|part| part.geometry.0.iter().cloned()),
                    );
                }
            }
            ensure!(!polygons.is_empty(), "empty mapped exclusion mask");
            let geometry = MultiPolygon(polygons);
            // Valid disjoint source components already form the required mask.
            // Union only overlapping inputs: the sweep union can round exact
            // transformed contours, leaving slivers when subtracting an island.
            if clacks_geography::geometry::validate_topology(&geometry).is_ok() {
                geometry
            } else {
                geo::unary_union(geometry.0.iter())
            }
        };
        exclusions.insert(&exception.id, geometry);
    }
    for mapping in &recipe.mappings {
        check_cancelled(cancelled)?;
        ensure!(
            !mapping.source_feature_selectors.is_empty() || !mapping.source_components.is_empty(),
            "mapping has no source selectors"
        );
        let rules: Vec<_> = recipe
            .exceptions
            .iter()
            .filter(|e| e.affected_mapping_ids.contains(&mapping.id))
            .collect();
        for (index, a) in rules.iter().enumerate() {
            for b in rules.iter().skip(index + 1) {
                let first = exclusions.get(&a.id).context("missing exclusion")?;
                let second = exclusions.get(&b.id).context("missing exclusion")?;
                ensure!(
                    first.intersection(second).0.is_empty(),
                    "overlapping exceptions for a mapping depend on input order"
                );
            }
        }
        for (identity, feature) in selected_features(mapping, features, components)? {
            for (index, part) in feature.parts.iter().enumerate() {
                let mut geometry = part.geometry.clone();
                for rule in &rules {
                    geometry = subtract(
                        &geometry,
                        exclusions.get(&rule.id).context("missing exclusion")?,
                    )
                    .with_context(|| {
                        format!("subtract exception {} from mapping {}", rule.id, mapping.id)
                    })?;
                }
                if geometry.0.is_empty() {
                    continue;
                }
                let key = format!(
                    "{}:{}:{}:{index}",
                    mapping.id, identity.source_id, identity.feature_id
                );
                let derivation = Derivation {
                    key: key.clone(),
                    source_identities: vec![identity.clone()],
                    operation_ids: part.operation_ids.clone(),
                    correction_ids: corrections.get(identity).cloned().into_iter().collect(),
                    mapping_ids: vec![mapping.id.clone()],
                    exception_ids: rules.iter().map(|rule| rule.id.clone()).collect(),
                };
                layers
                    .get_mut(&mapping.field)
                    .context("missing layer")?
                    .push(CompiledFeature {
                        part_id: key.clone(),
                        value_key: value(mapping)?.key,
                        derivation_key: key,
                        geometry,
                    });
                derivations.push(derivation);
            }
        }
    }
    for rule in &recipe.exceptions {
        if let Some(replacement) = &rule.replacement {
            ensure!(
                replacement.mapping.field
                    == recipe
                        .mappings
                        .iter()
                        .find(|m| rule.affected_mapping_ids.contains(&m.id))
                        .context("replacement target")?
                        .field,
                "replacement changes field role"
            );
            let geometry = read_geometry(
                &recipe.directory,
                &replacement.geometry_path,
                &replacement.sha256,
            )?;
            let exclusion = exclusions.get(&rule.id).context("missing exclusion")?;
            ensure!(
                exclusion.relate(&geometry).is_covers(),
                "replacement extends outside reviewed exclusion"
            );
            let key = format!("replacement:{}", rule.id);
            let source_identities: Vec<_> = recipe
                .mappings
                .iter()
                .filter(|m| rule.affected_mapping_ids.contains(&m.id))
                .flat_map(|m| {
                    m.source_feature_selectors
                        .iter()
                        .chain(
                            m.source_components
                                .iter()
                                .map(|component| &component.feature),
                        )
                        .cloned()
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            derivations.push(Derivation {
                key: key.clone(),
                source_identities,
                operation_ids: vec![],
                correction_ids: vec![],
                mapping_ids: vec![replacement.mapping.id.clone()],
                exception_ids: vec![rule.id.clone()],
            });
            layers
                .get_mut(&replacement.mapping.field)
                .context("missing replacement layer")?
                .push(CompiledFeature {
                    part_id: key.clone(),
                    value_key: value(&replacement.mapping)?.key,
                    derivation_key: key,
                    geometry,
                });
        }
    }
    Ok(MappedContents {
        layers: layers
            .into_iter()
            .map(|(field, features)| LayerInput { field, features })
            .collect(),
        values: values.into_values().collect(),
        derivations,
    })
}

/// Resolve full-feature and component selections identically for output and exclusions.
fn selected_features<'a>(
    mapping: &'a ValueMapping,
    features: &'a BTreeMap<SourceIdentity, TransformedFeature>,
    components: &'a MappingComponents,
) -> Result<Vec<(&'a SourceIdentity, &'a TransformedFeature)>> {
    let mut selected = BTreeSet::new();
    let mut result = Vec::new();
    for identity in &mapping.source_feature_selectors {
        ensure!(selected.insert(identity), "duplicate mapping selector");
        result.push((
            identity,
            features
                .get(identity)
                .context("mapping source not transformed")?,
        ));
    }
    if let Some(features) = components.get(&mapping.id) {
        for (identity, feature) in features {
            ensure!(
                selected.insert(identity),
                "mapping selects both full source and its component"
            );
            result.push((identity, feature));
        }
    }
    Ok(result)
}

/// Validate authored recorded values and produce a stable key for their scoped identity.
fn value(mapping: &ValueMapping) -> Result<ValueEntry> {
    use clacks_geography::artifact::ParentScope;
    ensure!(
        !mapping.display_label.is_empty()
            && !mapping.recordable_value.is_empty()
            && !mapping.citations.is_empty(),
        "mapping lacks value/label/evidence"
    );
    match (&mapping.field, &mapping.parent_scope) {
        (Field::Dxcc, ParentScope::Global) => {
            ensure!(
                mapping.recordable_value.parse::<u16>().is_ok_and(|v| v > 0),
                "mapping {} has an invalid DXCC value",
                mapping.id
            );
        }
        (Field::CqZone, ParentScope::Global) => {
            ensure!(
                mapping.recordable_value.parse::<u8>().is_ok_and(
                    |v| (1..=40).contains(&v) && v.to_string() == mapping.recordable_value
                ),
                "mapping {} has an invalid or noncanonical CQ zone",
                mapping.id
            );
        }
        (Field::ItuZone, ParentScope::Global) => {
            ensure!(
                mapping.recordable_value.parse::<u8>().is_ok_and(
                    |v| (1..=90).contains(&v) && v.to_string() == mapping.recordable_value
                ),
                "mapping {} has an invalid or noncanonical ITU zone",
                mapping.id
            );
        }
        (Field::PrimarySubdivision, ParentScope::Entity(entity)) => {
            ensure!(!entity.is_empty(), "empty parent entity")
        }
        (Field::SecondarySubdivision, ParentScope::Subdivision { entity, primary }) => ensure!(
            !entity.is_empty() && !primary.is_empty(),
            "empty subdivision scope"
        ),
        _ => anyhow::bail!("invalid mapped parent scope"),
    }
    let key = hex::encode(Sha256::digest(serde_json::to_vec(&(
        mapping.field,
        &mapping.parent_scope,
        &mapping.recordable_value,
    ))?));
    Ok(ValueEntry {
        key,
        field: mapping.field,
        parent_scope: mapping.parent_scope.clone(),
        recordable_value: mapping.recordable_value.clone(),
        display_label: mapping.display_label.clone(),
    })
}

/// Preserve whole island removals and disjoint components without overlay rounding.
fn subtract(
    geometry: &MultiPolygon<f64>,
    exclusion: &MultiPolygon<f64>,
) -> Result<MultiPolygon<f64>> {
    let mut remaining = Vec::new();
    for polygon in &geometry.0 {
        if exclusion.relate(polygon).is_covers() {
            continue;
        }
        if polygon.relate(exclusion).is_disjoint() {
            remaining.push(polygon.clone());
        } else {
            remaining.extend(
                overlay(
                    &MultiPolygon(vec![polygon.clone()]),
                    exclusion,
                    ClipType::Difference,
                )?
                .0,
            );
        }
    }
    Ok(MultiPolygon(remaining))
}

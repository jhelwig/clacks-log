//! Compilation of pinned geographic sources into classified artifact contents.

mod components;
mod coordinates;
mod corrections;
mod mappings;
mod normalize;
mod operations;
mod overlay;

#[cfg(test)]
mod tests;

use crate::{
    recipe::{BuildRecipe, validate_recipe},
    source::{DecodedSources, check_cancelled, decode_sources},
};
use anyhow::{Context, Result, ensure};
use clacks_geography::artifact::{
    BundleContents, Field, Manifest, ProvenanceRecord, Scope, SourceRecord,
};
use serde::Serialize;
use sha2_rsa::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::atomic::AtomicBool,
};

/// Classified artifact contents and source-operation seam diagnostics.
pub struct Compilation {
    /// Complete payload inputs ready for deterministic bundle encoding.
    pub contents: BundleContents,
    /// Measured disagreements retained without adjusting either operation.
    pub seams: Vec<SeamReport>,
}

/// Measured disagreement at sampled shared source boundary vertices.
#[derive(Clone, Debug, serde::Serialize)]
pub struct SeamReport {
    /// Source feature whose assignment pieces meet.
    pub source_identity: clacks_geography::artifact::SourceIdentity,
    /// The two exact operations, sorted by identity.
    pub operation_ids: [String; 2],
    /// Number of coincident source boundary vertices inspected.
    pub sampled_vertices: usize,
    /// Largest measured displacement; not an upper bound on unexamined edges.
    pub max_sampled_displacement_m: f64,
}

/// Compile verified cached sources without downloading or publishing a release.
#[tracing::instrument(skip_all, name = "geography.compile", level = "debug", fields())]
pub fn compile_bundle(
    recipe: &BuildRecipe,
    build_id: &str,
    cache: &Path,
    cancelled: &AtomicBool,
) -> Result<Compilation> {
    let sources = decode_sources(recipe, cache, cancelled)?;
    compile_selected(recipe, build_id, &sources, cancelled)
}

/// Compile already decoded originals while keeping all guarded interpretation at one boundary.
fn compile_selected(
    recipe: &BuildRecipe,
    build_id: &str,
    sources: &DecodedSources,
    cancelled: &AtomicBool,
) -> Result<Compilation> {
    validate_recipe(recipe)?;
    check_cancelled(cancelled)?;
    let replacements = corrections::prepare_corrections(recipe, &sources.features)?;
    let operations = operations::prepare_operations(&recipe.operations, sources)?;
    let mut transformed = BTreeMap::new();
    let mut seams = Vec::new();
    let mut correction_ids = BTreeMap::new();
    let mut closure_operations = Vec::new();
    let required: BTreeSet<_> = recipe
        .mappings
        .iter()
        .flat_map(|mapping| mapping.source_feature_selectors.iter())
        .collect();
    let uncorrected: Vec<_> = sources
        .features
        .iter()
        .filter(|feature| {
            required.contains(&feature.identity)
                && !replacements
                    .iter()
                    .any(|replacement| replacement.definition.feature == feature.identity)
        })
        .collect();
    transformed.extend(operations::transform_features(
        &uncorrected,
        &operations,
        cancelled,
    )?);
    for identity in required {
        let matching: Vec<_> = sources
            .features
            .iter()
            .filter(|feature| &feature.identity == identity)
            .collect();
        let [source] = matching.as_slice() else {
            anyhow::bail!(
                "mapping must identify exactly one selected feature: {}:{}",
                identity.source_id,
                identity.feature_id
            )
        };
        check_cancelled(cancelled)?;
        let mut feature = if let Some(replacement) = replacements
            .iter()
            .find(|r| &r.definition.feature == identity)
        {
            correction_ids.insert(identity.clone(), replacement.definition.id.clone());
            operations::TransformedFeature {
                parts: vec![operations::OperationPart {
                    operation_ids: vec![],
                    geometry: replacement.geometry.clone(),
                }],
                seams: vec![],
            }
        } else {
            transformed
                .remove(identity)
                .context("required source feature was not transformed")?
        };
        if !correction_ids.contains_key(identity)
            && let geo_types::Geometry::LineString(line) = &source.geometry
            && line.0.first() != line.0.last()
        {
            let pin = recipe
                .sources
                .iter()
                .find(|pin| pin.id == identity.source_id)
                .context("source closure pin missing")?;
            let crate::recipe::SourceKind::Zones { open_rings, .. } = &pin.kind else {
                anyhow::bail!("unreviewed open source linework")
            };
            ensure!(
                open_rings.contains(&identity.feature_id),
                "unreviewed source ring closure"
            );
            let id = format!(
                "close-source-linework:{}:{}",
                identity.source_id, identity.feature_id
            );
            ensure!(
                !recipe.operations.iter().any(|operation| operation.id == id),
                "source conversion ID collides with coordinate operation"
            );
            for part in &mut feature.parts {
                part.operation_ids.push(id.clone());
            }
            closure_operations.push(ProvenanceRecord {
                id,
                description: "Close explicitly accepted original open linework to its original first coordinate".into(),
                definition: serde_json::json!({"method": "close_original_ring", "source_identity": identity, "source_sha256": pin.archive_sha256, "feature_sha256": source.feature_sha256}),
                source_ids: vec![identity.source_id.clone()],
                citations: pin.citation_urls.clone(),
                notice_paths: pin.notice_paths.clone(),
            });
        }
        seams.extend(feature.seams.iter().cloned());
        transformed.insert(identity.clone(), feature);
    }
    ensure!(
        replacements
            .iter()
            .all(|r| transformed.contains_key(&r.definition.feature)),
        "unused correction"
    );
    let components = components::transform_components(recipe, sources, &operations, cancelled)?;
    seams.extend(
        components
            .values()
            .flat_map(|features| features.values())
            .flat_map(|feature| feature.seams.iter().cloned()),
    );
    let mapped = mappings::compile_mappings(
        recipe,
        &transformed,
        &components,
        &correction_ids,
        cancelled,
    )?;
    let mut limitations = recipe.limitations.clone();
    for seam in &seams {
        limitations.push(format!("Exact-operation seam {} / {} on {}:{}: {} shared vertices sampled; maximum observed displacement {:.6} m (not an accuracy or unsampled-edge bound). Gaps/overlaps retained under D269.",seam.operation_ids.first().context("seam operation")?,seam.operation_ids.last().context("seam operation")?,seam.source_identity.source_id,seam.source_identity.feature_id,seam.sampled_vertices,seam.max_sampled_displacement_m));
    }
    let mut seen_limits = BTreeSet::new();
    limitations.retain(|entry| seen_limits.insert(entry.clone()));
    let manifest = Manifest {
        schema_version: 1,
        release_id: build_id.to_owned(),
        recipe_sha256: hex::encode(Sha256::digest(serde_json::to_vec(recipe)?)),
        files: vec![],
        layers: vec![],
        values: mapped.values,
        derivations: mapped.derivations,
        sources: recipe
            .sources
            .iter()
            .map(|source| SourceRecord {
                id: source.id.clone(),
                publisher: source.publisher.clone(),
                upstream_version: source.upstream_version.clone(),
                sha256: source.archive_sha256.clone(),
                coordinate_declaration: source.expected_coordinate_declaration.clone(),
                citations: source.citation_urls.clone(),
                notice_paths: source.notice_paths.clone(),
            })
            .collect(),
        operations: recipe
            .operations
            .iter()
            .map(|rule| {
                provenance(
                    &rule.id,
                    &rule.description,
                    rule,
                    operation_sources(rule),
                    rule.citations.clone(),
                    vec![],
                )
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .chain(closure_operations)
            .collect(),
        corrections: recipe
            .corrections
            .iter()
            .map(|rule| {
                provenance(
                    &rule.id,
                    &rule.rationale,
                    rule,
                    vec![rule.feature.source_id.clone()],
                    rule.supporting_citations.clone(),
                    rule.notice_paths.clone(),
                )
            })
            .collect::<Result<_>>()?,
        mappings: recipe
            .mappings
            .iter()
            .chain(
                recipe
                    .exceptions
                    .iter()
                    .filter_map(|e| e.replacement.as_ref().map(|r| &r.mapping)),
            )
            .map(|rule| {
                provenance(
                    &rule.id,
                    "Explicit source-to-recorded-value mapping",
                    rule,
                    rule.source_feature_selectors
                        .iter()
                        .chain(
                            rule.source_components
                                .iter()
                                .map(|component| &component.feature),
                        )
                        .map(|id| id.source_id.clone())
                        .collect(),
                    rule.citations.clone(),
                    vec![],
                )
            })
            .collect::<Result<_>>()?,
        exceptions: recipe
            .exceptions
            .iter()
            .map(|rule| {
                provenance(
                    &rule.id,
                    &rule.rationale,
                    rule,
                    recipe
                        .mappings
                        .iter()
                        .filter(|mapping| rule.affected_mapping_ids.contains(&mapping.id))
                        .flat_map(|m| {
                            m.source_feature_selectors
                                .iter()
                                .chain(
                                    m.source_components
                                        .iter()
                                        .map(|component| &component.feature),
                                )
                                .map(|id| id.source_id.clone())
                        })
                        .collect(),
                    rule.citations.clone(),
                    rule.notice_paths.clone(),
                )
            })
            .collect::<Result<_>>()?,
        omissions: recipe.omissions.clone(),
        scope: Scope {
            description: recipe.scope.clone(),
        },
        limitations,
    };
    tracing::debug!(
        compiled_features = mapped
            .layers
            .iter()
            .map(|layer| layer.features.len())
            .sum::<usize>(),
        seam_reports = seams.len(),
        "Geographic source compilation completed"
    );

    Ok(Compilation {
        contents: BundleContents {
            manifest,
            layers: mapped.layers,
            notices: recipe.notices.clone(),
        },
        seams,
    })
}

/// Retain rule parameters once, alongside their shared descriptive provenance.
fn provenance(
    id: &str,
    description: &str,
    definition: &impl Serialize,
    mut source_ids: Vec<String>,
    citations: Vec<String>,
    notice_paths: Vec<String>,
) -> Result<ProvenanceRecord> {
    source_ids.sort();
    source_ids.dedup();
    let mut definition = serde_json::to_value(definition)?;
    if let Some(fields) = definition.as_object_mut() {
        for key in [
            "id",
            "description",
            "rationale",
            "citations",
            "supporting_citations",
            "notice_paths",
        ] {
            fields.remove(key);
        }
        fields.retain(|_, value| {
            !value.is_null()
                && value.as_str() != Some("")
                && !value.as_array().is_some_and(Vec::is_empty)
        });
    }
    Ok(ProvenanceRecord {
        id: id.into(),
        description: description.into(),
        definition,
        source_ids,
        citations,
        notice_paths,
    })
}

/// Include mask and grid input pins as well as directly transformed source pins.
fn operation_sources(rule: &crate::recipe::CoordinateOperation) -> Vec<String> {
    use crate::recipe::{Assignment, CoordinateMethod};
    let mut sources = rule.source_ids.clone();
    if let CoordinateMethod::Grid { source_id } = &rule.method {
        sources.push(source_id.clone());
    }
    if let Assignment::SourceMask {
        identities,
        source_ids,
        components,
        ..
    } = &rule.assignment
    {
        sources.extend(source_ids.iter().cloned());
        sources.extend(identities.iter().map(|id| id.source_id.clone()));
        sources.extend(
            components
                .iter()
                .map(|component| component.feature.source_id.clone()),
        );
    }
    sources
}

/// Result of checking the declared recipe against a reopened production bundle.
#[derive(Debug, PartialEq, Eq)]
pub struct AcceptanceReport {
    /// Independently specified query cases successfully checked.
    pub checked_cases: usize,
    /// Reviewed omissions carried by the matching recipe.
    pub reviewed_omissions: usize,
}

/// Check recipe identity and independent query expectations for production-validated data.
#[tracing::instrument(skip_all, name = "geography.verify", level = "debug", fields())]
pub fn verify_bundle(
    bundle: &mut clacks_geography::artifact::read::Bundle,
    recipe: &BuildRecipe,
) -> Result<AcceptanceReport> {
    use crate::recipe::{AcceptanceGeometry, ExpectedCoverage};
    use clacks_geography::lookup::{Coverage, FieldLookup};
    use clacks_model::position::{Coordinates, Locator, LookupGeometry};
    validate_recipe(recipe)?;
    ensure!(
        bundle.manifest().recipe_sha256 == hex::encode(Sha256::digest(serde_json::to_vec(recipe)?)),
        "bundle does not match resolved recipe"
    );
    ensure!(
        !recipe.acceptance_cases.is_empty(),
        "recipe has no independent acceptance cases"
    );
    let mut ids = BTreeSet::new();
    for case in &recipe.acceptance_cases {
        ensure!(
            !case.id.is_empty()
                && ids.insert(&case.id)
                && !case.expected.is_empty()
                && !case.citations.is_empty(),
            "invalid/duplicate acceptance case"
        );
        let query = match &case.query {
            AcceptanceGeometry::Point {
                latitude_microdegrees,
                longitude_microdegrees,
            } => LookupGeometry::Point(Coordinates::from_microdegrees(
                *latitude_microdegrees,
                *longitude_microdegrees,
            )?),
            AcceptanceGeometry::Grid(grid) => LookupGeometry::Area(Locator::parse(grid)?.bounds()),
        };
        let actual = bundle
            .lookup(query)
            .with_context(|| format!("acceptance query {}", case.id))?;
        for (field, expected) in &case.expected {
            let expected_coverage = match expected.coverage {
                ExpectedCoverage::Complete => Coverage::Complete,
                ExpectedCoverage::Partial => Coverage::Partial,
                ExpectedCoverage::None => Coverage::None,
            };
            let check = |coverage: Coverage, candidates: BTreeSet<_>| -> Result<()> {
                let wanted: BTreeSet<_> = expected
                    .candidates
                    .iter()
                    .map(|candidate| (candidate.parent_scope.clone(), candidate.value.clone()))
                    .collect();
                ensure!(
                    wanted.len() == expected.candidates.len(),
                    "duplicate expected candidate in {}",
                    case.id
                );
                ensure!(
                    coverage == expected_coverage && candidates == wanted,
                    "acceptance {} {field:?}: expected {expected_coverage:?} {wanted:?}, got {coverage:?} {candidates:?}",
                    case.id
                );
                Ok(())
            };
            let strings = |values: &FieldLookup<String>| {
                values
                    .candidates
                    .iter()
                    .map(|candidate| (candidate.parent_scope.clone(), candidate.value.clone()))
                    .collect()
            };
            match field {
                Field::Dxcc => check(actual.dxcc.coverage, strings(&actual.dxcc))?,
                Field::PrimarySubdivision => check(
                    actual.primary_subdivision.coverage,
                    strings(&actual.primary_subdivision),
                )?,
                Field::SecondarySubdivision => check(
                    actual.secondary_subdivision.coverage,
                    strings(&actual.secondary_subdivision),
                )?,
                Field::CqZone => check(
                    actual.cq_zone.coverage,
                    actual
                        .cq_zone
                        .candidates
                        .iter()
                        .map(|candidate| {
                            (
                                candidate.parent_scope.clone(),
                                candidate.value.get().to_string(),
                            )
                        })
                        .collect(),
                )?,
                Field::ItuZone => check(
                    actual.itu_zone.coverage,
                    actual
                        .itu_zone
                        .candidates
                        .iter()
                        .map(|candidate| {
                            (
                                candidate.parent_scope.clone(),
                                candidate.value.get().to_string(),
                            )
                        })
                        .collect(),
                )?,
            }
        }
    }
    Ok(AcceptanceReport {
        checked_cases: ids.len(),
        reviewed_omissions: recipe.omissions.len(),
    })
}

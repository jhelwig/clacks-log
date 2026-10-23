//! Verified acquisition and decoding of pinned geographic source families.

use crate::recipe::{BuildRecipe, SourceKind};
use anyhow::{Context, ensure};
use clacks_geography::artifact::SourceIdentity;
use geo::CoordsIter;
use geo_types::Geometry;
use serde_json::Value;
use sha2_rsa::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

mod archives;
mod cache;
mod shapes;
mod zones;

/// A decoded original feature, before closure, coordinate transformation or repair.
#[derive(Clone, Debug)]
pub struct SourceFeature {
    /// Original source identity selected by the recipe.
    pub identity: SourceIdentity,
    /// Original source attributes; directory or QSO information is never inserted.
    pub properties: BTreeMap<String, Value>,
    /// Original coordinate order and geometry, including guarded defects.
    pub geometry: Geometry<f64>,
    /// Original source coordinate declaration.
    pub coordinate_declaration: String,
    /// Canonical original geometry/attribute digest for reviewed replacements.
    pub feature_sha256: String,
}

/// Verified selected features and mandatory grid bytes for one compilation.
#[derive(Debug)]
pub struct DecodedSources {
    /// Selected source features in deterministic identity order.
    pub features: Vec<SourceFeature>,
    /// Verified coordinate grids, keyed by recipe source identity.
    pub grids: BTreeMap<String, Vec<u8>>,
}

/// Outcome of explicit acquisition, distinct from artifact publication.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct FetchReport {
    /// Newly downloaded and verified cache entries.
    pub installed: usize,
    /// Existing entries whose bytes were verified before reuse.
    pub reused: usize,
}

/// A requested interruption observed before a publication commit point.
#[derive(Debug, thiserror::Error)]
#[error("geographic operation cancelled before publication")]
pub struct BuildCancelled;

/// Stop at a safe processing boundary when the application requests cancellation.
pub(crate) fn check_cancelled(cancelled: &AtomicBool) -> anyhow::Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(BuildCancelled.into());
    }
    Ok(())
}

/// Acquire missing pinned inputs; existing cache entries are reverified.
///
/// This is the only source-network boundary. A failed or interrupted download
/// never installs unverified bytes or replaces an existing corrupt entry silently.
#[tracing::instrument(skip_all, name = "geography.sources.fetch", level = "info")]
pub fn fetch_sources(
    recipe: &BuildRecipe,
    cache: &Path,
    cancelled: &AtomicBool,
) -> anyhow::Result<FetchReport> {
    crate::recipe::validate_recipe(recipe)?;
    std::fs::create_dir_all(cache)?;
    let mut report = FetchReport::default();
    for pin in &recipe.sources {
        check_cancelled(cancelled)?;
        if cache::fetch(pin, cache, cancelled)
            .with_context(|| format!("fetch source {}", pin.id))?
        {
            report.installed += 1;
        } else {
            report.reused += 1;
        }
    }
    Ok(report)
}

/// Decode exact selected source features from already-verified local inputs.
///
/// No network requests are made. Changed declarations, unexpected structures,
/// selection counts and source-integrity failures stop the build.
#[tracing::instrument(skip_all, name = "geography.sources.decode", level = "debug")]
pub fn decode_sources(
    recipe: &BuildRecipe,
    cache: &Path,
    cancelled: &AtomicBool,
) -> anyhow::Result<DecodedSources> {
    crate::recipe::validate_recipe(recipe)?;
    let mut features = Vec::new();
    let mut grids = BTreeMap::new();
    let mut identities = BTreeSet::new();
    let mut coordinates = 0usize;
    for pin in &recipe.sources {
        check_cancelled(cancelled)?;
        let path =
            cache::verify(pin, cache).with_context(|| format!("verify source {}", pin.id))?;
        if matches!(pin.kind, SourceKind::Grid) {
            grids.insert(pin.id.clone(), std::fs::read(path)?);
            continue;
        }
        let selections: Vec<_> = recipe
            .selections
            .iter()
            .filter(|s| s.source_id == pin.id)
            .collect();
        ensure!(
            !selections.is_empty(),
            "source {} has no explicit selection",
            pin.id
        );
        let mut counts = vec![0usize; selections.len()];
        let mut accept = |properties: BTreeMap<String, Value>,
                          geometry: Geometry<f64>|
         -> anyhow::Result<()> {
            check_cancelled(cancelled)?;
            for (field, allowed) in &pin.attribute_domains {
                ensure!(
                    properties
                        .get(field)
                        .is_some_and(|v| allowed.contains(&property_text(v))),
                    "source {} has missing or unknown attribute domain value for {field}",
                    pin.id
                );
            }
            for (selection, count) in selections.iter().zip(&mut counts) {
                if !selection.properties.iter().all(|(field, allowed)| {
                    properties
                        .get(field)
                        .is_some_and(|v| allowed.contains(&property_text(v)))
                }) {
                    continue;
                }
                ensure!(
                    geometry
                        .coords_iter()
                        .all(|p| p.x.is_finite() && p.y.is_finite()),
                    "source {} has non-finite coordinates",
                    pin.id
                );
                let digest = hex::encode(Sha256::digest(serde_json::to_vec(
                    &serde_json::json!({"properties":properties,"geometry":geometry}),
                )?));
                if selection
                    .feature_sha256
                    .as_ref()
                    .is_some_and(|expected| expected != &digest)
                {
                    continue;
                }
                *count += 1;
                let key = if selection.identity_fields.is_empty() {
                    format!("sha256:{digest}")
                } else {
                    let keys = selection.identity_fields.iter().map(|key| {
                        properties.get(key).filter(|value|!value.is_null())
                            .map(property_text).filter(|value|!value.is_empty())
                            .with_context(||format!("source {} missing or empty identity field {key}; available fields: {:?}",pin.id,properties.keys().collect::<Vec<_>>()))
                    }).collect::<anyhow::Result<Vec<_>>>()?;
                    if let [key] = keys.as_slice() {
                        key.clone()
                    } else {
                        serde_json::to_string(&keys)?
                    }
                };
                let identity = SourceIdentity {
                    source_id: pin.id.clone(),
                    feature_id: key,
                };
                ensure!(
                    identities.insert(identity.clone()),
                    "duplicate selected source identity {identity:?}"
                );
                coordinates = coordinates
                    .checked_add(geometry.coords_count())
                    .context("coordinate count overflow")?;
                ensure!(
                    coordinates <= 20_000_000,
                    "selected geometry exceeds coordinate limit"
                );
                features.push(SourceFeature {
                    identity,
                    properties: properties.clone(),
                    geometry: geometry.clone(),
                    coordinate_declaration: pin.expected_coordinate_declaration.clone(),
                    feature_sha256: digest,
                });
            }
            Ok(())
        };
        match &pin.kind {
            SourceKind::Shapefile => {
                let expanded = archives::extract(pin, &path, cancelled)
                    .with_context(|| format!("expand source {}", pin.id))?;
                shapes::decode(pin, expanded.path(), &mut accept)
                    .with_context(|| format!("decode shapefile source {}", pin.id))?;
            }
            SourceKind::Zones {
                assignment,
                expected_features,
                open_rings,
            } => zones::decode(
                &std::fs::read(path)?,
                assignment,
                *expected_features,
                open_rings,
                &mut accept,
            )
            .with_context(|| format!("decode zone source {}", pin.id))?,
            SourceKind::Grid => (),
        }
        for (selection, count) in selections.iter().zip(counts) {
            ensure!(
                count == selection.expected_count,
                "source {} selection count {count}, expected {}",
                pin.id,
                selection.expected_count
            );
        }
    }
    features.sort_by(|a, b| a.identity.cmp(&b.identity));
    Ok(DecodedSources { features, grids })
}

/// Retain textual keys without inventing numeric normalization or label matching.
fn property_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

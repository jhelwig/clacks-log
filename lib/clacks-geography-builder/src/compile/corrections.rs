//! Guarded, confined, complete WGS84 geometry replacement inputs.

use crate::{
    recipe::{BuildRecipe, GeometryCorrection, validate_digest, validate_relative},
    source::SourceFeature,
};
use anyhow::{Context, Result, ensure};
use geo_types::{Geometry, MultiPolygon};
use sha2_rsa::{Digest, Sha256};
use std::{fs, path::Path};

/// A complete replacement retained separately from the original guarded source.
pub(super) struct Replacement<'a> {
    /// Rule whose original-input guards were verified.
    pub definition: &'a GeometryCorrection,
    /// Valid normalized target-coordinate geometry.
    pub geometry: MultiPolygon<f64>,
}

/// Verify every correction refers to exactly one selected source and an unchanged original.
pub(super) fn prepare_corrections<'a>(
    recipe: &'a BuildRecipe,
    features: &[SourceFeature],
) -> Result<Vec<Replacement<'a>>> {
    let mut seen = std::collections::BTreeSet::new();
    let mut output = Vec::new();
    for correction in &recipe.corrections {
        ensure!(
            seen.insert(&correction.feature),
            "multiple corrections target one source feature"
        );
        ensure!(
            !correction.id.is_empty()
                && !correction.rationale.is_empty()
                && !correction.supporting_citations.is_empty(),
            "correction lacks evidence"
        );
        let matches: Vec<_> = features
            .iter()
            .filter(|feature| feature.identity == correction.feature)
            .collect();
        let [feature] = matches.as_slice() else {
            anyhow::bail!("correction must identify exactly one selected feature")
        };
        let pin = recipe
            .sources
            .iter()
            .find(|pin| pin.id == correction.feature.source_id)
            .context("correction source not pinned")?;
        ensure!(
            pin.archive_sha256 == correction.expected_source_digest
                && feature.feature_sha256 == correction.expected_feature_sha256,
            "correction original-source guard mismatch"
        );
        ensure!(
            correction
                .notice_paths
                .iter()
                .all(|path| recipe.notices.contains_key(path)),
            "correction lacks notice"
        );
        let geometry = read_geometry(
            &recipe.directory,
            &correction.replacement_geojson_path,
            &correction.replacement_sha256,
        )?;
        output.push(Replacement {
            definition: correction,
            geometry,
        });
    }
    Ok(output)
}

/// Read a checksum-guarded confined regular GeoJSON geometry without following symlinks.
pub(super) fn read_geometry(
    directory: &Path,
    path: &str,
    digest: &str,
) -> Result<MultiPolygon<f64>> {
    validate_relative(path)?;
    validate_digest(digest)?;
    let mut file = directory.to_path_buf();
    ensure!(
        !fs::symlink_metadata(&file)?.file_type().is_symlink(),
        "recipe directory is a symlink"
    );
    for component in Path::new(path).components() {
        file.push(component);
        ensure!(
            !fs::symlink_metadata(&file)?.file_type().is_symlink(),
            "geometry path contains symlink"
        );
    }
    let metadata = fs::metadata(&file)?;
    ensure!(
        metadata.is_file() && metadata.len() <= 64 * 1024 * 1024,
        "invalid/oversized geometry file"
    );
    let bytes = fs::read(&file).with_context(|| format!("read guarded geometry {path}"))?;
    ensure!(
        hex::encode(Sha256::digest(&bytes)) == digest,
        "geometry digest mismatch for {path}"
    );
    let geometry: geojson::Geometry =
        serde_json::from_slice(&bytes).context("decode complete replacement geometry")?;
    let geometry: Geometry<f64> = geometry.try_into()?;
    let polygon = match geometry {
        Geometry::Polygon(p) => MultiPolygon(vec![p]),
        Geometry::MultiPolygon(p) => p,
        _ => anyhow::bail!("replacement is not polygon geometry"),
    };
    super::normalize::normalize(&polygon)
}

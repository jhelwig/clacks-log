//! Selected artifact loading with integrity checks and retained file handles.

use super::{CompiledFeature, Field, FileDigest, Manifest};
use crate::error::{GeographyError, invalid};
use anyhow::{Context, ensure};
use flatgeobuf::GeometryType;
use geo_types::Geometry;
use geozero::ToGeo;
use sha2_rsa::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{BufReader, Seek},
    path::{Component, Path, PathBuf},
};

#[cfg(test)]
mod tests;

/// An immutable release with verified metadata and retained payload handles.
///
/// Keep the release directory unchanged for this handle's lifetime. Ordinary
/// publication creates another directory rather than replacing these files.
#[derive(Debug)]
pub struct Bundle {
    /// Verified metadata bound to these handles.
    pub(crate) manifest: Manifest,
    /// Retained regular files used by indexed lookup.
    pub(crate) files: BTreeMap<Field, File>,
    /// Root used only for contextual diagnostics, never to re-resolve a release.
    pub(crate) root: PathBuf,
}

impl Bundle {
    /// Inspect the pinned release, provenance and declared coverage limits.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
}

/// Load the exact selected artifact without repeating production acceptance.
///
/// The expected manifest SHA-256 comes from trusted package selection or explicit
/// caller configuration, never from the directory being selected. Hash that same
/// manifest handle before interpreting it, check reader compatibility, verify all
/// declared payload bytes and retain their handles. Features and indexes are read
/// only as queries use them; exhaustive acceptance belongs to `validate::open_bundle`.
#[tracing::instrument(skip_all, name = "geography.bundle.open", level = "debug")]
pub fn open_bundle(root: &Path, expected_manifest_sha256: &str) -> Result<Bundle, GeographyError> {
    let expected = hex::decode(expected_manifest_sha256)
        .map_err(|_| invalid("invalid selected manifest digest"))?;
    if expected.len() != 32 {
        return Err(invalid("invalid selected manifest digest"));
    }
    let mut file = open_regular(root, "manifest.json")?;
    let mut digest = Sha256::new();
    std::io::copy(&mut file, &mut digest).map_err(|source| GeographyError::Read {
        path: root.join("manifest.json"),
        source,
    })?;
    if digest.finalize().as_slice() != expected.as_slice() {
        return Err(invalid("selected manifest digest mismatch"));
    }
    file.rewind().map_err(|source| GeographyError::Read {
        path: root.join("manifest.json"),
        source,
    })?;
    let manifest = read_manifest(root, file)?;
    open_files(root, manifest)
}

/// Decode metadata only after selection has established identity, or for production acceptance.
pub(super) fn read_manifest(root: &Path, mut file: File) -> Result<Manifest, GeographyError> {
    let manifest_path = root.join("manifest.json");
    let version: ManifestVersion =
        serde_json::from_reader(BufReader::new(&mut file)).map_err(|source| {
            GeographyError::Decode {
                path: manifest_path.clone(),
                source: source.into(),
            }
        })?;
    if version.schema_version != 1 {
        return Err(GeographyError::UnsupportedSchema(version.schema_version));
    }
    file.rewind().map_err(|source| GeographyError::Read {
        path: manifest_path.clone(),
        source,
    })?;
    serde_json::from_reader(BufReader::new(file)).map_err(|source| GeographyError::Decode {
        path: manifest_path,
        source: source.into(),
    })
}

/// Verify bytes and retain files; metadata semantics are established by the producer.
pub(super) fn open_files(root: &Path, manifest: Manifest) -> Result<Bundle, GeographyError> {
    let mut files = BTreeMap::new();
    for expected in &manifest.files {
        let mut file = open_regular(root, &expected.path)?;
        verify_file(&mut file, expected)?;
        if let Some(layer) = manifest.layers.iter().find(|l| l.path == expected.path) {
            files.insert(layer.field, file);
        }
    }
    Ok(Bundle {
        manifest,
        files,
        root: root.to_owned(),
    })
}

/// Verify an open payload without flattening filesystem errors into integrity failures.
fn verify_file(file: &mut File, expected: &FileDigest) -> Result<(), GeographyError> {
    let io_error = |source| GeographyError::Read {
        path: expected.path.clone().into(),
        source,
    };
    file.rewind().map_err(io_error)?;
    let mut hash = Sha256::new();
    let bytes = std::io::copy(&mut *file, &mut hash).map_err(io_error)?;
    if bytes != expected.bytes || hex::encode(hash.finalize()) != expected.sha256 {
        return Err(invalid(format!(
            "payload size or digest mismatch: {}",
            expected.path
        )));
    }
    file.rewind().map_err(io_error)?;
    Ok(())
}

/// Open a confined regular file, checking every path component for symlinks.
pub(super) fn open_regular(root: &Path, relative: &str) -> Result<File, GeographyError> {
    super::validate_path(relative)?;
    let mut current = root.to_path_buf();
    let mut components = Path::new(relative).components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(component) = component else {
            return Err(invalid("non-normal artifact path"));
        };
        current.push(component);
        let metadata = fs::symlink_metadata(&current).map_err(|source| GeographyError::Read {
            path: current.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(invalid("symlink in artifact path"));
        }
        if components.peek().is_none() && !metadata.is_file() {
            return Err(invalid("payload is not a regular file"));
        }
    }
    let file = File::open(&current).map_err(|source| GeographyError::Read {
        path: current.clone(),
        source,
    })?;
    if !file
        .metadata()
        .map_err(|source| GeographyError::Read {
            path: current,
            source,
        })?
        .is_file()
    {
        return Err(invalid("payload is not a regular file"));
    }
    Ok(file)
}

/// Format gate decoded independently of any particular version's remaining metadata.
#[derive(serde::Deserialize)]
struct ManifestVersion {
    /// Bundle contract revision declared by its producer.
    schema_version: u32,
}

/// Decode one validated-format feature for read-time semantic checks or lookup.
pub(crate) fn decode_feature(feature: &flatgeobuf::FgbFeature) -> anyhow::Result<CompiledFeature> {
    validate_geometry_layout(feature.geometry().context("missing geometry")?)?;
    let geometry = match feature.to_geo()? {
        Geometry::MultiPolygon(geometry) => geometry,
        _ => anyhow::bail!("feature is not a multipolygon"),
    };
    let raw = feature.fbs_feature();
    let properties = raw.properties().context("missing feature properties")?;
    let mut bytes = properties.bytes();
    let (mut part_id, mut value_key, mut derivation_key) = (None, None, None);
    while !bytes.is_empty() {
        let (column, rest) = bytes
            .split_first_chunk::<2>()
            .context("truncated property column")?;
        let (length, rest) = rest
            .split_first_chunk::<4>()
            .context("truncated property length")?;
        let length = usize::try_from(u32::from_le_bytes(*length))?;
        let (text, rest) = rest
            .split_at_checked(length)
            .context("property exceeds feature buffer")?;
        let slot = match u16::from_le_bytes(*column) {
            0 => &mut part_id,
            1 => &mut value_key,
            2 => &mut derivation_key,
            _ => anyhow::bail!("unsupported feature property column"),
        };
        ensure!(slot.is_none(), "duplicate feature property");
        *slot = Some(std::str::from_utf8(text)?.to_owned());
        bytes = rest;
    }
    Ok(CompiledFeature {
        part_id: part_id.context("missing part ID")?,
        value_key: value_key.context("missing value key")?,
        derivation_key: derivation_key.context("missing derivation key")?,
        geometry,
    })
}

/// Guard vector/ring lengths before the geometry decoder uses indexed coordinates.
fn validate_geometry_layout(geometry: flatgeobuf::Geometry<'_>) -> anyhow::Result<()> {
    ensure!(
        geometry.xy().is_none() && geometry.ends().is_none(),
        "multipolygon has unexpected coordinate vectors"
    );
    ensure!(
        geometry.z().is_none()
            && geometry.m().is_none()
            && geometry.t().is_none()
            && geometry.tm().is_none(),
        "unexpected feature dimensions"
    );
    let parts = geometry.parts().context("missing multipolygon parts")?;
    ensure!(!parts.is_empty(), "empty multipolygon");
    for part in parts.iter() {
        ensure!(
            part.parts().is_none()
                && matches!(part.type_(), GeometryType::Unknown | GeometryType::Polygon),
            "unsupported nested geometry"
        );
        ensure!(
            part.z().is_none() && part.m().is_none() && part.t().is_none() && part.tm().is_none(),
            "unexpected polygon dimensions"
        );
        let xy: Vec<_> = part
            .xy()
            .context("missing polygon coordinates")?
            .iter()
            .collect();
        let (pairs, rest) = xy.as_chunks::<2>();
        ensure!(rest.is_empty(), "odd polygon coordinate count");
        let ends: Vec<usize> = match part.ends() {
            Some(ends) if !ends.is_empty() => {
                ends.iter().map(usize::try_from).collect::<Result<_, _>>()?
            }
            _ => vec![pairs.len()],
        };
        let mut start = 0;
        for end in ends {
            let ring = pairs
                .get(start..end)
                .context("invalid polygon ring offsets")?;
            ensure!(
                ring.len() >= 4 && ring.first() == ring.last(),
                "unclosed or undersized ring"
            );
            start = end;
        }
        ensure!(start == pairs.len(), "unreferenced polygon coordinates");
    }
    Ok(())
}

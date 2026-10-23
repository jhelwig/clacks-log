//! Fully validated bundle opening with pinned file handles.

use super::{CompiledFeature, Field, FileDigest, Manifest};
use crate::{
    error::{GeographyError, invalid},
    geometry,
};
use anyhow::{Context, ensure};
use flatgeobuf::{ColumnType, FallibleStreamingIterator, FgbReader, GeometryType};
use geo_types::Geometry;
use geozero::ToGeo;
use sha2_rsa::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{BufReader, Read, Seek, SeekFrom},
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

/// Open and validate every declared payload before permitting queries.
///
/// Rejects unsupported schemas, integrity or semantic violations, symlink paths
/// and failed reads. An empty valid layer is distinct from a missing payload.
#[tracing::instrument(skip_all, name = "geography.bundle.open", level = "debug")]
pub fn open_bundle(root: &Path) -> Result<Bundle, GeographyError> {
    let manifest_path = root.join("manifest.json");
    let mut file = open_regular(root, "manifest.json")?;
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
    let manifest: Manifest =
        serde_json::from_reader(BufReader::new(file)).map_err(|source| GeographyError::Decode {
            path: manifest_path,
            source: source.into(),
        })?;
    manifest.validate()?;
    let mut files = BTreeMap::new();
    for expected in &manifest.files {
        let mut file = open_regular(root, &expected.path)?;
        verify_file(&mut file, expected)?;
        if let Some(layer) = manifest.layers.iter().find(|l| l.path == expected.path) {
            validate_layer(&mut file, layer, &manifest).map_err(|source| {
                GeographyError::Decode {
                    path: root.join(&expected.path),
                    source,
                }
            })?;
            file.rewind().map_err(|source| GeographyError::Read {
                path: root.join(&expected.path),
                source,
            })?;
            files.insert(layer.field, file);
        }
    }
    let expected: BTreeSet<_> = manifest
        .files
        .iter()
        .map(|f| f.path.clone())
        .chain(std::iter::once("manifest.json".into()))
        .collect();
    validate_inventory(root, root, &expected)?;
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
fn open_regular(root: &Path, relative: &str) -> Result<File, GeographyError> {
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

/// Reject undeclared payload files and nonregular entries in an otherwise valid bundle.
fn validate_inventory(
    root: &Path,
    directory: &Path,
    expected: &BTreeSet<String>,
) -> Result<(), GeographyError> {
    for entry in fs::read_dir(directory).map_err(|source| GeographyError::Read {
        path: directory.into(),
        source,
    })? {
        let entry = entry.map_err(|source| GeographyError::Read {
            path: directory.into(),
            source,
        })?;
        let path = entry.path();
        let kind = entry.file_type().map_err(|source| GeographyError::Read {
            path: path.clone(),
            source,
        })?;
        if kind.is_symlink() {
            return Err(invalid("symlink in bundle inventory"));
        }
        if kind.is_dir() {
            validate_inventory(root, &path, expected)?;
        } else {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| invalid("inventory outside bundle"))?
                .to_string_lossy()
                .replace('\\', "/");
            if !kind.is_file() || !expected.contains(&relative) {
                return Err(invalid(format!("undeclared bundle entry {relative}")));
            }
        }
    }
    Ok(())
}

/// Format gate decoded independently of any particular version's remaining metadata.
#[derive(serde::Deserialize)]
struct ManifestVersion {
    /// Bundle contract revision declared by its producer.
    schema_version: u32,
}

/// Check the header, every decoded feature and aggregate bounds against metadata.
fn validate_layer(
    file: &mut File,
    layer: &super::LayerDescriptor,
    manifest: &Manifest,
) -> anyhow::Result<()> {
    let framing = scan_frames(file, layer.feature_count)?;
    file.rewind()?;
    let file_bytes = file.metadata()?.len();
    let reader = FgbReader::open(BufReader::new(&mut *file))?;
    let header = reader.header();
    ensure!(
        header.geometry_type() == GeometryType::MultiPolygon
            && !header.has_z()
            && !header.has_m()
            && !header.has_t()
            && !header.has_tm(),
        "unsupported layer geometry or dimensions"
    );
    ensure!(
        header.features_count() == layer.feature_count,
        "feature count differs from manifest"
    );
    ensure!(
        layer.feature_count <= file_bytes / 4,
        "impossible feature count"
    );
    ensure!(
        layer.feature_count == 0 || header.index_node_size() == 16,
        "nonempty layer lacks spatial index"
    );
    let crs = header.crs().context("missing layer coordinate metadata")?;
    ensure!(
        crs.org() == Some("OGC") && crs.code_string() == Some("CRS84"),
        "layer coordinate metadata mismatch"
    );
    let columns = header.columns().context("missing columns")?;
    ensure!(columns.len() == 3, "unexpected feature columns");
    for (column, name) in columns
        .iter()
        .zip(["part_id", "value_key", "derivation_key"])
    {
        ensure!(
            column.name() == name && column.type_() == ColumnType::String,
            "unsupported property column"
        );
    }
    if layer.feature_count > 0 {
        let envelope = header.envelope().context("missing header bounds")?;
        ensure!(
            Some(envelope.iter().collect::<Vec<_>>())
                == layer.extent_or_empty.map(|v| v.into_iter().collect()),
            "header extent differs from manifest"
        );
    }
    let mut features = reader.select_all_seq()?;
    let mut count = 0u64;
    let mut extent = None;
    let mut parts = BTreeSet::new();
    let mut feature_extents = Vec::new();
    while let Some(feature) = features.next()? {
        let row = decode_feature(feature)?;
        ensure!(
            parts.insert(row.part_id.clone()) && !row.part_id.is_empty(),
            "duplicated or empty part ID"
        );
        let value = manifest
            .values
            .iter()
            .find(|v| v.key == row.value_key)
            .context("missing feature value")?;
        ensure!(
            value.field == layer.field,
            "feature value has wrong layer role"
        );
        ensure!(
            manifest
                .derivations
                .iter()
                .any(|d| d.key == row.derivation_key),
            "missing feature derivation"
        );
        let bounds = geometry::validate(&row.geometry)?;
        feature_extents.push(bounds);
        geometry::extend(&mut extent, bounds);
        count += 1;
    }
    ensure!(
        count == layer.feature_count && extent == layer.extent_or_empty,
        "decoded count/extent differs from manifest"
    );
    drop(features);
    validate_index(file, &feature_extents, extent, &framing)?;
    Ok(())
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

/// Checked feature framing independent of FlatBuffers feature allocation.
struct FeatureFraming {
    /// Byte position of the packed spatial index.
    index_start: u64,
    /// Expected packed index byte length.
    index_size: usize,
    /// Feature positions relative to the beginning of the feature section.
    offsets: Vec<u64>,
}

/// Bound every size-prefixed frame against the file before passing it to the dependency.
fn scan_frames(file: &mut File, count: u64) -> anyhow::Result<FeatureFraming> {
    use flatgeobuf::packed_r_tree::PackedRTree;
    let file_length = file.metadata()?.len();
    ensure!(count <= file_length / 4, "impossible frame count");
    let count = usize::try_from(count)?;
    count
        .checked_mul(80)
        .context("packed index size overflow")?;
    file.seek(SeekFrom::Start(8))?;
    let mut size = [0u8; 4];
    file.read_exact(&mut size)?;
    let header_size = u32::from_le_bytes(size);
    ensure!(header_size >= 8, "undersized layer header");
    let index_start = 12 + u64::from(header_size);
    let index_size = if count == 0 {
        0
    } else {
        PackedRTree::index_size(count, 16)
    };
    let feature_start = index_start
        .checked_add(u64::try_from(index_size)?)
        .context("index size overflow")?;
    ensure!(feature_start <= file_length, "index exceeds file length");
    file.seek(SeekFrom::Start(feature_start))?;
    let mut offsets = Vec::new();
    for _ in 0..count {
        offsets.push(
            file.stream_position()?
                .checked_sub(feature_start)
                .context("feature offset underflow")?,
        );
        file.read_exact(&mut size)?;
        let length = u32::from_le_bytes(size);
        ensure!(length >= 8, "undersized feature frame");
        let end = file
            .stream_position()?
            .checked_add(u64::from(length))
            .context("feature length overflow")?;
        ensure!(end <= file_length, "feature exceeds file length");
        file.seek(SeekFrom::Start(end))?;
    }
    ensure!(
        file.stream_position()? == file_length,
        "trailing or missing feature bytes"
    );
    Ok(FeatureFraming {
        index_start,
        index_size,
        offsets,
    })
}

/// Rebuild the canonical packed index from decoded bounds and compare its encoded nodes.
fn validate_index(
    file: &mut File,
    bounds: &[[f64; 4]],
    extent: Option<[f64; 4]>,
    framing: &FeatureFraming,
) -> anyhow::Result<()> {
    use flatgeobuf::packed_r_tree::{NodeItem, PackedRTree};
    ensure!(
        bounds.len() == framing.offsets.len(),
        "decoded frame count mismatch"
    );
    let nodes: Vec<_> = bounds
        .iter()
        .zip(&framing.offsets)
        .map(|(&[west, south, east, north], &offset)| {
            let mut node = NodeItem::bounds(west, south, east, north);
            node.offset = offset;
            node
        })
        .collect();
    if let Some([west, south, east, north]) = extent {
        let tree = PackedRTree::build(&nodes, &NodeItem::bounds(west, south, east, north), 16)?;
        let mut expected = Vec::new();
        tree.stream_write(&mut expected)?;
        file.seek(SeekFrom::Start(framing.index_start))?;
        let mut actual = vec![0; framing.index_size];
        file.read_exact(&mut actual)?;
        ensure!(
            actual == expected,
            "spatial index differs from decoded feature bounds/offsets"
        );
    }
    Ok(())
}

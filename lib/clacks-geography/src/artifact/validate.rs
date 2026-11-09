//! Exhaustive artifact acceptance performed during geographic data production.

use super::{
    Manifest,
    read::{Bundle, decode_feature, open_files, open_regular, read_manifest},
};
use crate::{
    error::{GeographyError, invalid},
    geometry,
};
use anyhow::{Context, ensure};
use flatgeobuf::{ColumnType, FallibleStreamingIterator, FgbReader, GeometryType};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{BufReader, Read, Seek, SeekFrom},
    path::Path,
};

/// Open unselected production output and validate all metadata, features and inventory.
///
/// Build, explicit verification and archival use this operation before retaining a
/// release. Application loading uses `read::open_bundle` with an independently
/// selected manifest identity instead of repeating these acceptance checks.
#[tracing::instrument(skip_all, name = "geography.bundle.validate", level = "debug")]
pub fn open_bundle(root: &Path) -> Result<Bundle, GeographyError> {
    let manifest = read_manifest(root, open_regular(root, "manifest.json")?)?;
    manifest.validate()?;
    let mut bundle = open_files(root, manifest)?;
    for layer in &bundle.manifest.layers {
        let file = bundle
            .files
            .get_mut(&layer.field)
            .ok_or_else(|| crate::error::invalid("missing layer handle"))?;
        validate_layer(file, layer, &bundle.manifest).map_err(|source| GeographyError::Decode {
            path: root.join(&layer.path),
            source,
        })?;
    }
    let expected = bundle
        .manifest
        .files
        .iter()
        .map(|f| f.path.clone())
        .chain(std::iter::once("manifest.json".into()))
        .collect();
    validate_inventory(root, root, &expected)?;
    Ok(bundle)
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

//! Deterministic encoding of complete compiled bundles into private staging directories.

use super::{
    BundleContents, CompiledFeature, Field, FileDigest, LayerDescriptor, Manifest, validate_path,
};
use crate::geometry;
use anyhow::{Context, ensure};
use flatgeobuf::{ColumnType, FgbCrs, FgbWriter, FgbWriterOptions, GeometryType};
use geo::{Orient, orient::Direction};
use geo_types::Geometry;
use geozero::{ColumnValue, FeatureProcessor, GeozeroGeometry, PropertyProcessor};
use sha2_rsa::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{BufWriter, Write},
    path::Path,
};

/// Encode a complete bundle into a new or empty private directory.
///
/// The returned manifest includes generated payload hashes and bounds. This does
/// not select a release for readers or overwrite an existing nonempty directory.
/// Invalid metadata, geometry and filesystem failures retain their error causes.
#[tracing::instrument(skip_all, name="geography.bundle.write", level="debug", fields(release_id = %contents.manifest.release_id))]
pub fn write_bundle(root: &Path, contents: &BundleContents) -> anyhow::Result<Manifest> {
    if root.exists() {
        ensure!(
            fs::read_dir(root)?.next().is_none(),
            "bundle destination is not empty"
        );
    } else {
        fs::create_dir_all(root)?;
    }
    let mut manifest = contents.manifest.clone();
    manifest.files.clear();
    manifest.layers.clear();
    let roles: BTreeSet<_> = contents.layers.iter().map(|l| l.field).collect();
    ensure!(
        contents.layers.len() == 5 && roles == Field::ALL.into_iter().collect(),
        "exactly five layer roles required"
    );
    fs::create_dir(root.join("layers"))?;
    for field in Field::ALL {
        let layer = contents
            .layers
            .iter()
            .find(|l| l.field == field)
            .context("missing layer role")?;
        let mut extent = None;
        let mut parts = BTreeSet::new();
        for feature in &layer.features {
            ensure!(
                !feature.part_id.is_empty() && parts.insert(&feature.part_id),
                "empty or duplicated part identity"
            );
            let value = manifest
                .values
                .iter()
                .find(|v| v.key == feature.value_key)
                .context("missing value reference")?;
            ensure!(value.field == field, "value belongs to a different field");
            ensure!(
                manifest
                    .derivations
                    .iter()
                    .any(|d| d.key == feature.derivation_key),
                "missing derivation reference"
            );
            geometry::extend(&mut extent, geometry::validate(&feature.geometry)?);
        }
        let path = field.path();
        let mut writer = FgbWriter::create_with_options(
            path,
            GeometryType::MultiPolygon,
            FgbWriterOptions {
                write_index: true,
                detect_type: false,
                promote_to_multi: false,
                crs: FgbCrs {
                    org: Some("OGC"),
                    code_string: Some("CRS84"),
                    name: Some("WGS84 longitude latitude"),
                    ..Default::default()
                },
                ..Default::default()
            },
        )?;
        for name in ["part_id", "value_key", "derivation_key"] {
            writer.add_column(name, ColumnType::String, |_, _| {});
        }
        let mut features: Vec<_> = layer.features.iter().collect();
        features.sort_by(|a, b| a.part_id.cmp(&b.part_id));
        for (index, feature) in features.into_iter().enumerate() {
            write_feature(&mut writer, index as u64, feature)?;
        }
        let mut output = BufWriter::new(File::create(root.join(path))?);
        writer.write(&mut output)?;
        output.flush()?;
        output.get_ref().sync_all()?;
        manifest.files.push(digest_file(root, path)?);
        manifest.layers.push(LayerDescriptor {
            field,
            path: path.into(),
            feature_count: layer.features.len() as u64,
            extent_or_empty: extent,
        });
    }
    for (path, text) in &contents.notices {
        validate_path(path)?;
        ensure!(
            path.starts_with("notices/"),
            "notice outside notices directory"
        );
        let destination = root.join(path);
        fs::create_dir_all(destination.parent().context("notice parent")?)?;
        let mut output = File::create(&destination)?;
        output.write_all(text.as_bytes())?;
        output.sync_all()?;
        manifest.files.push(digest_file(root, path)?);
    }
    manifest.files.sort_by(|a, b| a.path.cmp(&b.path));
    manifest.values.sort_by(|a, b| a.key.cmp(&b.key));
    manifest.derivations.sort_by(|a, b| a.key.cmp(&b.key));
    manifest.validate()?;
    let mut output = File::create(root.join("manifest.json"))?;
    serde_json::to_writer_pretty(&mut output, &manifest)?;
    output.write_all(b"\n")?;
    output.sync_all()?;
    Ok(manifest)
}

/// Encode fixed properties with fallible processor calls rather than a nonfallible configuration closure.
fn write_feature(
    writer: &mut FgbWriter<'_>,
    index: u64,
    feature: &CompiledFeature,
) -> anyhow::Result<()> {
    writer.feature_begin(index)?;
    writer.properties_begin()?;
    for (column, (name, value)) in [
        ("part_id", &feature.part_id),
        ("value_key", &feature.value_key),
        ("derivation_key", &feature.derivation_key),
    ]
    .into_iter()
    .enumerate()
    {
        writer.property(column, name, &ColumnValue::String(value))?;
    }
    writer.properties_end()?;
    writer.geometry_begin()?;
    Geometry::MultiPolygon(feature.geometry.orient(Direction::Default)).process_geom(writer)?;
    writer.geometry_end()?;
    writer.feature_end(index)?;
    Ok(())
}

/// Compute a streaming digest without depending on file-buffer allocation size.
pub(crate) fn digest_file(root: &Path, path: &str) -> anyhow::Result<FileDigest> {
    let mut file = File::open(root.join(path)).with_context(|| format!("open payload {path}"))?;
    let mut hasher = Sha256::new();
    let bytes = std::io::copy(&mut file, &mut hasher)?;
    Ok(FileDigest {
        path: path.into(),
        bytes,
        sha256: hex::encode(hasher.finalize()),
    })
}

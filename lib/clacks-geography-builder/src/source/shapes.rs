//! Streaming shapefile/attribute decoding shared by Census and Natural Earth inputs.

use crate::recipe::SourcePin;
use anyhow::{Context, ensure};
use geo_types::Geometry;
use serde_json::Value;
use shapefile::{Shape, dbase};
use std::{collections::BTreeMap, fs, path::Path};

/// Verify sidecar declarations and preserve source geometry/attributes without simplification.
pub(super) fn decode(
    pin: &SourcePin,
    directory: &Path,
    accept: &mut impl FnMut(BTreeMap<String, Value>, Geometry<f64>) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let shapes: Vec<_> = pin
        .archive_members
        .iter()
        .filter(|p| p.ends_with(".shp"))
        .collect();
    let [shape_path] = shapes.as_slice() else {
        anyhow::bail!("source must declare exactly one shapefile");
    };
    let path = directory.join(shape_path);
    let declaration = fs::read_to_string(path.with_extension("prj"))?;
    ensure!(
        declaration.trim() == pin.expected_coordinate_declaration.trim(),
        "source coordinate declaration changed"
    );
    ensure!(
        pin.text_encoding == "UTF-8",
        "unsupported strict shapefile text encoding"
    );
    let cpg = path.with_extension("cpg");
    if cpg.exists() {
        let encoding = fs::read_to_string(cpg)?;
        ensure!(
            matches!(
                encoding.trim().to_ascii_uppercase().as_str(),
                "UTF-8" | "UTF8" | "65001"
            ),
            "source encoding declaration changed"
        );
    }
    let shape_reader = shapefile::ShapeReader::from_path(&path)?;
    let shape_count = shape_reader.shape_count()?;
    let dbf_reader = dbase::ReaderBuilder::new()
        .with_encoding(dbase::encoding::Unicode)
        .open(path.with_extension("dbf"))?;
    ensure!(
        shape_count == usize::try_from(dbf_reader.header().num_records)?,
        "shape/attribute record counts differ"
    );
    validate_attribute_rows(&path.with_extension("dbf"), &dbf_reader)?;
    let mut reader = shapefile::Reader::new(shape_reader, dbf_reader);
    let mut decoded = 0;
    for row in reader.iter_shapes_and_records() {
        let (shape, record) = row?;
        let Shape::Polygon(polygon) = shape else {
            anyhow::bail!("unexpected source shape kind");
        };
        let geometry = Geometry::MultiPolygon(polygon.try_into()?);
        let properties = record
            .into_iter()
            .map(|(name, value)| Ok((name, attribute(value)?)))
            .collect::<anyhow::Result<BTreeMap<_, _>>>()?;
        accept(properties, geometry)?;
        decoded += 1;
    }
    ensure!(
        decoded == shape_count,
        "decoded shape/attribute count differs from source header"
    );
    Ok(())
}

/// Reject deleted/incomplete attribute rows before an iterator could shift shape correspondence.
fn validate_attribute_rows(
    path: &Path,
    reader: &dbase::Reader<impl std::io::Read + std::io::Seek>,
) -> anyhow::Result<()> {
    use std::io::{Read, Seek, SeekFrom};
    let header = reader.header();
    ensure!(
        !header.is_transaction_incomplete
            && header.encryption_flag == 0
            && header.size_of_record > 0,
        "unsupported attribute table state"
    );
    let mut file = std::fs::File::open(path)?;
    let first = u64::from(header.offset_to_first_record);
    let width = u64::from(header.size_of_record);
    let end = first
        .checked_add(
            u64::from(header.num_records)
                .checked_mul(width)
                .context("attribute table size overflow")?,
        )
        .context("attribute table offset overflow")?;
    ensure!(end <= file.metadata()?.len(), "truncated attribute table");
    for row in 0..header.num_records {
        file.seek(SeekFrom::Start(first + u64::from(row) * width))?;
        let mut flag = [0u8; 1];
        file.read_exact(&mut flag)?;
        ensure!(flag == *b" ", "deleted or invalid attribute row {row}");
    }
    Ok(())
}

/// Preserve supported source attribute values with strict numeric finiteness checks.
fn attribute(value: dbase::FieldValue) -> anyhow::Result<Value> {
    use dbase::FieldValue;
    Ok(match value {
        FieldValue::Character(value) => value.map(Value::String).unwrap_or(Value::Null),
        FieldValue::Numeric(value) => match value {
            Some(value) => serde_json::Number::from_f64(value)
                .map(Value::Number)
                .context("non-finite numeric source attribute")?,
            None => Value::Null,
        },
        FieldValue::Float(value) => match value {
            Some(value) => serde_json::Number::from_f64(f64::from(value))
                .map(Value::Number)
                .context("non-finite float source attribute")?,
            None => Value::Null,
        },
        FieldValue::Logical(value) => value.map(Value::Bool).unwrap_or(Value::Null),
        FieldValue::Integer(value) => value.into(),
        FieldValue::Double(value) | FieldValue::Currency(value) => {
            serde_json::Number::from_f64(value)
                .map(Value::Number)
                .context("non-finite source attribute")?
        }
        FieldValue::Date(value) => value
            .map(|value| Value::String(value.to_string()))
            .unwrap_or(Value::Null),
        FieldValue::Memo(value) => Value::String(value),
        FieldValue::DateTime(_) => anyhow::bail!("unsupported source date-time attribute"),
    })
}

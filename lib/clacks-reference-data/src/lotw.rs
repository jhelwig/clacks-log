//! Extraction of pinned LoTW reference data without executing reference code.

use anyhow::{Context, ensure};
use regex::Regex;
use roxmltree::{Document, Node};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::{fs, path::Path};

#[cfg(test)]
mod tests;

/// Source revision shared by both derived compatibility datasets.
const UPSTREAM_COMMIT: &str = "6ce824363e27da455c6cf6fd440b9c401945bfcb";

/// Check or regenerate both derived files from read-only pinned inputs.
#[tracing::instrument(
    skip_all,
    name = "maintenance.lotw_data",
    level = "debug",
    fields(check)
)]
pub(super) fn update(check: bool) -> anyhow::Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .context("maintenance crate must be under lib/")?;
    let xml = fs::read_to_string(root.join("references/trustedqsl/upstream/src/config.xml"))?;
    let source = fs::read_to_string(root.join("references/trustedqsl/upstream/src/location.cpp"))?;
    let (model, signatures) = render(&xml, &source)?;
    for (relative, contents) in [
        ("lib/clacks-model/data/lotw-11.34.json", model),
        ("lib/clacks-lotw/data/signatures-11.34.json", signatures),
    ] {
        let target = root.join(relative);
        if check {
            ensure!(
                fs::read_to_string(&target).with_context(|| format!("read {relative}"))?
                    == contents,
                "derived data differs from pinned source: {relative}"
            );
        } else {
            fs::write(&target, contents).with_context(|| format!("write {relative}"))?;
        }
    }
    tracing::debug!(
        event = "maintenance.lotw_data.completed",
        check,
        file_count = 2
    );
    println!("LoTW normalization and signature data match the pinned sources.");
    Ok(())
}

/// Extract deterministic normalization and ordered signature-definition JSON.
fn render(xml: &str, source: &str) -> anyhow::Result<(String, String)> {
    let document = Document::parse(xml).context("parse pinned configuration XML")?;
    let root = document.root_element();
    let release = format!(
        "{}.{}",
        attribute(root, "majorversion")?,
        attribute(root, "minorversion")?
    );
    ensure!(
        release == "11.34",
        "unexpected pinned configuration release {release}"
    );
    let mut mapping = Map::new();
    for row in rows(section(root, "adifmap")?, "adifmode") {
        let label = text(row);
        let parent = row.attribute("adif-mode").unwrap_or(label);
        let sub = row.attribute("adif-submode").unwrap_or_default();
        if let Some(mode) = row.attribute("mode").filter(|m| !m.is_empty()) {
            for key in [
                parent.to_owned(),
                sub.to_owned(),
                label.to_owned(),
                if !parent.is_empty() && !sub.is_empty() {
                    format!("{parent}%{sub}")
                } else {
                    String::new()
                },
            ] {
                if !key.is_empty() {
                    mapping.insert(key.to_uppercase(), json!(mode.to_uppercase()));
                }
            }
        }
    }
    let mut fields = Map::new();
    for field in rows(section(root, "locfields")?, "field") {
        let mut values = Vec::new();
        for enums in rows(field, "enums") {
            for value in rows(enums, "enum") {
                values.push(json!({
                    "dependency": enums.attribute("dependency"),
                    "value": value.attribute("value").unwrap_or_default(),
                    "label": text(value),
                    "zones": zones(value.attribute("zonemap"))?,
                }));
            }
        }
        fields.insert(
            attribute(field, "Id")?.into(),
            json!({
                "length": field.attribute("len").unwrap_or("0").parse::<u64>()?,
                "flags": field.attribute("flags").unwrap_or("0").parse::<u64>()?,
                "depends_on": field.attribute("dependsOn"),
                "values": values,
            }),
        );
    }
    let mut entity_fields: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for page in rows(section(root, "locpages")?, "page") {
        if page.attribute("dependsOn") == Some("DXCC") {
            let selected = entity_fields
                .entry(attribute(page, "dependency")?.into())
                .or_default();
            selected.extend(
                rows(page, "pageField")
                    .map(text)
                    .filter(|name| fields.contains_key(*name))
                    .map(str::to_owned),
            );
        }
    }
    let mut modes = Map::new();
    for mode in rows(section(root, "modes")?, "mode") {
        modes.insert(text(mode).into(), json!(mode.attribute("group")));
    }
    let mut bands = Vec::new();
    for band in rows(section(root, "bands")?, "band") {
        bands.push(json!({"label": text(band), "lower_hz": hertz(attribute(band, "low")?)?, "upper_hz": hertz(attribute(band, "high")?)?}));
    }
    let mut entity_zones = Map::new();
    for entity in rows(section(root, "dxcc")?, "entity") {
        entity_zones.insert(
            attribute(entity, "arrlId")?.into(),
            json!(zones(entity.attribute("zonemap"))?),
        );
    }
    let specifications = section(root, "sigspecs")?;
    let first = rows(specifications, "sigspec")
        .next()
        .context("missing signature definitions")?;
    let station_fields: Vec<_> = section(first, "tSTATION")?
        .children()
        .filter(Node::is_element)
        .map(|n| n.tag_name().name())
        .collect();
    let primary_fields = primary_fields(source)?;
    let model = json!({
        "release": release, "upstream_commit": UPSTREAM_COMMIT, "mapping": mapping,
        "modes": modes, "bands": bands, "fields": fields, "primary_fields": primary_fields,
        "secondary_fields": ["US_COUNTY", "JA_CITY_GUN_KU"],
        "entity_zones": entity_zones, "entity_fields": entity_fields, "station_fields": station_fields,
    });
    let mut definitions = Vec::new();
    for definition in rows(specifications, "sigspec") {
        definitions.push(SignatureDefinition {
            name: attribute(definition, "name")?.into(),
            version: attribute(definition, "version")?.into(),
            deprecated: definition.attribute("status") == Some("deprecated"),
            station: signing_fields(section(definition, "tSTATION")?),
            contact: signing_fields(section(definition, "tCONTACT")?),
        });
    }
    let signatures = Signatures {
        release,
        upstream_commit: UPSTREAM_COMMIT,
        definitions,
    };
    Ok((
        format!("{}\n", serde_json::to_string_pretty(&model)?),
        format!("{}\n", serde_json::to_string_pretty(&signatures)?),
    ))
}

/// One source-derived signature file, preserving the documented JSON field order.
#[derive(Serialize)]
struct Signatures {
    /// Configuration release paired with these definitions.
    release: String,
    /// Revision of the read-only upstream source.
    upstream_commit: &'static str,
    /// Ordered signature specifications.
    definitions: Vec<SignatureDefinition>,
}

/// Source-defined station and contact signing order for one protocol version.
#[derive(Serialize)]
struct SignatureDefinition {
    /// Signing protocol name.
    name: String,
    /// Major/minor version spelling from XML.
    version: String,
    /// Whether the source excludes this definition from selection.
    deprecated: bool,
    /// Ordered station signing fields.
    station: Vec<SigningField>,
    /// Ordered contact signing fields.
    contact: Vec<SigningField>,
}

/// A field's signing position and source representation hint.
#[derive(Serialize)]
struct SigningField {
    /// Protocol field name.
    name: String,
    /// Whether the source requires a value.
    required: bool,
    /// Optional source-supplied conversion method.
    method: Option<String>,
}

/// Preserve every signing field in source order without protocol-specific filtering.
fn signing_fields(node: Node<'_, '_>) -> Vec<SigningField> {
    node.children()
        .filter(Node::is_element)
        .map(|field| SigningField {
            name: field.tag_name().name().into(),
            required: field.attribute("required") == Some("1"),
            method: field.attribute("method").map(str::to_owned),
        })
        .collect()
}

/// Read the required named child section, reporting a missing source contract.
fn section<'a, 'input>(node: Node<'a, 'input>, name: &str) -> anyhow::Result<Node<'a, 'input>> {
    node.children()
        .find(|n| n.has_tag_name(name))
        .with_context(|| format!("missing XML section {name}"))
}

/// Iterate immediate child rows without searching nested or unrelated sections.
fn rows<'a, 'input>(
    node: Node<'a, 'input>,
    name: &'a str,
) -> impl Iterator<Item = Node<'a, 'input>> {
    node.children().filter(move |n| n.has_tag_name(name))
}

/// Read an attribute whose absence would make the derived data incomplete.
fn attribute<'a, 'input>(node: Node<'a, 'input>, name: &str) -> anyhow::Result<&'a str> {
    node.attribute(name)
        .with_context(|| format!("missing XML attribute {name}"))
}

/// Interpret a source text value with its documented boundary whitespace removed.
fn text<'a, 'input>(node: Node<'a, 'input>) -> &'a str {
    node.text().unwrap_or_default().trim()
}

/// Decode comma-separated ITU:CQ pairs without dropping malformed values.
fn zones(value: Option<&str>) -> anyhow::Result<Vec<Value>> {
    value
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|pair| {
            let (itu, cq) = pair.split_once(':').context("zone pair must be ITU:CQ")?;
            Ok(json!({"itu": itu.parse::<u32>()?, "cq": cq.parse::<u32>()?}))
        })
        .collect()
}

/// Convert the source's decimal kilohertz values exactly into whole hertz.
fn hertz(value: &str) -> anyhow::Result<u64> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    ensure!(
        !whole.is_empty()
            && whole.bytes().all(|b| b.is_ascii_digit())
            && fraction.len() <= 3
            && fraction.bytes().all(|b| b.is_ascii_digit()),
        "invalid whole-hertz band bound {value}"
    );
    let fraction = format!("{fraction:0<3}").parse::<u64>()?;
    whole
        .parse::<u64>()?
        .checked_mul(1000)
        .and_then(|v| v.checked_add(fraction))
        .context("band bound exceeds whole-hertz range")
}

/// Read the declared primary-subdivision field table as source text only.
fn primary_fields(source: &str) -> anyhow::Result<Vec<String>> {
    let table = Regex::new(r"(?s)sasMapping\[\]\s*=\s*\{(.*?)\r?\n\};")?;
    let body = table
        .captures(source)
        .and_then(|c| c.get(1))
        .context("pinned primary-subdivision table missing")?;
    let field = Regex::new(r#"\{\s*"([A-Z_]+)""#)?;
    let fields: Vec<_> = field
        .captures_iter(body.as_str())
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_owned()))
        .collect();
    ensure!(!fields.is_empty(), "pinned primary-subdivision table empty");
    Ok(fields)
}

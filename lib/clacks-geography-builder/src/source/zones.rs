//! Extract a pinned embedded JSON collection without executing its JavaScript wrapper.

use anyhow::{Context, ensure};
use geo_types::Geometry;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Decode one expected collection and preserve original linework, including guarded open rings.
pub(super) fn decode(
    bytes: &[u8],
    assignment: &str,
    expected: usize,
    open: &BTreeSet<String>,
    accept: &mut impl FnMut(BTreeMap<String, Value>, Geometry<f64>) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let text = std::str::from_utf8(bytes)?;
    let marker = format!("var {assignment} =");
    let mut matches = text.match_indices(&marker);
    let (offset, _) = matches.next().context("missing expected zone assignment")?;
    ensure!(matches.next().is_none(), "duplicate zone assignment");
    let remaining = text
        .get(offset + marker.len()..)
        .context("assignment boundary")?
        .trim_start();
    ensure!(
        remaining.starts_with('{'),
        "zone assignment is not a JSON object"
    );
    let (mut quoted, mut escaped, mut depth, mut end) = (false, false, 0usize, None);
    for (index, ch) in remaining.char_indices() {
        if quoted {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                quoted = false;
            }
            continue;
        }
        match ch {
            '"' => quoted = true,
            '{' | '[' => {
                depth += 1;
                ensure!(depth <= 8, "zone JSON nesting limit exceeded");
            }
            '}' | ']' => {
                depth = depth.checked_sub(1).context("unbalanced collection")?;
                if depth == 0 {
                    end = Some(index + ch.len_utf8());
                    break;
                }
            }
            _ => (),
        }
    }
    let end = end.context("unterminated JSON collection")?;
    ensure!(
        remaining
            .get(end..)
            .context("collection end")?
            .trim_start()
            .starts_with(';'),
        "unexpected collection terminator"
    );
    let json = json_separators(remaining.get(..end).context("collection slice")?);
    let collection: geojson::FeatureCollection = serde_json::from_str(&json)?;
    ensure!(
        collection.features.len() == expected,
        "unexpected zone feature count"
    );
    let mut names = BTreeSet::new();
    for feature in collection.features {
        let properties = feature.properties.context("zone lacks properties")?;
        let name = properties
            .get("name")
            .and_then(Value::as_str)
            .context("zone lacks source name")?;
        ensure!(
            names.insert(name.to_owned()),
            "duplicate zone source identity"
        );
        let geometry: Geometry<f64> = feature
            .geometry
            .context("zone lacks geometry")?
            .try_into()?;
        let Geometry::LineString(line) = &geometry else {
            anyhow::bail!("zone source is not linework");
        };
        ensure!(line.0.len() >= 4, "undersized zone linework");
        ensure!(
            line.0.first() == line.0.last() || open.contains(name),
            "unexpected unclosed zone {name}"
        );
        accept(properties.into_iter().collect(), geometry)?;
    }
    ensure!(
        open.is_subset(&names),
        "declared open-ring identity missing from source"
    );
    Ok(())
}

/// Remove the pinned JavaScript data's trailing separators outside quoted strings.
///
/// All other JSON grammar remains subject to the strict JSON decoder. This does
/// not evaluate expressions, comments, single-quoted strings or wrapper code.
fn json_separators(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let (mut quoted, mut escaped) = (false, false);
    for (index, ch) in input.char_indices() {
        if quoted {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                quoted = false;
            }
        } else if ch == '"' {
            quoted = true;
            output.push(ch);
        } else if ch == ','
            && input
                .get(index + 1..)
                .and_then(|rest| rest.chars().find(|c| !c.is_ascii_whitespace()))
                .is_some_and(|next| matches!(next, ']' | '}'))
        {
        } else {
            output.push(ch);
        }
    }
    output
}

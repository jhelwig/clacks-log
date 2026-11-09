//! Authored classified rectangles for desktop composition and persistence scenarios.

use clacks_geography::artifact::{BundleContents, CompiledFeature, Field, LayerInput, Manifest};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};

/// Select the exact manifest emitted by this authored fixture's production writer.
pub(crate) fn selection(root: &Path) -> anyhow::Result<crate::geography::Selection> {
    Ok(crate::geography::Selection {
        path: root.to_owned(),
        manifest_sha256: hex::encode(Sha256::digest(std::fs::read(root.join("manifest.json"))?)),
    })
}

/// Write a small real bundle with all five classifications inside the supplied bounds.
pub(crate) fn write(root: &Path, bounds: [f64; 4]) -> anyhow::Result<()> {
    let [west, south, east, north] = bounds;
    let mut manifest: Manifest = serde_json::from_value(serde_json::json!({
        "schema_version":1,"release_id":"desktop-authored-1","recipe_sha256":"0".repeat(64),
        "files":[],"layers":[],"values":[],
        "derivations":[{"key":"authored","source_identities":[{"source_id":"fixture","feature_id":"rectangle"}],"operation_ids":[],"correction_ids":[],"mapping_ids":[],"exception_ids":[]}],
        "sources":[{"id":"fixture","publisher":"Clacks tests","upstream_version":"1","sha256":"1".repeat(64),"coordinate_declaration":"WGS84","citations":["https://example.com/authored"],"notice_paths":["notices/fixture.txt"]}],
        "operations":[],"corrections":[],"mappings":[],"exceptions":[],"omissions":[],
        "scope":{"description":"authored rectangle"},"limitations":["not real geographic data"]
    }))?;
    let geometry = serde_json::json!([{"exterior":[
        {"x":west,"y":south},{"x":east,"y":south},{"x":east,"y":north},
        {"x":west,"y":north},{"x":west,"y":south}
    ],"interiors":[]}]);
    let mut layers = Vec::new();
    for (field, value) in [
        (Field::Dxcc, "291"),
        (Field::PrimarySubdivision, "WA"),
        (Field::SecondarySubdivision, "KING"),
        (Field::CqZone, "3"),
        (Field::ItuZone, "6"),
    ] {
        let key = format!("{field:?}");
        let parent_scope = match field {
            Field::PrimarySubdivision => serde_json::json!({"entity":"291"}),
            Field::SecondarySubdivision => {
                serde_json::json!({"subdivision":{"entity":"291","primary":"WA"}})
            }
            _ => serde_json::json!("global"),
        };
        manifest
            .values
            .push(serde_json::from_value(serde_json::json!({
                "key":key,"field":field,"parent_scope":parent_scope,
                "recordable_value":value,"display_label":format!("Authored {value}")
            }))?);
        layers.push(LayerInput {
            field,
            features: vec![CompiledFeature {
                part_id: key.clone(),
                value_key: key,
                derivation_key: "authored".into(),
                geometry: serde_json::from_value(geometry.clone())?,
            }],
        });
    }
    clacks_geography::artifact::write::write_bundle(
        root,
        &BundleContents {
            manifest,
            layers,
            notices: BTreeMap::from([("notices/fixture.txt".into(), "Authored test data".into())]),
        },
    )?;
    Ok(())
}

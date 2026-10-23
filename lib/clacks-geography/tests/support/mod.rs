use clacks_geography::artifact::{BundleContents, CompiledFeature, Field, LayerInput, Manifest};
use geo_types::{MultiPolygon, polygon};
use std::collections::BTreeMap;

pub fn contents() -> BundleContents {
    let manifest: Manifest = serde_json::from_value(serde_json::json!({
        "schema_version":1,"release_id":"representative-1","producer_revision":"test",
        "recipe_sha256":"0".repeat(64),"coordinate_convention":"wgs84_longitude_latitude_degrees",
        "files":[],"layers":[],
        "values":[{"key":"wa","field":"primary_subdivision","parent_scope":{"entity":"291"},"recordable_value":"WA","display_label":"Washington"}],
        "derivations":[{"key":"census-wa","source_identities":[{"source_id":"census","feature_id":"53"}],"operation_ids":[],"correction_ids":[],"mapping_ids":[],"exception_ids":[]}],
        "sources":[{"id":"census","publisher":"Census","upstream_version":"2026","sha256":"1".repeat(64),"coordinate_declaration":"NAD83","citations":["https://www.census.gov/"],"notice_paths":["notices/census.txt"]}],
        "operations":[],"corrections":[],"mappings":[],"exceptions":[],"omissions":[],
        "scope":{"description":"synthetic geometry","selected_source_identities":[{"source_id":"census","feature_id":"53"}]},"limitations":["not real boundaries"]
    })).unwrap();
    let geometry = MultiPolygon(vec![
        polygon![(x:0.,y:0.),(x:2.,y:0.),(x:2.,y:1.),(x:0.,y:1.),(x:0.,y:0.)],
    ]);
    let layers = Field::ALL
        .into_iter()
        .map(|field| LayerInput {
            field,
            features: if field == Field::PrimarySubdivision {
                vec![CompiledFeature {
                    part_id: "wa-1".into(),
                    value_key: "wa".into(),
                    derivation_key: "census-wa".into(),
                    geometry: geometry.clone(),
                }]
            } else {
                vec![]
            },
        })
        .collect();
    BundleContents {
        manifest,
        layers,
        notices: BTreeMap::from([("notices/census.txt".into(), "Census attribution".into())]),
    }
}

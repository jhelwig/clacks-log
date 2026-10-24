//! Rust-authored pinned source and recipe for end-to-end publication tests.
use sha2_rsa::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
pub fn recipe(directory: &Path) -> PathBuf {
    let bytes=br#"var zones = {"type":"FeatureCollection","features":[{"type":"Feature","properties":{"name":"one"},"geometry":{"type":"LineString","coordinates":[[0,0],[10,0],[10,10],[0,10],[0,0]]}}]};"#;
    let hash = hex::encode(Sha256::digest(bytes));
    fs::create_dir(directory.join("cache")).unwrap();
    fs::write(directory.join("cache").join(format!("{hash}.blob")), bytes).unwrap();
    let json = serde_json::json!({"schema_version":1,"release_id":"one","producer_revision":"test","sources":[{"id":"source","publisher":"Test author","upstream_version":"one","citation_urls":["authored"],"download_url":"https://example.invalid/source","archive_sha256":hash,"expected_coordinate_declaration":"WGS84","text_encoding":"UTF-8","archive_members":[],"notice_paths":[],"kind":{"zones":{"assignment":"zones","expected_features":1,"open_rings":[]}},"limits":{"download_bytes":10000,"member_bytes":10000,"expanded_bytes":10000,"members":10}}],"selections":[{"source_id":"source","identity_fields":["name"],"properties":{},"expected_count":1}],"operations":[{"id":"identity","source_ids":["source"],"method":"identity","assignment":"entire","description":"WGS84 source","citations":["authored"]}],"corrections":[],"mappings":[{"id":"value","source_feature_selectors":[{"source_id":"source","feature_id":"one"}],"field":"dxcc","parent_scope":"global","recordable_value":"291","display_label":"United States","citations":["authored"]}],"exceptions":[],"omissions":[],"acceptance_cases":[{"id":"interior","query":{"point":{"latitude_microdegrees":1000000,"longitude_microdegrees":1000000}},"expected":{"dxcc":{"coverage":"complete","candidates":[{"parent_scope":"global","value":"291"}]}},"citations":["authored square"]}],"notices":{},"scope":"Synthetic publication test","limitations":[]});
    let path = directory.join("recipe.json");
    fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    path
}

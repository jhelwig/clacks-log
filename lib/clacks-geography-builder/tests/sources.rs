mod support;

use clacks_geography_builder::{
    recipe::read_recipe,
    source::{decode_sources, fetch_sources},
};
use sha2_rsa::{Digest, Sha256};
use std::{fs, sync::atomic::AtomicBool};

fn recipe(dir: &std::path::Path, text: &[u8], source_url: &str) -> std::path::PathBuf {
    let digest = hex::encode(Sha256::digest(text));
    let json = serde_json::json!({
        "schema_version":1,"release_id":"test-1","producer_revision":"test",
        "sources":[{"id":"zones","publisher":"test","upstream_version":"1","citation_urls":["https://example.invalid/source"],"download_url":source_url,
        "archive_sha256":digest,"expected_coordinate_declaration":"GeoJSON WGS84","text_encoding":"UTF-8","archive_members":[],"notice_paths":["notices/test.txt"],"kind":{"zones":{"assignment":"cqZones","expected_features":1,"open_rings":[]}},
        "limits":{"download_bytes":10000,"member_bytes":10000,"expanded_bytes":20000,"members":8}}],
        "selections":[{"source_id":"zones","identity_fields":["name"],"properties":{"name":["3"]},"expected_count":1}],
        "operations":[],"corrections":[],"mappings":[],"exceptions":[],"omissions":[],"acceptance_cases":[],
        "notices":{"notices/test.txt":"test notice"},"scope":"synthetic","limitations":["not source accuracy evidence"]
    });
    let path = dir.join("recipe.json");
    fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    fs::create_dir_all(dir.join("cache")).unwrap();
    fs::write(dir.join("cache").join(format!("{digest}.blob")), text).unwrap();
    path
}
const WRAPPED:&[u8]=br#"/* wrapper */ var cqZones = {"type":"FeatureCollection","features":[{"type":"Feature","properties":{"name":"3","label":"brace } and escaped \" quote"},"geometry":{"type":"LineString","coordinates":[[0,0],[1,0],[1,1],[0,0]]}}]}; trailing wrapper"#;

#[test]
fn pinned_wrapper_is_parsed_without_executing_code() {
    let dir = tempfile::tempdir().unwrap();
    let path = recipe(dir.path(), WRAPPED, "https://example.invalid/zones");
    let recipe = read_recipe(&path).unwrap();
    let sources =
        decode_sources(&recipe, &dir.path().join("cache"), &AtomicBool::new(false)).unwrap();
    assert_eq!(sources.features.len(), 1);
    assert_eq!(sources.features[0].identity.feature_id, "3");
    assert_eq!(sources.features[0].feature_sha256.len(), 64);
    assert_eq!(
        fetch_sources(&recipe, &dir.path().join("cache"), &AtomicBool::new(false))
            .unwrap()
            .reused,
        1
    );
}

#[test]
fn malformed_wrapper_changed_counts_and_cache_corruption_fail() {
    for text in [
        br#"var cqZones = {}; var cqZones = {};"#.as_slice(),
        br#"var cqZones = {"type":"FeatureCollection","features":[]};"#.as_slice(),
        br#"var cqZones = {"type":"FeatureCollection","features":["truncated""#.as_slice(),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = recipe(dir.path(), text, "https://example.invalid/zones");
        let recipe = read_recipe(&path).unwrap();
        assert!(
            decode_sources(&recipe, &dir.path().join("cache"), &AtomicBool::new(false)).is_err()
        );
    }
    let dir = tempfile::tempdir().unwrap();
    let path = recipe(dir.path(), WRAPPED, "https://example.invalid/zones");
    let recipe = read_recipe(&path).unwrap();
    fs::write(
        dir.path()
            .join("cache")
            .join(format!("{}.blob", recipe.sources[0].archive_sha256)),
        b"changed",
    )
    .unwrap();
    assert!(decode_sources(&recipe, &dir.path().join("cache"), &AtomicBool::new(false)).is_err());
    assert!(fetch_sources(&recipe, &dir.path().join("cache"), &AtomicBool::new(true)).is_err());
}

#[test]
fn pinned_javascript_trailing_separators_are_converted_without_touching_strings() {
    let text=br#"var cqZones = {"type":"FeatureCollection","features":[{"type":"Feature","properties":{"name":"3","label":"comma ,] inside text"},"geometry":{"type":"LineString","coordinates":[[0,0],[1,0],[1,1],[0,0],],},},],};"#;
    let dir = tempfile::tempdir().unwrap();
    let path = recipe(dir.path(), text, "https://example.invalid/source");
    let recipe = read_recipe(&path).unwrap();
    let decoded =
        decode_sources(&recipe, &dir.path().join("cache"), &AtomicBool::new(false)).unwrap();
    assert_eq!(
        decoded.features[0].properties["label"],
        "comma ,] inside text"
    );
}

fn shapefile_recipe(dir: &std::path::Path, bytes: &[u8]) -> std::path::PathBuf {
    let path = recipe(dir, bytes, "https://example.invalid/source.zip");
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let pin = &mut json["sources"][0];
    pin["kind"] = "shapefile".into();
    pin["expected_coordinate_declaration"] = "test source declaration".into();
    pin["archive_members"] = serde_json::json!([
        "source.shp",
        "source.shx",
        "source.dbf",
        "source.prj",
        "source.cpg"
    ]);
    pin["limits"] = serde_json::json!({"download_bytes":1000000,"member_bytes":1000000,"expanded_bytes":1000000,"members":8});
    json["selections"][0]["identity_fields"] = serde_json::json!(["GEOID"]);
    json["selections"][0]["properties"] = serde_json::json!({"GEOID":["53"]});
    fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    path
}

#[test]
fn source_shapefiles_preserve_multipart_and_hole_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = support::archive(dir.path(), "valid");
    let path = shapefile_recipe(dir.path(), &bytes);
    let recipe = read_recipe(&path).unwrap();
    let decoded =
        decode_sources(&recipe, &dir.path().join("cache"), &AtomicBool::new(false)).unwrap();
    let geo_types::Geometry::MultiPolygon(geometry) = &decoded.features[0].geometry else {
        panic!("wrong geometry")
    };
    assert_eq!(geometry.0.len(), 2);
    assert_eq!(geometry.0[0].interiors().len(), 1);
}

#[test]
fn malformed_declarations_records_selectors_and_archive_limits_fail() {
    for mutation in [
        "prj",
        "encoding",
        "counts",
        "deleted",
        "truncated",
        "shape_kind",
        "unlisted",
        "member_limit",
        "expanded_limit",
        "entry_limit",
        "absent",
        "duplicate",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let bytes = support::archive(dir.path(), mutation);
        let path = shapefile_recipe(dir.path(), &bytes);
        let mut json: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        match mutation {
            "unlisted" => {
                json["sources"][0]["archive_members"]
                    .as_array_mut()
                    .unwrap()
                    .pop();
            }
            "member_limit" => json["sources"][0]["limits"]["member_bytes"] = 1.into(),
            "expanded_limit" => json["sources"][0]["limits"]["expanded_bytes"] = 1.into(),
            "entry_limit" => json["sources"][0]["limits"]["members"] = 1.into(),
            "absent" => {
                json["selections"][0]["properties"] = serde_json::json!({"GEOID":["missing"]})
            }
            "duplicate" => {
                let duplicate = json["selections"][0].clone();
                json["selections"].as_array_mut().unwrap().push(duplicate);
            }
            _ => (),
        }
        fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        let recipe = read_recipe(&path).unwrap();
        assert!(
            decode_sources(&recipe, &dir.path().join("cache"), &AtomicBool::new(false)).is_err(),
            "accepted {mutation}"
        );
    }
}

#[test]
fn unsafe_archive_paths_and_symlinks_are_rejected() {
    use std::io::Write;
    for name in ["../escape", "/absolute", "link"] {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        if name == "link" {
            zip.add_symlink("link", "target", zip::write::SimpleFileOptions::default())
                .unwrap();
        } else {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"bytes").unwrap();
        }
        let bytes = zip.finish().unwrap().into_inner();
        let dir = tempfile::tempdir().unwrap();
        let path = shapefile_recipe(dir.path(), &bytes);
        let recipe = read_recipe(&path).unwrap();
        assert!(
            decode_sources(&recipe, &dir.path().join("cache"), &AtomicBool::new(false)).is_err()
        );
    }
}

#[test]
fn explicit_loopback_fetch_installs_only_verified_complete_bytes() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };
    for mode in ["valid", "wrong_digest", "oversized", "interrupted"] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/source", listener.local_addr().unwrap());
        let dir = tempfile::tempdir().unwrap();
        let path = recipe(dir.path(), WRAPPED, &url);
        let mut recipe = read_recipe(&path).unwrap();
        let blob = dir
            .path()
            .join("cache")
            .join(format!("{}.blob", recipe.sources[0].archive_sha256));
        fs::remove_file(&blob).unwrap();
        if mode == "wrong_digest" {
            recipe.sources[0].archive_sha256 = "a".repeat(64);
        }
        if mode == "oversized" {
            recipe.sources[0].limits.download_bytes = 10;
        }
        let handle = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0; 2048];
            let _ = socket.read(&mut request).unwrap();
            let length = if mode == "interrupted" {
                WRAPPED.len() + 10
            } else {
                WRAPPED.len()
            };
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
            socket.write_all(WRAPPED).unwrap();
        });
        let result = fetch_sources(&recipe, &dir.path().join("cache"), &AtomicBool::new(false));
        handle.join().unwrap();
        assert_eq!(result.is_ok(), mode == "valid", "mode {mode}: {result:?}");
        assert_eq!(blob.exists(), mode == "valid");
    }
}

#[test]
fn duplicate_zip_member_names_do_not_silently_replace_each_other() {
    use std::io::Write;
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for name in ["a.bin", "b.bin"] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"x").unwrap();
    }
    let mut bytes = zip.finish().unwrap().into_inner();
    let positions: Vec<_> = bytes
        .windows(5)
        .enumerate()
        .filter(|(_, w)| *w == b"b.bin")
        .map(|(i, _)| i)
        .collect();
    for position in positions {
        bytes[position..position + 5].copy_from_slice(b"a.bin");
    }
    let dir = tempfile::tempdir().unwrap();
    let path = shapefile_recipe(dir.path(), &bytes);
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    json["sources"][0]["archive_members"] = serde_json::json!(["a.bin"]);
    fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    let recipe = read_recipe(&path).unwrap();
    let error = decode_sources(&recipe, &dir.path().join("cache"), &AtomicBool::new(false))
        .err()
        .unwrap();
    assert!(
        format!("{error:#}")
            .to_ascii_lowercase()
            .contains("duplicate"),
        "{error:#}"
    );
}

#[test]
fn entrypoints_revalidate_mutated_recipe_data_before_cache_access() {
    let dir = tempfile::tempdir().unwrap();
    let path = recipe(dir.path(), WRAPPED, "https://example.invalid/source");
    let mut recipe = read_recipe(&path).unwrap();
    recipe.sources[0].archive_sha256 = "not-a-digest".into();
    let error = decode_sources(
        &recipe,
        &dir.path().join("missing-cache"),
        &AtomicBool::new(false),
    )
    .err()
    .unwrap();
    assert!(
        format!("{error:#}").contains("invalid SHA-256"),
        "{error:#}"
    );
}

#[test]
fn an_absent_or_empty_source_key_is_not_an_invented_identity() {
    for replacement in [
        "\"name\":\"3\",\"external_key\":null",
        "\"name\":\"3\",\"external_key\":\"\"",
    ] {
        let text = std::str::from_utf8(WRAPPED)
            .unwrap()
            .replace("\"name\":\"3\"", replacement);
        let dir = tempfile::tempdir().unwrap();
        let path = recipe(
            dir.path(),
            text.as_bytes(),
            "https://example.invalid/source",
        );
        let mut json: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        json["selections"][0]["identity_fields"] = serde_json::json!(["external_key"]);
        fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        let recipe = read_recipe(&path).unwrap();
        assert!(
            decode_sources(&recipe, &dir.path().join("cache"), &AtomicBool::new(false)).is_err()
        );
    }
}

#[test]
fn misspelled_recipe_rules_are_rejected_instead_of_ignored() {
    let directory = tempfile::tempdir().unwrap();
    let path = recipe(directory.path(), WRAPPED, "https://example.invalid/source");
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    json["sources"][0]["attribute_domans"] = serde_json::json!({"LWFLAG":["L"]});
    fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    assert!(read_recipe(&path).is_err());
}

//! Build-identity wiring checked against production-encoded bundle bytes.
use super::*;
use clacks_geography::artifact::{
    BundleContents, Field, LayerInput, Manifest, write::write_bundle,
};
use sha2_rsa::{Digest, Sha256};

fn fixture(root: &Path) -> (String, std::path::PathBuf) {
    let manifest: Manifest = serde_json::from_value(serde_json::json!({
        "schema_version":1,"release_id":"package-test","recipe_sha256":"0".repeat(64),
        "files":[],"layers":[],"values":[],"derivations":[],"sources":[],
        "operations":[],"corrections":[],"mappings":[],"exceptions":[],"omissions":[],
        "scope":{"description":"empty authored packaging fixture"},
        "limitations":["no geographic coverage"]
    }))
    .unwrap();
    let bundle = root.join("source");
    write_bundle(
        &bundle,
        &BundleContents {
            manifest,
            layers: Field::ALL
                .into_iter()
                .map(|field| LayerInput {
                    field,
                    features: vec![],
                })
                .collect(),
            notices: Default::default(),
        },
    )
    .unwrap();
    let digest = hex::encode(Sha256::digest(
        fs::read(bundle.join("manifest.json")).unwrap(),
    ));
    (digest, bundle)
}

#[test]
fn inspection_uses_the_build_identity() {
    let root = tempfile::tempdir().unwrap();
    let (expected, bundle) = fixture(root.path());
    assert!(inspect(&"0".repeat(64), &bundle).is_err());
    let evidence = inspect(&expected, &bundle).unwrap();
    assert_eq!(evidence.release_id, "package-test");
    assert_eq!(evidence.manifest_sha256, expected);
}

#[path = "support/reviewed_recipe.rs"]
mod reviewed_recipe;

use clacks_geography_builder::{compile::compile_bundle, recipe::read_recipe};
use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

fn cache() -> PathBuf {
    std::env::var_os("CLACKS_GEOGRAPHY_CACHE")
        .map(PathBuf::from)
        .expect("set CLACKS_GEOGRAPHY_CACHE to the explicitly fetched pinned-source cache")
}

#[test]
#[ignore = "requires pinned geographic source cache"]
fn compile_pinned_geographic_sources() {
    let recipe = read_recipe(Path::new("../../references/geography/representative.json")).unwrap();
    let compiled = compile_bundle(&recipe, &cache(), &AtomicBool::new(false)).unwrap();
    for mapping in ["state-WA", "state-OR", "state-CT"] {
        let derivations: Vec<_> = compiled
            .contents
            .manifest
            .derivations
            .iter()
            .filter(|derivation| derivation.mapping_ids.iter().any(|id| id == mapping))
            .collect();
        assert_eq!(
            derivations.len(),
            1,
            "{mapping} is entirely included in its own mainland source mask"
        );
        assert_eq!(derivations[0].operation_ids, vec!["mainland-1188"]);
    }
    let directory = tempfile::tempdir().unwrap();
    let encoded =
        clacks_geography::artifact::write::write_bundle(directory.path(), &compiled.contents)
            .unwrap();
    let parsed: clacks_geography::artifact::Manifest =
        serde_json::from_slice(&std::fs::read(directory.path().join("manifest.json")).unwrap())
            .unwrap();
    for layer in encoded.layers {
        assert_eq!(
            layer.extent_or_empty,
            parsed
                .layers
                .iter()
                .find(|entry| entry.field == layer.field)
                .unwrap()
                .extent_or_empty,
            "{:?} manifest floating-point round trip",
            layer.field
        );
    }
    let mut bundle = clacks_geography::artifact::read::open_bundle(directory.path()).unwrap();
    clacks_geography_builder::compile::verify_bundle(&mut bundle, &recipe).unwrap();
    // Compile independently again, so determinism covers the producer as well as encoding.
    let repeated = compile_bundle(&recipe, &cache(), &AtomicBool::new(false)).unwrap();
    let second = tempfile::tempdir().unwrap();
    let repeated_manifest =
        clacks_geography::artifact::write::write_bundle(second.path(), &repeated.contents).unwrap();
    assert_eq!(
        std::fs::read(directory.path().join("manifest.json")).unwrap(),
        std::fs::read(second.path().join("manifest.json")).unwrap(),
    );
    for file in repeated_manifest.files {
        assert_eq!(
            std::fs::read(directory.path().join(&file.path)).unwrap(),
            std::fs::read(second.path().join(&file.path)).unwrap(),
            "deterministic payload {}",
            file.path,
        );
    }
    assert_eq!(
        bundle
            .manifest()
            .scope
            .selected_source_identities
            .iter()
            .filter(|id| id.source_id == "ct-counties")
            .count(),
        8
    );
    assert!(
        !bundle
            .manifest()
            .scope
            .selected_source_identities
            .iter()
            .any(|id| id.source_id == "counties" && id.feature_id.starts_with("09"))
    );
}

#[test]
#[ignore = "explicit maintenance of reviewed inputs; requires pinned geographic source cache"]
fn reviewed_recipe_inputs_match_original_sources() {
    reviewed_recipe::verify_or_refresh(&cache());
}

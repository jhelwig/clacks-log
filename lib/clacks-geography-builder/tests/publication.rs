#[path = "support/publication_recipe.rs"]
mod publication_recipe;

use clacks_geography::artifact::{CurrentRelease, validate::open_bundle};
use clacks_geography_builder::{
    publish::{BuildOutcome, build_release},
    recipe::read_recipe,
};
use std::{fs, sync::atomic::AtomicBool};

#[test]
fn publish_reuse_failure_and_open_reader_follow_the_commit_point() {
    let directory = tempfile::tempdir().unwrap();
    let mut recipe = read_recipe(&publication_recipe::recipe(directory.path())).unwrap();
    let build_id = "one";
    let output = directory.path().join("output");
    let cache = directory.path().join("cache");
    let cancelled = AtomicBool::new(false);
    let first = build_release(&recipe, build_id, &cache, &output, &cancelled).unwrap();
    assert_eq!(first.outcome, BuildOutcome::Published);
    let original = fs::read(output.join("current.json")).unwrap();
    let pointer: CurrentRelease = serde_json::from_slice(&original).unwrap();
    let mut old = open_bundle(&output.join("releases").join(&pointer.release_id)).unwrap();
    assert_eq!(
        build_release(&recipe, build_id, &cache, &output, &cancelled)
            .unwrap()
            .outcome,
        BuildOutcome::Unchanged
    );
    recipe.mappings[0].display_label = "Changed bytes under same ID".into();
    assert!(build_release(&recipe, build_id, &cache, &output, &cancelled).is_err());
    assert_eq!(fs::read(output.join("current.json")).unwrap(), original);
    let build_id = "two";
    build_release(&recipe, build_id, &cache, &output, &cancelled).unwrap();
    assert_eq!(old.manifest().release_id, "one");
    use clacks_model::position::{Coordinates, LookupGeometry};
    assert_eq!(
        old.lookup(LookupGeometry::Point(
            Coordinates::from_microdegrees(1_000_000, 1_000_000).unwrap()
        ))
        .unwrap()
        .dxcc
        .candidates
        .len(),
        1
    );
    assert_ne!(fs::read(output.join("current.json")).unwrap(), original);
}

#[test]
fn cancelled_or_invalid_build_preserves_current_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let mut recipe = read_recipe(&publication_recipe::recipe(directory.path())).unwrap();
    let build_id = "one";
    let output = directory.path().join("output");
    let cache = directory.path().join("cache");
    build_release(&recipe, build_id, &cache, &output, &AtomicBool::new(false)).unwrap();
    let original = fs::read(output.join("current.json")).unwrap();
    let build_id = "two";
    assert!(build_release(&recipe, build_id, &cache, &output, &AtomicBool::new(true)).is_err());
    recipe.acceptance_cases[0]
        .expected
        .get_mut(&clacks_geography::artifact::Field::Dxcc)
        .unwrap()
        .candidates[0]
        .value = "236".into();
    assert!(build_release(&recipe, build_id, &cache, &output, &AtomicBool::new(false)).is_err());
    assert_eq!(fs::read(output.join("current.json")).unwrap(), original);
    assert!(!output.join("releases/two").exists());
}

#[test]
fn concurrent_publishers_are_serialized_without_deleting_old_releases() {
    let directory = tempfile::tempdir().unwrap();
    let recipe = read_recipe(&publication_recipe::recipe(directory.path())).unwrap();
    let build_id = "one";
    let output = directory.path().join("output");
    let cache = directory.path().join("cache");
    let outcomes = std::thread::scope(|scope| {
        let one = scope.spawn(|| {
            build_release(&recipe, build_id, &cache, &output, &AtomicBool::new(false))
                .unwrap()
                .outcome
        });
        let two = scope.spawn(|| {
            build_release(&recipe, build_id, &cache, &output, &AtomicBool::new(false))
                .unwrap()
                .outcome
        });
        vec![one.join().unwrap(), two.join().unwrap()]
    });
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| **outcome == BuildOutcome::Published)
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| **outcome == BuildOutcome::Unchanged)
            .count(),
        1
    );
}

#[cfg(windows)]
#[test]
fn sharing_locked_pointer_rejects_replace_and_allows_a_normal_retry() {
    use std::os::windows::fs::OpenOptionsExt;
    let directory = tempfile::tempdir().unwrap();
    let mut recipe = read_recipe(&publication_recipe::recipe(directory.path())).unwrap();
    let build_id = "one";
    let output = directory.path().join("output");
    let cache = directory.path().join("cache");
    build_release(&recipe, build_id, &cache, &output, &AtomicBool::new(false)).unwrap();
    let original = fs::read(output.join("current.json")).unwrap();
    // Permit reads/writes but deliberately withhold Windows delete-sharing needed by replacement.
    let locked = fs::OpenOptions::new()
        .read(true)
        .share_mode(0x1 | 0x2)
        .open(output.join("current.json"))
        .unwrap();
    let build_id = "two";
    assert!(build_release(&recipe, build_id, &cache, &output, &AtomicBool::new(false)).is_err());
    assert_eq!(fs::read(output.join("current.json")).unwrap(), original);
    drop(locked);
    assert_eq!(
        build_release(&recipe, build_id, &cache, &output, &AtomicBool::new(false))
            .unwrap()
            .outcome,
        BuildOutcome::Published
    );
}

#[test]
fn unresolved_exclusion_preserves_the_selected_release() {
    use clacks_geography_builder::recipe::MappingException;
    use sha2_rsa::{Digest, Sha256};
    let directory = tempfile::tempdir().unwrap();
    let mut recipe = read_recipe(&publication_recipe::recipe(directory.path())).unwrap();
    let build_id = "one";
    let output = directory.path().join("output");
    let cache = directory.path().join("cache");
    build_release(&recipe, build_id, &cache, &output, &AtomicBool::new(false)).unwrap();
    let original = fs::read(output.join("current.json")).unwrap();
    let geometry = geojson::Geometry::from(
        &geo_types::Rect::new((5. - 1e-15, 4.), (5. + 1e-15, 6.)).to_polygon(),
    );
    let bytes = serde_json::to_vec(&geometry).unwrap();
    fs::write(directory.path().join("unresolved.json"), &bytes).unwrap();
    let build_id = "two";
    recipe.exceptions.push(MappingException {
        id: "unresolved".into(),
        affected_mapping_ids: vec!["value".into()],
        exclusion_mapping_ids: vec![],
        exclusion_geometry_path: "unresolved.json".into(),
        exclusion_sha256: hex::encode(Sha256::digest(&bytes)),
        replacement: None,
        rationale: "Analytical positive-area exclusion below clipping precision".into(),
        citations: vec!["Authored rectangle".into()],
        notice_paths: vec![],
    });
    let error =
        build_release(&recipe, build_id, &cache, &output, &AtomicBool::new(false)).unwrap_err();
    assert!(
        format!("{error:#}").contains("producer overlay precision"),
        "{error:#}"
    );
    assert_eq!(fs::read(output.join("current.json")).unwrap(), original);
    assert!(!output.join("releases/two").exists());
}

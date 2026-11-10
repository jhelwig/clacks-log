//! Command discoverability and production boundary parsing.

use super::*;

use crate::publication_recipe;
mod telemetry;

#[test]
fn build_reporting_counts_real_outcomes_and_not_staged_success() {
    let directory = tempfile::tempdir().unwrap();
    let recipe = publication_recipe::recipe(directory.path());
    let output = directory.path().join("private-output-marker");
    let cache = directory.path().join("cache");
    let captured = telemetry::capture_filtered(
        || {
            let first = execute(
                Command::Build {
                    build_id: "one".into(),
                    recipe: recipe.clone(),
                    cache: cache.clone(),
                    output: output.clone(),
                },
                &AtomicBool::new(false),
            );
            let second = execute(
                Command::Build {
                    build_id: "one".into(),
                    recipe: recipe.clone(),
                    cache: cache.clone(),
                    output: output.clone(),
                },
                &AtomicBool::new(false),
            );
            let cancelled = execute(
                Command::Build {
                    build_id: "one".into(),
                    recipe: recipe.clone(),
                    cache,
                    output: output.clone(),
                },
                &AtomicBool::new(true),
            );
            let invalid = execute(
                Command::Build {
                    build_id: "one".into(),
                    recipe: directory.path().join("missing.json"),
                    cache: directory.path().join("cache"),
                    output,
                },
                &AtomicBool::new(false),
            );
            (first, second, cancelled, invalid)
        },
        |metadata| *metadata.level() <= tracing::Level::INFO,
    );
    assert!(
        captured.result.0.is_ok()
            && captured.result.1.is_ok()
            && captured.result.2.is_err()
            && captured.result.3.is_err()
    );
    for outcome in ["published", "unchanged", "cancelled", "failed"] {
        captured.counter(
            crate::application::BUILD_RUNS_METRIC,
            &[("outcome", outcome)],
            1,
        );
    }
    assert_eq!(
        captured
            .observations
            .events
            .iter()
            .filter(|event| event.name == "geography.bundle_published")
            .count(),
        1
    );
    let text = format!("{:?}", captured.observations);
    assert!(!text.contains("private-output-marker"));
    for metric in captured
        .metrics
        .iter()
        .filter(|metric| metric.name.starts_with("clacks_geography_build_"))
    {
        assert_eq!(metric.labels.len(), 1);
        assert!(metric.labels.contains_key("outcome"));
    }
}

#[test]
fn rejected_build_reports_release_and_category_with_debug_filtered() {
    let directory = tempfile::tempdir().unwrap();
    let recipe = publication_recipe::recipe(directory.path());
    let output = directory.path().join("blocked");
    std::fs::write(&output, "not a directory").unwrap();
    let captured = telemetry::capture_filtered(
        || {
            execute(
                Command::Build {
                    build_id: "one".into(),
                    recipe,
                    cache: directory.path().join("cache"),
                    output,
                },
                &AtomicBool::new(false),
            )
        },
        |metadata| *metadata.level() <= tracing::Level::INFO,
    );
    assert!(captured.result.is_err());
    let rejection = captured
        .observations
        .events
        .iter()
        .find(|event| event.name == "geography.bundle_rejected")
        .expect("missing build rejection");
    assert_eq!(
        rejection.fields.get("release_id").map(String::as_str),
        Some("one")
    );
    assert_eq!(
        rejection.fields.get("category").map(String::as_str),
        Some("build_failed")
    );
}

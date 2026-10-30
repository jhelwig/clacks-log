mod support;
mod telemetry;

use clacks_geography::artifact::{read::open_bundle, write::write_bundle};
use clacks_model::position::{Coordinates, LookupGeometry};

#[test]
fn lookup_reports_only_complete_decisions_and_excludes_query_payloads() {
    let directory = tempfile::tempdir().unwrap();
    let mut input = support::contents();
    let mut value = input.manifest.values[0].clone();
    value.key = "itu-6".into();
    value.field = clacks_geography::artifact::Field::ItuZone;
    value.parent_scope = clacks_geography::artifact::ParentScope::Global;
    value.recordable_value = "6".into();
    value.display_label = "ITU zone 6".into();
    input.manifest.values.push(value);
    let mut feature = input
        .layers
        .iter()
        .flat_map(|layer| &layer.features)
        .next()
        .unwrap()
        .clone();
    feature.part_id = "itu-part".into();
    feature.value_key = "itu-6".into();
    input
        .layers
        .iter_mut()
        .find(|layer| layer.field == clacks_geography::artifact::Field::ItuZone)
        .unwrap()
        .features
        .push(feature);
    write_bundle(directory.path(), &input).unwrap();
    let mut bundle = open_bundle(directory.path()).unwrap();
    let captured = telemetry::capture(|| {
        let parent = tracing::info_span!("request", request_id = "lookup-regression");
        parent.in_scope(|| {
            let success = bundle.lookup(LookupGeometry::Point(
                Coordinates::from_microdegrees(500_000, 500_000).unwrap(),
            ));
            // Damage the last layer after opening so earlier field decisions precede the read failure.
            std::fs::write(directory.path().join("layers/itu_zone.fgb"), b"damaged").unwrap();
            let failed = bundle.lookup(LookupGeometry::Point(
                Coordinates::from_microdegrees(500_000, 500_000).unwrap(),
            ));
            (success, failed)
        })
    });
    assert!(captured.result.0.is_ok() && captured.result.1.is_err());
    captured.counter(
        "clacks_geography_field_lookups_total",
        &[
            ("field", "primary_subdivision"),
            ("coverage", "complete"),
            ("candidate_class", "one"),
        ],
        1,
    );
    captured.counter(
        "clacks_geography_field_lookups_total",
        &[
            ("field", "itu_zone"),
            ("coverage", "complete"),
            ("candidate_class", "one"),
        ],
        1,
    );
    let span = captured.span("geography.lookup");
    assert!(span.fields.contains_key("release_id"));
    assert!(
        !span.fields.contains_key("coordinates")
            && !span.fields.contains_key("locator")
            && !span.fields.contains_key("operation")
    );
    for metric in &captured.metrics {
        let keys: std::collections::BTreeSet<_> =
            metric.labels.keys().map(String::as_str).collect();
        match metric.name.as_str() {
            "clacks_geography_lookup_duration_seconds" => {
                assert_eq!(keys, ["query_kind", "outcome"].into_iter().collect())
            }
            "clacks_geography_field_lookups_total" => assert_eq!(
                keys,
                ["field", "coverage", "candidate_class"]
                    .into_iter()
                    .collect()
            ),
            other => panic!("unexpected metric {other}"),
        };
    }
}

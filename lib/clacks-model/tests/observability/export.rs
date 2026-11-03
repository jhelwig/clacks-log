//! Reference-export diagnostics, filtering and reporting through actual output plans.

use super::support;
use clacks_model::export::ExportContext;
use clacks_model::export::evaluate::plan_reference_export;
use clacks_model::ids::{GroupId, OutputProfileId, QsoId};
use std::collections::BTreeMap;
use support::telemetry::capture;

#[test]
fn export_plan_reports_supplied_unique_and_expanded_counts() {
    let a = support::qso(support::id::<QsoId>(1));
    let b = support::qso(support::id::<QsoId>(2));
    let defs = support::definitions();
    let profiles = support::profiles();
    let capture = capture(|| {
        plan_reference_export(
            &[a.clone(), a, b]
                .iter()
                .map(clacks_model::export::evaluate::ReferenceExportInput::recorded)
                .collect::<Vec<_>>(),
            &ExportContext {
                output_profile_id: support::id::<OutputProfileId>(1),
                target_group_id: None,
            },
            &defs,
            &profiles,
            &BTreeMap::new(),
        )
    });
    let event = capture.event("reference_export.planned");
    assert!(!event.fields.contains_key("duration_seconds"));
    assert!(capture.metrics.iter().all(|m| !m.name.contains("duration")));

    assert!(
        capture
            .metrics
            .iter()
            .all(|m| !m.name.starts_with("clacks_library_"))
    );
    let plan = capture.result.as_ref().unwrap();
    assert_eq!(plan.source_contact_count, 2);
    assert_eq!(plan.files.iter().map(|f| f.entries.len()).sum::<usize>(), 2);
    assert_eq!(
        capture
            .event("reference_export.planned")
            .fields
            .get("readiness")
            .unwrap(),
        "ready"
    );
    for (kind, expected) in [("input", 3.), ("source_contact", 2.), ("output_entry", 2.)] {
        capture.event_count("reference_export.planned", kind, expected);
    }
}

#[test]
fn ambiguous_export_is_review_not_an_error_event() {
    let mut q = support::qso(support::id::<QsoId>(1));
    for g in [2, 3] {
        support::edit(&mut q, |q| {
            q.own_station
                .references
                .insert(support::reference(support::id::<GroupId>(g), None));

            Ok(())
        });
    }
    let defs = support::contest_definitions();
    let profiles = support::profiles();
    let context = ExportContext {
        output_profile_id: support::id::<OutputProfileId>(1),
        target_group_id: None,
    };
    let captured = capture(|| {
        plan_reference_export(
            &(std::slice::from_ref(&q))
                .iter()
                .map(clacks_model::export::evaluate::ReferenceExportInput::recorded)
                .collect::<Vec<_>>(),
            &context,
            &defs,
            &profiles,
            &BTreeMap::new(),
        )
    });
    assert_eq!(captured.result.as_ref().unwrap().decisions.len(), 1);
    assert_eq!(
        captured
            .event("reference_export.planned")
            .fields
            .get("readiness")
            .unwrap(),
        "review"
    );
    captured.event_count("reference_export.planned", "decision", 1.);
    assert!(
        captured
            .observations
            .events
            .iter()
            .all(|e| e.fields.get("level").is_none_or(|l| l != "ERROR"))
    );
    let mut changed = q.clone();
    support::edit(&mut changed, |changed| {
        changed.comment = Some("different working version".into());

        Ok(())
    });
    let captured = capture(|| {
        plan_reference_export(
            &[q, changed]
                .iter()
                .map(clacks_model::export::evaluate::ReferenceExportInput::recorded)
                .collect::<Vec<_>>(),
            &context,
            &defs,
            &profiles,
            &BTreeMap::new(),
        )
    });

    assert!(captured.result.is_err());

    assert!(captured.observations.events.iter().all(|e| {
        e.fields
            .get("event")
            .is_none_or(|name| name != "reference_export.planned")
    }));
    assert!(
        captured
            .metrics
            .iter()
            .all(|m| m.name != "clacks_reference_export_plans_total")
    );
}

#[test]
fn filtered_child_does_not_overwrite_caller_fields() {
    let q = support::qso(support::id(1));
    let defs = support::definitions();
    let profiles = support::profiles();
    let context = ExportContext {
        output_profile_id: support::id(1),
        target_group_id: None,
    };
    let captured = support::telemetry::capture_filtered(
        || {
            let caller = tracing::info_span!("caller", outcome = "caller_pending");
            caller.in_scope(|| {
                plan_reference_export(
                    &[q].iter()
                        .map(clacks_model::export::evaluate::ReferenceExportInput::recorded)
                        .collect::<Vec<_>>(),
                    &context,
                    &defs,
                    &profiles,
                    &BTreeMap::new(),
                )
            })
        },
        |metadata| metadata.name() != "model.reference_export.plan",
    );
    assert_eq!(captured.result.as_ref().unwrap().source_contact_count, 1);
    let caller = captured
        .observations
        .spans
        .iter()
        .find(|s| s.name == "caller")
        .unwrap();
    assert_eq!(
        caller.fields.get("outcome").map(String::as_str),
        Some("caller_pending")
    );
    assert!(
        captured
            .observations
            .spans
            .iter()
            .all(|s| s.name != "model.reference_export.plan")
    );
    let event = captured.event("reference_export.planned");
    assert_eq!(event.parent, caller.id);
    assert_eq!(
        event.fields.get("source_contact_count").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        event.fields.get("readiness").map(String::as_str),
        Some("ready")
    );
    captured.event_count("reference_export.planned", "source_contact", 1.);
}

#[test]
fn filtered_export_summary_can_be_a_root_event() {
    let defs = support::definitions();
    let profiles = support::profiles();
    let q = support::qso(support::id(1));
    let captured = support::telemetry::capture_filtered(
        || {
            plan_reference_export(
                &[q].iter()
                    .map(clacks_model::export::evaluate::ReferenceExportInput::recorded)
                    .collect::<Vec<_>>(),
                &ExportContext {
                    output_profile_id: support::id(1),
                    target_group_id: None,
                },
                &defs,
                &profiles,
                &BTreeMap::new(),
            )
        },
        |metadata| !metadata.is_span(),
    );
    assert_eq!(captured.result.as_ref().unwrap().source_contact_count, 1);
    let event = captured.event("reference_export.planned");
    assert_eq!(event.parent, None);
    assert_eq!(
        event.fields.get("source_contact_count").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        event.fields.get("readiness").map(String::as_str),
        Some("ready")
    );
    captured.event_count("reference_export.planned", "source_contact", 1.);
}

#[test]
fn configured_module_filters_select_export_diagnostics_without_gating_export() {
    let qso = support::qso(support::id(1));
    let definitions = support::definitions();
    let profiles = support::profiles();
    let context = ExportContext {
        output_profile_id: support::id(1),
        target_group_id: None,
    };

    for (directive, visible) in [
        ("off,clacks_model=debug", true),
        ("off,clacks_sync=debug", false),
    ] {
        let captured = support::telemetry::capture_with_filter(
            || {
                plan_reference_export(
                    &(std::slice::from_ref(&qso))
                        .iter()
                        .map(clacks_model::export::evaluate::ReferenceExportInput::recorded)
                        .collect::<Vec<_>>(),
                    &context,
                    &definitions,
                    &profiles,
                    &BTreeMap::new(),
                )
            },
            tracing_subscriber::EnvFilter::new(directive),
        );

        assert_eq!(captured.result.as_ref().unwrap().source_contact_count, 1);
        if visible {
            let summary = captured.event("reference_export.planned");
            assert_eq!(
                summary.fields.get("readiness").map(String::as_str),
                Some("ready")
            );
            captured.event_count("reference_export.planned", "source_contact", 1.);
        } else {
            assert!(captured.observations.events.is_empty());
            assert!(captured.observations.spans.is_empty());
        }
    }
}

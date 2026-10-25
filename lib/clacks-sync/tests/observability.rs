//! Cross-library operational context and bounded measurements through real planners.
mod support;

use clacks_model::ids::AccountId;
use clacks_sync::{
    acceptance::{AcceptanceDecision, AcceptanceInput, ServerChange, plan_server_acceptance},
    account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
    client::VersionCondition,
    membership::MembershipSnapshot,
    record::{Observation, RecordId, RecordKey, RecordState, RecordValue},
};
use support::telemetry::{Capture, capture};

/// Seed an ordinary account-shared definition using actual native validation and acceptance.
fn seed(account_id: AccountId, group_id: u128) -> anyhow::Result<AcceptanceDecision> {
    let (account, baseline) =
        AccountMetadata::initialize(account_id, support::id(2), support::id(3));
    let bases = [baseline.clone()];
    let evidence = EvidenceView {
        lineage: &baseline.lineage,
        baselines: &bases,
    };
    let membership = MembershipSnapshot {
        account: account_id,
        generation: 0,
        clients: vec![],
    };
    let groups: Vec<clacks_model::group::GroupDefinition> =
        serde_json::from_str(include_str!("support/definitions.json"))?;
    let mut group = groups.into_iter().next().unwrap();
    group.id = support::id(group_id);
    let key = RecordKey {
        account: account_id,
        id: RecordId::Group(group.id),
    };
    let absent = Observation::Absent {
        key,
        checkpoint: InstalledCheckpoint {
            account: account_id,
            baseline: baseline.id,
            coverage: baseline.coverage.clone(),
        },
    };
    let change = ServerChange {
        key,
        state: RecordState::Live(RecordValue::Group(Box::new(group))),
        expected: VersionCondition::from_observation(&absent)?,
        dependencies: vec![],
    };
    plan_server_acceptance(
        &change,
        &AcceptanceInput {
            destination_registry: None,
            bindings: &[],
            account: &account,
            membership: &membership,
            current: &absent,
            references: &[],
        },
        &evidence,
    )
}

#[test]
fn parallel_accounts_keep_cross_library_operation_context() {
    let threads: Vec<_> = [101,202].into_iter().map(|account| std::thread::spawn(move || {
        let captured = capture(|| {
            let root = tracing::info_span!("caller",account_id=%support::id::<AccountId>(account),request_id=account);
            root.in_scope(|| seed(support::id(account),1))
        });
        assert!(matches!(captured.result,Ok(AcceptanceDecision::Commit(_))));

            let sync = captured.span("sync.acceptance.server.plan");
            assert_eq!(sync.fields.get("record_id"), Some(&support::id::<clacks_model::ids::GroupId>(1).to_string()));
        assert_eq!(sync.fields.get("account_id"),Some(&support::id::<AccountId>(account).to_string()));
        let root = captured.observations.spans.iter().find(|s|s.name=="caller").unwrap();
        assert_eq!(sync.parent,root.id);
        for model in captured.observations.spans.iter().filter(|s|s.name=="model.reference_definition.compile") {
            assert_eq!(model.parent,sync.id);
        }
        assert!(captured.observations.spans.iter().any(|s|s.name=="model.reference_definition.compile"));
        captured
    })).collect();
    for thread in threads {
        let _ = thread.join().unwrap();
    }
}

/// Read the actual metric dimensions without assuming any recorder storage order.
fn dimensions<T>(
    capture: &Capture<T>,
) -> std::collections::BTreeSet<(String, std::collections::BTreeMap<String, String>)> {
    capture
        .metrics
        .iter()
        .map(|m| (m.name.clone(), m.labels.clone()))
        .collect()
}

#[test]
fn account_and_record_ids_do_not_multiply_metric_series() {
    let a = capture(|| seed(support::id(101), 11));
    let b = capture(|| seed(support::id(202), 22));
    assert!(a.result.is_ok());
    assert!(b.result.is_ok());
    assert_ne!(
        a.span("sync.acceptance.server.plan")
            .fields
            .get("record_id"),
        b.span("sync.acceptance.server.plan")
            .fields
            .get("record_id")
    );
    assert_eq!(dimensions(&a), dimensions(&b));
    assert_eq!(a.metrics.len(), 1);
    a.counter(
        "clacks_sync_acceptance_plans_total",
        &[("origin", "server"), ("decision", "proposed")],
        1,
    );
}

#[test]
fn service_operations_preserve_input_scope_and_missing_record_identity() -> anyhow::Result<()> {
    use clacks_model::{
        scalar::UtcTimestamp,
        service::{
            destination::{Destination, ExternalRecordKey, RemoteCollection, RemoteRecordId},
            evidence::{
                ServicePayload,
                processing::{AssociationState, ExternalAssociation},
                submission::{DispatchPhase, SubmissionAttempt, SubmissionProjection},
            },
            qrz::QrzSubmission,
        },
    };
    use clacks_sync::service::{
        ExternalBindingObservation, ServiceRecord, ServiceSupport, ServiceView,
        association::{BindingInput, plan_binding},
        combination::{ObservationSelectionInput, select_observations},
        dependency::{self, DependencyStatus},
        operation::ServiceDecision,
    };
    let (account, baseline) =
        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let bases = [baseline.clone()];
    let evidence = EvidenceView {
        lineage: &baseline.lineage,
        baselines: &bases,
    };
    let view = ServiceView {
        account: account.account,
        records: &[],
        bindings: &[],
        evidence: &evidence,
    };
    let target = Observation::Accepted(support::accepted(200, "fixture"));
    for n in [42, 43] {
        let destination = Destination {
            id: support::id(n),
            collection: RemoteCollection::Qrz {
                book_id: Some(n.to_string().parse().unwrap()),
            },
            description: "not telemetry".into(),
        };
        let attempt = SubmissionAttempt {
            id: support::id(n + 100),
            qso: support::id(10),
            projection: SubmissionProjection::Qrz(QrzSubmission {
                destination: destination.id,
                fields: Default::default(),
            }),
            phase: DispatchPhase::Intent,
            at: UtcTimestamp::new(0, 0)?,
            batch: None,
        };
        let record = ServiceRecord {
            payload: ServicePayload::Attempt(attempt.clone()),
            support: ServiceSupport {
                destination: Some(destination.clone()),
                ..Default::default()
            },
        };
        let captured = capture(|| dependency::check(&record, &view));
        assert!(
            matches!(captured.result, Ok(DependencyStatus::Complete)),
            "{:?}",
            captured.result
        );
        let span = captured.span("sync.evidence.dependencies.check");
        assert_eq!(
            span.fields.get("account_id"),
            Some(&account.account.to_string())
        );
        let external = ExternalRecordKey {
            destination: destination.id,
            record: RemoteRecordId::Qrz("7".parse().unwrap()),
        };
        let index = ExternalBindingObservation {
            account: account.account,
            external: external.clone(),
            binding: None,
        };
        let association = ExternalAssociation {
            id: support::id(n + 200),
            external: external.clone(),
            qso: support::id(10),
            state: AssociationState::Active,
        };
        let unsupported = ServiceRecord {
            payload: ServicePayload::Association(association.clone()),
            support: ServiceSupport {
                destination: Some(destination.clone()),
                ..Default::default()
            },
        };
        let captured = capture(|| dependency::check(&unsupported, &view));
        let Ok(DependencyStatus::Incomplete(missing)) = &captured.result else {
            panic!("missing contact support")
        };
        let event = captured.event("evidence.dependencies_missing");
        assert_eq!(
            event.fields.get("missing_dependency_count"),
            Some(&missing.len().to_string())
        );
        assert_eq!(
            event.fields.get("record_id"),
            Some(&association.id.to_string())
        );
        assert_eq!(
            event.fields.get("first_missing_record_id"),
            Some(&missing.first().unwrap().key.id.to_string())
        );
        assert!(!event.fields.contains_key("duration_seconds"));
        let captured = capture(|| {
            plan_binding(&BindingInput {
                account: account.account,
                association: &association,
                index: &index,
                target: &target,
            })
        });
        assert!(matches!(captured.result, Ok(ServiceDecision::Ready(_))));
        let span = captured.span("sync.association.binding.plan");
        assert_eq!(
            span.fields.get("association_id"),
            Some(&association.id.to_string())
        );
        assert_eq!(
            span.fields.get("destination_id"),
            Some(&destination.id.to_string())
        );
        let captured = capture(|| {
            select_observations(&ObservationSelectionInput {
                external: &external,
                view: &view,
            })
        });
        assert!(captured.result.is_ok());
        assert_eq!(
            captured
                .span("sync.observations.select")
                .fields
                .get("destination_id"),
            Some(&destination.id.to_string())
        );
    }
    Ok(())
}

#[test]
fn configured_module_filters_separate_acceptance_from_model_validation_diagnostics() {
    for (directive, acceptance_visible) in [
        ("off,clacks_sync=debug,clacks_model=off", true),
        ("off,clacks_sync=off,clacks_model=trace", false),
    ] {
        let captured = support::telemetry::capture_with_filter(
            || seed(support::id(101), 11),
            tracing_subscriber::EnvFilter::new(directive),
        );

        assert!(matches!(captured.result, Ok(AcceptanceDecision::Commit(_))));
        let acceptance = captured.observations.events.iter().find(|event| {
            event
                .fields
                .get("event")
                .is_some_and(|name| name == "acceptance.decided")
        });
        if acceptance_visible {
            let event = acceptance.expect("acceptance decision must be visible");
            assert_eq!(
                event.fields.get("decision").map(String::as_str),
                Some("proposed")
            );
            assert_eq!(
                event.fields.get("origin").map(String::as_str),
                Some("server")
            );
            assert_eq!(
                captured
                    .span("sync.acceptance.server.plan")
                    .fields
                    .get("account_id"),
                Some(&support::id::<AccountId>(101).to_string())
            );
            assert!(
                captured
                    .observations
                    .spans
                    .iter()
                    .all(|span| span.name != "model.reference_definition.compile")
            );
        } else {
            assert!(acceptance.is_none());
            assert!(
                captured
                    .observations
                    .spans
                    .iter()
                    .any(|span| span.name == "model.reference_definition.compile")
            );
            assert!(
                captured
                    .observations
                    .spans
                    .iter()
                    .all(|span| span.name != "sync.acceptance.server.plan")
            );
        }
    }
}

#[test]
fn witnessed_regression_reports_its_scope_without_a_completion_wrapper() -> anyhow::Result<()> {
    use clacks_sync::{
        lineage::Coverage,
        recovery::{HistoryCheck, HistoryObservation, check_history},
    };
    let (account, baseline) =
        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let bases = [baseline.clone()];
    let evidence = EvidenceView {
        lineage: &baseline.lineage,
        baselines: &bases,
    };
    let observation = HistoryObservation {
        account: account.account,
        advertised: account.head,
        remembered: Coverage::from_entries([(account.head.timeline, 200)])?,
    };
    let captured = support::telemetry::capture_filtered(
        || check_history(&observation, &evidence),
        |metadata| *metadata.level() <= tracing::Level::INFO,
    );
    assert!(matches!(
        captured.result,
        Ok(HistoryCheck::Regression { .. })
    ));
    let event = captured.event("history.regression_detected");
    assert_eq!(event.fields.get("level").map(String::as_str), Some("WARN"));
    assert!(captured.observations.spans.is_empty());
    assert_eq!(event.target, "clacks_sync::recovery");
    assert_eq!(
        event.fields.get("account_id"),
        Some(&account.account.to_string())
    );
    assert_eq!(
        event.fields.get("timeline_id"),
        Some(&account.head.timeline.to_string())
    );
    assert_eq!(
        event.fields.get("advertised_revision"),
        Some(&account.head.revision.to_string())
    );
    assert_eq!(
        event.fields.get("remembered_revision").map(String::as_str),
        Some("200")
    );
    assert!(!event.fields.contains_key("duration_seconds"));
    assert!(captured.metrics.is_empty());
    Ok(())
}

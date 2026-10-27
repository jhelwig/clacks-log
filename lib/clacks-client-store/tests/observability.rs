use crate::support;
use crate::telemetry;
use clacks_client_store::{
    save::{self, SaveOutcome},
    store::{ClientStore, StoreSettings},
    transfer,
};
use clacks_sync::{
    account::{AccountMetadata, InstalledCheckpoint},
    lineage::Coverage,
    transfer::{RecordRequirement, WindowManifest, WindowMode},
};

#[test]
fn telemetry_counts_only_committed_domain_changes_and_excludes_payloads() -> anyhow::Result<()> {
    let captured = telemetry::capture_filtered(
        || -> anyhow::Result<()> {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(async {
                    let dir = tempfile::tempdir()?;
                    let store = ClientStore::create(
                        &dir.path().join("sensitive-file-name.sqlite"),
                        support::id(1),
                        StoreSettings::default(),
                    )
                    .await?;
                    let key = support::qso_key(support::id(10));
                    let mut qso = support::qso(support::id(10));
                    qso.comment = Some("DO NOT EMIT THIS COMMENT".into());
                    let SaveOutcome::Saved(first) =
                        save::save(&store, key, None, support::contact(qso.clone())).await?
                    else {
                        panic!("save")
                    };
                    assert!(matches!(
                        save::save(&store, key, Some(&first), support::contact(qso.clone()))
                            .await?,
                        SaveOutcome::Unchanged(_)
                    ));
                    assert!(matches!(
                        save::save(&store, key, None, support::contact(qso.clone())).await?,
                        SaveOutcome::Stale { .. }
                    ));
                    qso.comment = Some("OTHER PRIVATE COMMENT".into());
                    let SaveOutcome::Saved(next) =
                        save::save(&store, key, Some(&first), support::contact(qso)).await?
                    else {
                        panic!("save")
                    };
                    save::save(
                        &store,
                        key,
                        Some(&next),
                        clacks_sync::record::RecordState::Deleted,
                    )
                    .await?;
                    // Transfer uses a different record and counts a newly incorporated
                    // envelope once, even when the same successful input is replayed.
                    let qso = support::accepted(1, support::contact(support::qso(support::id(11))));
                    let (_, baseline) =
                        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
                    let source = InstalledCheckpoint {
                        account: support::id(1),
                        baseline: baseline.id,
                        coverage: Coverage::from(qso.stamp.accepted_at),
                    };
                    let manifest = WindowManifest::new(
                        source.account,
                        source.clone(),
                        WindowMode::Full,
                        vec![RecordRequirement {
                            key: qso.key,
                            stamp: qso.stamp.clone(),
                            deleted: false,
                        }],
                        vec![],
                        vec![baseline],
                        vec![],
                    )?;
                    transfer::remember(&store, &manifest).await?;
                    for _ in 0..2 {
                        transfer::install(
                            &store,
                            &source,
                            &WindowMode::Full,
                            std::slice::from_ref(&qso),
                        )
                        .await?;
                    }
                    transfer::finish(&store, &source, &WindowMode::Full).await?;
                    store.close().await?;
                    Ok(())
                })
        },
        |metadata| *metadata.level() <= tracing::Level::INFO,
    );
    captured
        .result
        .as_ref()
        .map_err(|error| anyhow::anyhow!("{error:#}"))?;
    for change in ["create", "update", "delete"] {
        captured.counter(
            "clacks_client_qso_mutations_total",
            &[("change", change)],
            1,
        );
    }
    captured.counter("clacks_client_sync_records_installed_total", &[], 1);
    assert_eq!(
        captured
            .observations
            .events
            .iter()
            .filter(|event| event
                .fields
                .get("event")
                .is_some_and(|value| value == "record.saved"))
            .count(),
        3
    );
    assert!(captured.observations.spans.iter().all(|span| {
        span.fields
            .get("level")
            .is_none_or(|level| level != "DEBUG" && level != "TRACE")
    }));
    let all = format!("{captured:?}");
    for sensitive in [
        "DO NOT EMIT",
        "OTHER PRIVATE",
        "sensitive-file-name",
        "W7HEX",
        "K1ABC",
        "SELECT ",
        "INSERT ",
    ] {
        assert!(!all.contains(sensitive), "leaked {sensitive}");
    }
    assert!(captured.observations.events.iter().any(|event| {
        event
            .fields
            .get("event")
            .is_some_and(|value| value == "sync.window.completed")
            && event.fields.contains_key("account_id")
    }));
    for metric in captured
        .metrics
        .iter()
        .filter(|metric| metric.name.starts_with("clacks_client_"))
    {
        assert!(metric.labels.keys().all(|key| key == "change"));
    }
    Ok(())
}

#[test]
fn committed_resolution_keeps_context_with_debug_spans_filtered() -> anyhow::Result<()> {
    use clacks_client_store::{
        identity::{self, EnrollmentBinding},
        synchronization::{self, PrepareOutcome},
    };
    use clacks_sync::{acceptance::RejectionReason, client::Preparation};
    let captured = telemetry::capture_filtered(
        || -> anyhow::Result<()> {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(async {
                    let directory = tempfile::tempdir()?;
                    let store = ClientStore::create(
                        &directory.path().join("workspace.sqlite"),
                        support::id(1),
                        StoreSettings::default(),
                    )
                    .await?;
                    identity::bind(
                        &store,
                        None,
                        EnrollmentBinding {
                            account: support::id(1),
                            actor: support::id(9),
                            server: "https://private-endpoint.invalid".into(),
                            credential_reference: "DO NOT EMIT CREDENTIAL REFERENCE".into(),
                        },
                    )
                    .await?;
                    let (account, baseline) =
                        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
                    let source = InstalledCheckpoint {
                        account: account.account,
                        baseline: baseline.id,
                        coverage: account.installed,
                    };
                    let manifest = WindowManifest::new(
                        source.account,
                        source.clone(),
                        WindowMode::Full,
                        vec![],
                        vec![],
                        vec![baseline],
                        vec![],
                    )?;
                    transfer::remember(&store, &manifest).await?;
                    transfer::finish(&store, &source, &WindowMode::Full).await?;
                    let key = support::qso_key(support::id(10));
                    save::save(
                        &store,
                        key,
                        None,
                        support::contact(support::qso(support::id(10))),
                    )
                    .await?;
                    let PrepareOutcome::Core(prepared) =
                        synchronization::prepare(&store, key).await?
                    else {
                        panic!("enrolled")
                    };
                    assert!(matches!(*prepared, Preparation::Ready { .. }));
                    let rejected = synchronization::reject(
                        &store,
                        key,
                        &RejectionReason::ActorUnknown {
                            actor: support::id(9),
                        },
                    )
                    .await?;
                    let shown = rejected.review.as_ref().unwrap();
                    assert!(matches!(
                        synchronization::resolve(
                            &store,
                            &rejected.record,
                            shown,
                            shown.local.clone()
                        )
                        .await?,
                        SaveOutcome::Saved(_)
                    ));
                    store.close().await?;
                    Ok(())
                })
        },
        |metadata| *metadata.level() <= tracing::Level::INFO,
    );
    captured
        .result
        .as_ref()
        .map_err(|error| anyhow::anyhow!("{error:#}"))?;
    let event = captured.event("record.resolved");
    assert_eq!(
        event.fields.get("account_id"),
        Some(&support::id::<clacks_model::ids::AccountId>(1).to_string())
    );
    assert!(event.fields.contains_key("record_id") && event.fields.contains_key("review_reason"));
    let text = format!("{captured:?}");
    assert!(!text.contains("DO NOT EMIT CREDENTIAL") && !text.contains("private-endpoint"));
    Ok(())
}

//! Real bulk consumers retain cache identity, committed prefixes and safe outcomes.
use super::super::bulk::{
    BulkLookupHandle,
    report::{BulkCounts, BulkStop, BulkUpdate},
};
use super::*;
use clacks_client_store::save;
use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};

fn id(value: u128) -> QsoId {
    ulid::Ulid::from(value).to_string().parse().unwrap()
}

pub(crate) async fn finish(
    handle: &mut BulkLookupHandle,
) -> anyhow::Result<Arc<super::super::bulk::report::BulkReport>> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let BulkUpdate::Finished(report) = handle.updates.borrow().clone() {
                return Ok(report);
            }
            handle.updates.changed().await?;
        }
    })
    .await?
}

#[tokio::test]
async fn bulk_lookup_cancelled_before_preparation_reports_lookup_never_started()
-> anyhow::Result<()> {
    let fixture = fixture(vec![]).await?;
    let service = Service::new(fixture.endpoint.clone()).await?;
    let mut run = service
        .client
        .start_bulk([id(1)].into(), watch::channel(()).0)?;
    // The current-thread executor cannot poll the admitted task until we yield.
    run.cancel.cancel();
    let report = finish(&mut run).await?;
    assert!(matches!(report.stop, BulkStop::PreparationCancelled));
    assert_eq!(report.counts.unprocessed, 1);
    assert_eq!(report.counts.settled(), report.total);
    assert_eq!(lookup_count(&fixture), 0);
    service.close().await
}

#[tokio::test]
async fn bulk_lookup_stops_current_rate_limits_and_missing_configuration() -> anyhow::Result<()> {
    for configured in [true, false] {
        let fixture = fixture(if configured {
            vec![login(), response(429, "")]
        } else {
            vec![]
        })
        .await?;
        let service = Service::new(fixture.endpoint.clone()).await?;
        if !configured {
            service.available(false);
        }
        for n in [1, 2] {
            save::contacts::save(&service.store, None, contact(id(n), "W7TEST"), None).await?;
        }
        let mut run = service
            .client
            .start_bulk([id(1), id(2)].into(), watch::channel(()).0)?;
        let report = finish(&mut run).await?;
        assert_eq!(
            report.counts,
            BulkCounts {
                failed: 1,
                unprocessed: 1,
                ..Default::default()
            }
        );
        assert_eq!(lookup_count(&fixture), usize::from(configured));
        assert!(retained(&service, id(1)).await?.is_none());
        service.close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn bulk_lookup_obsolete_material_rejection_does_not_stop_replacement() -> anyhow::Result<()> {
    let (held, release) = delayed();
    let old = fixture(vec![login(), held, response(401, "")]).await?;
    let service = Service::new(old.endpoint.clone()).await?;
    for (n, call) in [(1, "OK2CQR"), (2, "W7NEXT")] {
        save::contacts::save(&service.store, None, contact(id(n), call), None).await?;
    }
    let mut run = service
        .client
        .start_bulk([id(1), id(2)].into(), watch::channel(()).0)?;
    requests(&old, 2).await;
    let replacement = fixture(vec![login(), found()]).await?;
    service.replace(replacement.endpoint.clone())?;
    release
        .send("<QRZDatabase><Session><Error>Session Timeout</Error></Session></QRZDatabase>")
        .unwrap();
    let report = finish(&mut run).await?;
    assert_eq!(
        report.counts,
        BulkCounts {
            new_updated: 1,
            failed: 1,
            ..Default::default()
        }
    );
    assert!(matches!(report.stop, BulkStop::Completed));
    assert_eq!(lookup_count(&replacement), 1);
    service.close().await
}

#[tokio::test]
async fn bulk_lookup_rate_failure_tracks_actual_configuration_not_reused_version_fields()
-> anyhow::Result<()> {
    let old = fixture(vec![login(), response(429, "")]).await?;
    let service = Service::new(old.endpoint.clone()).await?;
    let outcome = service.client.request(key("W7RATE")).wait().await;
    let LookupState::Failed(failure) = outcome.as_ref() else {
        panic!("rate failure");
    };
    let origin = failure.origin.as_ref().unwrap();
    assert!(service.client.is_current_origin(origin));
    let replacement = fixture(vec![login(), found()]).await?;
    {
        let mut state = service.client.inner.state.lock().unwrap();
        let Ok(Some(selected)) = &mut state.selected else {
            panic!("configuration");
        };
        selected.origin = Arc::new(());
        selected.material.login.password = "replacement-with-same-id-and-revision".into();
        selected.session = ProviderSession::Qrz(QrzSession::new(
            providers::client()?,
            replacement.endpoint.clone(),
            selected.material.clone(),
        ));
    }
    assert!(!service.client.is_current_origin(origin));
    save::contacts::save(&service.store, None, contact(id(1), "OK2CQR"), None).await?;
    let mut run = service
        .client
        .start_bulk([id(1)].into(), watch::channel(()).0)?;
    assert_eq!(finish(&mut run).await?.counts.new_updated, 1);
    service.close().await
}

#[tokio::test]
async fn bulk_lookup_rechecks_deleted_parents_and_changed_keys_at_application() -> anyhow::Result<()>
{
    for delete in [true, false] {
        let (held, release) = delayed();
        let fixture = fixture(vec![login(), held]).await?;
        let service = Service::new(fixture.endpoint.clone()).await?;
        let mut original = contact(id(1), "OK2CQR");
        let save::SaveOutcome::Saved(basis) =
            save::contacts::save(&service.store, None, original.clone(), None).await?
        else {
            panic!("save");
        };
        let mut run = service
            .client
            .start_bulk([id(1)].into(), watch::channel(()).0)?;
        requests(&fixture, 2).await;
        if delete {
            save::save(
                &service.store,
                basis.key(),
                Some(&basis),
                RecordState::Deleted,
            )
            .await?;
        } else {
            crate::test_support::edit(&mut original, |fields| {
                fields.contacted_station.callsign = RecordedCallsign::new("W7CHANGED")?;
                Ok(())
            });
            save::contacts::save(&service.store, Some(&basis), original, None).await?;
        }
        release
            .send(include_str!("../../providers/fixtures/qrz.xml"))
            .unwrap();
        let report = finish(&mut run).await?;
        assert_eq!(
            report.counts,
            BulkCounts {
                unprocessed: 1,
                ..Default::default()
            }
        );
        assert!(retained(&service, id(1)).await?.is_none());
        service.close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn bulk_lookup_storage_refusal_preserves_committed_prefix() -> anyhow::Result<()> {
    let (held, release) = delayed();
    let fixture = fixture(vec![login(), found(), held]).await?;
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("store.sqlite");
    let store = ClientStore::create(&file, Default::default(), Default::default()).await?;
    let service = Service::from_store(fixture.endpoint.clone(), store, directory)?;
    service.client.request(key("OK2CQR")).wait().await;
    for (n, call) in [(1, "OK2CQR"), (2, "W7WAIT"), (3, "W7LATER")] {
        save::contacts::save(&service.store, None, contact(id(n), call), None).await?;
    }
    let (changes, mut changed) = watch::channel(());
    let mut run = service
        .client
        .start_bulk([id(1), id(2), id(3)].into(), changes)?;
    tokio::time::timeout(Duration::from_secs(5), changed.changed()).await??;
    requests(&fixture, 3).await;
    service.store.close().await?;
    release
        .send(include_str!("../../providers/fixtures/qrz.xml"))
        .unwrap();
    let report = finish(&mut run).await?;
    assert_eq!(
        report.counts,
        BulkCounts {
            new_updated: 1,
            failed: 1,
            unprocessed: 1,
            ..Default::default()
        }
    );
    let reopened = ClientStore::open(&file, service.store.account_id(), Default::default()).await?;
    assert!(
        save::load(
            &reopened,
            RecordKey {
                account: reopened.account_id(),
                id: RecordId::Directory(id(1))
            }
        )
        .await?
        .is_some()
    );
    reopened.close().await?;
    service.close().await
}

async fn retained(service: &Service, qso: QsoId) -> anyhow::Result<Option<DirectorySnapshot>> {
    let row = save::load(
        &service.store,
        RecordKey {
            account: service.store.account_id(),
            id: RecordId::Directory(qso),
        },
    )
    .await?;
    Ok(match row.as_ref().and_then(|row| row.working_state()) {
        Some(RecordState::Live(RecordValue::Directory(value))) => value.snapshots().next().cloned(),
        _ => None,
    })
}

#[tokio::test]
async fn bulk_lookup_reuses_success_and_reports_real_partial_results() -> anyhow::Result<()> {
    let fixture = fixture(vec![
        login(),
        found(),
        response(
            200,
            "<QRZDatabase><Session><Error>Not found: unknown</Error></Session></QRZDatabase>",
        ),
        response(200, "<broken"),
        response(401, ""),
    ])
    .await?;
    let service = Service::new(fixture.endpoint.clone()).await?;
    let cached = service.client.request(key("OK2CQR")).wait().await;
    for (n, call) in [
        (1, "OK2CQR"),
        (2, "OK2CQR/P"),
        (3, "OK2CQR"),
        (4, "W7NONE"),
        (5, "W7BROKEN"),
        (6, "W7AUTH"),
        (7, "W7LATER"),
    ] {
        save::contacts::save(
            &service.store,
            None,
            contact(id(n), call),
            (n == 3).then(|| snapshot(&cached).clone()),
        )
        .await?;
    }
    let mut run = service
        .client
        .start_bulk((1..=7).map(id).collect(), watch::channel(()).0)?;
    let report = finish(&mut run).await?;
    assert_eq!(
        report.counts,
        BulkCounts {
            new_updated: 2,
            unchanged: 1,
            not_found: 1,
            failed: 2,
            unprocessed: 1
        }
    );
    assert_eq!(report.total, 7);
    assert_eq!(report.details.len(), 3);
    assert!(matches!(report.stop, BulkStop::Stopped { .. }));
    for n in [1, 2, 3] {
        assert_eq!(
            retained(&service, id(n)).await?.as_ref(),
            Some(snapshot(&cached))
        );
    }
    assert_eq!(lookup_count(&fixture), 4);
    let recorded = save::load(
        &service.store,
        RecordKey {
            account: service.store.account_id(),
            id: RecordId::Qso(id(2)),
        },
    )
    .await?
    .unwrap();
    assert_eq!(
        recorded.working_state(),
        Some(&RecordState::Live(RecordValue::Qso(Box::new(contact(
            id(2),
            "OK2CQR/P"
        )))))
    );
    service.close().await
}

#[tokio::test]
async fn bulk_lookup_cancellation_keeps_commits_without_owning_shared_work() -> anyhow::Result<()> {
    let (held, release) = delayed();
    let fixture = fixture(vec![login(), found(), held]).await?;
    let service = Service::new(fixture.endpoint.clone()).await?;
    service.client.request(key("OK2CQR")).wait().await;
    for (n, call) in [(1, "OK2CQR"), (2, "W7WAIT")] {
        save::contacts::save(&service.store, None, contact(id(n), call), None).await?;
    }
    let ordinary = service.client.request(key("W7WAIT"));
    requests(&fixture, 3).await;
    let (changes, mut changed) = watch::channel(());
    let mut run = service.client.start_bulk([id(1), id(2)].into(), changes)?;
    tokio::time::timeout(Duration::from_secs(5), changed.changed()).await??;
    run.cancel.cancel();
    let report = finish(&mut run).await?;
    assert_eq!(
        report.counts,
        BulkCounts {
            new_updated: 1,
            unprocessed: 1,
            ..Default::default()
        }
    );
    assert!(matches!(report.stop, BulkStop::Cancelled));
    release
        .send(include_str!("../../providers/fixtures/qrz.xml"))
        .unwrap();
    assert!(matches!(
        ordinary.wait().await.as_ref(),
        LookupState::Found(_)
    ));
    assert!(retained(&service, id(1)).await?.is_some());
    assert!(retained(&service, id(2)).await?.is_none());
    service.close().await
}

#[tokio::test]
async fn bulk_lookup_metadata_commit_notifies_history_but_reports_unchanged() -> anyhow::Result<()>
{
    let fixture = fixture(vec![login(), found()]).await?;
    let service = Service::new(fixture.endpoint.clone()).await?;
    let success = service.client.request(key("OK2CQR")).wait().await;
    let mut fields: clacks_model::directory::values::DirectorySnapshotFields =
        snapshot(&success).clone().into();
    fields.lookup_time = clacks_model::scalar::UtcTimestamp::new(1, 0)?;
    fields.lookup_id = ulid::Ulid::from(1_u128).to_string().parse()?;
    save::contacts::save(
        &service.store,
        None,
        contact(id(1), "OK2CQR"),
        Some(DirectorySnapshot::new(fields)?),
    )
    .await?;
    let (changes, mut changed) = watch::channel(());
    let mut run = service.client.start_bulk([id(1)].into(), changes)?;
    let report = finish(&mut run).await?;
    assert_eq!(
        report.counts,
        BulkCounts {
            unchanged: 1,
            ..Default::default()
        }
    );
    tokio::time::timeout(Duration::from_secs(1), changed.changed()).await??;
    assert_eq!(
        retained(&service, id(1)).await?.as_ref(),
        Some(snapshot(&success))
    );
    service.close().await
}

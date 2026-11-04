//! Real saved-contact application across debounce, deletion and call changes.
use super::*;
#[tokio::test]
async fn manual_save_before_lookup_due_keeps_original_deadline() -> anyhow::Result<()> {
    let (held, release) = delayed();
    let fixture = fixture(vec![login(), held]).await?;
    let service = Service::new(fixture.endpoint.clone()).await?;
    let qso = QsoId::new();
    let (changes, mut changed) = watch::channel(());
    tokio::time::pause();
    let due = Instant::now() + Duration::from_millis(500);
    tokio::time::advance(Duration::from_millis(100)).await;
    let prepared = persistence::prepare(
        service.client.clone(),
        service.store.clone(),
        qso,
        LookupDemand {
            key: key("OK2CQR"),
            due,
            handle: None,
        },
        changes,
    );
    // Save is immediately available independently of the sleeping directory demand.
    tokio::time::resume();
    clacks_client_store::save::contacts::save(
        &service.store,
        None,
        contact(qso, "OK2CQR"),
        prepared.ready_snapshot().map(|value| *value),
    )
    .await?;
    prepared.settle(true);
    tokio::time::pause();
    let until_due = due.saturating_duration_since(Instant::now());
    if until_due > Duration::from_millis(1) {
        tokio::time::advance(until_due - Duration::from_millis(1)).await;
    }
    assert_eq!(lookup_count(&fixture), 0);
    tokio::time::advance(Duration::from_millis(1)).await;
    tokio::time::resume();
    requests(&fixture, 2).await;
    release
        .send(include_str!("../../providers/fixtures/qrz.xml"))
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), changed.changed()).await??;
    let row = clacks_client_store::save::load(
        &service.store,
        clacks_sync::record::RecordKey {
            account: service.store.account_id(),
            id: clacks_sync::record::RecordId::Directory(qso),
        },
    )
    .await?;
    assert!(row.is_some());
    service.close().await
}

#[tokio::test]
async fn directory_lookup_late_response_is_saved_only_on_its_original_contact() -> anyhow::Result<()>
{
    let dir = tempfile::tempdir()?;
    ownership_scenario(&dir.path().join("store.sqlite")).await?;
    Ok(())
}

#[tokio::test]
async fn saved_editor_keeps_lookup_trigger_and_result_guards() -> anyhow::Result<()> {
    use clacks_client_store::save::{self, SaveOutcome};
    use clacks_sync::record::{RecordId, RecordKey, RecordState};
    for delete in [false, true] {
        let (held, release) = delayed();
        let fixture = fixture(vec![login(), held]).await?;
        let service = Service::new(fixture.endpoint.clone()).await?;
        let id = QsoId::new();
        let original = contact(id, "OK2CQR");
        let SaveOutcome::Saved(basis) =
            save::contacts::save(&service.store, None, original.clone(), None).await?
        else {
            panic!("save")
        };
        let handle = service.client.request(key("OK2CQR"));
        requests(&fixture, 2).await;
        let (changes, _changed) = watch::channel(());
        let prepared = persistence::prepare(
            service.client.clone(),
            service.store.clone(),
            id,
            LookupDemand {
                key: key("OK2CQR"),
                due: Instant::now(),
                handle: Some(handle.clone()),
            },
            changes,
        );
        prepared.settle(true);
        if delete {
            save::save(
                &service.store,
                basis.key(),
                Some(&basis),
                RecordState::Deleted,
            )
            .await?;
        } else {
            save::contacts::save(&service.store, Some(&basis), contact(id, "W7OTHER"), None)
                .await?;
        }
        release
            .send(include_str!("../../providers/fixtures/qrz.xml"))
            .unwrap();
        handle.wait().await;
        // Shutdown waits for saved-result application to finish.
        service.client.inner.tasks.close();
        service.client.inner.tasks.wait().await;
        let directory = save::load(
            &service.store,
            RecordKey {
                account: service.store.account_id(),
                id: RecordId::Directory(id),
            },
        )
        .await?;
        assert!(directory.is_none());
        service.close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn late_snapshot_commit_requests_its_own_repaint() -> anyhow::Result<()> {
    let (held, release) = delayed();
    let fixture = fixture(vec![login(), held]).await?;
    let service = Service::new(fixture.endpoint.clone()).await?;
    let context = eframe::egui::Context::default();
    assert!(service.client.inner.context.set(context.clone()).is_ok());
    let (changes, changed) = watch::channel(());
    let (wake, mut wakes) = tokio::sync::mpsc::unbounded_channel();
    context.set_repaint_observer(move |_| {
        let _ = wake.send(changed.has_changed().unwrap_or(false));
    });
    let qso = QsoId::new();
    let handle = service.client.request(key("OK2CQR"));
    let prepared = persistence::prepare(
        service.client.clone(),
        service.store.clone(),
        qso,
        LookupDemand {
            key: key("OK2CQR"),
            due: Instant::now(),
            handle: Some(handle.clone()),
        },
        changes,
    );
    clacks_client_store::save::contacts::save(&service.store, None, contact(qso, "OK2CQR"), None)
        .await?;
    requests(&fixture, 2).await;
    release
        .send(include_str!("../../providers/fixtures/qrz.xml"))
        .unwrap();
    handle.wait().await;
    // The save gate is still closed: observe the provider wake before allowing application.
    assert!(
        !tokio::time::timeout(Duration::from_secs(5), wakes.recv())
            .await?
            .unwrap()
    );
    prepared.settle(true);
    tokio::time::timeout(Duration::from_secs(5), async {
        while !wakes.recv().await.unwrap() {}
    })
    .await?;
    service.settled().await;
    service.close().await
}

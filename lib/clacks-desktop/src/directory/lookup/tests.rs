//! Clock and captured-owner checks at the lookup admission boundary.

use super::*;

#[tokio::test(start_paused = true)]
async fn directory_lookup_debounces_actual_calls_and_preserves_saved_owner() {
    let qso = QsoId::new();
    let mut entries = BTreeMap::new();
    observe(&mut entries, qso, "w7test/p");
    let first = entries[&qso].due.unwrap();
    tokio::time::advance(Duration::from_millis(499)).await;
    assert!(first > Instant::now());
    observe(&mut entries, qso, " W7TEST ");
    assert_eq!(entries[&qso].due, Some(first));
    tokio::time::advance(Duration::from_millis(1)).await;
    assert_eq!(first, Instant::now());
    let key = entries[&qso].key.clone();
    observe(&mut entries, QsoId::new(), "");
    assert_eq!(entries[&qso].key, key);
    observe(&mut entries, qso, "");
    assert!(!entries.contains_key(&qso));
}

#[tokio::test]
async fn directory_lookup_does_not_pause_new_material_for_old_rejection() -> anyhow::Result<()> {
    use super::super::{
        providers::{self, qrz::QrzSession},
        test_support::{fixture, key, material, response},
    };
    let current_material = material();
    let mut old_material = current_material.clone();
    old_material.login.password = "old-rejected-login".into();
    let old = fixture(vec![response(401, "")]).await?;
    let old_session = QrzSession::new(
        providers::client()?,
        old.endpoint.clone(),
        old_material.clone(),
    );
    let pending = tokio::spawn(async move { old_session.lookup(&key()).await });
    requests(&old, 1).await;
    let new = fixture(vec![response(200, "<QRZDatabase><Session><Key>new-key</Key></Session></QRZDatabase>"), response(200, "<QRZDatabase><Session><Key>new-key</Key><Error>Not found: unknown</Error></Session></QRZDatabase>")]).await?;
    let session = ProviderSession::Qrz(QrzSession::new(
        providers::client()?,
        new.endpoint.clone(),
        current_material.clone(),
    ));
    let mut current = Ok(Some(Selected {
        material: current_material.clone(),
        provider: clacks_model::directory::values::DirectoryProvider::Qrz,
        session,
        paused: false,
        retry_after: None,
    }));
    assert!(matches!(pending.await?, Err(ProviderError::Authentication)));
    assert!(matches!(
        authentication_rejected(
            &mut current,
            clacks_model::directory::values::DirectoryProvider::Qrz,
            &old_material
        ),
        LookupState::Failed(_)
    ));
    let selected = current.as_ref().unwrap().as_ref().unwrap();
    assert!(!selected.paused);
    assert!(matches!(
        selected.session.lookup(&key()).await?,
        ProviderLookup::NotFound
    ));
    assert_eq!(new.requests.lock().unwrap().len(), 2);
    assert!(matches!(
        authentication_rejected(
            &mut current,
            clacks_model::directory::values::DirectoryProvider::Qrz,
            &current_material
        ),
        LookupState::PausedAuthentication
    ));
    assert!(current.unwrap().unwrap().paused);
    Ok(())
}

/// Let real socket dispatch settle without advancing simulated debounce time.
async fn requests(fixture: &super::super::test_support::Fixture, count: usize) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if fixture.requests.lock().unwrap().len() >= count {
                break;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
}

/// Real provider/store composition shared with the configured telemetry scenario.
pub(crate) async fn ownership_scenario(
    database: &std::path::Path,
) -> anyhow::Result<(LookupId, QsoId)> {
    use super::super::{
        providers::{self, qrz::QrzSession},
        test_support::{fixture, material, response},
    };
    use clacks_client_store::{save, store::StoreSettings};
    use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};
    let store = ClientStore::create_with_definitions(
        database,
        Default::default(),
        &clacks_model::defaults::account_definitions()?,
        StoreSettings::default(),
    )
    .await?;
    let (release, released) = tokio::sync::oneshot::channel::<&'static str>();
    let delayed = axum::response::Response::builder().body(axum::body::Body::from_stream(
        futures_util::stream::once(async {
            Ok::<_, std::convert::Infallible>(axum::body::Bytes::from(released.await.unwrap()))
        }),
    ))?;
    let fixture = fixture(vec![
        response(
            200,
            "<QRZDatabase><Session><Key>fixture-key</Key></Session></QRZDatabase>",
        ),
        delayed,
        response(200, "<QRZDatabase><Session><Key>fixture-key</Key><Error>Not found: W7NEXT</Error></Session></QRZDatabase>"),
    ])
    .await?;
    let selected = Selected {
        material: material(),
        provider: clacks_model::directory::values::DirectoryProvider::Qrz,
        session: ProviderSession::Qrz(QrzSession::new(
            providers::client()?,
            fixture.endpoint.clone(),
            material(),
        )),
        paused: false,
        retry_after: None,
    };
    let (commands, input) = mpsc::channel(16);
    let (updates, mut output) = mpsc::channel(32);
    let (installed, installation) = watch::channel(None);
    let (_binding_updates, binding_view) = watch::channel(None);
    let (changes, changes_view) = watch::channel(());
    let stop = CancellationToken::new();
    let owner = QsoId::new();
    let runtime = Runtime {
        store: store.clone(),
        binding: binding_view,
        installed: installation,
        outputs: Outputs {
            updates,
            context: Default::default(),
        },
        changes,
        stop: stop.clone(),
    };
    let task = tokio::spawn(runtime.serve(input, Ok(Some(selected))));
    tokio::time::pause();
    commands
        .send(LookupCommand::CallChanged {
            qso: owner,
            call: "OK2CQR/P".into(),
        })
        .await?;
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    tokio::time::advance(Duration::from_millis(499)).await;
    assert!(fixture.requests.lock().unwrap().is_empty());
    commands
        .send(LookupCommand::CallChanged {
            qso: owner,
            call: "ok2cqr".into(),
        })
        .await?;
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    tokio::time::advance(Duration::from_millis(1)).await;
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    tokio::time::resume();
    let pending = output.recv().await.unwrap();
    assert_eq!(pending.qso, owner);
    assert!(matches!(pending.state, LookupState::Pending));
    requests(&fixture, 2).await;
    let mut form = crate::logging::form::ContactForm::empty();
    form.own.callsign = "W7HEX".into();
    form.dx.callsign = "OK2CQR".into();
    form.primary.band = "20m".into();
    form.mode = "SSB".into();
    form.start = crate::logging::time::ContactTimestamp::recorded(Some(
        clacks_model::scalar::UtcSecond::from_unix_seconds(1),
    ));
    let qso = form
        .build(owner, &Default::default())
        .map_err(|errors| anyhow::anyhow!("{errors:?}"))?;
    save::contacts::save(&store, None, qso, None).await?;
    commands.send(LookupCommand::Saved { qso: owner }).await?;
    commands.send(LookupCommand::Deleted { qso: owner }).await?;
    let next = QsoId::new();
    commands
        .send(LookupCommand::CallChanged {
            qso: next,
            call: "W7NEXT".into(),
        })
        .await?;
    requests(&fixture, 3).await;
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let update = output.recv().await.unwrap();
            if update.qso == next && matches!(update.state, LookupState::NotFound) {
                break;
            }
        }
    })
    .await?;
    assert!(
        !changes_view.has_changed()?,
        "no snapshot has committed before the delayed success"
    );
    release
        .send(include_str!("../providers/fixtures/qrz.xml"))
        .unwrap();
    let snapshot = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let update = output.recv().await.unwrap();
            if let LookupState::Found(snapshot) = update.state {
                assert_eq!(update.qso, owner);
                assert!(update.changed);
                break snapshot;
            }
        }
    })
    .await?;
    assert!(
        changes_view.has_changed()?,
        "a late committed snapshot must wake synchronization without another edit"
    );
    let row = save::load(
        &store,
        RecordKey {
            account: store.account_id(),
            id: RecordId::Directory(owner),
        },
    )
    .await?
    .unwrap();
    let Some(RecordState::Live(RecordValue::Directory(value))) = row.working_state() else {
        panic!("saved snapshot");
    };
    assert_eq!(value.snapshots().next().unwrap(), snapshot.as_ref());
    assert_eq!(
        snapshot
            .facts()
            .references
            .first()
            .unwrap()
            .identifier()
            .as_deref(),
        Some("NA-065")
    );
    assert!(snapshot.facts().supplied_coordinates.is_some());
    assert!(snapshot.facts().supplied_locator.is_some());
    let parent = save::load(
        &store,
        RecordKey {
            account: store.account_id(),
            id: RecordId::Qso(owner),
        },
    )
    .await?
    .unwrap();
    let Some(RecordState::Live(RecordValue::Qso(parent))) = parent.working_state() else {
        panic!("saved parent");
    };
    assert_eq!(parent.contacted_station().callsign.as_str(), "OK2CQR");
    assert!(
        save::load(
            &store,
            RecordKey {
                account: store.account_id(),
                id: RecordId::Directory(next)
            }
        )
        .await?
        .is_none()
    );
    assert_eq!(fixture.requests.lock().unwrap().len(), 3);
    stop.cancel();
    task.await?;
    drop(installed);
    store.close().await?;
    Ok((snapshot.lookup_id(), owner))
}

#[tokio::test]
async fn directory_lookup_late_response_is_saved_only_on_its_original_contact() -> anyhow::Result<()>
{
    let directory = tempfile::tempdir()?;
    ownership_scenario(&directory.path().join("store.sqlite")).await?;
    Ok(())
}

#[tokio::test]
async fn directory_lookup_honors_retry_after_without_queuing_automatic_retry() -> anyhow::Result<()>
{
    use super::super::{
        providers::{self, qrz::QrzSession},
        test_support::{fixture, material, response},
    };
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create(
        &directory.path().join("store.sqlite"),
        Default::default(),
        Default::default(),
    )
    .await?;
    let mut limited = response(429, "");
    limited.headers_mut().insert("retry-after", "600".parse()?);
    let fixture = fixture(vec![response(200, "<QRZDatabase><Session><Key>fixture-key</Key></Session></QRZDatabase>"), limited, response(200, "<QRZDatabase><Session><Key>fixture-key</Key><Error>Not found: unknown</Error></Session></QRZDatabase>")]).await?;
    let selected = Selected {
        material: material(),
        provider: clacks_model::directory::values::DirectoryProvider::Qrz,
        session: ProviderSession::Qrz(QrzSession::new(
            providers::client()?,
            fixture.endpoint.clone(),
            material(),
        )),
        paused: false,
        retry_after: None,
    };
    let (_binding, binding) = watch::channel(None);
    let (_installed, installed) = watch::channel(None);
    let (commands, input) = mpsc::channel(16);
    let (updates, mut output) = mpsc::channel(16);
    let stop = CancellationToken::new();
    let runtime = Runtime {
        store: store.clone(),
        binding,
        installed,
        outputs: Outputs {
            updates,
            context: Default::default(),
        },
        changes: watch::channel(()).0,
        stop: stop.clone(),
    };
    let task = tokio::spawn(runtime.serve(input, Ok(Some(selected))));
    let owner = QsoId::new();
    commands
        .send(LookupCommand::CallChanged {
            qso: owner,
            call: "W7FIRST".into(),
        })
        .await?;
    assert!(matches!(
        output.recv().await.unwrap().state,
        LookupState::Pending
    ));
    assert!(matches!(
        output.recv().await.unwrap().state,
        LookupState::Failed(_)
    ));
    assert_eq!(fixture.requests.lock().unwrap().len(), 2);
    tokio::time::pause();
    commands
        .send(LookupCommand::CallChanged {
            qso: owner,
            call: "W7NEXT".into(),
        })
        .await?;
    commands
        .send(LookupCommand::CallChanged {
            qso: QsoId::new(),
            call: String::new(),
        })
        .await?;
    assert!(matches!(
        output.recv().await.unwrap().state,
        LookupState::Cleared
    ));
    tokio::time::advance(Duration::from_millis(500)).await;
    let update = output.recv().await.unwrap();
    assert!(matches!(update.state, LookupState::Failed(_)));
    assert_eq!(fixture.requests.lock().unwrap().len(), 2);
    tokio::time::advance(Duration::from_secs(600)).await;
    assert_eq!(fixture.requests.lock().unwrap().len(), 2);
    commands.send(LookupCommand::Retry { qso: owner }).await?;
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    tokio::time::resume();
    assert!(matches!(
        output.recv().await.unwrap().state,
        LookupState::Pending
    ));
    assert!(matches!(
        output.recv().await.unwrap().state,
        LookupState::NotFound
    ));
    assert_eq!(fixture.requests.lock().unwrap().len(), 3);
    stop.cancel();
    task.await?;
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn directory_lookup_concurrent_limits_preserve_the_longest_quiet_period() -> anyhow::Result<()>
{
    use super::super::{
        providers::{self, qrz::QrzSession},
        test_support::{material, response},
    };
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    for second_delay in [None, Some("1")] {
        let directory = tempfile::tempdir()?;
        let store = ClientStore::create(
            &directory.path().join("store.sqlite"),
            Default::default(),
            Default::default(),
        )
        .await?;
        let requests = Arc::new(AtomicUsize::new(0));
        let release = Arc::new(tokio::sync::Semaphore::new(0));
        let router = axum::Router::new().fallback({
            let requests = requests.clone();
            let release = release.clone();
            move || {
                let requests = requests.clone();
                let release = release.clone();
                async move {
                    match requests.fetch_add(1, Ordering::SeqCst) {
                        0 => response(200, "<QRZDatabase><Session><Key>fixture-key</Key></Session></QRZDatabase>"),
                        1 => {
                            let mut limited = response(429, "");
                            limited.headers_mut().insert("retry-after", "600".parse().unwrap());
                            limited
                        }
                        2 => {
                            release.acquire().await.unwrap().forget();
                            let mut limited = response(429, "");
                            if let Some(delay) = second_delay {
                                limited.headers_mut().insert("retry-after", delay.parse().unwrap());
                            }
                            limited
                        }
                        _ => response(200, "<QRZDatabase><Session><Key>fixture-key</Key><Error>Not found: unknown</Error></Session></QRZDatabase>"),
                    }
                }
            }
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}/", listener.local_addr()?).parse()?;
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let selected = Selected {
            material: material(),
            provider: clacks_model::directory::values::DirectoryProvider::Qrz,
            session: ProviderSession::Qrz(QrzSession::new(
                providers::client()?,
                endpoint,
                material(),
            )),
            paused: false,
            retry_after: None,
        };
        let (_binding, binding) = watch::channel(None);
        let (_installed, installed) = watch::channel(None);
        let (commands, input) = mpsc::channel(16);
        let (updates, mut output) = mpsc::channel(16);
        let stop = CancellationToken::new();
        let runtime = Runtime {
            store: store.clone(),
            binding,
            installed,
            outputs: Outputs {
                updates,
                context: Default::default(),
            },
            changes: watch::channel(()).0,
            stop: stop.clone(),
        };
        let task = tokio::spawn(runtime.serve(input, Ok(Some(selected))));
        let owner = QsoId::new();

        tokio::time::pause();
        for qso in [owner, QsoId::new()] {
            commands
                .send(LookupCommand::CallChanged {
                    qso,
                    call: "W7TEST".into(),
                })
                .await?;
        }
        // An empty observation acknowledges that both requests have their debounce deadline.
        commands
            .send(LookupCommand::CallChanged {
                qso: QsoId::new(),
                call: String::new(),
            })
            .await?;
        assert!(matches!(
            output.recv().await.unwrap().state,
            LookupState::Cleared
        ));
        tokio::time::advance(Duration::from_millis(500)).await;
        for _ in 0..2 {
            assert!(matches!(
                output.recv().await.unwrap().state,
                LookupState::Pending
            ));
        }
        tokio::time::resume();
        assert!(matches!(
            output.recv().await.unwrap().state,
            LookupState::Failed(_)
        ));
        release.add_permits(1);
        assert!(matches!(
            output.recv().await.unwrap().state,
            LookupState::Failed(_)
        ));
        assert_eq!(requests.load(Ordering::SeqCst), 3);

        tokio::time::pause();
        tokio::time::advance(Duration::from_secs(2)).await;
        commands.send(LookupCommand::Retry { qso: owner }).await?;
        assert!(
            matches!(output.recv().await.unwrap().state, LookupState::Failed(_)),
            "a later {second_delay:?} limit must preserve the earlier 600-second gate"
        );
        assert_eq!(requests.load(Ordering::SeqCst), 3);
        tokio::time::advance(Duration::from_secs(600)).await;
        assert_eq!(requests.load(Ordering::SeqCst), 3, "no automatic retry");
        commands.send(LookupCommand::Retry { qso: owner }).await?;
        assert!(matches!(
            output.recv().await.unwrap().state,
            LookupState::Pending
        ));
        tokio::time::resume();
        assert!(matches!(
            output.recv().await.unwrap().state,
            LookupState::NotFound
        ));
        assert_eq!(requests.load(Ordering::SeqCst), 4);
        stop.cancel();
        task.await?;
        server.abort();
        store.close().await?;
    }
    Ok(())
}

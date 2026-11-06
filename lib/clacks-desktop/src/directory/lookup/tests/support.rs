//! Concrete local provider and account fixtures shared by actual consumers.
use super::*;
/// Provider and local-store fixture for one temporary account workspace.
pub(crate) struct Service {
    pub(crate) client: LookupClient,
    pub(crate) store: ClientStore,
    stop: CancellationToken,
    task: tokio::task::JoinHandle<anyhow::Result<()>>,
    _directory: tempfile::TempDir,
    selected: Selected,
}

impl Service {
    pub(crate) async fn new(endpoint: url::Url) -> anyhow::Result<Self> {
        let directory = tempfile::tempdir()?;
        let store = ClientStore::create(
            &directory.path().join("store.sqlite"),
            Default::default(),
            Default::default(),
        )
        .await?;
        Self::from_store(endpoint, store, directory)
    }

    pub(crate) fn from_store(
        endpoint: url::Url,
        store: ClientStore,
        directory: tempfile::TempDir,
    ) -> anyhow::Result<Self> {
        let selected = Selected {
            origin: Arc::new(()),
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
        let (client, runtime) = LookupRuntime::with_selection(
            store.clone(),
            watch::channel(None).1,
            watch::channel(None).1,
            Default::default(),
            Ok(Some(selected.clone())),
        );
        let stop = CancellationToken::new();
        let task = tokio::spawn(runtime.run(stop.clone()));
        Ok(Self {
            client,
            store,
            stop,
            task,
            _directory: directory,
            selected,
        })
    }

    /// Fixture configuration input to the real client and its actual consumer notification.
    pub(crate) fn available(&self, available: bool) {
        self.client.inner.state.lock().unwrap().selected =
            Ok(available.then(|| self.selected.clone()));
        self.client.inner.configuration.send_replace(());
    }

    /// Install replacement dispatch material through the real client's configuration input.
    pub(crate) fn replace(&self, endpoint: url::Url) -> anyhow::Result<()> {
        configure_client(&self.client, endpoint)
    }

    pub(crate) async fn settled(&self) {
        self.client.inner.tasks.close();
        self.client.inner.tasks.wait().await;
    }

    pub(crate) async fn close(self) -> anyhow::Result<()> {
        self.stop.cancel();
        self.task.await??;
        self.store.close().await?;
        Ok(())
    }
}

/// Select the concrete fixture provider for an existing real workspace client.
pub(crate) fn configure_client(client: &LookupClient, endpoint: url::Url) -> anyhow::Result<()> {
    let replacement = material();
    client.inner.state.lock().unwrap().selected = Ok(Some(Selected {
        origin: Arc::new(()),
        material: replacement.clone(),
        provider: clacks_model::directory::values::DirectoryProvider::Qrz,
        session: ProviderSession::Qrz(QrzSession::new(providers::client()?, endpoint, replacement)),
        paused: false,
        retry_after: None,
    }));
    client.inner.configuration.send_replace(());
    Ok(())
}

pub(crate) fn key(call: &str) -> LookupKey {
    LookupKey::from_recorded(&RecordedCallsign::new(call).unwrap())
}
pub(crate) fn login() -> axum::response::Response {
    response(
        200,
        "<QRZDatabase><Session><Key>fixture-key</Key></Session></QRZDatabase>",
    )
}
pub(crate) fn found() -> axum::response::Response {
    response(200, include_str!("../../providers/fixtures/qrz.xml"))
}
pub(crate) fn lookup_count(fixture: &test_support::Fixture) -> usize {
    fixture
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|request| request.method == "GET")
        .count()
}
pub(crate) fn snapshot(outcome: &LookupState) -> &DirectorySnapshot {
    match outcome {
        LookupState::Found(snapshot) => snapshot,
        other => panic!("expected found, got {other:?}"),
    }
}
pub(crate) async fn requests(fixture: &test_support::Fixture, count: usize) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while fixture.requests.lock().unwrap().len() < count {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
}
pub(crate) fn delayed() -> (axum::response::Response, oneshot::Sender<&'static str>) {
    let (release, released) = oneshot::channel();
    let response = axum::response::Response::builder()
        .body(axum::body::Body::from_stream(futures_util::stream::once(
            async {
                Ok::<_, std::convert::Infallible>(axum::body::Bytes::from(released.await.unwrap()))
            },
        )))
        .unwrap();
    (response, release)
}

/// A real submitted contact for snapshot and early-save scenarios.
pub(crate) fn contact(id: QsoId, call: &str) -> clacks_model::contact::Qso {
    let mut form = crate::logging::form::ContactForm::empty();
    form.own.callsign = "W7HEX".into();
    form.dx.callsign = call.into();
    form.primary.band = "20m".into();
    form.mode = "SSB".into();
    form.start = crate::logging::time::ContactTimestamp::recorded(Some(
        clacks_model::scalar::UtcSecond::from_unix_seconds(1),
    ));
    form.build(id, &Default::default()).unwrap()
}

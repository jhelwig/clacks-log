//! Authored local HTTP fixture shared by actual adapter and lookup-runtime scenarios.

use axum::{
    body::Body,
    extract::{Request, State},
    response::Response,
};
use clacks_model::{
    directory::values::LookupKey, ids::DirectoryMaterialId, scalar::RecordedCallsign,
};
use clacks_sync::protocol::directory::{DirectoryLogin, DirectoryMaterial};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

/// Recorded wire input excludes any production service or account.
#[derive(Clone)]
pub(super) struct Captured {
    /// Actual encoded request URI.
    pub(super) uri: String,
    /// Method chosen by the adapter.
    pub(super) method: String,
    /// Authorization supplied to the fixture.
    pub(super) authorization: Option<String>,
    /// Actual form or JSON bytes.
    pub(super) body: String,
}

/// Ordered authored responses and captured real HTTP requests.
pub(crate) struct Fixture {
    /// Local endpoint used by the ordinary adapter constructor.
    pub(crate) endpoint: url::Url,
    /// Captured inputs.
    pub(super) requests: Arc<Mutex<Vec<Captured>>>,
    /// Fixture listener owner.
    server: tokio::task::JoinHandle<()>,
}

/// Server fixture state has no provider behavior beyond its finite response sequence.
#[derive(Clone)]
struct FixtureState {
    /// Recorded actual inputs.
    pub(super) requests: Arc<Mutex<Vec<Captured>>>,
    /// Authored ordered outputs.
    responses: Arc<Mutex<VecDeque<Response>>>,
}

impl Fixture {
    /// Count actual QRZ lookup HTTP requests, excluding authentication.
    pub(crate) fn lookup_count(&self) -> usize {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request.method == "GET")
            .count()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

/// Serve finite responses; an unexpected request fails rather than authenticating by fiction.
pub(crate) async fn fixture(responses: Vec<Response>) -> anyhow::Result<Fixture> {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let queue = Arc::new(Mutex::new(VecDeque::from(responses)));
    let state = FixtureState {
        requests: requests.clone(),
        responses: queue,
    };
    let router = axum::Router::new()
        .fallback(
            async |State(state): State<FixtureState>, request: Request| {
                let uri = request.uri().to_string();
                let method = request.method().to_string();
                let authorization = request
                    .headers()
                    .get("authorization")
                    .map(|value| value.to_str().unwrap().to_owned());
                let bytes = axum::body::to_bytes(request.into_body(), 4096)
                    .await
                    .unwrap();
                state.requests.lock().unwrap().push(Captured {
                    uri,
                    method,
                    authorization,
                    body: String::from_utf8(bytes.to_vec()).unwrap(),
                });
                state
                    .responses
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("unexpected provider request")
            },
        )
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}/", listener.local_addr()?).parse()?;
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    Ok(Fixture {
        endpoint,
        requests,
        server,
    })
}

/// Author a response without fixture-owned retry logic.
pub(crate) fn response(status: u16, body: &'static str) -> Response {
    Response::builder()
        .status(status)
        .body(Body::from(body))
        .unwrap()
}

/// Synthetic login whose exact bytes must be form/JSON encoded only in the POST body.
pub(crate) fn material() -> DirectoryMaterial {
    DirectoryMaterial {
        id: DirectoryMaterialId::new(),
        revision: 1,
        login: DirectoryLogin {
            username: "fixture-user".into(),
            password: "secret&+ sentinel".into(),
        },
    }
}

/// Normalize using the same recorded-call boundary as production.
pub(crate) fn key() -> LookupKey {
    LookupKey::from_recorded(&RecordedCallsign::new("EA8/W7TEST").unwrap())
}

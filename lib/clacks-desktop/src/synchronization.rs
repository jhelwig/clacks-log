//! Session-owned authenticated synchronization; offline local work remains independently available.

/// Metric name for native upgrade request through response headers or failure.
pub(crate) const CLIENT_REQUEST_DURATION_METRIC: &str = "http.client.request.duration";

/// Metric name for completed or failed bounded synchronization phase.
pub(crate) const PHASE_DURATION_METRIC: &str = "clacks_desktop_sync_phase_duration_seconds";

mod exchange;
mod pending;

#[cfg(test)]
mod pause_tests;

#[cfg(all(test, any(target_os = "macos", windows)))]
pub(crate) mod tests;

use clacks_client_store::{
    identity::EnrollmentBinding, settings::SettingsSnapshot, store::ClientStore,
};
use clacks_sync::protocol::messages::{ClientMessage, FailureKind, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use huskarl::authorizer::HttpAuthorizer;
use opentelemetry::trace::TraceContextExt;
use std::sync::Arc;
use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock, watch};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream,
    tungstenite::{self, client::IntoClientRequest},
};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Admission to sync exchanges for one workspace. Multiple callers can hold a
/// pause concurrently. An exchange has exclusive access until it settles.
#[derive(Clone, Debug, Default)]
pub(crate) struct ExchangeGate(Arc<RwLock<()>>);

impl ExchangeGate {
    /// Wait for any active exchange to finish and return a shared pause guard.
    /// A cancelled wait returns an error without taking a hold.
    pub(crate) async fn pause(
        &self,
        stop: &CancellationToken,
    ) -> anyhow::Result<OwnedRwLockReadGuard<()>> {
        tokio::select! {
            biased;
            _ = stop.cancelled() => anyhow::bail!("Import cancelled"),
            guard = async {
                match self.0.clone().try_read_owned() {
                    Ok(guard) => guard,
                    Err(_) => {
                        tracing::info!(event = "desktop.import.waiting_for_sync_pause");
                        self.0.clone().read_owned().await
                    }
                }
            } => Ok(guard),
        }
    }

    async fn exchange(
        &self,
        stop: &CancellationToken,
    ) -> anyhow::Result<OwnedRwLockWriteGuard<()>> {
        tokio::select! {
            biased;
            _ = stop.cancelled() => anyhow::bail!("Synchronization stopped"),
            guard = async {
                match self.0.clone().try_write_owned() {
                    Ok(guard) => guard,
                    Err(_) => {
                        tracing::info!(event = "sync.waiting_for_workspace_hold");
                        self.0.clone().write_owned().await
                    }
                }
            } => Ok(guard),
        }
    }
}

/// Workspace shutdown token and exchange gate reused across reconnects.
#[derive(Clone, Default)]
pub(crate) struct Control {
    pub stop: CancellationToken,
    pub gate: ExchangeGate,
}

/// Nonsecret connection/error presentation; no aggregate completion timestamp is implied.
#[derive(Clone, Debug, Default)]
pub(crate) enum Status {
    /// No native connection is active.
    #[default]
    Disconnected,
    /// Connecting or refreshing through the existing enrolled credential.
    Connecting,
    /// The native connection is established; individual records may still require review.
    Connected,
    /// Current actionable failure, without any loss of local work.
    Failed(String),
}

/// A server-reported failure retained as a typed cause for connection diagnostics.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
struct ServerFailure {
    /// Operational category supplied by the server.
    kind: FailureKind,
    /// Safe diagnostic returned to the desktop user.
    message: String,
}

/// Concrete Rustls client socket; authentication remains in its protected upgrade headers.
type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// Current physical connection ownership and its nonblocking directory delivery sink.
pub(super) struct ConnectionContext {
    /// Cancellation also invalidates native deliveries from this connection.
    pub stop: CancellationToken,
    pub gate: ExchangeGate,
    /// Existing connection trace linked by each synchronization operation.
    pub trace: opentelemetry::trace::SpanContext,
    /// Purpose-specific responses leave the socket loop through this channel.
    pub directory: watch::Sender<Option<crate::directory::provisioning::Delivery>>,
}

/// Run until session cancellation, reconnecting through the library-owned refresh credential.
pub(crate) async fn run(
    store: ClientStore,
    binding: EnrollmentBinding,
    mut changes: watch::Receiver<()>,
    status: watch::Sender<Status>,
    updates: watch::Sender<Option<Arc<SettingsSnapshot>>>,
    control: Control,
    directory_outputs: crate::directory::provisioning::Outputs,
) {
    let Control { stop, gate } = control;
    let (directory_deliveries, incoming) = watch::channel(None);
    let provisioning_stop = stop.child_token();
    let provisioning = tokio::spawn(crate::directory::provisioning::run(
        store.clone(),
        binding.clone(),
        incoming,
        directory_outputs,
        provisioning_stop.clone(),
    ));
    let mut delay = 1_u64;

    let mut authorizer = None;
    loop {
        if stop.is_cancelled() {
            break;
        }
        status.send_replace(Status::Connecting);
        let connection_stop = stop.child_token();
        let result = async {
            if authorizer.is_none() {
                authorizer = Some(crate::enrollment::flow::protected(&binding).await?);
            }
            let authorizer = authorizer
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("protected authorization unavailable"))?;
            let span = tracing::info_span!(parent: None, "desktop.sync.connect", otel.kind="client", otel.name="GET", network.protocol.version="1.1", http.request.method="GET", account_id=%binding.account, actor_id=%binding.actor);
            span.in_scope(|| tracing::info!(event="sync.connecting", account_id=%binding.account, actor_id=%binding.actor));
            let mut socket = tokio::select! {

                _ = stop.cancelled() => anyhow::bail!("Synchronization stopped"),
                result = connect(&binding, authorizer, &span).instrument(span.clone()) => match result { Ok(socket) => socket, Err(error) => { span.set_status(opentelemetry::trace::Status::error("")); span.set_attribute("error.type", crate::telemetry::failure::error_class(&error)); return Err(error); } },
            };
            let context = span.context().span().span_context().clone();
            drop(span);
            tracing::info!(event="sync.connected", account_id=%binding.account, actor_id=%binding.actor);
            status.send_replace(Status::Connected);
            delay = 1;
            exchange::run(&mut socket, &store, &binding, &mut changes, &updates, &ConnectionContext { stop: connection_stop.clone(), gate: gate.clone(), trace: context, directory: directory_deliveries.clone() }).await
        }
        .await;
        connection_stop.cancel();
        if stop.is_cancelled() {
            break;
        }
        if let Err(error) = result {
            let failure_kind = error
                .downcast_ref::<ServerFailure>()
                .map(|failure| failure.kind);
            // Detailed errors are for the local operator; never serialize native grants or frames.
            status.send_replace(Status::Failed(error.to_string()));
            tracing::warn!(event="sync.connection.interrupted", account_id=%binding.account, actor_id=%binding.actor, ?failure_kind, error.type=crate::telemetry::failure::error_class(&error), "Account synchronization will reconnect");
            metrics::counter!("clacks_desktop_sync_reconnects_total").increment(1);
        }
        tokio::select! {_ = stop.cancelled() => break, _ = tokio::time::sleep(std::time::Duration::from_secs(delay)) => {}}
        delay = delay.saturating_mul(2).min(30);
    }
    provisioning_stop.cancel();
    drop(directory_deliveries);
    if let Err(error) = provisioning.await {
        tracing::warn!(event="directory.provisioning.worker_failed", error.type=if error.is_panic() { "task_panic" } else { "task_cancelled" });
    }
    status.send_replace(Status::Disconnected);
}

/// Use the same validated deployment origin for the HTTPS proof and WSS transport target.
async fn connect(
    binding: &EnrollmentBinding,
    authorizer: &HttpAuthorizer,
    span: &tracing::Span,
) -> anyhow::Result<Socket> {
    let target = crate::enrollment::flow::origin(&binding.server)?.join("api/sync")?;
    span.set_attribute("url.full", target.to_string());
    span.set_attribute("url.scheme", target.scheme().to_owned());
    span.set_attribute(
        "server.address",
        target.host_str().unwrap_or_default().to_owned(),
    );
    let proof_target: http::Uri = target.as_str().parse()?;
    let headers = authorizer
        .get_headers(&http::Method::GET, &proof_target)
        .await?;
    let mut endpoint = target;
    let scheme = if endpoint.scheme() == "https" {
        "wss"
    } else {
        "ws"
    };
    endpoint
        .set_scheme(scheme)
        .map_err(|_| anyhow::anyhow!("invalid native transport scheme"))?;
    let mut request = endpoint.as_str().into_client_request()?;
    request.headers_mut().extend(headers);
    opentelemetry::global::get_text_map_propagator(|propagator| {
        propagator.inject_context(
            &span.context(),
            &mut opentelemetry_http::HeaderInjector(request.headers_mut()),
        )
    });
    let started = std::time::Instant::now();
    let result = tokio_tungstenite::connect_async(request).await;
    let mut labels = vec![
        metrics::Label::new("http.request.method", "GET"),
        metrics::Label::new(
            "url.scheme",
            if binding.server.starts_with("https:") {
                "https"
            } else {
                "http"
            },
        ),
    ];
    let status = match &result {
        Ok((_, response)) => Some(response.status()),
        Err(tungstenite::Error::Http(response)) => Some(response.status()),
        _ => None,
    };
    if let Some(status) = status {
        labels.push(metrics::Label::new(
            "http.response.status_code",
            status.as_u16().to_string(),
        ));
    }
    if let Err(error) = &result {
        labels.push(metrics::Label::new(
            "error.type",
            crate::telemetry::failure::socket_error_class(error),
        ));
    }
    metrics::histogram!(CLIENT_REQUEST_DURATION_METRIC, labels)
        .record(started.elapsed().as_secs_f64());
    match result {
        Ok((socket, response)) => {
            span.set_attribute(
                "http.response.status_code",
                i64::from(response.status().as_u16()),
            );
            authorizer.process_response(&proof_target, response.headers());
            Ok(socket)
        }
        Err(tungstenite::Error::Http(response)) => {
            authorizer.process_response(&proof_target, response.headers());
            if response.status() == http::StatusCode::UNAUTHORIZED {
                authorizer.invalidate();
            }
            span.set_attribute(
                "http.response.status_code",
                i64::from(response.status().as_u16()),
            );
            Err(tungstenite::Error::Http(response).into())
        }
        Err(error) => Err(error.into()),
    }
}

/// Send one native message unless its owning session stops.
async fn send(
    socket: &mut Socket,
    stop: &CancellationToken,
    message: ClientMessage,
) -> anyhow::Result<()> {
    let frame = tungstenite::Message::Text(serde_json::to_string(&message)?.into());
    tokio::select! {
        biased;
        _ = stop.cancelled() => anyhow::bail!("Synchronization stopped"),
        result = socket.send(frame) => { result?; Ok(()) },
    }
}

/// Receive one native response while cancellation interrupts only the network wait.
async fn receive(
    socket: &mut Socket,
    connection: &ConnectionContext,
) -> anyhow::Result<ServerMessage> {
    let stop = &connection.stop;
    loop {
        let frame = tokio::select! {_ = stop.cancelled() => anyhow::bail!("Synchronization stopped"), frame = socket.next() => frame};
        match frame {
            Some(Ok(tungstenite::Message::Text(text))) => {
                let message: ServerMessage = serde_json::from_str(&text)?;
                if let ServerMessage::DirectoryConfiguration { configuration } = message {
                    connection.directory.send_replace(Some(
                        crate::directory::provisioning::Delivery {
                            configuration,
                            connection_stop: stop.clone(),
                        },
                    ));
                    continue;
                }
                return Ok(message);
            }
            Some(Ok(tungstenite::Message::Ping(_) | tungstenite::Message::Pong(_))) => {}
            Some(Err(error)) => return Err(error.into()),
            _ => return Err(tungstenite::Error::ConnectionClosed.into()),
        }
    }
}

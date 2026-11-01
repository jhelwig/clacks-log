//! Session-owned authenticated synchronization; offline local work remains independently available.

mod exchange;
mod pending;

#[cfg(all(test, any(target_os = "macos", windows)))]
pub(crate) mod tests;

use clacks_client_store::{
    identity::EnrollmentBinding, settings::SettingsSnapshot, store::ClientStore,
};
use clacks_sync::protocol::messages::{ClientMessage, FailureKind, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use huskarl::authorizer::HttpAuthorizer;
use std::sync::Arc;
use tokio::sync::watch;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream,
    tungstenite::{self, client::IntoClientRequest},
};
use tokio_util::sync::CancellationToken;

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

/// Run until session cancellation, reconnecting through the library-owned refresh credential.
pub(crate) async fn run(
    store: ClientStore,
    binding: EnrollmentBinding,
    mut changes: watch::Receiver<()>,
    status: watch::Sender<Status>,
    updates: watch::Sender<Option<Arc<SettingsSnapshot>>>,
    stop: CancellationToken,
) {
    let mut delay = 1_u64;
    let mut authorizer = None;
    loop {
        if stop.is_cancelled() {
            break;
        }
        status.send_replace(Status::Connecting);
        let result = async {
            if authorizer.is_none() {
                authorizer = Some(crate::enrollment::flow::protected(&binding).await?);
            }
            let authorizer = authorizer
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("protected authorization unavailable"))?;
            let mut socket = tokio::select! {
                _ = stop.cancelled() => anyhow::bail!("Synchronization stopped"),
                result = connect(&binding, authorizer) => result?,
            };
            status.send_replace(Status::Connected);
            delay = 1;
            exchange::run(&mut socket, &store, &binding, &mut changes, &updates, &stop).await
        }
        .await;
        if stop.is_cancelled() {
            break;
        }
        if let Err(error) = result {
            let failure_kind = error
                .downcast_ref::<ServerFailure>()
                .map(|failure| failure.kind);
            // Detailed errors are for the local operator; never serialize native grants or frames.
            status.send_replace(Status::Failed(error.to_string()));
            tracing::warn!(event="sync.connection.interrupted", account_id=%binding.account, actor_id=%binding.actor, ?failure_kind, "Account synchronization will reconnect");
            metrics::counter!("clacks_desktop_sync_reconnects_total").increment(1);
        }
        tokio::select! {_ = stop.cancelled() => break, _ = tokio::time::sleep(std::time::Duration::from_secs(delay)) => {}}
        delay = delay.saturating_mul(2).min(30);
    }
    status.send_replace(Status::Disconnected);
}

/// Use the same validated deployment origin for the HTTPS proof and WSS transport target.
async fn connect(
    binding: &EnrollmentBinding,
    authorizer: &HttpAuthorizer,
) -> anyhow::Result<Socket> {
    let target = crate::enrollment::flow::origin(&binding.server)?.join("api/sync")?;
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
    match tokio_tungstenite::connect_async(request).await {
        Ok((socket, response)) => {
            authorizer.process_response(&proof_target, response.headers());
            Ok(socket)
        }
        Err(tungstenite::Error::Http(response)) => {
            authorizer.process_response(&proof_target, response.headers());
            if response.status() == http::StatusCode::UNAUTHORIZED {
                authorizer.invalidate();
            }
            anyhow::bail!("Server refused synchronization: {}", response.status())
        }
        Err(_) => anyhow::bail!("Could not connect to the synchronization server"),
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
async fn receive(socket: &mut Socket, stop: &CancellationToken) -> anyhow::Result<ServerMessage> {
    loop {
        let frame = tokio::select! {_ = stop.cancelled() => anyhow::bail!("Synchronization stopped"), frame = socket.next() => frame};
        match frame {
            Some(Ok(tungstenite::Message::Text(text))) => {
                return serde_json::from_str(&text)
                    .map_err(|_| anyhow::anyhow!("Invalid server synchronization message"));
            }
            Some(Ok(tungstenite::Message::Ping(_) | tungstenite::Message::Pong(_))) => {}
            _ => anyhow::bail!("Synchronization connection closed"),
        }
    }
}

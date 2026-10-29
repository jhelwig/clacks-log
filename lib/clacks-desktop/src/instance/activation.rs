//! Bounded activation messages and explicitly joined local listener lifetime.

use super::ActivationAddress;
use interprocess::local_socket::{
    tokio::{Listener, Stream},
    traits::tokio::{Listener as _, Stream as _},
};
use std::{io, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::watch,
    task::{JoinHandle, JoinSet},
};
use tokio_util::sync::CancellationToken;

/// Versioned fixed-size activation request, containing no user data.
const REQUEST: &[u8; 8] = b"CLKACT01";
/// Acknowledgment of receipt, without promising foreground focus.
const ACK: &[u8; 8] = b"CLKACK01";
/// Deadline for one peer exchange, including partial/malformed input.
const CONNECTION_TIMEOUT: Duration = Duration::from_millis(250);
/// Maximum secondary-launch wait for a starting owner.
const ACTIVATION_TIMEOUT: Duration = Duration::from_secs(3);

/// Running activation work, stopped and joined before releasing workspace ownership.
pub(crate) struct ActivationListener {
    /// Explicit listener-stop signal, independent of background save cancellation.
    stop: CancellationToken,
    /// Background accept/handler lifetime observed during shutdown.
    task: JoinHandle<anyhow::Result<()>>,
}

impl ActivationListener {
    /// Stop accepting, finish bounded exchanges and release the endpoint name.
    pub async fn shutdown(self) -> anyhow::Result<()> {
        self.stop.cancel();
        self.task.await??;
        Ok(())
    }
}

/// Track a bound listener and its bounded activation handlers on the owner runtime.
pub(super) fn listen(
    listener: Listener,
    runtime: &tokio::runtime::Handle,
    notices: watch::Sender<()>,
) -> ActivationListener {
    let stop = CancellationToken::new();
    let task = runtime.spawn(serve(listener, stop.clone(), notices));
    ActivationListener { stop, task }
}

/// Receive one versioned request and acknowledge after notifying the UI path.
async fn receive(mut stream: Stream, notices: watch::Sender<()>) -> io::Result<()> {
    let mut request = [0_u8; 8];
    stream.read_exact(&mut request).await?;
    if &request != REQUEST {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported activation message",
        ));
    }

    notices.send_replace(());
    stream.write_all(ACK).await?;
    Ok(())
}

/// Own the accept loop and a bounded set of peer exchanges.
async fn serve(
    listener: Listener,
    stop: CancellationToken,
    notices: watch::Sender<()>,
) -> anyhow::Result<()> {
    let mut handlers = JoinSet::new();

    loop {
        tokio::select! {
            _ = stop.cancelled() => break,
            connection = listener.accept(), if handlers.len() < 4 => {
                let connection = connection?;
                let notices = notices.clone();
                handlers.spawn(async move {
                    match tokio::time::timeout(CONNECTION_TIMEOUT, receive(connection, notices)).await {
                        Ok(Ok(())) => {},
                        Ok(Err(error)) => tracing::debug!(event = "desktop.activation_invalid", error_class = ?error.kind()),
                        Err(_) => tracing::debug!(event = "desktop.activation_timed_out"),
                    }
                });
            },
            result = handlers.join_next(), if !handlers.is_empty() => {
                if let Some(result) = result {
                    result?;
                }
            }
        }
    }

    while let Some(result) = handlers.join_next().await {
        result?;
    }

    drop(listener);
    Ok(())
}

/// Request activation from a launching process without starting a workspace session.
pub(crate) fn request(address: &ActivationAddress) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    runtime.block_on(request_async(address))
}

/// Activate an existing workspace from an async task, allowing three seconds for startup.
pub(crate) async fn request_async(address: &ActivationAddress) -> anyhow::Result<()> {
    tokio::time::timeout(ACTIVATION_TIMEOUT, async {
        loop {
            let attempt = async {
                let mut stream = Stream::connect(address.name.clone()).await?;
                stream.write_all(REQUEST).await?;
                let mut acknowledgment = [0_u8; 8];
                stream.read_exact(&mut acknowledgment).await?;
                if &acknowledgment != ACK {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "unsupported activation acknowledgment",
                    ));
                }
                Ok::<_, io::Error>(())
            };

            match tokio::time::timeout(CONNECTION_TIMEOUT, attempt).await {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(error))
                    if matches!(
                        error.kind(),
                        io::ErrorKind::PermissionDenied | io::ErrorKind::InvalidData
                    ) =>
                {
                    return Err(error.into());
                }
                _ => tokio::time::sleep(Duration::from_millis(25)).await,
            }
        }
    })
    .await
    .map_err(|_| {
        anyhow::anyhow!("existing workspace owner could not be activated within three seconds")
    })?
}

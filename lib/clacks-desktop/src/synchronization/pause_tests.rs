use super::ExchangeGate;
use tokio_util::sync::CancellationToken;
use tracing::instrument::WithSubscriber;

#[tokio::test]
async fn shutdown_releases_a_worker_waiting_for_an_import() -> anyhow::Result<()> {
    let gate = ExchangeGate::default();
    let stop = CancellationToken::new();
    let import = gate.pause(&stop).await?;
    let output = tempfile::NamedTempFile::new()?;
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::sync::Mutex::new(output.reopen()?))
        .finish();
    let exchange = gate.exchange(&stop).with_subscriber(subscriber);
    tokio::pin!(exchange);
    assert!(futures_util::poll!(&mut exchange).is_pending());
    let logged = std::fs::read_to_string(output.path())?;
    assert!(logged.contains("sync.waiting_for_workspace_hold"));

    stop.cancel();
    assert!(matches!(
        futures_util::poll!(&mut exchange),
        std::task::Poll::Ready(Err(_))
    ));
    drop(import);
    Ok(())
}

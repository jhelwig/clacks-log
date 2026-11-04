//! Handling-boundary feedback survives the ordinary latest-value view publication.
use super::super::{Outputs, ReceiverView, ReceptionState, runtime};
use clacks_client_store::{
    store::ClientStore,
    ws_suite::{self, ReceiverConnection, ReceiverSettings},
};
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn capture_rejection_survives_next_success() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let store = ClientStore::create(
        &dir.path().join("workspace.sqlite"),
        Default::default(),
        Default::default(),
    )
    .await?;
    let probe = std::net::UdpSocket::bind("127.0.0.1:0")?;
    let address = probe.local_addr()?;
    drop(probe);
    ws_suite::save(
        &store,
        &ReceiverSettings {
            enabled: true,
            connection: ReceiverConnection::Unicast {
                address: address.ip(),
                port: address.port(),
            },
        },
    )
    .await?;
    let (updates, mut view) = watch::channel(ReceiverView::default());
    let (changes, mut changed) = watch::channel(());
    let (commands, input) = mpsc::channel(8);
    let (calls, mut entry_calls) = mpsc::unbounded_channel();
    let stop = CancellationToken::new();
    let (lookup, lookup_runtime) = crate::directory::lookup::runtime::LookupRuntime::new(
        store.clone(),
        watch::channel(None).1,
        watch::channel(None).1,
        Default::default(),
    )
    .await;
    let lookup_task = tokio::spawn(lookup_runtime.run(stop.clone()));
    let task = tokio::spawn(runtime::run(
        store.clone(),
        lookup,
        input,
        Outputs {
            calls,
            updates,
            context: Default::default(),
        },
        watch::channel(None).0,
        changes,
        Default::default(),
        stop.clone(),
    ));
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if view.borrow().reception == ReceptionState::Listening {
                break;
            }
            view.changed().await?;
        }
        anyhow::Ok(())
    })
    .await??;
    let sender = tokio::net::UdpSocket::bind("127.0.0.1:0").await?;
    for call in ["W7HEX", "W7HEX", ""] {
        sender
            .send_to(&crate::test_support::ws_suite::status(call), address)
            .await?;
        let received = tokio::time::timeout(Duration::from_secs(5), entry_calls.recv())
            .await?
            .unwrap();
        assert_eq!(
            received.1.call, call,
            "each packet retains arrival order, including repeats"
        );
    }
    assert!(!changed.has_changed()?);
    // A real, identifiable completion without required own identity is rejected.
    let rejected = super::protocol::adif(
        b"<CALL:5>W7BAD<QSO_DATE:8>19700101<TIME_ON:6>000000<FREQ:6>14.074<MODE:3>FT8<EOR>",
    );
    sender.send_to(&rejected, address).await?;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if view.borrow().capture_notice.is_some() {
                break;
            }
            view.changed().await?;
        }
        anyhow::Ok(())
    })
    .await??;
    sender
        .send_to(&crate::test_support::ws_suite::adif("W7HEX"), address)
        .await?;
    tokio::time::timeout(Duration::from_secs(5), changed.changed()).await??;
    stop.cancel();
    task.await??;
    assert!(
        view.borrow().capture_notice.is_some(),
        "a different saved contact must not erase capture failure feedback"
    );
    drop(commands);
    lookup_task.await??;
    store.close().await?;
    Ok(())
}

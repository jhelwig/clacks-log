use super::{ConnectionContext, Status, run, synchronization};
use clacks_client_store::{identity, identity::EnrollmentBinding, store::ClientStore};
use clacks_model::ids::{AccountId, BaselineId, ClientActorId, TimelineId};
use clacks_sync::{
    account::{AccountMetadata, InstalledCheckpoint},
    protocol::{
        ClientIdentity,
        messages::{self, ClientMessage, ServerMessage, TransferMessage},
    },
    transfer::{
        WindowMode,
        discovery::{DiscoveryEnd, DiscoveryPage, DiscoveryStart},
    },
};
use futures_util::{SinkExt, StreamExt};
use opentelemetry::trace::SpanContext;
use std::time::Duration;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::watch,
};
use tokio_tungstenite::{WebSocketStream, accept_async, connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;

/// Read the next application message from the worker under test.
async fn from_worker(peer: &mut WebSocketStream<TcpStream>) -> anyhow::Result<ClientMessage> {
    let Some(Ok(Message::Text(text))) = peer.next().await else {
        anyhow::bail!("Expected a synchronization message from the worker");
    };
    Ok(serde_json::from_str(&text)?)
}

/// Deliver one server message through the real exchange transport.
async fn to_worker(
    peer: &mut WebSocketStream<TcpStream>,
    message: ServerMessage,
) -> anyhow::Result<()> {
    peer.send(Message::Text(serde_json::to_string(&message)?.into()))
        .await?;
    Ok(())
}

#[tokio::test]
async fn status_stays_syncing_until_initial_receipt_completes() -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(15), async {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let directory = tempfile::tempdir()?;
        let account = AccountId::new();
        let binding = EnrollmentBinding {
            account,
            actor: ClientActorId::new(),
            server: format!("http://{address}"),
            credential_reference: "exchange-test".into(),
        };
        let store = ClientStore::create(
            &directory.path().join("workspace.sqlite"),
            account,
            Default::default(),
        )
        .await?;
        identity::bind(&store, None, binding.clone()).await?;
        let (metadata, baseline) =
            AccountMetadata::initialize(account, TimelineId::new(), BaselineId::new());
        let checkpoint = InstalledCheckpoint {
            account,
            baseline: baseline.id,
            coverage: metadata.installed,
        };

        let stop = CancellationToken::new();
        let connection = ConnectionContext {
            stop: stop.clone(),
            gate: Default::default(),
            trace: SpanContext::empty_context(),
            directory: watch::channel(None).0,
        };
        let (_local_changes, mut changes) = watch::channel(());
        let (status, mut view) = watch::channel(Status::Disconnected);
        let (updates, _settings) = watch::channel(None);
        let worker = async {
            let (mut socket, _) = connect_async(format!("ws://{address}")).await?;
            run(
                &mut socket,
                &store,
                &binding,
                &mut changes,
                &status,
                &updates,
                &connection,
            )
            .await
        };
        let peer = async {
            let (socket, _) = listener.accept().await?;
            let mut peer = accept_async(socket).await?;
            assert!(matches!(
                from_worker(&mut peer).await?,
                ClientMessage::Hello {
                    checkpoint: None,
                    ..
                }
            ));
            to_worker(
                &mut peer,
                ServerMessage::Hello {
                    version: messages::VERSION,
                    identity: ClientIdentity {
                        account,
                        actor: binding.actor,
                    },
                    checkpoint: checkpoint.clone(),
                    baselines: vec![baseline.clone()],
                    lineage: baseline.lineage.clone(),
                },
            )
            .await?;
            assert!(matches!(
                from_worker(&mut peer).await?,
                ClientMessage::DirectoryConfiguration
            ));
            let ClientMessage::Transfer {
                exchange,
                message: TransferMessage::Request { after: None },
            } = from_worker(&mut peer).await?
            else {
                anyhow::bail!("Expected a full initial transfer request");
            };
            to_worker(
                &mut peer,
                ServerMessage::Transfer {
                    exchange,
                    message: TransferMessage::Start(DiscoveryStart {
                        source: checkpoint.clone(),
                        mode: WindowMode::Full,
                    }),
                },
            )
            .await?;
            to_worker(
                &mut peer,
                ServerMessage::Transfer {
                    exchange,
                    message: TransferMessage::Page(DiscoveryPage {
                        sequence: 0,
                        requirements: vec![],
                        accepted: vec![],
                        support: vec![],
                        baselines: vec![baseline],
                        boundaries: vec![],
                        units: vec![],
                    }),
                },
            )
            .await?;
            assert!(matches!(
                from_worker(&mut peer).await?,
                ClientMessage::Transfer {
                    exchange: acknowledged,
                    message: TransferMessage::PageAck { sequence: 0 },
                } if acknowledged == exchange
            ));
            let partial_status = view.borrow_and_update().clone();
            assert!(
                matches!(partial_status, Status::Syncing),
                "partial receipt should report Syncing, got {partial_status:?}"
            );
            assert!(synchronization::state(&store).await?.checkpoint.is_none());

            to_worker(
                &mut peer,
                ServerMessage::Transfer {
                    exchange,
                    message: TransferMessage::End(DiscoveryEnd {
                        source: checkpoint.clone(),
                        pages: 1,
                    }),
                },
            )
            .await?;
            view.wait_for(|status| matches!(status, Status::UpToDate))
                .await?;
            assert_eq!(
                synchronization::state(&store).await?.checkpoint,
                Some(checkpoint)
            );
            stop.cancel();
            Ok::<_, anyhow::Error>(())
        };
        tokio::select! {
            biased;
            result = peer => result?,
            result = worker => {
                result?;
                anyhow::bail!("Worker stopped before the settled status was verified");
            }
        }
        store.close().await?;
        Ok(())
    })
    .await?
}

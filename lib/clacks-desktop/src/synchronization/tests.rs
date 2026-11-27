//! Real authenticated endpoint checks using the existing native enrollment host.

use super::{Connected, connect, exchange, receive, send};
use clacks_client_store::{
    identity::{self, EnrollmentBinding},
    save,
    store::ClientStore,
    synchronization,
};
use clacks_model::{
    ids::{AccountId, TagId},
    tag::LogbookTag,
};
use clacks_sync::{
    client::Preparation,
    protocol::{
        contracts::SyncContract,
        messages::{ClientMessage, FailureKind, ProposalOutcome, ServerMessage, TransferMessage},
    },
    record::{RecordId, RecordKey, RecordState, RecordValue},
};
use futures_util::SinkExt;
use tokio_util::sync::CancellationToken;

/// Check negotiation/scope and a committed proposal whose response is not installed locally.
pub(crate) async fn verify(binding: &EnrollmentBinding) -> anyhow::Result<()> {
    let authorizer = crate::enrollment::flow::protected(binding).await?;
    let stop = CancellationToken::new();
    let connection = super::ConnectionContext {
        stop: stop.clone(),
        gate: Default::default(),
        trace: opentelemetry::trace::SpanContext::empty_context(),
        directory: tokio::sync::watch::channel(None).0,
    };
    let Connected {
        mut socket,
        contract: SyncContract::V1,
    } = connect(binding, &authorizer, &tracing::Span::none()).await?;
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(
        send(
            &mut socket,
            &cancelled,
            ClientMessage::Hello {
                checkpoint: None,
                baselines: vec![],
                lineage: None
            }
        )
        .await
        .is_err()
    );

    let directory = tempfile::tempdir()?;
    let store = ClientStore::create(
        &directory.path().join("workspace.sqlite"),
        binding.account,
        Default::default(),
    )
    .await?;
    identity::bind(&store, None, binding.clone()).await?;
    let Connected {
        mut socket,
        contract: SyncContract::V1,
    } = connect(binding, &authorizer, &tracing::Span::none()).await?;
    send(
        &mut socket,
        &stop,
        ClientMessage::Hello {
            checkpoint: None,
            baselines: vec![],
            lineage: None,
        },
    )
    .await?;
    let ServerMessage::Hello {
        checkpoint: initial,
        ..
    } = receive(&mut socket, &connection).await?
    else {
        panic!("hello")
    };
    // A foreign-account checkpoint is not evidence for this authorized account.
    let mut foreign = initial.clone();
    foreign.account = AccountId::new();
    send(
        &mut socket,
        &stop,
        ClientMessage::Transfer {
            exchange: 1,
            message: TransferMessage::Request {
                after: Some(foreign),
            },
        },
    )
    .await?;
    assert!(matches!(
        receive(&mut socket, &connection).await?,
        ServerMessage::Failure {
            kind: FailureKind::Invalid,
            ..
        }
    ));
    assert!(receive(&mut socket, &connection).await.is_err());
    // Invalid contents must not be echoed back as a diagnostic.
    let Connected {
        socket: mut malformed,
        contract: SyncContract::V1,
    } = connect(binding, &authorizer, &tracing::Span::none()).await?;
    malformed
        .send(super::tungstenite::Message::Text(
            r#"{"unrecognized":"PRIVATE-PAYLOAD-CANARY"}"#.into(),
        ))
        .await?;
    let mut dirty = false;
    let failure = exchange::response(&mut malformed, &store, &mut dirty, &connection)
        .await
        .unwrap_err();
    let typed = failure.downcast_ref::<super::ServerFailure>().unwrap();
    assert_eq!(typed.kind, FailureKind::Invalid);
    assert!(!typed.message.contains("PRIVATE-PAYLOAD-CANARY"));

    let Connected {
        mut socket,
        contract: SyncContract::V1,
    } = connect(binding, &authorizer, &tracing::Span::none()).await?;
    send(
        &mut socket,
        &stop,
        ClientMessage::Hello {
            checkpoint: None,
            baselines: vec![],
            lineage: None,
        },
    )
    .await?;
    assert!(matches!(
        receive(&mut socket, &connection).await?,
        ServerMessage::Hello {checkpoint, ..} if checkpoint == initial
    ));
    let mut dirty = false;
    exchange::inbound(&mut socket, &store, 1, &mut dirty, &connection).await?;
    let key = RecordKey {
        account: binding.account,
        id: RecordId::Tag(TagId::new()),
    };
    let RecordId::Tag(id) = key.id else {
        unreachable!()
    };
    save::save(
        &store,
        key,
        None,
        RecordState::Live(RecordValue::Tag(Box::new(LogbookTag::new(
            id,
            "Retry endpoint check".into(),
        )))),
    )
    .await?;
    let synchronization::PrepareOutcome::Core(preparation) =
        synchronization::prepare(&store, key).await?
    else {
        panic!("enrolled")
    };
    let Preparation::Ready { change, .. } = *preparation else {
        panic!("ready")
    };
    send(
        &mut socket,
        &stop,
        ClientMessage::Propose {
            request: 1,
            change: change.clone(),
        },
    )
    .await?;
    let ServerMessage::Proposal {
        outcome: ProposalOutcome::Accepted(accepted),
        ..
    } = exchange::response(&mut socket, &store, &mut dirty, &connection).await?
    else {
        panic!("accepted")
    };
    // Deliberately discard the response before the local receipt boundary, then reconnect.
    drop(socket);
    assert!(
        save::load(&store, key)
            .await?
            .unwrap()
            .pending()
            .unwrap()
            .prepared
            .is_some()
    );
    let Connected {
        mut socket,
        contract: SyncContract::V1,
    } = connect(binding, &authorizer, &tracing::Span::none()).await?;
    let local = synchronization::state(&store).await?;
    send(
        &mut socket,
        &stop,
        ClientMessage::Hello {
            checkpoint: local.checkpoint,
            baselines: local.baselines,
            lineage: local.lineage,
        },
    )
    .await?;
    assert!(matches!(
        receive(&mut socket, &connection).await?,
        ServerMessage::Hello { .. }
    ));
    exchange::inbound(&mut socket, &store, 2, &mut dirty, &connection).await?;
    send(
        &mut socket,
        &stop,
        ClientMessage::Propose { request: 2, change },
    )
    .await?;
    let ServerMessage::Proposal {
        outcome: ProposalOutcome::Covered(retried),
        ..
    } = exchange::response(&mut socket, &store, &mut dirty, &connection).await?
    else {
        panic!("covered")
    };
    assert_eq!(retried, accepted);
    assert!(save::load(&store, key).await?.unwrap().pending().is_none());
    socket.close(None).await?;
    store.close().await?;
    Ok(())
}

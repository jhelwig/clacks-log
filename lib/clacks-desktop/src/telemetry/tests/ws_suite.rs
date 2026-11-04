//! One real capture and activation failure under ordinary production filters.
use crate::{
    workspace::WorkspacePaths,
    ws_suite::{
        self, Outputs, ReceiverAction, ReceiverRequest, ReceiverResponse, ReceiverView,
        ReceptionState,
    },
};
use clacks_client_store::{
    store::ClientStore,
    ws_suite::{ReceiverConnection, ReceiverSettings},
};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, watch};
use tokio_util::sync::CancellationToken;

/// Capture an authored pair, then fail an actual replacement bind without losing its save.
pub(super) async fn run(paths: &WorkspacePaths) -> anyhow::Result<()> {
    let store =
        ClientStore::create(&paths.database, Default::default(), Default::default()).await?;
    let probe = std::net::UdpSocket::bind("127.0.0.1:0")?;
    let endpoint = probe.local_addr()?;
    drop(probe);
    let settings = ReceiverSettings {
        enabled: true,
        connection: ReceiverConnection::Unicast {
            address: endpoint.ip(),
            port: endpoint.port(),
        },
    };
    let (commands, input) = mpsc::channel(8);
    let outputs = Outputs {
        calls: tokio::sync::mpsc::unbounded_channel().0,
        updates: watch::channel(ReceiverView::default()).0,
        context: Default::default(),
    };
    let stop = CancellationToken::new();
    let (changes, mut committed) = watch::channel(());
    let task = tokio::spawn(ws_suite::runtime::run(
        store.clone(),
        input,
        outputs,
        watch::channel(None).0,
        changes,
        Default::default(),
        stop.clone(),
    ));
    let (reply, result) = oneshot::channel();
    commands
        .send(ReceiverRequest {
            action: ReceiverAction::Apply { next: settings },
            reply,
        })
        .await?;
    let ReceiverResponse::Applied(view) = result.await? else {
        anyhow::bail!("receiver apply failed")
    };
    assert_eq!(view.reception, ReceptionState::Listening);
    let sender = tokio::net::UdpSocket::bind("127.0.0.1:0").await?;
    sender
        .send_to(
            &crate::test_support::ws_suite::structured("W7HEX"),
            endpoint,
        )
        .await?;
    sender
        .send_to(&crate::test_support::ws_suite::adif("W7HEX"), endpoint)
        .await?;
    tokio::time::timeout(Duration::from_secs(5), committed.changed()).await??;
    let conflict = std::net::UdpSocket::bind("127.0.0.1:0")?;
    let endpoint = conflict.local_addr()?;
    let (reply, result) = oneshot::channel();
    commands
        .send(ReceiverRequest {
            action: ReceiverAction::Apply {
                next: ReceiverSettings {
                    enabled: true,
                    connection: ReceiverConnection::Unicast {
                        address: endpoint.ip(),
                        port: endpoint.port(),
                    },
                },
            },
            reply,
        })
        .await?;
    let ReceiverResponse::Applied(view) = result.await? else {
        anyhow::bail!("receiver apply failed")
    };
    assert!(view.settings.unwrap().enabled);
    assert!(matches!(view.reception, ReceptionState::Failed(_)));
    stop.cancel();
    task.await??;
    store.close().await?;
    Ok(())
}

/// Verify bounded capture metrics and payload exclusion in the existing exported requests.
pub(super) fn verify(requests: &[super::collector::Request]) -> anyhow::Result<()> {
    use opentelemetry_proto::tonic::{
        collector::metrics::v1::ExportMetricsServiceRequest,
        metrics::v1::{metric::Data, number_data_point::Value},
    };
    use prost::Message;
    let mut saved = 0;
    for request in requests {
        for secret in ["packet-private!", "structured-only", "W7HEX", "W7ABC"] {
            assert!(
                !request
                    .body
                    .windows(secret.len())
                    .any(|bytes| bytes == secret.as_bytes())
            );
        }
        if request.path != "/v1/metrics" {
            continue;
        }
        let metrics = ExportMetricsServiceRequest::decode(request.body.as_slice())?;
        for metric in metrics
            .resource_metrics
            .iter()
            .flat_map(|r| &r.scope_metrics)
            .flat_map(|scope| &scope.metrics)
        {
            if metric.name == "clacks_ws_suite_captures_total" {
                let Some(Data::Sum(sum)) = &metric.data else {
                    panic!("capture sum")
                };
                for point in &sum.data_points {
                    assert_eq!(point.attributes.len(), 2);
                    assert!(
                        point.attributes.iter().all(|attribute| matches!(
                            attribute.key.as_str(),
                            "format" | "outcome"
                        ))
                    );
                    if let Some(Value::AsInt(value)) = point.value {
                        saved += value;
                    }
                }
            }
        }
    }
    assert_eq!(saved, 1);
    Ok(())
}

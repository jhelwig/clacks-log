//! Wire acceptance of the production session's startup and ordered shutdown.

use crate::{
    instance::{self, Election},
    session::{self, SessionCommand, SessionResources, SessionStatus},
    telemetry::TelemetryGuard,
    workspace::WorkspacePaths,
};
use clacks_model::ids::AccountId;
use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::sync::{mpsc, watch};

/// Run real ownership, startup and cleanup without requiring a display or collector daemon.
pub(super) fn complete(
    runtime: &tokio::runtime::Runtime,
    paths: &WorkspacePaths,
    telemetry: TelemetryGuard,
) -> anyhow::Result<AccountId> {
    let Election::Owner(owner) = instance::elect(paths)? else {
        anyhow::bail!("owner expected")
    };
    let (notices, activation) = watch::channel(());

    let listener = owner.listen(runtime.handle(), notices)?;
    let (commands, input) = mpsc::channel(8);
    let (updates, mut status) = watch::channel(SessionStatus::Opening);
    let resources = SessionResources {
        paths: paths.clone(),
        listener,
        telemetry,
        activation,
        context: Arc::new(OnceLock::new()),
        opening_gate: None,
    };
    let account = runtime.block_on(async {
        let task = tokio::spawn(session::run(resources, input, updates));
        let identity = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if let SessionStatus::Ready { identity, .. } = *status.borrow_and_update() {
                    return anyhow::Ok(identity);
                }
                status.changed().await?;
            }
        })
        .await??;
        commands.send(SessionCommand::BeginShutdown).await?;
        task.await??;

        assert!(matches!(*status.borrow(), SessionStatus::Closed));
        let store = clacks_client_store::store::ClientStore::open_discovered(
            &paths.database,
            Default::default(),
        )
        .await?;

        assert_eq!(store.account_id(), identity.account);
        store.close().await?;
        anyhow::Ok(identity.account)
    })?;
    drop(owner);

    assert!(matches!(instance::elect(paths)?, Election::Owner(_)));
    Ok(account)
}

/// Verify the workflow measurement, safe identity context and pre-flush domain milestone.
pub(super) fn assert_wire(requests: &[super::collector::Request]) -> anyhow::Result<()> {
    use opentelemetry_proto::tonic::{
        collector::{logs::v1::ExportLogsServiceRequest, metrics::v1::ExportMetricsServiceRequest},
        metrics::v1::metric::Data,
    };
    use prost::Message;
    let mut samples = 0;
    let mut ready = false;
    let mut drained = false;
    for request in requests {
        if request.path == "/v1/metrics" {
            let export = ExportMetricsServiceRequest::decode(request.body.as_slice())?;
            for metric in export
                .resource_metrics
                .iter()
                .flat_map(|resource| &resource.scope_metrics)
                .flat_map(|scope| &scope.metrics)
            {
                if metric.name == crate::application::STARTUP_DURATION_METRIC {
                    let Some(Data::Histogram(histogram)) = &metric.data else {
                        panic!("startup histogram required")
                    };
                    for point in &histogram.data_points {
                        assert_eq!(
                            point.attributes.len(),
                            1,
                            "only the bounded outcome dimension"
                        );

                        assert!(point.attributes.iter().any(|attribute| attribute.key == "outcome" && attribute.value.as_ref().is_some_and(|value| value.value == Some(opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue("ready".into())))));
                        samples += point.count;
                    }
                }
            }
        }
        if request.path == "/v1/logs" {
            let export = ExportLogsServiceRequest::decode(request.body.as_slice())?;
            for record in export
                .resource_logs
                .iter()
                .flat_map(|resource| &resource.scope_logs)
                .flat_map(|scope| &scope.log_records)
            {
                let event = record
                    .attributes
                    .iter()
                    .find(|attribute| attribute.key == "event")
                    .and_then(|attribute| attribute.value.as_ref())
                    .and_then(|value| value.value.as_ref());
                use opentelemetry_proto::tonic::common::v1::any_value::Value;
                if matches!(event, Some(Value::StringValue(value)) if value == "desktop.workspace_ready")
                {
                    ready = true;

                    assert!(
                        record
                            .attributes
                            .iter()
                            .any(|attribute| attribute.key == "account_id")
                    );
                }
                drained |= matches!(event, Some(Value::StringValue(value)) if value == "desktop.work_drained");
            }
        }
    }

    assert_eq!(samples, 1);
    assert!(ready && drained);
    Ok(())
}

//! Standard process observability for server and operator executables.

use opentelemetry::{metrics::MeterProvider as _, trace::TracerProvider as _};
use opentelemetry_otlp::{Protocol, WithExportConfig};
use opentelemetry_sdk::{
    Resource, logs::SdkLoggerProvider, metrics::SdkMeterProvider, trace::SdkTracerProvider,
};
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};

/// SDK resources retained until serving and database work have stopped.
#[derive(Default)]
pub struct Telemetry {
    /// Trace provider retained through the final request and flush.
    traces: Option<SdkTracerProvider>,
    /// Metric provider retained through the final measurement and flush.
    metrics: Option<SdkMeterProvider>,
    /// Log provider retained through shutdown diagnostics and flush.
    logs: Option<SdkLoggerProvider>,
}

impl Telemetry {
    /// Flush opted-in exporters after request shutdown, outside an async worker.
    /// Returns an error if any exporter fails or exceeds its shutdown bound.
    pub fn shutdown(self) -> anyhow::Result<()> {
        let timeout = std::time::Duration::from_secs(5);
        let mut failed = false;
        if let Some(provider) = self.traces {
            failed |= provider.shutdown_with_timeout(timeout).is_err();
        }
        if let Some(provider) = self.metrics {
            failed |= provider.shutdown_with_timeout(timeout).is_err();
        }
        if let Some(provider) = self.logs {
            failed |= provider.shutdown_with_timeout(timeout).is_err();
        }
        anyhow::ensure!(
            !failed,
            "telemetry export did not finish successfully during shutdown"
        );
        Ok(())
    }
}

/// Install structured stderr logs and optional standard OTLP HTTP/protobuf export.
///
/// Exporter endpoints, headers and timeouts use the OpenTelemetry environment
/// variables. Call once in an executable, retaining the result through shutdown.
pub fn init(otlp: bool) -> anyhow::Result<Telemetry> {
    let mut telemetry = Telemetry::default();
    if otlp {
        let resource = Resource::builder()
            .with_service_name("clacks-server")
            .with_attribute(opentelemetry::KeyValue::new(
                "service.version",
                env!("CARGO_PKG_VERSION"),
            ))
            .build();
        let trace_exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .build()
            .map_err(|_| {
                anyhow::anyhow!(
                    "invalid trace exporter configuration; check endpoint, headers and transport"
                )
            })?;
        let metric_exporter = opentelemetry_otlp::MetricExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .build()
            .map_err(|_| {
                anyhow::anyhow!(
                    "invalid metric exporter configuration; check endpoint, headers and transport"
                )
            })?;
        let log_exporter = opentelemetry_otlp::LogExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .build()
            .map_err(|_| {
                anyhow::anyhow!(
                    "invalid log exporter configuration; check endpoint, headers and transport"
                )
            })?;
        telemetry.traces = Some(
            SdkTracerProvider::builder()
                .with_batch_exporter(trace_exporter)
                .with_resource(resource.clone())
                .build(),
        );
        let metrics = SdkMeterProvider::builder()
            .with_periodic_exporter(metric_exporter)
            .with_resource(resource.clone())
            .build();
        let mut recorder = metrics_opentelemetry::OpenTelemetryRecorder::new(
            metrics_opentelemetry::OpenTelemetryMetrics::new(metrics.meter("clacks_server")),
        );
        let fine = &[
            0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            30.0, 60.0, 300.0,
        ];
        recorder.set_histogram_bounds_mut(
            [
                crate::store::retention::SYNC_BASELINE_LOCK_WAIT_SECONDS_METRIC,
                crate::store::retention::SYNC_BASELINE_LOCK_HOLD_SECONDS_METRIC,
                crate::store::synchronization::SYNC_ACCEPT_LOCK_WAIT_SECONDS_METRIC,
                crate::store::synchronization::SYNC_ACKNOWLEDGMENT_LOCK_WAIT_SECONDS_METRIC,
                crate::store::synchronization::SYNC_RETURN_COMPLETE_LOCK_WAIT_SECONDS_METRIC,
                crate::store::synchronization::SYNC_RETURN_LOCK_WAIT_SECONDS_METRIC,
                crate::store::synchronization::SYNC_NEGOTIATION_LOCK_WAIT_SECONDS_METRIC,
                crate::enrollment::ENROLLMENT_REVOCATION_LOCK_WAIT_SECONDS_METRIC,
                crate::store::retention::SYNC_BASELINE_INSTALL_DURATION_SECONDS_METRIC,
                crate::store::retention::SYNC_JOURNAL_PRUNE_DURATION_SECONDS_METRIC,
                crate::store::retention::SYNC_METADATA_RECLAIM_DURATION_SECONDS_METRIC,
                crate::synchronization::SYNC_DISPATCH_DURATION_SECONDS_METRIC,
            ]
            .into_iter(),
            fine,
        );
        recorder.set_histogram_bounds_mut(
            [crate::store::retention::SYNC_BASELINE_CONTEXTS_EXAMINED_METRIC].into_iter(),
            &[0.0, 1.0, 10.0, 100.0, 1000.0, 10000.0, 100000.0],
        );
        metrics::set_global_recorder(recorder)
            .map_err(|_| anyhow::anyhow!("a metrics recorder is already installed"))?;
        for (name, description) in [
            (
                crate::store::retention::SYNC_BASELINE_LOCK_WAIT_SECONDS_METRIC,
                "Baseline account lock acquisition wait",
            ),
            (
                crate::store::retention::SYNC_BASELINE_LOCK_HOLD_SECONDS_METRIC,
                "Baseline account lock holding through transaction completion",
            ),
            (
                crate::store::synchronization::SYNC_ACCEPT_LOCK_WAIT_SECONDS_METRIC,
                "Publication account lock acquisition wait",
            ),
            (
                crate::store::synchronization::SYNC_ACKNOWLEDGMENT_LOCK_WAIT_SECONDS_METRIC,
                "Acknowledgement account lock acquisition wait",
            ),
            (
                crate::store::synchronization::SYNC_RETURN_COMPLETE_LOCK_WAIT_SECONDS_METRIC,
                "Returned window completion account lock wait",
            ),
            (
                crate::store::synchronization::SYNC_RETURN_LOCK_WAIT_SECONDS_METRIC,
                "Returned page account lock wait",
            ),
            (
                crate::store::synchronization::SYNC_NEGOTIATION_LOCK_WAIT_SECONDS_METRIC,
                "Negotiation account lock wait",
            ),
            (
                crate::enrollment::ENROLLMENT_REVOCATION_LOCK_WAIT_SECONDS_METRIC,
                "Revocation account lock wait",
            ),
            (
                crate::store::retention::SYNC_BASELINE_INSTALL_DURATION_SECONDS_METRIC,
                "Baseline installation including transaction completion",
            ),
            (
                crate::store::retention::SYNC_JOURNAL_PRUNE_DURATION_SECONDS_METRIC,
                "One journal pruning transaction",
            ),
            (
                crate::store::retention::SYNC_METADATA_RECLAIM_DURATION_SECONDS_METRIC,
                "One metadata reclamation transaction",
            ),
            (
                crate::synchronization::SYNC_DISPATCH_DURATION_SECONDS_METRIC,
                "One completed server synchronization message dispatch",
            ),
        ] {
            metrics::describe_histogram!(name, metrics::Unit::Seconds, description);
        }
        metrics::describe_histogram!(
            crate::store::retention::SYNC_BASELINE_CONTEXTS_EXAMINED_METRIC,
            metrics::Unit::Count,
            "Current contexts examined for baseline installation"
        );
        metrics::describe_counter!(
            crate::oauth::OAUTH_EXPIRED_ROWS_TOTAL_METRIC,
            metrics::Unit::Count,
            "Expired issuer rows removed by confirmed commits"
        );
        opentelemetry::global::set_meter_provider(metrics.clone());
        telemetry.metrics = Some(metrics);
        if let Some(provider) = &telemetry.traces {
            opentelemetry::global::set_tracer_provider(provider.clone());
        }

        telemetry.logs = Some(
            SdkLoggerProvider::builder()
                .with_batch_exporter(log_exporter)
                .with_resource(resource)
                .build(),
        );
        opentelemetry::global::set_text_map_propagator(
            opentelemetry_sdk::propagation::TraceContextPropagator::new(),
        );
    }

    // Dependency diagnostics can include bearer IDs, request bodies or URLs.
    let mut filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    for directive in [
        "axum_session=off",
        "reqwest=warn",
        "hyper=warn",
        "h2=warn",
        "sqlx=warn",
        "opentelemetry=warn",
    ] {
        filter = filter.add_directive(directive.parse()?);
    }
    let spans = telemetry.traces.as_ref().map(|provider| {
        tracing_opentelemetry::layer().with_tracer(provider.tracer("clacks_server"))
    });
    let logs = telemetry.logs.as_ref().map(|provider| {
        opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(provider)
            .with_filter(
                tracing_subscriber::filter::Targets::new()
                    .with_default(tracing_subscriber::filter::LevelFilter::TRACE)
                    .with_targets(
                        ["opentelemetry", "reqwest", "hyper", "h2"]
                            .map(|target| (target, tracing_subscriber::filter::LevelFilter::OFF)),
                    ),
            )
    });
    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::filter::Targets::new()
                .with_default(tracing_subscriber::filter::LevelFilter::TRACE)
                .with_target("axum_session", tracing_subscriber::filter::LevelFilter::OFF)
                .with_targets(
                    ["opentelemetry", "reqwest", "hyper", "h2", "sqlx"]
                        .map(|target| (target, tracing::Level::WARN)),
                ),
        )
        .with(tracing_subscriber::filter::filter_fn(|metadata| {
            // This field can contain arbitrary collector-provided text.
            metadata.fields().field("error_message").is_none()
        }))
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_writer(std::io::stderr),
        )
        .with(spans)
        .with(logs)
        .try_init()
        .map_err(|error| anyhow::anyhow!("could not initialize operator logging: {error}"))?;
    Ok(telemetry)
}

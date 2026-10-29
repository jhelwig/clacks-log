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
            .build()?;
        let metric_exporter = opentelemetry_otlp::MetricExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .build()?;
        let log_exporter = opentelemetry_otlp::LogExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .build()?;
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
        recorder.set_histogram_bounds_mut(
            ["clacks_http_request_duration_seconds"].into_iter(),
            &[
                0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ],
        );
        metrics::set_global_recorder(recorder)
            .map_err(|_| anyhow::anyhow!("a metrics recorder is already installed"))?;
        telemetry.metrics = Some(metrics);
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
            .with_filter(tracing_subscriber::filter::filter_fn(|metadata| {
                let target = metadata.target();
                !["opentelemetry", "reqwest", "hyper", "h2"]
                    .iter()
                    .any(|prefix| target.starts_with(prefix))
            }))
    });
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::filter::filter_fn(|metadata| {
            // Partial-success replies carry arbitrary collector-provided text.
            !metadata.target().starts_with("opentelemetry")
                || metadata.fields().field("error_message").is_none()
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

//! Executable-owned standard exporters for short-lived geographic workflows.

use opentelemetry::{metrics::MeterProvider as _, trace::TracerProvider as _};
use opentelemetry_otlp::{Protocol, WithExportConfig};
use opentelemetry_sdk::{
    Resource, logs::SdkLoggerProvider, metrics::SdkMeterProvider, trace::SdkTracerProvider,
};
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};

/// Retain standard SDK workers until the command has finished, including failures.
#[derive(Default)]
pub(crate) struct Telemetry {
    /// Optional trace provider and its batch worker.
    traces: Option<SdkTracerProvider>,
    /// Optional metric provider, recorder and periodic reader.
    metrics: Option<SdkMeterProvider>,
    /// Optional structured log provider and its batch worker.
    logs: Option<SdkLoggerProvider>,
}

impl Telemetry {
    /// Complete export before process exit without changing the command's outcome.
    pub(crate) fn shutdown(self) {
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
        if failed {
            let _local = opentelemetry::Context::enter_telemetry_suppressed_scope();
            tracing::warn!("Geographic telemetry export incomplete during shutdown");
        }
    }
}

/// Install structured stderr and explicitly opted-in standard OTLP exporters once.
pub(crate) fn initialize(
    enabled: bool,
    filter: tracing_subscriber::EnvFilter,
) -> anyhow::Result<Telemetry> {
    let mut telemetry = Telemetry::default();
    if enabled {
        let resource = Resource::builder()
            .with_service_name("clacks-geography-build")
            .with_attribute(opentelemetry::KeyValue::new(
                "service.version",
                env!("CARGO_PKG_VERSION"),
            ))
            .build();
        telemetry.traces = Some(
            SdkTracerProvider::builder()
                .with_batch_exporter(
                    opentelemetry_otlp::SpanExporter::builder()
                        .with_http()
                        .with_protocol(Protocol::HttpBinary)
                        .build().map_err(|_| anyhow::anyhow!("invalid trace exporter configuration; check endpoint, headers and transport"))?,
                )
                .with_resource(resource.clone())
                .build(),
        );
        let provider = SdkMeterProvider::builder()
            .with_periodic_exporter(
                opentelemetry_otlp::MetricExporter::builder()
                    .with_http()
                    .with_protocol(Protocol::HttpBinary)
                    .build().map_err(|_| anyhow::anyhow!("invalid metric exporter configuration; check endpoint, headers and transport"))?,
            )
            .with_resource(resource.clone())
            .build();
        let mut recorder = metrics_opentelemetry::OpenTelemetryRecorder::new(
            metrics_opentelemetry::OpenTelemetryMetrics::new(
                provider.meter("clacks_geography_builder"),
            ),
        );
        recorder.set_histogram_bounds_mut(
            [
                crate::application::BUILD_DURATION_METRIC,
                clacks_geography::lookup::LOOKUP_DURATION_METRIC,
            ]
            .into_iter(),
            &[
                0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
                10.0, 30.0, 60.0, 300.0,
            ],
        );
        metrics::set_global_recorder(recorder)
            .map_err(|_| anyhow::anyhow!("a metrics recorder is already installed"))?;
        telemetry.metrics = Some(provider);
        telemetry.logs = Some(
            SdkLoggerProvider::builder()
                .with_batch_exporter(
                    opentelemetry_otlp::LogExporter::builder()
                        .with_http()
                        .with_protocol(Protocol::HttpBinary)
                        .build().map_err(|_| anyhow::anyhow!("invalid log exporter configuration; check endpoint, headers and transport"))?,
                )
                .with_resource(resource)
                .build(),
        );
    }
    metrics::describe_histogram!(
        crate::application::BUILD_DURATION_METRIC,
        metrics::Unit::Seconds,
        "Build attempt through publication, failure or cancellation"
    );
    metrics::describe_histogram!(
        clacks_geography::lookup::LOOKUP_DURATION_METRIC,
        metrics::Unit::Seconds,
        "Complete geographic lookup"
    );
    metrics::describe_counter!(
        crate::application::BUILD_RUNS_METRIC,
        metrics::Unit::Count,
        "Build attempts by actual publication outcome"
    );
    let spans = telemetry.traces.as_ref().map(|provider| {
        tracing_opentelemetry::layer().with_tracer(provider.tracer("clacks_geography_builder"))
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
                .with_targets(
                    ["opentelemetry", "reqwest", "hyper", "h2"]
                        .map(|target| (target, tracing::Level::WARN)),
                ),
        )
        .with(tracing_subscriber::filter::filter_fn(|metadata| {
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
        .map_err(|error| anyhow::anyhow!("configure geographic diagnostics: {error}"))?;
    Ok(telemetry)
}

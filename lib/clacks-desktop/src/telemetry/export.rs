//! Standard OTLP HTTP/protobuf providers and metrics-facade integration.

use crate::config::telemetry::TelemetryConfig;
use opentelemetry::{KeyValue, metrics::MeterProvider as _};
use opentelemetry_otlp::{Protocol, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::{
    Resource, logs::SdkLoggerProvider, metrics::SdkMeterProvider, trace::SdkTracerProvider,
};
use std::collections::{BTreeMap, HashMap};

/// Preserve literal resolved header values across the SDK's decoding boundary.
fn sdk_headers(headers: &BTreeMap<String, String>) -> HashMap<String, String> {
    headers
        .iter()
        .map(|(name, value)| {
            (
                name.clone(),
                percent_encoding::utf8_percent_encode(value, percent_encoding::NON_ALPHANUMERIC)
                    .to_string(),
            )
        })
        .collect()
}

/// Standard provider ownership retained through application shutdown.
#[derive(Default)]
pub(super) struct Providers {
    /// Tracing adapter's provider, when span export initialized.
    pub traces: Option<SdkTracerProvider>,
    /// Metrics recorder's provider, when measurement export initialized.
    pub metrics: Option<SdkMeterProvider>,
    /// Structured-event bridge's provider, when log export initialized.
    pub logs: Option<SdkLoggerProvider>,
}

/// Construct each opted-in signal independently with standard bounded SDK workers.
pub(super) fn initialize(config: &TelemetryConfig, diagnostics: &mut Vec<String>) -> Providers {
    if !config.enabled {
        return Providers::default();
    }

    let resource = Resource::builder()
        .with_service_name("clacks-log")
        .with_attributes([
            KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
            KeyValue::new("process.pid", i64::from(std::process::id())),
        ])
        .build();

    let traces = match opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(&config.traces.endpoint)
        .with_headers(sdk_headers(&config.traces.headers))
        .with_timeout(config.traces.timeout)
        .build()
    {
        Ok(exporter) => Some(
            SdkTracerProvider::builder()
                .with_batch_exporter(exporter)
                .with_resource(resource.clone())
                .build(),
        ),
        Err(error) => {
            diagnostics.push(super::diagnostics::exporter("Trace", error));
            None
        }
    };

    let metrics = match opentelemetry_otlp::MetricExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(&config.metrics.endpoint)
        .with_headers(sdk_headers(&config.metrics.headers))
        .with_timeout(config.metrics.timeout)
        .build()
    {
        Ok(exporter) => Some(
            SdkMeterProvider::builder()
                .with_periodic_exporter(exporter)
                .with_resource(resource.clone())
                .build(),
        ),
        Err(error) => {
            diagnostics.push(super::diagnostics::exporter("Metric", error));
            None
        }
    };

    let logs = match opentelemetry_otlp::LogExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(&config.logs.endpoint)
        .with_headers(sdk_headers(&config.logs.headers))
        .with_timeout(config.logs.timeout)
        .build()
    {
        Ok(exporter) => Some(
            SdkLoggerProvider::builder()
                .with_batch_exporter(exporter)
                .with_resource(resource)
                .build(),
        ),
        Err(error) => {
            diagnostics.push(super::diagnostics::exporter("Log", error));
            None
        }
    };

    if let Some(provider) = &metrics {
        let meter = provider.meter("clacks_desktop");
        let mut recorder = metrics_opentelemetry::OpenTelemetryRecorder::new(
            metrics_opentelemetry::OpenTelemetryMetrics::new(meter),
        );
        recorder.set_histogram_bounds_mut(
            [crate::application::STARTUP_DURATION_METRIC].into_iter(),
            crate::application::STARTUP_DURATION_BOUNDS,
        );
        recorder.set_histogram_bounds_mut(
            [
                crate::synchronization::CLIENT_REQUEST_DURATION_METRIC,
                crate::session::settings::CONTACT_QUERY_DURATION_METRIC,
                clacks_client_store::retention::METADATA_RECLAIM_DURATION_METRIC,
                clacks_geography::lookup::LOOKUP_DURATION_METRIC,
                crate::synchronization::PHASE_DURATION_METRIC,
            ]
            .into_iter(),
            &[
                0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
                10.0, 30.0, 60.0, 300.0,
            ],
        );
        if metrics::set_global_recorder(recorder).is_err() {
            diagnostics.push(
                "Metric export could not install its recorder; an existing recorder is active"
                    .into(),
            );
        }
    }

    for (name, description) in [
        (
            crate::synchronization::CLIENT_REQUEST_DURATION_METRIC,
            "Native upgrade request through response headers or failure",
        ),
        (
            crate::session::settings::CONTACT_QUERY_DURATION_METRIC,
            "Completed contact read including snapshot release",
        ),
        (
            clacks_client_store::retention::METADATA_RECLAIM_DURATION_METRIC,
            "Completed local synchronization metadata reclamation",
        ),
        (
            clacks_geography::lookup::LOOKUP_DURATION_METRIC,
            "Complete geographic lookup",
        ),
        (
            crate::synchronization::PHASE_DURATION_METRIC,
            "Completed or failed bounded synchronization phase",
        ),
    ] {
        metrics::describe_histogram!(name, metrics::Unit::Seconds, description);
    }
    Providers {
        traces,

        metrics,
        logs,
    }
}

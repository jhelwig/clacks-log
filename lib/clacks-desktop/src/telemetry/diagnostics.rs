//! Safe operator diagnostics for export construction failures.

/// Describe the corrective category without formatting secret-bearing SDK messages.
pub(super) fn exporter(signal: &str, error: opentelemetry_otlp::ExporterBuildError) -> String {
    let reason = match error {
        opentelemetry_otlp::ExporterBuildError::InvalidConfiguration(_) => {
            "invalid exporter configuration"
        }
        opentelemetry_otlp::ExporterBuildError::InternalFailure(_) => {
            "HTTP/TLS client initialization failed"
        }
    };
    format!("{signal} export unavailable: {reason}; check telemetry configuration")
}

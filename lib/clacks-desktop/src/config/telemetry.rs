//! Resolve optional OTLP controls without exposing endpoints or headers in errors.

use super::{
    Cli, Environment,
    file::{SignalFile, TelemetryFile},
};
use http::{HeaderName, HeaderValue};
use std::{collections::BTreeMap, time::Duration};

/// Export opt-in and independently resolved signal destinations.
#[derive(Clone)]
pub(crate) struct TelemetryConfig {
    /// Explicit application permission to initialize any exporters.
    pub enabled: bool,
    /// Completed trace destination settings.
    pub traces: SignalConfig,
    /// Completed metric destination settings.
    pub metrics: SignalConfig,
    /// Completed structured-event destination settings.
    pub logs: SignalConfig,
}

/// HTTP/protobuf destination settings without secret-bearing Debug formatting.
#[derive(Clone)]
pub(crate) struct SignalConfig {
    /// Validated complete destination URL, possibly containing sensitive query data.
    pub endpoint: String,
    /// Validated request header names/values; not ordinary diagnostic fields.
    pub headers: BTreeMap<String, String>,
    /// Finite positive request deadline.
    pub timeout: Duration,
}

/// Read an OpenTelemetry setting, treating empty values as unset per its contract.
fn setting(environment: &Environment, name: &str) -> anyhow::Result<Option<String>> {
    Ok(environment.value(name)?.filter(|value| !value.is_empty()))
}

/// Parse a supported textual Boolean without echoing its input.
fn boolean(value: &str) -> anyhow::Result<bool> {
    if value.eq_ignore_ascii_case("true") {
        Ok(true)
    } else if value.eq_ignore_ascii_case("false") {
        Ok(false)
    } else {
        anyhow::bail!("invalid telemetry Boolean setting")
    }
}

/// Resolve application opt-in before constructing any destination adapters.
pub(super) fn resolve(
    cli: &Cli,
    environment: &Environment,
    file: &TelemetryFile,
) -> anyhow::Result<TelemetryConfig> {
    let enabled = if let Some(value) = cli.otlp {
        value
    } else {
        let disabled = setting(environment, "OTEL_SDK_DISABLED")?
            .map(|value| boolean(&value))
            .transpose()?;
        !disabled.unwrap_or(false) && file.enabled.unwrap_or(false)
    };
    Ok(TelemetryConfig {
        enabled,
        traces: signal(
            "traces",
            cli.otlp_traces_endpoint.as_deref(),
            cli,
            environment,
            file,
            &file.traces,
        )?,
        metrics: signal(
            "metrics",
            cli.otlp_metrics_endpoint.as_deref(),
            cli,
            environment,
            file,
            &file.metrics,
        )?,
        logs: signal(
            "logs",
            cli.otlp_logs_endpoint.as_deref(),
            cli,
            environment,
            file,
            &file.logs,
        )?,
    })
}

/// Validate an endpoint and append the standard path only for generic bases.
fn endpoint(value: &str, append_signal: Option<&str>) -> anyhow::Result<String> {
    let mut url =
        url::Url::parse(value).map_err(|_| anyhow::anyhow!("invalid telemetry endpoint URL"))?;
    anyhow::ensure!(
        matches!(url.scheme(), "http" | "https") && url.host().is_some(),
        "telemetry endpoints require HTTP or HTTPS"
    );
    anyhow::ensure!(
        url.username().is_empty() && url.password().is_none(),
        "telemetry endpoint credentials must use configured headers"
    );
    if let Some(signal) = append_signal {
        let path = format!("{}/v1/{signal}", url.path().trim_end_matches('/'));
        url.set_path(&path);
    }
    Ok(url.into())
}

/// Validate header entries without retaining parser errors containing values.
fn headers(values: BTreeMap<String, String>) -> anyhow::Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    for (name, value) in values {
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| anyhow::anyhow!("invalid telemetry header name"))?;
        HeaderValue::from_str(&value)
            .map_err(|_| anyhow::anyhow!("invalid telemetry header value"))?;
        result.insert(name.as_str().into(), value);
    }
    Ok(result)
}

/// Decode the standard comma-separated OpenTelemetry header setting.
fn environment_headers(value: String) -> anyhow::Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    for entry in value.split(',') {
        let (name, value) = entry
            .trim()
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("invalid telemetry header setting"))?;
        let value = percent_encoding::percent_decode_str(value.trim())
            .decode_utf8()
            .map_err(|_| anyhow::anyhow!("invalid telemetry header encoding"))?;
        result.insert(name.trim().into(), value.into_owned());
    }
    headers(result)
}

/// Select one signal's destination with tier priority ahead of signal specificity.
fn signal(
    name: &str,
    cli_endpoint: Option<&str>,
    cli: &Cli,
    environment: &Environment,
    file: &TelemetryFile,
    specific: &SignalFile,
) -> anyhow::Result<SignalConfig> {
    let prefix = format!("OTEL_EXPORTER_OTLP_{}", name.to_uppercase());
    let protocol = signal_setting(environment, &prefix, "PROTOCOL")?;
    anyhow::ensure!(
        protocol
            .as_deref()
            .is_none_or(|value| value == "http/protobuf"),
        "unsupported telemetry transport; use http/protobuf"
    );
    let endpoint = if let Some(value) = cli_endpoint {
        endpoint(value, None)?
    } else if let Some(value) = &cli.otlp_endpoint {
        endpoint(value, Some(name))?
    } else if let Some(value) = setting(environment, &format!("{prefix}_ENDPOINT"))? {
        endpoint(&value, None)?
    } else if let Some(value) = setting(environment, "OTEL_EXPORTER_OTLP_ENDPOINT")? {
        endpoint(&value, Some(name))?
    } else if let Some(value) = &specific.endpoint {
        endpoint(value, None)?
    } else {
        endpoint(
            file.endpoint.as_deref().unwrap_or("http://localhost:4318"),
            Some(name),
        )?
    };
    let env_headers = signal_setting(environment, &prefix, "HEADERS")?;
    let headers = if let Some(value) = env_headers {
        environment_headers(value)?
    } else {
        headers(
            specific
                .headers
                .clone()
                .or(file.headers.clone())
                .unwrap_or_default(),
        )?
    };
    let timeout_ms = if let Some(value) = cli.otlp_timeout_ms {
        value
    } else {
        signal_setting(environment, &prefix, "TIMEOUT")?
            .map(|value| {
                value
                    .parse::<u64>()
                    .map_err(|_| anyhow::anyhow!("invalid telemetry timeout"))
            })
            .transpose()?
            .or(specific.timeout_ms)
            .or(file.timeout_ms)
            .unwrap_or(5000)
    };
    anyhow::ensure!(timeout_ms > 0, "telemetry timeout must be positive");
    Ok(SignalConfig {
        endpoint,
        headers,
        timeout: Duration::from_millis(timeout_ms),
    })
}

/// Honor per-signal environment values before reading a shadowed generic value.
fn signal_setting(
    environment: &Environment,
    prefix: &str,
    suffix: &str,
) -> anyhow::Result<Option<String>> {
    if let Some(value) = setting(environment, &format!("{prefix}_{suffix}"))? {
        Ok(Some(value))
    } else {
        setting(environment, &format!("OTEL_EXPORTER_OTLP_{suffix}"))
    }
}

//! OpenTelemetry setting conventions layered by config, then validated for export.

use super::{
    Cli,
    file::{SignalFile, TelemetryFile},
    sources,
};
use ::config::{Config, Environment};
use http::{HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
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

/// Endpoint interpretation supplied by the selected configuration source.
#[derive(Serialize, Deserialize)]
struct EndpointInput {
    /// Supplied URL before validation.
    value: String,
    /// Whether the standard per-signal path must be appended.
    base: bool,
}

/// One source's signal settings; absent fields do not override lower-priority sources.
#[derive(Default, Serialize)]
struct SignalLayer {
    /// Supplied endpoint and its generic-versus-complete meaning.
    #[serde(skip_serializing_if = "Option::is_none")]
    endpoint: Option<EndpointInput>,
    /// Request headers combined by key through ordinary config layering.
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<BTreeMap<String, String>>,
    /// External timeout text, parsed after higher-priority overrides are applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout_ms: Option<String>,
    /// External transport selection.
    #[serde(skip_serializing_if = "Option::is_none")]
    protocol: Option<String>,
}

/// Selected signal settings, before URL/header and duration validation.
#[derive(Deserialize)]
#[serde(default)]
struct SignalSettings {
    /// Selected endpoint with its interpretation intact.
    endpoint: EndpointInput,
    /// Combined request headers.
    headers: BTreeMap<String, String>,
    /// Positive request deadline in milliseconds.
    timeout_ms: u64,
    /// Supported wire protocol name.
    protocol: String,
}

impl Default for SignalSettings {
    fn default() -> Self {
        Self {
            endpoint: EndpointInput {
                value: "http://localhost:4318".into(),
                base: true,
            },
            headers: BTreeMap::new(),
            timeout_ms: 5000,
            protocol: "http/protobuf".into(),
        }
    }
}

/// Export permission before applying the SDK-disable policy.
#[derive(Default, Deserialize)]
#[serde(default)]
struct ExportPolicy {
    /// Explicit application opt-in.
    enabled: bool,
    /// Standard SDK veto, overridden by an explicit CLI choice.
    disabled: bool,
}

/// Resolve application policy and each signal with standard configuration layers.
pub(super) fn resolve(
    cli: &Cli,
    environment: &Environment,
    file: &TelemetryFile,
) -> anyhow::Result<TelemetryConfig> {
    let otel = sources::environment(environment, "OTEL")?;

    let policy = Config::builder()
        .set_override_option("enabled", file.enabled)?
        .add_source(
            Config::builder()
                .set_override_option("disabled", sources::optional(&otel, "sdk_disabled")?)?
                .build()?,
        )
        .set_override_option("enabled", cli.otlp)?
        .set_override_option("disabled", cli.otlp.map(|_| false))?
        .build()
        .and_then(Config::try_deserialize::<ExportPolicy>)
        .map_err(|error| sources::invalid("telemetry", error))?;

    let external = sources::environment(environment, "OTEL_EXPORTER_OTLP")?;
    Ok(TelemetryConfig {
        enabled: policy.enabled && !policy.disabled,
        traces: signal(
            "traces",
            cli.otlp_traces_endpoint.as_deref(),
            cli,
            file,
            &file.traces,
            &external,
        )?,
        metrics: signal(
            "metrics",
            cli.otlp_metrics_endpoint.as_deref(),
            cli,
            file,
            &file.metrics,
            &external,
        )?,
        logs: signal(
            "logs",
            cli.otlp_logs_endpoint.as_deref(),
            cli,
            file,
            &file.logs,
            &external,
        )?,
    })
}

/// Adapt the standard environment names and encoded header syntax into one source.
fn environment_layer(source: &Config, prefix: &str, base: bool) -> anyhow::Result<SignalLayer> {
    Ok(SignalLayer {
        endpoint: sources::optional(source, &format!("{prefix}endpoint"))?
            .map(|value| EndpointInput { value, base }),
        headers: sources::optional(source, &format!("{prefix}headers"))?
            .map(environment_headers)
            .transpose()?,
        timeout_ms: sources::optional(source, &format!("{prefix}timeout"))?,
        protocol: sources::optional(source, &format!("{prefix}protocol"))?,
    })
}

/// Layer generic and specific settings inside each source tier, with CLI overrides last.
fn signal(
    name: &str,
    cli_endpoint: Option<&str>,
    cli: &Cli,
    file: &TelemetryFile,
    specific: &SignalFile,
    external: &Config,
) -> anyhow::Result<SignalConfig> {
    let generic_file = SignalLayer {
        endpoint: file
            .endpoint
            .clone()
            .map(|value| EndpointInput { value, base: true }),
        headers: file.headers.clone().map(headers).transpose()?,
        timeout_ms: file.timeout_ms.map(|value| value.to_string()),
        protocol: None,
    };

    let specific_file = SignalLayer {
        endpoint: specific
            .endpoint
            .clone()
            .map(|value| EndpointInput { value, base: false }),
        headers: specific.headers.clone().map(headers).transpose()?,
        timeout_ms: specific.timeout_ms.map(|value| value.to_string()),
        protocol: None,
    };

    let generic_environment = environment_layer(external, "", true)?;

    let specific_environment = environment_layer(external, &format!("{name}_"), false)?;

    let generic_cli = SignalLayer {
        endpoint: cli
            .otlp_endpoint
            .clone()
            .map(|value| EndpointInput { value, base: true }),
        timeout_ms: cli.otlp_timeout_ms.map(|value| value.to_string()),
        ..Default::default()
    };

    let specific_cli = SignalLayer {
        endpoint: cli_endpoint.map(|value| EndpointInput {
            value: value.into(),
            base: false,
        }),
        ..Default::default()
    };

    let mut builder = Config::builder();
    for layer in [
        generic_file,
        specific_file,
        generic_environment,
        specific_environment,
        generic_cli,
        specific_cli,
    ] {
        builder = builder.add_source(
            Config::try_from(&layer).map_err(|error| sources::invalid("telemetry", error))?,
        );
    }

    let settings = builder
        .build()
        .and_then(Config::try_deserialize::<SignalSettings>)
        .map_err(|error| sources::invalid("telemetry", error))?;

    anyhow::ensure!(
        settings.protocol == "http/protobuf",
        "unsupported telemetry transport; use http/protobuf"
    );
    anyhow::ensure!(
        settings.timeout_ms > 0,
        "telemetry timeout must be positive"
    );
    Ok(SignalConfig {
        endpoint: endpoint(
            &settings.endpoint.value,
            settings.endpoint.base.then_some(name),
        )?,
        headers: headers(settings.headers)?,
        timeout: Duration::from_millis(settings.timeout_ms),
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

/// Validate and canonicalize request header names without exposing their values.
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

/// Decode the standard comma-separated, percent-encoded external header list.
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

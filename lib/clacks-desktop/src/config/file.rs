//! Typed optional startup-file values, before precedence resolution.

use serde::Deserialize;
use std::{collections::BTreeMap, path::PathBuf};

/// Optional file values; absent entries must not override other configuration tiers.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct FileConfig {
    /// Selected account workspace directory, relative to this file when not absolute.
    pub workspace: Option<PathBuf>,
    /// Local structured-output preferences.
    pub logs: LogFile,
    /// Explicit export opt-in and destination controls.
    pub telemetry: TelemetryFile,
}

/// File-supplied log controls.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct LogFile {
    /// Output directory override.
    pub directory: Option<PathBuf>,
    /// Tracing filter expression.
    pub filter: Option<String>,
    /// Whether to also emit console diagnostics.
    pub console: Option<bool>,
    /// Maximum matching rotated files retained.
    pub retained_files: Option<usize>,
    /// Bounded off-thread writer capacity in lines.
    pub buffered_lines: Option<usize>,
}

/// General export settings and signal-specific overrides.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct TelemetryFile {
    /// Explicit application opt-in, independent of endpoint configuration.
    pub enabled: Option<bool>,
    /// Generic endpoint base.
    pub endpoint: Option<String>,
    /// Secret-bearing generic request headers.
    pub headers: Option<BTreeMap<String, String>>,
    /// Request deadline in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Span destination overrides.
    pub traces: SignalFile,
    /// Measurement destination overrides.
    pub metrics: SignalFile,
    /// Event destination overrides.
    pub logs: SignalFile,
}

/// One signal's optional settings, without unrestricted Debug of credentials.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct SignalFile {
    /// Complete signal-specific endpoint.
    pub endpoint: Option<String>,
    /// Secret-bearing signal-specific request headers.
    pub headers: Option<BTreeMap<String, String>>,
    /// Signal-specific request deadline.
    pub timeout_ms: Option<u64>,
}

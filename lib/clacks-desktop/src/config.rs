//! Startup configuration resolved without mutating the process environment.

mod file;
pub(crate) mod telemetry;
#[cfg(test)]
mod tests;

use crate::workspace::{AppDirectories, relative_to};
use clap::Parser;
use file::FileConfig;
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf};

/// Explicit launch controls; omitted values preserve lower-priority settings.
#[derive(Parser)]
#[command(
    name = "clacks-log",
    version,
    about = "Clacks Log native desktop logger"
)]
pub(crate) struct Cli {
    /// Select an account workspace directory.
    #[arg(long)]
    pub workspace: Option<PathBuf>,
    /// Read this configuration file instead of the OS-conventional default.
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Override the structured log directory.
    #[arg(long)]
    pub log_directory: Option<PathBuf>,
    /// Set a tracing filter, such as info or clacks_desktop=debug.
    #[arg(long)]
    pub log_filter: Option<String>,
    /// Also write diagnostics to the console; use =false to disable.
    #[arg(long, action = clap::ArgAction::Set, num_args = 0..=1,
        default_missing_value = "true", require_equals = true, value_name = "BOOL")]
    pub console: Option<bool>,
    /// Maximum matching rotated log files retained.
    #[arg(long)]
    pub log_retained_files: Option<usize>,
    /// Bounded off-thread writer capacity, in lines.
    #[arg(long)]
    pub log_buffered_lines: Option<usize>,
    /// Enable optional telemetry export; use =false to disable.
    #[arg(long, action = clap::ArgAction::Set, num_args = 0..=1,
        default_missing_value = "true", require_equals = true, value_name = "BOOL")]
    pub otlp: Option<bool>,
    /// Set the generic OTLP HTTP endpoint base.
    #[arg(long)]
    pub otlp_endpoint: Option<String>,
    /// Set a complete trace-export endpoint.
    #[arg(long)]
    pub otlp_traces_endpoint: Option<String>,
    /// Set a complete metric-export endpoint.
    #[arg(long)]
    pub otlp_metrics_endpoint: Option<String>,
    /// Set a complete log-export endpoint.
    #[arg(long)]
    pub otlp_logs_endpoint: Option<String>,
    /// Request deadline in milliseconds for all exported signals.
    #[arg(long)]
    pub otlp_timeout_ms: Option<u64>,
}

/// Relevant environment values and platform roots captured at launch.
pub(crate) struct Environment {
    /// Relative command-line paths are based here.
    pub cwd: PathBuf,
    /// Relevant variables only; secret values are never formatted wholesale.
    pub values: BTreeMap<String, OsString>,
    /// OS-conventional configuration and local-data roots.
    pub directories: AppDirectories,
}

impl Environment {
    /// Capture supported environment settings without changing global state.
    pub fn capture() -> anyhow::Result<Self> {
        let values = std::env::vars_os()
            .filter_map(|(key, value)| {
                let key = key.to_str()?;
                (key == "RUST_LOG" || key.starts_with("OTEL_")).then(|| (key.to_owned(), value))
            })
            .collect();
        Ok(Self {
            cwd: std::env::current_dir()?,
            values,
            directories: AppDirectories::discover()?,
        })
    }

    /// Read one supported textual setting with value-free encoding diagnostics.
    fn value(&self, name: &str) -> anyhow::Result<Option<String>> {
        self.values
            .get(name)
            .map(|value| {
                value.to_str().map(str::to_owned).ok_or_else(|| {
                    anyhow::anyhow!("configuration variable {name} is not valid text")
                })
            })
            .transpose()
    }
}

/// Fully resolved startup settings, intentionally without secret-bearing Debug.
#[derive(Clone)]
pub(crate) struct DesktopConfig {
    /// Absolute selected workspace path, prior to canonicalization.
    pub workspace: PathBuf,
    /// Local diagnostic output settings.
    pub logs: LogConfig,
    /// Explicit export policy and resolved destinations.
    pub telemetry: telemetry::TelemetryConfig,
}

/// Fully resolved local writer/filter preferences.
#[derive(Clone)]
pub(crate) struct LogConfig {
    /// Actual directory for local diagnostic files.
    pub directory: PathBuf,
    /// Tracing filter applied at runtime.
    pub filter: tracing_subscriber::EnvFilter,
    /// Optional console output alongside local files.
    pub console: bool,
    /// Maximum matching files retained by daily rotation.
    pub retained_files: usize,
    /// Bounded nonblocking writer capacity in lines.
    pub buffered_lines: usize,
}

/// Resolve CLI/environment/file/default tiers without creating a workspace.
pub(crate) fn resolve(cli: Cli, environment: &Environment) -> anyhow::Result<DesktopConfig> {
    let explicit = cli.config.is_some();
    let config_path = cli
        .config
        .as_ref()
        .map(|path| relative_to(path, &environment.cwd))
        .unwrap_or_else(|| environment.directories.config.join("config.toml"));
    let file = match std::fs::read_to_string(&config_path) {
        Ok(text) => toml::from_str::<FileConfig>(&text).map_err(|error| {
            let line = error
                .span()
                .and_then(|span| text.get(..span.start))
                .map_or(1, |prefix| {
                    prefix
                        .bytes()
                        .filter(|byte| *byte == b'\n')
                        .count()
                        .saturating_add(1)
                });
            anyhow::anyhow!(
                "invalid startup configuration at line {line}; check supported keys and value types"
            )
        })?,
        Err(error) if !explicit && error.kind() == std::io::ErrorKind::NotFound => {
            FileConfig::default()
        }
        Err(error) => {
            return Err(anyhow::anyhow!(
                "startup configuration could not be read ({:?})",
                error.kind()
            ));
        }
    };
    let file_base = config_path.parent().unwrap_or(&environment.cwd);
    let workspace = cli
        .workspace
        .as_ref()
        .map(|path| relative_to(path, &environment.cwd))
        .or_else(|| {
            file.workspace
                .as_ref()
                .map(|path| relative_to(path, file_base))
        })
        .unwrap_or_else(|| environment.directories.data.join("workspace"));
    let logs = LogConfig {
        directory: cli
            .log_directory
            .as_ref()
            .map(|path| relative_to(path, &environment.cwd))
            .or_else(|| {
                file.logs
                    .directory
                    .as_ref()
                    .map(|path| relative_to(path, file_base))
            })
            .unwrap_or_else(|| workspace.join("logs")),
        filter: tracing_subscriber::EnvFilter::try_new(if let Some(value) = &cli.log_filter {
            value.clone()
        } else {
            environment
                .value("RUST_LOG")?
                .or(file.logs.filter.clone())
                .unwrap_or_else(|| "info".into())
        })
        .map_err(|_| anyhow::anyhow!("invalid log filter"))?,
        console: cli.console.or(file.logs.console).unwrap_or(false),
        retained_files: cli
            .log_retained_files
            .or(file.logs.retained_files)
            .unwrap_or(7),
        buffered_lines: cli
            .log_buffered_lines
            .or(file.logs.buffered_lines)
            .unwrap_or(8192),
    };
    anyhow::ensure!(
        logs.retained_files > 0 && logs.buffered_lines > 0,
        "log retention and writer capacity must be positive"
    );
    let telemetry = telemetry::resolve(&cli, environment, &file.telemetry)?;
    Ok(DesktopConfig {
        workspace,
        logs,
        telemetry,
    })
}

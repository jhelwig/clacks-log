//! Startup configuration resolved without mutating the process environment.

mod file;
mod sources;
pub(crate) mod telemetry;
#[cfg(test)]
mod tests;

use crate::workspace::{AppDirectories, relative_to};
use ::config::Config;
use clap::Parser;
use serde::Deserialize;
use std::path::PathBuf;

/// Explicit launch controls; omitted values preserve lower-priority settings.
#[derive(Parser)]
#[command(
    name = "clacks-log",
    version = crate::build_identity::BUILD_IDENTITY.version,
    long_version = crate::build_identity::BUILD_IDENTITY.to_string(),
    about = "Clacks Log native desktop logger"
)]
pub(crate) struct Cli {
    /// Select an account workspace directory.
    #[arg(long)]
    pub workspace: Option<PathBuf>,
    /// Read this configuration file instead of the OS-conventional default.
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Select an immutable geographic bundle instead of installed resources.
    #[arg(long)]
    pub geography_bundle: Option<PathBuf>,
    /// Expected manifest SHA-256 for an external bundle; defaults to packaged identity.
    #[arg(long)]
    pub geography_manifest_sha256: Option<String>,
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

/// OS startup context and the library-provided injectable environment source.
pub(crate) struct StartupContext {
    /// Relative command-line paths are based here.
    pub cwd: PathBuf,
    /// Environment settings source for this launch.
    pub environment: ::config::Environment,
    /// OS-conventional configuration and local-data roots.
    pub directories: AppDirectories,
}

impl StartupContext {
    /// Discover OS path conventions and select the normal process environment source.
    pub fn discover() -> anyhow::Result<Self> {
        Ok(Self {
            cwd: std::env::current_dir()?,
            environment: ::config::Environment::default(),
            directories: AppDirectories::discover()?,
        })
    }
}

/// Fully resolved startup settings, intentionally without secret-bearing Debug.
#[derive(Clone)]
pub(crate) struct DesktopConfig {
    /// Absolute selected workspace path, prior to canonicalization.
    pub workspace: PathBuf,
    /// Explicit geographic bundle, resolved relative to its supplying source.
    pub geography_bundle: Option<crate::geography::Selection>,
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

/// Log values after library source merging, before domain validation.
#[derive(Deserialize)]
#[serde(default)]
struct LogSettings {
    /// Tracing filter expression to validate.
    filter: String,
    /// Optional console reporting alongside local files.
    console: bool,
    /// Positive rotated-file retention limit.
    retained_files: usize,
    /// Positive nonblocking writer capacity.
    buffered_lines: usize,
}

impl Default for LogSettings {
    fn default() -> Self {
        Self {
            filter: "info".into(),
            console: false,
            retained_files: 7,
            buffered_lines: 8192,
        }
    }
}

/// Resolve standard sources and validate startup settings before opening a workspace.
pub(crate) fn resolve(cli: Cli, context: &StartupContext) -> anyhow::Result<DesktopConfig> {
    // Load the selected file before resolving values relative to its directory.
    let config_path = cli
        .config
        .as_ref()
        .map(|path| relative_to(path, &context.cwd))
        .unwrap_or_else(|| context.directories.config.join("config.toml"));
    let file = sources::file(&config_path, cli.config.is_some())?;

    // Paths retain the directory of the source that supplied them.
    let file_base = config_path.parent().unwrap_or(&context.cwd);
    let workspace = cli
        .workspace
        .as_ref()
        .map(|path| relative_to(path, &context.cwd))
        .or_else(|| {
            file.workspace
                .as_ref()
                .map(|path| relative_to(path, file_base))
        })
        .unwrap_or_else(|| context.directories.data.join("workspace"));

    let geography_bundle = cli
        .geography_bundle
        .as_ref()
        .map(|path| relative_to(path, &context.cwd))
        .or_else(|| {
            file.geography
                .bundle
                .as_ref()
                .map(|path| relative_to(path, file_base))
        });

    let expected_manifest = cli
        .geography_manifest_sha256
        .clone()
        .or(file.geography.manifest_sha256);
    anyhow::ensure!(
        geography_bundle.is_some() || expected_manifest.is_none(),
        "a geographic manifest digest requires an external bundle directory"
    );
    let geography_bundle = geography_bundle
        .map(|path| -> anyhow::Result<_> {
            let manifest_sha256 = match expected_manifest {
                Some(digest) => digest,
                None => crate::geography::runtime::packaged_manifest_sha256()?,
            };
            anyhow::ensure!(
                hex::decode(&manifest_sha256).is_ok_and(|bytes| bytes.len() == 32),
                "invalid geographic manifest SHA-256"
            );
            Ok(crate::geography::Selection {
                path,
                manifest_sha256,
            })
        })
        .transpose()?;

    let log_directory = cli
        .log_directory
        .as_ref()
        .map(|path| relative_to(path, &context.cwd))
        .or_else(|| {
            file.logs
                .directory
                .as_ref()
                .map(|path| relative_to(path, file_base))
        })
        .unwrap_or_else(|| workspace.join("logs"));

    // Logging sources are ordered from file to environment to command line.
    let rust = sources::environment(&context.environment, "RUST")?;

    let environment_logs = file::LogOptions {
        filter: sources::optional(&rust, "log")?,
        ..Default::default()
    };

    let command_logs = file::LogOptions {
        filter: cli.log_filter.clone(),
        console: cli.console,
        retained_files: cli.log_retained_files,
        buffered_lines: cli.log_buffered_lines,
        ..Default::default()
    };

    let mut builder = Config::builder();
    for source in [&file.logs, &environment_logs, &command_logs] {
        builder = builder
            .add_source(Config::try_from(source).map_err(|error| sources::invalid("logs", error))?);
    }

    let log_settings = builder
        .build()
        .and_then(Config::try_deserialize::<LogSettings>)
        .map_err(|error| sources::invalid("logs", error))?;

    // Validate selected values before constructing runtime controls.
    anyhow::ensure!(
        log_settings.retained_files > 0 && log_settings.buffered_lines > 0,
        "log retention and writer capacity must be positive"
    );

    let logs = LogConfig {
        directory: log_directory,
        filter: tracing_subscriber::EnvFilter::try_new(log_settings.filter)
            .map_err(|_| anyhow::anyhow!("invalid log filter"))?,
        console: log_settings.console,
        retained_files: log_settings.retained_files,
        buffered_lines: log_settings.buffered_lines,
    };

    let telemetry = telemetry::resolve(&cli, &context.environment, &file.telemetry)?;

    Ok(DesktopConfig {
        geography_bundle,
        workspace,
        logs,
        telemetry,
    })
}

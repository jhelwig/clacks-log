//! Process-wide standard tracing, structured output and optional OTLP export.

mod diagnostics;
mod export;
pub(crate) mod failure;
mod output;
#[cfg(test)]
pub(crate) mod tests;

use crate::{config::DesktopConfig, workspace::WorkspacePaths};
use opentelemetry::trace::TracerProvider as _;
use std::{
    io,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};
use tracing_appender::{
    non_blocking::{ErrorCounter, WorkerGuard},
    rolling::Rotation,
};
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};

/// Framework resources and degraded-status information owned by the desktop runtime.
pub(crate) struct TelemetryGuard {
    /// Background writers remain live until all producers/providers finish.
    writer: WorkerGuard,
    /// Standard appender's bounded-queue loss counter.
    loss: ErrorCounter,
    /// Sticky file I/O failure, independent of queue saturation.
    file_failure: Arc<AtomicBool>,
    /// SDK workers shut down before the file writer.
    providers: export::Providers,
    /// Safe startup degradation messages for the operator.
    diagnostics: Vec<String>,
}

impl TelemetryGuard {
    /// Share the persistent file-write failure indication with the native view.
    pub fn file_failure(&self) -> Arc<AtomicBool> {
        self.file_failure.clone()
    }

    /// Share the standard writer loss counter with the native diagnostic view.
    pub fn loss_counter(&self) -> ErrorCounter {
        self.loss.clone()
    }

    /// Borrow actionable startup degradation information without credential values.
    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }

    /// Read the standard writer's loss count for operational degradation reporting.
    pub fn dropped_lines(&self) -> usize {
        self.loss.dropped_lines()
    }

    /// Flush standard providers, then release the writer; call outside the UI thread.
    pub fn shutdown(self) -> anyhow::Result<()> {
        let mut failures = Vec::new();

        if let Some(provider) = self.providers.traces
            && let Err(error) = provider.shutdown_with_timeout(Duration::from_secs(5))
        {
            failures.push(format!("trace export: {error}"));
        }

        if let Some(provider) = self.providers.metrics
            && let Err(error) = provider.shutdown_with_timeout(Duration::from_secs(5))
        {
            failures.push(format!("metric export: {error}"));
        }

        if let Some(provider) = self.providers.logs
            && let Err(error) = provider.shutdown_with_timeout(Duration::from_secs(5))
        {
            failures.push(format!("log export: {error}"));
        }

        if !failures.is_empty() {
            tracing::warn!(
                event = "desktop.telemetry_flush_failed",
                failed_signals = failures.len()
            );
        }

        drop(self.writer);

        anyhow::ensure!(
            failures.is_empty(),
            "telemetry flush incomplete: {}",
            failures.join("; ")
        );
        Ok(())
    }
}

/// Configure ordinary framework layers and guarded resources for one process.
pub(crate) fn initialize(config: &DesktopConfig, paths: &WorkspacePaths) -> TelemetryGuard {
    let mut diagnostics = Vec::new();
    // Retention scans run before the appender creates its first file.
    let output = std::fs::create_dir_all(&paths.log_directory).and_then(|()| {
        tracing_appender::rolling::Builder::new()
            .rotation(Rotation::DAILY)
            .filename_prefix("clacks-log")
            .filename_suffix("jsonl")
            .max_log_files(config.logs.retained_files)
            .build(&paths.log_directory)
            .map_err(io::Error::other)
    });

    let fallback = output.is_err();
    let file_failure = Arc::new(AtomicBool::new(false));
    let builder = tracing_appender::non_blocking::NonBlockingBuilder::default()
        .buffered_lines_limit(config.logs.buffered_lines)
        .lossy(true);
    let (writer, guard) = match output {
        Ok(file) => builder.finish(output::FallbackWriter::new(
            file,
            io::stderr(),
            file_failure.clone(),
        )),
        Err(_) => {
            diagnostics.push("Local log files are unavailable; using stderr. Check the log directory and permissions".into());
            builder.finish(io::stderr())
        }
    };

    let loss = writer.error_counter();

    let providers = export::initialize(&config.telemetry, &mut diagnostics);
    if providers.traces.is_some() {
        opentelemetry::global::set_text_map_propagator(
            opentelemetry_sdk::propagation::TraceContextPropagator::new(),
        );
    }

    let json = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(writer)
        .with_current_span(true)
        .with_span_list(true);
    let console = (config.logs.console && !fallback).then(|| {
        tracing_subscriber::fmt::layer()
            .with_writer(io::stderr)
            .with_ansi(false)
    });

    let spans = providers.traces.as_ref().map(|provider| {
        tracing_opentelemetry::layer().with_tracer(provider.tracer("clacks_desktop"))
    });
    let logs = providers.logs.as_ref().map(|provider| {
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

    let subscriber = tracing_subscriber::registry()
        .with(config.logs.filter.clone())
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
        .with(json)
        .with(console)
        .with(spans)
        .with(logs);

    if subscriber.try_init().is_err() {
        diagnostics.push(
            "Desktop logging could not install its subscriber; an existing subscriber is active"
                .into(),
        );
    }

    TelemetryGuard {
        writer: guard,
        loss,
        file_failure,
        providers,
        diagnostics,
    }
}

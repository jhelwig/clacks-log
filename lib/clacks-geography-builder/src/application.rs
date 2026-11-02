//! Command-line composition for geographic acquisition, compilation and verification.

/// Metric name for build attempt duration.
pub(crate) const BUILD_DURATION_METRIC: &str = "clacks_geography_build_duration_seconds";

/// Metric name for build attempts by publication outcome.
pub(crate) const BUILD_RUNS_METRIC: &str = "clacks_geography_build_runs_total";

use crate::{
    recipe::read_recipe,
    source::{decode_sources, fetch_sources},
};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

#[cfg(test)]
mod tests;

/// Purpose-specific geographic-data preparation commands.
#[derive(Parser)]
#[command(about = "Build and verify offline Clacks geographic data", version)]
struct Arguments {
    /// Export diagnostics to the explicitly configured OpenTelemetry collector.
    #[arg(long, global = true)]
    otlp: bool,
    /// The explicitly requested source/data action.
    #[command(subcommand)]
    command: Command,
}

/// Commands whose effects are explicitly selected by the operator.
#[derive(Subcommand)]
enum Command {
    /// Inspect selected source identities, attributes and guards for recipe maintenance.
    Inspect {
        /// Pinned source/selection recipe.
        #[arg(long)]
        recipe: PathBuf,
        /// Already acquired digest-keyed source cache.
        #[arg(long)]
        cache: PathBuf,
    },
    /// Download missing pinned inputs and verify every reused cache entry.
    Fetch {
        /// Pinned source/selection recipe.
        #[arg(long)]
        recipe: PathBuf,
        /// Local digest-keyed source cache.
        #[arg(long)]
        cache: PathBuf,
    },
    /// Compile cached sources and atomically select a validated immutable data release.
    Build {
        /// Complete pinned compilation and acceptance recipe.
        #[arg(long)]
        recipe: PathBuf,
        /// Previously acquired source cache; no downloads occur.
        #[arg(long)]
        cache: PathBuf,
        /// Release store with an atomically replaced current pointer.
        #[arg(long)]
        output: PathBuf,
    },
    /// Validate an existing immutable bundle against an explicit recipe and its cases.
    Verify {
        /// Immutable bundle directory, not the current pointer.
        #[arg(long)]
        bundle: PathBuf,
        /// Complete pinned recipe supplying identity and acceptance expectations.
        #[arg(long)]
        recipe: PathBuf,
    },
}

/// Run the selected command with structured diagnostics and Ctrl-C cancellation.
pub fn run() -> Result<()> {
    let arguments = Arguments::parse();
    let filter = match std::env::var("RUST_LOG") {
        Ok(value) => tracing_subscriber::EnvFilter::try_new(value).context("parse RUST_LOG")?,
        Err(std::env::VarError::NotPresent) => tracing_subscriber::EnvFilter::new("info"),
        Err(error) => return Err(error.into()),
    };
    let telemetry = crate::telemetry::initialize(arguments.otlp, filter)?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&cancelled);
    ctrlc::set_handler(move || signal.store(true, Ordering::Relaxed))
        .context("install geographic cancellation handler")?;
    let result = execute(arguments.command, &cancelled);
    telemetry.shutdown();
    result
}

/// Compose one explicit maintenance workflow using the production library operations.
fn execute(command: Command, cancelled: &AtomicBool) -> Result<()> {
    match command {
        Command::Inspect { recipe, cache } => {
            let recipe = read_recipe(&recipe).inspect_err(|error| tracing::error!(category="recipe",error=%error,"Geographic recipe rejected"))?;
            let sources = decode_sources(&recipe, &cache, cancelled).inspect_err(|error| tracing::error!(release_id=%recipe.release_id,category="source",error=%error,"Geographic inspection failed"))?;
            let summaries:Vec<_>=sources.features.iter().map(|feature|serde_json::json!({"identity":feature.identity,"feature_sha256":feature.feature_sha256,"properties":feature.properties})).collect();
            serde_json::to_writer_pretty(std::io::stdout().lock(), &summaries).inspect_err(|error| tracing::error!(category="output",error=%error,"Geographic inspection output failed"))?;
        }
        Command::Fetch { recipe, cache } => {
            let recipe = read_recipe(&recipe).inspect_err(|error| tracing::error!(category="recipe",error=%error,"Geographic recipe rejected"))?;
            let report = fetch_sources(&recipe, &cache, cancelled).inspect_err(|error| tracing::error!(release_id=%recipe.release_id,category="source",error=%error,"Geographic acquisition failed"))?;
            println!(
                "Sources: {} installed, {} verified and reused",
                report.installed, report.reused
            );
        }
        Command::Build {
            recipe,
            cache,
            output,
        } => {
            let started = Instant::now();
            let result = read_recipe(&recipe).inspect_err(|error| tracing::error!(category="recipe",error=%error,"Geographic recipe rejected")).and_then(|recipe| {
                crate::publish::build_release(&recipe, &cache, &output, cancelled)
                    .inspect_err(|error| {
                        if error.is::<crate::source::BuildCancelled>() {
                            tracing::info!(name:"geography.build_cancelled",release_id=%recipe.release_id,"Geographic build cancelled before publication");
                        } else {
                            tracing::error!(name:"geography.bundle_rejected",release_id=%recipe.release_id,category="build_failed",error=?error,"Geographic bundle rejected");
                        }
                    })
            });
            let outcome = match &result {
                Ok(report) => report.outcome.diagnostic_name(),
                Err(error) if error.is::<crate::source::BuildCancelled>() => "cancelled",
                Err(_) => "failed",
            };
            metrics::histogram!(BUILD_DURATION_METRIC,"outcome"=>outcome)
                .record(started.elapsed().as_secs_f64());
            metrics::counter!(BUILD_RUNS_METRIC,"outcome"=>outcome).increment(1);
            let report = result?;
            println!(
                "{:?}: {} ({} acceptance cases; {} reviewed omissions)",
                report.outcome, report.release_id, report.checked_cases, report.reviewed_omissions
            );
        }
        Command::Verify { bundle, recipe } => {
            let recipe = read_recipe(&recipe).inspect_err(|error| tracing::error!(category="recipe",error=%error,"Geographic recipe rejected"))?;
            let mut bundle = clacks_geography::artifact::read::open_bundle(&bundle).inspect_err(|error| tracing::error!(release_id=%recipe.release_id,category="artifact",error=%error,"Geographic verification failed"))?;
            let report = crate::compile::verify_bundle(&mut bundle, &recipe).inspect_err(|error| tracing::error!(release_id=%recipe.release_id,category="acceptance",error=%error,"Geographic verification failed"))?;
            println!(
                "Verified {}: {} acceptance cases; {} reviewed omissions",
                recipe.release_id, report.checked_cases, report.reviewed_omissions
            );
        }
    }
    Ok(())
}

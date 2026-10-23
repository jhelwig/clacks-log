//! Command-line composition for explicitly acquiring pinned geographic sources.

use crate::{
    recipe::read_recipe,
    source::{decode_sources, fetch_sources},
};
use anyhow::Context;
use clap::{Parser, Subcommand};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// Purpose-specific geographic-data preparation commands.
#[derive(Parser)]
#[command(about = "Build and verify offline Clacks geographic data", version)]
struct Arguments {
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
}

/// Run the geographic-data command selected by ordinary command-line arguments.
///
/// Installs cancellation in application composition. Source libraries never
/// register signal handlers or perform implicit downloads during other actions.
pub fn run() -> anyhow::Result<()> {
    let arguments = Arguments::parse();
    let cancelled = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&cancelled);
    ctrlc::set_handler(move || signal.store(true, Ordering::Relaxed))
        .context("install geographic cancellation handler")?;
    match arguments.command {
        Command::Inspect { recipe, cache } => {
            let recipe = read_recipe(&recipe)?;
            let sources = decode_sources(&recipe, &cache, &cancelled)?;
            let summaries:Vec<_>=sources.features.iter().map(|feature|serde_json::json!({"identity":feature.identity,"feature_sha256":feature.feature_sha256,"properties":feature.properties})).collect();
            serde_json::to_writer_pretty(std::io::stdout().lock(), &summaries)?;
        }
        Command::Fetch { recipe, cache } => {
            let recipe = read_recipe(&recipe)?;
            let report = fetch_sources(&recipe, &cache, &cancelled)?;
            println!(
                "Sources: {} installed, {} verified and reused",
                report.installed, report.reused
            );
        }
    }
    Ok(())
}

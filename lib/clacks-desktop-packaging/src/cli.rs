//! Packaging command composition; child processes retain literal arguments.
use clap::{Parser, Subcommand};
use std::{fs, path::PathBuf};

/// Explicit desktop packaging operations; no generic command dispatcher.
#[derive(Parser)]
#[command(version, about)]
struct Arguments {
    /// Operation to perform.
    #[command(subcommand)]
    command: Operation,
}
/// Supported preparation operations.
#[derive(Subcommand)]
enum Operation {
    /// Regenerate the browser publication copy of implemented sync contracts.
    SyncContracts {
        /// Generated JSON destination.
        #[arg(long)]
        output: PathBuf,
    },
    /// Reopen an assembled geographic payload against the build-provided identity.
    GeographyInspect {
        /// Expected manifest digest from the common geographic build output.
        #[arg(long)]
        manifest_sha256: String,
        /// Installed or assembled bundle directory.
        #[arg(long)]
        bundle: PathBuf,
        /// Verified evidence JSON destination.
        #[arg(long)]
        output: PathBuf,
    },

    /// Reuse existing successful Linux/web verification for an exact commit.
    Gate {
        /// Repository owning the existing Rust workflow.
        #[arg(long)]
        repository: String,
        /// Exact commit that will be packaged.
        #[arg(long)]
        revision: String,
        /// Successful CI evidence destination.
        #[arg(long)]
        output: PathBuf,
    },
    /// Validate source-owned Cargo version and optional release tag.
    Prepare {
        /// Native package platform; source/tag validation is shared.
        #[arg(long, value_enum)]
        platform: crate::metadata::PackagePlatform,
        /// Standard cargo metadata JSON file.
        #[arg(long)]
        metadata: PathBuf,
        /// Exact source commit.
        #[arg(long)]
        revision: String,
        /// Release tag; omit for development builds.
        #[arg(long)]
        tag: Option<String>,
        /// Validated identity JSON destination.
        #[arg(long)]
        output: PathBuf,
    },
}
/// Install process-local diagnostics and execute one operation.
pub(crate) fn run() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .try_init()
        .map_err(|e| anyhow::anyhow!("cannot initialize packaging diagnostics: {e}"))?;
    let args = Arguments::parse();
    let result = match args.command {
        Operation::SyncContracts { output } => crate::sync_contracts::write(&output),
        Operation::GeographyInspect {
            manifest_sha256,
            bundle,
            output,
        } => crate::geography::inspect(&manifest_sha256, &bundle)
            .and_then(|e| crate::write_json(&output, &e)),

        Operation::Gate {
            repository,
            revision,
            output,
        } => crate::ci::gate(&repository, &revision).and_then(|e| crate::write_json(&output, &e)),
        Operation::Prepare {
            platform,
            metadata,
            revision,
            tag,
            output,
        } => {
            let input = serde_json::from_slice(&fs::read(metadata)?)?;
            crate::metadata::prepare(&input, &revision, tag.as_deref(), platform)
                .and_then(|e| crate::write_json(&output, &e))
        }
    };
    if let Err(ref error) = result {
        tracing::error!(%error,"packaging operation failed");
    }
    result
}

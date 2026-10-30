//! Operator key commands with distinct credentials and safe interruption.

use super::{
    account::{self, AccountRotationRequest},
    config::KeySettings,
    references,
    report::MaintenanceEnd,
    rewrap::{self, RewrapRequest},
};
use crate::secrets::keys::WrappingKeys;
use clacks_model::ids::AccountId;
use clap::{Args, Subcommand};
use sqlx::{Connection, PgConnection, postgres::PgConnectOptions};
use std::io::Write;
use std::{
    path::{Path, PathBuf},
    str::FromStr,
};
use tokio_util::sync::CancellationToken;

/// Inputs shared by operations that decrypt and update key envelopes.
#[derive(Args)]
pub(crate) struct RotationOptions {
    /// Server configuration file; only wrapping-key settings are loaded.
    #[arg(long)]
    config: PathBuf,
    /// File containing a maintenance-role PostgreSQL URL (direct or session pooled).
    #[arg(long)]
    database_url_file: PathBuf,
    /// Maximum rows committed in one transaction.
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u16).range(1..=100))]
    batch_size: u16,
}

/// Discoverable generation, metadata and migration operations.
#[derive(Subcommand)]
pub(crate) enum KeysCommand {
    /// Generate a protected raw 32-byte wrapping key without replacing an existing file.
    Generate {
        /// Destination in an existing private directory.
        #[arg(long)]
        output: PathBuf,
    },
    /// Show current database references without reading wrapping-key files.
    Status {
        /// File containing the maintenance-role PostgreSQL URL.
        #[arg(long)]
        database_url_file: PathBuf,
        /// Restrict the report to one existing account; keyless accounts are included.
        #[arg(long)]
        account: Option<AccountId>,
    },
    /// Rewrap every source account key after deploying and activating the target key.
    Rewrap {
        #[command(flatten)]
        options: RotationOptions,
        /// Deployment-key ID currently referenced by source envelopes.
        #[arg(long)]
        from: String,
        /// Configured active deployment-key ID.
        #[arg(long)]
        to: String,
    },
    /// Activate the next account key and migrate current encrypted references.
    RotateAccount {
        #[command(flatten)]
        options: RotationOptions,
        /// Existing account whose encryption key will change.
        #[arg(long)]
        account: AccountId,
        /// Explicit source version; reuse on retry.
        #[arg(long)]
        from_version: i64,
        /// Explicit next version; reuse on retry.
        #[arg(long)]
        to_version: i64,
    },
}

/// Open an owned maintenance session with a bounded database lock wait.
async fn connection(path: &Path) -> anyhow::Result<PgConnection> {
    let options = PgConnectOptions::from_str(&crate::config::secret(path)?)?
        .options([("lock_timeout", "5s")]);
    tracing::info!(
        event = "key.maintenance.connection",
        database_user = options.get_username(),
        "Opening maintenance database session"
    );
    Ok(PgConnection::connect_with(&options).await?)
}

/// Wait for the platform's ordinary interruption/termination signals.
async fn interrupted() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result,
            _ = terminate.recv() => Ok(()),
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await
    }
}

/// Execute key maintenance without constructing web state or loading unrelated secrets.
pub(crate) async fn run(command: &KeysCommand) -> anyhow::Result<MaintenanceEnd> {
    let stop = CancellationToken::new();

    let operation = async {
        if matches!(
            command,
            KeysCommand::Rewrap { .. } | KeysCommand::RotateAccount { .. }
        ) {
            let arguments: Vec<_> = std::env::args_os().collect();
            let invocation =
                shlex::bytes::try_join(arguments.iter().map(|arg| arg.as_encoded_bytes()))?;
            let mut stderr = std::io::stderr().lock();
            stderr.write_all(b"Retry (POSIX shell): ")?;
            stderr.write_all(&invocation)?;
            writeln!(stderr)?;
        }

        match command {
            KeysCommand::Generate { output } => {
                super::generate(output)?;
                println!("Wrapping key created at {}", output.display());
            }
            KeysCommand::Status {
                database_url_file,
                account,
            } => {
                let mut connection = connection(database_url_file).await?;
                let status = references::status(&mut connection, *account).await?;
                println!("{}", serde_json::to_string_pretty(&status)?);
                connection.close().await?;
            }
            KeysCommand::Rewrap { options, from, to } => {
                let settings = KeySettings::load(&options.config)?;
                let keys = WrappingKeys::load(&settings.wrapping_keys)?;
                let request = RewrapRequest::new(from.clone(), to.clone(), options.batch_size)?;
                let report = rewrap::run(
                    connection(&options.database_url_file).await?,
                    &keys,
                    &request,
                    &stop,
                )
                .await?;
                println!("{}", serde_json::to_string_pretty(&report)?);
                return Ok(report.end);
            }
            KeysCommand::RotateAccount {
                options,
                account,
                from_version,
                to_version,
            } => {
                let settings = KeySettings::load(&options.config)?;
                let keys = WrappingKeys::load(&settings.wrapping_keys)?;
                let request = AccountRotationRequest::new(
                    *account,
                    *from_version,
                    *to_version,
                    options.batch_size,
                )?;
                let report = account::rotate(
                    connection(&options.database_url_file).await?,
                    &keys,
                    &request,
                    &stop,
                )
                .await?;
                println!("{}", serde_json::to_string_pretty(&report)?);
                return Ok(report.end);
            }
        }
        Ok(MaintenanceEnd::Complete)
    };
    tokio::pin!(operation);
    let result: anyhow::Result<MaintenanceEnd> = tokio::select! {
        result = &mut operation => result,
        signal = interrupted() => {
            match signal {
                Ok(()) => {
                    stop.cancel();
                    operation.await
                }
                Err(error) => Err(error.into()),
            }
        }
    };

    if let Err(error) = &result {
        match command {
            KeysCommand::RotateAccount {
                account,
                from_version,
                to_version,
                ..
            } => {
                let progress = error.downcast_ref::<super::report::AccountRotationError>();
                let migrated = progress
                    .map(|failure| failure.migrated_receipts)
                    .unwrap_or_default();
                tracing::error!(event = "account.key.rotation.failed", account_id = %account,
                    from_version, to_version,
                    migrated_receipts = migrated, error = %format_args!("{error:#}"),
                    "Account-key migration failed; retry the same source/target");
            }
            KeysCommand::Rewrap { from, to, .. } => {
                let progress = error.downcast_ref::<super::report::RewrapError>();
                let migrated = progress
                    .map(|failure| failure.migrated_account_keys)
                    .unwrap_or_default();
                let account = progress.and_then(|failure| failure.account);
                let key_version = progress.and_then(|failure| failure.key_version);
                tracing::error!(event = "deployment.keys.rewrap.failed", from, to,
                    account_id = ?account, key_version, migrated_account_keys = migrated,
                    error = %format_args!("{error:#}"),
                    "Wrapping-key migration failed; retry the same source/target");
            }
            KeysCommand::Generate { .. } => {
                tracing::error!(event = "deployment.key.generate.failed", error = %format_args!("{error:#}"), "Wrapping-key generation failed");
            }
            KeysCommand::Status { account, .. } => {
                tracing::error!(event = "key.status.failed", account_id = ?account, error = %format_args!("{error:#}"), "Key reference lookup failed");
            }
        }
    }
    result
}

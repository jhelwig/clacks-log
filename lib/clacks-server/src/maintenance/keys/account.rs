//! Online account-key cutover and resumable bounded migration.

use super::{
    migration, references,
    report::{AccountRotationError, AccountRotationReport, MaintenanceEnd},
};
use crate::secrets::keys::{self, WrappingKeys};
use anyhow::Context;
use clacks_model::ids::AccountId;
use sqlx::{Connection, Either, PgConnection, postgres::PgAdvisoryLock};
use tokio_util::sync::CancellationToken;

/// Explicit source and next version make retries unambiguous.
pub(crate) struct AccountRotationRequest {
    /// Owning account.
    account: AccountId,
    /// Explicit source account-key version.
    from_version: i64,
    /// Adjacent positive target account-key version.
    to_version: i64,
    /// Checked maximum encrypted rows in one transaction.
    batch_size: super::MaintenanceBatchSize,
}

impl AccountRotationRequest {
    /// Select an explicit adjacent positive version transition for resumable rotation.
    pub(crate) fn new(
        account: AccountId,
        from_version: i64,
        to_version: i64,
        batch_size: u16,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            from_version > 0 && from_version.checked_add(1) == Some(to_version),
            "target must be the next positive account-key version"
        );
        Ok(Self {
            account,
            from_version,
            to_version,
            batch_size: batch_size.try_into()?,
        })
    }

    /// Return the owning account.
    pub(crate) fn account(&self) -> AccountId {
        self.account
    }

    /// Return the explicit source version retained across retries.
    pub(crate) fn source_version(&self) -> i64 {
        self.from_version
    }

    /// Return the explicit adjacent target version retained across retries.
    pub(crate) fn target_version(&self) -> i64 {
        self.to_version
    }

    /// Return the checked maximum rows per transaction.
    pub(crate) fn batch_size(&self) -> super::MaintenanceBatchSize {
        self.batch_size
    }
}

/// Establish or resume the requested target in one account-locked transaction.
///
/// The caller holds the maintenance guard. Historical keys are retained; a new
/// rotation is refused until older encrypted references have been migrated.
pub(crate) async fn cutover(
    connection: &mut PgConnection,
    wrapping: &WrappingKeys,
    request: &AccountRotationRequest,
) -> anyhow::Result<()> {
    let mut tx = connection.begin().await?;
    sqlx::query_file!("sql/account_lock.sql", request.account.to_string())
        .fetch_one(&mut *tx)
        .await?;
    let rows = sqlx::query_file!(
        "sql/secrets/rotation/state.sql",
        request.account.to_string()
    )
    .fetch_all(&mut *tx)
    .await?;
    let active = rows
        .iter()
        .find(|row| row.active)
        .context("account has no active key to rotate")?;
    let source = rows
        .iter()
        .find(|row| row.key_version == request.from_version)
        .context("source account key does not exist")?;
    if active.key_version == request.to_version && !source.active {
        tx.commit().await?;
        return Ok(());
    }
    anyhow::ensure!(
        active.key_version == request.from_version
            && !rows.iter().any(|row| row.key_version == request.to_version),
        "account-key state does not match requested source/target"
    );
    for row in rows.iter().filter(|row| !row.active) {
        let counts = references::count(&mut tx, request.account, row.key_version).await?;
        anyhow::ensure!(
            counts == 0,
            "older inactive key {} still has encrypted references; resume its rotation",
            row.key_version
        );
    }
    // Authenticate the source before publishing a new key when recovery material is missing.
    keys::load_version(&mut *tx, wrapping, request.account, request.from_version).await?;
    sqlx::query_file!(
        "sql/secrets/rotation/deactivate.sql",
        request.account.to_string(),
        request.from_version
    )
    .execute(&mut *tx)
    .await?;
    keys::create_in(&mut tx, wrapping, request.account, request.to_version).await?;
    tx.commit().await?;
    tracing::info!(event = "account.key.cutover", account_id = %request.account, from_version = request.from_version, to_version = request.to_version, "Account encryption key activated");
    Ok(())
}

/// Rotate under a session-owned maintenance guard, preserving committed batches.
///
/// Retry the same source/target after interruption. A busy maintenance command
/// or incompatible key state is an error, never an automatically chosen target.
#[tracing::instrument(skip_all, name = "account.key.rotate", level = "info", fields(account_id = %request.account, from_version = request.from_version, to_version = request.to_version))]
pub(crate) async fn rotate(
    connection: PgConnection,
    wrapping: &WrappingKeys,
    request: &AccountRotationRequest,
    stop: &CancellationToken,
) -> anyhow::Result<AccountRotationReport> {
    let lock = PgAdvisoryLock::new(super::MAINTENANCE_LOCK);
    let Either::Left(mut guard) = lock.try_acquire(connection).await? else {
        anyhow::bail!("key maintenance is busy; retry the same command");
    };
    let mut migrated_values = 0;

    let result: anyhow::Result<AccountRotationReport> = async {
        if !stop.is_cancelled() {
            cutover(guard.as_mut(), wrapping, request).await?;
        }
            while !stop.is_cancelled() {
                let changed = migration::batch(guard.as_mut(), wrapping, request)
                    .await
                    .with_context(|| {
                        format!(
                            "encrypted-value batch for account {}, source key version {}, target key version {} failed",
                            request.account, request.from_version, request.to_version
                        )
                    })?;
                migrated_values += changed;
                if changed == 0 {
                    break;
                }
            }
        let mut tx = guard.as_mut().begin().await?;
        sqlx::query_file!("sql/account_lock.sql", request.account.to_string())
            .fetch_one(&mut *tx).await?;
        let remaining = references::count(&mut tx, request.account, request.from_version).await?;
        let end = if stop.is_cancelled() {
            MaintenanceEnd::Interrupted
        } else {
            anyhow::ensure!(
                remaining == 0,
                "source references remain after migration"
            );
            MaintenanceEnd::Complete
        };
        tx.commit().await?;

        tracing::info!(event = "account.key.rotation", account_id = %request.account, from_version = request.from_version, to_version = request.to_version, migrated_values, remaining_values = remaining, outcome = ?end, "Account-key migration stopped");
        Ok(AccountRotationReport {
            account: request.account, from_version: request.from_version,
            to_version: request.to_version, end,
            migrated_values,
            remaining_values: remaining,
        })
    }.await;

    // Release explicitly even on an operation error; the owned connection also closes on drop.
    let released = guard.release_now().await;
    result
        .and_then(|report| {
            released?;
            Ok(report)
        })
        .map_err(|source| {
            AccountRotationError {
                account: request.account,
                from_version: request.from_version,
                to_version: request.to_version,
                migrated_values,
                source,
            }
            .into()
        })
}

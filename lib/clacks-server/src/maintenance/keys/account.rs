//! Online account-key cutover and resumable bounded migration.

use super::{
    migration::{self, CiphertextFamily},
    references,
    report::{AccountRotationError, AccountRotationReport, EncryptedCounts, MaintenanceEnd},
};
use crate::secrets::keys::{self, WrappingKeys};
use anyhow::Context;
use clacks_model::ids::AccountId;
use sqlx::{Connection, Either, PgConnection, postgres::PgAdvisoryLock};
use std::num::NonZeroU16;
use tokio_util::sync::CancellationToken;

/// Explicit source and next version make retries unambiguous.
pub(crate) struct AccountRotationRequest {
    /// Owning account.
    pub account: AccountId,
    /// Currently active source, or inactive source of an interrupted rotation.
    pub from_version: i64,
    /// Exact next version; never inferred from current state on retry.
    pub to_version: i64,
    /// Maximum rows committed in one batch, in the range 1–100.
    pub batch_size: NonZeroU16,
}

impl AccountRotationRequest {
    /// Refuse impossible transitions before acquiring or changing database state.
    pub(super) fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.from_version > 0 && self.from_version.checked_add(1) == Some(self.to_version),
            "target must be the next positive account-key version"
        );
        anyhow::ensure!(self.batch_size.get() <= 100, "batch size must be 1–100");
        Ok(())
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
    request.validate()?;
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
            counts.credentials == 0 && counts.receipts == 0,
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
    request.validate()?;
    let lock = PgAdvisoryLock::new(super::MAINTENANCE_LOCK);
    let Either::Left(mut guard) = lock.try_acquire(connection).await? else {
        anyhow::bail!("key maintenance is busy; retry the same command");
    };
    let mut migrated = EncryptedCounts::default();

    let mut current_family = None;
    let result: anyhow::Result<AccountRotationReport> = async {
        if !stop.is_cancelled() {
            cutover(guard.as_mut(), wrapping, request).await?;
        }
        for family in [CiphertextFamily::Credential, CiphertextFamily::Receipt] {
            current_family = Some(family);
            while !stop.is_cancelled() {
                let changed = migration::batch(guard.as_mut(), wrapping, request, family)
                    .await
                    .with_context(|| {
                        format!(
                            "{family:?} ciphertext batch for account {}, source key version {}, target key version {} failed",
                            request.account, request.from_version, request.to_version
                        )
                    })?;
                match family {
                    CiphertextFamily::Credential => migrated.credentials += changed,
                    CiphertextFamily::Receipt => migrated.receipts += changed,
                }
                if changed == 0 {
                    break;
                }
            }
        }
        current_family = None;
        let mut tx = guard.as_mut().begin().await?;
        sqlx::query_file!("sql/account_lock.sql", request.account.to_string())
            .fetch_one(&mut *tx).await?;
        let remaining = references::count(&mut tx, request.account, request.from_version).await?;
        let end = if stop.is_cancelled() {
            MaintenanceEnd::Interrupted
        } else {
            anyhow::ensure!(
                remaining.credentials == 0 && remaining.receipts == 0,
                "source references remain after migration"
            );
            MaintenanceEnd::Complete
        };
        tx.commit().await?;

        tracing::info!(event = "account.key.rotation", account_id = %request.account, from_version = request.from_version, to_version = request.to_version, credentials = migrated.credentials, receipts = migrated.receipts, remaining_credentials = remaining.credentials, remaining_receipts = remaining.receipts, outcome = ?end, "Account-key migration stopped");
        Ok(AccountRotationReport {
            account: request.account, from_version: request.from_version,
            to_version: request.to_version, end,
            migrated: EncryptedCounts {
                credentials: migrated.credentials, receipts: migrated.receipts,
            },
            remaining,
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
                migrated,
                family: current_family,
                source,
            }
            .into()
        })
}

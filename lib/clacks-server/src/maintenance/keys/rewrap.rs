//! Resumable deployment-key migration of active and inactive account envelopes.

use super::report::{MaintenanceEnd, RewrapReport};
use crate::secrets::keys::{self, WrappingKeys};
use anyhow::Context;
use sqlx::{Connection, Either, PgConnection, postgres::PgAdvisoryLock};
use std::num::NonZeroU16;
use tokio_util::sync::CancellationToken;

/// Explicit deployment-key migration parameters; retries use the same IDs.
pub(crate) struct RewrapRequest {
    /// Wrapping key currently referenced by source envelopes.
    pub from: String,
    /// Configured active deployment key.
    pub to: String,
    /// Maximum key envelopes committed for one account per transaction.
    pub batch_size: NonZeroU16,
}

/// Rewrap all source references while preserving account-key bytes and versions.
///
/// Operators must first distribute both keys and drain source-key writers. This
/// command reports current database references, not backup retention or rollout state.
#[tracing::instrument(skip_all, name = "deployment.keys.rewrap", level = "info", fields(from = %request.from, to = %request.to))]
pub(crate) async fn run(
    connection: PgConnection,
    wrapping: &WrappingKeys,
    request: &RewrapRequest,
    stop: &CancellationToken,
) -> anyhow::Result<RewrapReport> {
    anyhow::ensure!(
        request.from != request.to && request.to == wrapping.active,
        "rewrap target must be the distinct configured active key"
    );
    anyhow::ensure!(request.batch_size.get() <= 100, "batch size must be 1–100");
    let lock = PgAdvisoryLock::new(super::MAINTENANCE_LOCK);
    let Either::Left(mut guard) = lock.try_acquire(connection).await? else {
        anyhow::bail!("key maintenance is busy; retry the same command");
    };
    let mut migrated = 0u64;
    let mut current_account = None;
    let mut current_version = None;

    let result: anyhow::Result<RewrapReport> = async {
        while !stop.is_cancelled() {
            let Some(owner) = sqlx::query_file!(
                "sql/secrets/rotation/wrapping_accounts.sql", request.from
            ).fetch_optional(guard.as_mut()).await? else {
                break;
            };
            let account = owner.account_id.parse()?;
            current_account = Some(account);

            let mut tx = guard.as_mut().begin().await?;
            sqlx::query_file!("sql/account_lock.sql", owner.account_id)
                .fetch_one(&mut *tx).await?;
            let rows = sqlx::query_file!(
                "sql/secrets/rotation/wrapping_batch.sql",
                owner.account_id, request.from, i64::from(request.batch_size.get())
            ).fetch_all(&mut *tx).await?;
            let count = u64::try_from(rows.len())?;
            for row in rows {
                current_version = Some(row.key_version);
                keys::rewrap_in(&mut tx, wrapping, account, row.key_version)
                    .await
                    .with_context(|| {
                        format!(
                            "account {account}, key version {} could not be rewrapped",
                            row.key_version
                        )
                    })?;
            }
            tx.commit().await?;
            current_version = None;
            current_account = None;

            migrated += count;
            metrics::counter!("clacks_account_keys_rewrapped_total").increment(count);
        }

        let remaining = sqlx::query_file!(
            "sql/secrets/rotation/wrapping_count.sql", request.from
        ).fetch_one(guard.as_mut()).await?.remaining.try_into()?;
        let end = if stop.is_cancelled() {
            MaintenanceEnd::Interrupted
        } else {
            MaintenanceEnd::Complete
        };
        tracing::info!(event = "deployment.keys.rewrapped", from = %request.from, to = %request.to, migrated_account_keys = migrated, remaining_account_keys = remaining, outcome = ?end, "Wrapping-key migration stopped");
        Ok(RewrapReport {
            from: request.from.clone(), to: request.to.clone(), end,
            migrated_account_keys: migrated, remaining_account_keys: remaining,
        })
    }.await;

    let released = guard.release_now().await;
    result
        .and_then(|report| {
            released?;
            Ok(report)
        })
        .map_err(|source| {
            super::report::RewrapError {
                from: request.from.clone(),
                to: request.to.clone(),
                migrated_account_keys: migrated,
                account: current_account,
                key_version: current_version,
                source,
            }
            .into()
        })
}

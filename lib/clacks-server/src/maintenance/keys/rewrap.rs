//! Resumable deployment-key migration of active and inactive account envelopes.

use super::report::{MaintenanceEnd, RewrapReport};
use crate::secrets::keys::{self, WrappingKeys};
use anyhow::Context;
use sqlx::{Connection, Either, PgConnection, postgres::PgAdvisoryLock};
use tokio_util::sync::CancellationToken;

/// Explicit deployment-key migration parameters; retries use the same IDs.
pub(crate) struct RewrapRequest {
    /// Exact source deployment-key ID.
    from: String,
    /// Distinct target deployment-key ID.
    to: String,
    /// Checked maximum account keys per transaction.
    batch_size: super::MaintenanceBatchSize,
}

impl RewrapRequest {
    /// Select distinct nonempty key IDs and a bounded transaction size.
    pub(crate) fn new(from: String, to: String, batch_size: u16) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !from.is_empty() && !to.is_empty() && from != to,
            "rewrap requires distinct nonempty source and target key IDs"
        );
        Ok(Self {
            from,
            to,
            batch_size: batch_size.try_into()?,
        })
    }
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
        request.to == wrapping.active,
        "rewrap target must be the distinct configured active key"
    );
    let source_reference = crate::secrets::envelope::wrapping_reference(&request.from);
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
                "sql/secrets/rotation/wrapping_accounts.sql", source_reference
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
                owner.account_id, source_reference, i64::from(request.batch_size.get())
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
            "sql/secrets/rotation/wrapping_count.sql", source_reference
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

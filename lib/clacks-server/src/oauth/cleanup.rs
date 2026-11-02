//! Bounded removal of expired issuer state, preserving active enrollments.

use sqlx::PgPool;
use std::time::SystemTime;
use time::OffsetDateTime;

/// Reclaim expired credentials and abandoned provisional grants without idle expiry.
pub(crate) async fn sweep(pool: &PgPool, now: SystemTime) -> anyhow::Result<u64> {
    let now = OffsetDateTime::from(now);
    let accounts = sqlx::query_file!("sql/oauth/cleanup_accounts.sql", now)
        .fetch_all(pool)
        .await?;
    let mut removed = 0;
    for account in accounts {
        let mut account_removed = 0;
        let mut transaction = pool.begin().await?;
        sqlx::query_file!("sql/account_lock.sql", account.account_id)
            .fetch_one(&mut *transaction)
            .await?;
        account_removed += sqlx::query_file!(
            "sql/oauth/cleanup_desktop_authorization_codes.sql",
            account.account_id,
            now
        )
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        account_removed += sqlx::query_file!(
            "sql/oauth/cleanup_desktop_code_approvals.sql",
            account.account_id,
            now
        )
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        account_removed += sqlx::query_file!(
            "sql/oauth/cleanup_desktop_access_tokens.sql",
            account.account_id,
            now
        )
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        account_removed += sqlx::query_file!(
            "sql/oauth/cleanup_desktop_refresh_tokens.sql",
            account.account_id,
            now
        )
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        account_removed += sqlx::query_file!(
            "sql/oauth/cleanup_desktop_grants.sql",
            account.account_id,
            now
        )
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        transaction.commit().await?;
        metrics::counter!(crate::oauth::OAUTH_EXPIRED_ROWS_TOTAL_METRIC).increment(account_removed);
        removed += account_removed;
    }
    let count = sqlx::query_file!("sql/oauth/cleanup_replay.sql", now)
        .execute(pool)
        .await?
        .rows_affected();
    metrics::counter!(crate::oauth::OAUTH_EXPIRED_ROWS_TOTAL_METRIC).increment(count);
    removed += count;
    let count = sqlx::query_file!("sql/oauth/cleanup_resource_replay.sql", now)
        .execute(pool)
        .await?
        .rows_affected();
    metrics::counter!(crate::oauth::OAUTH_EXPIRED_ROWS_TOTAL_METRIC).increment(count);
    removed += count;
    let count = sqlx::query_file!("sql/oauth/cleanup_barriers.sql", now)
        .execute(pool)
        .await?
        .rows_affected();
    metrics::counter!(crate::oauth::OAUTH_EXPIRED_ROWS_TOTAL_METRIC).increment(count);
    removed += count;
    Ok(removed)
}

/// Reclaim expired state periodically while the production server owns this task.
pub(crate) async fn run(pool: PgPool, stop: tokio_util::sync::CancellationToken) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            biased;
            _ = stop.cancelled() => break,
            _ = interval.tick() => {}
        }
        match sweep(&pool, SystemTime::now()).await {
            Ok(removed) if removed > 0 => {
                tracing::debug!(
                    event = "oauth.cleanup.completed",
                    removed,
                    "Expired issuer rows reclaimed"
                );
            }
            Ok(_) => {}
            Err(error) => {
                tracing::error!(event="oauth.cleanup.failed", reason=%error, "Issuer cleanup failed; next scheduled pass will retry")
            }
        }
    }
}

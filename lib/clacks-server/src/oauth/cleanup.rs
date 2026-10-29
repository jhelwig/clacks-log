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
        let mut transaction = pool.begin().await?;
        sqlx::query_file!("sql/account_lock.sql", account.account_id)
            .fetch_one(&mut *transaction)
            .await?;
        removed += sqlx::query_file!(
            "sql/oauth/cleanup_desktop_authorization_codes.sql",
            account.account_id,
            now
        )
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        removed += sqlx::query_file!(
            "sql/oauth/cleanup_desktop_code_approvals.sql",
            account.account_id,
            now
        )
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        removed += sqlx::query_file!(
            "sql/oauth/cleanup_desktop_access_tokens.sql",
            account.account_id,
            now
        )
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        removed += sqlx::query_file!(
            "sql/oauth/cleanup_desktop_refresh_tokens.sql",
            account.account_id,
            now
        )
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        removed += sqlx::query_file!(
            "sql/oauth/cleanup_desktop_grants.sql",
            account.account_id,
            now
        )
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        transaction.commit().await?;
    }
    removed += sqlx::query_file!("sql/oauth/cleanup_replay.sql", now)
        .execute(pool)
        .await?
        .rows_affected();
    removed += sqlx::query_file!("sql/oauth/cleanup_resource_replay.sql", now)
        .execute(pool)
        .await?
        .rows_affected();
    removed += sqlx::query_file!("sql/oauth/cleanup_barriers.sql", now)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(removed)
}

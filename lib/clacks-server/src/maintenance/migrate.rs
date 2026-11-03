//! Release-managed PostgreSQL schema and existing-account synchronization initialization.

use clacks_model::ids::AccountId;
use sqlx::PgPool;

/// Apply this release's SQLx migrations using an authorized migration pool.
///
/// Existing accounts that predate synchronization receive the same initial
/// metadata and accepted definitions as new accounts. Each account is locked and
/// checked again before initialization; existing synchronization state is preserved.
#[tracing::instrument(skip_all, name = "server.database.migrate")]
pub async fn run(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;

    let accounts = sqlx::query_file!("sql/metadata/uninitialized_accounts.sql")
        .fetch_all(pool)
        .await?;

    for row in accounts {
        let owner: AccountId = row.account_id.parse()?;
        let mut transaction = pool.begin().await?;
        sqlx::query_file!("sql/account_lock.sql", row.account_id)
            .fetch_one(&mut *transaction)
            .await?;
        if sqlx::query_file!("sql/metadata/state_select.sql", row.account_id)
            .fetch_optional(&mut *transaction)
            .await?
            .is_some()
        {
            continue;
        }
        crate::store::synchronization::initialize(&mut transaction, owner).await?;
        transaction.commit().await?;
        tracing::info!(
            event = "account.synchronization.initialized",
            account_id = %owner,
            "Initialized synchronization for an existing account"
        );
    }
    Ok(())
}

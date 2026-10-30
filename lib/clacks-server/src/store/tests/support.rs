//! Shared account and transaction setup for issuer/key-maintenance tests.

use clacks_model::ids::AccountId;
use sqlx::PgConnection;

/// Acquire Clacks's account lock in an existing test transaction.
pub(super) async fn lock(connection: &mut PgConnection, account: AccountId) -> anyhow::Result<()> {
    sqlx::query_file!("sql/account_lock.sql", account.to_string())
        .fetch_one(connection)
        .await?;
    Ok(())
}

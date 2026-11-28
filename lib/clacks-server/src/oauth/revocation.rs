//! Grant revocation barriers and account-owned credential removal.

use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use oauth_as::store::RevocationWindow;
use sqlx::{PgConnection, PgPool};

/// Persist the issuer barrier before deleting credentials that it protects.
async fn barrier(
    connection: &mut PgConnection,
    scope: &str,
    target: &str,
    window: RevocationWindow,
) -> anyhow::Result<()> {
    anyhow::ensure!(!target.is_empty(), "revocation target is empty");
    let recorded = Timestamp::try_from(window.recorded_at)?;
    let until = Timestamp::try_from(window.until)?;
    sqlx::query_file!(
        "sql/oauth/barrier_put.sql",
        scope,
        target,
        recorded.to_sqlx() as _,
        until.to_sqlx() as _
    )
    .execute(connection)
    .await?;
    Ok(())
}

/// Revoke one grant without closing its separately managed synchronization actor.
pub(crate) async fn family_in(
    connection: &mut PgConnection,
    family: &str,
    window: RevocationWindow,
) -> anyhow::Result<u64> {
    barrier(connection, "family", family, window).await?;
    sqlx::query_file!("sql/oauth/grant_revoke.sql", family)
        .execute(&mut *connection)
        .await?;
    let access = sqlx::query_file!("sql/oauth/family_access_delete.sql", family)
        .execute(&mut *connection)
        .await?
        .rows_affected();
    let refresh = sqlx::query_file!("sql/oauth/family_refresh_delete.sql", family)
        .execute(connection)
        .await?
        .rows_affected();
    Ok(access + refresh)
}

/// Serialize a library grant revocation with account-owned credential issuance.
pub(crate) async fn family(
    pool: &PgPool,
    family: &str,
    window: RevocationWindow,
) -> anyhow::Result<u64> {
    let owner = sqlx::query_file!("sql/oauth/family_owner.sql", family)
        .fetch_optional(pool)
        .await?;
    let mut transaction = pool.begin().await?;
    if let Some(owner) = owner {
        sqlx::query_file!("sql/account_lock.sql", owner.account_id)
            .fetch_one(&mut *transaction)
            .await?;
    }
    let removed = family_in(&mut transaction, family, window).await?;
    transaction.commit().await?;
    Ok(removed)
}

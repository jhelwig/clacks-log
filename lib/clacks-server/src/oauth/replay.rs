//! Cross-instance proof replay exclusion using a single database claim.

use jiff_sqlx::ToSqlx;
use sqlx::PgPool;
use std::time::SystemTime;

/// Claim a previously unseen library-namespaced proof identifier.
pub(crate) async fn claim(pool: &PgPool, id: &str, expires: SystemTime) -> anyhow::Result<bool> {
    let expires = jiff::Timestamp::try_from(expires)?;
    Ok(
        sqlx::query_file!("sql/oauth/replay_claim.sql", id, expires.to_sqlx() as _)
            .fetch_optional(pool)
            .await?
            .is_some(),
    )
}

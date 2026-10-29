//! Cross-instance proof replay exclusion using a single database claim.

use sqlx::PgPool;
use std::time::SystemTime;

/// Claim a previously unseen library-namespaced proof identifier.
pub(crate) async fn claim(pool: &PgPool, id: &str, expires: SystemTime) -> anyhow::Result<bool> {
    let expires = time::OffsetDateTime::from(expires);
    Ok(sqlx::query_file!("sql/oauth/replay_claim.sql", id, expires)
        .fetch_optional(pool)
        .await?
        .is_some())
}

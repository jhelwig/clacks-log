//! Release-managed PostgreSQL schema installation.

use sqlx::PgPool;

/// Apply this release's SQLx migrations using an authorized migration pool.
pub async fn run(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

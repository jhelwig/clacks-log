//! PostgreSQL account operations and their transaction boundaries.

use sqlx::PgPool;

pub(crate) mod accounts;
mod administrators;
pub mod identities;
pub(crate) mod logins;

#[cfg(test)]
mod tests;

/// Concrete PostgreSQL access shared by account requests and maintenance work.
#[derive(Clone)]
pub struct AccountStore {
    pool: PgPool,
}

impl AccountStore {
    /// Use an existing role-appropriate pool; this does not migrate its database.
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Check account-schema usability under the runtime role without changing data.
    pub(crate) async fn ping(&self) -> anyhow::Result<()> {
        sqlx::query_file!("sql/readiness.sql")
            .fetch_optional(&self.pool)
            .await?;
        Ok(())
    }

    /// Release runtime connections after admitted requests have drained.
    pub(crate) async fn close(&self) {
        self.pool.close().await;
    }
}

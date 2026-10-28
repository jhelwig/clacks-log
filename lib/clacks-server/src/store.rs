//! PostgreSQL account operations and their transaction boundaries.

use sqlx::PgPool;

pub(crate) mod accounts;
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
}

impl AccountStore {
    /// Check database connectivity for runtime readiness without changing account data.
    pub(crate) async fn ping(&self) -> anyhow::Result<()> {
        use sqlx::Connection;
        self.pool.acquire().await?.ping().await?;
        Ok(())
    }

    /// Release runtime connections after admitted requests have drained.
    pub(crate) async fn close(&self) {
        self.pool.close().await;
    }
}

mod administrators;
pub mod identities;

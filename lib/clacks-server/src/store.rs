//! PostgreSQL account operations and their transaction boundaries.

use sqlx::PgPool;

pub(crate) mod accounts;
mod administrators;
pub(crate) mod diagnostics;
pub(crate) mod directory;
pub(crate) mod discovery;
pub mod identities;
pub(crate) mod logins;
pub(crate) mod metadata;
pub(crate) mod payload;
pub(crate) mod retention;
pub(crate) mod rows;

pub(crate) mod synchronization;

#[cfg(test)]
pub(crate) mod tests;

/// Concrete PostgreSQL access shared by account requests and maintenance work.
#[derive(Clone)]
pub struct AccountStore {
    pub(crate) pool: PgPool,
    /// Settled account changes wake connected clients; durable state remains authoritative.
    pub(crate) changes: tokio::sync::broadcast::Sender<clacks_model::ids::AccountId>,
}

impl AccountStore {
    /// Use an existing role-appropriate pool; this does not migrate its database.
    pub(crate) fn new(pool: PgPool) -> Self {
        let (changes, _) = tokio::sync::broadcast::channel(128);
        Self { pool, changes }
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

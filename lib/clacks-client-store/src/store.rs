//! Opening and connection policy for one account's local database.

use clacks_model::ids::AccountId;
use sqlx::{
    ConnectOptions, Connection, Sqlite, SqlitePool, Transaction,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};
use std::{path::Path, time::Duration};

mod creation;
mod opening;

/// Bounded connection and contention settings for a desktop workspace.
#[derive(Clone, Debug)]
pub struct StoreSettings {
    /// Maximum simultaneous pool connections; must be positive.
    pub max_connections: u32,
    /// SQLite lock wait before returning a busy error.
    pub busy_timeout: Duration,
}

impl Default for StoreSettings {
    fn default() -> Self {
        Self {
            max_connections: 4,
            busy_timeout: Duration::from_secs(5),
        }
    }
}

/// Durable local storage restricted to one native account.
///
/// Share this handle across application workflows and close it before a closed-file backup.
/// Network calls and user interaction belong outside its transactions.
#[derive(Clone, Debug)]
pub struct ClientStore {
    /// Account qualification enforced by all operations.
    pub(crate) account: AccountId,
    /// Ordinary SQLx pool; transactions keep their own checked-out connection.
    pub(crate) pool: SqlitePool,
}

/// Construct connection options without enabling SQL statement or bound-value logging.
pub(crate) fn options(path: &Path, settings: &StoreSettings) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(path)
        .busy_timeout(settings.busy_timeout)
        .disable_statement_logging()
}

impl ClientStore {
    /// Read the native account qualification used by this store's operations.
    ///
    /// Use this when constructing record keys after opening a discovered workspace.
    pub fn account_id(&self) -> AccountId {
        self.account
    }

    /// Build the normal pool only after startup safety checks and migrations succeed.
    async fn connect(
        path: &Path,
        account: AccountId,
        settings: &StoreSettings,
    ) -> anyhow::Result<Self> {
        let options = options(path, settings)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Full);
        let pool = SqlitePoolOptions::new()
            .max_connections(settings.max_connections)
            // SQLx's standard acquisition ping drains queued rollback work first.
            // SQLx 0.9 documents that is_in_transaction can retain depth after
            // automatic rollback. Its SQLite worker decrements depth only when
            // ROLLBACK succeeds; SQLITE_FULL can already have ended the engine
            // transaction. Discard that stale connection instead of reusing it.
            .before_acquire(|connection, _| {
                Box::pin(async move { Ok(!connection.is_in_transaction()) })
            })
            .connect_with(options)
            .await?;
        Ok(Self { account, pool })
    }

    /// Release pooled connections, waiting for outstanding operations to finish.
    pub async fn close(&self) -> anyhow::Result<()> {
        self.pool.close().await;
        Ok(())
    }

    /// Begin a tracked write transaction that acquires SQLite's reserved writer lock immediately.
    pub(crate) async fn write(&self) -> anyhow::Result<Transaction<'_, Sqlite>> {
        Ok(self.pool.begin_with("BEGIN IMMEDIATE").await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn startup_uses_wal_full_and_foreign_keys() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let account = ulid::Ulid::from(1_u128).to_string().parse()?;
        let store = ClientStore::create(
            &dir.path().join("workspace.sqlite"),
            account,
            StoreSettings::default(),
        )
        .await?;
        let mut connections = Vec::new();
        for _ in 0..4 {
            let mut connection = store.pool.acquire().await?;
            let row = sqlx::query_file!("sql/opening/settings.sql")
                .fetch_one(&mut *connection)
                .await?;
            assert_eq!(row.journal_mode, "wal");
            let synchronous = sqlx::query_file!("sql/opening/synchronous.sql")
                .fetch_one(&mut *connection)
                .await?;
            assert_eq!(synchronous.synchronous, Some(2));
            let foreign_keys = sqlx::query_file!("sql/opening/foreign_keys.sql")
                .fetch_one(&mut *connection)
                .await?;
            assert_eq!(foreign_keys.foreign_keys, Some(1));
            connections.push(connection);
        }
        drop(connections);
        store.close().await?;
        Ok(())
    }
}

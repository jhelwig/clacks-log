//! Opening and connection policy for one account's local database.

use crate::migration;
use clacks_model::ids::AccountId;
use sqlx::{
    ConnectOptions, Connection, Sqlite, SqlitePool, Transaction,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};
use std::{path::Path, time::Duration};

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
    /// Exclusively create a new local-only workspace without overwriting any existing file.
    #[tracing::instrument(skip_all, name = "client_store.create", fields(account_id = %account))]
    pub async fn create(
        path: &Path,
        account: AccountId,
        settings: StoreSettings,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            settings.max_connections > 0,
            "maximum connections must be positive"
        );
        let path = migration::creation_path(path)?;
        let _file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let mut connection = options(&path, &settings)
            .foreign_keys(true)
            .synchronous(SqliteSynchronous::Full)
            .connect()
            .await?;
        migration::apply(&mut connection, 0).await?;
        let account_text = account.to_string();
        sqlx::query_file!("sql/workspace/create.sql", account_text)
            .execute(&mut connection)
            .await?;
        connection.close().await?;
        let store = Self::connect(&path, account, &settings).await?;
        tracing::info!(event = "workspace.created", account_id = %account);
        Ok(store)
    }

    /// Open an existing workspace after read-only identity, account and compatibility checks.
    #[tracing::instrument(skip_all, name = "client_store.open", fields(account_id = %account))]
    pub async fn open(
        path: &Path,
        account: AccountId,
        settings: StoreSettings,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            settings.max_connections > 0,
            "maximum connections must be positive"
        );
        let path = path.canonicalize()?;
        let previous = migration::preflight(&path, account, &settings).await?;
        let mut connection = options(&path, &settings)
            .foreign_keys(true)
            .synchronous(SqliteSynchronous::Full)
            .connect()
            .await?;
        migration::apply(&mut connection, previous).await?;
        connection.close().await?;
        Self::connect(&path, account, &settings).await
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
            // SQLite can auto-roll back on SQLITE_FULL while the driver retains
            // transaction depth; discard that connection instead of reusing it.
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
            let checkpoint = sqlx::query_file!("sql/opening/checkpoint_settings.sql")
                .fetch_one(&mut *connection)
                .await?;
            assert_eq!(checkpoint.wal_autocheckpoint, Some(1000));
            connections.push(connection);
        }
        drop(connections);
        store.close().await?;
        Ok(())
    }
}

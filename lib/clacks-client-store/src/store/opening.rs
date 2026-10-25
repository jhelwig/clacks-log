//! Existing workspace opening with one read-only compatibility/account preflight.

use super::{ClientStore, StoreSettings, options};
use crate::migration;
use clacks_model::ids::AccountId;
use sqlx::{ConnectOptions, Connection, sqlite::SqliteSynchronous};
use std::path::Path;

impl ClientStore {
    /// Open an existing workspace after read-only compatibility and account checks.
    ///
    /// Use when enrollment or another caller already supplies the expected account.
    /// Account mismatch and unsupported history reject the open before writes.
    #[tracing::instrument(skip_all, name = "client_store.open", fields(account_id = %account))]
    pub async fn open(
        path: &Path,
        account: AccountId,
        settings: StoreSettings,
    ) -> anyhow::Result<Self> {
        open(path, Some(account), settings).await
    }

    /// Open an existing workspace using its stored native account identity.
    ///
    /// Use for local startup when only the selected workspace path is known. Account
    /// discovery shares the compatibility preflight; it grants no server access.
    #[tracing::instrument(skip_all, name = "client_store.open_discovered")]
    pub async fn open_discovered(path: &Path, settings: StoreSettings) -> anyhow::Result<Self> {
        open(path, None, settings).await
    }
}

/// Reuse the same preflight and migration path for known and discovered accounts.
async fn open(
    path: &Path,
    expected: Option<AccountId>,
    settings: StoreSettings,
) -> anyhow::Result<ClientStore> {
    anyhow::ensure!(
        settings.max_connections > 0,
        "maximum connections must be positive"
    );

    let path = path.canonicalize()?;
    let previous = migration::preflight(&path, expected, &settings).await?;

    let mut connection = options(&path, &settings)
        .foreign_keys(true)
        .synchronous(SqliteSynchronous::Full)
        .connect()
        .await?;

    let migrated = migration::apply(&mut connection, previous.previous).await;
    let closed = connection.close().await;
    migrated?;
    closed?;

    ClientStore::connect(&path, previous.account, &settings).await
}

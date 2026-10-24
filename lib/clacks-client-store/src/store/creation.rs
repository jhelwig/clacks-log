//! Exclusive file creation and atomic native account/definition initialization.

use super::{ClientStore, StoreSettings, options};
use crate::{migration, save::save_in};
use clacks_model::{defaults::AccountDefinitions, ids::AccountId};
use clacks_sync::record::{RecordKey, RecordState, RecordValue};
use sqlx::{ConnectOptions, Connection, SqliteConnection, sqlite::SqliteSynchronous};
use std::path::Path;

#[cfg(test)]
mod tests;

impl ClientStore {
    /// Exclusively create an empty local-only workspace without overwriting files.
    ///
    /// Use for callers that provide their own account provisioning or later enroll
    /// an empty workspace. This does not install bundled application definitions.
    #[tracing::instrument(skip_all, name = "client_store.create", fields(account_id = %account))]
    pub async fn create(
        path: &Path,
        account: AccountId,
        settings: StoreSettings,
    ) -> anyhow::Result<Self> {
        create(path, account, None, settings).await
    }

    /// Create a local account and supplied initial definitions in one transaction.
    ///
    /// Use for standalone account provisioning. Definitions are validated before
    /// creating the file and installed as ordinary unpublished native records.
    /// Failure never replaces files or leaves a partially initialized account.
    #[tracing::instrument(skip_all, name = "client_store.create_with_definitions", fields(account_id = %account))]
    pub async fn create_with_definitions(
        path: &Path,
        account: AccountId,
        definitions: &AccountDefinitions,
        settings: StoreSettings,
    ) -> anyhow::Result<Self> {
        definitions.validate()?;
        create(path, account, Some(definitions), settings).await
    }
}

/// Initialize the account row and all supplied ordinary working records atomically.
async fn initialize(
    connection: &mut SqliteConnection,
    account: AccountId,
    definitions: Option<&AccountDefinitions>,
) -> anyhow::Result<()> {
    let mut transaction = connection.begin_with("BEGIN IMMEDIATE").await?;
    let account_text = account.to_string();
    sqlx::query_file!("sql/workspace/create.sql", account_text)
        .execute(&mut *transaction)
        .await?;
    if let Some(definitions) = definitions {
        for value in definitions
            .output_profiles
            .iter()
            .map(|profile| RecordValue::OutputProfile(Box::new(profile.clone())))
            .chain(
                definitions
                    .groups
                    .iter()
                    .map(|group| RecordValue::Group(Box::new(group.clone()))),
            )
        {
            let key = RecordKey {
                account,
                id: value.id(),
            };
            save_in(&mut transaction, key, None, RecordState::Live(value)).await?;
        }
    }
    transaction.commit().await?;
    Ok(())
}

/// Create schema first, then install the complete initial account before exposing a pool.
async fn create(
    path: &Path,
    account: AccountId,
    definitions: Option<&AccountDefinitions>,
    settings: StoreSettings,
) -> anyhow::Result<ClientStore> {
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
    initialize(&mut connection, account, definitions).await?;
    connection.close().await?;
    let store = ClientStore::connect(&path, account, &settings).await?;
    let group_count = definitions.map_or(0, |definitions| definitions.groups.len());
    let output_profile_count =
        definitions.map_or(0, |definitions| definitions.output_profiles.len());
    tracing::info!(event = "workspace.created", account_id = %account, group_count, output_profile_count);
    Ok(store)
}

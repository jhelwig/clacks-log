//! Read-only startup checks and standard SQLx migrations.

use crate::{
    error::StoreError,
    store::{StoreSettings, options},
};
use clacks_model::ids::AccountId;
use sqlx::{ConnectOptions, Connection, SqliteConnection, migrate::Migrator};
use std::path::{Path, PathBuf};

/// Embedded forward-only production schema, with each migration committed independently.
static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

/// Resolve a creation target without following an existing database file.
pub(crate) fn creation_path(path: &Path) -> anyhow::Result<PathBuf> {
    let name = path
        .file_name()
        .ok_or_else(|| StoreError::InvalidDatabase {
            detail: "missing file name".into(),
        })?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    Ok(parent.canonicalize()?.join(name))
}

/// Check existing history and account in one read-only snapshot before permitting writes.
///
/// Returns the latest applied migration version for startup reporting. Missing,
/// incompatible or modified history and mismatched accounts reject the open.
pub(crate) async fn preflight(
    path: &Path,
    account: AccountId,
    settings: &StoreSettings,
) -> anyhow::Result<i64> {
    let mut connection = options(path, settings).read_only(true).connect().await?;
    let mut transaction = connection.begin().await?;
    let supported = MIGRATIONS
        .iter()
        .map(|migration| migration.version)
        .max()
        .unwrap_or(0);
    let history = sqlx::query_file!("sql/opening/migration_history.sql")
        .fetch_all(&mut *transaction)
        .await?;
    anyhow::ensure!(
        !history.is_empty(),
        StoreError::InvalidDatabase {
            detail: "migration history is empty".into()
        }
    );
    let mut latest = 0;
    for applied in history {
        let version = applied.version.ok_or_else(|| StoreError::InvalidDatabase {
            detail: "migration version is null".into(),
        })?;
        latest = latest.max(version);
        if version > supported {
            return Err(StoreError::IncompatibleSchema {
                found: version,
                supported,
            }
            .into());
        }
        let migration = MIGRATIONS
            .iter()
            .find(|migration| migration.version == version)
            .ok_or_else(|| StoreError::InvalidDatabase {
                detail: "unrecognized migration".into(),
            })?;
        anyhow::ensure!(
            applied.success && applied.checksum.as_slice() == migration.checksum.as_ref(),
            StoreError::InvalidDatabase {
                detail: "migration history is dirty or modified".into()
            }
        );
    }
    let actual = crate::identity::load(&mut transaction)
        .await?
        .ok_or_else(|| StoreError::InvalidDatabase {
            detail: "account row is missing".into(),
        })?
        .account;
    if actual != account {
        return Err(StoreError::WrongAccount {
            expected: account,
            actual,
        }
        .into());
    }
    transaction.commit().await?;
    connection.close().await?;
    Ok(latest)
}

/// Apply standard SQLx migrations without wrapping their individual commits in an outer transaction.
///
/// Supply the previously observed migration version, or zero for a new database,
/// to report a successful schema upgrade from the actual migration history.
#[tracing::instrument(skip_all, name = "client_store.schema.migrate", level = "debug")]
pub(crate) async fn apply(connection: &mut SqliteConnection, previous: i64) -> anyhow::Result<()> {
    MIGRATIONS.run(&mut *connection).await?;
    let current = sqlx::query_file!("sql/opening/migration_version.sql")
        .fetch_one(connection)
        .await?
        .version
        .unwrap_or(0);
    if previous != current {
        tracing::info!(
            event = "schema.upgraded",
            previous_schema = previous,
            current_schema = current
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "migration_tests.rs"]
mod tests;

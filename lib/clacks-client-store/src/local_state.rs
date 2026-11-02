//! Shared key/value storage for preferences and nonsecret workspace-local workflow metadata.

use sqlx::SqliteConnection;

/// Read a setting by key, returning `None` when it has not been stored.
pub(crate) async fn get_in(
    connection: &mut SqliteConnection,
    key: &str,
) -> anyhow::Result<Option<String>> {
    Ok(sqlx::query_file!("sql/local_state/get.sql", key)
        .fetch_optional(connection)
        .await?
        .map(|row| row.value))
}

/// Save one setting without changing other keys or committing the caller's work.
pub(crate) async fn set_in(
    connection: &mut SqliteConnection,
    key: &str,
    value: &str,
) -> anyhow::Result<()> {
    sqlx::query_file!("sql/local_state/set.sql", key, value)
        .execute(connection)
        .await?;
    Ok(())
}

use super::*;
use crate::store::ClientStore;
use sqlx::Executor;

#[tokio::test]
async fn startup_failure_keeps_prior_committed_migrations_and_retries_from_them()
-> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("workspace.sqlite");
    let account: AccountId = ulid::Ulid::from(1_u128).to_string().parse()?;
    let settings = StoreSettings::default();
    let mut connection = options(&file, &settings)
        .create_if_missing(true)
        .foreign_keys(true)
        .connect()
        .await?;
    let older = Migrator::with_migrations(
        MIGRATIONS
            .iter()
            .filter(|migration| migration.version <= 4)
            .cloned()
            .collect(),
    );
    older.run(&mut connection).await?;
    let account_text = account.to_string();
    sqlx::query_file!("sql/workspace/create.sql", account_text)
        .execute(&mut connection)
        .await?;
    // This test-owned obstruction makes the genuine sixth migration fail after
    // the genuine fifth migration commits; production migration files stay intact.
    connection
        .execute("CREATE TABLE operating_value (obstruction INTEGER)")
        .await?;
    connection.close().await?;
    assert!(
        ClientStore::open(&file, account, settings.clone())
            .await
            .is_err()
    );
    let mut connection = options(&file, &settings).connect().await?;
    assert_eq!(
        sqlx::query_file!("sql/opening/migration_version.sql")
            .fetch_one(&mut connection)
            .await?
            .version,
        Some(5)
    );
    let history = sqlx::query_file!("sql/opening/migration_history.sql")
        .fetch_all(&mut connection)
        .await?;
    assert_eq!(history.len(), 5);
    assert!(history.iter().all(|row| row.success));
    connection.execute("DROP TABLE operating_value").await?;
    connection.close().await?;
    let store = ClientStore::open(&file, account, settings.clone()).await?;
    store.close().await?;
    let mut connection = options(&file, &settings).connect().await?;
    assert_eq!(
        sqlx::query_file!("sql/opening/migration_version.sql")
            .fetch_one(&mut connection)
            .await?
            .version,
        Some(8)
    );
    connection
        .execute("UPDATE _sqlx_migrations SET checksum = X'00' WHERE version = 1")
        .await?;
    connection.close().await?;
    let before = std::fs::read(&file)?;
    let error = ClientStore::open(&file, account, settings)
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<StoreError>(),
        Some(StoreError::InvalidDatabase { .. })
    ));
    assert_eq!(std::fs::read(&file)?, before);
    Ok(())
}

#[tokio::test]
async fn creation_leaves_the_sqlite_application_id_unassigned() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("workspace.sqlite");
    let account: AccountId = ulid::Ulid::from(1_u128).to_string().parse()?;
    let settings = StoreSettings::default();
    let store = ClientStore::create(&file, account, settings.clone()).await?;
    store.close().await?;
    let mut connection = options(&file, &settings).read_only(true).connect().await?;
    let row = sqlx::query_file!("tests/sql/application_id.sql")
        .fetch_one(&mut connection)
        .await?;
    assert_eq!(row.application_id, Some(0));
    connection.close().await?;
    Ok(())
}

#[tokio::test]
async fn opening_requires_existing_history_and_account_without_an_application_id()
-> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("workspace.sqlite");
    let account: AccountId = ulid::Ulid::from(1_u128).to_string().parse()?;
    let settings = StoreSettings::default();
    let store = ClientStore::create(&file, account, settings.clone()).await?;
    store.close().await?;
    let mut connection = options(&file, &settings).connect().await?;
    sqlx::query_file!("tests/sql/clear_application_id.sql")
        .execute(&mut connection)
        .await?;
    connection.close().await?;
    let reopened = ClientStore::open(&file, account, settings.clone()).await?;
    reopened.close().await?;
    let mut connection = options(&file, &settings).connect().await?;
    connection.execute("DROP TABLE _sqlx_migrations").await?;
    connection.close().await?;
    let before = std::fs::read(&file)?;
    assert!(ClientStore::open(&file, account, settings).await.is_err());
    assert_eq!(std::fs::read(&file)?, before);
    Ok(())
}

#[tokio::test]
async fn opening_uses_migration_history_without_duplicate_version_metadata() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("workspace.sqlite");
    let account: AccountId = ulid::Ulid::from(1_u128).to_string().parse()?;
    let settings = StoreSettings::default();
    let store = ClientStore::create(&file, account, settings.clone()).await?;
    store.close().await?;
    let mut connection = options(&file, &settings).connect().await?;
    sqlx::query_file!("tests/sql/drop_legacy_schema.sql")
        .execute(&mut connection)
        .await?;
    sqlx::query_file!("tests/sql/unrelated_user_version.sql")
        .execute(&mut connection)
        .await?;
    connection.close().await?;
    let reopened = ClientStore::open(&file, account, settings).await?;
    reopened.close().await?;
    Ok(())
}

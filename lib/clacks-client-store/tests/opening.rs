use clacks_client_store::{
    identity::{self, EnrollmentBinding},
    store::{ClientStore, StoreSettings},
};
use clacks_model::ids::AccountId;
use tempfile::tempdir;

fn account(n: u128) -> AccountId {
    ulid::Ulid::from(n).to_string().parse().unwrap()
}

#[tokio::test]
async fn create_never_overwrites() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, account(1), StoreSettings::default()).await?;
    store.close().await?;
    let before = std::fs::read(&file)?;
    assert!(
        ClientStore::create(&file, account(1), StoreSettings::default())
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&file)?, before);
    Ok(())
}

#[tokio::test]
async fn open_rejects_wrong_account_or_unrelated_file() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, account(1), StoreSettings::default()).await?;
    store.close().await?;
    let before = std::fs::read(&file)?;
    assert!(
        ClientStore::open(&file, account(2), StoreSettings::default())
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&file)?, before);
    let unrelated = dir.path().join("other.sqlite");
    std::fs::write(&unrelated, b"not a database")?;
    assert!(
        ClientStore::open(&unrelated, account(1), StoreSettings::default())
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&unrelated)?, b"not a database");
    assert!(
        ClientStore::open(
            &dir.path().join("missing"),
            account(1),
            StoreSettings::default()
        )
        .await
        .is_err()
    );
    assert!(!dir.path().join("missing").exists());
    Ok(())
}

#[tokio::test]
async fn rejected_modified_history_leaves_the_database_untouched() -> anyhow::Result<()> {
    use sqlx::{ConnectOptions, Connection, sqlite::SqliteConnectOptions};

    let directory = tempdir()?;
    let file = directory.path().join("workspace.sqlite");
    ClientStore::create(&file, account(1), StoreSettings::default())
        .await?
        .close()
        .await?;
    let mut connection = SqliteConnectOptions::new()
        .filename(&file)
        .connect()
        .await?;
    sqlx::query_file!("tests/sql/modified_migration_history.sql")
        .execute(&mut connection)
        .await?;
    connection.close().await?;

    let before = std::fs::read(&file)?;
    let error = ClientStore::open(&file, account(1), StoreSettings::default())
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref(),
        Some(clacks_client_store::error::StoreError::InvalidDatabase { .. })
    ));
    assert_eq!(std::fs::read(&file)?, before);
    Ok(())
}

#[tokio::test]
async fn rejected_missing_history_leaves_the_database_untouched() -> anyhow::Result<()> {
    use sqlx::{ConnectOptions, Connection, sqlite::SqliteConnectOptions};

    let directory = tempdir()?;
    let file = directory.path().join("workspace.sqlite");
    ClientStore::create(&file, account(1), StoreSettings::default())
        .await?
        .close()
        .await?;
    let mut connection = SqliteConnectOptions::new()
        .filename(&file)
        .connect()
        .await?;
    sqlx::query_file!("tests/sql/missing_migration_history.sql")
        .execute(&mut connection)
        .await?;
    connection.close().await?;

    let before = std::fs::read(&file)?;
    assert!(
        ClientStore::open(&file, account(1), StoreSettings::default())
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&file)?, before);
    Ok(())
}

#[tokio::test]
async fn binding_checks_account_and_prior_binding() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, account(1), StoreSettings::default()).await?;
    let binding = EnrollmentBinding {
        account: account(1),
        server: "https://sync.example".into(),
        actor: ulid::Ulid::from(3_u128).to_string().parse()?,
        credential_reference: "protected:entry-1".into(),
    };
    identity::bind(&store, None, binding.clone()).await?;
    assert!(identity::bind(&store, None, binding.clone()).await.is_err());
    let mut next = binding.clone();
    next.actor = ulid::Ulid::from(4_u128).to_string().parse()?;
    identity::bind(&store, Some(&binding), next.clone()).await?;
    next.account = account(2);
    assert!(identity::bind(&store, Some(&binding), next).await.is_err());
    store.close().await?;
    let reopened = ClientStore::open(&file, account(1), StoreSettings::default()).await?;
    reopened.close().await?;
    Ok(())
}

#[tokio::test]
async fn newer_schema_is_not_modified() -> anyhow::Result<()> {
    use sqlx::{ConnectOptions, Connection, sqlite::SqliteConnectOptions};
    let dir = tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    ClientStore::create(&file, account(1), StoreSettings::default())
        .await?
        .close()
        .await?;
    let mut connection = SqliteConnectOptions::new()
        .filename(&file)
        .connect()
        .await?;
    sqlx::query_file!("tests/sql/newer_schema.sql")
        .execute(&mut connection)
        .await?;
    connection.close().await?;
    let before = std::fs::read(&file)?;
    let error = ClientStore::open(&file, account(1), StoreSettings::default())
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref(),
        Some(clacks_client_store::error::StoreError::IncompatibleSchema { found: 999, .. })
    ));
    assert_eq!(std::fs::read(&file)?, before);
    Ok(())
}

#[tokio::test]
async fn current_workspace_accepts_concurrent_store_handles() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    ClientStore::create(&file, account(1), StoreSettings::default())
        .await?
        .close()
        .await?;
    let (first, second) = tokio::join!(
        ClientStore::open(&file, account(1), StoreSettings::default()),
        ClientStore::open(&file, account(1), StoreSettings::default()),
    );
    for result in [first, second] {
        result?.close().await?;
    }
    Ok(())
}

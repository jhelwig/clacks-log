//! Account initialization rollback at an actual later relational write failure.

use super::*;

#[tokio::test]
async fn later_seed_failure_rolls_back_account_and_earlier_profile() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("workspace.sqlite");
    std::fs::File::create(&path)?;
    let mut connection = options(&path, &StoreSettings::default())
        .foreign_keys(true)
        .connect()
        .await?;
    migration::apply(&mut connection, 0).await?;
    sqlx::raw_sql(include_str!("tests/fail_group.sql"))
        .execute(&mut connection)
        .await?;
    let definitions = clacks_model::defaults::account_definitions()?;
    assert!(
        initialize(&mut connection, AccountId::new(), Some(&definitions))
            .await
            .is_err()
    );
    assert!(crate::identity::load(&mut connection).await?.is_none());
    let records = sqlx::query_file!("sql/workspace/populated.sql")
        .fetch_one(&mut connection)
        .await?;
    assert_eq!(records.populated, 0);
    connection.close().await?;
    assert!(
        ClientStore::open_discovered(&path, StoreSettings::default())
            .await
            .is_err()
    );
    assert!(path.exists());
    Ok(())
}

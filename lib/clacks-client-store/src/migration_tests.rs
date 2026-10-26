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
        Some(10)
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

/// Build the released schema and its real independent station rows before upgrading.
async fn legacy_operating_file(file: &Path, station_change: &str) -> anyhow::Result<AccountId> {
    use clacks_model::{
        contact::StationSnapshot, location::RecordedLocation, profile::SavedLocationProfile,
        scalar::RecordedCallsign,
    };
    use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};
    let account: AccountId = ulid::Ulid::from(1_u128).to_string().parse()?;
    let mut connection = options(file, &StoreSettings::default())
        .create_if_missing(true)
        .foreign_keys(true)
        .connect()
        .await?;
    Migrator::with_migrations(
        MIGRATIONS
            .iter()
            .filter(|m| m.version <= 8)
            .cloned()
            .collect(),
    )
    .run(&mut connection)
    .await?;
    let account_text = account.to_string();
    sqlx::query_file!("sql/workspace/create.sql", account_text)
        .execute(&mut connection)
        .await?;
    let profile = SavedLocationProfile {
        id: ulid::Ulid::from(30_u128).to_string().parse()?,
        name: "Home".into(),
        station: StationSnapshot {
            callsign: RecordedCallsign::new("W7HEX")?,
            location: None,
            references: Default::default(),
        },
        default_equipment: Some(ulid::Ulid::from(31_u128).to_string().parse()?),
    };
    let key = RecordKey {
        account,
        id: RecordId::Profile(profile.id),
    };
    let payload = crate::payload::insert(
        &mut connection,
        key,
        &RecordState::Live(RecordValue::Profile(Box::new(profile.clone()))),
    )
    .await?;
    let baseline_station =
        crate::payload::contact::insert_station(&mut connection, account, &profile.station).await?;
    let mut current_station = profile.station.clone();
    match station_change {
        "call" => current_station.callsign = RecordedCallsign::new("W7HEX/P")?,
        "geography" => {
            current_station.location = Some(RecordedLocation {
                dxcc: Some("291".into()),
                ..Default::default()
            })
        }
        "reference" => {
            current_station
                .references
                .insert(clacks_model::reference::ReferenceAssociation {
                    group_id: ulid::Ulid::from(50_u128).to_string().parse()?,
                    identifier: Some("US-0001".into()),
                });
        }
        _ => {}
    }
    let station =
        crate::payload::contact::insert_station(&mut connection, account, &current_station).await?;
    let operator_call = Some("W7ABC");
    let operator_name: Option<&str> = None;
    let power = Some(5000_i64);
    let equipment_values = clacks_model::equipment::EquipmentValues {
        equipment_description: Some("KX3".into()),
        antenna_description: Some("Vertical".into()),
        notes: None,
    };
    let equipment = Some(
        crate::payload::contact::insert_equipment(&mut connection, account, &equipment_values)
            .await?,
    );
    let saved_equipment = clacks_model::profile::EquipmentConfiguration {
        id: profile.default_equipment.unwrap(),
        name: "Rig A".into(),
        values: equipment_values,
    };
    crate::save::save_in(
        &mut connection,
        RecordKey {
            account,
            id: RecordId::Equipment(saved_equipment.id),
        },
        None,
        RecordState::Live(RecordValue::Equipment(Box::new(saved_equipment))),
    )
    .await?;
    let current = sqlx::query_file!(
        "sql/operating/value_insert.sql",
        account_text,
        station,
        operator_call,
        operator_name,
        power,
        equipment
    )
    .fetch_one(&mut connection)
    .await?
    .operating_id;
    let baseline = sqlx::query_file!(
        "sql/operating/value_insert.sql",
        account_text,
        baseline_station,
        operator_call,
        operator_name,
        power,
        equipment
    )
    .fetch_one(&mut connection)
    .await?
    .operating_id;
    sqlx::query(include_str!("../tests/sql/legacy_operating.sql"))
        .bind(&account_text)
        .bind(current)
        .bind(payload)
        .bind(profile.id.to_string())
        .bind(baseline)
        .execute(&mut connection)
        .await?;
    connection.close().await?;
    Ok(account)
}

#[tokio::test]
async fn old_baseline_migrates_by_station_values() -> anyhow::Result<()> {
    for change in ["none", "call", "geography", "reference"] {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("workspace.sqlite");
        let account = legacy_operating_file(&file, change).await?;
        let store = ClientStore::open(&file, account, StoreSettings::default()).await?;
        let state = crate::operating::load(&store).await?;
        assert_eq!(
            state.selected_profile.is_some(),
            change == "none",
            "{change}"
        );
        let current = state.current.unwrap();
        assert_eq!(current.transmit_power.unwrap().get(), 5000);
        assert_eq!(
            current.operator.unwrap().callsign.unwrap().as_str(),
            "W7ABC"
        );
        store.close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn legacy_equipment_selection_is_not_guessed() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let account = legacy_operating_file(&file, "none").await?;
    let store = ClientStore::open(&file, account, StoreSettings::default()).await?;
    let state = crate::operating::load(&store).await?;
    assert!(state.selected_profile.is_some());
    assert_eq!(state.selected_equipment, None);
    store.close().await?;
    Ok(())
}

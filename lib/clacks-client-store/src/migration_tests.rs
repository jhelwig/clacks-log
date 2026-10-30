use super::*;
use crate::store::ClientStore;

/// Create a version-8 workspace with station values and a profile baseline.
async fn legacy_operating_file(file: &Path, station_change: &str) -> anyhow::Result<AccountId> {
    use clacks_model::scalar::RecordedCallsign;
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
    let profile = clacks_model::profile::SavedLocationProfileFields {
        id: ulid::Ulid::from(30_u128).to_string().parse()?,
        name: "Home".into(),
        station: clacks_model::contact::StationSnapshot {
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
        &RecordState::Live(RecordValue::Profile(Box::new(
            (profile.clone()).try_into()?,
        ))),
    )
    .await?;
    let baseline_station =
        crate::payload::contact::insert_station(&mut connection, account, &profile.station).await?;
    let mut current_station = profile.station.clone();
    match station_change {
        "call" => current_station.callsign = RecordedCallsign::new("W7HEX/P")?,
        "geography" => {
            current_station.location = Some(
                (clacks_model::location::RecordedLocationFields {
                    dxcc: Some("291".into()),
                    ..Default::default()
                })
                .try_into()?,
            )
        }
        "reference" => {
            current_station
                .references
                .insert(clacks_model::reference::ReferenceAssociation::new(
                    ulid::Ulid::from(50_u128).to_string().parse()?,
                    Some("US-0001".into()),
                )?);
        }
        _ => {}
    }
    let station =
        crate::payload::contact::insert_station(&mut connection, account, &current_station).await?;
    let operator_call = Some("W7ABC");
    let operator_name: Option<&str> = None;
    let power = Some(5000_i64);
    let equipment_values = clacks_model::equipment::EquipmentValuesFields {
        equipment_description: Some("KX3".into()),
        antenna_description: Some("Vertical".into()),
        notes: None,
    };
    let equipment = Some(
        crate::payload::contact::insert_equipment(
            &mut connection,
            account,
            &clacks_model::equipment::EquipmentValues::try_from(equipment_values.clone())?,
        )
        .await?,
    );
    let saved_equipment = clacks_model::profile::EquipmentConfigurationFields {
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
        RecordState::Live(RecordValue::Equipment(Box::new(
            (saved_equipment).try_into()?,
        ))),
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
        assert_eq!(state.selected_equipment, None);
        assert_eq!(
            state.selected_profile.is_some(),
            change == "none",
            "{change}"
        );
        let current = state.current.unwrap();
        assert_eq!(current.transmit_power.unwrap().get(), 5000);
        assert_eq!(
            current
                .operator
                .as_ref()
                .unwrap()
                .callsign()
                .as_ref()
                .unwrap()
                .as_str(),
            "W7ABC"
        );
        store.close().await?;
    }
    Ok(())
}

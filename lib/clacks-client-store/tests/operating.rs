use crate::support;

use clacks_client_store::{
    operating::{self, OperatingSaveOutcome, OperatingState},
    retention,
    save::{self, SaveOutcome},
    store::{ClientStore, StoreSettings},
};
use clacks_model::scalar::{Milliwatts, RecordedCallsign};
use clacks_sync::record::{RecordState, RecordValue};
use support::id;

fn selected_state() -> anyhow::Result<OperatingState> {
    let profile = clacks_model::profile::SavedLocationProfileFields {
        id: id(30),
        name: "Home".into(),
        station: clacks_model::contact::StationSnapshot {
            callsign: RecordedCallsign::new("W7HEX")?,
            location: None,
            references: Default::default(),
        },
        default_equipment: Some(id(31)),
    };
    let equipment = clacks_model::profile::EquipmentConfigurationFields {
        id: id(32),
        name: "Rig B".into(),
        values: clacks_model::equipment::EquipmentValuesFields {
            equipment_description: Some("KX3".into()),
            antenna_description: Some("Vertical".into()),
            notes: None,
        },
    };
    Ok(OperatingState {
        current: Some(clacks_model::profile::CurrentOperatingValues {
            station: profile.station.clone(),
            operator: Some(
                (clacks_model::contact::OperatorInformationFields {
                    callsign: Some(RecordedCallsign::new("W7ABC")?),
                    recorded_name: Some("Recorded operator".into()),
                })
                .try_into()
                .expect("valid native fixture"),
            ),
            transmit_power: Some(Milliwatts::new(5000)?),
            equipment: Some(
                (equipment.values.clone())
                    .try_into()
                    .expect("valid native fixture"),
            ),
        }),
        selected_profile: Some((profile).try_into().expect("valid native fixture")),
        selected_equipment: Some((equipment).try_into().expect("valid native fixture")),
    })
}

#[tokio::test]
async fn independent_sources_survive_reopen() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, id(1), StoreSettings::default()).await?;
    let state = selected_state()?;
    let empty = operating::load(&store).await?;
    assert_eq!(empty, OperatingState::default());
    assert!(matches!(
        operating::save(&store, &empty, &state).await?,
        OperatingSaveOutcome::Saved(_)
    ));
    let profile_key = clacks_sync::record::RecordKey {
        account: id(1),
        id: clacks_sync::record::RecordId::Profile(id(30)),
    };
    assert!(save::load(&store, profile_key).await?.is_none());
    assert!(matches!(
        operating::save(&store, &state, &state).await?,
        OperatingSaveOutcome::Unchanged(_)
    ));
    store.close().await?;

    let reopened = ClientStore::open(&file, id(1), StoreSettings::default()).await?;
    let loaded = operating::load(&reopened).await?;
    assert_eq!(
        loaded.selected_profile.as_ref().map(|p| p.id()),
        Some(id(30))
    );
    assert_eq!(
        loaded.selected_equipment.as_ref().map(|e| e.id()),
        Some(id(32))
    );
    assert_eq!(
        loaded
            .selected_profile
            .as_ref()
            .and_then(|p| p.default_equipment()),
        Some(id(31))
    );
    assert_eq!(loaded.current, state.current);
    reopened.close().await?;
    Ok(())
}

#[tokio::test]
async fn apply_detaches_only_the_edited_component() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let store = ClientStore::create(
        &dir.path().join("workspace.sqlite"),
        id(1),
        StoreSettings::default(),
    )
    .await?;
    let state = selected_state()?;
    operating::save(&store, &OperatingState::default(), &state).await?;
    let mut station_edit = state.clone();
    station_edit.current.as_mut().unwrap().station.callsign = RecordedCallsign::new("W7HEX/P")?;
    station_edit.selected_profile = None;
    operating::save(&store, &state, &station_edit).await?;
    let saved = operating::load(&store).await?;
    assert_eq!(saved.selected_profile, None);
    assert_eq!(
        saved.selected_equipment.as_ref().map(|e| e.id()),
        Some(id(32))
    );

    assert_eq!(
        operating::save(&store, &state, &state).await?,
        OperatingSaveOutcome::Stale(saved.clone())
    );
    let mut equipment_edit = saved.clone();
    let current = equipment_edit.current.as_mut().unwrap();
    let mut equipment = current.equipment.take().unwrap().into_fields();
    equipment.antenna_description = Some("Loop".into());
    current.equipment = Some(equipment.try_into()?);
    equipment_edit.selected_equipment = None;
    operating::save(&store, &saved, &equipment_edit).await?;
    assert_eq!(operating::load(&store).await?, equipment_edit);
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn selection_roots_survive_collection() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, id(1), StoreSettings::default()).await?;
    let state = selected_state()?;
    operating::save(&store, &OperatingState::default(), &state).await?;
    let qso = support::qso(id(100));
    let value = RecordValue::Qso(Box::new(qso.clone()));
    let key = support::key(&value);
    assert!(matches!(
        save::save(&store, key, None, RecordState::Live(value)).await?,
        SaveOutcome::Saved(_)
    ));
    retention::collect_unreferenced(&store).await?;
    assert_eq!(operating::load(&store).await?, state);
    let mut detached = state.clone();
    detached.selected_equipment = None;
    operating::save(&store, &state, &detached).await?;
    retention::collect_unreferenced(&store).await?;
    let row = save::load(&store, key).await?.unwrap();
    assert_eq!(
        row.working_state(),
        Some(&RecordState::Live(RecordValue::Qso(Box::new(qso))))
    );
    store.close().await?;
    let reopened = ClientStore::open(&file, id(1), StoreSettings::default()).await?;
    assert_eq!(operating::load(&reopened).await?, detached);
    reopened.close().await?;
    Ok(())
}

mod support;
use clacks_client_store::{
    operating::{self, OperatingSaveOutcome, OperatingState},
    save::{self, SaveOutcome},
    store::{ClientStore, StoreSettings},
};
use clacks_model::{
    contact::{OperatorInformation, StationSnapshot},
    equipment::EquipmentValues,
    location::RecordedLocation,
    profile::{CurrentOperatingValues, SavedLocationProfile},
    scalar::{Milliwatts, RecordedCallsign},
};
use clacks_sync::record::{RecordState, RecordValue};
use support::id;

#[tokio::test]
async fn profile_load_replaces_station_and_preserves_current_context_and_copies()
-> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, id(1), StoreSettings::default()).await?;
    let empty = operating::load(&store).await?;
    assert_eq!(
        empty,
        OperatingState {
            current: None,
            loaded: None
        }
    );
    let current = CurrentOperatingValues {
        station: StationSnapshot {
            callsign: RecordedCallsign::new("W7HEX")?,
            location: Some(RecordedLocation {
                dxcc: Some("291".into()),
                ..Default::default()
            }),
            references: Default::default(),
        },
        operator: Some(OperatorInformation {
            callsign: None,
            recorded_name: Some("Recorded operator".into()),
        }),
        transmit_power: Some(Milliwatts::new(5000)?),
        equipment: Some(EquipmentValues {
            equipment_description: Some("Current rig".into()),
            ..Default::default()
        }),
    };
    let state = OperatingState {
        current: Some(current.clone()),
        loaded: None,
    };
    assert!(matches!(
        operating::save(&store, &empty, &state).await?,
        OperatingSaveOutcome::Saved(_)
    ));
    assert_eq!(
        operating::save(&store, &empty, &empty).await?,
        OperatingSaveOutcome::Stale(state.clone())
    );
    let profile = SavedLocationProfile {
        id: id(30),
        name: "Operating location".into(),
        station: StationSnapshot {
            callsign: RecordedCallsign::new("W7HEX/P")?,
            location: None,
            references: Default::default(),
        },
        default_equipment: None,
    };
    let value = RecordValue::Profile(Box::new(profile.clone()));
    let key = support::key(&value);
    let SaveOutcome::Saved(saved) = save::save(&store, key, None, RecordState::Live(value)).await?
    else {
        panic!("profile save expected")
    };
    let OperatingSaveOutcome::Saved(loaded) =
        operating::load_profile(&store, &state, &saved, None).await?
    else {
        panic!("profile load expected")
    };
    let loaded_current = loaded.current.as_ref().unwrap();
    assert_eq!(loaded_current.station, profile.station);
    assert_eq!(loaded_current.equipment, current.equipment);
    assert_eq!(loaded_current.operator, current.operator);
    assert_eq!(loaded_current.transmit_power, current.transmit_power);
    assert_eq!(loaded.loaded.as_ref().unwrap().profile, profile);
    let mut updated = profile;
    updated.station.callsign = RecordedCallsign::new("W7HEX/M")?;
    save::save(
        &store,
        key,
        Some(&saved),
        RecordState::Live(RecordValue::Profile(Box::new(updated))),
    )
    .await?;
    assert_eq!(operating::load(&store).await?, loaded);
    store.close().await?;
    let store = ClientStore::open(&file, id(1), StoreSettings::default()).await?;
    assert_eq!(operating::load(&store).await?, loaded);
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn equipment_default_is_checked_and_copied_without_a_live_link() -> anyhow::Result<()> {
    use clacks_model::profile::EquipmentConfiguration;
    let dir = tempfile::tempdir()?;
    let store = ClientStore::create(
        &dir.path().join("workspace.sqlite"),
        id(1),
        StoreSettings::default(),
    )
    .await?;
    let equipment = EquipmentConfiguration {
        id: id(50),
        name: "Selected".into(),
        values: EquipmentValues {
            equipment_description: Some("Selected rig".into()),
            ..Default::default()
        },
    };
    let value = RecordValue::Equipment(Box::new(equipment.clone()));
    let key = support::key(&value);
    let SaveOutcome::Saved(saved_equipment) =
        save::save(&store, key, None, RecordState::Live(value)).await?
    else {
        panic!("equipment save expected")
    };
    let profile = SavedLocationProfile {
        id: id(51),
        name: "Base".into(),
        station: StationSnapshot {
            callsign: RecordedCallsign::new("W7HEX")?,
            location: None,
            references: Default::default(),
        },
        default_equipment: Some(equipment.id),
    };
    let value = RecordValue::Profile(Box::new(profile));
    let key = support::key(&value);
    let SaveOutcome::Saved(profile) =
        save::save(&store, key, None, RecordState::Live(value)).await?
    else {
        panic!("profile save expected")
    };
    let empty = operating::load(&store).await?;
    assert!(
        operating::load_profile(&store, &empty, &profile, None)
            .await
            .is_err()
    );
    let OperatingSaveOutcome::Saved(loaded) =
        operating::load_profile(&store, &empty, &profile, Some(&saved_equipment)).await?
    else {
        panic!("load expected")
    };
    assert_eq!(
        loaded.current.as_ref().unwrap().equipment,
        Some(equipment.values)
    );
    save::save(
        &store,
        saved_equipment.key,
        Some(&saved_equipment),
        RecordState::Deleted,
    )
    .await?;
    assert_eq!(operating::load(&store).await?, loaded);
    assert_eq!(
        operating::load_profile(&store, &loaded, &profile, Some(&saved_equipment)).await?,
        OperatingSaveOutcome::Stale(loaded)
    );
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn saved_contact_keeps_its_station_snapshot_after_profile_edit_and_reopen()
-> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, id(1), StoreSettings::default()).await?;
    let profile = SavedLocationProfile {
        id: id(30),
        name: "Field station".into(),
        station: StationSnapshot {
            callsign: RecordedCallsign::new("W7HEX/P")?,
            location: Some(RecordedLocation {
                primary_subdivision: Some("WA".into()),
                ..Default::default()
            }),
            references: [clacks_model::reference::ReferenceAssociation {
                group_id: id(40),
                identifier: Some("US-1234".into()),
            }]
            .into(),
        },
        default_equipment: None,
    };
    let value = RecordValue::Profile(Box::new(profile.clone()));
    let key = support::key(&value);
    let SaveOutcome::Saved(saved_profile) =
        save::save(&store, key, None, RecordState::Live(value)).await?
    else {
        panic!("profile save expected");
    };
    let empty = operating::load(&store).await?;
    let OperatingSaveOutcome::Saved(loaded) =
        operating::load_profile(&store, &empty, &saved_profile, None).await?
    else {
        panic!("profile load expected");
    };
    let mut contact = support::qso(id(10));
    contact.own_station = loaded.current.as_ref().unwrap().station.clone();
    let contact_key = support::qso_key(contact.id);
    let SaveOutcome::Saved(saved_contact) =
        save::save(&store, contact_key, None, support::contact(contact)).await?
    else {
        panic!("contact save expected");
    };

    let mut changed = profile;
    changed.station.callsign = RecordedCallsign::new("W7HEX/M")?;
    changed.station.location = None;
    changed.station.references.clear();
    let SaveOutcome::Saved(updated_profile) = save::save(
        &store,
        key,
        Some(&saved_profile),
        RecordState::Live(RecordValue::Profile(Box::new(changed))),
    )
    .await?
    else {
        panic!("profile update expected");
    };
    let OperatingSaveOutcome::Saved(reloaded) =
        operating::load_profile(&store, &loaded, &updated_profile, None).await?
    else {
        panic!("updated profile load expected");
    };
    assert_eq!(
        reloaded.current.as_ref().unwrap().station.callsign.as_str(),
        "W7HEX/M"
    );
    assert!(
        reloaded
            .current
            .as_ref()
            .unwrap()
            .station
            .location
            .is_none()
    );
    store.close().await?;

    let store = ClientStore::open(&file, id(1), StoreSettings::default()).await?;
    assert_eq!(save::load(&store, contact_key).await?, Some(saved_contact));
    assert_eq!(operating::load(&store).await?, reloaded);
    store.close().await?;
    Ok(())
}

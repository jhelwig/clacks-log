use crate::settings_support;
use crate::support;

use clacks_client_store::{
    operating::{self, OperatingSaveOutcome, OperatingState},
    read,
    save::{self, SaveOutcome},
    settings,
    store::{ClientStore, StoreSettings},
};
use clacks_model::group::{GroupDefinition, IdentifierDefinition};
use clacks_sync::record::{RecordState, RecordValue};
use settings_support::{equipment, operating as state, profile};
use support::id;

fn saved(outcome: SaveOutcome) -> clacks_sync::client::ClientRecord {
    match outcome {
        SaveOutcome::Saved(row) | SaveOutcome::Unchanged(row) => row,
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn settings_snapshot_is_one_account_and_one_read_basis() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let store = ClientStore::create(
        &dir.path().join("account.sqlite"),
        id(1),
        StoreSettings::default(),
    )
    .await?;
    let a = equipment(31, "Rig A");
    let b = equipment(32, "Rig B");
    let home = profile(30, Some(31));
    settings::save_equipment(&store, None, &a).await?;
    settings::save_equipment(&store, None, &b).await?;
    settings::save_profile(&store, None, &home).await?;
    let group = GroupDefinition {
        id: id(50),
        name: "Field event".into(),
        identifier: IdentifierDefinition {
            required: false,
            normalization: vec![],
            syntax: None,
        },
        bears_location_context: true,
        catalog: None,
        export_mappings: Default::default(),
    };
    let value = RecordValue::Group(Box::new(group));
    save::save(&store, support::key(&value), None, RecordState::Live(value)).await?;
    let active = state(&home, &b);
    operating::save(&store, &OperatingState::default(), &active).await?;
    let mut snapshot = read::snapshot(&store).await?;
    let before = snapshot.settings().await?;
    let row = before.profiles.first().unwrap();
    let mut changed = home.clone();
    changed.name = "Renamed".into();
    settings::save_profile(&store, Some(row), &changed).await?;
    assert_eq!(snapshot.settings().await?.profiles, before.profiles);
    snapshot.close().await?;
    let after = settings::snapshot(&store).await?;
    assert_ne!(after.profiles, before.profiles);
    assert_eq!(after.operating, active);
    assert_eq!(after.groups.len(), 1);
    assert_eq!(after.equipment.len(), 2);

    let other = ClientStore::create(
        &dir.path().join("other.sqlite"),
        id(2),
        StoreSettings::default(),
    )
    .await?;
    assert!(
        settings::save_profile(&other, Some(row), &home)
            .await
            .is_err()
    );
    assert!(settings::snapshot(&other).await?.profiles.is_empty());
    other.close().await?;
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn profile_save_with_deleted_default_is_pending() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let store = ClientStore::create(
        &dir.path().join("account.sqlite"),
        id(1),
        StoreSettings::default(),
    )
    .await?;
    let a = equipment(31, "Rig A");
    let row = saved(settings::save_equipment(&store, None, &a).await?);
    save::save(&store, row.key, Some(&row), RecordState::Deleted).await?;
    let home = profile(30, Some(31));
    let result = settings::save_profile(&store, None, &home).await?;
    assert_eq!(result.missing_equipment, Some(a.id));
    let row = saved(result.outcome);
    assert!(row.pending.is_some());
    assert_eq!(
        row.working_state(),
        Some(&RecordState::Live(RecordValue::Profile(Box::new(home))))
    );
    let view = settings::snapshot(&store).await?;
    assert_eq!(view.operating, OperatingState::default());
    assert_eq!(
        view.equipment.first().unwrap().working_state(),
        Some(&RecordState::Deleted)
    );
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn profile_save_preserves_its_stale_guard() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let store = ClientStore::create(
        &dir.path().join("account.sqlite"),
        id(1),
        StoreSettings::default(),
    )
    .await?;
    let home = profile(30, None);
    let row = saved(settings::save_profile(&store, None, &home).await?.outcome);
    let mut changed = home.clone();
    changed.name = "Intervening".into();
    settings::save_profile(&store, Some(&row), &changed).await?;
    let mut stale = home.clone();
    stale.default_equipment = Some(id(31));
    let result = settings::save_profile(&store, Some(&row), &stale).await?;
    assert!(matches!(result.outcome, SaveOutcome::Stale { .. }));
    assert_eq!(result.missing_equipment, None);
    let snapshot = settings::snapshot(&store).await?;
    assert_eq!(
        snapshot.profiles.first().unwrap().working_state(),
        Some(&RecordState::Live(RecordValue::Profile(Box::new(changed))))
    );
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn apply_keeps_loaded_values_after_saved_source_update() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let store = ClientStore::create(
        &dir.path().join("account.sqlite"),
        id(1),
        StoreSettings::default(),
    )
    .await?;
    let a = equipment(31, "Rig A");
    let row = saved(settings::save_equipment(&store, None, &a).await?);
    let home = profile(30, Some(31));
    let active = state(&home, &a);
    let mut changed = a.clone();
    changed.values.antenna_description = Some("End-fed".into());
    settings::save_equipment(&store, Some(&row), &changed).await?;
    assert!(matches!(
        operating::save(&store, &OperatingState::default(), &active).await?,
        OperatingSaveOutcome::Saved(_)
    ));
    assert_eq!(operating::load(&store).await?, active);
    let mut invalid = active.clone();
    invalid.current.as_mut().unwrap().station.callsign =
        clacks_model::scalar::RecordedCallsign::new("K1ABC")?;
    assert!(operating::save(&store, &active, &invalid).await.is_err());
    assert_eq!(operating::load(&store).await?, active);
    store.close().await?;
    Ok(())
}

use crate::telemetry;

#[test]
fn settings_telemetry_counts_commits_without_payloads() -> anyhow::Result<()> {
    let capture = telemetry::capture_filtered(
        || -> anyhow::Result<()> {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(async {
                    let dir = tempfile::tempdir()?;
                    let store = ClientStore::create(
                        &dir.path().join("account.sqlite"),
                        id(1),
                        StoreSettings::default(),
                    )
                    .await?;
                    let mut rig = equipment(31, "SECRET EQUIPMENT NAME");
                    let first = saved(settings::save_equipment(&store, None, &rig).await?);
                    rig.values.notes = Some("SECRET EQUIPMENT NOTES".into());
                    let second = saved(settings::save_equipment(&store, Some(&first), &rig).await?);
                    assert!(matches!(
                        settings::save_equipment(&store, Some(&second), &rig).await?,
                        SaveOutcome::Unchanged(_)
                    ));
                    assert!(matches!(
                        settings::save_equipment(&store, Some(&first), &rig).await?,
                        SaveOutcome::Stale { .. }
                    ));
                    let mut home = profile(30, Some(31));
                    home.name = "SECRET PROFILE NAME".into();
                    let first_profile =
                        saved(settings::save_profile(&store, None, &home).await?.outcome);
                    home.name = "PRIVATE PROFILE NAME".into();
                    let second_profile = saved(
                        settings::save_profile(&store, Some(&first_profile), &home)
                            .await?
                            .outcome,
                    );
                    assert!(matches!(
                        settings::save_profile(&store, Some(&second_profile), &home)
                            .await?
                            .outcome,
                        SaveOutcome::Unchanged(_)
                    ));
                    let active = state(&home, &rig);
                    operating::save(&store, &OperatingState::default(), &active).await?;
                    operating::save(&store, &active, &active).await?;
                    save::save(
                        &store,
                        second_profile.key,
                        Some(&second_profile),
                        RecordState::Deleted,
                    )
                    .await?;
                    save::save(&store, second.key, Some(&second), RecordState::Deleted).await?;
                    store.close().await?;
                    Ok(())
                })
        },
        |metadata| *metadata.level() <= tracing::Level::INFO,
    );
    capture
        .result
        .as_ref()
        .map_err(|e| anyhow::anyhow!("{e:#}"))?;
    for metric in [
        "clacks_client_profile_mutations_total",
        "clacks_client_equipment_mutations_total",
    ] {
        for change in ["create", "update", "delete"] {
            capture.counter(metric, &[("change", change)], 1);
        }
    }
    capture.counter("clacks_client_operating_applications_total", &[], 1);
    assert!(
        capture
            .event("operating.applied")
            .fields
            .contains_key("account_id")
    );
    let observations = format!("{:?}", capture.observations);
    for text in [
        "SECRET EQUIPMENT NAME",
        "SECRET EQUIPMENT NOTES",
        "SECRET PROFILE NAME",
        "PRIVATE PROFILE NAME",
        "W7HEX",
    ] {
        assert!(!observations.contains(text), "leaked {text}");
    }
    Ok(())
}

#[path = "support/settings.rs"]
mod settings_support;
mod support;

use clacks_client_store::{
    operating::{self, OperatingSaveOutcome, OperatingState},
    save,
    settings::{self, EquipmentDeleteOutcome, ProfileEquipmentReplacement, SettingsSnapshot},
    store::{ClientStore, StoreSettings},
    transfer,
};
use clacks_model::{
    ids::{EquipmentConfigurationId, ProfileId},
    profile::{EquipmentConfiguration, SavedLocationProfile},
    scalar::Milliwatts,
};
use clacks_sync::{
    client::ClientRecord,
    record::{AcceptedRecord, RecordId, RecordKey, RecordState, RecordValue},
};
use settings_support::{equipment, operating as state, profile};
use support::id;

struct Fixture {
    directory: tempfile::TempDir,
    store: ClientStore,
    a: EquipmentConfiguration,
    b: EquipmentConfiguration,
    home: SavedLocationProfile,
    field: SavedLocationProfile,
    active: OperatingState,
}

async fn fixture() -> anyhow::Result<Fixture> {
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create(
        &directory.path().join("account.sqlite"),
        id(1),
        StoreSettings::default(),
    )
    .await?;
    let a = equipment(31, "Rig A");
    let b = equipment(32, "Rig B");
    settings::save_equipment(&store, None, &a).await?;
    settings::save_equipment(&store, None, &b).await?;
    let home = profile(30, Some(31));
    let mut field = profile(33, Some(31));
    field.name = "Field".into();
    settings::save_profile(&store, None, &home).await?;
    settings::save_profile(&store, None, &field).await?;
    let active = state(&home, &a);
    operating::save(&store, &OperatingState::default(), &active).await?;
    let mut qso = support::qso(id(100));
    qso.own_station = active.current.as_ref().unwrap().station.clone();
    qso.own_context.equipment = active.current.as_ref().unwrap().equipment.clone();
    let value = RecordValue::Qso(Box::new(qso));
    let key = support::key(&value);
    save::save(&store, key, None, RecordState::Live(value)).await?;
    Ok(Fixture {
        directory,
        store,
        a,
        b,
        home,
        field,
        active,
    })
}

#[tokio::test]
async fn historical_qso_copies_survive_source_changes_apply_collection_and_reopen()
-> anyhow::Result<()> {
    let f = fixture().await?;
    let qso_key = support::qso_key(id(100));
    let original = save::load(&f.store, qso_key).await?.unwrap();
    let home = record(&f.store, RecordId::Profile(f.home.id)).await;
    let mut changed_home = f.home.clone();
    changed_home.station.callsign = clacks_model::scalar::RecordedCallsign::new("W7HEX/P")?;
    settings::save_profile(&f.store, Some(&home), &changed_home).await?;
    let rig = record(&f.store, RecordId::Equipment(f.a.id)).await;
    let mut changed_rig = f.a.clone();
    changed_rig.values.antenna_description = Some("Loop".into());
    settings::save_equipment(&f.store, Some(&rig), &changed_rig).await?;
    assert_eq!(operating::load(&f.store).await?, f.active);

    let preview = settings::preview_equipment_deletion(&f.store, f.a.id).await?;
    assert_eq!(
        settings::delete_equipment(&f.store, &preview, &choices(&f)).await?,
        EquipmentDeleteOutcome::Applied
    );
    let home = record(&f.store, RecordId::Profile(f.home.id)).await;
    settings::delete_profile(&f.store, &home).await?;
    let after_deletions = operating::load(&f.store).await?;
    let mut next = after_deletions.clone();
    let current = next.current.as_mut().unwrap();
    current.station.callsign = clacks_model::scalar::RecordedCallsign::new("W7FIELD")?;
    current.equipment = Some(f.b.values.clone());
    current.transmit_power = Some(Milliwatts::new(25_000)?);
    operating::save(&f.store, &after_deletions, &next).await?;
    clacks_client_store::retention::collect_unreferenced(&f.store).await?;
    f.store.close().await?;

    let reopened = ClientStore::open(
        &f.directory.path().join("account.sqlite"),
        id(1),
        StoreSettings::default(),
    )
    .await?;
    assert_eq!(operating::load(&reopened).await?, next);
    let persisted = save::load(&reopened, qso_key).await?.unwrap();
    assert_eq!(persisted, original);
    let Some(RecordState::Live(RecordValue::Qso(qso))) = persisted.working_state() else {
        panic!("historical QSO must remain live");
    };
    assert_eq!(qso.own_station, f.active.current.as_ref().unwrap().station);
    assert_eq!(
        qso.own_context.equipment,
        f.active.current.as_ref().unwrap().equipment
    );
    reopened.close().await?;
    Ok(())
}

fn default(view: &SettingsSnapshot, id: ProfileId) -> Option<EquipmentConfigurationId> {
    view.profiles
        .iter()
        .find_map(|r| match r.working_state() {
            Some(RecordState::Live(RecordValue::Profile(p))) if p.id == id => {
                Some(p.default_equipment)
            }
            _ => None,
        })
        .expect("profile remains live")
}

async fn record(store: &ClientStore, id: RecordId) -> ClientRecord {
    save::load(
        store,
        RecordKey {
            account: store.account_id(),
            id,
        },
    )
    .await
    .unwrap()
    .unwrap()
}

fn choices(f: &Fixture) -> Vec<ProfileEquipmentReplacement> {
    vec![
        ProfileEquipmentReplacement {
            profile: f.home.id,
            equipment: Some(f.b.id),
        },
        ProfileEquipmentReplacement {
            profile: f.field.id,
            equipment: None,
        },
    ]
}

#[tokio::test]
async fn delete_source_clears_only_matching_metadata() -> anyhow::Result<()> {
    let f = fixture().await?;
    let home = record(&f.store, RecordId::Profile(f.home.id)).await;
    settings::delete_profile(&f.store, &home).await?;
    let after = operating::load(&f.store).await?;
    assert_eq!(after.current, f.active.current);
    assert_eq!(after.selected_profile, None);
    assert_eq!(after.selected_equipment, f.active.selected_equipment);
    let preview = settings::preview_equipment_deletion(&f.store, f.a.id).await?;
    settings::delete_equipment(
        &f.store,
        &preview,
        &[ProfileEquipmentReplacement {
            profile: f.field.id,
            equipment: None,
        }],
    )
    .await?;
    let after = operating::load(&f.store).await?;
    assert_eq!(after.current, f.active.current);
    assert_eq!(after.selected_equipment, None);
    f.store.close().await?;
    let reopened = ClientStore::open(
        &f.directory.path().join("account.sqlite"),
        id(1),
        StoreSettings::default(),
    )
    .await?;
    assert_eq!(operating::load(&reopened).await?, after);
    reopened.close().await?;
    Ok(())
}

#[tokio::test]
async fn equipment_delete_repairs_all_reviewed_profiles_atomically() -> anyhow::Result<()> {
    let f = fixture().await?;
    let qso_key = support::qso_key(id(100));
    let before = save::load(&f.store, qso_key).await?;
    let preview = settings::preview_equipment_deletion(&f.store, f.a.id).await?;
    assert_eq!(preview.referring_profiles.len(), 2);
    assert_eq!(
        settings::delete_equipment(&f.store, &preview, &choices(&f)).await?,
        EquipmentDeleteOutcome::Applied
    );
    let after = settings::snapshot(&f.store).await?;
    assert_eq!(default(&after, f.home.id), Some(f.b.id));
    assert_eq!(default(&after, f.field.id), None);
    assert_eq!(
        record(&f.store, RecordId::Equipment(f.a.id))
            .await
            .working_state(),
        Some(&RecordState::Deleted)
    );
    assert_eq!(after.operating.current, f.active.current);
    assert_eq!(after.operating.selected_equipment, None);
    assert_eq!(after.operating.selected_profile, f.active.selected_profile);
    assert_eq!(save::load(&f.store, qso_key).await?, before);
    f.store.close().await?;
    Ok(())
}

#[tokio::test]
async fn new_referrer_makes_preview_stale() -> anyhow::Result<()> {
    let f = fixture().await?;
    let preview = settings::preview_equipment_deletion(&f.store, f.a.id).await?;
    settings::save_profile(&f.store, None, &profile(34, Some(31))).await?;
    let before = settings::snapshot(&f.store).await?;
    assert!(matches!(
        settings::delete_equipment(&f.store, &preview, &choices(&f)).await?,
        EquipmentDeleteOutcome::Stale(_)
    ));
    assert_eq!(settings::snapshot(&f.store).await?, before);
    f.store.close().await?;
    Ok(())
}

#[tokio::test]
async fn replacement_edit_or_deletion_aborts_review() -> anyhow::Result<()> {
    for delete in [false, true] {
        let f = fixture().await?;
        let preview = settings::preview_equipment_deletion(&f.store, f.a.id).await?;
        let old = record(&f.store, RecordId::Equipment(f.b.id)).await;
        if delete {
            save::save(&f.store, old.key, Some(&old), RecordState::Deleted).await?;
        } else {
            let mut changed = f.b.clone();
            changed.values.notes = Some("Updated".into());
            settings::save_equipment(&f.store, Some(&old), &changed).await?;
        }
        let before = settings::snapshot(&f.store).await?;
        assert!(matches!(
            settings::delete_equipment(&f.store, &preview, &choices(&f)).await?,
            EquipmentDeleteOutcome::Stale(_)
        ));
        assert_eq!(settings::snapshot(&f.store).await?, before);
        f.store.close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn failed_equipment_delete_rolls_back_repairs_and_cleanup() -> anyhow::Result<()> {
    use sqlx::{Connection, Executor};
    let f = fixture().await?;
    let preview = settings::preview_equipment_deletion(&f.store, f.a.id).await?;
    let before = settings::snapshot(&f.store).await?;
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(f.directory.path().join("account.sqlite")),
    )
    .await?;
    connection
        .execute(include_str!("sql/fail_equipment_delete.sql"))
        .await?;
    connection.close().await?;
    assert!(
        settings::delete_equipment(&f.store, &preview, &choices(&f))
            .await
            .is_err()
    );
    assert_eq!(settings::snapshot(&f.store).await?, before);
    f.store.close().await?;
    let reopened = ClientStore::open(
        &f.directory.path().join("account.sqlite"),
        id(1),
        StoreSettings::default(),
    )
    .await?;
    assert_eq!(settings::snapshot(&reopened).await?, before);
    reopened.close().await?;
    Ok(())
}

#[tokio::test]
async fn apply_racing_delete_never_reattaches() -> anyhow::Result<()> {
    let f = fixture().await?;
    let expected = f.active.clone();
    let mut desired = expected.clone();
    desired.current.as_mut().unwrap().transmit_power = Some(Milliwatts::new(25_000)?);
    let home = record(&f.store, RecordId::Profile(f.home.id)).await;
    settings::delete_profile(&f.store, &home).await?;
    assert!(matches!(
        operating::save(&f.store, &expected, &desired).await?,
        OperatingSaveOutcome::Saved(_)
    ));
    let current = operating::load(&f.store).await?;
    assert_eq!(current.selected_profile, None);
    assert_eq!(
        current
            .current
            .as_ref()
            .unwrap()
            .transmit_power
            .unwrap()
            .get(),
        25_000
    );
    desired.current.as_mut().unwrap().transmit_power = Some(Milliwatts::new(10_000)?);
    assert!(matches!(
        operating::save(&f.store, &expected, &desired).await?,
        OperatingSaveOutcome::Stale(_)
    ));
    assert_eq!(operating::load(&f.store).await?, current);
    f.store.close().await?;
    Ok(())
}

async fn install(
    store: &ClientStore,
    accepted: AcceptedRecord,
    baseline: clacks_sync::account::Baseline,
) -> anyhow::Result<()> {
    use clacks_sync::{
        account::InstalledCheckpoint,
        lineage::Coverage,
        transfer::{RecordRequirement, WindowManifest, WindowMode},
    };
    let source = InstalledCheckpoint {
        account: id(1),
        baseline: baseline.id,
        coverage: Coverage::from(accepted.stamp.accepted_at),
    };
    let manifest = WindowManifest::new(
        id(1),
        source.clone(),
        WindowMode::Full,
        vec![RecordRequirement {
            key: accepted.key,
            stamp: accepted.stamp.clone(),
            deleted: matches!(accepted.state, RecordState::Deleted),
        }],
        vec![],
        vec![baseline],
        vec![],
    )?;
    transfer::remember(store, &manifest).await?;
    transfer::install(store, &source, &WindowMode::Full, &[accepted]).await?;
    transfer::finish(store, &source, &WindowMode::Full).await?;
    Ok(())
}

#[tokio::test]
async fn effective_received_deletion_forgets_source() -> anyhow::Result<()> {
    for pending_edit in [false, true] {
        let dir = tempfile::tempdir()?;
        let store = ClientStore::create(
            &dir.path().join("account.sqlite"),
            id(1),
            StoreSettings::default(),
        )
        .await?;
        let a = equipment(31, "Rig A");
        let home = profile(30, None);
        let (_, baseline) = clacks_sync::account::AccountMetadata::initialize(id(1), id(2), id(3));
        let live = support::accepted(
            1,
            RecordState::Live(RecordValue::Equipment(Box::new(a.clone()))),
        );
        install(&store, live, baseline.clone()).await?;
        let active = state(&home, &a);
        operating::save(&store, &OperatingState::default(), &active).await?;
        if pending_edit {
            let row = record(&store, RecordId::Equipment(a.id)).await;
            let mut changed = a.clone();
            changed.values.antenna_description = Some("Loop".into());
            settings::save_equipment(&store, Some(&row), &changed).await?;
        }
        let mut deleted = support::accepted(
            2,
            RecordState::Live(RecordValue::Equipment(Box::new(a.clone()))),
        );
        deleted.state = RecordState::Deleted;
        install(&store, deleted, baseline).await?;
        let current = operating::load(&store).await?;
        assert_eq!(current.current, active.current);
        assert_eq!(current.selected_profile, active.selected_profile);
        assert_eq!(current.selected_equipment.is_some(), pending_edit);
        assert_eq!(
            matches!(
                record(&store, RecordId::Equipment(a.id))
                    .await
                    .working_state(),
                Some(RecordState::Live(_))
            ),
            pending_edit
        );
        store.close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn invalid_or_unreviewed_deletion_choices_have_no_effects() -> anyhow::Result<()> {
    let f = fixture().await?;
    let preview = settings::preview_equipment_deletion(&f.store, f.a.id).await?;
    let before = settings::snapshot(&f.store).await?;
    let bad = vec![
        vec![],
        vec![
            ProfileEquipmentReplacement {
                profile: f.home.id,
                equipment: Some(f.a.id),
            },
            ProfileEquipmentReplacement {
                profile: f.field.id,
                equipment: None,
            },
        ],
        vec![
            ProfileEquipmentReplacement {
                profile: f.home.id,
                equipment: None,
            },
            ProfileEquipmentReplacement {
                profile: f.home.id,
                equipment: None,
            },
        ],
        vec![
            ProfileEquipmentReplacement {
                profile: f.home.id,
                equipment: Some(id(999)),
            },
            ProfileEquipmentReplacement {
                profile: f.field.id,
                equipment: None,
            },
        ],
    ];
    for choices in bad {
        assert!(
            settings::delete_equipment(&f.store, &preview, &choices)
                .await
                .is_err()
        );
        assert_eq!(settings::snapshot(&f.store).await?, before);
    }
    let row = record(&f.store, RecordId::Equipment(f.a.id)).await;
    assert!(
        save::save(&f.store, row.key, Some(&row), RecordState::Deleted)
            .await
            .is_err()
    );
    assert_eq!(settings::snapshot(&f.store).await?, before);
    f.store.close().await?;
    Ok(())
}

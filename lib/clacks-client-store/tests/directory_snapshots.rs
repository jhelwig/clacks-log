//! Real local operations preserve separate snapshots through restart, callsign edits and deletion.

use crate::support::{self, id, qso, qso_key};
use clacks_client_store::{
    directory::snapshots::{self, SnapshotOutcome},
    save::{self, SaveOutcome},
    store::ClientStore,
};
use clacks_model::{
    directory::values::{
        DirectoryFacts, DirectoryProvider, DirectorySnapshot, DirectorySnapshotFields, LookupKey,
    },
    scalar::{RecordedCallsign, UtcTimestamp},
};
use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};

/// Build a checked successful provider result, never a synthetic failure record.
pub(super) fn snapshot(
    call: &str,
    second: i64,
    name: Option<&str>,
) -> anyhow::Result<DirectorySnapshot> {
    DirectorySnapshot::new(DirectorySnapshotFields {
        provider: DirectoryProvider::Qrz,
        key: LookupKey::from_recorded(&RecordedCallsign::new(call)?),
        lookup_time: UtcTimestamp::new(second, 0)?,
        lookup_id: id(second as u128),
        facts: DirectoryFacts {
            name: name.map(str::to_owned),
            ..Default::default()
        },
    })
}

#[tokio::test]
async fn directory_snapshots_survive_restart_and_key_changes() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, id(1), Default::default()).await?;
    let original = qso(id(10));
    let call = original.contacted_station().callsign.as_str().to_owned();
    let SaveOutcome::Saved(saved) = save::contacts::save(
        &store,
        None,
        original.clone(),
        Some(snapshot(&call, 10, Some("First lookup"))?),
    )
    .await?
    else {
        panic!("save")
    };
    assert_eq!(
        snapshots::apply(&store, id(10), snapshot(&call, 11, None)?).await?,
        SnapshotOutcome::Applied
    );
    assert_eq!(
        save::load(&store, qso_key(id(10)))
            .await?
            .unwrap()
            .working_state(),
        Some(&support::contact(original.clone()))
    );
    store.close().await?;
    let store = ClientStore::open(&file, id(1), Default::default()).await?;
    let key = RecordKey {
        account: id(1),
        id: RecordId::Directory(id(10)),
    };
    let row = save::load(&store, key).await?.unwrap();
    let Some(RecordState::Live(RecordValue::Directory(values))) = row.working_state() else {
        panic!("directory")
    };
    let lookup = LookupKey::from_recorded(&RecordedCallsign::new(&call)?);
    assert_eq!(
        values.snapshot(&lookup).unwrap().lookup_time(),
        UtcTimestamp::new(11, 0)?
    );
    assert!(values.snapshot(&lookup).unwrap().facts().name.is_none());
    let mut changed = original.clone();
    support::edit(&mut changed, |fields| {
        fields.contacted_station.callsign = RecordedCallsign::new("K2ABC")?;
        Ok(())
    });
    let SaveOutcome::Saved(edited) =
        save::contacts::save(&store, Some(&saved), changed.clone(), None).await?
    else {
        panic!("edit")
    };
    assert_eq!(
        snapshots::apply(
            &store,
            id(10),
            snapshot(&call, 20, Some("Obsolete request"))?
        )
        .await?,
        SnapshotOutcome::Ineligible
    );
    assert_eq!(
        snapshots::apply(&store, id(10), snapshot("K2ABC", 21, Some("Other key"))?).await?,
        SnapshotOutcome::Applied
    );
    assert!(matches!(
        save::contacts::save(
            &store,
            Some(&saved),
            original,
            Some(snapshot(&call, 30, Some("Stale save"))?)
        )
        .await?,
        SaveOutcome::Stale { .. }
    ));
    save::save(&store, qso_key(id(10)), Some(&edited), RecordState::Deleted).await?;
    assert_eq!(
        snapshots::apply(
            &store,
            id(10),
            snapshot("K2ABC", 40, Some("Deleted parent"))?
        )
        .await?,
        SnapshotOutcome::Ineligible
    );
    let row = save::load(&store, key).await?.unwrap();
    let Some(RecordState::Live(RecordValue::Directory(values))) = row.working_state() else {
        panic!("retained")
    };
    assert_eq!(values.snapshots().count(), 2);
    assert_eq!(
        values.snapshot(&lookup).unwrap().lookup_time(),
        UtcTimestamp::new(11, 0)?
    );
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn directory_snapshots_stage_before_parent_and_complete_after_restart() -> anyhow::Result<()>
{
    use clacks_sync::{
        account::{AccountMetadata, InstalledCheckpoint},
        lineage::Coverage,
        record::AcceptedRecord,
        transfer::{
            RecordRequirement, WindowMode,
            discovery::{DiscoveryPage, DiscoveryStart},
        },
    };
    for deleted in [false, true] {
        let temporary = tempfile::tempdir()?;
        let file = temporary.path().join("transfer.sqlite");
        let store = ClientStore::create(&file, id(1), Default::default()).await?;
        let (_, baseline) = AccountMetadata::initialize(id(1), id(2), id(3));
        let live_parent = support::accepted(1, support::contact(qso(id(10))));
        let parent = if deleted {
            AcceptedRecord::new(
                live_parent.key(),
                RecordState::Deleted,
                support::accepted(3, support::contact(qso(id(10)))).stamp,
            )?
        } else {
            live_parent
        };
        let mut facts = clacks_model::directory::record::DirectoryRecord::new(id(10));
        facts.record(snapshot("K1ABC", 10, Some("Retained history"))?)?;
        let directory = support::accepted(
            2,
            RecordState::Live(RecordValue::Directory(Box::new(facts))),
        );
        let checkpoint = InstalledCheckpoint {
            account: id(1),
            baseline: baseline.id,
            coverage: Coverage::from(if deleted {
                parent.stamp.accepted_at()
            } else {
                directory.stamp.accepted_at()
            }),
        };
        let manifest = crate::incoming::Source {
            start: DiscoveryStart {
                source: checkpoint.clone(),
                mode: WindowMode::Full,
            },
            page: DiscoveryPage {
                sequence: 0,
                requirements: [&parent, &directory]
                    .into_iter()
                    .map(|value| RecordRequirement {
                        key: value.key(),
                        stamp: value.stamp.clone(),
                        deleted: matches!(value.state(), RecordState::Deleted),
                    })
                    .collect(),
                accepted: vec![directory.clone()],
                support: vec![],
                baselines: vec![baseline],
                boundaries: vec![],
                units: vec![],
            },
        };
        let incoming = crate::incoming::Incoming::begin(&store, &manifest).await?;
        assert!(
            save::load(&store, directory.key()).await?.is_none(),
            "structural parent identity is not accepted parent evidence"
        );
        assert!(incoming.finish(&store).await.is_err());
        drop(incoming);
        store.close().await?;
        let store = ClientStore::open(&file, id(1), Default::default()).await?;
        assert!(
            clacks_client_store::synchronization::state(&store)
                .await?
                .checkpoint
                .is_none()
        );
        let mut incoming = crate::incoming::Incoming::begin(&store, &manifest).await?;
        incoming.page(&store, std::slice::from_ref(&parent)).await?;
        assert_eq!(incoming.finish(&store).await?, checkpoint);
        assert_eq!(
            save::load(&store, directory.key())
                .await?
                .unwrap()
                .working_state(),
            Some(directory.state())
        );
        assert_eq!(
            save::load(&store, parent.key())
                .await?
                .unwrap()
                .working_state(),
            Some(parent.state())
        );
        store.close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn directory_available_on_idempotent_create_retry_is_saved_atomically() -> anyhow::Result<()>
{
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create(
        &directory.path().join("store.sqlite"),
        id(1),
        Default::default(),
    )
    .await?;
    let contact = qso(id(10));
    save::contacts::save(&store, None, contact.clone(), None).await?;
    let snapshot = snapshot(
        contact.contacted_station().callsign.as_str(),
        30,
        Some("Retry result"),
    )?;
    assert!(matches!(
        save::contacts::save(&store, None, contact.clone(), Some(snapshot.clone())).await?,
        SaveOutcome::Unchanged(_)
    ));
    let stored = save::load(
        &store,
        RecordKey {
            account: id(1),
            id: RecordId::Directory(contact.id()),
        },
    )
    .await?
    .unwrap();
    let Some(RecordState::Live(RecordValue::Directory(values))) = stored.working_state() else {
        panic!("directory");
    };
    assert_eq!(values.snapshot(snapshot.key()), Some(&snapshot));
    store.close().await?;
    Ok(())
}

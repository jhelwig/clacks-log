//! Real PostgreSQL admissions and SQLite full traversal for separate directory facts.

use super::super::support::synchronization::{NativeAccount, qso, transfer};
use crate::store::{metadata, synchronization};
use clacks_model::{
    directory::{
        record::DirectoryRecord,
        values::{
            DirectoryFacts, DirectoryProvider, DirectorySnapshot, DirectorySnapshotFields,
            LookupKey,
        },
    },
    ids::{LookupId, QsoId},
    location::RecordedLocationFields,
    position::{Coordinates, Locator, Position},
    reference::ReferenceAssociation,
    scalar::{RecordedCallsign, UtcTimestamp},
};
use clacks_sync::{
    acceptance::{AcceptanceDecision, RejectionReason},
    record::{Observation, RecordId, RecordKey, RecordState, RecordValue},
};

/// Checked synthetic provider facts exercise all retained child families.
pub(super) fn directory(
    qso: QsoId,
    provider: DirectoryProvider,
    second: i64,
    name: Option<&str>,
) -> anyhow::Result<DirectoryRecord> {
    let mut record = DirectoryRecord::new(qso);
    record.record(DirectorySnapshot::new(DirectorySnapshotFields {
        provider,
        key: LookupKey::from_recorded(&RecordedCallsign::new("K1ABC")?),
        lookup_time: UtcTimestamp::new(second, 123)?,
        lookup_id: LookupId::new(),
        facts: DirectoryFacts {
            name: name.map(str::to_owned),
            location: Some(
                RecordedLocationFields {
                    position: Some(Position::GridArea(Locator::parse("IO91AA")?)),
                    dxcc: Some("291".into()),
                    ..Default::default()
                }
                .try_into()?,
            ),
            references: [ReferenceAssociation::new(
                "01M5QZFPZZ9GPDY819J1WMTJAK".parse()?,
                Some("EU-005".into()),
            )?]
            .into(),
            locality: Some("Fixture locality".into()),
            returned_callsign: Some(RecordedCallsign::new("K1ABC")?),
            supplied_locator: Some(Locator::parse("IO91AA")?),
            supplied_coordinates: Some(Coordinates::from_microdegrees(51_000_000, -1_000_000)?),
            coordinate_source: Some("grid".into()),
        },
    })?)?;
    Ok(record)
}

#[tokio::test]
async fn directory_snapshots_round_trip_without_qso_edits_and_remain_inert_after_deletion()
-> anyhow::Result<()> {
    let native = NativeAccount::create().await?;
    let contact = qso(QsoId::new(), Default::default());
    let parent_key = RecordKey {
        account: native.first.account,
        id: RecordId::Qso(contact.id()),
    };
    let create = native
        .prepare(
            &native.first,
            parent_key,
            RecordState::Live(RecordValue::Qso(Box::new(contact.clone()))),
        )
        .await?;
    let AcceptanceDecision::Commit(parent) =
        synchronization::accept_pending(&native.store, &native.first, &create).await?
    else {
        panic!("parent")
    };
    let key = RecordKey {
        account: parent_key.account,
        id: RecordId::Directory(contact.id()),
    };
    let old = directory(contact.id(), DirectoryProvider::Qrz, 10, Some("Earlier"))?;
    let first = native
        .prepare(
            &native.first,
            key,
            RecordState::Live(RecordValue::Directory(Box::new(old.clone()))),
        )
        .await?;
    assert!(matches!(
        synchronization::accept_pending(&native.store, &native.first, &first).await?,
        AcceptanceDecision::Commit(_)
    ));
    let winner = directory(contact.id(), DirectoryProvider::HamQth, 20, None)?;
    let update = native
        .prepare(
            &native.second,
            key,
            RecordState::Live(RecordValue::Directory(Box::new(winner.clone()))),
        )
        .await?;
    assert!(matches!(
        synchronization::accept_pending(&native.store, &native.second, &update).await?,
        AcceptanceDecision::Commit(_)
    ));
    let late = native
        .prepare(
            &native.first,
            key,
            RecordState::Live(RecordValue::Directory(Box::new(old))),
        )
        .await?;
    let AcceptanceDecision::Commit(late) =
        synchronization::accept_pending(&native.store, &native.first, &late).await?
    else {
        panic!("late input")
    };
    assert!(
        !late.native_changed,
        "arrival order cannot replace the later successful lookup tuple"
    );
    let mut tx = native.store.pool.begin().await?;
    let account = metadata::account::load(&mut tx, parent_key.account).await?;
    assert_eq!(
        synchronization::context::observation(&mut tx, &account, parent_key).await?,
        Observation::Accepted(parent.accepted)
    );
    tx.commit().await?;

    let local_directory = tempfile::tempdir()?;
    let local = clacks_client_store::store::ClientStore::create(
        &local_directory.path().join("before-delete.sqlite"),
        native.first.account,
        Default::default(),
    )
    .await?;
    transfer::bind(&local, &native.first, None).await?;
    transfer::pull(&native.store, &native.first, &local).await?;
    let local_values = directory(
        contact.id(),
        DirectoryProvider::Qrz,
        40,
        Some("Local unpublished lookup"),
    )?;
    let local_snapshot = local_values.snapshots().next().unwrap().clone();
    assert_eq!(
        clacks_client_store::directory::snapshots::apply(&local, contact.id(), local_snapshot)
            .await?,
        clacks_client_store::directory::snapshots::SnapshotOutcome::Applied
    );
    let pending = native
        .prepare(
            &native.second,
            key,
            RecordState::Live(RecordValue::Directory(Box::new(directory(
                contact.id(),
                DirectoryProvider::HamQth,
                30,
                Some("Unpublished lookup"),
            )?))),
        )
        .await?;
    let delete = native
        .prepare(&native.first, parent_key, RecordState::Deleted)
        .await?;
    assert!(matches!(
        synchronization::accept_pending(&native.store, &native.first, &delete).await?,
        AcceptanceDecision::Commit(_)
    ));
    assert!(
        matches!(synchronization::accept_pending(&native.store,&native.second,&pending).await?,AcceptanceDecision::Rejected(RejectionReason::DeletedDirectoryParent { qso }) if qso==contact.id())
    );
    transfer::pull(&native.store, &native.first, &local).await?;
    let clacks_client_store::synchronization::PrepareOutcome::Core(prepared) =
        clacks_client_store::synchronization::prepare(&local, key).await?
    else {
        panic!("enrolled")
    };
    assert!(
        matches!(
            *prepared,
            clacks_sync::client::Preparation::Unchanged { .. }
        ),
        "obsolete lookup work must not become a manual review"
    );
    assert!(
        clacks_client_store::save::load(&local, key)
            .await?
            .unwrap()
            .pending()
            .is_none()
    );
    local.close().await?;

    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("directory.sqlite");
    let client = clacks_client_store::store::ClientStore::create(
        &path,
        native.first.account,
        Default::default(),
    )
    .await?;
    transfer::bind(&client, &native.second, None).await?;
    transfer::pull(&native.store, &native.second, &client).await?;
    client.close().await?;
    let client = clacks_client_store::store::ClientStore::open(
        &path,
        native.first.account,
        Default::default(),
    )
    .await?;
    let received = clacks_client_store::save::load(&client, key)
        .await?
        .unwrap();
    assert_eq!(
        received.working_state(),
        Some(&RecordState::Live(RecordValue::Directory(Box::new(winner))))
    );
    assert_eq!(
        clacks_client_store::save::load(&client, parent_key)
            .await?
            .unwrap()
            .working_state(),
        Some(&RecordState::Deleted)
    );
    assert!(received.pending().is_none());
    client.close().await?;
    Ok(())
}

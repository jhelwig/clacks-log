//! Guarded resolution of actual saved conflicts, including live dependencies.

use crate::{logbook_support, logbook_transfer::remember, support};
use clacks_client_store::{
    read,
    save::{self, SaveOutcome},
    synchronization,
};
use clacks_sync::{account::AccountMetadata, record::RecordState};

#[tokio::test]
async fn resolution_save_rechecks_basis_and_live_references() -> anyhow::Result<()> {
    let workspace = logbook_support::Workspace::new().await?;
    let store = &workspace.store;
    let (_, baseline) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let original = support::qso(support::id(100));
    let accepted = support::accepted(1, support::contact(original.clone()));
    let mut incoming = remember(store, &accepted, baseline).await?;
    incoming
        .page(store, std::slice::from_ref(&accepted))
        .await?;
    incoming.finish(store).await?;
    let before = save::load(store, accepted.key()).await?.unwrap();
    let mut local = original.clone();
    support::edit(&mut local, |qso| {
        qso.comment = Some("Local correction".into());
        Ok(())
    });
    save::save(
        store,
        accepted.key(),
        Some(&before),
        support::contact(local.clone()),
    )
    .await?;
    let mut shared = original;
    support::edit(&mut shared, |qso| {
        qso.comment = Some("Shared correction".into());
        Ok(())
    });
    synchronization::receive(store, &support::accepted(2, support::contact(shared))).await?;
    let mut snapshot = read::snapshot(store).await?;
    let row = snapshot.review_record(accepted.key()).await?.unwrap();
    snapshot.close().await?;
    let opened = row;
    support::edit(&mut local, |qso| {
        qso.comment = Some("Later local correction".into());
        Ok(())
    });
    save::save(
        store,
        accepted.key(),
        Some(&opened.record),
        support::contact(local.clone()),
    )
    .await?;
    assert!(matches!(
        synchronization::resolve(
            store,
            &opened.record,
            opened.review.as_ref().unwrap(),
            support::contact(local.clone())
        )
        .await?,
        SaveOutcome::Stale { .. }
    ));
    let mut snapshot = read::snapshot(store).await?;
    let row = snapshot.review_record(accepted.key()).await?.unwrap();
    snapshot.close().await?;
    let shown = row.review.as_ref().unwrap();
    assert_eq!(
        Some(&shown.local),
        row.record.working_state(),
        "ordinary edits must refresh the compared local values without resolving the conflict"
    );
    let tag = logbook_support::insert(store, 30, "Portable").await?;

    support::edit(&mut local, |qso| {
        qso.tags.insert(support::id(30));
        Ok(())
    });
    // A new local dependency is valid before it has ever been published.
    let chosen = support::contact(local);
    let missing_tag = support::id(31);
    let mut missing = match &chosen {
        RecordState::Live(clacks_sync::record::RecordValue::Qso(qso)) => qso.as_ref().clone(),
        _ => panic!("contact"),
    };
    support::edit(&mut missing, |qso| {
        qso.tags.insert(missing_tag);
        Ok(())
    });
    assert!(
        synchronization::resolve(store, &row.record, shown, support::contact(missing))
            .await
            .is_err()
    );
    assert_eq!(
        save::load(store, accepted.key()).await?,
        Some(row.record.clone())
    );
    // A tag deleted after opening the comparison cannot be saved back into a contact.
    save::save(store, tag.key(), Some(&tag), RecordState::Deleted).await?;
    assert!(
        synchronization::resolve(store, &row.record, shown, chosen.clone())
            .await
            .is_err()
    );
    let deleted_tag = save::load(store, tag.key()).await?.unwrap();
    let restored = clacks_model::tag::LogbookTag::new(support::id(30), "Portable".into());
    save::save(
        store,
        tag.key(),
        Some(&deleted_tag),
        RecordState::Live(clacks_sync::record::RecordValue::Tag(Box::new(restored))),
    )
    .await?;
    let SaveOutcome::Saved(resolved) =
        synchronization::resolve(store, &row.record, shown, chosen.clone()).await?
    else {
        panic!("saved")
    };
    assert_eq!(resolved.working_state(), Some(&chosen));
    assert!(resolved.pending().is_some());
    assert!(
        matches!(synchronization::resolve(store, &row.record, shown, chosen).await?, SaveOutcome::Stale { current: Some(current) } if current == resolved)
    );
    let mut snapshot = read::snapshot(store).await?;
    assert!(
        snapshot
            .review_record(accepted.key())
            .await?
            .unwrap()
            .review
            .is_none()
    );
    snapshot.close().await?;
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn saved_source_resolution_does_not_apply_operating_values() -> anyhow::Result<()> {
    use crate::settings_support;
    use clacks_client_store::{
        operating::{self, OperatingState},
        settings,
    };
    use clacks_sync::record::RecordValue;
    let workspace = logbook_support::Workspace::new().await?;
    let store = &workspace.store;
    let equipment = settings_support::equipment(40, "Rig A");
    settings::save_equipment(store, None, &equipment).await?;
    let profile = settings_support::profile(41, None);
    let original = support::accepted(
        1,
        RecordState::Live(RecordValue::Profile(Box::new(profile.clone()))),
    );
    let (_, baseline) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let mut incoming = remember(store, &original, baseline).await?;
    incoming
        .page(store, std::slice::from_ref(&original))
        .await?;
    incoming.finish(store).await?;
    let active = settings_support::operating(&profile, &equipment);
    operating::save(store, &OperatingState::default(), &active).await?;
    let history = support::qso(support::id(101));
    let history_key = support::qso_key(history.id());
    save::save(store, history_key, None, support::contact(history)).await?;
    let history_before = save::load(store, history_key).await?;
    let before = save::load(store, original.key()).await?.unwrap();
    let mut local = profile.clone().into_fields();
    local.name = "Local name".into();
    settings::save_profile(store, Some(&before), &local.try_into()?).await?;
    let mut remote = profile.into_fields();
    remote.name = "Shared name".into();
    let shared = support::accepted(
        2,
        RecordState::Live(RecordValue::Profile(Box::new(remote.clone().try_into()?))),
    );
    synchronization::receive(store, &shared).await?;
    let mut snapshot = read::snapshot(store).await?;
    let row = snapshot.review_record(original.key()).await?.unwrap();
    snapshot.close().await?;
    remote.default_equipment = Some(equipment.id());
    let chosen = RecordState::Live(RecordValue::Profile(Box::new(remote.try_into()?)));
    assert!(
        matches!(synchronization::resolve(store, &row.record, row.review.as_ref().unwrap(), chosen.clone()).await?, SaveOutcome::Saved(record) if record.working_state() == Some(&chosen))
    );
    assert_eq!(operating::load(store).await?, active);
    assert_eq!(save::load(store, history_key).await?, history_before);
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn blocked_deletion_waits_for_accepted_reference_repair() -> anyhow::Result<()> {
    use clacks_client_store::synchronization::RetryDeletionOutcome;
    use clacks_sync::{acceptance::RejectionReason, record::RecordValue};
    let workspace = logbook_support::Workspace::new().await?;
    let store = &workspace.store;
    let tag = clacks_model::tag::LogbookTag::new(support::id(50), "Field day".into());
    let accepted = support::accepted(1, RecordState::Live(RecordValue::Tag(Box::new(tag))));
    let (_, baseline) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let mut incoming = remember(store, &accepted, baseline).await?;
    incoming
        .page(store, std::slice::from_ref(&accepted))
        .await?;
    incoming.finish(store).await?;
    let mut referring = support::qso(support::id(110));
    support::edit(&mut referring, |qso| {
        qso.tags.insert(support::id(50));
        Ok(())
    });
    let referring = support::accepted(2, support::contact(referring));
    synchronization::receive(store, &referring).await?;
    let before = save::load(store, accepted.key()).await?.unwrap();
    save::save(store, accepted.key(), Some(&before), RecordState::Deleted).await?;
    synchronization::reject(
        store,
        accepted.key(),
        &RejectionReason::Referenced {
            key: accepted.key(),
            referrers: vec![referring.key()],
            total: 1,
        },
    )
    .await?;
    let mut second = support::qso(support::id(111));
    support::edit(&mut second, |qso| {
        qso.tags.insert(support::id(50));
        Ok(())
    });
    synchronization::receive(
        store,
        &support::accepted(3, support::contact(second.clone())),
    )
    .await?;
    let mut snapshot = read::snapshot(store).await?;

    let target = snapshot.review_record(accepted.key()).await?.unwrap();
    let references = snapshot.review_references(accepted.key(), None, 1).await?;
    let next = snapshot
        .review_references(accepted.key(), references.next, 1)
        .await?;
    assert!(references.next.is_some());
    assert_eq!(next.total, 2);
    assert_eq!(next.rows.len(), 1);
    assert_ne!(
        next.rows.first().unwrap().record.key(),
        references.rows.first().unwrap().record.key()
    );
    snapshot.close().await?;
    support::edit(&mut second, |qso| {
        qso.tags.clear();
        Ok(())
    });
    synchronization::receive(store, &support::accepted(4, support::contact(second))).await?;

    assert_eq!(references.total, 2);
    assert_eq!(
        references.rows.first().unwrap().record.key(),
        referring.key()
    );
    let before = save::load(store, referring.key()).await?.unwrap();
    let RecordState::Live(RecordValue::Qso(qso)) = referring.state() else {
        panic!("contact")
    };
    let mut repaired = qso.as_ref().clone();
    support::edit(&mut repaired, |qso| {
        qso.tags.clear();
        Ok(())
    });
    save::save(
        store,
        referring.key(),
        Some(&before),
        support::contact(repaired.clone()),
    )
    .await?;
    let shown = target.review.as_ref().unwrap();
    assert!(
        matches!(synchronization::retry_deletion(store, &target.record, shown).await?, RetryDeletionOutcome::StillReferenced { page } if page.total == 1)
    );
    synchronization::receive(store, &support::accepted(5, support::contact(repaired))).await?;
    let RetryDeletionOutcome::Ready { record } =
        synchronization::retry_deletion(store, &target.record, shown).await?
    else {
        panic!("ready")
    };
    assert_eq!(record.working_state(), Some(&RecordState::Deleted));
    assert!(record.pending().unwrap().prepared.is_none());
    assert!(matches!(
        synchronization::retry_deletion(store, &target.record, shown).await?,
        RetryDeletionOutcome::Stale { .. }
    ));
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn equipment_deletion_uses_accepted_profile_references() -> anyhow::Result<()> {
    use crate::settings_support;
    use clacks_client_store::synchronization::RetryDeletionOutcome;
    use clacks_sync::{acceptance::RejectionReason, record::RecordValue};
    let workspace = logbook_support::Workspace::new().await?;
    let store = &workspace.store;
    let equipment = settings_support::equipment(60, "Rig A");
    let accepted = support::accepted(
        1,
        RecordState::Live(RecordValue::Equipment(Box::new(equipment))),
    );
    let (_, baseline) = AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let mut incoming = remember(store, &accepted, baseline).await?;
    incoming
        .page(store, std::slice::from_ref(&accepted))
        .await?;
    incoming.finish(store).await?;
    let row = save::load(store, accepted.key()).await?.unwrap();
    save::save(store, accepted.key(), Some(&row), RecordState::Deleted).await?;
    let profile = settings_support::profile(61, Some(60));
    let referring = support::accepted(
        2,
        RecordState::Live(RecordValue::Profile(Box::new(profile.clone()))),
    );
    synchronization::receive(store, &referring).await?;
    synchronization::reject(
        store,
        accepted.key(),
        &RejectionReason::Referenced {
            key: accepted.key(),
            referrers: vec![referring.key()],
            total: 1,
        },
    )
    .await?;
    let mut snapshot = read::snapshot(store).await?;
    let target = snapshot.review_record(accepted.key()).await?.unwrap();
    let page = snapshot.review_references(accepted.key(), None, 10).await?;
    snapshot.close().await?;
    assert_eq!(page.rows.first().unwrap().record.key(), referring.key());
    let mut repaired = profile.into_fields();
    repaired.default_equipment = None;
    let chosen = RecordState::Live(RecordValue::Profile(Box::new(repaired.try_into()?)));
    let before = save::load(store, referring.key()).await?.unwrap();
    save::save(store, referring.key(), Some(&before), chosen.clone()).await?;
    let shown = target.review.as_ref().unwrap();
    assert!(
        matches!(synchronization::retry_deletion(store, &target.record, shown).await?, RetryDeletionOutcome::StillReferenced { page } if page.total == 1)
    );
    synchronization::receive(store, &support::accepted(3, chosen)).await?;
    assert!(matches!(
        synchronization::retry_deletion(store, &target.record, shown).await?,
        RetryDeletionOutcome::Ready { .. }
    ));
    store.close().await?;
    Ok(())
}

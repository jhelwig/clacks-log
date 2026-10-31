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
    let shown = row.review.as_ref().unwrap();
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

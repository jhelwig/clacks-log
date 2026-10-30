use crate::logbook_support;
use crate::support;

use clacks_client_store::{
    logbooks::{
        deletion::{self, TagDeleteOutcome},
        selection::{self, SelectedView},
    },
    save::{self, SaveOutcome},
    settings,
};
use clacks_model::tag::TagDeletionChoice;
use clacks_sync::record::RecordState;
use logbook_support::{Workspace, insert};
use support::{contact, id, qso, qso_key};

#[tokio::test]
async fn effective_tag_deletion_clears_only_the_matching_selected_kind() -> anyhow::Result<()> {
    let workspace = Workspace::new().await?;
    let tag = insert(&workspace.store, 20, "Main").await?;
    selection::select(&workspace.store, SelectedView::Logbook(id(20))).await?;
    let SaveOutcome::Saved(qso) = save::save(
        &workspace.store,
        qso_key(id(20)),
        None,
        contact(qso(id(20))),
    )
    .await?
    else {
        panic!("create");
    };
    save::save(
        &workspace.store,
        qso.key(),
        Some(&qso),
        RecordState::Deleted,
    )
    .await?;
    assert_eq!(
        settings::snapshot(&workspace.store)
            .await?
            .logbooks
            .selected_view,
        SelectedView::Logbook(id(20))
    );
    save::save(
        &workspace.store,
        tag.key(),
        Some(&tag),
        RecordState::Deleted,
    )
    .await?;
    assert_eq!(
        settings::snapshot(&workspace.store)
            .await?
            .logbooks
            .selected_view,
        SelectedView::AllContacts
    );
    workspace.store.close().await?;
    Ok(())
}

#[tokio::test]
async fn unrelated_fact_edit_invalidates_exact_deletion_basis() -> anyhow::Result<()> {
    let workspace = Workspace::new().await?;
    insert(&workspace.store, 20, "Main").await?;
    let mut value = qso(id(100));
    support::edit(&mut value, |value| {
        value.tags = [id(20)].into();
        Ok(())
    });
    let SaveOutcome::Saved(row) = save::save(
        &workspace.store,
        qso_key(id(100)),
        None,
        contact(value.clone()),
    )
    .await?
    else {
        panic!("create");
    };
    let preview = deletion::preview(&workspace.store, id(20)).await?;
    support::edit(&mut value, |value| {
        value.contacted_station.callsign = clacks_model::scalar::RecordedCallsign::new("W1AW")?;
        Ok(())
    });
    let current = save::save(&workspace.store, row.key(), Some(&row), contact(value)).await?;
    let SaveOutcome::Saved(current) = current else {
        panic!("updated contact")
    };
    let before = settings::snapshot(&workspace.store).await?;
    assert!(matches!(
        deletion::delete_tag(&workspace.store, &preview, TagDeletionChoice::DeleteAll).await?,
        TagDeleteOutcome::Stale { .. }
    ));
    assert_eq!(settings::snapshot(&workspace.store).await?, before);
    assert_eq!(
        save::load(&workspace.store, row.key()).await?,
        Some(current)
    );
    workspace.store.close().await?;
    Ok(())
}

#[tokio::test]
async fn unchanged_counts_do_not_validate_changed_preview_members() -> anyhow::Result<()> {
    let workspace = Workspace::new().await?;
    insert(&workspace.store, 20, "Main").await?;
    let mut value = qso(id(100));
    support::edit(&mut value, |value| {
        value.tags = [id(20)].into();
        Ok(())
    });
    let SaveOutcome::Saved(first) =
        save::save(&workspace.store, qso_key(id(100)), None, contact(value)).await?
    else {
        panic!("create");
    };
    let preview = deletion::preview(&workspace.store, id(20)).await?;
    save::save(
        &workspace.store,
        first.key(),
        Some(&first),
        RecordState::Deleted,
    )
    .await?;
    let mut replacement = qso(id(101));
    support::edit(&mut replacement, |replacement| {
        replacement.tags = [id(20)].into();
        Ok(())
    });
    save::save(
        &workspace.store,
        qso_key(id(101)),
        None,
        contact(replacement),
    )
    .await?;
    assert_eq!(
        deletion::preview(&workspace.store, id(20)).await?.counts,
        preview.counts
    );
    assert!(matches!(
        deletion::delete_tag(&workspace.store, &preview, TagDeletionChoice::DeleteAll).await?,
        TagDeleteOutcome::Stale { .. }
    ));
    assert_eq!(
        save::load(&workspace.store, first.key())
            .await?
            .unwrap()
            .working_state(),
        Some(&RecordState::Deleted)
    );
    assert!(matches!(
        save::load(&workspace.store, qso_key(id(101)))
            .await?
            .unwrap()
            .working_state(),
        Some(RecordState::Live(_))
    ));
    workspace.store.close().await?;
    Ok(())
}

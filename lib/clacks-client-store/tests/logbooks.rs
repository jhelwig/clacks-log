use crate::logbook_support;
use crate::support;

use clacks_client_store::{
    logbooks::selection::{self, SelectedView},
    read,
    save::{self, SaveOutcome},
    settings,
    store::{ClientStore, StoreSettings},
};
use clacks_model::tag::LogbookTag;
use clacks_sync::record::{RecordState, RecordValue};
use logbook_support::{Workspace, insert};
use sqlx::{ConnectOptions, Connection, sqlite::SqliteConnectOptions};
use support::id;

#[tokio::test]
async fn selection_and_deletion_preserve_other_local_state() -> anyhow::Result<()> {
    use clacks_client_store::logbooks::deletion;
    use clacks_model::tag::TagDeletionChoice;

    let workspace = Workspace::new().await?;
    let mut connection = SqliteConnectOptions::new()
        .filename(&workspace.path)
        .connect()
        .await?;
    sqlx::raw_sql(include_str!("sql/other_local_state.sql"))
        .execute(&mut connection)
        .await?;

    insert(&workspace.store, 20, "Main").await?;
    selection::select(&workspace.store, SelectedView::Logbook(id(20))).await?;
    let preview = deletion::preview(&workspace.store, id(20)).await?;
    deletion::delete_tag(&workspace.store, &preview, TagDeletionChoice::LeaveAll).await?;
    assert_eq!(
        settings::snapshot(&workspace.store)
            .await?
            .logbooks
            .selected_view,
        SelectedView::AllContacts
    );
    let value = sqlx::query_file!("tests/sql/read_other_local_state.sql")
        .fetch_one(&mut connection)
        .await?;
    assert_eq!(value.value, r#"{"theme":"light"}"#);

    connection.close().await?;
    workspace.store.close().await?;
    Ok(())
}

#[tokio::test]
async fn fresh_workspace_defaults_to_all_contacts() -> anyhow::Result<()> {
    let workspace = Workspace::new().await?;
    assert_eq!(
        settings::snapshot(&workspace.store)
            .await?
            .logbooks
            .selected_view,
        SelectedView::AllContacts
    );
    assert_eq!(selection::restore(&workspace.store).await?, None);
    workspace.store.close().await?;

    Ok(())
}

#[tokio::test]
async fn selected_view_is_local_durable_and_snapshot_consistent() -> anyhow::Result<()> {
    let workspace = Workspace::new().await?;
    let tag = insert(&workspace.store, 20, "Main").await?;
    let other = Workspace::new().await?;
    let mut before = read::snapshot(&workspace.store).await?;
    let old = before.settings().await?;

    selection::select(&workspace.store, SelectedView::Logbook(id(20))).await?;
    let renamed = LogbookTag::new(id(20), "Renamed".into());
    assert!(matches!(
        save::save(
            &workspace.store,
            tag.key,
            Some(&tag),
            RecordState::Live(RecordValue::Tag(Box::new(renamed)))
        )
        .await?,
        SaveOutcome::Saved(_)
    ));
    assert_eq!(before.settings().await?, old);
    before.close().await?;
    let current = settings::snapshot(&workspace.store).await?;
    assert_eq!(
        current.logbooks.selected_view,
        SelectedView::Logbook(id(20))
    );
    assert_eq!(
        settings::snapshot(&other.store)
            .await?
            .logbooks
            .selected_view,
        SelectedView::AllContacts
    );
    workspace.store.close().await?;

    let reopened = ClientStore::open(&workspace.path, id(1), StoreSettings::default()).await?;
    assert_eq!(
        settings::snapshot(&reopened).await?.logbooks,
        current.logbooks
    );
    selection::select(&reopened, SelectedView::Untagged).await?;
    reopened.close().await?;
    let reopened = ClientStore::open(&workspace.path, id(1), StoreSettings::default()).await?;
    assert_eq!(
        settings::snapshot(&reopened).await?.logbooks.selected_view,
        SelectedView::Untagged
    );
    reopened.close().await?;
    other.store.close().await?;
    Ok(())
}

#[tokio::test]
async fn selection_rejects_deleted_or_unavailable_targets_without_replacing_current_view()
-> anyhow::Result<()> {
    let workspace = Workspace::new().await?;
    insert(&workspace.store, 20, "Keep").await?;
    let removed = insert(&workspace.store, 21, "Removed").await?;
    selection::select(&workspace.store, SelectedView::Logbook(id(20))).await?;
    save::save(
        &workspace.store,
        removed.key,
        Some(&removed),
        RecordState::Deleted,
    )
    .await?;

    for target in [21, 99] {
        assert!(
            selection::select(&workspace.store, SelectedView::Logbook(id(target)))
                .await
                .is_err()
        );
        assert_eq!(
            settings::snapshot(&workspace.store)
                .await?
                .logbooks
                .selected_view,
            SelectedView::Logbook(id(20))
        );
    }
    workspace.store.close().await?;
    Ok(())
}

#[tokio::test]
async fn restore_preserves_unknown_target_and_repairs_only_known_deletion() -> anyhow::Result<()> {
    let workspace = Workspace::new().await?;
    let mut connection = SqliteConnectOptions::new()
        .filename(&workspace.path)
        .connect()
        .await?;
    sqlx::raw_sql(include_str!("sql/missing_selected_view.sql"))
        .execute(&mut connection)
        .await?;
    assert_eq!(selection::restore(&workspace.store).await?, None);
    assert_eq!(
        settings::snapshot(&workspace.store)
            .await?
            .logbooks
            .selected_view,
        SelectedView::Logbook(id(99))
    );

    let tag = insert(&workspace.store, 99, "Deleted").await?;
    save::save(&workspace.store, tag.key, Some(&tag), RecordState::Deleted).await?;
    sqlx::raw_sql(include_str!("sql/missing_selected_view.sql"))
        .execute(&mut connection)
        .await?;
    assert_eq!(selection::restore(&workspace.store).await?, Some(id(99)));
    assert_eq!(
        settings::snapshot(&workspace.store)
            .await?
            .logbooks
            .selected_view,
        SelectedView::AllContacts
    );
    connection.close().await?;
    workspace.store.close().await?;
    Ok(())
}

#[tokio::test]
async fn invalid_selected_view_does_not_reset_the_workspace() -> anyhow::Result<()> {
    let workspace = Workspace::new().await?;
    let tag = insert(&workspace.store, 20, "Preserved").await?;
    let mut connection = SqliteConnectOptions::new()
        .filename(&workspace.path)
        .connect()
        .await?;
    sqlx::raw_sql(include_str!("sql/invalid_selected_view.sql"))
        .execute(&mut connection)
        .await?;

    assert!(settings::snapshot(&workspace.store).await.is_err());
    assert!(selection::restore(&workspace.store).await.is_err());
    assert_eq!(save::load(&workspace.store, tag.key).await?, Some(tag));
    assert_eq!(workspace.store.account_id(), id(1));
    assert!(selection::restore(&workspace.store).await.is_err());
    connection.close().await?;
    workspace.store.close().await?;
    Ok(())
}

#[tokio::test]
async fn client_local_selection_does_not_make_an_empty_workspace_populated() -> anyhow::Result<()> {
    use clacks_client_store::identity::{self, EnrollmentBinding};

    let workspace = Workspace::new().await?;
    selection::select(&workspace.store, SelectedView::Untagged).await?;
    identity::bind(
        &workspace.store,
        None,
        EnrollmentBinding {
            account: id(1),
            server: "https://clacks.invalid".into(),
            actor: id(2),
            credential_reference: "keychain:fixture".into(),
        },
    )
    .await?;

    assert!(identity::status(&workspace.store).await?.enrolled);
    assert_eq!(
        settings::snapshot(&workspace.store)
            .await?
            .logbooks
            .selected_view,
        SelectedView::Untagged
    );
    workspace.store.close().await?;
    Ok(())
}

#[tokio::test]
async fn tag_save_preserves_identity_selection_and_pending_basis() -> anyhow::Result<()> {
    use clacks_client_store::logbooks::tags;
    use clacks_model::tag::AutomaticUploadPolicy;

    let workspace = Workspace::new().await?;
    let mut tag = LogbookTag::new(id(20), "Main".into());
    let SaveOutcome::Saved(first) = tags::save(&workspace.store, None, &tag).await? else {
        panic!("fresh tag must be saved");
    };
    let other = LogbookTag::new(id(21), "Main".into());
    let SaveOutcome::Saved(other_row) = tags::save(&workspace.store, None, &other).await? else {
        panic!("duplicate names must be allowed");
    };
    selection::select(&workspace.store, SelectedView::Logbook(tag.id)).await?;
    let before = settings::snapshot(&workspace.store).await?;

    tag.name = "Renamed".into();
    tag.automatic_upload.qrz = AutomaticUploadPolicy::Forbid;
    let SaveOutcome::Saved(updated) = tags::save(&workspace.store, Some(&first), &tag).await?
    else {
        panic!("changed tag must be saved");
    };
    assert_eq!(updated.key, first.key);
    assert_eq!(
        updated.pending.as_ref().unwrap().original,
        first.pending.as_ref().unwrap().original
    );
    assert_eq!(
        tags::save(&workspace.store, Some(&updated), &tag).await?,
        SaveOutcome::Unchanged(updated.clone())
    );
    assert_eq!(
        save::load(&workspace.store, other_row.key).await?,
        Some(other_row)
    );
    let after = settings::snapshot(&workspace.store).await?;
    assert_eq!(after.logbooks.selected_view, before.logbooks.selected_view);
    assert_eq!(after.operating, before.operating);
    workspace.store.close().await?;
    Ok(())
}

#[tokio::test]
async fn tag_editor_save_rejects_blank_names_and_stale_or_mismatched_bases() -> anyhow::Result<()> {
    use clacks_client_store::logbooks::tags;

    let workspace = Workspace::new().await?;
    let mut tag = LogbookTag::new(id(20), " \t\n".into());
    assert!(tags::save(&workspace.store, None, &tag).await.is_err());
    assert!(
        settings::snapshot(&workspace.store)
            .await?
            .logbooks
            .tags
            .is_empty()
    );
    tag.name = "  Preserved name  ".into();
    let SaveOutcome::Saved(first) = tags::save(&workspace.store, None, &tag).await? else {
        panic!("nonblank name must save");
    };
    assert_eq!(
        first.working_state(),
        Some(&RecordState::Live(RecordValue::Tag(Box::new(tag.clone()))))
    );
    tag.name = "Changed".into();
    let SaveOutcome::Saved(current) = tags::save(&workspace.store, Some(&first), &tag).await?
    else {
        panic!("rename must save");
    };
    assert!(matches!(
        tags::save(&workspace.store, Some(&first), &tag).await?,
        SaveOutcome::Stale { .. }
    ));
    tag.id = id(21);
    assert!(
        tags::save(&workspace.store, Some(&current), &tag)
            .await
            .is_err()
    );
    let mut wrong_account = current.clone();
    wrong_account.key.account = id(99);
    tag.id = id(20);
    assert!(
        tags::save(&workspace.store, Some(&wrong_account), &tag)
            .await
            .is_err()
    );
    assert_eq!(
        save::load(&workspace.store, current.key).await?,
        Some(current)
    );
    workspace.store.close().await?;
    Ok(())
}

#[path = "support/logbooks.rs"]
mod logbook_support;
mod support;

use clacks_client_store::{
    logbooks::{
        deletion::{self, TagDeleteOutcome, TagDeletionCounts},
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
async fn preview_counts_distinct_active_contacts_without_changing_selection() -> anyhow::Result<()>
{
    let workspace = Workspace::new().await?;
    insert(&workspace.store, 20, "Main").await?;
    selection::select(&workspace.store, SelectedView::Logbook(id(20))).await?;
    for n in 100..201 {
        let mut value = qso(id(n));
        value.tags = if n < 130 {
            [id(20), id(21)].into()
        } else {
            [id(20)].into()
        };
        let SaveOutcome::Saved(row) =
            save::save(&workspace.store, qso_key(id(n)), None, contact(value)).await?
        else {
            panic!("create");
        };
        if n == 200 {
            save::save(&workspace.store, row.key, Some(&row), RecordState::Deleted).await?;
        }
    }
    let preview = deletion::preview(&workspace.store, id(20)).await?;
    assert_eq!(
        preview.counts,
        TagDeletionCounts {
            affected: 100,
            also_tagged_elsewhere: 30,
            unique: 70
        }
    );
    assert_eq!(
        settings::snapshot(&workspace.store)
            .await?
            .logbooks
            .selected_view,
        SelectedView::Logbook(id(20))
    );
    workspace.store.close().await?;
    Ok(())
}

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
    save::save(&workspace.store, qso.key, Some(&qso), RecordState::Deleted).await?;
    assert_eq!(
        settings::snapshot(&workspace.store)
            .await?
            .logbooks
            .selected_view,
        SelectedView::Logbook(id(20))
    );
    save::save(&workspace.store, tag.key, Some(&tag), RecordState::Deleted).await?;
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
    value.tags = [id(20)].into();
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
    value.contacted_station.callsign = clacks_model::scalar::RecordedCallsign::new("W1AW")?;
    save::save(&workspace.store, row.key, Some(&row), contact(value)).await?;
    let before = settings::snapshot(&workspace.store).await?;
    assert!(matches!(
        deletion::delete_tag(&workspace.store, &preview, TagDeletionChoice::DeleteAll).await?,
        TagDeleteOutcome::Stale { .. }
    ));
    assert_eq!(settings::snapshot(&workspace.store).await?, before);
    workspace.store.close().await?;
    Ok(())
}

#[tokio::test]
async fn deletion_and_fallback_rollback_together_on_late_failure() -> anyhow::Result<()> {
    use clacks_client_store::store::{ClientStore, StoreSettings};
    use sqlx::{ConnectOptions, Connection, sqlite::SqliteConnectOptions};

    for failure in [
        include_str!("sql/fail_logbook_delete.sql"),
        include_str!("sql/fail_selected_view_fallback.sql"),
    ] {
        let workspace = Workspace::new().await?;
        let tag = insert(&workspace.store, 20, "Main").await?;
        selection::select(&workspace.store, SelectedView::Logbook(id(20))).await?;
        let mut value = qso(id(100));
        value.tags = [id(20)].into();
        let SaveOutcome::Saved(member) =
            save::save(&workspace.store, qso_key(id(100)), None, contact(value)).await?
        else {
            panic!("create");
        };
        let preview = deletion::preview(&workspace.store, id(20)).await?;
        let mut connection = SqliteConnectOptions::new()
            .filename(&workspace.path)
            .connect()
            .await?;
        sqlx::raw_sql(failure).execute(&mut connection).await?;
        assert!(
            deletion::delete_tag(&workspace.store, &preview, TagDeletionChoice::DeleteAll)
                .await
                .is_err()
        );
        connection.close().await?;
        workspace.store.close().await?;

        let reopened = ClientStore::open(&workspace.path, id(1), StoreSettings::default()).await?;
        assert_eq!(save::load(&reopened, tag.key).await?, Some(tag));
        assert_eq!(save::load(&reopened, member.key).await?, Some(member));
        assert_eq!(
            settings::snapshot(&reopened).await?.logbooks.selected_view,
            SelectedView::Logbook(id(20))
        );
        reopened.close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn unchanged_counts_do_not_validate_changed_preview_members() -> anyhow::Result<()> {
    let workspace = Workspace::new().await?;
    insert(&workspace.store, 20, "Main").await?;
    let mut value = qso(id(100));
    value.tags = [id(20)].into();
    let SaveOutcome::Saved(first) =
        save::save(&workspace.store, qso_key(id(100)), None, contact(value)).await?
    else {
        panic!("create");
    };
    let preview = deletion::preview(&workspace.store, id(20)).await?;
    save::save(
        &workspace.store,
        first.key,
        Some(&first),
        RecordState::Deleted,
    )
    .await?;
    let mut replacement = qso(id(101));
    replacement.tags = [id(20)].into();
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
    workspace.store.close().await?;
    Ok(())
}

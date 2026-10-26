use super::*;
use crate::ui::tests::logbooks_support::{LiveWindow, seed_contacts};
use clacks_client_store::{logbooks::deletion, save};
use sqlx::Connection;

fn visible(app: &LiveWindow, value: &str) -> bool {
    app.output.shapes.iter().any(|shape| matches!(&shape.shape, egui::epaint::Shape::Text(text) if text.galley.text().contains(value)))
}

#[tokio::test(flavor = "multi_thread")]
async fn deletion_result_and_fallback_survive_dialog_dismissal() -> anyhow::Result<()> {
    for dismissed in [false, true] {
        let mut app = LiveWindow::new().await?;
        let id = seed_contacts(&mut app).await?;
        app.click("All contacts");
        app.click("Main");
        app.settle().await?;
        app.click("Manage logbooks…");
        app.click("Delete logbook");
        app.settle().await?;
        app.click("Delete all contacts (delete 2)");
        let mut connection = sqlx::SqliteConnection::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new().filename(&app.paths.database),
        )
        .await?;
        let held = connection.begin_with("BEGIN IMMEDIATE").await?;
        app.click("Confirm deletion");
        if dismissed {
            app.frame(vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]);
        }
        held.rollback().await?;
        connection.close().await?;
        app.settle().await?;
        assert!(deletion::preview(&app.store, id).await.is_err());
        assert!(visible(&app, "Deleted 2 contacts"));
        assert!(visible(&app, "was deleted. Showing All contacts."));
        assert!(visible(&app, "All contacts"));
        if !dismissed {
            app.click("Close manager");
        }
        app.click("Dismiss");
        app.frame(vec![]);
        assert!(!visible(&app, "was deleted. Showing All contacts."));
        app.close().await?;
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn effective_deleted_selection_is_presented_once_without_repairing_pending_qsos()
-> anyhow::Result<()> {
    use clacks_client_store::{
        logbooks::{selection, tags},
        synchronization, transfer,
    };
    use clacks_model::ids::{BaselineId, TimelineId};
    use clacks_sync::{
        account::{AccountMetadata, InstalledCheckpoint},
        clock::{ClockActor, RecordVector},
        lineage::Coverage,
        record::{AcceptedRecord, Position, RecordKey, VersionStamp},
        transfer::{RecordRequirement, WindowManifest, WindowMode},
    };
    for pending in [false, true] {
        let mut app = LiveWindow::new().await?;
        let id = seed_contacts(&mut app).await?;
        let preview = deletion::preview(&app.store, id).await?;
        let tag = tag(&preview.tag).unwrap().clone();
        let account = app.store.account_id();
        let timeline = TimelineId::new();
        let (_, baseline) = AccountMetadata::initialize(account, timeline, BaselineId::new());
        let key = RecordKey {
            account,
            id: RecordId::Tag(id),
        };
        let live = AcceptedRecord::new(
            key,
            RecordState::Live(RecordValue::Tag(Box::new(tag.clone()))),
            VersionStamp {
                accepted_at: Position {
                    timeline,
                    revision: 1,
                },
                baseline: baseline.id,
                vector: RecordVector::from_entries([(ClockActor::Server(timeline), 1)])?,
            },
        )?;
        let source = InstalledCheckpoint {
            account,
            baseline: baseline.id,
            coverage: Coverage::from(live.stamp.accepted_at),
        };
        let manifest = WindowManifest::new(
            account,
            source.clone(),
            WindowMode::Full,
            vec![RecordRequirement {
                key,
                stamp: live.stamp.clone(),
                deleted: false,
            }],
            vec![],
            vec![baseline],
            vec![],
        )?;
        transfer::remember(&app.store, &manifest).await?;
        transfer::install(
            &app.store,
            &source,
            &WindowMode::Full,
            std::slice::from_ref(&live),
        )
        .await?;
        transfer::finish(&app.store, &source, &WindowMode::Full).await?;
        // Matching the locally created tag acknowledges that exact initial value.
        selection::select(&app.store, SelectedView::Logbook(id)).await?;
        app.settle().await?;
        if pending {
            let row = save::load(&app.store, key).await?.unwrap();
            let mut edited = tag.clone();
            edited.name = "Pending alternative".into();
            tags::save(&app.store, Some(&row), &edited).await?;
        }
        let mut deleted = live;
        deleted.state = RecordState::Deleted;
        deleted.stamp.accepted_at.revision = 2;
        deleted.stamp.vector = RecordVector::from_entries([(ClockActor::Server(timeline), 2)])?;
        synchronization::receive(&app.store, &deleted).await?;
        app.settle().await?;
        assert_eq!(
            visible(&app, "was deleted. Showing All contacts."),
            !pending
        );
        for member in &preview.members {
            assert_eq!(
                save::load(&app.store, member.key).await?,
                Some(member.clone())
            );
        }
        if !pending {
            app.click("Dismiss");
            app.frame(vec![]);
            app.frame(vec![]);
            assert!(!visible(&app, "was deleted. Showing All contacts."));
        }
        app.close().await?;
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn fallback_info_event_has_account_and_cause_with_debug_spans_filtered() -> anyhow::Result<()>
{
    use clacks_client_store::logbooks::selection;
    let mut app = LiveWindow::new().await?;
    let id = seed_contacts(&mut app).await?;
    selection::select(&app.store, SelectedView::Logbook(id)).await?;
    let before = app.snapshot().await?.logbooks;
    let preview = deletion::preview(&app.store, id).await?;
    deletion::delete_tag(
        &app.store,
        &preview,
        clacks_model::tag::TagDeletionChoice::LeaveAll,
    )
    .await?;
    let after = app.snapshot().await?.logbooks;
    let output = tempfile::NamedTempFile::new()?;
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::sync::Mutex::new(output.reopen()?))
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        let mut ui = LogbooksUi::default();
        ui.observe(&before, None);
        ui.observe(&after, None);
        ui.fallback_notice = None;
        ui.observe(&after, None);
    });
    let logged = std::fs::read_to_string(output.path())?;
    assert_eq!(
        logged
            .lines()
            .filter(|line| line.contains("logbook.view_fallback_observed"))
            .count(),
        1
    );
    assert!(logged.contains(&app.store.account_id().to_string()));
    assert!(logged.contains(&id.to_string()));
    assert!(logged.contains("deleted_tag"));
    assert!(!logged.contains("Main"));
    app.close().await
}

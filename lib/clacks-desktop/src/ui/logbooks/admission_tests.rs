//! Admitted work settles before another logbook action can supersede it.

use super::*;
use crate::ui::tests::logbooks_support::{LiveWindow, seed_contacts};
use sqlx::Connection;

fn visible(app: &LiveWindow, label: &str) -> bool {
    app.output.shapes.iter().any(|shape| {
        matches!(
            &shape.shape,
            egui::epaint::Shape::Text(text) if text.galley.text().contains(label)
        )
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn dismissed_deletion_settles_before_another_view_can_be_selected() -> anyhow::Result<()> {
    let mut app = LiveWindow::new().await?;
    seed_contacts(&mut app).await?;
    app.click("All contacts");
    app.click("Main");
    app.settle().await?;
    app.click("Manage logbooks…");
    app.click("Delete logbook");
    app.settle().await?;
    app.click("Leave all contacts (delete 0)");

    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(&app.paths.database),
    )
    .await?;
    let held = connection.begin_with("BEGIN IMMEDIATE").await?;
    app.click("Confirm deletion");
    app.frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);

    app.click("Main");
    app.frame(vec![]);
    assert!(
        !app.output.shapes.iter().any(|shape| matches!(
            &shape.shape,
            egui::epaint::Shape::Text(text) if text.galley.text() == "Built-in views"
        )),
        "view selector must wait for the dismissed deletion result"
    );
    app.click("Manage logbooks…");
    app.frame(vec![]);
    assert!(!visible(&app, "New logbook"));

    held.rollback().await?;
    connection.close().await?;
    app.settle().await?;
    assert_eq!(
        app.snapshot().await?.logbooks.selected_view,
        SelectedView::AllContacts
    );
    assert!(visible(&app, "All contacts"));

    app.click("All contacts");
    app.click("Untagged");
    app.settle().await?;
    assert_eq!(
        app.snapshot().await?.logbooks.selected_view,
        SelectedView::Untagged
    );
    assert!(visible(&app, "Untagged"));
    app.close().await
}

#[tokio::test(flavor = "multi_thread")]
async fn selection_settles_before_management_can_open() -> anyhow::Result<()> {
    let mut app = LiveWindow::new().await?;
    app.frame(vec![]);
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(&app.paths.database),
    )
    .await?;
    let held = connection.begin_with("BEGIN IMMEDIATE").await?;
    app.click("All contacts");
    app.click("Untagged");
    app.click("Manage logbooks…");
    app.frame(vec![]);
    assert!(!visible(&app, "New logbook"));
    held.rollback().await?;
    connection.close().await?;
    app.settle().await?;
    app.click("Manage logbooks…");
    app.frame(vec![]);
    assert!(visible(&app, "New logbook"));
    app.close().await
}

#[tokio::test(flavor = "multi_thread")]
async fn explicit_view_switch_clears_deleted_logbook_notice() -> anyhow::Result<()> {
    use clacks_client_store::logbooks::{deletion, selection};
    let mut app = LiveWindow::new().await?;
    let id = seed_contacts(&mut app).await?;
    selection::select(&app.store, SelectedView::Logbook(id)).await?;
    app.settle().await?;
    let preview = deletion::preview(&app.store, id).await?;
    deletion::delete_tag(
        &app.store,
        &preview,
        clacks_model::tag::TagDeletionChoice::LeaveAll,
    )
    .await?;
    app.settle().await?;
    assert!(visible(&app, "was deleted. Showing All contacts."));

    app.click("All contacts");
    app.click("Untagged");
    app.settle().await?;
    assert!(!visible(&app, "was deleted. Showing All contacts."));
    assert_eq!(
        app.snapshot().await?.logbooks.selected_view,
        SelectedView::Untagged
    );
    app.close().await
}

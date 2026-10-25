//! A committed result is distinct from a subsequent view-read failure.

use super::tests::support::Window;
use super::*;
use sqlx::{Connection, Executor};

#[tokio::test]
async fn committed_delete_with_failed_refresh_clears_pending_source_and_warns() -> anyhow::Result<()>
{
    let mut window = Window::new().await?;
    let sources = window.seed().await?;
    window.click("Operating settings");
    window.click("Operator / power");
    window.text("operating.power", "25");
    window.click("Station");
    window.click("Manage station profiles");
    window.click("Edit profile");
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(&window.database),
    )
    .await?;
    connection
        .execute(include_str!("../../session/settings/fail_refresh.sql"))
        .await?;
    connection.close().await?;
    window.click("Delete profile");
    window.settle().await;
    assert!(window.visible("last successfully read state"));
    window.click("Back");
    assert!(
        matches!(&window.view.stack.last().unwrap().screen, Screen::Operating(e) if e.selected_profile.is_none() && e.power_watts == "25")
    );
    window.frame(vec![]);
    assert!(
        !window.visible(&format!("Load {}", profile_label(&sources.home))),
        "known-deleted source remains selectable"
    );
    window.click("Cancel operating changes");
    let applied = clacks_client_store::operating::load(&window.store).await?;
    assert!(applied.selected_profile.is_none());
    assert_eq!(
        applied.current.as_ref().unwrap().station,
        sources.home.station
    );
    assert!(applied.current.as_ref().unwrap().transmit_power.is_none());
    window.click("Operating settings");
    assert!(
        matches!(&window.view.stack.last().unwrap().screen, Screen::Operating(e) if e.selected_profile.is_none())
    );
    window.click("Refresh settings");
    window.settle().await;
    assert!(window.visible("last successfully read state"));
    window.close().await?;
    Ok(())
}

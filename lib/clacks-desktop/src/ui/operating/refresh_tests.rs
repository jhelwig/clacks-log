//! A committed result is distinct from a subsequent view-read failure.

use super::tests::support::Window;
use super::*;
use sqlx::{Connection, Executor};

#[tokio::test]
async fn committed_delete_with_failed_refresh_clears_pending_source_and_warns() -> anyhow::Result<()>
{
    let mut window = Window::new().await?;
    let sources = window.seed().await?;
    window.click("Operating settings…");
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
    window.click("Operating settings…");
    assert!(
        matches!(&window.view.stack.last().unwrap().screen, Screen::Operating(e) if e.selected_profile.is_none())
    );
    window.click("Cancel operating changes");
    window.saved_settings("Refresh settings");
    window.settle().await;
    assert!(window.visible("last successfully read state"));
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn successful_profile_save_survives_failed_refresh_in_visible_list() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    window.saved_settings("Station profiles");
    window.click("New profile");
    window.text("profile.name", "Saved locally despite failed read");
    window.text("profile.station.call", "W7ACK");
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(&window.database),
    )
    .await?;
    connection
        .execute(include_str!("../../session/settings/fail_refresh.sql"))
        .await?;
    connection.close().await?;
    window.submit("Save profile");
    window.settle().await;
    window.frame(vec![]);
    assert!(window.visible("Saved locally despite failed read"));
    assert!(window.visible("saved locally"));
    window.click("Edit profile");
    let Screen::Profile(editor) = &window.view.stack.last().unwrap().screen else {
        panic!()
    };
    assert!(editor.expected.is_some());
    assert_eq!(editor.station.callsign, "W7ACK");
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn dismissed_equipment_save_retains_acknowledged_record_when_refresh_fails()
-> anyhow::Result<()> {
    let mut window = Window::new().await?;
    window.saved_settings("Equipment configurations");
    window.click("New configuration");
    window.text("equipment.name", "Saved after dismissal");
    let Screen::EquipmentEdit(editor) = &window.view.stack.last().unwrap().screen else {
        panic!()
    };
    let next = editor.build().unwrap();
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(&window.database),
    )
    .await?;
    connection
        .execute(include_str!("../../session/settings/fail_refresh.sql"))
        .await?;
    connection.close().await?;
    let (reply, result) = oneshot::channel();
    window
        .sender
        .send(SettingsRequest {
            action: SettingsAction::SaveEquipment {
                expected: None,
                next,
            },
            reply,
        })
        .await?;
    let (deliver, response) = oneshot::channel();
    window.view.stack.last_mut().unwrap().response = Some(response);
    window.click("×");
    deliver.send(result.await?).unwrap();
    window.settle().await;
    window.frame(vec![]);
    assert!(window.visible("Equipment saved locally"));
    assert!(window.visible("Saved after dismissal"));
    window.click("Edit configuration");
    let Screen::EquipmentEdit(editor) = &window.view.stack.last().unwrap().screen else {
        panic!()
    };
    assert!(editor.expected.is_some());
    assert_eq!(editor.name, "Saved after dismissal");
    window.close().await?;
    Ok(())
}

//! Modal dismissal preserves admitted work and the last acknowledged operating state.

use super::{tests::support::Window, *};

#[tokio::test]
async fn escape_cancels_only_top_editor() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    window.click("Operating settings…");
    window.text("operating.station.call", "W7HEX");
    window.click("Save station values as profile…");
    let opener = window.view.stack.last().unwrap().opener.unwrap();
    window.frame(vec![]);
    window.frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert!(
        matches!(&window.view.stack.last().unwrap().screen, Screen::Operating(editor) if editor.station.callsign == "W7HEX")
    );
    window.frame(vec![]);
    assert_eq!(
        window.context.memory(|memory| memory.focused()),
        Some(opener)
    );
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn committed_apply_refresh_failure_shows_acknowledged_state_once() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    window.click("Operating settings…");
    let mut acknowledged = window.snapshot.operating.clone();
    let Screen::Operating(editor) = &mut window.view.stack.last_mut().unwrap().screen else {
        panic!()
    };
    editor.station.callsign = "W7ACK".into();
    acknowledged.current = Some(editor.build(&Default::default()).unwrap().current.unwrap());
    let saved = clacks_client_store::operating::save(
        &window.store,
        &window.snapshot.operating,
        &acknowledged,
    )
    .await?;
    assert!(matches!(
        saved,
        clacks_client_store::operating::OperatingSaveOutcome::Saved(_)
    ));
    let committed = clacks_client_store::operating::load(&window.store).await?;
    let (reply, response) = oneshot::channel();
    window.view.stack.last_mut().unwrap().response = Some(response);
    reply
        .send(SettingsResponse::RefreshFailed {
            outcome: Box::new(SettingsResponse::Apply(saved)),
            message: "read unavailable".into(),
        })
        .unwrap();
    window.frame(vec![]);
    window.click("Operating settings…");
    assert!(
        matches!(&window.view.stack.last().unwrap().screen, Screen::Operating(editor) if editor.station.callsign == "W7ACK" && editor.expected.current.is_some())
    );
    window.frame(vec![]);
    window.frame(vec![]);
    assert!(window.view.dismissed.is_empty());
    assert!(
        window
            .view
            .stack
            .iter()
            .all(|frame| frame.response.is_none())
    );
    assert_eq!(
        clacks_client_store::operating::load(&window.store).await?,
        committed
    );
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn dismiss_after_ui_submission_retains_one_committed_equipment_save() -> anyhow::Result<()> {
    use sqlx::Connection;
    let mut window = Window::new().await?;
    window.saved_settings("Equipment configurations");
    window.click("New configuration");
    window.text("equipment.name", "Saved after close");
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(&window.database),
    )
    .await?;
    let held = connection.begin_with("BEGIN IMMEDIATE").await?;
    window.submit("Save configuration");
    assert!(window.view.stack.last().unwrap().response.is_some());
    window.click("×");
    assert_eq!(window.view.dismissed.len(), 1);
    held.rollback().await?;
    connection.close().await?;
    window.settle().await;
    assert!(window.visible("Saved after close"));
    let committed = clacks_client_store::settings::snapshot(&window.store).await?;
    assert_eq!(committed.equipment.len(), 1);
    assert!(
        matches!(committed.equipment.first().unwrap().working_state(), Some(RecordState::Live(RecordValue::Equipment(record))) if record.name == "Saved after close")
    );
    window.frame(vec![]);
    window.frame(vec![]);
    assert!(window.view.dismissed.is_empty());
    assert_eq!(
        clacks_client_store::settings::snapshot(&window.store).await?,
        committed
    );
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn backdrop_does_not_discard_editor() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    window.click("Operating settings…");
    let position = egui::pos2(2.0, 2.0);
    for pressed in [true, false] {
        window.frame(vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
    }
    assert!(matches!(
        &window.view.stack.last().unwrap().screen,
        Screen::Operating(_)
    ));
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn menu_escape_does_not_cancel_outer_editor() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    window.saved_settings("Equipment configurations");
    window.click("New configuration");
    window.click("Create new configuration");
    window.frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert!(matches!(
        &window.view.stack.last().unwrap().screen,
        Screen::EquipmentEdit(_)
    ));
    window.close().await?;
    Ok(())
}

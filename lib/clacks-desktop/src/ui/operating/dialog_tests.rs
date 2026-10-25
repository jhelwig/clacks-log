//! Modal dismissal preserves admitted work and the last acknowledged operating state.

use super::{tests::support::Window, *};

#[tokio::test]
async fn escape_cancels_only_top_editor() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    window.click("Operating settings");
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
    window.click("Operating settings");
    let mut acknowledged = window.snapshot.operating.clone();
    let Screen::Operating(editor) = &mut window.view.stack.last_mut().unwrap().screen else {
        panic!()
    };
    editor.station.callsign = "W7ACK".into();
    acknowledged.current = Some(editor.build(&Default::default()).unwrap().current.unwrap());
    let (reply, response) = oneshot::channel();
    window.view.stack.last_mut().unwrap().response = Some(response);
    reply
        .send(SettingsResponse::RefreshFailed {
            outcome: Box::new(SettingsResponse::Apply(
                clacks_client_store::operating::OperatingSaveOutcome::Saved(acknowledged),
            )),
            message: "read unavailable".into(),
        })
        .unwrap();
    window.frame(vec![]);
    window.click("Operating settings");
    assert!(
        matches!(&window.view.stack.last().unwrap().screen, Screen::Operating(editor) if editor.station.callsign == "W7ACK" && editor.expected.current.is_some())
    );
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn dismiss_after_request_admission_does_not_lose_completion() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    window.click("Equipment configurations");
    window.click("New configuration");
    let next = EquipmentConfiguration {
        id: Default::default(),
        name: "Saved after close".into(),
        values: Default::default(),
    };
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
    assert_eq!(window.view.dismissed.len(), 1);
    deliver.send(result.await?).unwrap();
    window.settle().await;
    window.refresh().await;
    assert!(window.visible("Saved after close"));
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn backdrop_does_not_discard_editor() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    window.click("Operating settings");
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
    window.click("Equipment configurations");
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

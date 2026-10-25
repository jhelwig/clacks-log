//! Inline controls use the production settings transaction and native keyboard input.

use super::{tests::support::Window, *};

fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

#[tokio::test]
async fn inline_power_point_five_commits_500_milliwatts_and_reopens() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    let sources = window.seed().await?;
    window.text("summary.power", ".5");
    window.frame(vec![key(egui::Key::Enter)]);
    window.settle().await;
    let stored = clacks_client_store::operating::load(&window.store).await?;
    assert_eq!(
        stored
            .current
            .as_ref()
            .unwrap()
            .transmit_power
            .unwrap()
            .get(),
        500
    );
    assert_eq!(stored.selected_profile, Some(sources.home));
    let reopened = clacks_client_store::store::ClientStore::open_discovered(
        &window.database,
        Default::default(),
    )
    .await?;
    assert_eq!(
        clacks_client_store::operating::load(&reopened).await?,
        stored
    );
    reopened.close().await?;
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn inline_power_tab_escape_invalid_and_blank_have_distinct_effects() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    window.seed().await?;
    window.text("summary.power", "0.001");
    window.frame(vec![key(egui::Key::Tab)]);
    window.settle().await;
    assert_eq!(
        clacks_client_store::operating::load(&window.store)
            .await?
            .current
            .unwrap()
            .transmit_power
            .unwrap()
            .get(),
        1
    );

    window.text("summary.power", "25");
    window.frame(vec![key(egui::Key::Escape)]);
    window.frame(vec![]);
    assert_eq!(window.view.quick.power_text, "0.001");
    for invalid in ["0", "invalid", "9223372036854776"] {
        window.text("summary.power", invalid);
        window.frame(vec![key(egui::Key::Enter)]);
        window.settle().await;
        assert_eq!(
            clacks_client_store::operating::load(&window.store)
                .await?
                .current
                .unwrap()
                .transmit_power
                .unwrap()
                .get(),
            1
        );
        assert_eq!(window.view.quick.power_text, invalid);
    }
    window.text("summary.power", "");
    window.frame(vec![key(egui::Key::Tab)]);
    window.settle().await;
    assert!(
        clacks_client_store::operating::load(&window.store)
            .await?
            .current
            .unwrap()
            .transmit_power
            .is_none()
    );
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn inline_power_reply_cannot_replace_later_typing() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    window.seed().await?;
    let (sender, mut requests) = mpsc::channel::<SettingsRequest>(1);
    window.sender = sender;
    window.text("summary.power", "25");
    window.frame(vec![key(egui::Key::Enter)]);
    let request = requests.try_recv()?;
    let SettingsAction::Apply { expected, next } = request.action else {
        panic!()
    };
    window.text("summary.power", "30");
    let outcome = clacks_client_store::operating::save(&window.store, &expected, &next).await?;
    request
        .reply
        .send(SettingsResponse::Apply(outcome))
        .unwrap();
    window.settle().await;
    assert_eq!(window.view.quick.power_text, "30");
    assert_eq!(
        clacks_client_store::operating::load(&window.store)
            .await?
            .current
            .unwrap()
            .transmit_power
            .unwrap()
            .get(),
        25_000
    );
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn quick_controls_without_active_station_do_not_create_incomplete_context()
-> anyhow::Result<()> {
    let mut window = Window::new().await?;
    window.text("summary.power", "25");
    window.frame(vec![key(egui::Key::Enter)]);
    assert!(
        clacks_client_store::operating::load(&window.store)
            .await?
            .current
            .is_none()
    );
    assert!(window.visible("No station configured"));
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn inline_power_failed_admission_preserves_active_value_and_diagnostic() -> anyhow::Result<()>
{
    let mut window = Window::new().await?;
    window.seed().await?;
    let (sender, receiver) = mpsc::channel(1);
    drop(receiver);
    window.sender = sender;
    window.text("summary.power", "25");
    window.frame(vec![key(egui::Key::Enter)]);
    window.advance(0.5);
    assert!(window.visible("Could not apply power"));
    assert_eq!(window.view.quick.power_text, "25");
    assert!(
        clacks_client_store::operating::load(&window.store)
            .await?
            .current
            .unwrap()
            .transmit_power
            .is_none()
    );
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn inline_power_blur_admits_only_one_request() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    window.seed().await?;
    let (sender, mut requests) = mpsc::channel::<SettingsRequest>(4);
    window.sender = sender;
    window.text("summary.power", "25");
    window
        .context
        .memory_mut(|memory| memory.surrender_focus(egui::Id::unique("summary.power")));
    window.frame(vec![]);
    let request = requests.try_recv()?;
    window.frame(vec![key(egui::Key::Enter)]);
    window.frame(vec![]);
    assert!(requests.try_recv().is_err());
    request
        .reply
        .send(SettingsResponse::Failed("disk unavailable".into()))
        .unwrap();
    window.settle().await;
    window.advance(0.5);
    assert!(window.visible("disk unavailable"));
    assert_eq!(window.view.quick.power_text, "25");
    assert!(
        clacks_client_store::operating::load(&window.store)
            .await?
            .current
            .unwrap()
            .transmit_power
            .is_none()
    );
    window.close().await?;
    Ok(())
}

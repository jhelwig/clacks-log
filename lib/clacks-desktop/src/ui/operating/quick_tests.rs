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

#[tokio::test]
async fn quick_equipment_selection_preserves_station_and_cancel_has_no_effect() -> anyhow::Result<()>
{
    let mut window = Window::new().await?;
    let sources = window.seed().await?;
    window.quick_change("Equipment");
    window.click(&format!("Load {}", equipment_label(&sources.rig_b)));
    window.click("Cancel");
    assert_eq!(
        clacks_client_store::operating::load(&window.store)
            .await?
            .selected_equipment,
        Some(sources.rig_a.clone())
    );
    window.quick_change("Equipment");
    window.click(&format!("Load {}", equipment_label(&sources.rig_b)));
    window.submit("Apply");
    window.settle().await;
    let active = clacks_client_store::operating::load(&window.store).await?;
    assert_eq!(active.selected_profile, Some(sources.home));
    assert_eq!(active.selected_equipment, Some(sources.rig_b));
    assert!(window.visible("Equipment override"));
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn quick_profiles_preserve_operator_power_and_absent_default_equipment() -> anyhow::Result<()>
{
    let mut window = Window::new().await?;
    let sources = window.seed().await?;
    window.quick_change("Operator");
    window.text("quick.operator.call", " ve7abc/p ");
    window.text("quick.operator.name", "Operator name");
    window.submit("Apply");
    window.settle().await;
    window.text("summary.power", "25");
    window.frame(vec![key(egui::Key::Enter)]);
    window.settle().await;
    let mut field = sources.home.clone();
    field.id = Default::default();
    field.name = "Field".into();
    field.station.callsign = clacks_model::scalar::RecordedCallsign::new("W7FIELD")?;
    field.default_equipment = Some(sources.rig_b.id);
    let mut portable = field.clone();
    portable.id = Default::default();
    portable.name = "Portable".into();
    portable.default_equipment = None;
    for saved in [&field, &portable] {
        clacks_client_store::settings::save_profile(&window.store, None, saved).await?;
    }
    window.refresh().await;
    for saved in [&field, &portable] {
        window.quick_change("Station");
        window.click(&format!("Load {}", profile_label(saved)));
        window.submit("Apply");
        window.settle().await;
        let active = clacks_client_store::operating::load(&window.store).await?;
        assert_eq!(active.selected_profile.as_ref(), Some(saved));
        assert_eq!(active.selected_equipment.as_ref(), Some(&sources.rig_b));
        let current = active.current.unwrap();
        assert_eq!(current.transmit_power.unwrap().get(), 25_000);
        let operator = current.operator.unwrap();
        assert_eq!(operator.callsign.unwrap().as_str(), "VE7ABC/P");
        assert_eq!(operator.recorded_name.as_deref(), Some("Operator name"));
    }
    window.quick_change("Operator");
    window.text("quick.operator.call", "");
    window.submit("Apply");
    window.settle().await;
    let active = clacks_client_store::operating::load(&window.store).await?;
    assert_eq!(active.selected_profile, Some(portable));
    assert_eq!(active.selected_equipment, Some(sources.rig_b));
    let operator = active.current.unwrap().operator.unwrap();
    assert!(operator.callsign.is_none());
    assert_eq!(operator.recorded_name.as_deref(), Some("Operator name"));
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn quick_missing_default_does_not_partially_load_station() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    let sources = window.seed().await?;
    let mut broken = sources.home.clone();
    broken.id = Default::default();
    broken.name = "Unavailable default".into();
    broken.station.callsign = clacks_model::scalar::RecordedCallsign::new("W7BROKEN")?;
    broken.default_equipment = Some(Default::default());
    clacks_client_store::settings::save_profile(&window.store, None, &broken).await?;
    window.refresh().await;
    window.quick_change("Station");
    window.click(&format!("Load {}", profile_label(&broken)));
    window.frame(vec![]);
    assert!(window.visible("Could not load profile"));
    window.submit("Apply");
    window.settle().await;
    let active = clacks_client_store::operating::load(&window.store).await?;
    assert_eq!(active.selected_profile, Some(sources.home));
    assert_eq!(active.selected_equipment, Some(sources.rig_a));
    window.close().await?;
    Ok(())
}

async fn publish(window: &mut Window) -> anyhow::Result<()> {
    let (reply, response) = oneshot::channel();
    window
        .sender
        .send(SettingsRequest {
            action: SettingsAction::Refresh,
            reply,
        })
        .await?;
    assert!(matches!(response.await?, SettingsResponse::Refresh(_)));
    window.snapshot = window.settings_view.borrow().clone().unwrap();
    window.frame(vec![]);
    Ok(())
}

#[tokio::test]
async fn quick_source_refresh_preserves_captured_values_and_concurrent_save_requires_review()
-> anyhow::Result<()> {
    let mut window = Window::new().await?;
    let sources = window.seed().await?;
    window.quick_change("Station");
    let expected = window.snapshot.operating.clone();
    let old = window
        .snapshot
        .profiles
        .iter()
        .find(|record| profile(record).is_some_and(|profile| profile.id == sources.home.id))
        .unwrap()
        .clone();
    let mut changed = sources.home.clone();
    changed.station.callsign = clacks_model::scalar::RecordedCallsign::new("W7NEW")?;
    clacks_client_store::settings::save_profile(&window.store, Some(&old), &changed).await?;
    let mut concurrent = expected.clone();
    concurrent.current.as_mut().unwrap().transmit_power =
        Some(clacks_model::scalar::Milliwatts::new(50_000)?);
    clacks_client_store::operating::save(&window.store, &expected, &concurrent).await?;
    publish(&mut window).await?;
    assert!(window.visible("Saved profile changed"));
    assert!(window.visible("Station source changed"));
    let editor = window
        .view
        .stack
        .last()
        .unwrap()
        .screen
        .operating()
        .unwrap();
    assert_eq!(editor.expected, expected);
    assert_eq!(editor.selected_profile.as_ref(), Some(&sources.home));
    window.submit("Apply");
    window.settle().await;
    assert!(window.visible("Current applied settings to review"));
    assert_eq!(
        window
            .view
            .stack
            .last()
            .unwrap()
            .screen
            .operating()
            .unwrap()
            .expected,
        expected
    );
    assert_eq!(
        clacks_client_store::operating::load(&window.store).await?,
        concurrent
    );
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn quick_profile_can_establish_initial_complete_context() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    let saved = SavedLocationProfile {
        id: Default::default(),
        name: "Initial station".into(),
        station: clacks_model::contact::StationSnapshot {
            callsign: clacks_model::scalar::RecordedCallsign::new("W7FIRST")?,
            location: None,
            references: Default::default(),
        },
        default_equipment: None,
    };
    clacks_client_store::settings::save_profile(&window.store, None, &saved).await?;
    window.refresh().await;
    window.quick_change("Station");
    window.click(&format!("Load {}", profile_label(&saved)));
    window.submit("Apply");
    window.settle().await;
    let active = clacks_client_store::operating::load(&window.store).await?;
    assert_eq!(active.current.unwrap().station, saved.station);
    assert_eq!(active.selected_profile, Some(saved));
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn inline_power_tab_during_pending_request_commits_new_value_after_acknowledgment()
-> anyhow::Result<()> {
    let mut window = Window::new().await?;
    window.seed().await?;
    let (sender, mut requests) = mpsc::channel::<SettingsRequest>(4);
    window.sender = sender;
    window.text("summary.power", "25");
    window.frame(vec![key(egui::Key::Enter)]);
    let first = requests.try_recv()?;
    window.text("summary.power", "30");
    window.frame(vec![key(egui::Key::Tab)]);
    window.frame(vec![]);
    window.frame(vec![]);
    assert!(window.view.quick.is_busy());
    assert!(requests.try_recv().is_err());
    let SettingsAction::Apply { expected, next } = first.action else {
        panic!()
    };
    let result = clacks_client_store::operating::save(&window.store, &expected, &next).await?;
    first.reply.send(SettingsResponse::Apply(result)).unwrap();
    window.frame(vec![]);
    let second = requests.try_recv()?;
    let SettingsAction::Apply { expected, next } = second.action else {
        panic!()
    };
    assert_eq!(
        expected
            .current
            .as_ref()
            .unwrap()
            .transmit_power
            .unwrap()
            .get(),
        25_000
    );
    let result = clacks_client_store::operating::save(&window.store, &expected, &next).await?;
    second.reply.send(SettingsResponse::Apply(result)).unwrap();
    window.settle().await;
    assert_eq!(
        clacks_client_store::operating::load(&window.store)
            .await?
            .current
            .unwrap()
            .transmit_power
            .unwrap()
            .get(),
        30_000
    );
    assert!(requests.try_recv().is_err());
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn inline_power_failed_prior_request_does_not_automatically_retry_waiting_edit()
-> anyhow::Result<()> {
    let mut window = Window::new().await?;
    window.seed().await?;
    let (sender, mut requests) = mpsc::channel::<SettingsRequest>(4);
    window.sender = sender;
    window.text("summary.power", "25");
    window.frame(vec![key(egui::Key::Enter)]);
    let first = requests.try_recv()?;
    window.text("summary.power", "30");
    window.frame(vec![key(egui::Key::Tab)]);
    window.frame(vec![]);
    window.frame(vec![]);
    first
        .reply
        .send(SettingsResponse::Failed(
            "Acknowledgment unavailable".into(),
        ))
        .unwrap();
    window.settle().await;
    window.frame(vec![]);
    assert!(requests.try_recv().is_err());
    assert_eq!(window.view.quick.power_text, "30");
    assert!(window.visible("Acknowledgment unavailable"));
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

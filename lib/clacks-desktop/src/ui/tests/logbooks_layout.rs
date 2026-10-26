//! Logbook controls, scrolling and keyboard focus across themes and scales.

use super::logbooks_support::LiveWindow;
use super::*;
use crate::ui::preferences::{Appearance, AppearancePreferences};

fn reachable(app: &LiveWindow, value: &str) -> bool {
    app.output.shapes.iter().any(|shape| matches!(&shape.shape,egui::epaint::Shape::Text(text) if text.galley.text()==value && shape.clip_rect.contains(text.visual_bounding_rect().center())))
}

fn scroll(app: &mut LiveWindow) {
    let center = app.window.context.content_rect().center();
    app.frame(vec![
        egui::Event::PointerMoved(center),
        egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
            delta: egui::vec2(0.0, -100.0),
            modifiers: egui::Modifiers::NONE,
        },
    ]);
}

#[tokio::test(flavor = "multi_thread")]
async fn logbook_fields_and_actions_remain_reachable_across_appearances_and_scales()
-> anyhow::Result<()> {
    let mut app = LiveWindow::new().await?;
    for appearance in [Appearance::Light, Appearance::Dark] {
        for zoom in [1.0, 1.25, 1.5, 2.0] {
            for (width, height) in [(680.0, 440.0), (1024.0, 768.0), (1440.0, 900.0)] {
                app.size = egui::vec2(width, height);
                theme::install(
                    &app.window.context,
                    &AppearancePreferences {
                        appearance,
                        zoom,
                        ..Default::default()
                    },
                );
                app.frame(vec![]);
                app.frame(vec![]);
                app.click("Manage logbooks…");
                app.click("New logbook");
                app.frame(vec![]);
                let mut required = std::collections::BTreeSet::from([
                    "Logbook name",
                    "Enter a logbook name.",
                    "Automatic uploads",
                    "LoTW",
                    "QRZ",
                    "Save logbook",
                    "Cancel edits",
                ]);
                for _ in 0..30 {
                    required.retain(|label| !reachable(&app, label));
                    if required.is_empty() {
                        break;
                    }
                    scroll(&mut app);
                }
                assert!(
                    required.is_empty(),
                    "unreachable {required:?} at {appearance:?} {zoom} {width}x{height}"
                );
                app.frame(vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }]);
                app.frame(vec![]);
            }
        }
    }
    app.close().await
}

#[tokio::test(flavor = "multi_thread")]
async fn manager_copy_and_keyboard_dismissal_restore_the_actual_opener() -> anyhow::Result<()> {
    use clacks_client_store::logbooks::tags;
    use clacks_model::tag::LogbookTag;
    let mut app = LiveWindow::new().await?;
    let tag = LogbookTag::new(Default::default(), "Main".into());
    tags::save(&app.store, None, &tag).await?;
    app.refresh_from_controls().await?;
    app.settle().await?;
    app.click("Manage logbooks…");
    let opener = app
        .window
        .context
        .interaction_snapshot(|interaction| interaction.clicked)
        .unwrap();
    app.click("Copy ID");
    assert!(app.output.platform_output.commands.iter().any(
        |command| matches!(command,egui::OutputCommand::CopyText(text) if text==&tag.id.to_string())
    ));
    app.click("Edit logbook");
    app.frame(vec![]);
    app.window
        .context
        .memory_mut(|memory| memory.request_focus(egui::Id::unique("logbook.name")));
    app.frame(vec![egui::Event::Key {
        key: egui::Key::Tab,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert_ne!(
        app.window.context.memory(|memory| memory.focused()),
        Some(opener)
    );
    app.frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    app.frame(vec![]);
    app.frame(vec![]);
    assert_eq!(
        app.window.context.memory(|memory| memory.focused()),
        Some(opener)
    );
    app.close().await
}

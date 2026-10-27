//! Current dialogs retain reachable fields/actions across supported appearance and sizing.

use super::{tests::support::Window, *};
use crate::ui::{
    preferences::{Appearance, AppearancePreferences},
    theme,
};

#[tokio::test]
async fn operator_fields_and_actions_remain_reachable_at_supported_scales() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    window.seed().await?;
    window.size = egui::vec2(680.0, 440.0);
    let mut case_number = 0;
    for appearance in [Appearance::Light, Appearance::Dark] {
        for zoom in [1.0, 2.0] {
            theme::install(
                &window.context,
                &AppearancePreferences {
                    appearance,
                    zoom,
                    ..Default::default()
                },
            );
            window.frame(vec![]);
            window.frame(vec![]);
            window.quick_change("Operator");
            case_number += 1;
            let callsign = format!("W7SCALE{case_number}");
            window.text("quick.operator.call", &callsign);
            window.advance(0.3);
            let mut apply_visible = false;
            for _ in 0..30 {
                window.frame(vec![]);
                apply_visible = window.output.shapes.iter().any(|shape| matches!(&shape.shape, egui::epaint::Shape::Text(text) if text.galley.text() == "Apply" && shape.clip_rect.contains(text.visual_bounding_rect().center())));
                if apply_visible {
                    break;
                }
                let center = window.context.content_rect().center();
                window.frame(vec![
                    egui::Event::PointerMoved(center),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        phase: egui::TouchPhase::Move,
                        source: egui::MouseWheelSource::Unknown,
                        delta: egui::vec2(0.0, -120.0),
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
            }
            assert!(
                apply_visible,
                "Apply unreachable at {appearance:?} {zoom} in the minimum window"
            );
            window.submit("Apply");
            window.settle().await;
            let current = clacks_client_store::operating::load(&window.store)
                .await?
                .current
                .unwrap();
            assert_eq!(
                current.operator.unwrap().callsign.unwrap().as_str(),
                callsign
            );
        }
    }
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn operator_keyboard_order_and_validation_preserve_field_identity() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    window.seed().await?;
    window.click("Operating settings…");
    window.click("Operator / power");
    window.text("operating.operator.call", "W7KEY");
    for expected in ["operating.operator.name", "operating.power"] {
        window.frame(vec![egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert_eq!(
            window.context.memory(|memory| memory.focused()),
            Some(egui::Id::unique(expected))
        );
        window.frame(vec![egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
    }
    window.text("operating.power", "invalid");
    window.advance(0.3);
    assert_eq!(
        window.context.memory(|memory| memory.focused()),
        Some(egui::Id::unique("operating.power"))
    );
    assert!(window.visible("Transmit power"));
    window.click("Equipment");
    window.click("Operator / power");
    assert!(
        matches!(&window.view.stack.last().unwrap().screen, Screen::Operating(editor) if editor.operator_callsign == "W7KEY" && editor.power_watts == "invalid")
    );
    window.close().await?;
    Ok(())
}

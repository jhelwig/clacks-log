//! Appearance returns keyboard focus to the actual header control that opened it.

use super::*;
use eframe::egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput};

impl Window {
    fn appearance_frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        use eframe::App;
        let mut frame = eframe::Frame::_new_kittest();
        let input = RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(1200.0, 1000.0),
            )),
            ..Default::default()
        };
        let mut output = self.context.run_ui(input, |ui| self.app.ui(ui, &mut frame));
        output.textures_delta.clear();
        output
    }

    fn appearance_click(&mut self, label: &str) -> egui::Id {
        self.appearance_frame(vec![]).drop_without_applying_deltas();
        let output = self.appearance_frame(vec![]);
        let position = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.visual_bounding_rect().center())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing control: {label}"));
        output.drop_without_applying_deltas();
        for pressed in [true, false] {
            self.appearance_frame(vec![
                Event::PointerMoved(position),
                Event::PointerButton {
                    pos: position,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ])
            .drop_without_applying_deltas();
        }
        self.context
            .interaction_snapshot(|interaction| interaction.clicked)
            .unwrap()
    }
}

#[test]
fn appearance_dismissal_restores_its_actual_opener_after_keyboard_interaction() {
    for dismissal in ["Escape", "Cancel", "×", "Apply"] {
        let mut window = Window::new();
        let opener = window.appearance_click("Appearance…");
        window
            .appearance_frame(vec![])
            .drop_without_applying_deltas();
        for pressed in [true, false] {
            window
                .appearance_frame(vec![Event::Key {
                    key: Key::Tab,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }])
                .drop_without_applying_deltas();
        }
        let inside = window.context.memory(|memory| memory.focused());
        assert!(inside.is_some());
        assert_ne!(inside, Some(opener));

        if dismissal == "Escape" {
            window
                .appearance_frame(vec![Event::Key {
                    key: Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }])
                .drop_without_applying_deltas();
        } else {
            window.appearance_click(dismissal);
        }
        window
            .appearance_frame(vec![])
            .drop_without_applying_deltas();
        window
            .appearance_frame(vec![])
            .drop_without_applying_deltas();

        assert!(
            window.app.appearance.editor.is_none(),
            "{dismissal} did not dismiss Appearance"
        );
        assert_eq!(
            window.context.memory(|memory| memory.focused()),
            Some(opener),
            "{dismissal} did not restore opener focus"
        );
    }
}

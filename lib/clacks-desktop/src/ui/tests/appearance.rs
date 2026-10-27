//! Dismissing Appearance restores the control that opened it.

use crate::ui::appearance::AppearanceUi;
use egui_kittest::{Harness, kittest::Queryable};

#[test]
fn dismissal_restores_opener_focus_after_keyboard_interaction() {
    for dismissal in ["Escape", "Cancel", "×", "Apply"] {
        let mut ui = Harness::builder()
            .with_size(eframe::egui::vec2(900.0, 900.0))
            .build_ui_state(
                |ui, state: &mut AppearanceUi| {
                    let opener = ui.button("Appearance…");
                    if opener.clicked() {
                        state.open(opener.id);
                    }
                    state.show(ui.ctx());
                },
                AppearanceUi::default(),
            );
        ui.get_by_label("Appearance…").click();
        ui.run();
        ui.key_press(eframe::egui::Key::Tab);
        ui.run();
        assert!(!ui.get_by_label("Appearance…").is_focused());

        if dismissal == "Escape" {
            ui.key_press(eframe::egui::Key::Escape);
        } else {
            ui.get_by_label(dismissal).click();
        }
        ui.run();
        assert!(ui.state().editor.is_none(), "{dismissal}");
        assert!(ui.get_by_label("Appearance…").is_focused(), "{dismissal}");
    }
}

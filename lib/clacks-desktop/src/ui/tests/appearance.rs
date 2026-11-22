//! Dismissing Settings restores the control that opened it.

use crate::ui::{SessionStatus, settings::SettingsDestination, tests::Window};
use eframe::egui;
use egui_kittest::{Harness, kittest::Queryable};

#[test]
fn dismissal_restores_opener_focus_after_keyboard_interaction() {
    for dismissal in ["Escape", "Cancel"] {
        let mut ui = Harness::builder()
            .with_size(egui::vec2(900.0, 900.0))
            .build_ui_state(
                |ui, window: &mut Window| {
                    let opener = ui.button("Open settings");
                    if opener.clicked() {
                        ui.ctx()
                            .memory_mut(|memory| memory.request_focus(opener.id));
                        window
                            .app
                            .open_settings(ui.ctx(), SettingsDestination::Default);
                    }
                    window.app.show_settings(ui, &SessionStatus::Opening);
                },
                Window::new(),
            );
        ui.get_by_label("Open settings").click();
        ui.run();
        ui.get_by_label("Dark").click();
        ui.run();

        if dismissal == "Escape" {
            ui.key_press(egui::Key::Escape);
        } else {
            ui.get_by_label(dismissal).click();
        }
        ui.run();
        assert!(ui.state().app.appearance.editor.is_none(), "{dismissal}");
        assert!(ui.get_by_label("Open settings").is_focused(), "{dismissal}");
    }
}

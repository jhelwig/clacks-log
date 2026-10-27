//! Combined UTC controls using the shared field feedback and independent locks.

use super::entry::EntryAction;
use crate::{
    logging::time::{self, ContactTimestamp},
    operating::FieldError,
};
use eframe::egui;

/// Edit a complete UTC timestamp; clearing a locked end retains absence.
pub(super) fn show(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut ContactTimestamp,
    baseline: &ContactTimestamp,
    errors: &[FieldError],
    live_controls: bool,
) -> EntryAction {
    let mut action = EntryAction::default();
    let id = ui.make_persistent_id(label);
    let had_focus = ui.memory(|m| m.has_focus(id));
    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
    let width = (ui.available_width() - if live_controls { 52.0 } else { 0.0 }).max(128.0);
    ui.horizontal_top(|ui| {
        let edited = !value.live && value.text != baseline.text;
        let read_only = value.live || !ui.is_enabled();
        let response = ui
            .allocate_ui(egui::vec2(width, 0.0), |ui| {
                super::super::fields::text(
                    ui,
                    id,
                    label,
                    &mut value.text,
                    super::super::fields::FieldPresentation {
                        edited,
                        read_only,
                        errors,
                        validation_label: Some(if label.starts_with("Start") {
                            "Start"
                        } else {
                            "End"
                        }),
                        radio_data: true,
                    },
                )
            })
            .inner;
        action.edited |= response.changed();
        action.save |= had_focus && response.lost_focus() && enter;
        if live_controls
            && ui
                .small_button(if value.live { "Live" } else { "Locked" })
                .clicked()
        {
            if value.live {
                value.lock();
            } else if let Err(error) = value.unlock(time::now()) {
                ui.label(error.to_string());
            }
            action.edited = true;
        }
    });
    action
}

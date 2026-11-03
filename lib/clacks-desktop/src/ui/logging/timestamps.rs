//! Combined UTC controls using the shared field feedback and independent locks.

use super::entry::EntryAction;
use crate::{
    logging::time::{self, ContactTimestamp},
    operating::FieldError,
};
use eframe::egui;

/// Width needed to show an entire timestamp and its optional lock control.
pub(super) fn minimum_width(ui: &egui::Ui, live_controls: bool) -> f32 {
    let text = egui::WidgetText::from(egui::RichText::new("0000-00-00 00:00:00").monospace())
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Monospace,
        )
        .size()
        .x
        + 2.0 * ui.spacing().button_padding.x;
    text + if live_controls {
        toggle_width(ui) + ui.spacing().item_spacing.x
    } else {
        0.0
    }
}

/// Keep the lock label readable with the currently selected UI typography.
fn toggle_width(ui: &egui::Ui) -> f32 {
    egui::WidgetText::from("Locked")
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Button,
        )
        .size()
        .x
        + 2.0 * ui.spacing().button_padding.x
}

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
    let button_width = toggle_width(ui);
    let width = (ui.available_width()
        - if live_controls {
            button_width + ui.spacing().item_spacing.x
        } else {
            0.0
        })
    .max(128.0);
    ui.horizontal_top(|ui| {
        let edited = !value.live && value.text != baseline.text;
        let read_only = value.live || !ui.is_enabled();
        let response = ui
            .allocate_ui_with_layout(
                egui::vec2(width, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
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
                },
            )
            .inner;
        action.edited |= response.changed();
        action.save |= had_focus && response.lost_focus() && enter;
        if live_controls
            && ui
                .put(
                    egui::Rect::from_min_size(
                        egui::pos2(ui.next_widget_position().x, response.rect.top()),
                        egui::vec2(button_width, response.rect.height()),
                    ),
                    egui::Button::new(if value.live { "Live" } else { "Locked" }),
                )
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

//! Persistent actionable feedback, distinct from normal saved-state labels.

use eframe::egui;

/// Present an unresolved request or read failure without a disappearing toast.
pub(super) fn problem(ui: &mut egui::Ui, message: &str) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.colored_label(ui.visuals().warn_fg_color, message);
    });
}

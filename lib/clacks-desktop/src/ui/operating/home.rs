//! Existing reusable settings actions grouped separately from current operation.

use eframe::egui;

/// Explicit action requested from the saved-settings section.
pub(super) enum HomeAction {
    /// Leave the current workflow unchanged.
    None,
    /// Open the reusable station-profile list.
    Profiles,
    /// Open the reusable equipment list.
    Equipment,
    /// Refresh the actual committed settings view.
    Refresh,
}

/// Present management actions without applying any reusable values.
pub(super) fn show(ui: &mut egui::Ui, enabled: bool) -> HomeAction {
    let mut action = HomeAction::None;
    crate::ui::theme::section_frame(ui).show(ui, |ui| {
        ui.heading("Saved settings");
        ui.separator();
        ui.small("Manage reusable station profiles and equipment independently of active operating values.");
        ui.add_enabled_ui(enabled, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Station profiles").clicked() {
                    action = HomeAction::Profiles;
                }
                if ui.button("Equipment configurations").clicked() {
                    action = HomeAction::Equipment;
                }
                if ui.button("Refresh settings").clicked() {
                    action = HomeAction::Refresh;
                }
            });
        });
    });
    action
}

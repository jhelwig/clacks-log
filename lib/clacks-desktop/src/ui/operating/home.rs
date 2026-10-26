//! Menus for managing saved station profiles and equipment configurations.

use eframe::egui;

/// Explicit action requested from the saved-settings menu.
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

/// Show saved-profile and equipment-management actions in an application menu.
pub(super) fn show(ui: &mut egui::Ui, enabled: bool) -> egui::InnerResponse<Option<HomeAction>> {
    ui.menu_button("Saved settings", |ui| {
        let mut action = HomeAction::None;
        ui.add_enabled_ui(enabled, |ui| {
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
        if !matches!(action, HomeAction::None) {
            ui.close();
        }
        action
    })
}

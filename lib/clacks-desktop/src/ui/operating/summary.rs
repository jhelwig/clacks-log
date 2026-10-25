//! Compact applied station context with direct access to the full operating editor.

use super::{quick::QuickOperatingUi, *};
use clacks_client_store::operating::OperatingState;

/// An explicit transition from the summary to an existing workflow.
#[derive(Default)]
pub(super) enum SummaryAction {
    /// Keep the current workflow.
    #[default]
    None,
    /// Open the complete pending operating editor.
    Operating,
    /// Preview a saved station profile and its equipment consequence.
    Profile,
    /// Select equipment independently of the current station profile.
    Equipment,
    /// Change optional operator information only.
    Operator,
}

/// Show actual active values, keeping full optional facts in the operating editor.
pub(super) fn show(
    ui: &mut egui::Ui,
    active: &OperatingState,
    quick: &mut QuickOperatingUi,
    sender: &mpsc::Sender<SettingsRequest>,
) -> SummaryAction {
    let mut action = SummaryAction::None;
    crate::ui::theme::section_frame(ui).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Operating context");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button("Operating settings")
                    .on_hover_text("Edit station, equipment, operator and power")
                    .clicked()
                {
                    action = SummaryAction::Operating;
                }
            });
        });
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.strong("Active station");
            if let Some(current) = &active.current {
                ui.monospace(current.station.callsign.as_str());
                if let Some(position) = current
                    .station
                    .location
                    .as_ref()
                    .and_then(|location| location.position.as_ref())
                {
                    ui.monospace(position.display_locator().to_string());
                }
            } else {
                ui.label("No station configured");
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "Profile: {}",
                active
                    .selected_profile
                    .as_ref()
                    .map(|profile| profile.name.as_str())
                    .unwrap_or("None")
            ));
            if ui.button("Change profile…").clicked() {
                action = SummaryAction::Profile;
            }
            ui.separator();
            ui.label(format!(
                "Equipment: {}",
                active
                    .selected_equipment
                    .as_ref()
                    .map(|equipment| equipment.name.as_str())
                    .unwrap_or("Custom / none")
            ));
            if ui
                .add_enabled(
                    active.current.is_some(),
                    egui::Button::new("Change equipment…"),
                )
                .on_disabled_hover_text("Set up a station in Operating settings first.")
                .clicked()
            {
                action = SummaryAction::Equipment;
            }
            if let Some(profile) = &active.selected_profile
                && profile.default_equipment.is_some()
                && profile.default_equipment
                    != active
                        .selected_equipment
                        .as_ref()
                        .map(|equipment| equipment.id)
            {
                ui.small("Equipment override");
            }
            if let Some(equipment) = active
                .current
                .as_ref()
                .and_then(|value| value.equipment.as_ref())
            {
                ui.label(equipment.equipment_description.as_deref().unwrap_or(""));
                ui.label(equipment.antenna_description.as_deref().unwrap_or(""));
            }
        });
        ui.horizontal_wrapped(|ui| {
            let operator = active
                .current
                .as_ref()
                .and_then(|value| value.operator.as_ref());
            ui.label("Operator:");
            ui.monospace(
                operator
                    .and_then(|value| value.callsign.as_ref())
                    .map(|call| call.as_str())
                    .unwrap_or("—"),
            );
            if let Some(name) = operator.and_then(|value| value.recorded_name.as_deref()) {
                ui.label(name);
            }
            if ui
                .add_enabled(
                    active.current.is_some(),
                    egui::Button::new("Change operator…"),
                )
                .on_disabled_hover_text("Set up a station in Operating settings first.")
                .clicked()
            {
                action = SummaryAction::Operator;
            }
            ui.separator();
            quick.power(ui, active, sender);
        });
    });
    action
}

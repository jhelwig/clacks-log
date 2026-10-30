//! Applied operating values and quick actions in the workbench's operating strip.

use super::{quick::QuickOperatingUi, *};

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

/// Show one operating strip; narrow windows may reflow its existing controls.
pub(super) fn show(
    ui: &mut egui::Ui,
    snapshot: &SettingsSnapshot,
    quick: &mut QuickOperatingUi,
    sender: &mpsc::Sender<SettingsRequest>,
) -> SummaryAction {
    let mut action = SummaryAction::None;
    let active = &snapshot.operating;
    ui.horizontal_wrapped(|ui| {
        let profile_name = active
            .selected_profile()
            .map(|profile| profile.name())
            .unwrap_or("Custom");
        if crate::ui::fields::choice(ui,"Station",format!("{profile_name} ▾"),true)
            .on_hover_text(format!("Change station profile. Current: {profile_name}."))
            .clicked()
        {
            action = SummaryAction::Profile;
        }
        if let Some(current) = &active.current() {
            ui.monospace(current.station.callsign.as_str());
            if let Some(position) = current
                .station
                .location
                .as_ref()
                .and_then(|location| location.position().as_ref())
            {
                ui.monospace(position.display_locator().to_string());
            }
        } else {
            ui.weak("No station configured");
        }
        if let Some(selected) = active.selected_profile()
            && snapshot.profiles.iter().filter_map(profile).any(|profile|profile.id() == selected.id() && profile != selected) {
            ui.small("Station source changed").on_hover_text("Applied station values have not been reloaded from the changed saved profile.");
        }

        let equipment = active
            .selected_equipment()
            .map(|equipment| equipment.name())
            .or_else(||active.current().and_then(|current|current.equipment.as_ref()).and_then(|equipment|equipment.equipment_description().as_deref()))
            .unwrap_or("Custom / none");
        if crate::ui::fields::choice(ui,"Equipment",format!("{equipment} ▾"),active.current().is_some())
            .on_hover_text(format!("Change equipment configuration. Current: {equipment}."))
            .on_disabled_hover_text("Set up a station in Operating settings first.")
            .clicked()
        {
            action = SummaryAction::Equipment;
        }
        if let Some(selected) = active.selected_equipment()
            && snapshot.equipment.iter().filter_map(configuration).any(|equipment|equipment.id() == selected.id() && equipment != selected) {
            ui.small("Equipment source changed").on_hover_text("Applied equipment values have not been reloaded from the changed saved configuration.");
        }
        if let Some(profile) = active.selected_profile()
            && profile.default_equipment().is_some()
            && profile.default_equipment()
                != active
                    .selected_equipment()
                    .map(|equipment| equipment.id())
        {
            ui.small("Equipment override");
        }

        let operator = active
            .current()
            .and_then(|value| value.operator.as_ref());
        let operator_label = operator
            .and_then(|value| value.callsign().as_ref())
            .map(|call| call.as_str())
            .or_else(|| operator.and_then(|value| value.recorded_name().as_deref()))
            .unwrap_or("—");
        if crate::ui::fields::choice(ui,"Operator",egui::RichText::new(format!("{operator_label} ▾")).text_style(egui::TextStyle::Monospace),active.current().is_some())
            .on_hover_text(format!("Change operator. Current: {operator_label}."))
            .on_disabled_hover_text("Set up a station in Operating settings first.")
            .clicked()
        {
            action = SummaryAction::Operator;
        }
        quick.power(ui, active, sender);
        if ui
            .button("Operating settings…")
            .on_hover_text("Edit station, equipment, operator and power")
            .clicked()
        {
            action = SummaryAction::Operating;
        }
    });
    action
}

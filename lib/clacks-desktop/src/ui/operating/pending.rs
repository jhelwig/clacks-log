//! The single pending operating edit, with explicit source loading and apply/discard.

use super::*;

/// Render one operation-wide form; nested reusable editors retain this exact caller.
pub(super) fn show(
    ui: &mut egui::Ui,
    frame: &mut Frame,
    snapshot: &SettingsSnapshot,
    definitions: &DefinitionSet,
    sender: &mpsc::Sender<SettingsRequest>,
    navigation: &mut Navigation,
    known_deletions: &BTreeSet<RecordId>,
) {
    let Screen::Operating(editor) = &mut frame.screen else {
        return;
    };
    ui.heading("Pending operating settings");
    ui.horizontal(|ui| {
        ui.selectable_value(&mut frame.tab, OperatingTab::Station, "Station");
        ui.selectable_value(&mut frame.tab, OperatingTab::Equipment, "Equipment");
        ui.selectable_value(&mut frame.tab, OperatingTab::Operator, "Operator / power");
    });
    match &frame.tab {
        OperatingTab::Station => {
            let baseline = editor
                .expected
                .current()
                .map(|value| crate::operating::station::StationForm::from_snapshot(&value.station));
            ui.label(format!(
                "Selected profile: {}",
                editor
                    .selected_profile
                    .as_ref()
                    .map(profile_label)
                    .unwrap_or_else(|| "None".into())
            ));
            for record in snapshot
                .profiles
                .iter()
                .filter(|r| !known_deletions.contains(&r.key().id))
            {
                if let Some(profile) = profile(record) {
                    ui.push_id(profile.id(), |ui| {
                        if ui
                            .button(format!("Load {}", profile_label(profile)))
                            .clicked()
                        {
                            let equipment = profile.default_equipment().and_then(|id| {
                                snapshot
                                    .equipment
                                    .iter()
                                    .filter(|r| !known_deletions.contains(&r.key().id))
                                    .filter_map(configuration)
                                    .find(|e| e.id() == id)
                            });
                            if let Err(error) = editor.stage_profile(profile, equipment) {
                                frame.notice = Some(format!("Could not load profile: {error:#}"));
                            } else {
                                frame.validation.invalidate();
                            }
                        }
                    });
                }
            }
            if let Some(selected) = &editor.selected_profile
                && snapshot
                    .profiles
                    .iter()
                    .filter_map(profile)
                    .any(|p| p.id() == selected.id() && p != selected)
            {
                ui.label("Selected profile has changed. Values remain as copied; explicitly load it to refresh.");
            }
            let previous = editor.station.clone();
            if fields::station(
                ui,
                "operating.station",
                &mut editor.station,
                definitions,
                &frame.errors,
                baseline.as_ref(),
            ) {
                frame.validation.edited(ui.input(|input| input.time));
            }
            editor.station_changed(&previous, definitions);
            if frame.geography.show(ui, &mut editor.station) {
                frame.validation.edited(ui.input(|input| input.time));
                editor.station_changed(&previous, definitions);
            }
            if ui.button("Save station values as profile…").clicked() {
                navigation.next = Some(Screen::Profile(Box::new(ProfileEditor::new(
                    None,
                    editor.station.clone(),
                ))));
            }
            if ui.button("Manage station profiles").clicked() {
                navigation.next = Some(Screen::Profiles);
            }
        }
        OperatingTab::Equipment => {
            let baseline = crate::operating::equipment::EquipmentForm::from_values(
                editor
                    .expected
                    .current()
                    .and_then(|value| value.equipment.as_ref()),
            );
            ui.label(format!(
                "Selected configuration: {}",
                editor
                    .selected_equipment
                    .as_ref()
                    .map(equipment_label)
                    .unwrap_or_else(|| "None".into())
            ));
            for equipment in snapshot
                .equipment
                .iter()
                .filter(|r| !known_deletions.contains(&r.key().id))
                .filter_map(configuration)
            {
                ui.push_id(equipment.id(), |ui| {
                    if ui
                        .button(format!("Load {}", equipment_label(equipment)))
                        .clicked()
                    {
                        editor.stage_equipment(equipment);
                        frame.validation.invalidate();
                    }
                });
            }
            if let Some(selected) = &editor.selected_equipment
                && snapshot
                    .equipment
                    .iter()
                    .filter_map(configuration)
                    .any(|e| e.id() == selected.id() && e != selected)
            {
                ui.label("Selected configuration has changed. Values remain as copied; explicitly load it to refresh.");
            }
            let previous = editor.equipment.clone();
            if fields::equipment(
                ui,
                "operating.equipment",
                &mut editor.equipment,
                &frame.errors,
                Some(&baseline),
            ) {
                frame.validation.edited(ui.input(|input| input.time));
            }
            editor.equipment_changed(&previous);
            if ui
                .button("Save equipment values as configuration…")
                .clicked()
            {
                navigation.next = Some(Screen::EquipmentEdit(Box::new(EquipmentEditor::new(
                    None,
                    editor.equipment.clone(),
                ))));
            }
            if ui.button("Manage equipment").clicked() {
                navigation.next = Some(Screen::Equipment);
            }
        }
        OperatingTab::Operator => {
            let original = editor
                .expected
                .current()
                .and_then(|value| value.operator.as_ref());
            let call_changed = fields::callsign_changed(
                &editor.operator_callsign,
                original
                    .and_then(|value| value.callsign().as_ref())
                    .map(|call| call.as_str()),
            );
            let mut changed = fields::radio_text(
                ui,
                "operating.operator.call",
                "Operator callsign",
                &mut editor.operator_callsign,
                &frame.errors,
                call_changed,
            );
            let name_changed = fields::text_changed(
                &editor.operator_name,
                original.and_then(|value| value.recorded_name().as_deref()),
            );
            changed |= fields::text(
                ui,
                "operating.operator.name",
                "Operator name",
                &mut editor.operator_name,
                &frame.errors,
                name_changed,
            );
            let original_power = editor
                .expected
                .current()
                .and_then(|value| value.transmit_power);
            let power_changed = crate::operating::power::parse_watts(&editor.power_watts)
                .map(|value| value != original_power)
                .unwrap_or(true);
            changed |= fields::radio_text(
                ui,
                "operating.power",
                "Transmit power",
                &mut editor.power_watts,
                &frame.errors,
                power_changed,
            );
            ui.small("Power is entered in watts (W).");
            if changed {
                frame.validation.edited(ui.input(|input| input.time));
            }
        }
    }
    ui.separator();
    if crate::ui::actions::primary(ui, "Apply", frame.validation.can_submit(&frame.errors))
        .clicked()
    {
        match editor.build(definitions) {
            Ok(next) => {
                let expected = editor.expected.clone();
                frame.send(sender, SettingsAction::Apply { expected, next });
            }
            Err(errors) => frame.errors = errors,
        }
    }
    if ui.button("Cancel operating changes").clicked() {
        navigation.back = true;
    }
}

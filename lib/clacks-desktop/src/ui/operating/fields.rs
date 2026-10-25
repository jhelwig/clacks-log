//! Shared native form fields and explicit source/value presentation.

use super::*;
use crate::operating::{
    equipment::EquipmentForm,
    position::PositionKind,
    station::{ReferenceForm, StationForm},
};
use clacks_client_store::operating::OperatingState;
use clacks_model::position::GridLength;

/// Present a stable-ID text control and its matching validation messages.
pub(in crate::ui) fn text(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    value: &mut String,
    errors: &[FieldError],
) -> bool {
    ui.label(label);
    let changed = ui
        .add(egui::TextEdit::singleline(value).id(egui::Id::unique(id)))
        .changed();
    for error in errors.iter().filter(|e| e.field == label) {
        ui.colored_label(egui::Color32::RED, &error.message);
    }
    changed
}

/// Render all station facts, preserving area/point intent and explicit reference edits.
pub(in crate::ui) fn station(
    ui: &mut egui::Ui,
    id: &str,
    form: &mut StationForm,
    definitions: &DefinitionSet,
    errors: &[FieldError],
) {
    text(
        ui,
        &format!("{id}.call"),
        "Station callsign",
        &mut form.callsign,
        errors,
    );
    let mut grid = form.position.grid.clone();
    if text(
        ui,
        &format!("{id}.grid"),
        "Grid locator (4–12 characters)",
        &mut grid,
        errors,
    ) {
        form.edit_grid(grid);
    }
    let mut latitude = form.position.latitude.clone();
    let mut longitude = form.position.longitude.clone();
    let latitude_changed = text(
        ui,
        &format!("{id}.latitude"),
        "Latitude",
        &mut latitude,
        errors,
    );
    let longitude_changed = text(
        ui,
        &format!("{id}.longitude"),
        "Longitude",
        &mut longitude,
        errors,
    );
    if latitude_changed || longitude_changed {
        form.edit_coordinates(latitude, longitude);
    }
    ui.label(match form.position.kind {
        PositionKind::Grid => "Position: grid area; coordinates show its center",
        PositionKind::Coordinates => "Position: explicit coordinates; locator is derived",
    });
    let mut length = form.position.length;
    if form.position.kind == PositionKind::Coordinates {
        egui::ComboBox::from_id_salt((id, "precision"))
            .selected_text(format!("{}-character locator", length.get()))
            .show_ui(ui, |ui| {
                for size in [4, 6, 8, 10, 12] {
                    if let Ok(candidate) = GridLength::new(size) {
                        ui.selectable_value(&mut length, candidate, format!("{size}"));
                    }
                }
            });
        if length != form.position.length {
            form.edit_precision(length);
        }
    }
    ui.label("Geographic details (manual corrections; automatic lookup comes later)");
    text(
        ui,
        &format!("{id}.dxcc"),
        "DXCC entity",
        &mut form.dxcc,
        errors,
    );
    text(
        ui,
        &format!("{id}.state"),
        "State / province",
        &mut form.primary_subdivision,
        errors,
    );
    text(
        ui,
        &format!("{id}.county"),
        "County / subdivision",
        &mut form.secondary_subdivision,
        errors,
    );
    text(
        ui,
        &format!("{id}.cq"),
        "CQ zone",
        &mut form.cq_zone,
        errors,
    );
    text(
        ui,
        &format!("{id}.itu"),
        "ITU zone",
        &mut form.itu_zone,
        errors,
    );
    ui.separator();
    ui.label("Participation / references");
    let mut remove = None;
    for (index, row) in form.references.iter_mut().enumerate() {
        ui.push_id((id, "reference", index), |ui| {
            let previous = row.group;
            let label = row
                .group
                .map(|id| {
                    definitions
                        .get(id)
                        .map(|g| g.name.clone())
                        .unwrap_or_else(|| format!("Unknown group {id}"))
                })
                .unwrap_or_else(|| "Select group".into());
            egui::ComboBox::from_id_salt("group")
                .selected_text(label)
                .show_ui(ui, |ui| {
                    for group in definitions.iter() {
                        ui.selectable_value(
                            &mut row.group,
                            Some(group.id),
                            format!("{} · {}", group.name, group.id),
                        );
                    }
                });
            row.edited |= previous != row.group;
            row.edited |= text(
                ui,
                &format!("{id}.reference.{index}"),
                "Reference identifier",
                &mut row.identifier,
                errors,
            );
            if ui.button("Remove reference").clicked() {
                remove = Some(index);
            }
        });
    }
    if let Some(index) = remove {
        form.references.remove(index);
    }
    if ui.button("Add reference").clicked() {
        form.references.push(ReferenceForm::new());
    }
}

/// Render equipment values independently of configuration identity or station settings.
pub(in crate::ui) fn equipment(
    ui: &mut egui::Ui,
    id: &str,
    form: &mut EquipmentForm,
    errors: &[FieldError],
) {
    text(
        ui,
        &format!("{id}.description"),
        "Equipment description",
        &mut form.description,
        errors,
    );
    text(
        ui,
        &format!("{id}.antenna"),
        "Antenna",
        &mut form.antenna,
        errors,
    );
    text(
        ui,
        &format!("{id}.notes"),
        "Equipment notes",
        &mut form.notes,
        errors,
    );
}

/// Show applied facts separately from pending edits and reusable definitions.
pub(super) fn active_summary(ui: &mut egui::Ui, state: &OperatingState) {
    ui.label(format!(
        "Station profile: {}",
        state
            .selected_profile
            .as_ref()
            .map(profile_label)
            .unwrap_or_else(|| "None".into())
    ));
    ui.label(format!(
        "Equipment configuration: {}",
        state
            .selected_equipment
            .as_ref()
            .map(equipment_label)
            .unwrap_or_else(|| "None".into())
    ));
    if let Some(current) = &state.current {
        ui.label(format!("Callsign: {}", current.station.callsign.as_str()));
        if let Some(location) = &current.station.location {
            if let Some(position) = &location.position {
                ui.label(format!("Grid: {}", position.display_locator()));
                let coordinates = position.display_coordinates();
                ui.label(format!(
                    "Coordinates{}: {:.6}, {:.6}",
                    if matches!(position, clacks_model::position::Position::GridArea(_)) {
                        " (grid center)"
                    } else {
                        ""
                    },
                    f64::from(coordinates.latitude_microdegrees()) / 1_000_000.0,
                    f64::from(coordinates.longitude_microdegrees()) / 1_000_000.0,
                ));
            }
            if let Some(value) = &location.dxcc {
                ui.label(format!("DXCC: {value}"));
            }
            if let Some(value) = &location.primary_subdivision {
                ui.label(format!("State / province: {value}"));
            }
            if let Some(value) = &location.secondary_subdivision {
                ui.label(format!("County / subdivision: {value}"));
            }
            if let Some(value) = location.cq_zone {
                ui.label(format!("CQ zone: {}", value.get()));
            }
            if let Some(value) = location.itu_zone {
                ui.label(format!("ITU zone: {}", value.get()));
            }
        }
        for reference in &current.station.references {
            ui.label(format!(
                "Reference: {} {}",
                reference.group_id,
                reference.identifier.as_deref().unwrap_or("")
            ));
        }
        if let Some(equipment) = &current.equipment {
            ui.label(format!(
                "Equipment: {} · {}",
                equipment.equipment_description.as_deref().unwrap_or(""),
                equipment.antenna_description.as_deref().unwrap_or("")
            ));
            if let Some(notes) = &equipment.notes {
                ui.label(format!("Equipment notes: {notes}"));
            }
        }
        if let Some(operator) = &current.operator {
            ui.label(format!(
                "Operator: {} {}",
                operator
                    .callsign
                    .as_ref()
                    .map(|c| c.as_str().to_owned())
                    .unwrap_or_default(),
                operator.recorded_name.as_deref().unwrap_or("")
            ));
        }
        if let Some(power) = current.transmit_power {
            ui.label(format!("Transmit power: {} mW", power.get()));
        }
    } else {
        ui.label("No operating settings applied");
    }
}

/// Keep boundary-level and cross-field validation visible alongside field-local feedback.
pub(super) fn unplaced_errors(ui: &mut egui::Ui, errors: &[FieldError]) {
    for error in errors {
        ui.colored_label(
            egui::Color32::RED,
            format!("{}: {}", error.field, error.message),
        );
    }
}

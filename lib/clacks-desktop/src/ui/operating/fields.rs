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
    edited: bool,
) -> bool {
    crate::ui::fields::text(
        ui,
        egui::Id::unique(id),
        label,
        value,
        crate::ui::fields::FieldPresentation {
            edited: edited && ui.is_enabled(),
            read_only: !ui.is_enabled(),
            errors,
            validation_label: None,
            radio_data: false,
        },
    )
    .changed()
}

/// Present a radio-data field using the selected radio typeface and native field feedback.
pub(in crate::ui) fn radio_text(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    value: &mut String,
    errors: &[FieldError],
    edited: bool,
) -> bool {
    crate::ui::fields::text(
        ui,
        egui::Id::unique(id),
        label,
        value,
        crate::ui::fields::FieldPresentation {
            edited: edited && ui.is_enabled(),
            read_only: !ui.is_enabled(),
            errors,
            validation_label: None,
            radio_data: true,
        },
    )
    .changed()
}

/// Attach a native Position validation error to its locator and coordinate controls.
fn position_text(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    value: &mut String,
    errors: &[FieldError],
    edited: bool,
) -> bool {
    crate::ui::fields::text(
        ui,
        egui::Id::unique(id),
        label,
        value,
        crate::ui::fields::FieldPresentation {
            edited: edited && ui.is_enabled(),
            read_only: !ui.is_enabled(),
            errors,
            validation_label: Some("Position"),
            radio_data: true,
        },
    )
    .changed()
}

/// Compare optional text without treating whitespace-only absence as a new recorded value.
pub(in crate::ui) fn text_changed(value: &str, baseline: Option<&str>) -> bool {
    let baseline = baseline.unwrap_or_default();
    if value.trim().is_empty() && baseline.trim().is_empty() {
        false
    } else {
        value != baseline
    }
}

/// Compare callsign input using the same trim/uppercase semantics as recorded callsigns.
pub(in crate::ui) fn callsign_changed(value: &str, baseline: Option<&str>) -> bool {
    value.trim().to_uppercase() != baseline.unwrap_or_default().trim().to_uppercase()
}

/// Render all station facts, preserving area/point intent and explicit reference edits.
pub(in crate::ui) fn station(
    ui: &mut egui::Ui,
    id: &str,
    form: &mut StationForm,
    definitions: &DefinitionSet,
    errors: &[FieldError],
    baseline: Option<&StationForm>,
) -> bool {
    station_fields(ui, id, form, definitions, errors, baseline, true)
}

/// Render additional remote facts without duplicating the quick callsign/grid fields.
pub(in crate::ui) fn station_details(
    ui: &mut egui::Ui,
    id: &str,
    form: &mut StationForm,
    definitions: &DefinitionSet,
    errors: &[FieldError],
    baseline: Option<&StationForm>,
) -> bool {
    station_fields(ui, id, form, definitions, errors, baseline, false)
}

/// Compose the shared station editor with the quick-field ownership chosen by its caller.
fn station_fields(
    ui: &mut egui::Ui,
    id: &str,
    form: &mut StationForm,
    definitions: &DefinitionSet,
    errors: &[FieldError],
    baseline: Option<&StationForm>,
    quick_fields: bool,
) -> bool {
    let call_changed = callsign_changed(
        &form.callsign,
        baseline.map(|value| value.callsign.as_str()),
    );
    let mut changed = false;
    if quick_fields {
        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            ui.strong("Station identity");
            changed |= radio_text(
                ui,
                &format!("{id}.call"),
                "Station callsign",
                &mut form.callsign,
                errors,
                call_changed,
            );
        });
    }
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());
        ui.strong("Operating position");
        let position_changed = match (
            form.position.build(),
            baseline
                .map(|value| value.position.build())
                .unwrap_or(Ok(None)),
        ) {
            (Ok(value), Ok(previous)) => value != previous,
            _ => baseline.is_none_or(|value| value.position != form.position),
        };
        let mut grid = form.position.grid.clone();
        if quick_fields
            && position_text(
                ui,
                &format!("{id}.grid"),
                "Grid locator (4–12 characters)",
                &mut grid,
                errors,
                position_changed,
            )
        {
            form.edit_grid(grid);
            changed = true;
        }
        let mut latitude = form.position.latitude.clone();
        let mut longitude = form.position.longitude.clone();
        let latitude_changed = position_text(
            ui,
            &format!("{id}.latitude"),
            "Latitude",
            &mut latitude,
            errors,
            position_changed,
        );
        let longitude_changed = position_text(
            ui,
            &format!("{id}.longitude"),
            "Longitude",
            &mut longitude,
            errors,
            position_changed,
        );
        if latitude_changed || longitude_changed {
            form.edit_coordinates(latitude, longitude);
            changed = true;
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
                changed = true;
            }
        }
    });
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());
        ui.strong("Geographic details");
        let edited = text_changed(&form.dxcc, baseline.map(|value| value.dxcc.as_str()));
        changed |= radio_text(
            ui,
            &format!("{id}.dxcc"),
            "DXCC entity",
            &mut form.dxcc,
            errors,
            edited,
        );
        let edited = text_changed(
            &form.primary_subdivision,
            baseline.map(|value| value.primary_subdivision.as_str()),
        );
        changed |= radio_text(
            ui,
            &format!("{id}.state"),
            "State / province",
            &mut form.primary_subdivision,
            errors,
            edited,
        );
        let edited = text_changed(
            &form.secondary_subdivision,
            baseline.map(|value| value.secondary_subdivision.as_str()),
        );
        changed |= radio_text(
            ui,
            &format!("{id}.county"),
            "County / subdivision",
            &mut form.secondary_subdivision,
            errors,
            edited,
        );
        let edited = zone_changed(&form.cq_zone, baseline.map(|value| value.cq_zone.as_str()));
        changed |= radio_text(
            ui,
            &format!("{id}.cq"),
            "CQ zone",
            &mut form.cq_zone,
            errors,
            edited,
        );
        let edited = zone_changed(
            &form.itu_zone,
            baseline.map(|value| value.itu_zone.as_str()),
        );
        changed |= radio_text(
            ui,
            &format!("{id}.itu"),
            "ITU zone",
            &mut form.itu_zone,
            errors,
            edited,
        );
    });
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());
        ui.strong("Participation / references");
        let mut remove = None;
        for (index, row) in form.references.iter_mut().enumerate() {
            ui.push_id((id, "reference", index), |ui| {
                let previous = row.group;
                let label = row
                    .group
                    .map(|id| {
                        definitions
                            .get(id)
                            .map(|g| g.name().to_owned())
                            .unwrap_or_else(|| format!("Unknown group {id}"))
                    })
                    .unwrap_or_else(|| "Select group".into());
                egui::ComboBox::from_id_salt("group")
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        for group in definitions.iter() {
                            ui.selectable_value(
                                &mut row.group,
                                Some(group.id()),
                                format!("{} · {}", group.name(), group.id()),
                            );
                        }
                    });
                let group_changed = previous != row.group;
                row.edited |= group_changed;
                changed |= group_changed;
                let reference_changed = row.is_changed(definitions);
                let input_changed = radio_text(
                    ui,
                    &format!("{id}.reference.{index}"),
                    "Reference identifier",
                    &mut row.identifier,
                    errors,
                    reference_changed,
                );
                row.edited |= input_changed;
                changed |= input_changed;
                if ui.button("Remove reference").clicked() {
                    remove = Some(index);
                }
            });
        }
        if let Some(index) = remove {
            form.references.remove(index);
            changed = true;
        }
        if ui.button("Add reference").clicked() {
            form.references.push(ReferenceForm::new());
            changed = true;
        }
    });
    changed
}

/// Compare recognized numeric zone input without creating edits from zero padding.
fn zone_changed(value: &str, baseline: Option<&str>) -> bool {
    let baseline = baseline.unwrap_or_default();
    match (value.trim().parse::<u8>(), baseline.trim().parse::<u8>()) {
        (Ok(value), Ok(previous)) => value != previous,
        _ => text_changed(value, Some(baseline)),
    }
}

/// Render equipment values independently of configuration identity or station settings.
pub(in crate::ui) fn equipment(
    ui: &mut egui::Ui,
    id: &str,
    form: &mut EquipmentForm,
    errors: &[FieldError],
    baseline: Option<&EquipmentForm>,
) -> bool {
    let edited = text_changed(
        &form.description,
        baseline.map(|value| value.description.as_str()),
    );
    let mut changed = text(
        ui,
        &format!("{id}.description"),
        "Equipment description",
        &mut form.description,
        errors,
        edited,
    );
    let edited = text_changed(&form.antenna, baseline.map(|value| value.antenna.as_str()));
    changed |= text(
        ui,
        &format!("{id}.antenna"),
        "Antenna",
        &mut form.antenna,
        errors,
        edited,
    );
    let edited = text_changed(&form.notes, baseline.map(|value| value.notes.as_str()));
    changed |= text(
        ui,
        &format!("{id}.notes"),
        "Equipment notes",
        &mut form.notes,
        errors,
        edited,
    );
    changed
}

/// Show applied facts separately from pending edits and reusable definitions.
pub(super) fn active_summary(ui: &mut egui::Ui, state: &OperatingState) {
    ui.label(format!(
        "Station profile: {}",
        state
            .selected_profile()
            .map(profile_label)
            .unwrap_or_else(|| "None".into())
    ));
    ui.label(format!(
        "Equipment configuration: {}",
        state
            .selected_equipment()
            .map(equipment_label)
            .unwrap_or_else(|| "None".into())
    ));
    if let Some(current) = &state.current() {
        ui.label(format!("Callsign: {}", current.station.callsign.as_str()));
        if let Some(location) = &current.station.location {
            if let Some(position) = &location.position() {
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
            if let Some(value) = &location.dxcc() {
                ui.label(format!("DXCC: {value}"));
            }
            if let Some(value) = &location.primary_subdivision() {
                ui.label(format!("State / province: {value}"));
            }
            if let Some(value) = &location.secondary_subdivision() {
                ui.label(format!("County / subdivision: {value}"));
            }
            if let Some(value) = location.cq_zone() {
                ui.label(format!("CQ zone: {}", value.get()));
            }
            if let Some(value) = location.itu_zone() {
                ui.label(format!("ITU zone: {}", value.get()));
            }
        }
        for reference in &current.station.references {
            ui.label(format!(
                "Reference: {} {}",
                reference.group_id,
                reference.identifier().as_deref().unwrap_or("")
            ));
        }
        if let Some(equipment) = &current.equipment {
            ui.label(format!(
                "Equipment: {} · {}",
                equipment.equipment_description().as_deref().unwrap_or(""),
                equipment.antenna_description().as_deref().unwrap_or("")
            ));
            if let Some(notes) = &equipment.notes() {
                ui.label(format!("Equipment notes: {notes}"));
            }
        }
        if let Some(operator) = &current.operator {
            ui.label(format!(
                "Operator: {} {}",
                operator
                    .callsign()
                    .as_ref()
                    .map(|c| c.as_str().to_owned())
                    .unwrap_or_default(),
                operator.recorded_name().as_deref().unwrap_or("")
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
            ui.visuals().error_fg_color,
            format!("{}: {}", error.field, error.message),
        );
    }
}

//! Review source disputes, candidate assignments, and edits to saved contacts.

use crate::{
    import::{
        mapping::Choice,
        workflow::{Action, PreparedImport, tags},
    },
    review::fields::{FieldId, record::NativeFields},
    ui::{
        feedback,
        review::{
            choices::{self, SourceChoice},
            value,
        },
    },
};
use clacks_client_store::settings::SettingsSnapshot;
use clacks_model::{
    contact::Qso,
    group::DefinitionSet,
    ids::{QsoId, TagId},
};
use eframe::egui;
use std::collections::BTreeMap;

pub(super) fn review_row(ui: &mut egui::Ui, prepared: &mut PreparedImport, row: usize) {
    let Some(review) = prepared.rows.get(row) else {
        return;
    };
    ui.heading(format!("Source row {}", review.source.number));
    for warning in &review.source.warnings {
        feedback::problem(ui, &warning.message);
    }

    let mut selected = None;
    for (index, dispute) in review.source.active_disputes(&review.choices) {
        ui.group(|ui| {
            ui.label(format!("Disputed {}", dispute.fact));
            let choice = review.choices.get(&index);
            for (alternative, value) in dispute.alternatives.iter().enumerate() {
                let option = Choice::Use(alternative);
                if ui
                    .radio(
                        choice == Some(&option),
                        format!("{} · {}", value.source, value.value),
                    )
                    .on_hover_text(format!(
                        "Source bytes {}–{}",
                        value.span.start, value.span.end
                    ))
                    .clicked()
                {
                    selected = Some((index, option));
                }
            }
            if ui
                .radio(choice == Some(&Choice::Omit), "Omit disputed fact")
                .clicked()
            {
                selected = Some((index, Choice::Omit));
            }
        });
    }
    if let Some((index, choice)) = selected {
        prepared.resolve(row, index, choice);
    }

    let Some(review) = prepared.rows.get(row) else {
        return;
    };
    let action = prepared.action(row);
    let source_resolved = review.source_resolved();
    let mut next = None;
    let tag_names = tags(&prepared.settings);
    let definitions = prepared.catalogs().definitions();
    for candidate in &review.candidates {
        if let Some(qso) = prepared.saved(candidate.id) {
            let assigned = prepared.other_assignment(row, candidate.id);
            let mut label = format!(
                "Use saved {} · {} · {} UTC",
                qso.contacted_station().callsign.as_str(),
                qso.own_station().callsign.as_str(),
                crate::logging::time::format(qso.start())
            );
            if candidate.seconds_apart != 0 {
                label.push_str(&format!(" · time differs by {}s", candidate.seconds_apart));
            }
            if let Some(band) = qso.primary().band() {
                label.push_str(&format!(" · {}", band.label()));
            }
            use clacks_model::recognition::mode::ModeIdentity;
            let (ModeIdentity::Parent { label: mode }
            | ModeIdentity::Specialization { label: mode, .. }) = qso.mode().identity();
            label.push_str(&format!(" · {mode}"));
            for gap in &candidate.gaps {
                label.push_str(&format!(" · {}", gap.description()));
            }
            if let Some(other) = assigned {
                label.push_str(&format!(" · assigned to row {other}"));
            }
            if ui
                .add_enabled(
                    assigned.is_none() && source_resolved,
                    egui::RadioButton::new(action == Some(Action::Update(candidate.id)), label),
                )
                .on_hover_text(format!("Saved QSO {}", candidate.id))
                .clicked()
            {
                next = Some(Action::Update(candidate.id));
            }
            if action != Some(Action::Update(candidate.id)) {
                candidate_details(ui, row, qso, &tag_names, definitions, &prepared.settings);
            }
        }
    }
    if ui
        .add_enabled(
            source_resolved,
            egui::RadioButton::new(action == Some(Action::Add), "Create a separate QSO"),
        )
        .clicked()
    {
        next = Some(Action::Add);
    }
    if ui
        .radio(action == Some(Action::Drop), "Drop this row")
        .clicked()
    {
        next = Some(Action::Drop);
    }
    if let Some(next) = next {
        prepared.select(row, next);
    }
    match prepared.action(row) {
        Some(Action::Add) => {
            if let Some(review) = prepared.rows.get_mut(row)
                && review.needs_station()
            {
                ui.horizontal(|ui| {
                    ui.label("Historical own-station callsign");
                    ui.text_edit_singleline(&mut review.historical_call);
                });
            }
        }
        Some(Action::Update(target)) => review_fields(ui, prepared, row, target, &tag_names),
        Some(Action::Drop) | None => {}
    }
    if let Err(error) = prepared.readiness(row) {
        feedback::problem(ui, &error.to_string());
    }
}

fn candidate_details(
    ui: &mut egui::Ui,
    row: usize,
    qso: &Qso,
    tags: &BTreeMap<TagId, String>,
    definitions: &DefinitionSet,
    settings: &SettingsSnapshot,
) {
    egui::CollapsingHeader::new("Saved QSO details")
        .id_salt(("adi_candidate", row, qso.id()))
        .show(ui, |ui| {
            let saved = NativeFields::Qso(Box::new(qso.clone().into_fields()));
            saved_qso_id(ui, qso.id());
            let max_width = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
            egui::Grid::new("saved_qso_fields")
                .num_columns(2)
                .max_col_width(max_width)
                .show(ui, |ui| {
                    for field in saved.fields() {
                        ui.label(field.label());
                        saved_value(ui, field, &saved, tags, definitions, settings);
                        ui.end_row();
                    }
                });
        });
}

fn saved_qso_id(ui: &mut egui::Ui, target: QsoId) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Saved QSO ID");
        ui.monospace(target.to_string());
    });
}

fn saved_value(
    ui: &mut egui::Ui,
    field: FieldId,
    saved: &NativeFields,
    tags: &BTreeMap<TagId, String>,
    definitions: &DefinitionSet,
    settings: &SettingsSnapshot,
) {
    match saved.input(field) {
        Ok(input) => value::show(ui, field, &input, tags, definitions, settings),
        Err(error) => feedback::problem(ui, &error.to_string()),
    }
}

fn review_fields(
    ui: &mut egui::Ui,
    prepared: &mut PreparedImport,
    row: usize,
    target: QsoId,
    tag_names: &BTreeMap<TagId, String>,
) {
    let (saved, differences) = match prepared.review_comparison(row, target) {
        Ok(comparison) => comparison,
        Err(error) => {
            feedback::problem(ui, &error.to_string());
            return;
        }
    };
    let definitions = prepared.catalogs().definitions();
    let settings = &prepared.settings;
    let Some(review) = prepared.rows.get(row) else {
        return;
    };
    let mut picked = vec![];
    if differences.is_empty() {
        ui.label("Contact facts agree.");
    } else {
        ui.horizontal_wrapped(|ui| {
            if let Some(incoming) = choices::choose_all(
                ui,
                [
                    (true, "Use all imported values", true),
                    (false, "Keep all saved values", true),
                ],
            ) {
                picked.extend(differences.iter().map(|value| (value.field, incoming)));
            }
        });
    }
    saved_qso_id(ui, target);
    let differences: BTreeMap<_, _> = differences
        .iter()
        .map(|difference| (difference.field, difference))
        .collect();
    let max_width = (ui.available_width() - 2.0 * ui.spacing().item_spacing.x) / 3.0;
    egui::Grid::new(("adi_review_fields", row, target))
        .num_columns(3)
        .max_col_width(max_width)
        .striped(true)
        .show(ui, |ui| {
            ui.strong("Field");
            ui.strong("Saved");
            ui.strong("Proposed update");
            ui.end_row();
            for field in saved.fields() {
                ui.label(field.label());
                if let Some(difference) = differences.get(&field) {
                    let selected = review.fields.get(&field).copied();
                    for (incoming, input) in
                        [(false, &difference.saved), (true, &difference.incoming)]
                    {
                        ui.vertical(|ui| {
                            let choice = SourceChoice {
                                label: if incoming {
                                    "Use incoming"
                                } else {
                                    "Keep saved"
                                },
                                selected: selected == Some(incoming),
                                input: Ok(input),
                            };
                            if choice.show(ui, field, tag_names, definitions, settings) {
                                picked.push((field, incoming));
                            }
                        });
                    }
                } else {
                    saved_value(ui, field, &saved, tag_names, definitions, settings);
                }
                ui.end_row();
            }
        });

    if let Some(review) = prepared.rows.get_mut(row) {
        review.fields.extend(picked);
    }
}

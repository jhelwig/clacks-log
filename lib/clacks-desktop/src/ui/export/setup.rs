//! Export scope, format, and preflight decisions.

use super::{
    ActiveExport, Phase,
    report::{contact_label, show_affected_contacts},
};
use crate::{
    export::{ExportFormat, SkippedContact, publish::file_names},
    session::{SessionCommand, export::Scope},
};
use clacks_model::{
    export::select::{ContextResolution, ExportDecision},
    scalar::UtcSecond,
};
use eframe::egui;
use futures_util::future::BoxFuture;
use std::path::PathBuf;
use tokio::sync::mpsc;

impl ActiveExport {
    pub(super) fn show_setup(
        &mut self,
        ui: &mut egui::Ui,
        commands: &mpsc::Sender<SessionCommand>,
        parent: &eframe::Frame,
        picker: &mut Option<BoxFuture<'static, Option<PathBuf>>>,
    ) -> bool {
        let previous = (self.scope, self.format);
        self.show_scope(ui);
        ui.separator();
        ui.strong("Export format");
        ui.radio_value(&mut self.format, ExportFormat::Ordinary, "Ordinary ADI");
        ui.radio_value(&mut self.format, ExportFormat::Pota, "POTA-compatible");
        if previous != (self.scope, self.format) {
            self.prepare(commands, false);
        }
        let mut ready = false;
        if let Some(plan) = self.plan.clone() {
            ready = plan.eligible_count() > 0 && plan.decisions.is_empty();
            if self.format == ExportFormat::Pota && !plan.skipped.is_empty() {
                ui.indent("pota.readiness", |ui| {
                    show_skipped(
                        ui,
                        plan.requested_count,
                        &plan.skipped,
                        self.capture.as_deref(),
                    )
                });
            } else if !plan.decisions.is_empty() {
                self.show_decisions(ui, commands, &plan.decisions);
            }
            if plan.requested_count == 0 {
                ui.label("This scope contains no saved contacts.");
            }
        } else if self.pending.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Preparing export…");
            });
        }
        ui.separator();
        let mut dismiss = false;
        ui.horizontal(|ui| {
            dismiss = ui.button("Cancel").clicked();
            if !ready
                && self.pending.is_none()
                && self.problem.is_some()
                && ui.button("Retry preparation").clicked()
            {
                self.prepare(commands, self.capture.is_some());
            }
            if ui
                .add_enabled(ready, egui::Button::new("Continue…"))
                .clicked()
            {
                match self.format {
                    ExportFormat::Ordinary => self.open_picker(ui.ctx(), parent, picker),
                    ExportFormat::Pota => {
                        if let Some(plan) = &self.plan {
                            let now = UtcSecond::from_unix_seconds(
                                time::OffsetDateTime::now_utc().unix_timestamp(),
                            );
                            match file_names(plan, now) {
                                Ok(names) => {
                                    self.names = names;
                                    self.phase = Phase::Files;
                                }
                                Err(error) => self.problem = Some(error.to_string()),
                            }
                        }
                    }
                }
            }
        });
        dismiss
    }

    fn show_scope(&mut self, ui: &mut egui::Ui) {
        let counts = self.capture.as_ref().map(|capture| &capture.counts);
        ui.strong("Which contacts?");
        ui.add_enabled_ui(!self.history.selected.is_empty(), |ui| {
            ui.radio_value(&mut self.scope, Scope::Selected, "Selected contacts");
        });
        if let Some(counts) = counts {
            ui.indent("selected.count", |ui| {
                ui.label(format!(
                    "{} contacts, including {} outside the current view and filter",
                    counts.selected, counts.hidden
                ));
            });
        }
        ui.radio_value(
            &mut self.scope,
            Scope::Matching,
            "Contacts matching the current view and filter",
        );
        ui.indent("matching.count", |ui| {
            let count = counts
                .map(|counts| format!("{} contacts · ", counts.matching))
                .unwrap_or_default();
            let expression = if self.applied.trim().is_empty() {
                String::new()
            } else {
                format!(" · {}", self.applied)
            };
            ui.label(format!("{count}{}{expression}", self.view_label));
        });
        ui.radio_value(
            &mut self.scope,
            Scope::All,
            "All contacts in this workspace",
        );
        if let Some(counts) = counts {
            ui.indent("all.count", |ui| {
                ui.label(format!("{} contacts across all logbooks", counts.all));
            });
        }
    }

    fn show_decisions(
        &mut self,
        ui: &mut egui::Ui,
        commands: &mpsc::Sender<SessionCommand>,
        decisions: &[ExportDecision],
    ) {
        ui.label("Choose a contest for each QSO below or omit its contest identifier.");
        let mut changed = false;
        for ExportDecision::AmbiguousContextField {
            qso_id,
            field,
            candidates,
        } in decisions
        {
            let label = contact_label(self.capture.as_deref(), *qso_id, "Contact");
            ui.push_id(qso_id, |ui| {
                let label = ui.label(format!("{label} · {field}"));
                let mut value = self.resolutions.get(qso_id).cloned();
                let selected = match &value {
                    Some(ContextResolution::ChooseTarget(group)) => candidates
                        .get(group)
                        .map(String::as_str)
                        .unwrap_or("Choose a contest"),
                    Some(ContextResolution::OmitReported) => "Omit contest identifier",
                    None => "Choose a contest",
                };
                egui::ComboBox::from_id_salt("contest")
                    .selected_text(selected)
                    .show_ui(ui, |ui| {
                        for (group, name) in candidates {
                            ui.selectable_value(
                                &mut value,
                                Some(ContextResolution::ChooseTarget(*group)),
                                name,
                            );
                        }
                        ui.selectable_value(
                            &mut value,
                            Some(ContextResolution::OmitReported),
                            "Omit contest identifier",
                        );
                    })
                    .response
                    .labelled_by(label.id);
                if let Some(value) = value
                    && self.resolutions.get(qso_id) != Some(&value)
                {
                    self.resolutions.insert(*qso_id, value);
                    changed = true;
                }
            });
        }
        if changed && self.resolutions.len() == decisions.len() {
            self.prepare(commands, true);
        }
    }
}

fn show_skipped(
    ui: &mut egui::Ui,
    requested: usize,
    skipped: &[SkippedContact],
    capture: Option<&crate::session::export::Capture>,
) {
    let count = skipped.len();
    let summary = if count == requested {
        format!("All {requested} QSOs are missing required POTA information.")
    } else if count == 1 {
        format!(
            "1 of {requested} QSOs is missing required POTA information. Continuing will skip it."
        )
    } else {
        format!(
            "{count} of {requested} QSOs are missing required POTA information. Continuing will skip those {count}."
        )
    };
    ui.label(summary);
    egui::CollapsingHeader::new("Show affected QSOs").show(ui, |ui| {
        show_affected_contacts(ui, skipped, capture);
    });
}

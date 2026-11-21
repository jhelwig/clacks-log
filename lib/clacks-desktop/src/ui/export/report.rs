//! Reviewed file names and final export reporting.

use super::super::feedback;
use crate::{
    export::{
        PreparedExport, SkippedContact,
        publish::{ExportDestination, ExportReport, PlannedFileName, PublishError},
    },
    logging,
    session::export::Capture,
};
use clacks_model::ids::QsoId;
use eframe::egui;

pub(super) fn show_files(ui: &mut egui::Ui, names: &[PlannedFileName]) {
    ui.strong("Review POTA files");
    for file in names {
        ui.label(format!(
            "{} · {}",
            file.name(),
            quantity(file.entry_count(), "entry", "entries")
        ));
    }
}

pub(super) fn show_result(
    ui: &mut egui::Ui,
    destination: &ExportDestination,
    outcome: Option<&Result<ExportReport, PublishError>>,
    plan: Option<&PreparedExport>,
    capture: Option<&Capture>,
) {
    let path = match destination {
        ExportDestination::Ordinary { path, .. } => path,
        ExportDestination::Pota { directory, .. } => directory,
    };
    let Some(outcome) = outcome else {
        ui.strong("Export did not finish");
        ui.label(path.display().to_string());
        ui.label("Check the destination before trying again.");
        return;
    };
    let effects = match outcome {
        Ok(report) => {
            ui.strong("Export complete");
            report
        }
        Err(error) => {
            ui.strong(if error.is_cancelled() {
                "Export cancelled"
            } else {
                "Export failed"
            });
            if !error.is_cancelled() {
                feedback::problem(ui, &error.to_string());
            }
            error.report()
        }
    };
    ui.label(path.display().to_string());
    let skipped = plan.map_or(0, |value| value.skipped.len());
    ui.label(format!(
        "{} exported · {skipped} skipped · {} · {}",
        quantity(effects.qsos_exported, "QSO", "QSOs"),
        quantity(effects.entries_written, "entry", "entries"),
        quantity(effects.paths.len(), "file", "files"),
    ));
    if outcome.is_err() && !effects.paths.is_empty() {
        ui.strong("Files published before the failure");
        for path in &effects.paths {
            ui.label(path.display().to_string());
        }
    }
    if let Some(plan) = plan {
        if !plan.skipped.is_empty() {
            egui::CollapsingHeader::new("Skipped QSOs").show(ui, |ui| {
                show_affected_contacts(ui, &plan.skipped, capture);
            });
        }
        if !plan.warnings.is_empty() {
            egui::CollapsingHeader::new(format!(
                "Conversion and omission details ({})",
                plan.warnings.len()
            ))
            .show(ui, |ui| {
                for warning in &plan.warnings {
                    let label = contact_label(capture, warning.qso_id, &warning.callsign);
                    ui.label(format!("{label} · {}. {}", warning.field, warning.detail));
                }
            });
        }
    }
}

pub(super) fn show_affected_contacts(
    ui: &mut egui::Ui,
    skipped: &[SkippedContact],
    capture: Option<&Capture>,
) {
    for contact in skipped {
        let fields = contact
            .missing
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let label = contact_label(capture, contact.qso_id, &contact.callsign);
        ui.label(format!("{label} · {fields}"));
    }
}

pub(super) fn contact_label(capture: Option<&Capture>, id: QsoId, fallback: &str) -> String {
    if let Some(capture) = capture
        && let Ok(index) = capture
            .input
            .contacts
            .binary_search_by_key(&id, |contact| contact.qso.id())
    {
        let qso = &capture.input.contacts[index].qso;
        format!(
            "{} · {} UTC",
            qso.contacted_station().callsign.as_str(),
            logging::time::format(qso.start())
        )
    } else {
        fallback.to_owned()
    }
}

fn quantity(count: usize, singular: &str, plural: &str) -> String {
    let label = if count == 1 { singular } else { plural };
    format!("{count} {label}")
}

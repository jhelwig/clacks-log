//! Account-wide individual contact deletion confirmation.

use clacks_sync::{
    client::ClientRecord,
    record::{RecordState, RecordValue},
};
use eframe::egui;

/// The exact contact shown in the deletion prompt.
pub(super) struct DeletionConfirmation {
    /// Expected record retained until confirmation settles.
    pub expected: ClientRecord,
    /// Initially place focus on Cancel rather than the destructive action.
    pub first_frame: bool,
}

/// Explicit choice from the contact deletion prompt.
#[derive(Default)]
pub(super) enum DeletionAction {
    /// Keep the prompt open.
    #[default]
    None,
    /// Cancel without changing the contact.
    Cancel,
    /// Delete the reviewed record across all logbooks.
    Delete,
}

/// Explain the account-wide effect before any deletion request is admitted.
pub(super) fn show(
    context: &egui::Context,
    confirmation: &mut DeletionConfirmation,
    busy: bool,
) -> DeletionAction {
    let result = super::super::dialog::show(
        context,
        egui::Id::unique("contacts.delete"),
        "Delete QSO?",
        |ui| {
            if let Some(RecordState::Live(RecordValue::Qso(qso))) =
                confirmation.expected.working_state()
            {
                ui.monospace(format!(
                    "{} · {} · {}",
                    qso.contacted_station.callsign.as_str(),
                    crate::logging::time::format(qso.start),
                    qso.primary
                        .band()
                        .map(|v| v.label())
                        .unwrap_or("Unclassified frequency")
                ));
                let mode = match qso.mode.identity() {
                    clacks_model::recognition::mode::ModeIdentity::Parent { label }
                    | clacks_model::recognition::mode::ModeIdentity::Specialization {
                        label, ..
                    } => label,
                };
                ui.monospace(mode);
            }
            ui.label(
                "Delete this contact across all logbooks. This is not removing a logbook tag.",
            );
            let mut action = DeletionAction::None;
            ui.horizontal(|ui| {
                let cancel = ui.add_enabled(!busy, egui::Button::new("Cancel"));
                if confirmation.first_frame {
                    cancel.request_focus();
                }
                if cancel.clicked() {
                    action = DeletionAction::Cancel;
                }
                if ui
                    .add_enabled(!busy, egui::Button::new("Delete QSO"))
                    .clicked()
                {
                    action = DeletionAction::Delete;
                }
            });
            action
        },
    );
    confirmation.first_frame = false;
    if result.cancel_requested && !busy {
        DeletionAction::Cancel
    } else {
        result.inner
    }
}

//! Useful startup, identity and degraded-diagnostic presentation.

use super::DesktopApp;
use crate::session::{SessionCommand, SessionStatus};
use eframe::egui;

/// Present persisted readiness without inventing connection or future workflow status.
pub(super) fn show(app: &mut DesktopApp, ui: &mut egui::Ui, state: &SessionStatus) {
    ui.label(format!("Workspace: {}", app.paths.directory.display()));
    ui.label(format!(
        "Diagnostic logs: {}",
        app.paths.log_directory.display()
    ));
    ui.separator();

    match state {
        SessionStatus::Opening => {
            ui.spinner();
            ui.label("Opening local account…");
        }
        SessionStatus::Ready {
            identity,
            has_operating_location,
        } => {
            ui.label("Local workspace ready");
            ui.label(format!("Account: {}", identity.account));
            ui.label(if identity.enrolled {
                "Enrolled account"
            } else {
                "Standalone account"
            });
            ui.label(if *has_operating_location {
                "Operating defaults loaded"
            } else {
                "No operating defaults selected"
            });
        }
        SessionStatus::Failed { message } => {
            ui.label(message);
            if !app.close_requested && ui.button("Retry").clicked() {
                if app.commands.try_send(SessionCommand::RetryOpen).is_err() {
                    app.command_error = Some("Unable to request retry. The background session may have stopped; close and relaunch.".into());
                } else {
                    app.command_error = None;
                }
            }
        }
        SessionStatus::Closing { .. } => {
            ui.label("Closing…");
        }
        SessionStatus::Closed => {
            ui.label("Closed");
        }
    }

    for diagnostic in &app.diagnostics {
        ui.label(diagnostic);
    }

    let dropped = app.loss.dropped_lines();
    if app.file_failure.load(std::sync::atomic::Ordering::Relaxed) {
        ui.label("Local log file writes failed; diagnostics are using stderr. Check disk space and permissions, then relaunch to restore file output.");
    }
    if dropped > 0 {
        ui.label(format!(
            "Diagnostic output is overloaded: {dropped} log lines dropped."
        ));
    }

    if let Some(error) = &app.command_error {
        ui.label(error);
    }
}

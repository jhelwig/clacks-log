//! Compact workspace status, lifecycle presentation and actionable diagnostics.

use super::{DesktopApp, theme};
use crate::session::{SessionCommand, SessionStatus};
use eframe::egui;

/// Show current local readiness without implying a server connection.
pub(super) fn status(ui: &mut egui::Ui, state: &SessionStatus) {
    let label = match state {
        SessionStatus::Opening => "Opening local workspace…",
        SessionStatus::Ready { identity, .. } if identity.enrolled => {
            "Local workspace ready · Awaiting initial sync"
        }
        SessionStatus::Ready { .. } => "Local workspace ready · Standalone account",
        SessionStatus::Failed { .. } => "Workspace needs attention",
        SessionStatus::Closing { .. } => "Closing workspace…",
        SessionStatus::Closed => "Workspace closed",
    };
    ui.small(egui::RichText::new(label).color(theme::palette(ui.ctx().theme()).muted));
}

/// Present an active opening or failure state with its real recovery action.
pub(super) fn show(app: &mut DesktopApp, ui: &mut egui::Ui, state: &SessionStatus) {
    if !matches!(state, SessionStatus::Opening | SessionStatus::Failed { .. }) {
        return;
    }
    theme::section_frame(ui).show(ui, |ui| {
        ui.heading("Local workspace");
        ui.separator();
        match state {
            SessionStatus::Opening => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Opening local account…");
                });
            }
            SessionStatus::Failed { message } => {
                ui.colored_label(ui.visuals().error_fg_color, message);
                if !app.close_requested && ui.button("Retry").clicked() {
                    if app.commands.try_send(SessionCommand::RetryOpen).is_err() {
                        app.command_error = Some("Unable to request retry. The background session may have stopped; close and relaunch.".into());
                    } else {
                        app.command_error = None;
                    }
                }
            }
            _ => {}
        }
    });
}

/// Keep workspace identity and file locations available outside the operating flow.
pub(super) fn details(app: &DesktopApp, ui: &mut egui::Ui, state: &SessionStatus) {
    ui.menu_button("Workspace & diagnostics", |ui| {
        ui.label(format!("Workspace: {}", app.paths.directory.display()));
        ui.label(format!(
            "Diagnostic logs: {}",
            app.paths.log_directory.display()
        ));
        if let SessionStatus::Ready { identity, .. } = state {
            ui.monospace(format!("Account: {}", identity.account));
        }
    });
}

/// Present diagnostic degradation even when workspace details are collapsed.
pub(super) fn diagnostics(app: &DesktopApp, ui: &mut egui::Ui) {
    let dropped = app.loss.dropped_lines();
    let file_failed = app.file_failure.load(std::sync::atomic::Ordering::Relaxed);
    if app.diagnostics.is_empty() && dropped == 0 && !file_failed && app.command_error.is_none() {
        return;
    }
    theme::section_frame(ui).show(ui, |ui| {
        ui.heading("Diagnostics need attention");
        ui.separator();
        for diagnostic in &app.diagnostics {
            ui.colored_label(ui.visuals().warn_fg_color, diagnostic);
        }

        if file_failed {
            ui.colored_label(ui.visuals().warn_fg_color, "Local log file writes failed; diagnostics are using stderr. Check disk space and permissions, then relaunch to restore file output.");
        }
        if dropped > 0 {
            ui.colored_label(ui.visuals().warn_fg_color, format!("Diagnostic output is overloaded: {dropped} log lines dropped."));
        }
        if let Some(error) = &app.command_error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    });
}

//! Explicit delayed-close choice; no automatic process-exit deadline.

use super::DesktopApp;
use eframe::egui;
use std::time::{Duration, Instant};

/// Delay before presenting the explicit force-quit choice.
const SHUTDOWN_NOTICE_DELAY: Duration = Duration::from_secs(5);

/// Determine whether cleanup has exceeded the operator-facing notice threshold.
pub(super) fn delayed(started: Instant) -> bool {
    started.elapsed() >= SHUTDOWN_NOTICE_DELAY
}

/// Keep cleanup visible while allowing a deliberate immediate process termination.
pub(super) fn show(app: &mut DesktopApp, ui: &mut egui::Ui, phase: &str, started: Instant) {
    ui.label(phase);
    ui.spinner();
    if delayed(started) && !app.continuing {
        ui.label("Shutdown is taking longer than usual. Force Quit can lose unfinished work and diagnostics.");
        if ui.button("Continue Waiting").clicked() {
            app.continuing = true;
        }
        if ui.button("Force Quit").clicked() {
            tracing::warn!(event = "desktop.force_quit_selected");
            std::process::exit(1);
        }
    }
}

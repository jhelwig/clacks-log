//! Existing workspace details presented as a dismissible Help destination.
use crate::{session::SessionStatus, workspace::WorkspacePaths};
use eframe::egui;

/// No duplicate diagnostic state: contents are read from the application.
#[derive(Default)]
pub(super) struct DiagnosticsUi {
    /// Whether the details dialog is visible.
    open: bool,
    /// Focus before opening a native menu action.
    opener: Option<egui::Id>,
    /// Restore focus after the modal has left the frame.
    restore_focus: Option<egui::Id>,
}
impl DiagnosticsUi {
    /// Whether this dialog owns modal input.
    pub fn is_open(&self) -> bool {
        self.open
    }
    /// Repeated opens do not overwrite the original focus target.
    pub fn open(&mut self, opener: Option<egui::Id>) {
        if !self.open {
            self.open = true;
            self.opener = opener;
        }
    }
    /// Show the same identity/path fields as the former toolbar popup.
    pub fn show(&mut self, context: &egui::Context, paths: &WorkspacePaths, state: &SessionStatus) {
        if let Some(id) = self.restore_focus.take() {
            context.memory_mut(|m| m.request_focus(id));
        }
        if !self.open {
            return;
        }
        let response = super::dialog::show(
            context,
            egui::Id::unique("workspace.details"),
            "Workspace & diagnostics",
            |ui| {
                ui.label(format!("Workspace: {}", paths.directory.display()));
                ui.label(format!(
                    "Diagnostic logs: {}",
                    paths.log_directory.display()
                ));
                if let SessionStatus::Ready { identity, .. } = state {
                    ui.monospace(format!("Account: {}", identity.account));
                }
                ui.button("Done").clicked()
            },
        );
        if response.cancel_requested || response.inner {
            self.open = false;
            self.restore_focus = self.opener.take();
            context.request_repaint();
        }
    }
}

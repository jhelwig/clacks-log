//! Native presentation and lifecycle requests, with no database work in callbacks.

mod appearance;
mod closing;
mod dialog;
mod equipment;
mod feedback;
mod fields;
pub(crate) mod fonts;
mod logbooks;
mod operating;
mod preferences;
mod profiles;
mod startup;
#[cfg(test)]
mod tests;
mod theme;
mod validation;

use crate::{
    session::{SessionCommand, SessionStatus, settings::SettingsRequest},
    workspace::WorkspacePaths,
};
use clacks_client_store::settings::SettingsSnapshot;
use eframe::egui;
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};
use tokio::sync::{mpsc, watch};
use tracing_appender::non_blocking::ErrorCounter;

/// Runtime handles and diagnostic resources transferred to the native window.
pub(crate) struct UiResources {
    /// Workspace lifecycle notifications.
    pub status: watch::Receiver<SessionStatus>,
    /// Existing-instance activation notifications.
    pub activation: watch::Receiver<()>,
    /// Lifecycle commands owned by the native window.
    pub commands: mpsc::Sender<SessionCommand>,
    /// Domain request admission without UI-thread database work.
    pub settings_requests: mpsc::Sender<SettingsRequest>,
    /// Last usable committed settings snapshot.
    pub settings_view: watch::Receiver<Option<Arc<SettingsSnapshot>>>,
    /// Workspace paths displayed directly to the user.
    pub paths: WorkspacePaths,
    /// Startup diagnostic notices.
    pub diagnostics: Vec<String>,
    /// Standard logger loss counter.
    pub loss: ErrorCounter,
    /// Shared local-file failure indicator.
    pub file_failure: Arc<AtomicBool>,
}

/// Window-owned view state and standard communication handles.
pub(crate) struct DesktopApp {
    /// Committed local appearance and an optional unapplied preview.
    appearance: appearance::AppearanceUi,
    /// Runtime-owned lifecycle snapshot.
    status: watch::Receiver<SessionStatus>,
    /// Coalesced activation notices, including notices before window construction.
    activation: watch::Receiver<()>,
    /// Bounded lifecycle-only requests.
    commands: mpsc::Sender<SessionCommand>,
    /// Domain operations sent through the application runtime.
    settings_requests: mpsc::Sender<SettingsRequest>,
    /// Committed settings snapshots received from the runtime.
    settings_view: watch::Receiver<Option<Arc<SettingsSnapshot>>>,
    /// Real settings navigation and pending editor ownership.
    settings: operating::SettingsUi,
    /// Local logbook selector and independently owned management workflow.
    logbooks: logbooks::LogbooksUi,
    /// Local paths displayed to the operator, never diagnostic payloads.
    paths: WorkspacePaths,
    /// Safe startup degradation details.
    diagnostics: Vec<String>,
    /// Standard nonblocking appender loss counter.
    loss: ErrorCounter,
    /// Persistent local file sink failure; output continues through stderr.
    file_failure: Arc<AtomicBool>,
    /// Retry sending shutdown if a bounded channel was temporarily full.
    shutdown_pending: bool,
    /// Prevent repeated close requests from restarting shutdown.
    close_requested: bool,
    /// Emit a delayed-shutdown warning once.
    delayed_reported: bool,
    /// Hide the choice after explicitly selecting continued waiting.
    continuing: bool,
    /// Present a failed lifecycle send rather than silently discarding it.
    command_error: Option<String>,
}

impl DesktopApp {
    /// Attach the actual session and diagnostic state to one native window.
    pub fn new(resources: UiResources) -> Self {
        let UiResources {
            status,
            activation,
            commands,
            settings_requests,
            settings_view,
            paths,
            diagnostics,
            loss,
            file_failure,
        } = resources;
        Self {
            appearance: Default::default(),
            status,
            activation,
            commands,
            settings_requests,
            settings_view,
            paths,
            diagnostics,
            loss,
            file_failure,
            settings: Default::default(),
            logbooks: Default::default(),
            shutdown_pending: false,
            close_requested: false,
            delayed_reported: false,
            continuing: false,
            command_error: None,
        }
    }

    /// Restore presentation preferences before the first native frame.
    pub(crate) fn restore_appearance(
        &mut self,
        context: &egui::Context,
        storage: Option<&dyn eframe::Storage>,
    ) {
        self.appearance = appearance::AppearanceUi::restore(storage);
        theme::install(context, &self.appearance.committed);
    }

    /// Request orderly cleanup once; repeated native closes retain the original deadline.
    fn begin_close(&mut self) {
        if !self.close_requested {
            self.shutdown_pending = true;
            self.close_requested = true;
        }
    }

    /// Apply background notifications and native close handling, including while hidden.
    fn update_lifecycle(&mut self, context: &egui::Context) {
        if self.activation.has_changed().unwrap_or(false) {
            self.activation.borrow_and_update();
            context.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            context.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            context.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        let state = self.status.borrow_and_update().clone();
        if matches!(state, SessionStatus::Closed) {
            context.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        if context.input(|input| input.viewport().close_requested()) {
            context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.begin_close();
            self.continuing = false;
        }

        if self.shutdown_pending {
            match self.commands.try_send(SessionCommand::BeginShutdown) {
                Ok(()) => self.shutdown_pending = false,
                Err(mpsc::error::TrySendError::Full(_)) => {}
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    self.command_error = Some(
                        "The background session stopped unexpectedly. Close the window to exit."
                            .into(),
                    );
                    context.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }

        if let SessionStatus::Closing { started, .. } = state {
            if closing::delayed(started) && !self.delayed_reported {
                tracing::warn!(event = "desktop.shutdown_delayed");
                self.delayed_reported = true;
            }
            context.request_repaint_after(Duration::from_millis(250));
        } else if self.shutdown_pending {
            context.request_repaint_after(Duration::from_millis(50));
        }
    }
}

impl eframe::App for DesktopApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.appearance.save(storage);
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_lifecycle(context);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let state = self.status.borrow().clone();
        let closing = self.close_requested
            || matches!(
                &state,
                SessionStatus::Closing { .. } | SessionStatus::Closed
            );
        if closing && self.appearance.editor.is_some() {
            self.appearance
                .finish(ui.ctx(), appearance::AppearanceAction::Cancel);
        }
        self.appearance.update_fonts(ui.ctx(), &self.commands);
        egui::Panel::top("application.header").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Clacks Log");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let appearance = ui.add_enabled(!closing, egui::Button::new("Appearance…"));
                    if appearance.clicked() {
                        self.appearance.open(appearance.id);
                    }
                });
            });
        });
        egui::Panel::bottom("application.status").show(ui, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(!closing, egui::Button::new("Close"))
                    .clicked()
                {
                    self.begin_close();
                }
                ui.separator();
                startup::status(ui, &state);
            });
        });
        let background = ui.visuals().extreme_bg_color;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::central_panel(ui.style())
                    .fill(background)
                    .inner_margin(12),
            )
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("application.content")
                    .show(ui, |ui| {
                        if let Some(notice) = &self.appearance.notice {
                            feedback::problem(ui, notice);
                        }
                        startup::diagnostics(self, ui);
                        startup::show(self, ui, &state);

                        if matches!(state, SessionStatus::Ready { .. })
                            && !self.close_requested
                            && let Some(snapshot) = self.settings_view.borrow_and_update().clone()
                        {
                            let restored = match state {
                                SessionStatus::Ready {
                                    restored_view_from, ..
                                } => restored_view_from,
                                _ => None,
                            };
                            self.logbooks.observe(&snapshot.logbooks, restored);
                            self.logbooks
                                .show(ui, &snapshot.logbooks, &self.settings_requests);
                            ui.add_space(8.0);
                            self.settings.show(ui, &snapshot, &self.settings_requests);
                        }

                        if let SessionStatus::Closing { phase, started } = state {
                            theme::section_frame(ui).show(ui, |ui| {
                                ui.heading("Closing workspace");
                                ui.separator();
                                closing::show(self, ui, phase, started);
                            });
                        }
                        startup::details(self, ui, &state);
                    });
            });
        if !closing {
            self.appearance.show(ui.ctx());
        }
    }
}

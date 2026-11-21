//! Native presentation and lifecycle requests, with no database work in callbacks.

mod actions;
mod appearance;
mod closing;
mod contact_table;
mod contacts;
mod diagnostics;
mod dialog;
mod enrollment;
mod equipment;
mod export;
mod feedback;
mod fields;
pub(crate) mod fonts;
mod geography;
mod import;
mod logbooks;
mod logging;
mod menu_commands;
#[cfg(any(test, not(any(target_os = "macos", windows))))]
mod menus;
mod operating;
mod preferences;
mod profiles;
mod review;
mod settings;
mod startup;
mod tag_picker;
#[cfg(test)]
mod tests;

mod theme;
mod validation;
mod workbench;
mod worked_before;
mod ws_suite;

use crate::{
    session::{
        SessionCommand, SessionStatus, SettingsBasis, SettingsSessionId, settings::SettingsRequest,
    },
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
    /// Selected geographic data readiness, independent of workspace initialization.
    pub geography_view: watch::Receiver<crate::geography::GeographyStatus>,
    /// Ordered WS entry updates belonging to this workspace.
    pub entry_calls: mpsc::UnboundedReceiver<crate::ws_suite::EntryCall>,
    /// Current workspace's shared directory client.
    pub lookup_view: watch::Receiver<Option<crate::directory::lookup::LookupClient>>,
    /// Local commits invalidate history even without synchronization.
    pub contact_changes: watch::Receiver<()>,
    /// Separate protected-directory installation diagnostics.
    pub directory_status: watch::Receiver<crate::directory::provisioning::ProvisioningStatus>,
    /// Nonsecret enrollment context and progress from the owning session.
    pub enrollment_view: watch::Receiver<crate::enrollment::EnrollmentStatus>,
    /// Receiver preference and failure updates.
    pub ws_suite_view: watch::Receiver<crate::ws_suite::ReceiverView>,
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
    /// Current Settings interval, including dismissal while the session is busy.
    pub settings_intent: watch::Sender<Option<SettingsSessionId>>,
    /// Workspace data available after pausing sync.
    pub settings_basis: watch::Receiver<Option<SettingsBasis>>,
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
    /// Shared reader readiness for operation-specific geographic requests.
    geography_view: watch::Receiver<crate::geography::GeographyStatus>,
    /// Native Select all is delivered through egui's next ordinary text input pass.
    pending_text_select_all: Option<egui::Id>,
    /// Entry updates drained even while entry is hidden.
    entry_calls: mpsc::UnboundedReceiver<crate::ws_suite::EntryCall>,
    /// Latest WS Suite callsign update deferred during Import, including an empty call.
    import_entry_call: Option<crate::ws_suite::EntryCall>,
    /// Receiver configuration modal and retained outcomes.
    ws_suite: ws_suite::ReceiverUi,
    /// Runtime-owned receiver view.
    ws_suite_view: watch::Receiver<crate::ws_suite::ReceiverView>,
    /// Pending results drained independently from form navigation.
    lookup_view: watch::Receiver<Option<crate::directory::lookup::LookupClient>>,
    /// Local commit notifications, independent of settings changes.
    contact_changes: watch::Receiver<()>,
    /// Current directory-only diagnosis; logging remains independently available.
    directory_status: watch::Receiver<crate::directory::provisioning::ProvisioningStatus>,
    /// Native menu resources, never constructed by headless component tests.
    #[cfg(any(target_os = "macos", windows))]
    native_menus: Option<crate::menus::native::MenuAdapter>,
    /// Native activation events consumed by root logic.
    #[cfg(any(target_os = "macos", windows))]
    menu_receiver: Option<std::sync::mpsc::Receiver<crate::menus::MenuCommand>>,
    /// Workspace information opened from Help.
    details: diagnostics::DiagnosticsUi,
    /// Connection dialog, independent of logging and saved-settings editors.
    enrollment: enrollment::EnrollmentUi,
    /// Latest authoritative enrollment progress from the runtime.
    enrollment_view: watch::Receiver<crate::enrollment::EnrollmentStatus>,
    /// Committed local appearance and an optional unapplied preview.
    appearance: appearance::AppearanceUi,
    /// One production logging region and its transient entry.
    workbench: workbench::Workbench,
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
    /// Current Settings navigation intent consumed by the session.
    settings_intent: watch::Sender<Option<SettingsSessionId>>,
    /// The session's editable basis for the current Settings interval.
    settings_basis: watch::Receiver<Option<SettingsBasis>>,
    /// Monotonic identity of the latest opening in this window.
    next_settings_id: u64,
    /// Shared Settings editor when the workbench is hidden.
    settings_screen: Option<settings::SettingsScreen>,
    /// Reread the saved view after losing a Settings Apply acknowledgment.
    recover_settings_view: bool,
    /// Real settings navigation and pending editor ownership.
    settings: operating::SettingsUi,
    /// Local logbook selector and independently owned management workflow.
    logbooks: logbooks::LogbooksUi,
    /// Account-wide review in its independent native viewport.
    review: review::ReviewUi,
    import: import::ImportUi,
    export: export::ExportUi,
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
    /// Original close deadline also covers final preference acknowledgment.
    close_started: Option<std::time::Instant>,
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
            geography_view,
            entry_calls,
            ws_suite_view,
            lookup_view,
            contact_changes,
            directory_status,
            enrollment_view,
            status,
            activation,
            commands,
            settings_requests,
            settings_view,
            settings_intent,
            settings_basis,
            paths,
            diagnostics,
            loss,
            file_failure,
        } = resources;
        Self {
            geography_view,
            entry_calls,
            import_entry_call: None,
            ws_suite: Default::default(),
            ws_suite_view,
            lookup_view,
            contact_changes,
            directory_status,
            #[cfg(any(target_os = "macos", windows))]
            native_menus: None,
            #[cfg(any(target_os = "macos", windows))]
            menu_receiver: None,
            pending_text_select_all: None,
            details: Default::default(),
            enrollment: Default::default(),
            enrollment_view,
            appearance: Default::default(),
            workbench: Default::default(),
            status,
            activation,
            commands,
            settings_requests,
            settings_view,
            settings_intent,
            settings_basis,
            paths,
            diagnostics,
            loss,
            file_failure,
            settings_screen: None,
            recover_settings_view: false,
            next_settings_id: 0,
            settings: Default::default(),
            logbooks: Default::default(),
            review: Default::default(),
            import: Default::default(),
            export: Default::default(),
            shutdown_pending: false,
            close_requested: false,
            close_started: None,
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
        self.workbench.restore(storage);
        theme::install(context, &self.appearance.committed);
    }

    /// Request orderly cleanup once; repeated native closes retain the original deadline.
    fn begin_close(&mut self) {
        if !self.close_requested
            && !(self.import.is_picking_file() || self.export.is_picking_file())
        {
            self.shutdown_pending = true;
            self.close_requested = true;
            self.close_started = Some(std::time::Instant::now());
        }
    }

    /// Apply background notifications and native close handling, including while hidden.
    fn update_lifecycle(&mut self, context: &egui::Context) {
        self.import.poll_picker(context);
        let geography = self.geography_view.borrow_and_update().clone();
        self.workbench
            .advance_geography(&self.commands, &geography, self.close_requested);
        self.settings
            .advance_geography(&self.commands, &geography, self.close_requested);
        if self.contact_changes.has_changed().unwrap_or(false) {
            self.contact_changes.borrow_and_update();
            self.workbench.entry.changed = true;
        }
        {
            let workspace = self.lookup_view.borrow();
            let is_current = |call: &crate::ws_suite::EntryCall| {
                workspace
                    .as_ref()
                    .is_some_and(|current| current.same_workspace(&call.workspace))
            };
            let importing = self.import.is_open();
            let reviewing = self.review.is_exclusive();
            let mut latest = self.import_entry_call.take().filter(is_current);
            while let Ok(call) = self.entry_calls.try_recv() {
                if self.settings_screen.is_some() || !is_current(&call) {
                    continue;
                }
                if importing || latest.is_some() {
                    latest = Some(call);
                } else if !reviewing {
                    self.workbench.incoming_call(&call.status.call);
                }
            }
            if importing || reviewing || self.settings_screen.is_some() {
                self.import_entry_call = latest;
            } else if let Some(call) = latest {
                self.workbench.incoming_call(&call.status.call);
            }
        }
        self.ws_suite.poll(&self.ws_suite_view.borrow());
        self.enrollment.poll(context, &mut self.enrollment_view);
        self.settings.poll();
        self.logbooks.poll();
        self.settings.recover_view(&self.commands);
        self.logbooks.recover_view(&self.commands);
        if self.recover_settings_view {
            self.recover_settings_view = matches!(
                self.commands.try_send(SessionCommand::RecoverSettingsView),
                Err(mpsc::error::TrySendError::Full(_)),
            );
            if self.recover_settings_view {
                context.request_repaint_after(Duration::from_millis(50));
            }
        }
        self.workbench
            .advance_lookup(context, self.lookup_view.borrow().as_ref());
        if self.activation.has_changed().unwrap_or(false) {
            self.activation.borrow_and_update();
            context.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            context.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            context.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        let state = self.status.borrow_and_update().clone();
        let account = match state {
            SessionStatus::Ready { identity, .. } => Some(identity.account),
            _ => None,
        };
        self.export.poll(context, &self.commands, account);
        if matches!(state, SessionStatus::Closed) {
            if !(self.import.is_picking_file() || self.export.is_picking_file()) {
                context.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            return;
        }

        if context.input(|input| input.viewport().close_requested()) {
            context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.begin_close();
            self.continuing = false;
        }

        if self.shutdown_pending && self.workbench.flush_query(context, &self.settings_requests) {
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
    fn raw_input_hook(&mut self, context: &egui::Context, input: &mut egui::RawInput) {
        if input.viewport_id == egui::ViewportId::ROOT
            && self.review.is_exclusive()
            && !context.embed_viewports()
        {
            input.events.clear();
            self.pending_text_select_all = None;
            return;
        }
        if let Some(target) = self.pending_text_select_all.take()
            && context.memory(|memory| memory.focused() == Some(target))
        {
            let modifiers = egui::Modifiers {
                command: true,
                mac_cmd: cfg!(target_os = "macos"),
                ctrl: !cfg!(target_os = "macos"),
                ..Default::default()
            };
            let selection = [true, false].map(|pressed| egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            });
            input.events.splice(0..0, selection);
        }
    }

    fn on_exit(&mut self) {
        #[cfg(any(target_os = "macos", windows))]
        self.native_menus.take();
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.appearance.save(storage);
        self.workbench.save(storage);
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_lifecycle(context);
        #[cfg(any(target_os = "macos", windows))]
        self.process_menus(context);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
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
        let settings_changed = self.settings_view.has_changed().unwrap_or(false);
        let snapshot = if !closing && matches!(state, SessionStatus::Ready { .. }) {
            self.settings_view.borrow_and_update().clone()
        } else {
            None
        };
        if let Some(snapshot) = &snapshot {
            if settings_changed {
                self.review.invalidate();
                self.workbench
                    .invalidate_contacts(snapshot.logbooks.selected_view);
            }
            let restored = match state {
                SessionStatus::Ready {
                    restored_view_from, ..
                } => restored_view_from,
                _ => None,
            };
            self.logbooks.observe(&snapshot.logbooks, restored);
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        self.show_menus(ui);
        if !closing && self.settings_screen.is_some() {
            self.show_settings(ui, &state);
            #[cfg(any(target_os = "macos", windows))]
            self.update_menu_state(ui.ctx());
            return;
        }
        if !closing
            && (self.review.is_exclusive()
                || self.import.is_open()
                || self.export.is_picking_file())
        {
            ui.disable();
        }
        egui::Panel::top("application.header").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("Clacks Log");
                if let Some(snapshot) = &snapshot {
                    self.logbooks
                        .show(ui, &snapshot.logbooks, &self.settings_requests);
                    let review_available = !self.modal_open();
                    self.review.action(ui, review_available);
                }
            });
        });
        if !closing && self.review.is_exclusive() {
            ui.disable();
        }
        if let Some(snapshot) = &snapshot {
            egui::Panel::top("operating.strip")
                .frame(
                    egui::Frame::side_top_panel(ui.style())
                        .fill(theme::palette(ui.ctx().theme()).header),
                )
                .show(ui, |ui| {
                    self.settings.show(ui, snapshot, &self.settings_requests);
                });
        }
        egui::Panel::bottom("application.status").show(ui, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                startup::status(ui, &state);
            });
        });
        let background = theme::palette(ui.ctx().theme()).workspace;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::central_panel(ui.style())
                    .fill(background)
                    .inner_margin(0),
            )
            .show(ui, |ui| {
                if let Some(notice) = &self.appearance.notice {
                    feedback::problem(ui, notice);
                }
                let receiver_available = self
                    .menu_enabled(ui.ctx(), crate::menus::MenuCommand::OpenSettings)
                    && snapshot.is_some();
                if let Some(opener) = self.ws_suite.feedback(ui, receiver_available) {
                    ui.ctx().memory_mut(|memory| memory.request_focus(opener));
                    self.open_settings(ui.ctx(), settings::SettingsDestination::Receiver);
                }
                if let SessionStatus::Ready {
                    settings_read_error: Some(error),
                    ..
                } = &state
                {
                    feedback::problem(ui, error);
                }
                startup::diagnostics(self, ui);
                startup::show(self, ui, &state);
                if let Some(snapshot) = &snapshot {
                    self.logbooks.feedback(ui);
                    let available = ui.available_size();
                    if available.x < 900.0 || available.y < 580.0 {
                        egui::ScrollArea::both()
                            .id_salt("workbench.overflow")
                            .show(ui, |ui| {
                                ui.allocate_ui(available.max(egui::vec2(900.0, 580.0)), |ui| {
                                    self.workbench.show(ui, snapshot, &self.settings_requests)
                                });
                            });
                    } else {
                        self.workbench.show(ui, snapshot, &self.settings_requests);
                    }
                }
                if let SessionStatus::Closing { phase, started } = state {
                    theme::section_frame(ui).show(ui, |ui| {
                        ui.heading("Closing workspace");
                        closing::show(self, ui, phase, started);
                    });
                } else if self.shutdown_pending
                    && let Some(started) = self.close_started
                {
                    theme::section_frame(ui).show(ui, |ui| {
                        ui.heading("Closing workspace");
                        if let Some(error) = self.workbench.query_save_error() {
                            feedback::problem(ui, error);
                            if ui.button("Retry saving history query").clicked() {
                                self.workbench.retry_query_save();
                            }
                        }
                        closing::show(self, ui, "Saving history query…", started);
                    });
                }
            });
        if !closing {
            self.review.show_window(ui.ctx(), &self.settings_requests);
            if let Some(record) = self.review.take_repair() {
                ui.ctx()
                    .send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Focus);
                match record.key().id {
                    clacks_sync::record::RecordId::Qso(id) => {
                        self.workbench.open_contact(id, &self.settings_requests)
                    }
                    clacks_sync::record::RecordId::Profile(id) => {
                        self.open_settings(ui.ctx(), settings::SettingsDestination::Profile(id))
                    }
                    _ => {}
                }
            }

            self.details.show(ui.ctx(), &self.paths, &state);
            self.import
                .show(ui.ctx(), &self.commands, snapshot.as_deref(), frame);
            self.export.show(ui.ctx(), &self.commands, frame);
        }
        #[cfg(any(target_os = "macos", windows))]
        self.update_menu_state(ui.ctx());
    }
}

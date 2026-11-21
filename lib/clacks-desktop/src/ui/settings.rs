//! One desktop Settings session, with shared Apply and dismissal.

pub(super) mod draft;
#[cfg(test)]
mod tests;

use super::{DesktopApp, actions, appearance::AppearanceAction, feedback};
use crate::session::{
    SessionStatus, SettingsSessionId,
    settings::{SettingsAction, SettingsRequest, SettingsResponse},
};
use clacks_client_store::settings::SettingsSnapshot;
use clacks_model::ids::ProfileId;
use clacks_sync::record::RecordId;
use eframe::egui;
use std::sync::Arc;
use tokio::sync::oneshot;

/// Workbench and menu destinations for the same Settings editor.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SettingsDestination {
    /// Open the default section.
    Default,
    /// Configure the receiver from its workbench diagnostic.
    Receiver,
    /// Edit this profile from conflict review.
    Profile(ProfileId),
}

/// Sections in their navigation order.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    /// Named logbooks and their upload policies.
    Logbooks,
    /// Reusable station profiles.
    StationProfiles,
    /// Reusable equipment configurations.
    Equipment,
    /// Server connection and browser sign-in.
    Connection,
    /// Local receiver configuration.
    WsSuite,
    /// Client theme, scale, and fonts.
    Appearance,
}

impl Section {
    /// Visible navigation label.
    fn label(self) -> &'static str {
        match self {
            Self::Logbooks => "Logbooks",
            Self::StationProfiles => "Station profiles",
            Self::Equipment => "Equipment",
            Self::Connection => "Connection",
            Self::WsSuite => "WS Suite",
            Self::Appearance => "Appearance",
        }
    }
}

/// One Apply can include account edits and separate receiver/appearance outcomes.
#[derive(Default)]
struct Applying {
    /// Settings worker results for this submission.
    account: Option<oneshot::Receiver<SettingsResponse>>,
    /// A receiver preference response is outstanding.
    receiver: bool,
}

/// State scoped to one opening of the Settings screen.
pub(super) struct SettingsScreen {
    /// Interval used to match the background editing basis.
    id: SettingsSessionId,
    /// Workbench widget to refocus on ordinary dismissal.
    opener: Option<egui::Id>,
    /// Currently visible content.
    section: Section,
    /// Raw edits and confirmed destructive operations for shared records.
    draft: draft::AccountDraft,
    /// Submitted changes awaiting settlement.
    applying: Option<Applying>,
    /// Last Apply diagnostics at receiver settings.
    receiver_errors: Vec<crate::operating::FieldError>,
    /// Last Apply rejected the appearance preview.
    appearance_error: bool,
    /// A busy runtime could not accept sign-in cancellation.
    close_error: bool,
}

impl SettingsScreen {
    /// Whether this section has an unresolved Apply diagnostic.
    fn failed(&self, section: Section) -> bool {
        match section {
            Section::Logbooks => self
                .draft
                .section_failed(|id| matches!(id, RecordId::Tag(_))),
            Section::StationProfiles => self
                .draft
                .section_failed(|id| matches!(id, RecordId::Profile(_))),
            Section::Equipment => self
                .draft
                .section_failed(|id| matches!(id, RecordId::Equipment(_))),
            Section::Connection => false,
            Section::WsSuite => !self.receiver_errors.is_empty(),
            Section::Appearance => self.appearance_error,
        }
    }

    /// Select content without replacing edits already in the draft.
    fn navigate(&mut self, destination: SettingsDestination) {
        match destination {
            SettingsDestination::Default => {}
            SettingsDestination::Receiver => self.section = Section::WsSuite,
            SettingsDestination::Profile(id) => {
                self.section = Section::StationProfiles;
                self.draft.open_profile(id);
            }
        }
    }
}

/// Navigation order shared by status and section buttons.
const SECTIONS: [Section; 6] = [
    Section::Logbooks,
    Section::StationProfiles,
    Section::Equipment,
    Section::Connection,
    Section::WsSuite,
    Section::Appearance,
];

impl DesktopApp {
    /// Enter Settings without replacing a draft already open in this window.
    pub(super) fn open_settings(
        &mut self,
        context: &egui::Context,
        destination: SettingsDestination,
    ) {
        if let Some(screen) = &mut self.settings_screen {
            if screen.applying.is_none() && !screen.draft.reviewing() {
                screen.navigate(destination);
            }
            return;
        }

        self.next_settings_id = self.next_settings_id.wrapping_add(1);
        let id = SettingsSessionId(self.next_settings_id);
        let mut screen = SettingsScreen {
            id,
            opener: context.memory(|memory| memory.focused()),
            section: if matches!(*self.status.borrow(), SessionStatus::Ready { .. }) {
                Section::Logbooks
            } else {
                Section::Appearance
            },
            draft: Default::default(),
            applying: None,
            receiver_errors: Vec::new(),
            appearance_error: false,
            close_error: false,
        };
        screen.navigate(destination);
        self.settings_intent.send_replace(Some(id));
        self.appearance.open();
        self.enrollment.open();
        self.settings_screen = Some(screen);
        context.request_repaint();
    }

    /// Poll outcomes and render the current section of the shared editor.
    pub(super) fn show_settings(&mut self, ui: &mut egui::Ui, state: &SessionStatus) {
        let Some(mut screen) = self.settings_screen.take() else {
            return;
        };
        let basis = self.settings_basis.borrow_and_update().clone().filter(|basis| {
            basis.id == screen.id
                && matches!(state, SessionStatus::Ready { identity, .. } if *identity == basis.identity)
        });
        let basis = basis.map(|basis| basis.snapshot);
        let enrollment = self.enrollment_view.borrow().clone();
        let connection_busy = self.enrollment.busy(&enrollment);

        if basis.is_some() {
            self.ws_suite
                .open(&self.ws_suite_view.borrow(), &self.commands);
        }
        self.poll_settings_apply(&mut screen);
        if !self.ws_suite.reception_failed() {
            screen
                .receiver_errors
                .retain(|error| error.field != "Reception");
        }
        let snapshot = basis;
        let pending = screen.applying.is_some();
        let reviewing = screen.draft.reviewing();
        let popup_open = egui::Popup::is_any_open(ui.ctx());
        let workspace_editable = snapshot.is_some() && !connection_busy;
        let workspace_dirty = screen.draft.dirty() || self.ws_suite.changed();
        let dirty = workspace_dirty || self.appearance.changed();

        let mut submit = false;
        let mut dismiss = false;
        egui::Panel::bottom("settings.actions").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let failed = SECTIONS.into_iter().any(|section| screen.failed(section));
                let can_apply = dirty
                    && !pending
                    && !reviewing
                    && !connection_busy
                    && (!workspace_dirty || workspace_editable)
                    && (!self.ws_suite.changed() || !self.ws_suite.busy());
                let text = if failed { "Apply ⚠" } else { "Apply" };
                let action = actions::primary(ui, text, can_apply);
                if failed {
                    action.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::Role::Button,
                            can_apply,
                            "Apply, some changes could not be applied",
                        )
                    });
                }
                submit = action.clicked();
                let label = if dirty { "Cancel" } else { "Close" };
                dismiss = ui
                    .add_enabled(!pending && !reviewing, egui::Button::new(label))
                    .clicked();
                if pending {
                    ui.spinner();
                    ui.label("Applying changes…");
                }
            });
            if screen.close_error {
                feedback::problem(ui, "Could not cancel sign-in. Try closing Settings again.");
            }
        });

        let navigation_width = 160.0_f32.min(ui.available_width() / 3.0);
        egui::Panel::left("settings.navigation")
            .exact_size(navigation_width)
            .resizable(false)
            .show(ui, |ui| {
                ui.heading("Settings");
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for section in SECTIONS {
                        let label = section.label();
                        let failed = screen.failed(section);
                        let text = if failed {
                            format!("{label} ⚠")
                        } else {
                            label.to_owned()
                        };
                        let tab = ui.add_sized(
                            [ui.available_width(), 0.0],
                            egui::Button::selectable(screen.section == section, text).wrap(),
                        );
                        if failed {
                            tab.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::Role::Button,
                                    true,
                                    format!("{label}, changes could not be applied"),
                                )
                            });
                        }
                        if tab.clicked() && !reviewing {
                            screen.section = section;
                        }
                    }
                });
            });

        egui::CentralPanel::default().show(ui, |ui| {
            if screen.section != Section::Appearance
                && let SessionStatus::Ready {
                    settings_read_error: Some(error),
                    ..
                } = state
            {
                feedback::problem(ui, error);
            }
            egui::ScrollArea::vertical()
                .id_salt(screen.section.label())
                .show(ui, |ui| {
                    ui.add_enabled_ui(!pending, |ui| match screen.section {
                        Section::Appearance => {
                            self.appearance.show_section(ui);
                            if screen.appearance_error {
                                feedback::problem(
                                    ui,
                                    "Appearance changes were invalid and could not be applied.",
                                );
                            }
                        }
                        Section::Connection if snapshot.is_some() || connection_busy => {
                            if let SessionStatus::Ready { identity, .. } = state
                                && identity.enrolled
                            {
                                ui.label(format!("Account {}", identity.account));
                            }
                            self.enrollment
                                .show_section(ui, &self.commands, &enrollment);
                        }
                        section => {
                            let Some(snapshot) = &snapshot else {
                                match state {
                                    SessionStatus::Failed { .. } => {
                                        ui.label("Workspace settings are unavailable.")
                                    }
                                    SessionStatus::Ready {
                                        settings_read_error: Some(_),
                                        ..
                                    } => return,
                                    _ => {
                                        ui.spinner();
                                        ui.label("Loading workspace settings…")
                                    }
                                };
                                return;
                            };
                            ui.add_enabled_ui(workspace_editable, |ui| match section {
                                Section::Logbooks => {
                                    screen.draft.logbooks(ui, snapshot, &self.settings_requests)
                                }
                                Section::StationProfiles => {
                                    let definitions = self.settings.observe_for_settings(snapshot);
                                    screen.draft.station_profiles(
                                        ui,
                                        snapshot,
                                        definitions,
                                        &self.commands,
                                        &self.geography_view.borrow(),
                                    );
                                }
                                Section::Equipment => {
                                    let definitions = self.settings.observe_for_settings(snapshot);
                                    screen.draft.equipment_configurations(
                                        ui,
                                        snapshot,
                                        definitions,
                                    );
                                }
                                Section::WsSuite => {
                                    self.ws_suite.show_section(
                                        ui,
                                        &self.commands,
                                        &screen.receiver_errors,
                                    );
                                }
                                _ => {}
                            });
                        }
                    });
                });
        });

        if !reviewing && !screen.draft.reviewing() && !pending && !popup_open {
            dismiss |=
                ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        }
        if dismiss {
            if self.enrollment.close(&self.commands, &enrollment) {
                self.appearance.finish(ui.ctx(), AppearanceAction::Cancel);
                self.ws_suite.close();
                self.settings_intent.send_replace(None);
                if let Some(id) = screen.opener {
                    ui.ctx().memory_mut(|memory| memory.request_focus(id));
                }
                ui.ctx().request_repaint();
                return;
            }
            screen.close_error = true;
        }
        if submit {
            self.start_settings_apply(ui.ctx(), &mut screen, snapshot.as_ref());
        }
        self.settings_screen = Some(screen);
    }

    /// Validate and submit each independent operation in the shared draft.
    fn start_settings_apply(
        &mut self,
        context: &egui::Context,
        screen: &mut SettingsScreen,
        snapshot: Option<&Arc<SettingsSnapshot>>,
    ) {
        screen.receiver_errors.clear();
        screen.appearance_error = false;
        let mut applying = Applying::default();
        if let Some(snapshot) = snapshot {
            let definitions = self.settings.observe_for_settings(snapshot);
            let changes = screen.draft.submit(snapshot, definitions);
            if !changes.is_empty() {
                let (reply, result) = oneshot::channel();
                let request = SettingsRequest {
                    action: SettingsAction::ApplyAccountChanges(changes),
                    reply,
                };
                if self.settings_requests.try_send(request).is_ok() {
                    applying.account = Some(result);
                } else {
                    screen
                        .draft
                        .fail_submitted("Could not submit the workspace settings changes.");
                }
            }
        }
        if self.ws_suite.changed() {
            match self.ws_suite.apply(&self.commands) {
                Ok(()) => applying.receiver = true,
                Err(errors) => screen.receiver_errors = errors,
            }
        }
        if self.appearance.changed() {
            if self
                .appearance
                .editor
                .as_ref()
                .is_some_and(|editor| editor.pending.validate().is_ok())
            {
                self.appearance.finish(context, AppearanceAction::Apply);
            } else {
                screen.appearance_error = true;
                self.appearance.finish(context, AppearanceAction::Cancel);
            }
            self.appearance.open();
        }
        screen.applying = Some(applying);
        context.request_repaint();
    }

    /// Settle the submitted draft without disturbing the current section.
    fn poll_settings_apply(&mut self, screen: &mut SettingsScreen) {
        let Some(applying) = &mut screen.applying else {
            return;
        };
        if let Some(result) = &mut applying.account {
            let response = match result.try_recv() {
                Ok(response) => Some(response),
                Err(oneshot::error::TryRecvError::Empty) => None,
                Err(oneshot::error::TryRecvError::Closed) => {
                    self.recover_settings_view = true;
                    Some(SettingsResponse::Failed(
                        "Could not verify which workspace settings saved.".into(),
                    ))
                }
            };
            if let Some(response) = response {
                applying.account = None;
                let response = match response {
                    SettingsResponse::RefreshFailed { outcome, .. } => *outcome,
                    response => response,
                };
                match response {
                    SettingsResponse::ApplyAccountChanges(result) => {
                        screen.draft.settled(result);
                    }
                    SettingsResponse::Failed(error) => screen.draft.fail_submitted(&error),
                    _ => screen
                        .draft
                        .fail_submitted("Could not confirm the workspace settings changes."),
                }
            }
        }
        if applying.receiver
            && let Some(result) = self.ws_suite.take_applied()
        {
            applying.receiver = false;
            if let Err(error) = result {
                screen.receiver_errors.push(error);
            }
        }
        if applying.account.is_none() && !applying.receiver {
            screen.applying = None;
            screen.draft.clear_forms();
            self.ws_suite.reset_form();
        }
    }
}

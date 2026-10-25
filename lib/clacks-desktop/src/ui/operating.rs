//! Navigation owns one operating edit and independent reusable-record editors.

#[cfg(test)]
mod dialog_tests;
pub(super) mod fields;
mod pending;
#[cfg(test)]
mod refresh_tests;
mod responses;
#[cfg(test)]
mod stale_tests;
#[cfg(test)]
mod tests;

use super::{equipment, profiles};
use crate::{
    operating::{
        FieldError,
        editor::{EquipmentEditor, OperatingEditor, ProfileEditor},
    },
    session::settings::{SettingsAction, SettingsRequest, SettingsResponse},
};
use clacks_client_store::{
    save::SaveOutcome,
    settings::{EquipmentDeletionPreview, SettingsSnapshot},
};
use clacks_model::{
    group::DefinitionSet,
    ids::{EquipmentConfigurationId, ProfileId},
    profile::{EquipmentConfiguration, SavedLocationProfile},
};
use clacks_sync::{
    client::ClientRecord,
    record::{RecordId, RecordState, RecordValue},
};
use eframe::egui;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use tokio::sync::{mpsc, oneshot};

/// A separately navigable editor; the stack preserves each caller without applying it.
pub(super) enum Screen {
    /// Applied summary and workflow entry points.
    Home,
    /// Transient operation-wide form.
    Operating(Box<OperatingEditor>),
    /// Reusable station-profile list.
    Profiles,
    /// Independent saved station-profile form.
    Profile(Box<ProfileEditor>),
    /// Reusable equipment list.
    Equipment,
    /// Independent saved equipment form.
    EquipmentEdit(Box<EquipmentEditor>),
    /// Atomic equipment deletion and complete reference repair.
    DeleteEquipment(Box<DeletionReview>),
}

/// A view transition, applied only after rendering the current form.
pub(super) struct Navigation {
    /// New child editor or list.
    pub(super) next: Option<Screen>,
    /// Discard or finish the current editor and reveal its preserved caller.
    pub(super) back: bool,
}

/// One explicit decision per referring profile; absent entries remain unanswered.
pub(super) struct DeletionReview {
    /// Complete read basis returned by the store.
    pub(super) preview: EquipmentDeletionPreview,
    /// Explicit removal or replacement, keyed by profile identity.
    pub(super) choices: BTreeMap<ProfileId, Option<EquipmentConfigurationId>>,
}

/// An originating screen's form, validation and independently owned response.
pub(super) struct Frame {
    /// Native widget to focus when this destination is dismissed.
    opener: Option<egui::Id>,
    /// Purpose-specific form or list.
    pub(super) screen: Screen,
    /// Selected view of this single pending operating form.
    tab: OperatingTab,
    /// In-flight result; no globally routed response can overwrite another editor.
    response: Option<oneshot::Receiver<SettingsResponse>>,
    /// Actionable native field errors for the latest validated input.
    pub(super) errors: Vec<FieldError>,
    /// Deadline/freshness of the existing errors for this frame's current form.
    pub(super) validation: crate::ui::validation::ValidationState,
    /// Outcome, stale-save instructions or request failure.
    pub(super) notice: Option<String>,
    /// Current competing record for explicit reviewed retry.
    pub(super) stale: Option<Option<ClientRecord>>,
    /// Competing active state requiring explicit review before Apply again.
    stale_operating: Option<clacks_client_store::operating::OperatingState>,
}
impl Frame {
    /// Open a new navigation destination without inheriting another form's request.
    fn new(screen: Screen) -> Self {
        Self {
            opener: None,
            screen,
            tab: Default::default(),
            response: None,
            errors: vec![],
            validation: Default::default(),
            notice: None,
            stale: None,
            stale_operating: None,
        }
    }

    /// Refresh native validation when due, without submitting or changing operating values.
    fn validate(&mut self, definitions: &DefinitionSet, context: &egui::Context) {
        let now = context.input(|input| input.time);
        if self.validation.due(now) {
            self.errors = match &self.screen {
                Screen::Operating(editor) => editor.build(definitions).err().unwrap_or_default(),
                Screen::Profile(editor) => editor.build(definitions).err().unwrap_or_default(),
                Screen::EquipmentEdit(editor) => editor.build().err().unwrap_or_default(),
                _ => Vec::new(),
            };
            self.validation.validated();
        }
        self.validation.request_repaint(context, now);
    }

    /// Submit once, retaining this editor and its text on admission failure.
    pub(super) fn send(&mut self, sender: &mpsc::Sender<SettingsRequest>, action: SettingsAction) {
        let (reply, response) = oneshot::channel();
        match sender.try_send(SettingsRequest { action, reply }) {
            Ok(()) => {
                self.response = Some(response);
                self.errors.clear();
                self.notice = None;
            }
            Err(error) => self.notice = Some(format!("Could not submit settings request: {error}")),
        }
    }
}

/// Window-owned settings presentation; no database reads occur in rendering callbacks.
#[derive(Default)]
pub(super) struct SettingsUi {
    /// Hidden originating forms retained only until their admitted response settles.
    dismissed: Vec<Frame>,
    /// Result of a dismissed operation that still needs the operator's attention.
    completion_notice: Option<String>,
    /// Last accepted Apply basis until a subsequent atomic view arrives.
    acknowledged: Option<clacks_client_store::operating::OperatingState>,
    /// Native opener awaiting focus after modal removal.
    restore_focus: Option<egui::Id>,
    /// Explicit navigation stack, preserving pending callers.
    stack: Vec<Frame>,
    /// Last observed atomic snapshot, used only as a refresh identity.
    previous: Option<Arc<SettingsSnapshot>>,
    /// Compiled account definitions, rebuilt only when group records change.
    definitions: DefinitionSet,
    /// Authoritative source deletions retained even if the following view read fails.
    known_deletions: BTreeSet<RecordId>,
    /// A failed post-operation read makes the last view visibly stale.
    refresh_error: Option<String>,
}
/// Views share one pending operation and a single Apply/Cancel pair.
#[derive(Default, PartialEq, Eq)]
enum OperatingTab {
    /// Station facts and explicit profile loading.
    #[default]
    Station,
    /// Equipment facts and independent configuration loading.
    Equipment,
    /// Operator identity and transmit power.
    Operator,
}
impl SettingsUi {
    /// Consume committed view updates, poll this screen's response, and render real workflows.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &Arc<SettingsSnapshot>,
        sender: &mpsc::Sender<SettingsRequest>,
    ) {
        if self.stack.is_empty() {
            self.stack.push(Frame::new(Screen::Home));
        }
        self.refresh(snapshot);
        self.poll();
        let mut effective = snapshot.as_ref().clone();
        if let Some(acknowledged) = &self.acknowledged {
            effective.operating = acknowledged.clone();
        }
        if effective
            .operating
            .selected_profile
            .as_ref()
            .is_some_and(|profile| {
                self.known_deletions
                    .contains(&RecordId::Profile(profile.id))
            })
        {
            effective.operating.selected_profile = None;
        }
        if effective
            .operating
            .selected_equipment
            .as_ref()
            .is_some_and(|equipment| {
                self.known_deletions
                    .contains(&RecordId::Equipment(equipment.id))
            })
        {
            effective.operating.selected_equipment = None;
        }
        let snapshot = &effective;
        if let Some(opener) = self.restore_focus.take() {
            ui.memory_mut(|memory| memory.request_focus(opener));
        }
        if !self.dismissed.is_empty() {
            ui.spinner();
            ui.label("Finishing submitted settings request…");
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }
        if let Some(notice) = &self.completion_notice {
            super::feedback::problem(ui, notice);
            if ui.button("Dismiss notice").clicked() {
                self.completion_notice = None;
            }
        }
        if self.stack.is_empty() {
            self.stack.push(Frame::new(Screen::Home));
        }

        ui.separator();
        ui.heading("Active operating settings");
        if let Some(error) = &self.refresh_error {
            ui.colored_label(egui::Color32::YELLOW, error);
            ui.label(
                "The summary below is the last successfully read state and may be out of date.",
            );
        }
        fields::active_summary(ui, &snapshot.operating);
        if let Some(selected) = &snapshot.operating.selected_profile
            && snapshot
                .profiles
                .iter()
                .filter_map(profile)
                .any(|p| p.id == selected.id && p != selected)
        {
            ui.label(
                "Active profile source changed; applied station values have not been reloaded.",
            );
        }
        if let Some(selected) = &snapshot.operating.selected_equipment
            && snapshot
                .equipment
                .iter()
                .filter_map(configuration)
                .any(|e| e.id == selected.id && e != selected)
        {
            ui.label(
                "Active equipment source changed; applied equipment values have not been reloaded.",
            );
        }
        if ui.button("Refresh settings").clicked()
            && let Some(frame) = self.stack.last_mut()
            && frame.response.is_none()
        {
            frame.send(sender, SettingsAction::Refresh);
        }
        ui.separator();

        let Some(frame) = self.stack.last_mut() else {
            return;
        };
        frame.validate(&self.definitions, ui.ctx());
        let busy = frame.response.is_some();
        if busy {
            ui.spinner();
            ui.label("Finishing settings request…");
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }
        let errors = frame.errors.clone();
        let mut navigation = Navigation {
            next: None,
            back: false,
        };
        let context = ui.ctx().clone();
        let modal = !matches!(frame.screen, Screen::Home);
        let mut body = |ui: &mut egui::Ui| {
            if let Some(notice) = &frame.notice {
                super::feedback::problem(ui, notice);
            }
            ui.add_enabled_ui(!busy, |ui| {
                responses::stale_review(ui, frame);
                match &mut frame.screen {
                    Screen::Home => {
                        if ui.button("Operating settings").clicked() {
                            let mut editor = OperatingEditor::new(&snapshot.operating);
                            for id in &self.known_deletions {
                                editor.forget_source(*id);
                            }
                            navigation.next = Some(Screen::Operating(Box::new(editor)));
                        }
                        if ui.button("Station profiles").clicked() {
                            navigation.next = Some(Screen::Profiles);
                        }
                        if ui.button("Equipment configurations").clicked() {
                            navigation.next = Some(Screen::Equipment);
                        }
                    }
                    Screen::Operating(_) => pending::show(
                        ui,
                        frame,
                        snapshot,
                        &self.definitions,
                        sender,
                        &mut navigation,
                        &self.known_deletions,
                    ),
                    Screen::Profiles | Screen::Profile(_) => profiles::show(
                        ui,
                        frame,
                        snapshot,
                        &self.definitions,
                        sender,
                        &mut navigation,
                        &self.known_deletions,
                    ),
                    Screen::Equipment | Screen::EquipmentEdit(_) | Screen::DeleteEquipment(_) => {
                        equipment::show(
                            ui,
                            frame,
                            snapshot,
                            sender,
                            &mut navigation,
                            &self.known_deletions,
                        )
                    }
                }
            });
            if busy && ui.button("Close dialog").clicked() {
                navigation.back = true;
            }
            fields::unplaced_errors(ui, &errors);
        };
        if modal {
            let result = super::dialog::show(
                &context,
                egui::Id::unique("settings.editor"),
                "Operating settings",
                &mut body,
            );
            let () = result.inner;
            navigation.back |= result.cancel_requested;
        } else {
            body(ui);
        }
        frame
            .validation
            .request_repaint(ui.ctx(), ui.input(|input| input.time));
        if frame.errors.is_empty() && !frame.validation.can_submit(&frame.errors) {
            ui.small("Checking changes…");
        }
        if navigation.back
            && self.stack.len() > 1
            && let Some(frame) = self.stack.pop()
        {
            self.restore_focus = frame.opener;
            if frame.response.is_some() {
                self.dismissed.push(frame);
            }
        }
        if let Some(screen) = navigation.next {
            let mut frame = Frame::new(screen);
            frame.opener = context
                .interaction_snapshot(|interaction| interaction.clicked)
                .or_else(|| context.memory(|memory| memory.focused()));
            self.stack.push(frame);
        }
    }

    /// Refresh known deletions without treating absence or a failed read as a tombstone.
    fn refresh(&mut self, snapshot: &Arc<SettingsSnapshot>) {
        if self
            .previous
            .as_ref()
            .is_some_and(|p| Arc::ptr_eq(p, snapshot))
        {
            return;
        }
        self.acknowledged = None;
        for frame in &mut self.stack {
            frame.validation.invalidate();
        }
        if self
            .previous
            .as_ref()
            .is_none_or(|p| p.groups != snapshot.groups)
        {
            let groups = snapshot
                .groups
                .iter()
                .filter_map(|r| match r.working_state() {
                    Some(RecordState::Live(RecordValue::Group(g))) => Some(g.as_ref().clone()),
                    _ => None,
                })
                .collect();
            match DefinitionSet::new(groups) {
                Ok(set) => self.definitions = set,
                Err(error) => self.refresh_error = Some(error.to_string()),
            }
        }
        for record in snapshot.profiles.iter().chain(&snapshot.equipment) {
            match record.working_state() {
                Some(RecordState::Deleted) => {
                    self.known_deletions.insert(record.key.id);
                }
                Some(RecordState::Live(_)) => {
                    self.known_deletions.remove(&record.key.id);
                }
                None => {}
            }
        }
        for frame in &mut self.stack {
            if let Screen::Operating(editor) = &mut frame.screen {
                editor.refresh_sources(snapshot);
                for id in &self.known_deletions {
                    editor.forget_source(*id);
                }
            }
        }
        self.previous = Some(snapshot.clone());
    }
}

/// Return a live profile only, preserving tombstones/unavailable rows elsewhere in the snapshot.
pub(super) fn profile(record: &ClientRecord) -> Option<&SavedLocationProfile> {
    match record.working_state() {
        Some(RecordState::Live(RecordValue::Profile(p))) => Some(p),
        _ => None,
    }
}
/// Return a live configuration without inventing one for a missing record.
pub(super) fn configuration(record: &ClientRecord) -> Option<&EquipmentConfiguration> {
    match record.working_state() {
        Some(RecordState::Live(RecordValue::Equipment(e))) => Some(e),
        _ => None,
    }
}
/// Distinguish duplicate profile names with the saved native identity.
pub(super) fn profile_label(p: &SavedLocationProfile) -> String {
    format!("{} · {} · {}", p.name, p.station.callsign.as_str(), p.id)
}
/// Distinguish duplicate equipment names with the saved native identity.
pub(super) fn equipment_label(e: &EquipmentConfiguration) -> String {
    format!("{} · {}", e.name, e.id)
}

//! Local view selection and independent saved-logbook management.

mod deletion;
pub(super) mod editor;
pub(super) mod labels;
mod responses;

use super::{dialog, feedback, theme, validation::ValidationState};
use crate::{
    logbooks::TagEditor,
    operating::FieldError,
    session::settings::{
        SettingsAction, SettingsRequest, SettingsResponse,
        logbooks::{LogbookAction, LogbookResponse},
    },
};
use clacks_client_store::{
    logbooks::{LogbookSnapshot, selection::SelectedView},
    save::SaveOutcome,
};
use clacks_model::{ids::TagId, tag::LogbookTag};
use clacks_sync::{
    client::ClientRecord,
    record::{RecordId, RecordState, RecordValue},
};
use eframe::egui;
use std::collections::{BTreeMap, BTreeSet};
use tokio::sync::{mpsc, oneshot};

/// Saved-record editing state, distinct from the workspace's selected view.
struct TagForm {
    /// Unapplied field values.
    editor: TagEditor,
    /// Complete original save basis.
    expected: Option<ClientRecord>,
    /// Current field feedback.
    errors: Vec<FieldError>,
    /// Validation timing and Save availability for the edited fields.
    validation: ValidationState,
    /// Whether the saved tag changed after this editor was opened.
    stale: bool,
}

impl TagForm {
    /// Start a new or existing tag form without selecting it.
    fn new(expected: Option<ClientRecord>) -> Self {
        let editor = expected
            .as_ref()
            .and_then(tag)
            .map(TagEditor::from_tag)
            .unwrap_or_else(|| TagEditor::new(TagId::new()));
        Self {
            editor,
            expected,
            errors: vec![],
            validation: Default::default(),
            stale: false,
        }
    }
}

/// Management-dialog state retained until its pending request finishes.
struct Management {
    /// New/edit form, absent for the saved-tag list.
    form: Option<TagForm>,
    /// Deletion preview and the chosen QSO treatment.
    deletion: Option<deletion::DeletionReview>,
    /// Reply for a submitted operation, retained if the dialog closes.
    response: Option<oneshot::Receiver<SettingsResponse>>,
    /// Actionable failure or stale-review feedback.
    notice: Option<String>,
}

/// Window-owned presentation; SQLite remains the selected-view authority.
#[derive(Default)]
pub(super) struct LogbooksUi {
    /// Last catalog observed from the existing session snapshot.
    observed: Option<LogbookSnapshot>,
    /// Exact selection acknowledgment retained when a following read fails.
    acknowledged_view: Option<SelectedView>,
    /// Exact saved acknowledgments retained until a fresh catalog arrives.
    acknowledged_tags: BTreeMap<TagId, ClientRecord>,
    /// Known committed deletions absent from a stale catalog.
    deleted_tags: BTreeSet<TagId>,
    /// Selection request, separate from management editing.
    selection_response: Option<oneshot::Receiver<SettingsResponse>>,
    /// Desired view while its write is pending.
    selecting: Option<SelectedView>,
    /// Visible management destination.
    management: Option<Management>,
    /// Persistent operation/read error.
    error: Option<String>,
    /// Committed destructive effects, retained independently of dialog visibility.
    completion_notice: Option<String>,
    /// Deleted tag whose fallback needs explanation.
    fallback_notice: Option<TagId>,
    /// Whether the startup fallback has already been considered.
    startup_observed: bool,
    /// Focused workbench widget before management opened.
    opener: Option<egui::Id>,
    /// Button to focus after the modal has finished closing.
    restore_focus: Option<egui::Id>,
}

impl LogbooksUi {
    /// Observe committed catalog changes without replacing unapplied form values.
    pub fn observe(&mut self, snapshot: &LogbookSnapshot, restored_from: Option<TagId>) {
        let startup_fallback = if !self.startup_observed {
            self.startup_observed = true;
            restored_from
        } else {
            None
        };
        if self.observed.as_ref() == Some(snapshot) {
            return;
        }
        let previous = self
            .acknowledged_view
            .or_else(|| self.observed.as_ref().map(|view| view.selected_view));
        let detected_fallback = if let Some(SelectedView::Logbook(id)) = previous
            && snapshot.selected_view == SelectedView::AllContacts
            && snapshot.tags.iter().any(|row| {
                row.key().id == RecordId::Tag(id)
                    && matches!(row.working_state(), Some(RecordState::Deleted))
            }) {
            Some(id)
        } else {
            None
        };
        self.acknowledged_view = None;
        self.acknowledged_tags.clear();
        self.deleted_tags.clear();
        if let Some(form) = self.management.as_mut().and_then(|view| view.form.as_mut())
            && form.expected.is_some()
            && snapshot
                .tags
                .iter()
                .find(|row| row.key().id == RecordId::Tag(form.editor.id))
                != form.expected.as_ref()
        {
            form.stale = true;
        }
        self.observed = Some(snapshot.clone());
        if let Some(id) = startup_fallback.or(detected_fallback) {
            self.fallback(id);
        }
    }

    /// Whether the logbook manager is already open.
    pub fn is_open(&self) -> bool {
        self.management.is_some()
    }

    /// Open management without replacing an unfinished edit.
    pub fn open_management(&mut self, opener: Option<egui::Id>) {
        if self.management.is_none() && !self.is_busy() {
            self.opener = opener;
            self.management = Some(Management {
                form: None,
                deletion: None,
                response: None,
                notice: None,
            });
        }
    }

    /// Render immediate view selection in the in-window toolbar.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &LogbookSnapshot,
        sender: &mpsc::Sender<SettingsRequest>,
    ) {
        self.poll();
        if self.restore_focus.is_some() {
            if ui.ctx().memory(|memory| memory.top_modal_layer().is_none()) {
                if let Some(id) = self.restore_focus.take() {
                    ui.ctx().memory_mut(|memory| memory.request_focus(id));
                }
            } else {
                ui.ctx().request_repaint();
            }
        }
        let records = self.records(snapshot);
        let mut tags: Vec<_> = records
            .iter()
            .filter_map(tag)
            .filter(|value| !self.deleted_tags.contains(&value.id))
            .collect();
        tags.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        let labels = labels::labels(&tags);
        let selected = self.acknowledged_view.unwrap_or(snapshot.selected_view);
        let caption = match selected {
            SelectedView::AllContacts => "All contacts".into(),
            SelectedView::Untagged => "Untagged".into(),
            SelectedView::Logbook(id) => labels
                .get(&id)
                .cloned()
                .unwrap_or_else(|| format!("Unavailable logbook {id}")),
        };
        let label =
            ui.small(egui::RichText::new("Logbook").color(theme::palette(ui.ctx().theme()).muted));
        ui.add_enabled_ui(!self.is_busy(), |ui| {
            egui::ComboBox::from_id_salt("logbooks.selection")
                .selected_text(caption)
                .show_ui(ui, |ui| {
                    ui.strong("Built-in views");
                    for (view, label) in [
                        (SelectedView::AllContacts, "All contacts"),
                        (SelectedView::Untagged, "Untagged"),
                    ] {
                        if ui.selectable_label(selected == view, label).clicked()
                            && selected != view
                        {
                            self.select(sender, view);
                        }
                    }
                    ui.separator();
                    ui.strong("Named logbooks");
                    for tag in &tags {
                        if let Some(label) = labels.get(&tag.id)
                            && ui
                                .push_id(tag.id, |ui| {
                                    ui.selectable_label(
                                        selected == SelectedView::Logbook(tag.id),
                                        label,
                                    )
                                })
                                .inner
                                .clicked()
                            && selected != SelectedView::Logbook(tag.id)
                        {
                            self.select(sender, SelectedView::Logbook(tag.id));
                        }
                    }
                })
                .response
                .labelled_by(label.id);
        });
        if self.is_busy() {
            ui.spinner();
            ui.small(if self.selecting.is_some() {
                "Changing view…"
            } else {
                "Finishing logbook request…"
            });
        }
    }

    /// Present actionable results outside the compact application bar.
    pub fn feedback(&mut self, ui: &mut egui::Ui, sender: &mpsc::Sender<SettingsRequest>) {
        if let Some(id) = self.fallback_notice {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "The selected logbook {id} was deleted. Showing All contacts."
                ));
                if ui.button("Dismiss").clicked() {
                    self.fallback_notice = None;
                }
            });
        }
        if let Some(error) = &self.error {
            feedback::problem(ui, error);
        }
        if let Some(message) = &self.completion_notice {
            let dismiss = ui
                .horizontal_wrapped(|ui| {
                    ui.label(message);
                    ui.button("Dismiss result").clicked()
                })
                .inner;
            if dismiss {
                self.completion_notice = None;
            }
        }
        if self.error.is_some()
            && ui
                .add_enabled(!self.is_busy(), egui::Button::new("Refresh logbooks"))
                .clicked()
        {
            self.send_selection(sender, SettingsAction::Refresh);
        }
    }

    /// Show the native manager after the window's ordinary chrome has been laid out.
    pub fn dialog(
        &mut self,
        context: &egui::Context,
        snapshot: &LogbookSnapshot,
        sender: &mpsc::Sender<SettingsRequest>,
    ) {
        if self.management.is_none() {
            return;
        }
        let records = self.records(snapshot);
        let mut tags: Vec<_> = records
            .iter()
            .filter_map(tag)
            .filter(|tag| !self.deleted_tags.contains(&tag.id))
            .collect();
        tags.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        let labels = labels::labels(&tags);
        self.show_management(context, sender, &records, &tags, &labels);
    }

    /// Merge acknowledged saves into the catalog until a refreshed catalog arrives.
    fn records(&self, snapshot: &LogbookSnapshot) -> Vec<ClientRecord> {
        let mut records = snapshot.tags.clone();
        for (id, record) in &self.acknowledged_tags {
            records.retain(|row| row.key().id != RecordId::Tag(*id));
            records.push(record.clone());
        }
        records
    }

    /// Whether a submitted logbook request still has an unconsumed reply.
    pub fn is_busy(&self) -> bool {
        self.selection_response.is_some()
            || self
                .management
                .as_ref()
                .is_some_and(|frame| frame.response.is_some())
    }

    /// Discard unapplied management edits and schedule focus restoration to the opener.
    fn cancel(&mut self) {
        self.management = None;
        self.restore_focus = self.opener.take();
    }

    /// Submit an explicit view choice without changing its committed caption early.
    fn select(&mut self, sender: &mpsc::Sender<SettingsRequest>, next: SelectedView) {
        if self.send_selection(
            sender,
            SettingsAction::Logbooks(LogbookAction::SelectView { next }),
        ) {
            self.selecting = Some(next);
        }
    }

    /// Submit a view change or refresh, retaining the receiver if submission succeeds.
    fn send_selection(
        &mut self,
        sender: &mpsc::Sender<SettingsRequest>,
        action: SettingsAction,
    ) -> bool {
        let (reply, response) = oneshot::channel();
        match sender.try_send(SettingsRequest { action, reply }) {
            Ok(()) => {
                self.selection_response = Some(response);
                self.error = None;
                true
            }
            Err(error) => {
                self.error = Some(format!("Could not submit logbook request: {error}"));
                false
            }
        }
    }

    /// Show an explanation for a view change caused by tag deletion.
    fn fallback(&mut self, id: TagId) {
        if self.fallback_notice != Some(id) {
            let account = self
                .observed
                .as_ref()
                .and_then(|snapshot| {
                    snapshot
                        .tags
                        .iter()
                        .find(|row| row.key().id == RecordId::Tag(id))
                })
                .map(|row| row.key().account);
            if let Some(account) = account {
                tracing::info!(event="logbook.view_fallback_observed", account_id=%account, tag_id=%id, cause="deleted_tag");
            } else {
                tracing::info!(event="logbook.view_fallback_observed", tag_id=%id, cause="deleted_tag");
            }
            self.fallback_notice = Some(id);
        }
    }
}

/// Read only a live effective tag, excluding tombstones and unavailable states.
fn tag(record: &ClientRecord) -> Option<&LogbookTag> {
    match record.working_state() {
        Some(RecordState::Live(RecordValue::Tag(value))) => Some(value),
        _ => None,
    }
}

impl Management {
    /// Submit one request and retain its reply receiver until completion.
    fn send(&mut self, sender: &mpsc::Sender<SettingsRequest>, action: LogbookAction) {
        let (reply, response) = oneshot::channel();
        match sender.try_send(SettingsRequest {
            action: SettingsAction::Logbooks(action),
            reply,
        }) {
            Ok(()) => {
                self.response = Some(response);
                self.notice = None;
            }
            Err(error) => self.notice = Some(format!("Could not submit logbook request: {error}")),
        }
    }
}

#[cfg(test)]
mod tests;

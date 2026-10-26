//! Local view selection and independent saved-logbook management.

mod editor;
mod labels;
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
    /// Debounced readiness for the current fields.
    validation: ValidationState,
    /// Intervening record requires explicit reload before retry.
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

/// One management destination, retained until any admitted work settles.
struct Management {
    /// New/edit form, absent for the saved-tag list.
    form: Option<TagForm>,
    /// Admitted operation owned independently of dialog visibility.
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
    /// Dismissed destinations whose admitted results still need settling.
    dismissed: Vec<Management>,
    /// Persistent operation/read error.
    error: Option<String>,
    /// Deleted tag whose fallback needs explanation.
    fallback_notice: Option<TagId>,
    /// Startup cause is handled once per window.
    startup_observed: bool,
    /// Native management opener for focus restoration.
    opener: Option<egui::Id>,
    /// Deferred native focus request after dismissal.
    restore_focus: Option<egui::Id>,
}

impl LogbooksUi {
    /// Observe committed catalog changes without replacing unapplied form values.
    pub fn observe(&mut self, snapshot: &LogbookSnapshot, restored_from: Option<TagId>) {
        if !self.startup_observed {
            self.startup_observed = true;
            if let Some(id) = restored_from {
                self.fallback(id);
            }
        }
        if self.observed.as_ref() == Some(snapshot) {
            return;
        }
        let previous = self
            .acknowledged_view
            .or_else(|| self.observed.as_ref().map(|view| view.selected_view));
        if let Some(SelectedView::Logbook(id)) = previous
            && snapshot.selected_view == SelectedView::AllContacts
            && snapshot.tags.iter().any(|row| {
                row.key.id == RecordId::Tag(id)
                    && matches!(row.working_state(), Some(RecordState::Deleted))
            })
        {
            self.fallback(id);
        }
        self.acknowledged_view = None;
        self.acknowledged_tags.clear();
        self.deleted_tags.clear();
        if let Some(form) = self.management.as_mut().and_then(|view| view.form.as_mut())
            && form.expected.is_some()
            && snapshot
                .tags
                .iter()
                .find(|row| row.key.id == RecordId::Tag(form.editor.id))
                != form.expected.as_ref()
        {
            form.stale = true;
        }
        self.observed = Some(snapshot.clone());
    }

    /// Render the real selector and dedicated management dialog without store I/O.
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
        let mut records = snapshot.tags.clone();
        for (id, record) in &self.acknowledged_tags {
            records.retain(|row| row.key.id != RecordId::Tag(*id));
            records.push(record.clone());
        }
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
        theme::section_frame(ui).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Logbook");
                ui.add_enabled_ui(self.selection_response.is_none(), |ui| {
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
                        });
                });
                if self.selecting.is_some() {
                    ui.spinner();
                    ui.small("Changing view…");
                }
                let manage = ui.button("Manage logbooks…");
                if manage.clicked() {
                    self.opener = Some(manage.id);
                    self.management = Some(Management {
                        form: None,
                        response: None,
                        notice: None,
                    });
                }
            });
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
            if self.error.is_some()
                && ui
                    .add_enabled(
                        self.selection_response.is_none(),
                        egui::Button::new("Refresh logbooks"),
                    )
                    .clicked()
            {
                self.send_selection(sender, SettingsAction::Refresh);
            }
        });
        self.show_management(ui.ctx(), sender, &records, &tags, &labels);
    }

    /// Dismiss unapplied management edits while retaining admitted results.
    fn cancel(&mut self) {
        if let Some(frame) = self.management.take()
            && frame.response.is_some()
        {
            self.dismissed.push(frame);
        }
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

    /// Retain the response receiver only when the existing lane admits this request.
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

    /// Explain an observed committed fallback without claiming an uncommitted write.
    fn fallback(&mut self, id: TagId) {
        if self.fallback_notice != Some(id) {
            tracing::info!(event="logbook.view_fallback_observed", tag_id=%id);
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
    /// Admit one operation without dropping its eventual acknowledgment.
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

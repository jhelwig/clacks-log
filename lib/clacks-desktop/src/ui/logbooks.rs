//! Immediate selection of the working logbook view.

pub(super) mod editor;
pub(super) mod labels;

use super::{feedback, theme};
use crate::session::{
    SessionCommand,
    settings::{
        SettingsAction, SettingsRequest, SettingsResponse,
        logbooks::{LogbookAction, LogbookResponse},
    },
};
use clacks_client_store::logbooks::{LogbookSnapshot, selection::SelectedView};
use clacks_model::{ids::TagId, tag::LogbookTag};
use clacks_sync::{
    client::ClientRecord,
    record::{RecordId, RecordState, RecordValue},
};
use eframe::egui;
use std::collections::BTreeMap;
use tokio::sync::{mpsc, oneshot};

/// Window-owned presentation; SQLite remains the selected-view authority.
#[derive(Default)]
pub(super) struct LogbooksUi {
    /// Last catalog observed from the existing session snapshot.
    observed: Option<LogbookSnapshot>,
    /// Exact selection acknowledgment retained when a following read fails.
    acknowledged_view: Option<SelectedView>,
    /// Pending selection until the store acknowledges its result.
    selection_response: Option<oneshot::Receiver<SettingsResponse>>,
    /// Read the authoritative selection after a lost response.
    recover: bool,
    /// Persistent selection failure.
    error: Option<String>,
    /// Deleted tag whose fallback needs explanation.
    fallback_notice: Option<TagId>,
    /// Whether the startup fallback has already been considered.
    startup_observed: bool,
}

impl LogbooksUi {
    /// Observe the selected view and explain a fallback caused by deleting its logbook.
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
        self.observed = Some(snapshot.clone());
        if let Some(id) = startup_fallback.or(detected_fallback) {
            self.fallback(id);
        }
    }

    /// Describe a history scope using the same live catalog as the toolbar.
    pub(super) fn view_caption(&self, snapshot: &LogbookSnapshot, view: SelectedView) -> String {
        let tags: Vec<_> = snapshot.tags.iter().filter_map(tag).collect();
        view_caption(view, &labels::labels(&tags))
    }

    /// Render immediate view selection in the in-window toolbar.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &LogbookSnapshot,
        sender: &mpsc::Sender<SettingsRequest>,
    ) {
        let mut tags: Vec<_> = snapshot.tags.iter().filter_map(tag).collect();
        tags.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        let labels = labels::labels(&tags);
        let selected = self.acknowledged_view.unwrap_or(snapshot.selected_view);
        let caption = view_caption(selected, &labels);
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
            ui.small("Changing view…");
        }
    }

    /// Present actionable results outside the compact application bar.
    pub fn feedback(&mut self, ui: &mut egui::Ui) {
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
    }

    /// Whether an immediate selection is awaiting acknowledgment.
    pub fn is_busy(&self) -> bool {
        self.selection_response.is_some()
    }

    /// Submit an explicit view choice without changing its committed caption early.
    fn select(&mut self, sender: &mpsc::Sender<SettingsRequest>, next: SelectedView) {
        let (reply, response) = oneshot::channel();
        let action = SettingsAction::Logbooks(LogbookAction::SelectView { next });
        match sender.try_send(SettingsRequest { action, reply }) {
            Ok(()) => {
                self.selection_response = Some(response);
                self.error = None;
            }
            Err(error) => self.error = Some(format!("Could not select the logbook: {error}")),
        }
    }

    /// Consume the selection acknowledgment independently of its follow-up read.
    pub(in crate::ui) fn poll(&mut self) {
        let Some(response) = &mut self.selection_response else {
            return;
        };
        let mut outcome = match response.try_recv() {
            Ok(outcome) => outcome,
            Err(oneshot::error::TryRecvError::Empty) => return,
            Err(oneshot::error::TryRecvError::Closed) => {
                self.recover = true;
                self.selection_response = None;
                return;
            }
        };
        self.selection_response = None;
        if let SettingsResponse::RefreshFailed {
            outcome: settled, ..
        } = outcome
        {
            outcome = *settled;
        }
        match outcome {
            SettingsResponse::Logbooks(LogbookResponse::Selected(view)) => {
                self.acknowledged_view = Some(view);
                self.fallback_notice = None;
            }
            SettingsResponse::Failed(message) => self.error = Some(message),
            SettingsResponse::Closing => self.error = Some("The application is closing.".into()),
            _ => self.recover = true,
        }
    }

    /// Request an authoritative read after losing a selection response.
    pub(super) fn recover_view(&mut self, commands: &mpsc::Sender<SessionCommand>) {
        if self.recover
            && commands
                .try_send(SessionCommand::RecoverSettingsView)
                .is_ok()
        {
            self.recover = false;
            self.acknowledged_view = None;
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

/// Format a built-in view or a named logbook using the provided catalog labels.
fn view_caption(view: SelectedView, labels: &BTreeMap<TagId, String>) -> String {
    match view {
        SelectedView::AllContacts => "All contacts".into(),
        SelectedView::Untagged => "Untagged".into(),
        SelectedView::Logbook(id) => labels
            .get(&id)
            .cloned()
            .unwrap_or_else(|| format!("Unavailable logbook {id}")),
    }
}

/// Read only a live effective tag, excluding tombstones and unavailable states.
pub(super) fn tag(record: &ClientRecord) -> Option<&LogbookTag> {
    match record.working_state() {
        Some(RecordState::Live(RecordValue::Tag(value))) => Some(value),
        _ => None,
    }
}
#[cfg(test)]
mod tests;

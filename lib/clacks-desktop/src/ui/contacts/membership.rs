//! Selected-contact assignment, with frozen targets and retained operation results.

use super::{
    ContactAction, ContactResponse, ContactsUi, SettingsAction, SettingsRequest, SettingsResponse,
};
use crate::{
    session::settings::contacts::membership::{MembershipBasis, MembershipSubmission},
    ui::{
        feedback,
        tag_picker::{self, MembershipPicker},
    },
};
use clacks_client_store::save::{
    SaveOutcome,
    tags::{TagChangeProgress, TagChangeReport, TagChangeStop},
};
use clacks_model::ids::QsoId;
use clacks_sync::record::{RecordState, RecordValue};
use eframe::egui;
use std::collections::{BTreeMap, BTreeSet};
use tokio::sync::{mpsc, oneshot, watch};
use tokio_util::sync::CancellationToken;

/// One pending assignment workflow, independent of the native popup lifetime.
pub(super) struct BulkMembership {
    /// Selected identities captured before reading their complete bases.
    ids: BTreeSet<QsoId>,
    /// Hidden selection count presented with the captured scope.
    hidden: Option<u64>,
    /// Originating preparation or submission response.
    reply: Option<oneshot::Receiver<SettingsResponse>>,
    /// Consistent save bases, absent until preparation succeeds.
    basis: Option<MembershipBasis>,
    /// Pending tag choices, independent of search visibility.
    picker: Option<MembershipPicker>,
    /// Cancellation exists only once submission has been admitted.
    cancel: Option<CancellationToken>,
    /// Latest settled progress, not an acknowledgment of the whole operation.
    progress: Option<watch::Receiver<TagChangeProgress>>,
    /// Final authoritative outcomes retained until explicitly dismissed.
    report: Option<TagChangeReport>,
    /// Existing contacted callsigns for useful per-record review.
    captions: BTreeMap<QsoId, String>,
    /// Preparation, submission or refresh problem.
    error: Option<String>,
}

impl BulkMembership {
    /// Whether this workflow has submitted writes, including retained final results.
    fn submitted(&self) -> bool {
        self.cancel.is_some()
    }

    /// Whether a submitted operation is awaiting its final acknowledgment.
    fn running(&self) -> bool {
        self.submitted() && self.reply.is_some()
    }

    /// Accept the originating outcome without losing it to a later refresh error.
    fn accept(&mut self, response: SettingsResponse) -> bool {
        match response {
            SettingsResponse::Contacts(ContactResponse::MembershipPrepared(basis)) => {
                let memberships: Vec<_> = basis
                    .records
                    .iter()
                    .filter_map(|row| match row.working_state() {
                        Some(RecordState::Live(RecordValue::Qso(qso))) => Some(qso.tags().clone()),
                        _ => None,
                    })
                    .collect();
                self.captions = basis
                    .records
                    .iter()
                    .filter_map(|row| match row.working_state() {
                        Some(RecordState::Live(RecordValue::Qso(qso))) => Some((
                            qso.id(),
                            qso.contacted_station().callsign.as_str().to_owned(),
                        )),
                        _ => None,
                    })
                    .collect();
                self.picker = Some(MembershipPicker::new(&basis.catalog, &memberships));
                self.basis = Some(basis);
                false
            }
            SettingsResponse::Contacts(ContactResponse::MembershipChanged(report)) => {
                let changed = report
                    .entries
                    .iter()
                    .any(|entry| matches!(entry.outcome, SaveOutcome::Saved(_)));
                self.report = Some(report);
                changed
            }
            SettingsResponse::RefreshFailed { outcome, message } => {
                let changed = self.accept(*outcome);
                self.error = Some(message);
                changed
            }
            SettingsResponse::Failed(message) => {
                self.error = Some(message);
                false
            }
            SettingsResponse::Closing => {
                self.error = Some("The workspace is closing; no further work was admitted.".into());
                false
            }
            _ => {
                self.error =
                    Some("Unexpected assignment response. Refresh before retrying.".into());
                false
            }
        }
    }

    /// Submit exactly the records and checkbox intent displayed in this picker.
    fn apply(&mut self, requests: &mpsc::Sender<SettingsRequest>) {
        let (Some(basis), Some(picker)) = (&self.basis, &self.picker) else {
            return;
        };
        if self.submitted() || !picker.is_changed() {
            return;
        }
        let intent = picker.intent();
        let cancel = CancellationToken::new();
        let (progress, receiver) = watch::channel(TagChangeProgress {
            total: basis.records.len(),
            ..Default::default()
        });
        let (reply, response) = oneshot::channel();
        let action = ContactAction::ChangeMembership(MembershipSubmission {
            basis: basis.records.clone(),
            add: intent.add,
            remove: intent.remove,
            cancel: cancel.clone(),
            progress,
        });
        match requests.try_send(SettingsRequest {
            action: SettingsAction::Contacts(action),
            reply,
        }) {
            Ok(()) => {
                self.cancel = Some(cancel);
                self.progress = Some(receiver);
                self.reply = Some(response);
                self.error = None;
            }
            Err(error) => {
                self.error = Some(format!("Could not submit assignment changes: {error}"))
            }
        }
    }
}

impl ContactsUi {
    /// Render the contextual action and native popup without changing history scope.
    pub(super) fn membership_action(
        &mut self,
        ui: &mut egui::Ui,
        requests: &mpsc::Sender<SettingsRequest>,
    ) {
        if self.selected.is_empty() {
            return;
        }
        let running = self
            .membership
            .as_ref()
            .is_some_and(BulkMembership::running);
        let button = ui.add_enabled(!running, egui::Button::new("Edit assigned logbooks…"));
        if button.clicked() {
            let (reply, receiver) = oneshot::channel();
            let ids = self.selected.clone();
            let result = requests.try_send(SettingsRequest {
                action: SettingsAction::Contacts(ContactAction::PrepareMembership {
                    ids: ids.clone(),
                }),
                reply,
            });
            self.membership = Some(BulkMembership {
                ids,
                hidden: self
                    .page
                    .as_ref()
                    .filter(|_| self.page_request.is_none() && !self.dirty)
                    .map(|page| page.selected_outside_filter),
                reply: result.as_ref().ok().map(|_| receiver),
                basis: None,
                picker: None,
                cancel: None,
                progress: None,
                report: None,
                captions: BTreeMap::new(),
                error: result
                    .err()
                    .map(|error| format!("Could not read selected contacts: {error}")),
            });
        }
        let mut apply = false;
        let mut dismiss = false;
        let popup = egui::Popup::from_toggle_button_response(&button)
            .id(egui::Id::unique("contacts.membership"))
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
        let popup_id = popup.get_id();
        popup.show(|ui| {
            let Some(pending) = &mut self.membership else {
                return;
            };
            ui.strong("Edit assigned logbooks");
            ui.small(format!("Selected contacts: {}", pending.ids.len()));
            if let Some(hidden) = pending.hidden {
                ui.small(format!(
                    "{hidden} outside filter; off-page selections are included."
                ));
            } else {
                ui.small("Includes selected contacts on other pages and outside the filter.");
            }
            if let Some(error) = &pending.error {
                feedback::problem(ui, error);
            }
            let submitted = pending.submitted();
            if let Some(picker) = &mut pending.picker {
                ui.add_enabled_ui(!submitted, |ui| {
                    tag_picker::show_contents(ui, picker);
                });
                if !submitted {
                    ui.horizontal(|ui| {
                        dismiss = ui.button("Cancel").clicked();
                        apply = ui
                            .add_enabled(picker.is_changed(), egui::Button::new("Apply"))
                            .clicked();
                    });
                } else {
                    ui.small("Submitted; progress and results are shown in contact history.");
                }
            } else if pending.reply.is_some() {
                ui.label("Loading selected contacts…");
            }
        });
        if apply && let Some(pending) = &mut self.membership {
            pending.apply(requests);
        }
        if dismiss {
            egui::Popup::close_id(ui.ctx(), popup_id);
        }
        if !egui::Popup::is_id_open(ui.ctx(), popup_id)
            && self
                .membership
                .as_ref()
                .is_some_and(|pending| !pending.submitted())
        {
            self.membership = None;
        }
    }

    /// Poll captured reads/writes even when the popup or original selection is gone.
    pub(super) fn poll_membership(&mut self) {
        let Some(pending) = &mut self.membership else {
            return;
        };
        let result = pending.reply.as_mut().map(oneshot::Receiver::try_recv);
        match result {
            Some(Ok(response)) => {
                pending.reply = None;
                if pending.accept(response) {
                    self.changed = true;
                    self.dirty = true;
                }
            }
            Some(Err(oneshot::error::TryRecvError::Closed)) => {
                pending.reply = None;
                pending.error = Some("Assignment acknowledgment was lost. Refresh and review current contacts before retrying; do not assume the writes failed.".into());
                if pending.submitted() {
                    self.dirty = true;
                    self.changed = true;
                }
            }
            _ => {}
        }
    }

    /// Drop obsolete preparation, retaining admitted writes and their results.
    pub(super) fn membership_view_changed(&mut self) {
        if self
            .membership
            .as_ref()
            .is_some_and(|pending| !pending.submitted())
        {
            self.membership = None;
        }
    }

    /// Keep progress and per-record outcomes visible independently of the popup.
    pub(super) fn membership_feedback(&mut self, ui: &mut egui::Ui) {
        let Some(pending) = &mut self.membership else {
            return;
        };
        if pending.reply.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
        if pending.running() {
            let progress = pending
                .progress
                .as_ref()
                .map(|receiver| receiver.borrow().clone())
                .unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label(format!(
                    "Assigning logbooks: {} of {} contacts processed",
                    progress.completed, progress.total
                ));
                if ui.button("Cancel processing").clicked()
                    && let Some(cancel) = &pending.cancel
                {
                    cancel.cancel();
                }
            });
        }
        if let Some(error) = &pending.error {
            feedback::problem(ui, error);
        }
        let mut dismiss = false;
        if let Some(report) = &pending.report {
            let changed = report
                .entries
                .iter()
                .filter(|entry| matches!(entry.outcome, SaveOutcome::Saved(_)))
                .count();
            let unchanged = report
                .entries
                .iter()
                .filter(|entry| matches!(entry.outcome, SaveOutcome::Unchanged(_)))
                .count();
            let stale = report
                .entries
                .iter()
                .filter(|entry| matches!(entry.outcome, SaveOutcome::Stale { .. }))
                .count();
            let failed = usize::from(matches!(report.stop, Some(TagChangeStop::Failed { .. })));
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Logbook assignments: {changed} saved · {unchanged} unchanged · {stale} stale · {} unprocessed · {failed} failed/unacknowledged", report.unprocessed.len()));
                dismiss = ui.small_button("Dismiss assignment results").clicked();
            });
            if matches!(report.stop, Some(TagChangeStop::Cancelled)) {
                ui.small("Processing cancelled; completed changes were retained.");
            }
            if let Some(TagChangeStop::Failed { id, .. }) = &report.stop {
                feedback::problem(
                    ui,
                    &format!(
                        "{id} · No successful acknowledgment. Review current state before retrying."
                    ),
                );
            }
            ui.collapsing("Assignment results", |ui| {
                egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                    for entry in &report.entries {
                        let caption = pending.captions.get(&entry.id).map(String::as_str).unwrap_or("");
                        let result = match entry.outcome { SaveOutcome::Saved(_) => "Saved", SaveOutcome::Unchanged(_) => "Unchanged", SaveOutcome::Stale { .. } => "Stale — refresh and review" };
                        ui.label(format!("{caption} · {} · {result}", entry.id));
                    }
                    if let Some(TagChangeStop::Failed { id, error }) = &report.stop {
                        feedback::problem(ui, &format!("{id} · No successful acknowledgment: {error:#}. Review current state before retrying."));
                    }
                    for id in &report.unprocessed { ui.label(format!("{id} · Unprocessed")); }
                });
            });
        } else if pending.submitted() && !pending.running() {
            dismiss = ui.small_button("Dismiss assignment notice").clicked();
        }
        if dismiss {
            self.membership = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::{Harness, kittest::Queryable};

    #[test]
    fn failed_assignment_is_visible_without_expanding_results() {
        let id = QsoId::new();
        let pending = BulkMembership {
            ids: [id].into(),
            hidden: None,
            reply: None,
            basis: None,
            picker: None,
            cancel: Some(CancellationToken::new()),
            progress: None,
            report: Some(TagChangeReport {
                entries: vec![],
                stop: Some(TagChangeStop::Failed {
                    id,
                    error: anyhow::anyhow!("write acknowledgment unavailable"),
                }),
                unprocessed: vec![],
            }),
            captions: BTreeMap::new(),
            error: None,
        };
        let view = ContactsUi {
            membership: Some(pending),
            ..Default::default()
        };
        let ui = Harness::builder().build_ui_state(
            |ui, view: &mut ContactsUi| view.membership_feedback(ui),
            view,
        );
        assert!(
            ui.query_by_label_contains("1 failed/unacknowledged")
                .is_some()
        );
        assert!(
            ui.query_by_label_contains("No successful acknowledgment")
                .is_some()
        );
    }
}

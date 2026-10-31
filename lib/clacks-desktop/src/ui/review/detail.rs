//! Selected-record presentation, including deletion and recovery states.

use super::{ReviewUi, caption, comparison::ComparisonAction, reason_label};
use clacks_client_store::read::review::ReviewRow;
use clacks_sync::record::{Observation, RecordState};
use eframe::egui;

impl ReviewUi {
    /// Show the pinned comparison while newer saved roles remain separate.
    pub(super) fn show_detail(
        &mut self,
        ui: &mut egui::Ui,
        requests: &tokio::sync::mpsc::Sender<crate::session::settings::SettingsRequest>,
    ) {
        let Some(mut row) = self.detail.clone() else {
            ui.label("Select a record to review.");
            return;
        };
        let key = row.record.key();
        if let Some(editor) = self.drafts.get(&key) {
            row.record = editor.draft.basis.record.clone();
            row.review = Some(editor.draft.basis.review.clone());
        }
        let actionable = self
            .drafts
            .get(&key)
            .is_none_or(|editor| !editor.settled && !editor.draft.is_stale());
        ui.heading(caption(&row.record, &self.tag_labels));
        ui.label(format!("ID: {}", key.id));
        if let Some(review) = &row.review {
            ui.label(reason_label(&review.reason));
        }
        if let Some(review) = &row.review
            && matches!(
                review.reason,
                clacks_sync::client::ReviewReason::Referenced { .. }
            )
        {
            self.show_references(ui, &row.record, review, requests, actionable);
        }
        let busy = self.reply.is_some() || self.detail_dirty;
        if let Some(review) = &row.review
            && matches!(
                key.id,
                clacks_sync::record::RecordId::Qso(_)
                    | clacks_sync::record::RecordId::Profile(_)
                    | clacks_sync::record::RecordId::Equipment(_)
                    | clacks_sync::record::RecordId::Tag(_)
            )
        {
            let label = match &review.shared {
                Observation::Absent { .. } => Some("Discard local version"),
                Observation::Accepted(shared)
                    if matches!(shared.state(), RecordState::Deleted)
                        && matches!(review.local, RecordState::Deleted) =>
                {
                    Some("Confirm deletion")
                }
                _ => None,
            };
            if let Some(label) = label
                && ui
                    .add_enabled(!busy && actionable, egui::Button::new(label))
                    .clicked()
            {
                self.submit = Some(super::SaveRequest {
                    expected: Box::new(row.record.clone()),
                    shown: Box::new(review.clone()),
                    chosen: RecordState::Deleted,
                });
            }
        }

        let action =
            if let (Some(editor), Some(settings)) = (self.drafts.get_mut(&key), &self.settings) {
                editor.show(ui, settings, &self.tag_labels, busy)
            } else {
                show_pending(ui, &row);
                ComparisonAction::None
            };
        match action {
            ComparisonAction::Refresh => {
                self.refresh_for = Some(key);
                self.detail_dirty = true;
            }
            ComparisonAction::Save { chosen } => {
                if let Some(editor) = self.drafts.get(&key) {
                    self.submit = Some(super::SaveRequest {
                        expected: Box::new(editor.draft.basis.record.clone()),
                        shown: Box::new(editor.draft.basis.review.clone()),
                        chosen,
                    });
                }
            }
            ComparisonAction::None => {}
        }
        if self.saving == Some(key) {
            ui.label("Saving…");
        }
    }
}

/// Describe actual unpublished and blocked work without offering a fake resolution.
fn show_pending(ui: &mut egui::Ui, row: &ReviewRow) {
    if matches!(row.record.latest(), Observation::Unavailable { .. })
        || (row.review.is_none()
            && (row.authorization_hold.is_some() || !row.record.retained().is_empty()))
    {
        ui.label("Waiting for comparison data. Local work is preserved.");
    } else if row.review.is_none() {
        ui.label(
            if matches!(row.record.working_state(), Some(RecordState::Deleted)) {
                "Deletion saved locally; pending synchronization."
            } else {
                "Saved locally; pending synchronization."
            },
        );
    } else if matches!(
        row.record.key().id,
        clacks_sync::record::RecordId::Qso(_)
            | clacks_sync::record::RecordId::Profile(_)
            | clacks_sync::record::RecordId::Equipment(_)
            | clacks_sync::record::RecordId::Tag(_)
    ) {
        ui.label("There are no live field values in the current comparison.");
    } else {
        ui.label(
            "This kind of record cannot be edited in this window. Its saved data is preserved.",
        );
    }
}

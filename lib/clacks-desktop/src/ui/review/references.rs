//! Accepted referrers and explicit repair navigation for blocked deletions.

use super::{ReviewUi, caption};
use crate::session::settings::{SettingsRequest, review::ReviewAction};
use clacks_client_store::read::review::references::ReferencePage;
use clacks_sync::{
    client::{ClientRecord, ReviewInput},
    record::{RecordKey, RecordState},
};
use eframe::egui;
use tokio::sync::mpsc;

/// Loaded accepted reference page, qualified by its actual deletion target.
pub(super) struct References {
    /// Native target identity.
    pub key: RecordKey,
    /// Accepted evidence count and bounded referrers.
    pub page: ReferencePage,
}

impl ReviewUi {
    /// Show shared blockers and navigate to the existing owning repair workflow.
    pub(super) fn show_references(
        &mut self,
        ui: &mut egui::Ui,
        record: &ClientRecord,
        review: &ReviewInput,
        requests: &mpsc::Sender<SettingsRequest>,
        actionable: bool,
    ) {
        let key = record.key();
        if self
            .references
            .as_ref()
            .is_none_or(|value| value.key != key)
            && self.read_references.is_none()
        {
            self.read_references = Some((key, None));
        }
        ui.strong("Accepted references blocking deletion");
        ui.label("Local repairs remain listed until they are accepted through synchronization.");
        if ui
            .add_enabled(
                self.reply.is_none(),
                egui::Button::new("Refresh references"),
            )
            .clicked()
        {
            self.read_references = Some((key, None));
        }
        if let Some(references) = &self.references
            && references.key == key
        {
            ui.label(format!(
                "{} accepted referring records",
                references.page.total
            ));
            for row in &references.page.rows {
                ui.horizontal_wrapped(|ui| {
                    ui.label(caption(&row.record, &self.tag_labels));
                    if row.review.is_some() {
                        if ui.button("Review conflict").clicked() {
                            self.selected = Some(row.record.key());
                            self.detail_dirty = true;
                        }
                    } else if matches!(row.record.working_state(), Some(RecordState::Live(_))) {
                        if ui.button("Edit referring record").clicked() {
                            self.repair = Some(row.record.clone());
                        }
                    } else {
                        ui.weak("Local deletion awaiting acceptance");
                    }
                });
            }
            if let Some(after) = references.page.next
                && ui
                    .add_enabled(
                        self.reply.is_none(),
                        egui::Button::new("Load more references"),
                    )
                    .clicked()
            {
                self.read_references = Some((key, Some(after)));
            }
        }
        if ui
            .add_enabled(
                self.reply.is_none() && actionable,
                egui::Button::new("Retry deletion"),
            )
            .clicked()
            && self.request(
                ReviewAction::RetryDeletion {
                    expected: Box::new(record.clone()),
                    shown: Box::new(review.clone()),
                },
                requests,
            )
        {
            self.saving = Some(key);
        }
        ui.separator();
    }
}

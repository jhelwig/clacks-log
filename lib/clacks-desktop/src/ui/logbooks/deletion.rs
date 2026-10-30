//! Review QSO changes before deleting a logbook tag.

use super::*;
use clacks_client_store::logbooks::deletion::TagDeletionPreview;
use clacks_model::tag::TagDeletionChoice;
use std::sync::Arc;

/// Tag/QSO snapshot and the user's deletion choice.
pub(super) struct DeletionReview {
    /// Tag and QSO snapshot used to show and confirm this deletion.
    pub(super) preview: Arc<TagDeletionPreview>,
    /// User's chosen QSO treatment; absent until explicitly selected.
    pub(super) choice: Option<TagDeletionChoice>,
    /// Require another preview after the saved basis changed.
    pub(super) stale: bool,
    /// Focus the safe action once when entering or reloading this review.
    pub(super) focus_cancel: bool,
}

/// Show deletion choices and require explicit confirmation.
pub(super) fn show(
    ui: &mut egui::Ui,
    frame: &mut Management,
    sender: &mpsc::Sender<SettingsRequest>,
) {
    let Some(review) = &mut frame.deletion else {
        return;
    };
    if let Some(tag) = tag(&review.preview.tag) {
        ui.heading(format!("Delete {}", tag.name));
    }
    ui.monospace(review.preview.tag.key().id.to_string());
    let counts = &review.preview.counts;
    ui.label(format!(
        "{} contacts · {} also tagged elsewhere · {} unique to this logbook",
        counts.affected, counts.also_tagged_elsewhere, counts.unique
    ));
    ui.label("Deleting a contact marks the QSO deleted. Leaving contacts removes only this tag; other logbooks and external service copies remain untouched.");
    ui.add_enabled_ui(!review.stale, |ui| {
        for (choice, label) in [
            (
                TagDeletionChoice::LeaveAll,
                "Leave all contacts (delete 0)".into(),
            ),
            (
                TagDeletionChoice::DeleteAll,
                format!("Delete all contacts (delete {})", counts.affected),
            ),
            (
                TagDeletionChoice::DeleteUnique,
                format!("Delete unique contacts (delete {})", counts.unique),
            ),
        ] {
            ui.radio_value(&mut review.choice, Some(choice), label);
        }
    });
    let confirm = ui
        .add_enabled(
            !review.stale && review.choice.is_some(),
            egui::Button::new("Confirm deletion"),
        )
        .clicked();
    let reload = review.stale && ui.button("Reload deletion review").clicked();
    let cancel = ui.button("Cancel deletion");
    if review.focus_cancel {
        cancel.request_focus();
        review.focus_cancel = false;
    }
    if confirm && let Some(choice) = review.choice {
        let expected = Arc::clone(&review.preview);
        frame.send(sender, LogbookAction::DeleteTag { expected, choice });
    } else if reload {
        let RecordId::Tag(id) = review.preview.tag.key().id else {
            return;
        };
        frame.send(sender, LogbookAction::PreviewDeletion { id });
    } else if cancel.clicked() {
        frame.deletion = None;
        frame.notice = None;
    }
}

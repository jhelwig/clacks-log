//! Execute logbook management and view-selection requests on the desktop session.

use clacks_client_store::{
    logbooks::{
        deletion::{self, TagDeleteOutcome, TagDeletionPreview},
        selection::{self, SelectedView},
        tags,
    },
    save::SaveOutcome,
    store::ClientStore,
};
use clacks_model::{
    ids::TagId,
    tag::{LogbookTag, TagDeletionChoice},
};
use clacks_sync::client::ClientRecord;
use std::sync::Arc;

/// Logbook requests that can finish even if their originating dialog closes.
pub(crate) enum LogbookAction {
    /// Persist the local view without changing station settings.
    SelectView {
        /// Desired built-in or stable tag identity.
        next: SelectedView,
    },
    /// Save a tag without selecting it.
    SaveTag {
        /// Full original record, absent for a new tag.
        expected: Option<Box<ClientRecord>>,
        /// Validated desired values.
        next: LogbookTag,
    },
    /// Load the tag, affected QSOs and counts for deletion review.
    PreviewDeletion {
        /// Tag being reviewed.
        id: TagId,
    },
    /// Commit a reviewed deletion and its matching selection fallback.
    DeleteTag {
        /// Tag and QSO snapshot shown in the deletion dialog.
        expected: Arc<TagDeletionPreview>,
        /// Explicit contact treatment chosen by the operator.
        choice: TagDeletionChoice,
    },
}

impl LogbookAction {
    /// Local operation name without containing-action context or request payloads.
    pub(crate) fn diagnostic_name(&self) -> &'static str {
        match self {
            Self::SelectView { .. } => "select_view",
            Self::SaveTag { .. } => "save_tag",
            Self::PreviewDeletion { .. } => "preview_deletion",
            Self::DeleteTag { .. } => "delete_tag",
        }
    }
}

/// Store result retained even when the subsequent catalog refresh fails.
#[derive(Debug)]
pub(crate) enum LogbookResponse {
    /// Committed selected view.
    Selected(SelectedView),
    /// Saved, unchanged or stale tag values.
    Saved(Box<SaveOutcome>),
    /// Tag and QSO snapshot with counts to show before deletion.
    DeletionPreview(Arc<TagDeletionPreview>),
    /// Committed deletion effects or a stale review.
    Deleted(TagDeleteOutcome),
}

/// Execute a logbook request and return its store result.
pub(crate) async fn execute(
    store: &ClientStore,
    action: LogbookAction,
) -> anyhow::Result<LogbookResponse> {
    match action {
        LogbookAction::SelectView { next } => selection::select(store, next)
            .await
            .map(LogbookResponse::Selected),
        LogbookAction::SaveTag { expected, next } => tags::save(store, expected.as_deref(), &next)
            .await
            .map(|outcome| LogbookResponse::Saved(Box::new(outcome))),
        LogbookAction::PreviewDeletion { id } => deletion::preview(store, id)
            .await
            .map(|preview| LogbookResponse::DeletionPreview(Arc::new(preview))),
        LogbookAction::DeleteTag { expected, choice } => {
            deletion::delete_tag(store, &expected, choice)
                .await
                .map(LogbookResponse::Deleted)
        }
    }
}

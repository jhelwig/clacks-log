//! Logbook operations admitted through the existing settings request lane.

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

/// Owned logbook inputs, retained independently from the originating dialog.
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
    /// Read exact contact effects before asking for a deletion choice.
    PreviewDeletion {
        /// Tag being reviewed.
        id: TagId,
    },
    /// Commit a reviewed deletion and its matching selection fallback.
    DeleteTag {
        /// Immutable complete basis shared with the dialog.
        expected: Arc<TagDeletionPreview>,
        /// Explicit contact treatment chosen by the operator.
        choice: TagDeletionChoice,
    },
}

/// Authoritative logbook result, independent of following catalog refreshes.
#[derive(Debug)]
pub(crate) enum LogbookResponse {
    /// Committed selected view.
    Selected(SelectedView),
    /// Saved, unchanged or stale tag values.
    Saved(Box<SaveOutcome>),
    /// Exact review with precomputed impact counts.
    DeletionPreview(Arc<TagDeletionPreview>),
    /// Committed deletion effects or a stale review.
    Deleted(TagDeleteOutcome),
}

/// Run one admitted operation using the production store transaction boundaries.
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

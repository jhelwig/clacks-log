//! Workbench view selection and consistent logbook deletion previews.

use clacks_client_store::{
    logbooks::{
        deletion::{self, TagDeletionPreview},
        selection::{self, SelectedView},
    },
    store::ClientStore,
};
use clacks_model::ids::TagId;
use std::sync::Arc;

/// Logbook selection or impact review requested from the current workspace.
pub(crate) enum LogbookAction {
    /// Persist the local view without changing station settings.
    SelectView {
        /// Desired built-in or stable tag identity.
        next: SelectedView,
    },
    /// Load the tag, affected QSOs and counts for deletion review.
    PreviewDeletion {
        /// Tag being reviewed.
        id: TagId,
    },
}

impl LogbookAction {
    /// Local operation name without containing-action context or request payloads.
    pub(crate) fn diagnostic_name(&self) -> &'static str {
        match self {
            Self::SelectView { .. } => "select_view",
            Self::PreviewDeletion { .. } => "preview_deletion",
        }
    }
}

/// Store result retained even when the subsequent catalog refresh fails.
#[derive(Debug)]
pub(crate) enum LogbookResponse {
    /// Committed selected view.
    Selected(SelectedView),
    /// Tag and QSO snapshot with counts to show before deletion.
    DeletionPreview(Arc<TagDeletionPreview>),
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
        LogbookAction::PreviewDeletion { id } => deletion::preview(store, id)
            .await
            .map(|preview| LogbookResponse::DeletionPreview(Arc::new(preview))),
    }
}

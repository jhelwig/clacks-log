//! Tracked workspace initialization; admitted database work is never aborted on close.

use crate::workspace::{self, OpenedWorkspace, WorkspacePaths};
use tokio::task::JoinSet;

/// Start one real open attempt, keeping its returned pool available for orderly cleanup.
pub(super) fn begin(tasks: &mut JoinSet<anyhow::Result<OpenedWorkspace>>, paths: WorkspacePaths) {
    tasks.spawn(async move { workspace::open(&paths).await });
}

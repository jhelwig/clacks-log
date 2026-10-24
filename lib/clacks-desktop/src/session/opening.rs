//! Tracked workspace initialization; admitted database work is never aborted on close.

use crate::workspace::{self, OpenedWorkspace, WorkspacePaths};
use tokio::task::JoinSet;

/// Start one real open attempt, keeping its returned pool available for orderly cleanup.
pub(super) fn begin(
    tasks: &mut JoinSet<anyhow::Result<OpenedWorkspace>>,
    paths: WorkspacePaths,
    #[cfg(test)] gate: Option<tokio::sync::oneshot::Receiver<()>>,
) {
    tasks.spawn(async move {
        #[cfg(test)]
        if let Some(gate) = gate {
            let _ = gate.await;
        }
        workspace::open(&paths).await
    });
}

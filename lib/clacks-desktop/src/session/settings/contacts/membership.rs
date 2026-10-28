//! Prepare selected working bases and report guarded membership changes.

use clacks_client_store::{
    logbooks::LogbookSnapshot,
    read,
    save::tags::{self, TagChangeProgress, TagChangeReport, TagChangeStop},
    store::ClientStore,
};
use clacks_model::ids::{QsoId, TagId};
use clacks_sync::{
    client::ClientRecord,
    record::{RecordId, RecordKey, RecordState, RecordValue},
};
use std::{collections::BTreeSet, ops::ControlFlow};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

/// Captured save bases and logbook catalog from one local snapshot.
#[derive(Debug)]
pub(crate) struct MembershipBasis {
    /// Complete live contact records in stable ID order.
    pub records: Vec<ClientRecord>,
    /// Catalog read alongside the contacts.
    pub catalog: LogbookSnapshot,
}

/// Frozen bulk operation owned independently of the popup's lifetime.
pub(crate) struct MembershipSubmission {
    /// Complete records the operator reviewed.
    pub basis: Vec<ClientRecord>,
    /// Explicit inclusion intent.
    pub add: BTreeSet<TagId>,
    /// Explicit removal intent.
    pub remove: BTreeSet<TagId>,
    /// Cancellation is observed only between saves.
    pub cancel: CancellationToken,
    /// Latest settled progress, independent of the final response.
    pub progress: watch::Sender<TagChangeProgress>,
}

/// Read all selected live contacts without truncating them to the table page.
pub(crate) async fn prepare(
    store: &ClientStore,
    ids: &BTreeSet<QsoId>,
) -> anyhow::Result<MembershipBasis> {
    anyhow::ensure!(
        !ids.is_empty(),
        "Select contacts before editing assigned logbooks"
    );
    let mut snapshot = read::snapshot(store).await?;
    let mut records = Vec::with_capacity(ids.len());
    for id in ids {
        let row = snapshot
            .record(RecordKey {
                account: store.account_id(),
                id: RecordId::Qso(*id),
            })
            .await?;
        match row {
            Some(row)
                if matches!(
                    row.working_state(),
                    Some(RecordState::Live(RecordValue::Qso(_)))
                ) =>
            {
                records.push(row)
            }
            _ => {
                snapshot.close().await?;
                anyhow::bail!(
                    "Selected contact {id} is no longer live or available; refresh the selection"
                );
            }
        }
    }
    let catalog = snapshot.settings().await?.logbooks;
    snapshot.close().await?;
    Ok(MembershipBasis { records, catalog })
}

/// Apply explicit membership intent while retaining every acknowledged save.
#[tracing::instrument(skip_all, name = "desktop.contacts.membership")]
pub(crate) async fn apply(
    store: &ClientStore,
    submission: MembershipSubmission,
) -> anyhow::Result<TagChangeReport> {
    let account_id = store.account_id();
    let mut final_progress = TagChangeProgress::default();
    let report = tags::change_tags(
        store,
        &submission.basis,
        &submission.add,
        &submission.remove,
        |progress| {
            final_progress = progress.clone();
            submission.progress.send_replace(progress.clone());
            if submission.cancel.is_cancelled() {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        },
    )
    .await?;
    let cancelled = matches!(report.stop, Some(TagChangeStop::Cancelled));
    let unprocessed = report.unprocessed.len();
    if let Some(TagChangeStop::Failed { id, error }) = &report.stop {
        tracing::error!(event = "membership.change.failed", account_id = %account_id, qso_id = %id, error = %error, changed = final_progress.changed, unchanged = final_progress.unchanged, stale = final_progress.stale, unprocessed);
    } else {
        tracing::info!(
            event = "membership.change.completed", account_id = %account_id,
            changed = final_progress.changed,
            unchanged = final_progress.unchanged,
            stale = final_progress.stale,
            unprocessed,
            cancelled
        );
    }
    Ok(report)
}

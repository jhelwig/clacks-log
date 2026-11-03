//! Atomic ordinary QSO save with any already available successful lookup.

use super::{SaveOutcome, report_saved, save_in};
use crate::{directory::snapshots, store::ClientStore};
use clacks_model::{contact::Qso, directory::values::DirectorySnapshot};
use clacks_sync::{
    client::ClientRecord,
    record::{RecordId, RecordKey, RecordState, RecordValue},
};

/// Save immediately; attach only an available success in the same guarded transaction.
///
/// A stale contact basis installs neither contact changes nor directory facts.
#[tracing::instrument(skip_all, name="client_store.contact.save", fields(account_id=%store.account))]
pub async fn save(
    store: &ClientStore,
    expected: Option<&ClientRecord>,
    next: Qso,
    snapshot: Option<DirectorySnapshot>,
) -> anyhow::Result<SaveOutcome> {
    let qso = next.id();
    let key = RecordKey {
        account: store.account,
        id: RecordId::Qso(qso),
    };
    let mut transaction = store.write().await?;
    let next = RecordState::Live(RecordValue::Qso(Box::new(next)));
    let mut outcome = save_in(&mut transaction, key, expected, next.clone()).await?;
    let mut acknowledged_retry = false;
    if expected.is_none()
        && let SaveOutcome::Stale {
            current: Some(current),
        } = &outcome
        && current
            .working_state()
            .is_some_and(|current| current.same_values(&next))
    {
        outcome = SaveOutcome::Unchanged(current.clone());
        acknowledged_retry = true;
    }
    let installed = if !matches!(outcome, SaveOutcome::Stale { .. }) {
        if let Some(snapshot) = snapshot {
            Some(snapshots::apply_in(&mut transaction, store.account, qso, snapshot).await?)
        } else {
            None
        }
    } else {
        None
    };
    transaction.commit().await?;
    if acknowledged_retry {
        tracing::info!(event="contact.create_retry_acknowledged", account_id=%store.account, record_id=%key.id);
    }
    report_saved(key, expected, &outcome);
    if installed == Some(snapshots::SnapshotOutcome::Applied) {
        tracing::info!(event="directory.snapshot.saved", account_id=%store.account, qso_id=%qso);
    }
    Ok(outcome)
}

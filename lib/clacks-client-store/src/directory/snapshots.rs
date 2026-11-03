//! Guarded contact-owned successful snapshot writes through ordinary client record roles.

use crate::{
    metadata::client,
    save::{self, SaveOutcome},
    store::ClientStore,
};
use clacks_model::{
    directory::{
        record::DirectoryRecord,
        values::{DirectorySnapshot, LookupKey},
    },
    ids::{AccountId, QsoId},
};
use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};
use sqlx::SqliteConnection;

/// Result of applying one real successful lookup to its owning saved contact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotOutcome {
    /// A changed complete winner committed.
    Applied,
    /// Existing winners already dominate or equal this lookup.
    Unchanged,
    /// Parent is missing/deleted or its current normalized callsign differs.
    Ineligible,
}

/// Install a successful lookup without changing any recorded contact values.
#[tracing::instrument(skip_all, name="client_store.directory.apply", fields(account_id=%store.account, qso_id=%qso))]
pub async fn apply(
    store: &ClientStore,
    qso: QsoId,
    snapshot: DirectorySnapshot,
) -> anyhow::Result<SnapshotOutcome> {
    let mut transaction = store.write().await?;
    let outcome = apply_in(&mut transaction, store.account, qso, snapshot).await?;
    transaction.commit().await?;
    if outcome == SnapshotOutcome::Applied {
        tracing::info!(event="directory.snapshot.saved", account_id=%store.account, qso_id=%qso);
    }
    Ok(outcome)
}

/// Share the exact parent/key guard with atomic contact-and-snapshot saving.
pub(crate) async fn apply_in(
    connection: &mut SqliteConnection,
    account: AccountId,
    qso: QsoId,
    snapshot: DirectorySnapshot,
) -> anyhow::Result<SnapshotOutcome> {
    let parent = client::load(
        connection,
        RecordKey {
            account,
            id: RecordId::Qso(qso),
        },
    )
    .await?;
    let Some(RecordState::Live(RecordValue::Qso(parent))) =
        parent.as_ref().and_then(|row| row.working_state())
    else {
        return Ok(SnapshotOutcome::Ineligible);
    };
    if LookupKey::from_recorded(&parent.contacted_station().callsign) != *snapshot.key() {
        return Ok(SnapshotOutcome::Ineligible);
    }
    let key = RecordKey {
        account,
        id: RecordId::Directory(qso),
    };
    let current = client::load(connection, key).await?;
    let mut directory = match current.as_ref().and_then(|row| row.working_state()) {
        Some(RecordState::Live(RecordValue::Directory(value))) => value.as_ref().clone(),
        None | Some(RecordState::Deleted) => DirectoryRecord::new(qso),
        _ => anyhow::bail!("Directory identity contains a different record family"),
    };
    if !directory.record(snapshot)? {
        return Ok(SnapshotOutcome::Unchanged);
    }
    match save::save_in(
        connection,
        key,
        current.as_ref(),
        RecordState::Live(RecordValue::Directory(Box::new(directory))),
    )
    .await?
    {
        SaveOutcome::Saved(_) => Ok(SnapshotOutcome::Applied),
        SaveOutcome::Unchanged(_) => Ok(SnapshotOutcome::Unchanged),
        SaveOutcome::Stale { .. } => {
            anyhow::bail!("Directory basis changed within its owning transaction")
        }
    }
}

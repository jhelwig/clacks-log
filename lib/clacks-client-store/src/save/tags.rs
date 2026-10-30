//! Guarded membership changes with record-specific partial results.

use super::{SaveOutcome, save};
use crate::store::ClientStore;
use clacks_model::{
    contact::Qso,
    ids::{QsoId, TagId},
};
use clacks_sync::{
    client::ClientRecord,
    record::{RecordId, RecordState, RecordValue},
};
use std::{collections::BTreeSet, ops::ControlFlow};

/// A settled guarded save for one selected contact.
#[derive(Debug)]
pub struct TagChangeEntry {
    /// Contact whose captured basis was compared.
    pub id: QsoId,
    /// Authoritative save result, including unchanged or stale state.
    pub outcome: SaveOutcome,
}

/// Settled membership work available while an operation is running.
#[derive(Clone, Debug, Default)]
pub struct TagChangeProgress {
    /// Number of captured targets.
    pub total: usize,
    /// Targets with a returned save outcome.
    pub completed: usize,
    /// Targets with an acknowledged committed change.
    pub changed: usize,
    /// Targets already containing the desired values.
    pub unchanged: usize,
    /// Targets whose complete captured basis no longer matches.
    pub stale: usize,
}

/// Reason processing stopped before every target settled.
#[derive(Debug)]
pub enum TagChangeStop {
    /// Cancellation observed between contact transactions.
    Cancelled,
    /// No successful acknowledgment was obtained for this target.
    Failed {
        /// Target whose outcome needs review before retrying.
        id: QsoId,
        /// Original operational error; a commit error does not prove rollback.
        error: anyhow::Error,
    },
}

/// Completed outcomes and the exact remainder of one membership operation.
#[derive(Debug)]
pub struct TagChangeReport {
    /// Saved, unchanged and stale targets in processing order.
    pub entries: Vec<TagChangeEntry>,
    /// Early stop, absent when every target settled.
    pub stop: Option<TagChangeStop>,
    /// Targets never attempted, excluding an operationally failing target.
    pub unprocessed: Vec<QsoId>,
}

/// Validated contact and its complete saved basis.
struct Target<'a> {
    /// Contact identity validated before any write.
    id: QsoId,
    /// Complete original save basis.
    record: &'a ClientRecord,
    /// Live working values from the same captured record.
    qso: &'a Qso,
}

/// Change only memberships against complete captured contact bases.
///
/// Inputs are validated before writing. Each target uses an ordinary guarded
/// transaction; completed saves survive cancellation or later failure. The
/// callback receives initial and settled progress and can stop before the next
/// transaction. Completion takes precedence over cancellation after the last save.
#[tracing::instrument(skip_all, name = "client_store.tags.change", fields(account_id = %store.account))]
pub async fn change_tags(
    store: &ClientStore,
    expected: &[ClientRecord],
    add: &BTreeSet<TagId>,
    remove: &BTreeSet<TagId>,
    mut progress: impl FnMut(&TagChangeProgress) -> ControlFlow<()>,
) -> anyhow::Result<TagChangeReport> {
    anyhow::ensure!(
        add.is_disjoint(remove),
        "a tag cannot be both added and removed"
    );
    let mut keys = BTreeSet::new();
    let mut targets = Vec::with_capacity(expected.len());
    for row in expected {
        let RecordId::Qso(id) = row.key.id else {
            anyhow::bail!("selected record is not a contact")
        };
        anyhow::ensure!(
            row.key.account == store.account && keys.insert(row.key),
            "selected contacts have duplicate or invalid keys"
        );
        let Some(RecordState::Live(RecordValue::Qso(qso))) = row.working_state() else {
            anyhow::bail!("selected contact has no live working value")
        };
        targets.push(Target {
            id,
            record: row,
            qso,
        });
    }
    let mut report = TagChangeReport {
        entries: vec![],
        stop: None,
        unprocessed: vec![],
    };
    let mut status = TagChangeProgress {
        total: targets.len(),
        ..Default::default()
    };
    let mut continuing = progress(&status);
    let mut remaining = targets.into_iter();
    while let Some(Target {
        id,
        record: row,
        qso,
    }) = remaining.next()
    {
        if continuing.is_break() {
            report.stop = Some(TagChangeStop::Cancelled);
            report.unprocessed.push(id);
            report.unprocessed.extend(remaining.map(|target| target.id));
            break;
        }
        let mut next = qso.clone().into_fields();
        next.tags.retain(|tag| !remove.contains(tag));
        next.tags.extend(add);
        let result = save(
            store,
            row.key,
            Some(row),
            RecordState::Live(RecordValue::Qso(Box::new(
                clacks_model::contact::Qso::try_from(next)?,
            ))),
        )
        .await;
        match result {
            Ok(outcome) => {
                status.completed += 1;
                match &outcome {
                    SaveOutcome::Saved(_) => status.changed += 1,
                    SaveOutcome::Unchanged(_) => status.unchanged += 1,
                    SaveOutcome::Stale { .. } => status.stale += 1,
                }
                report.entries.push(TagChangeEntry { id, outcome });
                continuing = progress(&status);
            }
            Err(error) => {
                report.stop = Some(TagChangeStop::Failed { id, error });
                report.unprocessed.extend(remaining.map(|target| target.id));
                break;
            }
        }
    }
    Ok(report)
}

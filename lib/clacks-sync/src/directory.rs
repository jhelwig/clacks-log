//! Concrete per-key directory value policy, independent of whole-QSO reconciliation.

use crate::{
    client::{ClientRecord, OriginalBasis, PendingRecord},
    record::{Observation, RecordState, RecordValue},
};
use clacks_model::directory::record::DirectoryRecord;

/// Combine complete successful winners with their original lookup time and identity.
pub fn combine(left: &DirectoryRecord, right: &DirectoryRecord) -> anyhow::Result<DirectoryRecord> {
    let mut combined = left.clone();
    combined.merge_from(right)?;
    Ok(combined)
}

/// Only new publication needs a live parent; already accepted history may remain inert.
pub(crate) fn reject_deleted_parent(
    state: &RecordState,
    input: &crate::acceptance::AcceptanceInput<'_>,
) -> Option<crate::acceptance::RejectionReason> {
    let RecordState::Live(RecordValue::Directory(value)) = state else {
        return None;
    };
    input.references.iter().any(|row| row.key().account == input.account.account && row.key().id == crate::record::RecordId::Qso(value.qso()) && matches!(row,Observation::Accepted(parent) if matches!(parent.state(),RecordState::Deleted)))
        .then_some(crate::acceptance::RejectionReason::DeletedDirectoryParent { qso: value.qso() })
}

/// Apply only the concrete directory policy; other record families keep ordinary rules.
pub fn combine_states(
    left: &RecordState,
    right: &RecordState,
) -> anyhow::Result<Option<RecordState>> {
    match (left, right) {
        (
            RecordState::Live(RecordValue::Directory(left)),
            RecordState::Live(RecordValue::Directory(right)),
        ) => Ok(Some(RecordState::Live(RecordValue::Directory(Box::new(
            combine(left, right)?,
        ))))),
        _ => Ok(None),
    }
}

/// Carry every known key winner into local working values without changing accepted versions.
pub(crate) fn retain_winners(next: &mut ClientRecord, prior: &ClientRecord) -> anyhow::Result<()> {
    let Some(RecordState::Live(RecordValue::Directory(_))) = next.working_state() else {
        return Ok(());
    };
    let mut combined = next
        .working_state()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Directory state missing"))?;
    for state in prior
        .working_state()
        .into_iter()
        .chain(match &prior.latest {
            Observation::Accepted(value) => Some(value.state()),
            _ => None,
        })
        .chain(match &next.latest {
            Observation::Accepted(value) => Some(value.state()),
            _ => None,
        })
        .chain(
            prior
                .retained
                .iter()
                .chain(&next.retained)
                .map(|value| value.state()),
        )
    {
        if let Some(merged) = combine_states(&combined, state)? {
            combined = merged;
        }
    }
    if let Some(pending) = &mut next.pending {
        pending.working = combined;
    } else if next.working_state() != Some(&combined) {
        let original = match &next.latest {
            Observation::Accepted(value) => OriginalBasis::Accepted(value.clone()),
            _ => return Ok(()),
        };
        next.pending = Some(PendingRecord {
            key: next.key,
            original,
            working: combined,
            prepared: None,
            witness: None,
        });
    }
    Ok(())
}

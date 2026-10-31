//! Bounded native installation with shared comparison and service dependency decisions.

use super::{InstallBatch, receipt::ReceiptState, satisfies};
use crate::{
    account::EvidenceView,
    client::{ClientRecord, ClientUpdate, receive_accepted},
    record::{AcceptedRecord, Observation, RecordKey, RecordState},
};

/// Plan a bounded installation from rows loaded in the same actual write transaction.
///
/// Atomic installation includes accepted values, client comparison evidence and
/// receipt progress. A local conflict is retained without blocking shared receipt.
#[tracing::instrument(
    skip_all,
    name = "sync.transfer.batch.plan",
    level = "debug",
    fields(
        account_id = %receipt.source.account,
    ),
)]
pub fn plan_batch(
    receipt: &ReceiptState,
    staged: &[AcceptedRecord],
    support: &[AcceptedRecord],
    incoming: &[AcceptedRecord],
    local: &[ClientRecord],
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<InstallBatch> {
    let mut candidates = staged.to_vec();
    for row in incoming {
        anyhow::ensure!(
            declared(receipt, row, evidence)?,
            "incoming version is not a declared requirement or qualifying successor"
        );
        if let Some(old) = candidates
            .iter()
            .find(|old| old.key == row.key && old.stamp == row.stamp)
        {
            anyhow::ensure!(old == row, "accepted context has contradictory payloads");
        } else {
            candidates.push(row.clone());
        }
    }
    for row in &candidates {
        if let Some(existing) = local.iter().find(|r| r.key == row.key)
            && let Observation::Accepted(old) = &existing.latest
        {
            crate::service::check_integrity(row.key, &old.state, &row.state)?;
        }
    }
    let heads: Vec<_> = local
        .iter()
        .filter_map(|row| match row.latest() {
            Observation::Accepted(value) => Some(value.clone()),
            _ => None,
        })
        .collect();
    let mut support = support.to_vec();
    support.extend(local.iter().flat_map(|row| row.retained().iter().cloned()));
    let readiness = super::readiness::plan_readiness(
        &super::readiness::ReadinessInput {
            account: receipt.source.account,
            candidates: &candidates,
            heads: &heads,
            support: &support,
            units: &receipt.units,
        },
        evidence,
    )?;
    let staged = readiness.staged;
    let mut completed_units = receipt.completed_units.clone();
    completed_units.extend(readiness.completed_units);
    let ready = readiness.ready;
    let mut accepted = Vec::new();
    let mut client_updates: std::collections::BTreeMap<RecordKey, ClientUpdate> =
        std::collections::BTreeMap::new();
    for row in &ready {
        anyhow::ensure!(
            declared(receipt, row, evidence)?,
            "ready version is not declared"
        );
        let update = if let Some(existing) = client_updates
            .get(&row.key)
            .map(|update| &update.record)
            .or_else(|| local.iter().find(|r| r.key == row.key))
        {
            receive_accepted(existing, row, evidence)?
        } else {
            ClientUpdate {
                record: ClientRecord {
                    retained: Vec::new(),
                    key: row.key,
                    latest: Observation::Accepted(row.clone()),
                    pending: None,
                },
                review: None,
            }
        };
        accepted.push(row.clone());
        client_updates.insert(row.key, update);
    }
    let plan = InstallBatch {
        accepted,
        client_updates: client_updates.into_values().collect(),
        staged,
        completed_units,
    };
    let readiness = if plan.staged.is_empty() {
        "ready"
    } else {
        "staged"
    };
    metrics::counter!("clacks_sync_transfer_batch_plans_total", "result" => readiness).increment(1);
    tracing::debug!(
        event = "transfer.batch.planned",
        readiness,
        input_count = incoming.len(),
        proposed_record_count = plan.accepted.len(),
        staged_record_count = plan.staged.len(),
        proposed_head_count = plan.client_updates.len()
    );
    Ok(plan)
}

/// Accept an exact obligation or a causally qualifying successor without arbitrary same-key selection.
fn declared(
    receipt: &ReceiptState,
    value: &AcceptedRecord,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<bool> {
    for required in receipt
        .records
        .iter()
        .filter(|required| required.key == value.key)
    {
        let exact_staging = required.stamp == value.stamp
            && required.deleted == matches!(value.state, RecordState::Deleted)
            && (crate::service::payload(value).is_some()
                || receipt
                    .units
                    .iter()
                    .any(|unit| unit.members.iter().any(|member| member.key == value.key)));
        if exact_staging || satisfies(required, value, evidence)? {
            return Ok(true);
        }
    }
    Ok(false)
}

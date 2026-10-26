//! Guarded local saved-state operations for one account workspace.

use crate::{metadata::client, store::ClientStore};
use clacks_sync::{client::ClientRecord, record::RecordKey};

/// Read a complete client record from one ordinary snapshot transaction.
///
/// A missing row means no locally saved record, not proof of shared absence.
#[tracing::instrument(skip_all, name = "client_store.record.load", level = "debug", fields(record_id = %key.id))]
pub async fn load(store: &ClientStore, key: RecordKey) -> anyhow::Result<Option<ClientRecord>> {
    anyhow::ensure!(
        key.account == store.account,
        crate::error::StoreError::WrongAccount {
            expected: store.account,
            actual: key.account
        }
    );
    let mut transaction = store.pool.begin().await?;
    let record = client::load(&mut transaction, key).await?;
    transaction.commit().await?;
    Ok(record)
}

/// Result of comparing an editor's basis and committing a local value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaveOutcome {
    /// A changed local working version was committed.
    Saved(ClientRecord),
    /// The existing working values already match; pending evidence remains intact.
    Unchanged(ClientRecord),
    /// Reload this current row before applying the editor's intent again.
    Stale {
        /// Current saved row, absent when the expected row no longer exists.
        current: Option<ClientRecord>,
    },
}

/// Apply one local edit to actual current roles inside the caller's immediate transaction.
pub(crate) async fn save_in(
    connection: &mut sqlx::SqliteConnection,
    key: RecordKey,
    expected: Option<&ClientRecord>,
    next: clacks_sync::record::RecordState,
) -> anyhow::Result<SaveOutcome> {
    use clacks_sync::{
        client::{OriginalBasis, PendingRecord},
        error::UnknownReason,
        record::Observation,
    };
    next.validate(key)?;
    let current = client::load(connection, key).await?;
    if current.as_ref() != expected {
        return Ok(SaveOutcome::Stale { current });
    }
    if let Some(current) = &current
        && current
            .working_state()
            .is_some_and(|state| state.same_values(&next))
    {
        return Ok(SaveOutcome::Unchanged(current.clone()));
    }
    let mut record = match current {
        Some(current) => current,
        None => {
            let latest = match crate::metadata::lineage::current_checkpoint(connection, key.account)
                .await?
            {
                Some(checkpoint) => Observation::Absent { key, checkpoint },
                None => Observation::Unavailable {
                    key,
                    reason: UnknownReason::MissingOriginal,
                },
            };
            ClientRecord {
                key,
                latest,
                pending: None,
                retained: vec![],
            }
        }
    };
    if let Some(pending) = &mut record.pending {
        pending.working = next;
    } else {
        let original = match &record.latest {
            Observation::Accepted(value) => OriginalBasis::Accepted(value.clone()),
            Observation::Absent { checkpoint, .. } => OriginalBasis::NeverCreated {
                key,
                checkpoint: checkpoint.clone(),
            },
            Observation::Unavailable { reason, .. } => OriginalBasis::Unavailable {
                key,
                reason: *reason,
            },
        };
        record.pending = Some(PendingRecord {
            key,
            original,
            working: next,
            prepared: None,
            witness: None,
        });
    }
    client::store(connection, &record).await?;
    Ok(SaveOutcome::Saved(record))
}

/// Save validated local values only when the complete editor basis still matches.
///
/// First edits retain their original evidence. Later edits change working values
/// without altering any frozen proposal or allocating a writer identity.
#[tracing::instrument(skip_all, name = "client_store.record.save", fields(account_id = %store.account, record_id = %key.id))]
pub async fn save(
    store: &ClientStore,
    key: RecordKey,
    expected: Option<&ClientRecord>,
    next: clacks_sync::record::RecordState,
) -> anyhow::Result<SaveOutcome> {
    use clacks_sync::record::{RecordId, RecordState};
    anyhow::ensure!(
        key.account == store.account,
        crate::error::StoreError::WrongAccount {
            expected: store.account,
            actual: key.account
        }
    );
    let mut transaction = store.write().await?;
    if matches!(next, RecordState::Deleted)
        && let RecordId::Equipment(id) = key.id
    {
        anyhow::ensure!(
            crate::settings::deletion::referring_profiles(&mut transaction, store.account, id)
                .await?
                .is_empty(),
            "equipment has referring profiles; use reviewed equipment deletion"
        );
    }
    let outcome = save_in(&mut transaction, key, expected, next).await?;
    transaction.commit().await?;
    if let SaveOutcome::Saved(record) = &outcome {
        if matches!(
            key.id,
            RecordId::Qso(_) | RecordId::Profile(_) | RecordId::Equipment(_) | RecordId::Tag(_)
        ) {
            let change = if matches!(record.working_state(), Some(RecordState::Deleted)) {
                "delete"
            } else if expected
                .and_then(ClientRecord::working_state)
                .is_some_and(|state| !matches!(state, RecordState::Deleted))
            {
                "update"
            } else {
                "create"
            };
            let kind = crate::payload::kind(key.id);
            tracing::info!(event = "record.saved", account_id = %key.account, record_id = %key.id, record_kind = kind, change);
            match key.id {
                RecordId::Tag(_) => {
                    metrics::counter!("clacks_client_tag_mutations_total", "change" => change)
                        .increment(1);
                }
                RecordId::Qso(_) => {
                    metrics::counter!("clacks_client_qso_mutations_total", "change" => change)
                        .increment(1)
                }
                RecordId::Profile(_) => {
                    metrics::counter!("clacks_client_profile_mutations_total", "change" => change)
                        .increment(1);
                }
                RecordId::Equipment(_) => {
                    metrics::counter!("clacks_client_equipment_mutations_total", "change" => change).increment(1);
                }
                _ => {}
            }
        } else {
            tracing::info!(event = "record.saved", account_id = %key.account, record_id = %key.id);
        }
    }
    Ok(outcome)
}

/// Add/remove membership on selected contact snapshots, preserving each completed record transaction.
#[tracing::instrument(skip_all, name = "client_store.tags.change", fields(account_id = %store.account))]
pub async fn change_tags(
    store: &ClientStore,
    expected: &[ClientRecord],
    add: &std::collections::BTreeSet<clacks_model::ids::TagId>,
    remove: &std::collections::BTreeSet<clacks_model::ids::TagId>,
) -> anyhow::Result<Vec<SaveOutcome>> {
    use anyhow::Context;
    use clacks_sync::record::{RecordId, RecordState, RecordValue};
    anyhow::ensure!(
        add.is_disjoint(remove),
        "a tag cannot be both added and removed"
    );
    let mut keys = std::collections::BTreeSet::new();
    for row in expected {
        anyhow::ensure!(
            row.key.account == store.account
                && matches!(row.key.id, RecordId::Qso(_))
                && keys.insert(row.key),
            "selected contacts have duplicate or invalid keys"
        );
        anyhow::ensure!(
            matches!(
                row.working_state(),
                Some(RecordState::Live(RecordValue::Qso(_)))
            ),
            "selected contact has no live working value"
        );
    }
    let mut outcomes = Vec::new();
    for row in expected {
        let Some(RecordState::Live(RecordValue::Qso(qso))) = row.working_state() else {
            anyhow::bail!("selected contact has no live working value")
        };
        let mut qso = qso.clone();
        qso.tags.retain(|tag| !remove.contains(tag));
        qso.tags.extend(add);
        let outcome = save(
            store,
            row.key,
            Some(row),
            RecordState::Live(RecordValue::Qso(qso)),
        )
        .await
        .with_context(|| {
            format!(
                "tag membership change stopped after {} completed contacts",
                outcomes.len()
            )
        })?;
        outcomes.push(outcome);
    }
    Ok(outcomes)
}

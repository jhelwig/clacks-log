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

/// Result of revalidating a complete tag-deletion preview.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TagDeleteOutcome {
    /// The tag and its classified contact effects committed together.
    Applied(clacks_model::tag::TagDeletionPlan),
    /// The tag or complete membership basis changed; no effects were committed.
    Stale {
        /// Current tag row for refreshing the preview.
        current: Option<Box<ClientRecord>>,
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
    let outcome = save_in(&mut transaction, key, expected, next).await?;
    transaction.commit().await?;
    if let SaveOutcome::Saved(record) = &outcome {
        tracing::info!(event = "record.saved", account_id = %key.account, record_id = %key.id);
        if matches!(key.id, RecordId::Qso(_)) {
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
            metrics::counter!("clacks_client_qso_mutations_total", "change" => change).increment(1);
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

/// Recheck the entire preview before atomically deleting a tag and applying its contact effects.
#[tracing::instrument(skip_all, name = "client_store.tag.delete", fields(account_id = %store.account, deletion_choice = ?choice))]
pub async fn delete_tag(
    store: &ClientStore,
    expected_tag: &ClientRecord,
    expected_members: &[ClientRecord],
    choice: clacks_model::tag::TagDeletionChoice,
) -> anyhow::Result<TagDeleteOutcome> {
    use clacks_model::tag::plan_tag_deletion;
    use clacks_sync::record::{RecordId, RecordState, RecordValue};
    anyhow::ensure!(
        expected_tag.key.account == store.account,
        crate::error::StoreError::WrongAccount {
            expected: store.account,
            actual: expected_tag.key.account
        }
    );
    let RecordId::Tag(tag) = expected_tag.key.id else {
        anyhow::bail!("tag deletion requires a tag key")
    };
    let mut transaction = store.write().await?;
    let current = client::load(&mut transaction, expected_tag.key).await?;
    if current.as_ref() != Some(expected_tag) {
        return Ok(TagDeleteOutcome::Stale {
            current: current.map(Box::new),
        });
    }
    anyhow::ensure!(
        matches!(
            expected_tag.working_state(),
            Some(RecordState::Live(RecordValue::Tag(_)))
        ),
        "tag has no live working value"
    );
    let account = store.account.to_string();
    let tag_text = tag.to_string();
    let mut members = std::collections::BTreeMap::new();
    for row in sqlx::query_file!("sql/save/members.sql", account, tag_text)
        .fetch_all(&mut *transaction)
        .await?
    {
        let key = RecordKey {
            account: store.account,
            id: RecordId::Qso(row.record_id.parse()?),
        };
        let value = client::load(&mut transaction, key)
            .await?
            .ok_or_else(|| anyhow::anyhow!("tag member lost its client row"))?;
        members.insert(key, value);
    }
    let mut expected = std::collections::BTreeMap::new();
    for row in expected_members {
        anyhow::ensure!(
            row.key.account == store.account && expected.insert(row.key, row.clone()).is_none(),
            "invalid or duplicate deletion-preview member"
        );
    }
    if members != expected {
        return Ok(TagDeleteOutcome::Stale {
            current: current.map(Box::new),
        });
    }
    let mut qsos = Vec::new();
    for row in members.values() {
        let Some(RecordState::Live(RecordValue::Qso(qso))) = row.working_state() else {
            anyhow::bail!("tag member has no live QSO")
        };
        qsos.push((**qso).clone());
    }
    let plan = plan_tag_deletion(tag, choice, &qsos);
    let mut deleted = 0;
    let mut updated = 0;
    for row in members.values() {
        let RecordId::Qso(id) = row.key.id else {
            anyhow::bail!("invalid tag member kind")
        };
        let next = if plan.mark_deleted.contains(&id) {
            RecordState::Deleted
        } else {
            let Some(RecordState::Live(RecordValue::Qso(qso))) = row.working_state() else {
                anyhow::bail!("tag member has no live QSO")
            };
            let mut qso = qso.clone();
            qso.tags.remove(&tag);
            RecordState::Live(RecordValue::Qso(qso))
        };
        match save_in(&mut transaction, row.key, Some(row), next).await? {
            SaveOutcome::Saved(_) => {
                if plan.mark_deleted.contains(&id) {
                    deleted += 1;
                } else {
                    updated += 1;
                }
            }
            SaveOutcome::Unchanged(_) => {}
            SaveOutcome::Stale { .. } => {
                anyhow::bail!("tag deletion member changed within its transaction")
            }
        }
    }
    anyhow::ensure!(
        !matches!(
            save_in(
                &mut transaction,
                expected_tag.key,
                Some(expected_tag),
                RecordState::Deleted
            )
            .await?,
            SaveOutcome::Stale { .. }
        ),
        "tag changed within deletion transaction"
    );
    transaction.commit().await?;
    if deleted > 0 {
        metrics::counter!("clacks_client_qso_mutations_total", "change" => "delete")
            .increment(deleted);
    }
    if updated > 0 {
        metrics::counter!("clacks_client_qso_mutations_total", "change" => "update")
            .increment(updated);
    }
    tracing::info!(event = "tag.deleted", account_id = %store.account, tag_id = %tag, deleted_contacts = deleted, changed_memberships = updated);
    Ok(TagDeleteOutcome::Applied(plan))
}

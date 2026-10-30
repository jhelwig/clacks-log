//! Guarded local saved-state operations for one account workspace.

pub mod tags;

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
        Some(current) => current.into_fields(),
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
            clacks_sync::client::ClientRecordFields {
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
    let record: ClientRecord = record.try_into()?;
    client::store(connection, &record).await?;
    if crate::metadata::review::load(connection, key)
        .await?
        .is_some_and(|review| {
            matches!(
                review.reason,
                clacks_sync::client::ReviewReason::ExcludedActor(_)
            )
        })
    {
        crate::enrollment::holds::review_in(connection, &record).await?;
    }
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

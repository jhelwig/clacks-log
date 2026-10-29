//! Transactional adapters for native preparation, receipt and explicit resolution.

use crate::{
    identity,
    metadata::{self, client, lineage, review},
    save::SaveOutcome,
    store::ClientStore,
};
use clacks_sync::{
    acceptance::{self, RejectionReason},
    account::AccountMetadata,
    client::{ClientRecord, ClientUpdate, Preparation, ReviewInput},
    error::{MetadataError, UnknownReason},
    membership::MembershipSnapshot,
    record::{AcceptedRecord, Observation, RecordKey, RecordState},
};
use sqlx::SqliteConnection;

/// Distinguish a local-only workspace from a native preparation result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrepareOutcome {
    /// No authorized actor is enrolled; no counter was allocated.
    NotEnrolled,
    /// Old pending work requires comparison before it can use a replacement writer.
    AuthorizationReview {
        /// Former writer whose work remains frozen and unsubmitted.
        former_actor: clacks_model::ids::ClientActorId,
    },
    /// Native outcome whose returned state has committed before it is exposed.
    Core(Box<Preparation>),
}

/// Require the owning account before accessing any record-qualified state.
pub(crate) fn check_account(store: &ClientStore, key: RecordKey) -> anyhow::Result<()> {
    anyhow::ensure!(
        key.account == store.account,
        crate::error::StoreError::WrongAccount {
            expected: store.account,
            actual: key.account
        }
    );
    Ok(())
}

/// Load actual evidence; an unenrolled or not-yet-seeded workspace has no fabricated lineage.
pub(crate) async fn evidence(
    connection: &mut SqliteConnection,
    account: clacks_model::ids::AccountId,
) -> anyhow::Result<lineage::Interpretation> {
    lineage::interpretation(connection, account)
        .await?
        .ok_or_else(|| {
            MetadataError::Incomplete {
                detail: "no received account lineage is installed".into(),
            }
            .into()
        })
}

/// Read a local row or qualify its absence only with an actual completed checkpoint.
pub(crate) async fn current(
    connection: &mut SqliteConnection,
    key: RecordKey,
) -> anyhow::Result<ClientRecord> {
    if let Some(value) = client::load(connection, key).await? {
        return Ok(value);
    }
    let latest = match lineage::current_checkpoint(connection, key.account).await? {
        Some(checkpoint) => Observation::Absent { key, checkpoint },
        None => Observation::Unavailable {
            key,
            reason: UnknownReason::MissingOriginal,
        },
    };
    Ok(ClientRecord {
        key,
        latest,
        pending: None,
        retained: vec![],
    })
}

/// Persist a core update and its review together without clearing an unchanged rejection review.
pub(crate) async fn store_update(
    connection: &mut SqliteConnection,
    previous: &ClientRecord,
    update: &mut ClientUpdate,
) -> anyhow::Result<()> {
    crate::enrollment::holds::preserve_in(connection, previous, update).await?;
    client::store(connection, &update.record).await?;
    if update.review.is_some() || update.record != *previous {
        review::store(connection, update.record.key, update.review.as_ref()).await?;
    }
    Ok(())
}

/// Freeze current working values and reserve their writer counter atomically before transmission.
///
/// Retries reuse the committed proposal. Mandatory reference conditions are read
/// from shared observations in this transaction, never from structural identity rows.
#[tracing::instrument(skip_all,name="client_store.prepare",level="debug",fields(account_id=%store.account,record_id=%key.id))]
pub async fn prepare(store: &ClientStore, key: RecordKey) -> anyhow::Result<PrepareOutcome> {
    check_account(store, key)?;
    let mut transaction = store.write().await?;
    let identity = identity::load(&mut transaction)
        .await?
        .ok_or_else(|| anyhow::anyhow!("workspace identity is missing"))?;
    let Some(binding) = identity.binding else {
        return Ok(PrepareOutcome::NotEnrolled);
    };
    if let Some(former_actor) = crate::enrollment::holds::actor_in(&mut transaction, key).await? {
        let record = current(&mut transaction, key).await?;
        if let Some(input) = review::load(&mut transaction, key).await?
            && input.reason == clacks_sync::client::ReviewReason::ExcludedActor(former_actor)
        {
            transaction.commit().await?;
            return Ok(PrepareOutcome::Core(Box::new(Preparation::Review {
                record,
                input,
            })));
        }
        return Ok(PrepareOutcome::AuthorizationReview { former_actor });
    }
    let context = evidence(&mut transaction, store.account).await?;
    let row = current(&mut transaction, key).await?;
    let counter = client::counter(&mut transaction, key, binding.actor).await?;
    let mut dependencies = vec![];
    if let Some(pending) = &row.pending
        && pending.prepared.is_none()
    {
        for key in acceptance::required_references(key, &pending.working) {
            let dependency = current(&mut transaction, key).await?;
            match &dependency.latest {
                Observation::Accepted(value) if !matches!(value.state, RecordState::Deleted) => {
                    dependencies.push(clacks_sync::client::VersionCondition::from_observation(
                        &dependency.latest,
                    )?)
                }
                Observation::Unavailable { .. } => {
                    return Err(MetadataError::Incomplete {
                        detail: format!("shared reference evidence unavailable for {key:?}"),
                    }
                    .into());
                }
                _ => {
                    let mut update = clacks_sync::client::receive_rejection(
                        &row,
                        &RejectionReason::MissingReference { key },
                    )?;
                    store_update(&mut transaction, &row, &mut update).await?;
                    let input = update.review.ok_or_else(|| {
                        anyhow::anyhow!("missing-reference transition did not produce review")
                    })?;
                    transaction.commit().await?;
                    return Ok(PrepareOutcome::Core(Box::new(Preparation::Review {
                        record: update.record,
                        input,
                    })));
                }
            }
        }
    }
    let result = clacks_sync::client::prepare_change(
        &row,
        binding.actor,
        &counter,
        &dependencies,
        &context.view(),
    )?;
    match &result {
        Preparation::Ready {
            record, counter, ..
        } => {
            client::store(&mut transaction, record).await?;
            client::store_counter(&mut transaction, key, counter).await?;
            review::store(&mut transaction, key, None).await?;
        }
        Preparation::Review { record, input } => {
            client::store(&mut transaction, record).await?;
            review::store(&mut transaction, key, Some(input)).await?;
        }
        Preparation::Unchanged { record } => {
            client::store(&mut transaction, record).await?;
            review::store(&mut transaction, key, None).await?;
        }
        Preparation::Retry { record, .. } => {
            client::store(&mut transaction, record).await?;
        }
    }
    transaction.commit().await?;
    Ok(PrepareOutcome::Core(Box::new(result)))
}

/// Incorporate one genuine accepted response without advancing the completed account receipt.
#[tracing::instrument(skip_all,name="client_store.receive",level="debug",fields(account_id=%store.account,record_id=%incoming.key.id))]
pub async fn receive(
    store: &ClientStore,
    incoming: &AcceptedRecord,
) -> anyhow::Result<ClientUpdate> {
    check_account(store, incoming.key)?;
    let mut transaction = store.write().await?;
    let context = evidence(&mut transaction, store.account).await?;
    let row = current(&mut transaction, incoming.key).await?;
    let mut update = clacks_sync::client::receive_accepted(&row, incoming, &context.view())?;
    store_update(&mut transaction, &row, &mut update).await?;
    if let Some(review) =
        crate::enrollment::holds::review_in(&mut transaction, &update.record).await?
    {
        update.review = Some(review);
    }
    metadata::binding::remove(&mut transaction, incoming.key).await?;
    metadata::binding::insert(&mut transaction, incoming.key).await?;
    if matches!(incoming.key.id, clacks_sync::record::RecordId::Service(_)) {
        metadata::binding::validate_namespaces(&mut transaction, store.account).await?;
    }
    transaction.commit().await?;
    Ok(update)
}

/// Save an actual admission rejection with all original and frozen work intact.
#[tracing::instrument(skip_all,name="client_store.reject",level="debug",fields(account_id=%store.account,record_id=%key.id))]
pub async fn reject(
    store: &ClientStore,
    key: RecordKey,
    reason: &RejectionReason,
) -> anyhow::Result<ClientUpdate> {
    check_account(store, key)?;
    let mut transaction = store.write().await?;
    let row = current(&mut transaction, key).await?;
    let mut update = clacks_sync::client::receive_rejection(&row, reason)?;
    store_update(&mut transaction, &row, &mut update).await?;
    transaction.commit().await?;
    Ok(update)
}

/// Commit an explicit resolution only while both the record and shown review still match.
#[tracing::instrument(skip_all,name="client_store.resolve",fields(account_id=%store.account,record_id=%expected.key.id))]
pub async fn resolve(
    store: &ClientStore,
    expected: &ClientRecord,
    shown: &ReviewInput,
    chosen: RecordState,
) -> anyhow::Result<SaveOutcome> {
    check_account(store, expected.key)?;
    let mut transaction = store.write().await?;
    let row = client::load(&mut transaction, expected.key).await?;
    let saved = review::load(&mut transaction, expected.key).await?;
    if row.as_ref() != Some(expected)
        || saved.as_ref() != Some(shown)
        || expected.working_state() != Some(&shown.local)
        || expected.latest != shown.shared
        || expected.retained != shown.retained
        || expected.pending.as_ref().map(|pending| &pending.original) != Some(&shown.original)
    {
        return Ok(SaveOutcome::Stale { current: row });
    }
    let context = evidence(&mut transaction, store.account).await?;
    let record = clacks_sync::client::resolve_review(shown, chosen, &context.view())?;
    client::store(&mut transaction, &record).await?;
    review::store(&mut transaction, expected.key, None).await?;
    if let clacks_sync::client::ReviewReason::ExcludedActor(former) = shown.reason {
        crate::enrollment::holds::clear_in(&mut transaction, expected.key, former).await?;
    }
    transaction.commit().await?;
    tracing::info!(event="record.resolved",account_id=%store.account,record_id=%expected.key.id,review_reason=?shown.reason);
    Ok(SaveOutcome::Saved(record))
}

/// Persist an authenticated coordinator's observed account view under its exact prior basis.
///
/// Authentication and rollback decisions belong to the caller. This saves supplied
/// metadata for reconnection and diagnostics; it does not certify local receipt.
#[tracing::instrument(skip_all,name="client_store.account.observe",level="debug",fields(account_id=%store.account))]
pub async fn observe_account(
    store: &ClientStore,
    expected: Option<&(AccountMetadata, MembershipSnapshot)>,
    next: &AccountMetadata,
    membership: &MembershipSnapshot,
) -> anyhow::Result<bool> {
    anyhow::ensure!(
        next.account == store.account,
        "account view belongs to another account"
    );
    let mut transaction = store.write().await?;
    if metadata::account::load(&mut transaction, store.account)
        .await?
        .as_ref()
        != expected
    {
        return Ok(false);
    }
    metadata::account::store(&mut transaction, next, membership).await?;
    transaction.commit().await?;
    Ok(true)
}

#[cfg(test)]
mod tests;

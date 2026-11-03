//! Transactional adapters for native preparation, receipt and explicit resolution.

use crate::{
    identity,
    metadata::{self, client, lineage, review},
    save::SaveOutcome,
    store::ClientStore,
};
use clacks_sync::{
    acceptance::{self, RejectionReason},
    client::{ClientRecord, ClientUpdate, Preparation, ReviewInput},
    error::{MetadataError, UnknownReason},
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
    clacks_sync::client::ClientRecordFields {
        key,
        latest,
        pending: None,
        retained: vec![],
    }
    .try_into()
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
        review::store(connection, update.record.key(), update.review.as_ref()).await?;
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
    if let Some(pending) = &row.pending()
        && pending.prepared.is_none()
    {
        for key in acceptance::required_references(key, &pending.working) {
            let dependency = current(&mut transaction, key).await?;
            match &dependency.latest() {
                Observation::Accepted(value)
                    if matches!(value.state(), RecordState::Deleted)
                        && matches!(row.key().id, clacks_sync::record::RecordId::Directory(_)) =>
                {
                    let clacks_sync::record::RecordId::Qso(qso) = key.id else {
                        anyhow::bail!("Directory parent has the wrong record kind")
                    };
                    let mut update = clacks_sync::client::receive_rejection(
                        &row,
                        &RejectionReason::DeletedDirectoryParent { qso },
                    )?;
                    store_update(&mut transaction, &row, &mut update).await?;
                    transaction.commit().await?;
                    return Ok(PrepareOutcome::Core(Box::new(Preparation::Unchanged {
                        record: update.record,
                    })));
                }
                Observation::Accepted(value) if !matches!(value.state(), RecordState::Deleted) => {
                    dependencies.push(clacks_sync::client::VersionCondition::from_observation(
                        dependency.latest(),
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
    let result = clacks_sync::client::preparation::prepare_change(
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
#[tracing::instrument(skip_all,name="client_store.receive",level="debug",fields(account_id=%store.account,record_id=%incoming.key().id))]
pub async fn receive(
    store: &ClientStore,
    incoming: &AcceptedRecord,
) -> anyhow::Result<ClientUpdate> {
    check_account(store, incoming.key())?;
    let mut transaction = store.write().await?;
    let context = evidence(&mut transaction, store.account).await?;
    let row = current(&mut transaction, incoming.key()).await?;
    let mut update = clacks_sync::client::receive_accepted(&row, incoming, &context.view())?;
    store_update(&mut transaction, &row, &mut update).await?;
    if let Some(review) =
        crate::enrollment::holds::review_in(&mut transaction, &update.record).await?
    {
        update.review = Some(review);
    }
    metadata::binding::remove(&mut transaction, incoming.key()).await?;
    metadata::binding::insert(&mut transaction, incoming.key()).await?;
    if matches!(incoming.key().id, clacks_sync::record::RecordId::Service(_)) {
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
#[tracing::instrument(skip_all,name="client_store.resolve",fields(account_id=%store.account,record_id=%expected.key().id))]
pub async fn resolve(
    store: &ClientStore,
    expected: &ClientRecord,
    shown: &ReviewInput,
    chosen: RecordState,
) -> anyhow::Result<SaveOutcome> {
    check_account(store, expected.key())?;
    let mut transaction = store.write().await?;
    if !matches_review(&mut transaction, expected, shown).await? {
        return Ok(SaveOutcome::Stale {
            current: client::load(&mut transaction, expected.key()).await?,
        });
    }
    if matches!(chosen, RecordState::Deleted)
        && matches!(
            expected.key().id,
            clacks_sync::record::RecordId::Tag(_) | clacks_sync::record::RecordId::Equipment(_)
        )
    {
        let references =
            crate::read::review::references::read_in(&mut transaction, expected.key(), None, 1)
                .await?;
        anyhow::ensure!(
            references.total == 0,
            "Deletion is blocked by {} accepted referring records",
            references.total
        );
    }
    let record = resolve_in(&mut transaction, store.account, expected, shown, chosen).await?;
    transaction.commit().await?;
    tracing::info!(event="record.resolved",account_id=%store.account,record_id=%expected.key().id,review_reason=?shown.reason);
    Ok(SaveOutcome::Saved(record))
}

/// Recheck the exact persisted editor roles inside the admitted write transaction.
async fn matches_review(
    connection: &mut sqlx::SqliteConnection,
    expected: &ClientRecord,
    shown: &ReviewInput,
) -> anyhow::Result<bool> {
    let row = client::load(connection, expected.key()).await?;
    let saved = review::load(connection, expected.key()).await?;
    Ok(row.as_ref() == Some(expected)
        && saved.as_ref() == Some(shown)
        && expected.working_state() == Some(&shown.local)
        && expected.latest() == &shown.shared
        && expected.retained() == shown.retained
        && expected.pending().map(|pending| &pending.original) == Some(&shown.original))
}

/// Apply the existing core resolution while retaining the caller's transaction.
async fn resolve_in(
    connection: &mut sqlx::SqliteConnection,
    account: clacks_model::ids::AccountId,
    expected: &ClientRecord,
    shown: &ReviewInput,
    chosen: RecordState,
) -> anyhow::Result<ClientRecord> {
    let context = evidence(connection, account).await?;
    for reference in acceptance::required_references(expected.key(), &chosen) {
        let target = client::load(connection, reference).await?;
        anyhow::ensure!(
            target
                .as_ref()
                .and_then(ClientRecord::working_state)
                .is_some_and(|state| matches!(state, RecordState::Live(_))),
            "Cannot save resolution: referenced record {} is unavailable or deleted",
            reference.id
        );
    }
    let record = clacks_sync::client::preparation::resolve_review(shown, chosen, &context.view())?;

    client::store(connection, &record).await?;
    review::store(connection, expected.key(), None).await?;
    if let clacks_sync::client::ReviewReason::ExcludedActor(former) = shown.reason {
        crate::enrollment::holds::clear_in(connection, expected.key(), former).await?;
    }
    Ok(record)
}

#[cfg(test)]
mod tests;

/// Durable synchronization knowledge used for negotiation and pending publication.
pub struct SyncState {
    /// Actually completed receipt, absent before full initial synchronization.
    pub checkpoint: Option<clacks_sync::account::InstalledCheckpoint>,
    /// Retained immutable interpretations, never synthesized from record maxima.
    pub baselines: Vec<clacks_sync::account::Baseline>,
    /// Genuine account ancestry, absent before any source interpretation arrives.
    pub lineage: Option<clacks_sync::lineage::Lineage>,
    /// Actual records with pending local values, independent of current history filters.
    pub pending: Vec<crate::read::review::ReviewRow>,
}

/// Read complete coverage, retained meanings and pending identities from one local snapshot.
pub async fn state(store: &ClientStore) -> anyhow::Result<SyncState> {
    let mut transaction = store.pool.begin().await?;
    let checkpoint = lineage::current_checkpoint(&mut transaction, store.account).await?;
    let interpretation = lineage::interpretation(&mut transaction, store.account).await?;
    let mut pending = Vec::new();
    for row in sqlx::query_file!("sql/retention/pending.sql")
        .fetch_all(&mut *transaction)
        .await?
    {
        let key = RecordKey {
            account: row.account.parse()?,
            id: crate::payload::record_id(&row.kind, &row.record_id)?,
        };
        check_account(store, key)?;
        if let Some(record) = client::load(&mut transaction, key).await? {
            pending.push(crate::read::review::ReviewRow {
                record,
                review: review::load(&mut transaction, key).await?,
                authorization_hold: crate::enrollment::holds::actor_in(&mut transaction, key)
                    .await?,
            });
        }
    }
    let (baselines, ancestry) = match interpretation {
        Some(value) => (value.baselines, Some(value.lineage)),
        None => (vec![], None),
    };
    transaction.commit().await?;
    Ok(SyncState {
        checkpoint,
        baselines,
        lineage: ancestry,
        pending,
    })
}

/// Apply an explicit server stale response atomically with release of its matching frozen proposal.
pub async fn stale(
    store: &ClientStore,
    change: &clacks_sync::client::PreparedChange,
    observed: &Observation,
) -> anyhow::Result<ClientUpdate> {
    check_account(store, change.key())?;
    check_account(store, observed.key())?;
    let mut transaction = store.write().await?;
    let evidence = evidence(&mut transaction, store.account).await?;
    if observed.key() != change.key()
        && let Observation::Accepted(value) = observed
    {
        let previous = current(&mut transaction, value.key()).await?;
        let mut update = clacks_sync::client::receive_accepted(&previous, value, &evidence.view())?;
        store_update(&mut transaction, &previous, &mut update).await?;
        metadata::binding::remove(&mut transaction, value.key()).await?;
        metadata::binding::insert(&mut transaction, value.key()).await?;
    }
    let previous = current(&mut transaction, change.key()).await?;
    let mut update =
        clacks_sync::client::receive_stale(&previous, change, observed, &evidence.view())?;
    store_update(&mut transaction, &previous, &mut update).await?;
    transaction.commit().await?;
    Ok(update)
}

/// Persist a hub's returned recovery comparison while retaining current local edits.
#[tracing::instrument(skip_all, name="client_store.recovery.review", fields(account_id=%store.account))]
pub async fn receive_review(
    store: &ClientStore,
    input: &ReviewInput,
) -> anyhow::Result<ClientUpdate> {
    let key = input.shared.key();
    check_account(store, key)?;
    let mut transaction = store.write().await?;
    let previous = current(&mut transaction, key).await?;
    let mut update = clacks_sync::client::returned::receive_review(&previous, input)?;
    store_update(&mut transaction, &previous, &mut update).await?;
    metadata::binding::remove(&mut transaction, key).await?;
    metadata::binding::insert(&mut transaction, key).await?;
    transaction.commit().await?;
    tracing::info!(event="sync.recovery.review_committed", account_id=%store.account, record_id=%key.id, reason=?input.reason);
    Ok(update)
}

/// Result of explicitly retrying a deletion against current accepted references.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RetryDeletionOutcome {
    /// Reference repair permits normal pending publication again.
    Ready {
        /// Rebased local deletion, with no counter reserved by the retry.
        record: Box<ClientRecord>,
    },
    /// Shared references remain, even if local working repairs already exist.
    StillReferenced {
        /// Bounded current referrers and their full count.
        page: crate::read::review::references::ReferencePage,
    },
    /// Compared target or review changed; no transition was applied.
    Stale {
        /// Current roles and review for an explicit refreshed comparison.
        current: Option<Box<crate::read::review::ReviewRow>>,
    },
}

/// Retry a blocked deletion only after its accepted referrers have been repaired.
#[tracing::instrument(skip_all, name = "client_store.deletion.retry", fields(record_id = %expected.key().id))]
pub async fn retry_deletion(
    store: &ClientStore,
    expected: &ClientRecord,
    shown: &ReviewInput,
) -> anyhow::Result<RetryDeletionOutcome> {
    check_account(store, expected.key())?;
    let mut transaction = store.write().await?;
    if !matches_review(&mut transaction, expected, shown).await? {
        let current = match client::load(&mut transaction, expected.key()).await? {
            Some(record) => Some(Box::new(crate::read::review::ReviewRow {
                record,
                review: review::load(&mut transaction, expected.key()).await?,
                authorization_hold: crate::enrollment::holds::actor_in(
                    &mut transaction,
                    expected.key(),
                )
                .await?,
            })),
            None => None,
        };
        return Ok(RetryDeletionOutcome::Stale { current });
    }
    anyhow::ensure!(
        matches!(
            shown.reason,
            clacks_sync::client::ReviewReason::Referenced { .. }
        ) && matches!(shown.local, RecordState::Deleted),
        "Only a blocked local deletion can retry"
    );
    let page =
        crate::read::review::references::read_in(&mut transaction, expected.key(), None, 100)
            .await?;
    if page.total > 0 {
        return Ok(RetryDeletionOutcome::StillReferenced { page });
    }
    let record = resolve_in(
        &mut transaction,
        store.account,
        expected,
        shown,
        RecordState::Deleted,
    )
    .await?;
    transaction.commit().await?;
    tracing::info!(event = "deletion.retry_ready", record_id = %expected.key().id);
    Ok(RetryDeletionOutcome::Ready {
        record: Box::new(record),
    })
}

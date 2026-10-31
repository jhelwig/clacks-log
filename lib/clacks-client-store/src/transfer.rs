//! Transactional receipt of accepted state without persistent exchange bookkeeping.

#[cfg(test)]
mod tests;

use crate::{
    metadata::{binding, client, lineage, units},
    store::ClientStore,
    synchronization,
};
use clacks_sync::{
    account::InstalledCheckpoint,
    record::{AcceptedRecord, Observation},
    service::VersionReference,
    transfer::{
        InstallBatch, WindowMode,
        discovery::{DiscoveryEnd, DiscoveryPage, DiscoveryStart},
        receipt::{ReceiptState, finish_receipt, register_page},
    },
};
use sqlx::SqliteConnection;

/// Load current records and exact support required by one bounded batch.
async fn load_local(
    connection: &mut SqliteConnection,
    store: &ClientStore,
    receipt: &ReceiptState,
    staged: &[AcceptedRecord],
    support: &mut Vec<AcceptedRecord>,
    incoming: &[AcceptedRecord],
) -> anyhow::Result<Vec<clacks_sync::client::ClientRecord>> {
    let mut local = std::collections::BTreeMap::new();
    let keys = incoming
        .iter()
        .chain(staged)
        .map(|value| value.key())
        .chain(
            receipt
                .units
                .iter()
                .filter(|unit| !receipt.completed_units.contains(&unit.id))
                .flat_map(|unit| unit.members.iter().map(|value| value.key)),
        )
        .collect::<std::collections::BTreeSet<_>>();
    for key in keys {
        if let Some(row) = client::load(connection, key).await? {
            local.insert(row.key(), row);
        }
    }
    let mut queue: Vec<_> = incoming.iter().chain(staged).cloned().collect();
    queue.extend(local.values().filter_map(|row| match &row.latest() {
        Observation::Accepted(value) => Some(value.clone()),
        _ => None,
    }));
    while let Some(value) = queue.pop() {
        let mut keys = clacks_sync::acceptance::required_references(value.key(), value.state());
        if let clacks_sync::record::RecordState::Live(clacks_sync::record::RecordValue::Service(
            service,
        )) = value.state()
        {
            keys.extend(clacks_sync::service::dependency::required_keys(
                store.account,
                service,
            ));
            if service.support.destination.is_none()
                && !matches!(
                    service.payload,
                    clacks_model::service::evidence::ServicePayload::Destination(_)
                )
                && let Some(id) = service.payload.destination()
            {
                keys.push(clacks_sync::record::RecordKey {
                    account: store.account,
                    id: clacks_sync::record::RecordId::Service(
                        clacks_model::service::evidence::ServiceRecordId::Destination(id),
                    ),
                });
            }
            for reference in service
                .support
                .required
                .iter()
                .map(|value| value.reference())
            {
                keys.push(reference.key);
                if !support
                    .iter()
                    .any(|value| value.key() == reference.key && value.stamp == reference.stamp)
                    && let Some(fact) = client::observation::exact(connection, reference).await?
                {
                    queue.push(fact.clone());
                    support.push(fact);
                }
            }
        }
        for key in keys {
            if let std::collections::btree_map::Entry::Vacant(entry) = local.entry(key)
                && let Some(row) = client::load(connection, key).await?
            {
                if let Observation::Accepted(value) = &row.latest() {
                    queue.push(value.clone());
                }
                entry.insert(row);
            }
        }
    }
    Ok(local.into_values().collect())
}

/// Persist native row changes and receipt progress in the caller's transaction.
async fn apply_batch(
    connection: &mut SqliteConnection,
    store: &ClientStore,
    batch: &mut InstallBatch,
) -> anyhow::Result<u64> {
    for update in &batch.client_updates {
        binding::remove(connection, update.record.key()).await?;
    }
    let mut installed_count = 0_u64;
    for update in &mut batch.client_updates {
        let previous = synchronization::current(connection, update.record.key()).await?;
        if previous.latest() != update.record.latest() {
            installed_count += 1;
        }
        synchronization::store_update(connection, &previous, update).await?;
        if let Some(review) =
            crate::enrollment::holds::review_in(connection, &update.record).await?
        {
            update.review = Some(review);
        }
    }
    for update in &batch.client_updates {
        binding::insert(connection, update.record.key()).await?;
    }
    if batch.client_updates.iter().any(|update| {
        matches!(
            update.record.key().id,
            clacks_sync::record::RecordId::Service(_)
        )
    }) {
        binding::validate_namespaces(connection, store.account).await?;
    }
    for value in &batch.accepted {
        let envelope = client::observation::insert_envelope(connection, value).await?;
        sqlx::query_file!(
            "sql/receipt/unstage.sql",
            store.account.to_string(),
            envelope
        )
        .execute(&mut *connection)
        .await?;
    }
    for value in &batch.staged {
        let envelope = client::observation::insert_envelope(connection, value).await?;
        sqlx::query_file!(
            "sql/receipt/stage.sql",
            store.account.to_string(),
            crate::payload::kind(value.key().id),
            value.key().id.to_string(),
            envelope
        )
        .execute(&mut *connection)
        .await?;
    }

    Ok(installed_count)
}

/// Begin a connection-owned receipt without changing completed coverage.
#[tracing::instrument(skip_all, name="client_store.receipt.begin", level="debug", fields(account_id=%store.account))]
pub async fn begin(store: &ClientStore, start: &DiscoveryStart) -> anyhow::Result<ReceiptState> {
    anyhow::ensure!(
        start.source.account == store.account,
        "receipt crosses account"
    );
    let receipt = ReceiptState::begin(start)?;
    let mut transaction = store.write().await?;
    if let Some(checkpoint) = lineage::current_checkpoint(&mut transaction, store.account).await? {
        crate::retention::seal_pending(&mut transaction, &checkpoint).await?;
    }
    transaction.commit().await?;
    Ok(receipt)
}

/// Committed page changes and the next connection-owned receipt state.
struct Installation {
    /// Receipt to expose only after the caller commits.
    receipt: ReceiptState,
    /// Number of changed current accepted observations.
    installed_count: u64,
    /// Newly staged accepted inputs.
    staged_count: u64,
}

/// Apply actual input and reconsider relevant independently staged values.
async fn install(
    connection: &mut SqliteConnection,
    store: &ClientStore,
    receipt: &ReceiptState,
    incoming: &[AcceptedRecord],
    supplied_support: &[AcceptedRecord],
) -> anyhow::Result<Installation> {
    let mut staged = Vec::new();
    for row in sqlx::query_file!("sql/receipt/staged.sql", store.account.to_string())
        .fetch_all(&mut *connection)
        .await?
    {
        let key = clacks_sync::record::RecordKey {
            account: store.account,
            id: crate::payload::record_id(&row.kind, &row.record_id)?,
        };
        let value = client::observation::load_envelope(connection, key, row.envelope_id).await?;
        if receipt.records.iter().any(|required| {
            required.key == key
                && required.stamp == value.stamp
                && required.deleted
                    == matches!(value.state(), clacks_sync::record::RecordState::Deleted)
        }) && !supplied_support.contains(&value)
        {
            staged.push(value);
        }
    }
    let mut support = supplied_support.to_vec();
    let local = load_local(connection, store, receipt, &staged, &mut support, incoming).await?;
    let context = synchronization::evidence(connection, store.account).await?;
    let mut batch = clacks_sync::transfer::batch::plan_batch(
        receipt,
        &staged,
        &support,
        incoming,
        &local,
        &context.view(),
    )?;
    for value in supplied_support {
        if !batch.accepted.contains(value) {
            batch.accepted.push(value.clone());
        }
    }
    let staged_count = u64::try_from(
        batch
            .staged
            .iter()
            .filter(|value| !staged.contains(value))
            .count(),
    )?;
    let installed_count = apply_batch(connection, store, &mut batch).await?;
    let mut next = receipt.clone();
    next.staged = batch.staged.iter().map(VersionReference::from).collect();
    next.completed_units = batch.completed_units;
    Ok(Installation {
        receipt: next,
        installed_count,
        staged_count,
    })
}

/// Commit a page's data before advancing this connection's receipt and allowing acknowledgment.
#[tracing::instrument(skip_all, name="client_store.receipt.install", level="debug", fields(account_id=%store.account, page=page.sequence))]
pub async fn install_page(
    store: &ClientStore,
    receipt: &mut ReceiptState,
    page: &DiscoveryPage,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        receipt.source.account == store.account,
        "receipt crosses account"
    );
    let next = register_page(receipt, page)?;
    let mut transaction = store.write().await?;
    for baseline in &page.baselines {
        lineage::insert_baseline(&mut transaction, baseline).await?;
    }
    for descriptor in &page.units {
        units::remember(&mut transaction, store.account, descriptor).await?;
    }
    if let Some(baseline) = next
        .baselines
        .iter()
        .find(|baseline| baseline.id == next.source.baseline)
    {
        let mut ancestry = baseline.lineage.clone();
        for boundary in &next.boundaries {
            ancestry.add_boundary(boundary.clone())?;
        }
        crate::retention::merge_lineage(&mut transaction, &ancestry).await?;
    }
    let installed = install(
        &mut transaction,
        store,
        &next,
        &page.accepted,
        &page.support,
    )
    .await?;
    transaction.commit().await?;
    *receipt = installed.receipt;
    if installed.installed_count > 0 {
        metrics::counter!("clacks_client_sync_records_installed_total")
            .increment(installed.installed_count);
    }
    if installed.staged_count > 0 {
        metrics::counter!("clacks_client_sync_records_staged_total")
            .increment(installed.staged_count);
    }
    Ok(())
}

/// Verify source exhaustion and actual stored evidence, then commit complete coverage.
#[tracing::instrument(skip_all, name="client_store.receipt.complete", fields(account_id=%store.account))]
pub async fn complete(
    store: &ClientStore,
    receipt: &ReceiptState,
    end: &DiscoveryEnd,
) -> anyhow::Result<InstalledCheckpoint> {
    anyhow::ensure!(
        receipt.source.account == store.account,
        "receipt crosses account"
    );
    let mut transaction = store.write().await?;
    let applied = install(&mut transaction, store, receipt, &[], &[]).await?;
    let receipt = &applied.receipt;
    let interpretation = synchronization::evidence(&mut transaction, store.account).await?;
    let previous = lineage::current_checkpoint(&mut transaction, store.account).await?;
    let mut installed = Vec::new();
    for requirement in &receipt.records {
        if let Some(row) = client::load(&mut transaction, requirement.key).await? {
            if let Observation::Accepted(value) = row.latest() {
                installed.push(value.clone());
            }
            installed.extend(row.retained().iter().cloned());
        }
        let reference = VersionReference {
            key: requirement.key,
            stamp: requirement.stamp.clone(),
            deleted: requirement.deleted,
        };
        if let Some(value) = client::observation::exact(&mut transaction, &reference).await? {
            installed.push(value);
        }
    }
    let next = finish_receipt(
        receipt,
        end,
        &installed,
        previous.as_ref(),
        &interpretation.view(),
    )?;
    lineage::set_checkpoint(&mut transaction, &next).await?;
    if matches!(receipt.mode, WindowMode::Full) {
        for row in sqlx::query_file!("sql/retention/pending.sql")
            .fetch_all(&mut *transaction)
            .await?
        {
            let key = clacks_sync::record::RecordKey {
                account: row.account.parse()?,
                id: crate::payload::record_id(&row.kind, &row.record_id)?,
            };
            if receipt.records.iter().any(|value| value.key == key) {
                continue;
            }
            if let Some(record) = client::load(&mut transaction, key).await? {
                let mut fields = record.into_fields();
                if matches!(fields.latest, Observation::Unavailable { .. }) {
                    fields.latest = Observation::Absent {
                        key,
                        checkpoint: next.clone(),
                    };
                    if let Some(pending) = &mut fields.pending
                        && matches!(
                            pending.original,
                            clacks_sync::client::OriginalBasis::NeverCreated {
                                checkpoint: None,
                                ..
                            }
                        )
                    {
                        pending.original = clacks_sync::client::OriginalBasis::NeverCreated {
                            key,
                            checkpoint: Some(next.clone()),
                        };
                    }
                    client::store(&mut transaction, &fields.try_into()?).await?;
                }
            }
        }
    }
    crate::enrollment::holds::completed_in(
        &mut transaction,
        &next,
        matches!(receipt.mode, WindowMode::Full),
    )
    .await?;
    transaction.commit().await?;
    if applied.installed_count > 0 {
        metrics::counter!("clacks_client_sync_records_installed_total")
            .increment(applied.installed_count);
    }
    if applied.staged_count > 0 {
        metrics::counter!("clacks_client_sync_records_staged_total")
            .increment(applied.staged_count);
    }
    tracing::info!(event="sync.receipt.completed", account_id=%store.account, pages=receipt.next_page);
    Ok(next)
}

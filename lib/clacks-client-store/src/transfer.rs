//! Install synchronization batches and commit completed account checkpoints.

use crate::{
    metadata::{binding, client, lineage, transfer},
    store::ClientStore,
    synchronization,
};
use clacks_sync::{
    account::InstalledCheckpoint,
    record::{AcceptedRecord, Observation},
    transfer::{InstallBatch, WindowManifest, WindowMode, WindowProgress},
};
use sqlx::SqliteConnection;

/// A remembered window and its durable progress from one transaction.
struct StoredWindow {
    /// Local identity of the immutable manifest.
    id: i64,
    /// Requirements and source interpretation for the window.
    manifest: WindowManifest,
    /// Already installed and staged contents.
    progress: WindowProgress,
}

/// Read the immutable manifest and its progress from one existing transaction.
async fn load(
    connection: &mut SqliteConnection,
    source: &InstalledCheckpoint,
    mode: &WindowMode,
) -> anyhow::Result<StoredWindow> {
    let id = transfer::find(connection, source, mode)
        .await?
        .ok_or_else(|| anyhow::anyhow!("immutable window has not been remembered"))?;
    Ok(StoredWindow {
        id,
        manifest: transfer::load_manifest(connection, source.account, id).await?,
        progress: transfer::progress(connection, source.account, id).await?,
    })
}

/// Store genuine supplied baseline and ancestry definitions without certifying receipt.
///
/// A manifest may carry several retained baselines. Its source baseline supplies
/// the current ancestry, which must contain every required window boundary.
#[tracing::instrument(skip_all,name="client_store.window.remember",level="debug",fields(account_id=%store.account))]
pub async fn remember(store: &ClientStore, manifest: &WindowManifest) -> anyhow::Result<()> {
    anyhow::ensure!(
        manifest.account == store.account,
        "window belongs to another account"
    );
    let mut transaction = store.write().await?;
    if let Some(checkpoint) = lineage::current_checkpoint(&mut transaction, store.account).await? {
        crate::retention::seal_pending(&mut transaction, &checkpoint).await?;
    }
    transfer::remember(&mut transaction, manifest).await?;
    // A source baseline is real server-supplied interpretation, not a synthesized root.
    if let Some(baseline) = manifest
        .baselines
        .iter()
        .find(|value| value.id == manifest.source.baseline)
    {
        let mut lineage = baseline.lineage.clone();
        for boundary in &manifest.boundaries {
            lineage.add_boundary(boundary.clone())?;
        }
        crate::retention::merge_lineage(&mut transaction, &lineage).await?;
    }
    transaction.commit().await?;
    Ok(())
}

/// Read committed staging and installation progress for an immutable source/mode.
pub async fn progress(
    store: &ClientStore,
    source: &InstalledCheckpoint,
    mode: &WindowMode,
) -> anyhow::Result<WindowProgress> {
    anyhow::ensure!(
        source.account == store.account,
        "window belongs to another account"
    );
    let mut transaction = store.pool.begin().await?;
    let StoredWindow { progress, .. } = load(&mut transaction, source, mode).await?;
    transaction.commit().await?;
    Ok(progress)
}

/// Plan and commit a bounded incoming batch against actual current client rows.
///
/// Incomplete atomic units remain staged. Current ownership, working/original
/// updates, review and progress commit together; no receipt advances here.
#[tracing::instrument(skip_all,name="client_store.window.install",level="debug",fields(account_id=%store.account))]
pub async fn install(
    store: &ClientStore,
    source: &InstalledCheckpoint,
    mode: &WindowMode,
    incoming: &[AcceptedRecord],
) -> anyhow::Result<InstallBatch> {
    anyhow::ensure!(
        source.account == store.account,
        "window belongs to another account"
    );
    let mut transaction = store.write().await?;
    let StoredWindow {
        id,
        manifest,
        progress,
    } = load(&mut transaction, source, mode).await?;
    let context = synchronization::evidence(&mut transaction, store.account).await?;
    let mut local = std::collections::BTreeMap::new();
    for requirement in &manifest.records {
        if let Some(row) = client::load(&mut transaction, requirement.key).await? {
            local.insert(row.key(), row);
        }
    }
    let mut queue: Vec<_> = incoming.iter().chain(&progress.staged).cloned().collect();
    queue.extend(local.values().filter_map(|row| match &row.latest() {
        Observation::Accepted(value) => Some(value.clone()),
        _ => None,
    }));
    while let Some(value) = queue.pop() {
        let clacks_sync::record::RecordState::Live(clacks_sync::record::RecordValue::Service(
            service,
        )) = &value.state()
        else {
            continue;
        };
        // Every supplied version can introduce different dependencies. Only
        // database loads are deduplicated; an older local version must not hide
        // support referenced by an incoming or previously staged version.
        let destination = if service.support.destination.is_none()
            && !matches!(
                service.payload,
                clacks_model::service::evidence::ServicePayload::Destination(_)
            ) {
            service
                .payload
                .destination()
                .map(|id| clacks_sync::record::RecordKey {
                    account: store.account,
                    id: clacks_sync::record::RecordId::Service(
                        clacks_model::service::evidence::ServiceRecordId::Destination(id),
                    ),
                })
        } else {
            None
        };
        let keys = clacks_sync::service::dependency::required_keys(store.account, service)
            .into_iter()
            .chain(
                service
                    .support
                    .required
                    .iter()
                    .map(|item| item.reference().key),
            )
            .chain(destination);
        for key in keys {
            if let std::collections::btree_map::Entry::Vacant(entry) = local.entry(key)
                && let Some(row) = client::load(&mut transaction, key).await?
            {
                if let Observation::Accepted(value) = &row.latest() {
                    queue.push(value.clone());
                }
                entry.insert(row);
            }
        }
    }
    let local: Vec<_> = local.into_values().collect();
    let mut batch =
        clacks_sync::transfer::plan_batch(&manifest, &progress, incoming, &local, &context.view())?;
    let staged_count = u64::try_from(
        batch
            .staged
            .iter()
            .filter(|value| !progress.staged.contains(value))
            .count(),
    )?;
    for update in &batch.client_updates {
        binding::remove(&mut transaction, update.record.key()).await?;
    }
    let mut installed_count = 0_u64;
    for update in &mut batch.client_updates {
        let previous = synchronization::current(&mut transaction, update.record.key()).await?;
        if previous.latest() != update.record.latest() {
            installed_count += 1;
        }
        synchronization::store_update(&mut transaction, &previous, update).await?;
        if let Some(review) =
            crate::enrollment::holds::review_in(&mut transaction, &update.record).await?
        {
            update.review = Some(review);
        }
    }
    for update in &batch.client_updates {
        binding::insert(&mut transaction, update.record.key()).await?;
    }
    if batch.client_updates.iter().any(|update| {
        matches!(
            update.record.key().id,
            clacks_sync::record::RecordId::Service(_)
        )
    }) {
        binding::validate_namespaces(&mut transaction, store.account).await?;
    }
    transfer::store_batch(&mut transaction, store.account, id, &batch).await?;
    transaction.commit().await?;
    if installed_count > 0 {
        metrics::counter!("clacks_client_sync_records_installed_total").increment(installed_count);
    }
    if staged_count > 0 {
        metrics::counter!("clacks_client_sync_records_staged_total").increment(staged_count);
    }
    Ok(batch)
}

/// Verify every requirement and atomically install the completed account checkpoint.
///
/// The returned value is reloaded after commit. Retrying after a lost acknowledgment
/// returns durable state; neither an advertised head nor a single response is receipt.
#[tracing::instrument(skip_all,name="client_store.window.finish",fields(account_id=%store.account))]
pub async fn finish(
    store: &ClientStore,
    source: &InstalledCheckpoint,
    mode: &WindowMode,
) -> anyhow::Result<InstalledCheckpoint> {
    anyhow::ensure!(
        source.account == store.account,
        "window belongs to another account"
    );
    let mut transaction = store.write().await?;
    let StoredWindow {
        manifest, progress, ..
    } = load(&mut transaction, source, mode).await?;
    let context = synchronization::evidence(&mut transaction, store.account).await?;
    let previous = lineage::current_checkpoint(&mut transaction, store.account).await?;
    let mut installed = vec![];
    for requirement in &manifest.records {
        if let Some(row) = client::load(&mut transaction, requirement.key).await?
            && let Observation::Accepted(value) = row.latest()
        {
            installed.push(value.clone());
        }
    }
    let next = match previous {
        Some(previous) => {
            clacks_sync::transfer::finish_window(
                &manifest,
                &progress,
                &installed,
                &previous,
                &context.view(),
            )?
            .next
        }
        None => clacks_sync::transfer::finish_initial_window(
            &manifest,
            &progress,
            &installed,
            &context.view(),
        )?,
    };
    lineage::set_checkpoint(&mut transaction, &next).await?;
    crate::enrollment::holds::completed_in(
        &mut transaction,
        &next,
        matches!(mode, WindowMode::Full),
    )
    .await?;
    transaction.commit().await?;
    let mut transaction = store.pool.begin().await?;
    let checkpoint = lineage::current_checkpoint(&mut transaction, store.account)
        .await?
        .ok_or_else(|| anyhow::anyhow!("completed checkpoint disappeared"))?;
    transaction.commit().await?;
    tracing::info!(event="sync.window.completed",account_id=%store.account);
    Ok(checkpoint)
}

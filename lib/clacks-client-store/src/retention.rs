//! Received interpretation, retained pending comparison evidence and local relational cleanup.

use crate::{
    metadata::{client, lineage},
    store::ClientStore,
};
use clacks_sync::{
    account::{Baseline, InstalledCheckpoint},
    lineage::Lineage,
    record::{Observation, RecordKey},
};
use sqlx::SqliteConnection;

/// Incorporate received ancestry without forgetting previously learned immutable boundaries.
pub(crate) async fn merge_lineage(
    connection: &mut SqliteConnection,
    received: &Lineage,
) -> anyhow::Result<()> {
    let mut merged = match lineage::interpretation(connection, received.account()).await? {
        Some(current) => {
            anyhow::ensure!(
                current.lineage.initial_timeline() == received.initial_timeline(),
                "received ancestry has a different initial timeline"
            );
            current.lineage
        }
        None => received.clone(),
    };
    for boundary in received.boundaries() {
        merged.add_boundary(boundary.clone())?;
    }
    lineage::set_lineage(connection, &merged).await?;
    Ok(())
}

/// Retain self-contained pending bases before changing received comparison interpretation.
///
/// Insufficient coverage retains the original and all its interpretation roots.
/// Other integrity/scope failures abort the transaction instead of discarding evidence.
pub(crate) async fn seal_pending(
    connection: &mut SqliteConnection,
    checkpoint: &InstalledCheckpoint,
) -> anyhow::Result<()> {
    let Some(context) = lineage::interpretation(connection, checkpoint.account).await? else {
        return Ok(());
    };
    for key in sqlx::query_file!("sql/retention/pending.sql")
        .fetch_all(&mut *connection)
        .await?
    {
        let key = RecordKey {
            account: key.account.parse()?,
            id: crate::payload::record_id(&key.kind, &key.record_id)?,
        };
        let mut row = client::load(connection, key)
            .await?
            .ok_or_else(|| anyhow::anyhow!("pending row disappeared"))?;
        let Some(pending) = &mut row.pending else {
            continue;
        };
        if pending.witness.is_some() {
            continue;
        }
        let Observation::Accepted(observed) = &row.latest else {
            continue;
        };
        match clacks_sync::compaction::seal_basis(pending, observed, checkpoint, &context.view()) {
            Ok(witness) => pending.witness = Some(witness),
            Err(error)
                if matches!(
                    error.downcast_ref::<clacks_sync::error::MetadataError>(),
                    Some(clacks_sync::error::MetadataError::Incomplete { .. })
                ) =>
            {
                continue;
            }
            Err(error) => return Err(error),
        }
        client::store(connection, &row).await?;
    }
    Ok(())
}

/// Install supplied baseline meanings and ancestry under the exact completed-receipt basis.
///
/// Pending bases are sealed while old evidence is readable. This does not advance
/// receipt, select a server compaction boundary or rewrite accepted stamps.
#[tracing::instrument(skip_all,name="client_store.interpretation.install",fields(account_id=%store.account))]
pub async fn install_interpretation(
    store: &ClientStore,
    expected: &InstalledCheckpoint,
    baselines: &[Baseline],
    received: &Lineage,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        expected.account == store.account && received.account() == store.account,
        "interpretation belongs to another account"
    );
    let mut transaction = store.write().await?;
    anyhow::ensure!(
        lineage::current_checkpoint(&mut transaction, store.account)
            .await?
            .as_ref()
            == Some(expected),
        "installed checkpoint changed before interpretation installation"
    );
    seal_pending(&mut transaction, expected).await?;
    for baseline in baselines {
        anyhow::ensure!(
            baseline.account == store.account,
            "baseline belongs to another account"
        );
        lineage::insert_baseline(&mut transaction, baseline).await?;
    }
    merge_lineage(&mut transaction, received).await?;
    transaction.commit().await?;
    Ok(())
}

/// Reclaim unreferenced local role and payload rows without deleting synchronized identities.
///
/// Immutable service facts and required support are retained for MVP recovery. Current,
/// original, frozen, review, staged and loaded-default roots remain reachable.
/// The returned count covers removed payload roots, not synchronization history or actors.
#[tracing::instrument(skip_all,name="client_store.collect",level="debug",fields(account_id=%store.account))]
pub async fn collect_unreferenced(store: &ClientStore) -> anyhow::Result<u64> {
    let mut transaction = store.write().await?;
    sqlx::query_file!("sql/retention/pending_garbage.sql")
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/retention/prepared_garbage.sql")
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/retention/witness_garbage.sql")
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/retention/original_garbage.sql")
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/retention/condition_garbage.sql")
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/retention/observation_garbage.sql")
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/retention/envelope_garbage.sql")
        .execute(&mut *transaction)
        .await?;
    let removed = sqlx::query_file!("sql/retention/payload_garbage.sql")
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    sqlx::query_file!("sql/retention/operating_garbage.sql")
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/retention/station_garbage.sql")
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/retention/equipment_garbage.sql")
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/retention/stamp_garbage.sql")
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/retention/vector_garbage.sql")
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/retention/checkpoint_garbage.sql")
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/retention/baseline_garbage.sql")
        .execute(&mut *transaction)
        .await?;

    sqlx::query_file!("sql/retention/lineage_garbage.sql")
        .execute(&mut *transaction)
        .await?;
    sqlx::query_file!("sql/retention/coverage_garbage.sql")
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    if removed > 0 {
        tracing::debug!(event="storage.garbage_collected",payload_count=removed,account_id=%store.account);
    }
    Ok(removed)
}

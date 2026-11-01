//! Bounded cleanup of obsolete local roles and their unreachable relational copies.

use crate::store::ClientStore;

/// Bound one cycle's physical cleanup without delaying the desktop for an unbounded backlog.
const BATCH_SIZE: i64 = 1_000;

/// Recompute role and support roots inside one ordinary immediate write transaction.
pub(super) async fn batch(store: &ClientStore) -> anyhow::Result<bool> {
    let mut transaction = store.write().await?;

    if crate::metadata::lineage::current_checkpoint(&mut transaction, store.account)
        .await?
        .is_none()
    {
        return Ok(false);
    }

    let account = store.account.to_string();
    let mut remaining = BATCH_SIZE;

    let pending = sqlx::query_file!("sql/retention/delete_pending.sql", account, remaining)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    remaining -= i64::try_from(pending)?;
    let prepared = sqlx::query_file!("sql/retention/delete_prepared.sql", account, remaining)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    remaining -= i64::try_from(prepared)?;
    let witnesses = sqlx::query_file!("sql/retention/delete_witnesses.sql", account, remaining)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    remaining -= i64::try_from(witnesses)?;
    let originals = sqlx::query_file!("sql/retention/delete_originals.sql", account, remaining)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    remaining -= i64::try_from(originals)?;
    let observations =
        sqlx::query_file!("sql/retention/delete_observations.sql", account, remaining)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
    remaining -= i64::try_from(observations)?;
    let conditions = sqlx::query_file!("sql/retention/delete_conditions.sql", account, remaining)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    remaining -= i64::try_from(conditions)?;

    let operating = sqlx::query_file!("sql/retention/delete_operating.sql", account, remaining)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    remaining -= i64::try_from(operating)?;

    let roots = super::roots::load(&mut transaction, store.account).await?;
    let bodies = serde_json::to_string(&roots.envelopes)?;
    let envelopes = sqlx::query_file!(
        "sql/retention/delete_envelopes.sql",
        account,
        bodies,
        remaining
    )
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    remaining -= i64::try_from(envelopes)?;
    let payloads = sqlx::query_file!("sql/retention/delete_payloads.sql", account, remaining)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    remaining -= i64::try_from(payloads)?;

    let stations = sqlx::query_file!("sql/retention/delete_stations.sql", account, remaining)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    remaining -= i64::try_from(stations)?;
    let equipment = sqlx::query_file!("sql/retention/delete_equipment.sql", account, remaining)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    remaining -= i64::try_from(equipment)?;
    let stamps = sqlx::query_file!("sql/retention/delete_stamps.sql", account, remaining)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    remaining -= i64::try_from(stamps)?;
    let vectors = sqlx::query_file!("sql/retention/delete_vectors.sql", account, remaining)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    remaining -= i64::try_from(vectors)?;

    let checkpoints = sqlx::query_file!("sql/retention/delete_checkpoints.sql", account, remaining)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    remaining -= i64::try_from(checkpoints)?;
    let baselines = sqlx::query_file!("sql/retention/delete_baselines.sql", account, remaining)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    remaining -= i64::try_from(baselines)?;
    let lineages = sqlx::query_file!("sql/retention/delete_lineages.sql", account, remaining)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    remaining -= i64::try_from(lineages)?;
    let coverage = sqlx::query_file!("sql/retention/delete_coverage.sql", account, remaining)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    remaining -= i64::try_from(coverage)?;
    let counters = sqlx::query_file!("sql/retention/delete_counters.sql", account, remaining)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    remaining -= i64::try_from(counters)?;

    transaction.commit().await?;

    metrics::counter!("clacks_client_sync_pending_copies_removed_total").increment(pending);
    metrics::counter!("clacks_client_sync_prepared_copies_removed_total").increment(prepared);
    metrics::counter!("clacks_client_sync_witness_copies_removed_total").increment(witnesses);
    metrics::counter!("clacks_client_sync_original_copies_removed_total").increment(originals);
    metrics::counter!("clacks_client_sync_observation_copies_removed_total")
        .increment(observations);
    metrics::counter!("clacks_client_sync_condition_copies_removed_total").increment(conditions);
    metrics::counter!("clacks_client_sync_operating_copies_removed_total").increment(operating);
    metrics::counter!("clacks_client_sync_envelopes_removed_total").increment(envelopes);
    metrics::counter!("clacks_client_sync_payloads_removed_total").increment(payloads);
    metrics::counter!("clacks_client_sync_station_snapshots_removed_total").increment(stations);
    metrics::counter!("clacks_client_sync_equipment_snapshots_removed_total").increment(equipment);
    metrics::counter!("clacks_client_sync_stamps_removed_total").increment(stamps);
    metrics::counter!("clacks_client_sync_vectors_removed_total").increment(vectors);
    metrics::counter!("clacks_client_sync_checkpoints_removed_total").increment(checkpoints);
    metrics::counter!("clacks_client_sync_baselines_removed_total").increment(baselines);
    metrics::counter!("clacks_client_sync_lineages_removed_total").increment(lineages);
    metrics::counter!("clacks_client_sync_coverage_rows_removed_total").increment(coverage);
    metrics::counter!("clacks_client_sync_retired_counters_removed_total").increment(counters);
    tracing::debug!(
        event = "sync.local_metadata.reclaimed",
        account_id = %store.account,
        pending,
        prepared,
        witnesses,
        originals,
        observations,
        conditions,
        operating,
        envelopes,
        payloads,
        stations,
        equipment,
        stamps,
        vectors,
        checkpoints,
        baselines,
        lineages,
        coverage,
        counters
    );

    Ok(remaining < BATCH_SIZE)
}

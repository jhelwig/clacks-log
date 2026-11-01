//! Bounded relational reclamation from freshly proved account roots.

use crate::store::AccountStore;
use clacks_model::ids::AccountId;

/// Result of one account's bounded physical-reclamation attempt.
pub(super) enum Outcome {
    /// Unreachable rows were committed; another pass may expose further rows.
    Reclaimed,
    /// No unreachable rows were removed.
    Unchanged,
    /// Another account writer holds the lock; no reclamation was attempted.
    Contended,
}

/// Remove only unneeded representations, reporting whether another pass may make progress.
#[tracing::instrument(skip_all, name="server.sync.metadata.reclaim", level="debug", fields(account_id=%owner))]
pub(super) async fn reclaim(store: &AccountStore, owner: AccountId) -> anyhow::Result<Outcome> {
    let mut transaction = store.pool.begin().await?;

    if sqlx::query_file!("sql/retention/lock.sql", owner.to_string())
        .fetch_optional(&mut *transaction)
        .await?
        .is_none()
    {
        return Ok(Outcome::Contended);
    }

    let roots = super::roots::load(&mut transaction, owner).await?;
    let account = owner.to_string();
    let mut remaining = super::BATCH_SIZE;

    let envelopes = sqlx::query_file!(
        "sql/retention/delete_envelopes.sql",
        account,
        &roots.envelopes,
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

    let stamps = sqlx::query_file!(
        "sql/retention/delete_stamps.sql",
        account,
        &roots.stamps,
        remaining
    )
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

    transaction.commit().await?;

    metrics::counter!("clacks_sync_envelopes_removed_total").increment(envelopes);
    metrics::counter!("clacks_sync_payloads_removed_total").increment(payloads);
    metrics::counter!("clacks_sync_station_snapshots_removed_total").increment(stations);
    metrics::counter!("clacks_sync_equipment_snapshots_removed_total").increment(equipment);
    metrics::counter!("clacks_sync_stamps_removed_total").increment(stamps);
    metrics::counter!("clacks_sync_vectors_removed_total").increment(vectors);
    metrics::counter!("clacks_sync_checkpoints_removed_total").increment(checkpoints);
    metrics::counter!("clacks_sync_baselines_removed_total").increment(baselines);
    metrics::counter!("clacks_sync_lineages_removed_total").increment(lineages);
    metrics::counter!("clacks_sync_coverage_rows_removed_total").increment(coverage);
    tracing::debug!(
        event = "sync.metadata.reclaimed",
        account_id = %owner,
        envelopes,
        payloads,
        stations,
        equipment,
        stamps,
        vectors,
        checkpoints,
        baselines,
        lineages,
        coverage
    );

    Ok(if remaining < super::BATCH_SIZE {
        Outcome::Reclaimed
    } else {
        Outcome::Unchanged
    })
}

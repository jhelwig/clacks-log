//! Bounded prefix pruning with atomic publication of the lost incremental range.

use crate::store::{AccountStore, metadata::lineage};
use clacks_model::ids::AccountId;
use clacks_sync::{lineage::Coverage, record::Position};

/// Result of one account's bounded journal-pruning attempt.
pub(super) enum Outcome {
    /// Excess entries were committed; another batch may still be needed.
    Pruned {
        /// The removed prefix filled the batch limit; excess entries may remain.
        more: bool,
    },
    /// The journal already satisfies its retention target.
    Unchanged,
    /// Another account writer holds the lock; no pruning was attempted.
    Contended,
}

/// Remove one excess prefix and publish its floor together; report possible remaining batches.
#[tracing::instrument(skip_all, name="server.sync.journal.prune", level="debug", fields(account_id=%owner, target))]
pub(super) async fn prune(
    store: &AccountStore,
    owner: AccountId,
    target: i64,
) -> anyhow::Result<Outcome> {
    let mut transaction = store.pool.begin().await?;

    if sqlx::query_file!("sql/retention/lock.sql", owner.to_string())
        .fetch_optional(&mut *transaction)
        .await?
        .is_none()
    {
        return Ok(Outcome::Contended);
    }

    let rows = sqlx::query_file!(
        "sql/retention/journal_prefix.sql",
        owner.to_string(),
        target,
        super::BATCH_SIZE
    )
    .fetch_all(&mut *transaction)
    .await?;

    if rows.is_empty() {
        // Release the account lock before collection starts reclamation on
        // another connection; dropping the transaction only queues rollback.
        transaction.rollback().await?;
        return Ok(Outcome::Unchanged);
    }

    let state = sqlx::query_file!("sql/metadata/state_select.sql", owner.to_string())
        .fetch_one(&mut *transaction)
        .await?;
    let mut floor =
        lineage::load_coverage(&mut transaction, owner, state.retained_journal_coverage_id).await?;
    let mut ids = Vec::new();
    for row in &rows {
        floor = floor.join(&Coverage::from(Position {
            timeline: row.timeline_id.parse()?,
            revision: row.revision.try_into()?,
        }));
        ids.push(row.installation_order);
    }

    let coverage = lineage::insert_coverage(&mut transaction, owner, &floor).await?;
    let removed = sqlx::query_file!("sql/retention/journal_delete.sql", owner.to_string(), &ids)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    sqlx::query_file!(
        "sql/retention/floor_update.sql",
        owner.to_string(),
        coverage
    )
    .execute(&mut *transaction)
    .await?;

    // Account coverage and head are unchanged: pruning is not a recovered prefix.
    transaction.commit().await?;

    metrics::counter!("clacks_sync_journal_entries_removed_total").increment(removed);
    tracing::debug!(
        event = "sync.journal.pruned",
        account_id = %owner,
        removed,
        "Published the retained incremental boundary"
    );

    Ok(Outcome::Pruned {
        more: i64::try_from(rows.len())? == super::BATCH_SIZE,
    })
}

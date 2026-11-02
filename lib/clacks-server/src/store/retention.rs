//! Automatic account retention through concrete, independently committed phases.

/// Metric name for baseline account lock acquisition wait.
pub(crate) const SYNC_BASELINE_LOCK_WAIT_SECONDS_METRIC: &str =
    "clacks_sync_baseline_lock_wait_seconds";

/// Metric name for baseline account lock holding through transaction completion.
pub(crate) const SYNC_BASELINE_LOCK_HOLD_SECONDS_METRIC: &str =
    "clacks_sync_baseline_lock_hold_seconds";

/// Metric name for contexts examined during baseline installation.
pub(crate) const SYNC_BASELINE_CONTEXTS_EXAMINED_METRIC: &str =
    "clacks_sync_baseline_contexts_examined";

/// Metric name for baseline installation including transaction completion.
pub(crate) const SYNC_BASELINE_INSTALL_DURATION_SECONDS_METRIC: &str =
    "clacks_sync_baseline_install_duration_seconds";

/// Metric name for one journal pruning transaction.
pub(crate) const SYNC_JOURNAL_PRUNE_DURATION_SECONDS_METRIC: &str =
    "clacks_sync_journal_prune_duration_seconds";

/// Metric name for one metadata reclamation transaction.
pub(crate) const SYNC_METADATA_RECLAIM_DURATION_SECONDS_METRIC: &str =
    "clacks_sync_metadata_reclaim_duration_seconds";

mod installation;
mod journal;
mod reclaim;
mod roots;

use super::AccountStore;
use clacks_model::ids::AccountId;

/// Maximum owning rows removed by one physical cleanup transaction.
const BATCH_SIZE: i64 = 1_000;

/// Collect eligible metadata without requiring an absent client to participate.
pub(crate) async fn collect(
    store: &AccountStore,
    account: AccountId,
    journal_entries: i64,
    stop: &tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        journal_entries > 0,
        "journal retention target must be positive"
    );

    if stop.is_cancelled() {
        return Ok(());
    }

    let started = std::time::Instant::now();
    let result = installation::install(store, account).await;
    let outcome = match &result {
        Ok(installation::Outcome::Installed { .. }) => "installed",
        Ok(installation::Outcome::Unchanged) => "unchanged",
        Ok(installation::Outcome::Contended) => "contended",
        Err(error)
            if matches!(
                error.downcast_ref::<clacks_sync::error::MetadataError>(),
                Some(clacks_sync::error::MetadataError::Incomplete { .. })
            ) =>
        {
            "blocked"
        }
        Err(_) => "failed",
    };
    metrics::histogram!(SYNC_BASELINE_INSTALL_DURATION_SECONDS_METRIC, "outcome" => outcome)
        .record(started.elapsed().as_secs_f64());

    match result {
        Ok(installation::Outcome::Installed { converted }) => {
            metrics::counter!("clacks_sync_baseline_contexts_converted_total").increment(converted);
            tracing::info!(
                event = "sync.baseline.installed",
                account_id = %account,
                converted,
                "Equivalent synchronization baseline installed"
            );
            let _ = store.changes.send(account);
        }
        Ok(installation::Outcome::Contended) => return Ok(()),
        Ok(installation::Outcome::Unchanged) => {}
        Err(error) if outcome == "blocked" => {
            tracing::debug!(
                event = "sync.baseline.blocked",
                account_id = %account,
                reason = %error,
                "Retaining unproved ordering evidence"
            );
            return Ok(());
        }
        Err(error) => return Err(error),
    }

    while !stop.is_cancelled() {
        let started = std::time::Instant::now();
        let result = journal::prune(store, account, journal_entries).await;
        let outcome = match &result {
            Ok(journal::Outcome::Pruned { .. }) => "pruned",
            Ok(journal::Outcome::Unchanged) => "unchanged",
            Ok(journal::Outcome::Contended) => "contended",
            Err(_) => "failed",
        };
        metrics::histogram!(SYNC_JOURNAL_PRUNE_DURATION_SECONDS_METRIC, "outcome" => outcome)
            .record(started.elapsed().as_secs_f64());

        if !matches!(result?, journal::Outcome::Pruned { more: true }) {
            break;
        }
    }

    while !stop.is_cancelled() {
        let started = std::time::Instant::now();
        let result = reclaim::reclaim(store, account).await;
        let outcome = match &result {
            Ok(reclaim::Outcome::Reclaimed) => "reclaimed",
            Ok(reclaim::Outcome::Unchanged) => "unchanged",
            Ok(reclaim::Outcome::Contended) => "contended",
            Err(_) => "failed",
        };
        metrics::histogram!(SYNC_METADATA_RECLAIM_DURATION_SECONDS_METRIC, "outcome" => outcome)
            .record(started.elapsed().as_secs_f64());

        if !matches!(result?, reclaim::Outcome::Reclaimed) {
            break;
        }
    }

    Ok(())
}

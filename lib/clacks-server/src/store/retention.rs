//! Automatic account retention through concrete, independently committed phases.

mod installation;

use super::AccountStore;
use clacks_model::ids::AccountId;

/// Collect eligible metadata without requiring an absent client to participate.
pub(crate) async fn collect(store: &AccountStore, account: AccountId) -> anyhow::Result<()> {
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
    metrics::histogram!("clacks_sync_baseline_install_duration_seconds", "outcome" => outcome)
        .record(started.elapsed().as_secs_f64());
    match result {
        Ok(installation::Outcome::Installed { converted }) => {
            metrics::counter!("clacks_sync_baseline_contexts_converted_total").increment(converted);
            tracing::info!(event="sync.baseline.installed", account_id=%account, converted, "Equivalent synchronization baseline installed");
            let _ = store.changes.send(account);
        }
        Ok(_) => {}
        Err(error) if outcome == "blocked" => {
            tracing::debug!(event="sync.baseline.blocked", account_id=%account, reason=%error, "Retaining unproved ordering evidence");
        }
        Err(error) => return Err(error),
    }
    Ok(())
}

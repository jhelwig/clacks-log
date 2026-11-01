//! Runtime-owned periodic synchronization retention with account isolation.

use crate::store::AccountStore;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Starting sweep cadence; tune from observed installation and contention costs.
const SWEEP_INTERVAL: Duration = Duration::from_secs(300);

/// Bound account discovery without limiting a sweep to only its first page.
const ACCOUNT_PAGE_SIZE: i64 = 100;

/// Run automatic collection until shutdown, allowing admitted operations to finish.
pub(crate) async fn run(store: AccountStore, journal_entries: i64, stop: CancellationToken) {
    let mut interval = tokio::time::interval(SWEEP_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            biased;
            _ = stop.cancelled() => break,
            _ = interval.tick() => {}
        }

        if let Err(error) = sweep(&store, journal_entries, &stop).await {
            tracing::error!(
                event = "sync.retention.failed",
                reason = %error,
                "Synchronization retention will retry on its next sweep"
            );
        }
    }
}

/// Traverse all account pages while leaving unrelated accounts available on failure.
async fn sweep(
    store: &AccountStore,
    journal_entries: i64,
    stop: &CancellationToken,
) -> anyhow::Result<()> {
    let mut after = String::new();

    loop {
        let accounts = sqlx::query_file!("sql/retention/accounts.sql", after, ACCOUNT_PAGE_SIZE)
            .fetch_all(&store.pool)
            .await?;

        if accounts.is_empty() {
            return Ok(());
        }

        for account in accounts {
            if stop.is_cancelled() {
                return Ok(());
            }

            let owner = account.account_id.parse()?;
            if let Err(error) =
                crate::store::retention::collect(store, owner, journal_entries, stop).await
            {
                tracing::error!(
                    event = "sync.retention.account_failed",
                    account_id = %owner,
                    reason = %error,
                    "Account retention will retry on a later sweep"
                );
            }

            after = account.account_id;
        }
    }
}

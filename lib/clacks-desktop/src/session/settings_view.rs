//! Retry timing for verifying the current workspace's committed settings.

use clacks_client_store::{settings::SettingsSnapshot, store::ClientStore};
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;
use tokio::time::Instant;

/// A single pending read shared by all requests to recover the workspace view.
#[derive(Default)]
pub(super) struct Recovery {
    /// Deadline and delay of the next read; renewed only on read failure.
    pending: Option<(Instant, Duration)>,
}

impl Recovery {
    /// Coalesce read requests while allowing an acknowledged result to be displayed first.
    pub(super) fn schedule(&mut self) {
        if self.pending.is_none() {
            self.retry();
        }
    }

    /// Increase the delay after another unsuccessful read, up to thirty seconds.
    pub(super) fn retry(&mut self) {
        let delay = self
            .pending
            .map_or(Duration::from_millis(150), |(_, last)| {
                (last * 2).min(Duration::from_secs(30))
            });
        self.pending = Some((Instant::now() + delay, delay));
    }

    /// Whether a reread has been requested independently of a new Settings opening.
    pub(super) fn scheduled(&self) -> bool {
        self.pending.is_some()
    }

    /// Verify the stored state and publish it unless another source published during the read.
    pub(super) async fn read(
        &mut self,
        store: &ClientStore,
        updates: &watch::Sender<Option<Arc<SettingsSnapshot>>>,
    ) -> anyhow::Result<Option<Arc<SettingsSnapshot>>> {
        let previous = updates.borrow().clone();
        let snapshot = match clacks_client_store::settings::snapshot(store).await {
            Ok(view) => Arc::new(view),
            Err(error) => {
                tracing::warn!(event = "desktop.settings_read_failed", error.type = crate::telemetry::failure::error_class(&error));
                self.retry();
                return Err(error);
            }
        };
        self.pending = None;
        let published = updates.send_if_modified(|current| {
            let unchanged = match (&*current, &previous) {
                (Some(current), Some(previous)) => Arc::ptr_eq(current, previous),
                (None, None) => true,
                _ => false,
            };
            if unchanged {
                *current = Some(snapshot.clone());
            }
            unchanged
        });
        Ok(published.then_some(snapshot))
    }

    /// Wait for a scheduled retry, or proceed immediately for a fresh editing basis.
    pub(super) async fn wait(&self) {
        if let Some((deadline, _)) = self.pending {
            tokio::time::sleep_until(deadline).await;
        }
    }
}

//! Workspace-only successful station information and in-flight sharing.
use super::{LookupHandle, LookupState, Selected};
use clacks_model::{directory::values::LookupKey, ids::LookupId};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tokio::time::Instant;

/// Maximum age, measured from successful completion rather than cache access.
const RETENTION: Duration = Duration::from_secs(24 * 60 * 60);
/// Maximum number of reusable successful results; held handles survive eviction.
const CAPACITY: usize = 128;

/// One reusable result and its purely local expiry/eviction metadata.
struct Success {
    /// Complete immutable outcome with original provider/time/identity.
    outcome: Arc<LookupState>,
    /// Monotonic successful completion instant.
    completed: Instant,
    /// Monotonic last-use time for least-recently-used eviction.
    used: Instant,
}

/// One actual operation, without consumer ownership or subscriptions.
pub(super) struct Flight {
    /// Lookup identity used to match completion with its in-flight operation.
    pub id: LookupId,
    /// Consumers clone the same standard shared future.
    pub handle: LookupHandle,
}

/// Data accessed only while holding the workspace's short synchronous lock.
pub(super) struct State {
    /// Task admission is closed before shutdown waits for tracked work.
    pub closed: bool,
    /// Actual provider/recovery state; failures are safe, nonsecret strings.
    pub selected: Result<Option<Selected>, String>,
    /// Only running operations are retained here.
    pub inflight: BTreeMap<LookupKey, Flight>,
    /// No persisted reusable cache or cache entries for failures/not-found.
    successes: BTreeMap<LookupKey, Success>,
}

impl State {
    /// Initialize one account workspace without any reusable information.
    pub fn new(selected: Result<Option<Selected>, String>) -> Self {
        Self {
            closed: false,
            selected,
            inflight: BTreeMap::new(),
            successes: BTreeMap::new(),
        }
    }

    /// Return fresh success and update recency without extending its lifetime.
    pub fn cached(&mut self, key: &LookupKey, now: Instant) -> Option<LookupHandle> {
        self.successes
            .retain(|_, entry| now.duration_since(entry.completed) < RETENTION);
        let success = self.successes.get_mut(key)?;
        success.used = now;
        tracing::debug!(event = "directory.lookup.reused");
        Some(LookupHandle {
            ready: Some(success.outcome.clone()),
            operation: None,
        })
    }

    /// Retain only successful whole snapshots and evict the least-recently-used success.
    pub fn complete(&mut self, key: LookupKey, id: LookupId, outcome: Arc<LookupState>) {
        if !self
            .inflight
            .get(&key)
            .is_some_and(|flight| flight.id == id)
        {
            return;
        }
        self.inflight.remove(&key);
        if !self.closed && matches!(outcome.as_ref(), LookupState::Found(_)) {
            let now = Instant::now();
            self.successes
                .retain(|_, entry| now.duration_since(entry.completed) < RETENTION);
            self.successes.insert(
                key,
                Success {
                    outcome,
                    completed: now,
                    used: now,
                },
            );
            if self.successes.len() > CAPACITY {
                let oldest = self
                    .successes
                    .iter()
                    .min_by_key(|(key, entry)| (entry.used, *key))
                    .map(|(key, _)| key.clone());
                if let Some(oldest) = oldest {
                    self.successes.remove(&oldest);
                }
            }
        }
    }
}

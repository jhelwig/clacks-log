//! Shared callsign-directory operations, independent of forms and saved contacts.

mod cache;
pub(crate) mod persistence;
pub(crate) mod runtime;
#[cfg(test)]
pub(crate) mod tests;

use super::providers::ProviderSession;
use clacks_client_store::store::ClientStore;
use clacks_model::{
    directory::values::{DirectorySnapshot, LookupKey},
    ids::LookupId,
};
use clacks_sync::protocol::directory::DirectoryMaterial;
use futures_util::{
    FutureExt,
    future::{BoxFuture, Shared},
};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::{sync::watch, time::Instant};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

/// Logical provider operations, excluding consumer joins and cache hits.
pub(crate) const LOOKUPS_METRIC: &str = "clacks_directory_lookups_total";
/// Admission-to-terminal seconds, excluding entry debounce.
pub(crate) const LOOKUP_DURATION_METRIC: &str = "clacks_directory_lookup_duration_seconds";

/// Safe directory outcome; recorded contact fields are never altered by it.
#[derive(Clone, Debug)]
pub(crate) enum LookupState {
    /// Empty input, closed workspace or unavailable provider configuration.
    Cleared,
    /// A consumer is debouncing or awaiting its provider result.
    Pending,
    /// One complete successful station snapshot shared unchanged among consumers.
    Found(Box<DirectorySnapshot>),
    /// An explicit provider absence preserves an eligible older snapshot.
    NotFound,
    /// Current protected material requires replacement or explicit retry.
    PausedAuthentication,
    /// Safe failure diagnosis without provider payloads or secrets.
    Failed(String),
}

/// Standard future sharing provides independent completion handles.
type Operation = Shared<BoxFuture<'static, Arc<LookupState>>>;

/// A ready result or one shared asynchronous provider operation.
#[derive(Clone)]
pub(crate) struct LookupHandle {
    /// Cached outcomes are immediately inspectable without polling.
    ready: Option<Arc<LookupState>>,
    /// Pending operations are driven by the owning workspace.
    operation: Option<Operation>,
}

impl LookupHandle {
    /// Retain already obtained successful facts when a consumer freezes its save input.
    pub fn from_snapshot(snapshot: DirectorySnapshot) -> Self {
        Self::completed(LookupState::Found(Box::new(snapshot)))
    }
    /// Return an already available outcome without waiting for provider access.
    pub fn ready(&self) -> Option<Arc<LookupState>> {
        self.ready.clone().or_else(|| {
            self.operation
                .as_ref()
                .and_then(|work| work.peek().cloned())
        })
    }

    /// Await this consumer's shared result without affecting other consumers.
    pub async fn wait(self) -> Arc<LookupState> {
        if let Some(ready) = self.ready {
            return ready;
        }
        match self.operation {
            Some(work) => work.await,
            None => Arc::new(LookupState::Cleared),
        }
    }

    /// Construct a terminal result with no provider dispatch.
    fn completed(state: LookupState) -> Self {
        Self {
            ready: Some(Arc::new(state)),
            operation: None,
        }
    }
}

/// Immutable caller demand, including the original entry deadline when saving early.
#[derive(Clone)]
pub(crate) struct LookupDemand {
    /// Normalized requested station, independent of any QSO identity.
    pub key: LookupKey,
    /// Entry debounce deadline, or now for independent capture.
    pub due: Instant,
    /// Operation or result retained by this consumer when available.
    pub handle: Option<LookupHandle>,
}

/// Provider session and recovery state for the actual installed material.
#[derive(Clone)]
struct Selected {
    /// Captured protected value; never included in telemetry.
    material: DirectoryMaterial,
    /// Provider selected at actual network admission.
    provider: clacks_model::directory::values::DirectoryProvider,
    /// Pooled transport and transient authentication.
    session: ProviderSession,
    /// Authentication rejection pauses only the affected current material.
    paused: bool,
    /// Longest outstanding provider-requested quiet period.
    retry_after: Option<(Instant, std::time::Duration)>,
}

/// Resources shared within exactly one opened account workspace.
struct Inner {
    /// Only data access and task registration occur under this lock.
    state: Mutex<cache::State>,
    /// Account definitions used to map provider values into native directory facts.
    store: ClientStore,
    /// Explicit executor permits nonblocking requests from the native UI thread.
    executor: tokio::runtime::Handle,
    /// Standard workspace task ownership includes provider and saved-result work.
    tasks: TaskTracker,
    /// Cancellation never prevents an already admitted QSO save.
    stop: CancellationToken,
    /// Wake native views when outcomes or configuration become available.
    context: Arc<OnceLock<eframe::egui::Context>>,
    /// Current failed consumers can resume after actual configuration recovery.
    configuration: watch::Sender<()>,
}

/// Cloneable access to the current workspace's keyed directory operations.
#[derive(Clone)]
pub(crate) struct LookupClient {
    /// Workspace-scoped state; no form or QSO owns the provider request.
    inner: Arc<Inner>,
}

impl LookupClient {
    /// Determine whether two consumer handles refer to the same opened workspace.
    pub fn same_workspace(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
    /// Reuse success, join existing work or start one request for this key.
    pub fn request(&self, key: LookupKey) -> LookupHandle {
        let Ok(mut state) = self.inner.state.lock() else {
            return LookupHandle::completed(LookupState::Failed(
                "Directory lookup state is unavailable".into(),
            ));
        };
        if state.closed {
            return LookupHandle::completed(LookupState::Cleared);
        }
        let now = Instant::now();
        if let Some(handle) = state.cached(&key, now) {
            return handle;
        }
        if let Some(flight) = state.inflight.get(&key) {
            tracing::debug!(event="directory.lookup.joined", lookup_id=%flight.id);
            return flight.handle.clone();
        }
        let selected = match &state.selected {
            Ok(Some(selected)) if selected.paused => {
                return LookupHandle::completed(LookupState::PausedAuthentication);
            }
            Ok(Some(selected))
                if selected
                    .retry_after
                    .is_some_and(|(started, delay)| started.elapsed() < delay) =>
            {
                return LookupHandle::completed(LookupState::Failed(
                    "Directory rate limit reached; retry later.".into(),
                ));
            }
            Ok(Some(selected)) => selected.clone(),
            Ok(None) => return LookupHandle::completed(LookupState::Cleared),
            Err(message) => return LookupHandle::completed(LookupState::Failed(message.clone())),
        };
        let id = LookupId::new();
        let operation = runtime::fetch(Arc::downgrade(&self.inner), selected, key.clone(), id)
            .boxed()
            .shared();
        let handle = LookupHandle {
            ready: None,
            operation: Some(operation),
        };
        state.inflight.insert(
            key,
            cache::Flight {
                id,
                handle: handle.clone(),
            },
        );
        let driven = handle.clone();
        let inner = self.inner.clone();
        // Register under the admission lock: closing a tracker alone permits new spawns.
        self.inner.tasks.spawn_on(
            async move {
                driven.wait().await;
                if let Some(context) = inner.context.get() {
                    context.request_repaint();
                }
            },
            &self.inner.executor,
        );
        handle
    }

    /// Resume rejected authentication and retry failure through the ordinary keyed operation.
    pub fn retry(&self, key: LookupKey) -> LookupHandle {
        if let Ok(mut state) = self.inner.state.lock()
            && let Ok(Some(selected)) = &mut state.selected
        {
            if selected.paused {
                selected.session.reset_authentication();
            }
            selected.paused = false;
        }
        self.request(key)
    }

    /// Subscribe to changes in provider selection or protected material.
    pub fn configuration_changes(&self) -> watch::Receiver<()> {
        self.inner.configuration.subscribe()
    }
}

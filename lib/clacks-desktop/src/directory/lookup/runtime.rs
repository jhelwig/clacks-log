//! Provider configuration, safe outcomes and standard workspace task lifetime.
use super::{
    Inner, LOOKUP_DURATION_METRIC, LOOKUPS_METRIC, LookupClient, LookupState, Selected,
    cache::State,
};
use crate::directory::providers::{ProviderError, ProviderLookup, ProviderSession};
use clacks_client_store::{identity::EnrollmentBinding, store::ClientStore};
use clacks_model::{
    directory::values::{DirectorySnapshot, DirectorySnapshotFields, LookupKey},
    ids::LookupId,
    scalar::UtcTimestamp,
};
use clacks_sync::protocol::directory::ProvisionedConfiguration;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use tokio::{sync::watch, time::Instant};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

/// Configuration monitoring and shutdown for one active account workspace.
pub(crate) struct LookupRuntime {
    /// Shared provider/cache resources, never a registry of QSO consumers.
    client: LookupClient,
    /// Current protected enrollment account.
    binding: watch::Receiver<Option<EnrollmentBinding>>,
    /// Last installed provider configuration.
    installed: watch::Receiver<Option<ProvisionedConfiguration>>,
}

impl LookupRuntime {
    /// Initialize available configuration before consumers receive the client.
    pub async fn new(
        store: ClientStore,
        binding: watch::Receiver<Option<EnrollmentBinding>>,
        installed: watch::Receiver<Option<ProvisionedConfiguration>>,
        context: Arc<OnceLock<eframe::egui::Context>>,
    ) -> (LookupClient, Self) {
        let selected = selection(binding.borrow().as_ref(), installed.borrow().as_ref());
        Self::with_selection(store, binding, installed, context, selected)
    }

    /// Compose the same runtime with an already resolved concrete provider session.
    pub(super) fn with_selection(
        store: ClientStore,
        binding: watch::Receiver<Option<EnrollmentBinding>>,
        installed: watch::Receiver<Option<ProvisionedConfiguration>>,
        context: Arc<OnceLock<eframe::egui::Context>>,
        selected: Result<Option<Selected>, String>,
    ) -> (LookupClient, Self) {
        let client = LookupClient {
            inner: Arc::new(Inner {
                state: Mutex::new(State::new(selected)),
                store,
                executor: tokio::runtime::Handle::current(),
                tasks: TaskTracker::new(),
                stop: CancellationToken::new(),
                context,
                configuration: watch::channel(()).0,
            }),
        };
        (
            client.clone(),
            Self {
                client,
                binding,
                installed,
            },
        )
    }

    /// Observe actual installed material and join every owned task before store closure.
    pub async fn run(mut self, stop: CancellationToken) -> anyhow::Result<()> {
        let mut binding_open = true;
        let mut installation_open = true;
        loop {
            tokio::select! {
                _ = stop.cancelled() => break,
                result = self.binding.changed(), if binding_open => {
                    if result.is_err() { binding_open = false; continue; }
                    self.binding.borrow_and_update();
                    self.install();
                }
                result = self.installed.changed(), if installation_open => {
                    if result.is_err() { installation_open = false; continue; }
                    self.installed.borrow_and_update();
                    self.install();
                }
            }
        }
        if let Ok(mut state) = self.client.inner.state.lock() {
            state.closed = true;
        }
        self.client.inner.stop.cancel();
        self.client.inner.tasks.close();
        self.client.inner.tasks.wait().await;
        Ok(())
    }

    /// Replace dispatch configuration without erasing eligible station information.
    fn install(&self) {
        let next = selection(
            self.binding.borrow().as_ref(),
            self.installed.borrow().as_ref(),
        );
        if let Ok(mut state) = self.client.inner.state.lock() {
            if let (Ok(Some(current)), Ok(Some(next))) = (&state.selected, &next)
                && current.provider == next.provider
                && current.material == next.material
            {
                return;
            }
            if matches!((&state.selected, &next), (Ok(None), Ok(None))) {
                return;
            }
            state.selected = next;
        }
        self.client.inner.configuration.send_replace(());
        if let Some(context) = self.client.inner.context.get() {
            context.request_repaint();
        }
    }
}

/// Resolve account-scoped provider selection and protected material.
fn selection(
    binding: Option<&EnrollmentBinding>,
    installed: Option<&ProvisionedConfiguration>,
) -> Result<Option<Selected>, String> {
    let (Some(binding), Some(installed)) = (binding, installed) else {
        return Ok(None);
    };
    if binding.account != installed.account {
        return Err("Directory installation belongs to another account".into());
    }
    let (Some(provider), Some(material)) =
        (installed.configuration.selected, installed.material.clone())
    else {
        return Ok(None);
    };
    let session = ProviderSession::new(provider, material.clone()).map_err(|_| {
        "Directory configuration could not be used; retry directory storage.".to_string()
    })?;
    Ok(Some(Selected {
        material,
        provider,
        session,
        paused: false,
        retry_after: None,
    }))
}

/// One admitted logical lookup records its actual outcome even on cancellation.
struct Attempt {
    /// Bounded provider dimension.
    provider: &'static str,
    /// Lookup identity used for telemetry correlation.
    id: LookupId,
    /// Admission excludes entry debounce.
    started: std::time::Instant,
    /// Terminal category, defaulting to cancellation.
    outcome: &'static str,
}

impl Drop for Attempt {
    fn drop(&mut self) {
        metrics::counter!(LOOKUPS_METRIC, "provider"=>self.provider,"outcome"=>self.outcome)
            .increment(1);
        metrics::histogram!(LOOKUP_DURATION_METRIC,"provider"=>self.provider,"outcome"=>self.outcome).record(self.started.elapsed().as_secs_f64());
        tracing::debug!(event="directory.lookup.completed",lookup_id=%self.id,provider=self.provider,outcome=self.outcome);
    }
}

/// Fetch station information once, preserving provider/time/identity for every consumer.
#[tracing::instrument(skip_all, name="desktop.directory.lookup", fields(lookup_id=%id, provider=?selected.provider))]
pub(super) async fn fetch(
    inner: Weak<Inner>,
    selected: Selected,
    key: LookupKey,
    id: LookupId,
) -> Arc<LookupState> {
    let Some(resources) = inner.upgrade() else {
        return Arc::new(LookupState::Cleared);
    };
    let store = resources.store.clone();
    let stop = resources.stop.clone();
    drop(resources);
    let mut attempt = Attempt {
        provider: selected.provider.as_str(),
        id,
        started: std::time::Instant::now(),
        outcome: "cancelled",
    };
    let result = tokio::select! {
        _ = stop.cancelled() => None,
        result = selected.session.lookup(&key) => Some(result),
    };
    let mut retry_after = None;
    let state = match result {
        None => LookupState::Cleared,
        Some(Ok(ProviderLookup::Found(facts))) => {
            let mut facts = *facts;
            if let Ok(settings) = clacks_client_store::settings::snapshot(&store).await {
                facts.references = facts
                    .references
                    .into_iter()
                    .filter_map(|reference| {
                        let definition = settings
                            .groups
                            .iter()
                            .filter_map(|row| match row.working_state() {
                                Some(clacks_sync::record::RecordState::Live(
                                    clacks_sync::record::RecordValue::Group(group),
                                )) => Some(group.as_ref()),
                                _ => None,
                            })
                            .find(|definition| definition.id() == reference.group_id);
                        clacks_model::group::normalize_reference(&reference, definition).ok()
                    })
                    .collect();
            }
            let now = time::OffsetDateTime::now_utc();
            match UtcTimestamp::new(now.unix_timestamp(), now.nanosecond())
                .map_err(anyhow::Error::from)
                .and_then(|lookup_time| {
                    DirectorySnapshot::new(DirectorySnapshotFields {
                        provider: selected.provider,
                        key: key.clone(),
                        lookup_time,
                        lookup_id: id,
                        facts,
                    })
                }) {
                Ok(snapshot) => LookupState::Found(Box::new(snapshot)),
                Err(_) => LookupState::Failed("Directory returned invalid facts".into()),
            }
        }
        Some(Ok(ProviderLookup::NotFound)) => LookupState::NotFound,
        Some(Err(ProviderError::Authentication)) => {
            tracing::warn!(event="directory.authentication.rejected",lookup_id=%id,provider=?selected.provider);
            LookupState::PausedAuthentication
        }
        Some(Err(ProviderError::RateLimited { retry_after: delay })) => {
            retry_after = delay;
            tracing::warn!(event="directory.lookup.rate_limited",lookup_id=%id,provider=?selected.provider);
            LookupState::Failed("Directory rate limit reached; retry later.".into())
        }
        Some(Err(error)) => {
            tracing::warn!(event="directory.lookup.unavailable",lookup_id=%id,provider=?selected.provider,reason=%error);
            LookupState::Failed(error.to_string())
        }
    };
    attempt.outcome = match &state {
        LookupState::Found(_) => "success",
        LookupState::NotFound => "not_found",
        LookupState::PausedAuthentication => "authentication",
        LookupState::Cleared => "cancelled",
        _ => "unavailable",
    };
    let outcome = Arc::new(state);
    if let Some(inner) = inner.upgrade()
        && let Ok(mut current) = inner.state.lock()
    {
        if let Ok(Some(current)) = &mut current.selected
            && current.provider == selected.provider
            && current.material == selected.material
        {
            if matches!(outcome.as_ref(), LookupState::PausedAuthentication) {
                current.paused = true;
            }
            if let Some(delay) = retry_after {
                let remaining = current
                    .retry_after
                    .map(|(started, delay)| delay.saturating_sub(started.elapsed()))
                    .unwrap_or_default();
                if delay > remaining {
                    current.retry_after = Some((Instant::now(), delay));
                }
            }
        }
        current.complete(key, id, outcome.clone());
    }
    outcome
}

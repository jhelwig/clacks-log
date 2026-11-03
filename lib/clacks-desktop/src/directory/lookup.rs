//! Debounced entry lookups with immutable response ownership and guarded persistence.

#[cfg(test)]
pub(crate) mod tests;

use super::providers::{ProviderError, ProviderLookup, ProviderSession};
use clacks_client_store::{directory::snapshots, identity::EnrollmentBinding, store::ClientStore};
use clacks_model::{
    directory::values::{DirectorySnapshot, DirectorySnapshotFields, LookupKey},
    ids::{LookupId, QsoId},
    scalar::{RecordedCallsign, UtcTimestamp},
};
use clacks_sync::protocol::directory::{DirectoryMaterial, ProvisionedConfiguration};
use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::{
    sync::{mpsc, watch},
    task::JoinSet,
    time::Instant,
};
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, instrument::WithSubscriber};

/// One terminal logical lookup, independent of internal authentication renewal.
pub(crate) const LOOKUPS_METRIC: &str = "clacks_directory_lookups_total";
/// Admission-to-terminal seconds, excluding debounce.
pub(crate) const LOOKUP_DURATION_METRIC: &str = "clacks_directory_lookup_duration_seconds";

/// Entry changes share the existing ordered settings admission channel.
pub(crate) enum LookupCommand {
    /// Observe the current effective call without altering recorded facts.
    CallChanged { qso: QsoId, call: String },
    /// Retry a cached result only after the contact has actually committed.
    Saved { qso: QsoId },
    /// Forget the displayed entry after deletion or dismissal.
    Deleted { qso: QsoId },
    /// Explicitly retry authentication for this entry.
    Retry { qso: QsoId },
}

/// Nonsecret presentation scoped to one contact and normalized key.
pub(crate) struct LookupUpdate {
    /// Original response owner.
    pub qso: QsoId,
    /// Absent only when clearing empty text.
    pub key: Option<LookupKey>,
    /// Current lookup diagnosis or successful preview.
    pub state: LookupState,
    /// A directory winner actually committed and invalidates contact reads.
    pub changed: bool,
}

/// Lookup feedback is independent from saved recorded text and synchronization.
#[derive(Clone, Debug)]
pub(crate) enum LookupState {
    /// Empty text or unavailable configuration has no result.
    Cleared,
    /// Debouncing or awaiting the selected provider.
    Pending,
    /// Complete successful separate facts.
    Found(Box<DirectorySnapshot>),
    /// Explicit provider absence preserves previously retained success.
    NotFound,
    /// Current material requires explicit retry or replacement.
    PausedAuthentication,
    /// Safe diagnostic, never a secret or provider payload.
    Failed(String),
}

/// Concrete presentation outputs also wake hidden native windows.
#[derive(Clone)]
pub(crate) struct Outputs {
    /// Bounded actual window delivery.
    pub updates: mpsc::Sender<LookupUpdate>,
    /// Renderer attaches once after startup.
    pub context: Arc<OnceLock<eframe::egui::Context>>,
}

impl Outputs {
    /// Deliver one original-owner outcome without waiting for a redraw.
    async fn publish(
        &self,
        qso: QsoId,
        key: Option<LookupKey>,
        state: LookupState,
        changed: bool,
        stop: &CancellationToken,
    ) {
        tokio::select! { _ = stop.cancelled() => {}, _ = self.updates.send(LookupUpdate { qso, key, state, changed }) => {} }
        if let Some(context) = self.context.get() {
            context.request_repaint();
        }
    }
}

/// Only open entries retain debounce and successful preview state.
struct Entry {
    /// Current canonical text.
    key: LookupKey,
    /// Next admission, absent once dispatched.
    due: Option<Instant>,
    /// Available success retained across failures.
    success: Option<DirectorySnapshot>,
}

/// Normalize once at the same boundary used for snapshot eligibility.
fn observe(entries: &mut BTreeMap<QsoId, Entry>, qso: QsoId, call: &str) {
    let Ok(call) = RecordedCallsign::new(call) else {
        entries.remove(&qso);
        return;
    };
    let key = LookupKey::from_recorded(&call);
    if entries.get(&qso).is_some_and(|entry| entry.key == key) {
        return;
    }
    entries.insert(
        qso,
        Entry {
            key,
            due: Some(Instant::now() + Duration::from_millis(500)),
            success: None,
        },
    );
}

/// Reject only the actual native material captured by this response, including restored revisions.
fn authentication_rejected(
    selected: &mut anyhow::Result<Option<Selected>>,
    provider: clacks_model::directory::values::DirectoryProvider,
    material: &DirectoryMaterial,
) -> LookupState {
    if let Ok(Some(selected)) = selected
        && selected.provider == provider
        && &selected.material == material
    {
        selected.paused = true;
        LookupState::PausedAuthentication
    } else {
        LookupState::Failed("Earlier directory credentials were rejected".into())
    }
}

/// A logical lookup reports cancellation when shutdown drops its unfinished future.
struct Attempt {
    /// Provider captured at admission.
    provider: &'static str,
    /// Safe immutable correlation.
    lookup: LookupId,
    /// Original contact owner.
    qso: QsoId,
    /// Admission excludes debounce and includes any session contention.
    started: std::time::Instant,
    /// A response replaces the default cancellation outcome.
    outcome: &'static str,
}

impl Attempt {
    /// Settle this admitted attempt while retaining ownership of the cancellation guard.
    fn finish(&mut self, outcome: &'static str) {
        self.outcome = outcome;
    }
}

impl Drop for Attempt {
    fn drop(&mut self) {
        metrics::counter!(LOOKUPS_METRIC, "provider"=>self.provider, "outcome"=>self.outcome)
            .increment(1);
        metrics::histogram!(LOOKUP_DURATION_METRIC, "provider"=>self.provider, "outcome"=>self.outcome).record(self.started.elapsed().as_secs_f64());
        tracing::debug!(event="directory.lookup.completed", lookup_id=%self.lookup, qso_id=%self.qso, provider=self.provider, outcome=self.outcome);
    }
}

/// Immutable ownership travels back with the response, independently from later form edits.
struct LookupRequest {
    /// Original contact, also the unfinished entry's stable identity.
    qso: QsoId,
    /// Canonical call used by this HTTP request.
    key: LookupKey,
    /// Immutable ordering and diagnostic correlation.
    lookup_id: LookupId,
    /// Actual material captures replacement and restored content without an artificial epoch.
    material: DirectoryMaterial,
    /// Provider selected at admission.
    provider: clacks_model::directory::values::DirectoryProvider,
}

/// Execute an admitted request under its own context, retaining ownership through cancellation.
#[tracing::instrument(skip_all, name="desktop.directory.lookup", fields(qso_id=%request.qso, lookup_id=%request.lookup_id, provider=?request.provider))]
async fn fetch(
    session: ProviderSession,
    request: LookupRequest,
    mut attempt: Attempt,
) -> (
    LookupRequest,
    Result<ProviderLookup, ProviderError>,
    UtcTimestamp,
) {
    let result = session.lookup(&request.key).await;
    attempt.finish(match &result {
        Ok(ProviderLookup::Found(_)) => "success",
        Ok(ProviderLookup::NotFound) => "not_found",
        Err(ProviderError::Authentication) => "authentication",
        Err(_) => "unavailable",
    });
    let now = time::OffsetDateTime::now_utc();
    let completed =
        UtcTimestamp::new(now.unix_timestamp(), now.nanosecond()).expect("current UTC timestamp");
    (request, result, completed)
}

/// Session and pause belong to exactly one installed protected entry.
struct Selected {
    /// Captured material identity and contents; diagnostics redact its login.
    material: DirectoryMaterial,
    /// Concrete provider selected when this session was created.
    provider: clacks_model::directory::values::DirectoryProvider,
    /// Concrete sessions share only their transient authentication.
    session: ProviderSession,
    /// Authentication rejection pauses only this material.
    paused: bool,
    /// Provider-requested quiet period; a gate, never an automatic retry queue.
    retry_after: Option<(Instant, Duration)>,
}

/// Load native material off the async runtime's worker threads.
async fn selection(
    binding: Option<&EnrollmentBinding>,
    installation: Option<&ProvisionedConfiguration>,
) -> anyhow::Result<Option<Selected>> {
    let (Some(binding), Some(installation)) = (binding, installation) else {
        return Ok(None);
    };
    anyhow::ensure!(
        binding.account == installation.account,
        "Directory installation belongs to another account"
    );
    let (Some(provider), Some(material)) = (
        installation.configuration.selected,
        installation.material.clone(),
    ) else {
        return Ok(None);
    };
    Ok(Some(Selected {
        material: material.clone(),
        provider,
        session: ProviderSession::new(provider, material)?,
        paused: false,
        retry_after: None,
    }))
}

/// Keep successful persistence and normal sync/read invalidation at the same boundary.
async fn persist(
    store: &ClientStore,
    qso: QsoId,
    snapshot: DirectorySnapshot,
    changes: &watch::Sender<()>,
) -> bool {
    match snapshots::apply(store, qso, snapshot).await {
        Ok(snapshots::SnapshotOutcome::Applied) => {
            changes.send_replace(());
            true
        }
        Ok(_) => false,
        Err(_) => {
            tracing::warn!(event="directory.snapshot.failed", qso_id=%qso);
            false
        }
    }
}

/// Own all provider work until shutdown, independently from logging or sync availability.
pub(crate) async fn run(
    store: ClientStore,
    mut binding: watch::Receiver<Option<EnrollmentBinding>>,
    mut installed: watch::Receiver<Option<ProvisionedConfiguration>>,
    commands: mpsc::Receiver<LookupCommand>,
    outputs: Outputs,
    changes: watch::Sender<()>,
    stop: CancellationToken,
) {
    let initial = installed.borrow_and_update().clone();
    let initial_binding = binding.borrow_and_update().clone();
    let selected = selection(initial_binding.as_ref(), initial.as_ref()).await;
    Runtime {
        store,
        binding,
        installed,
        outputs,
        changes,
        stop,
    }
    .serve(commands, selected)
    .await;
}

/// Workspace resources shared by initial and subsequent selected sessions.
struct Runtime {
    /// Owning local workspace.
    store: ClientStore,
    /// Protected owner established by enrollment.
    binding: watch::Receiver<Option<EnrollmentBinding>>,
    /// Actual installed configuration stream.
    installed: watch::Receiver<Option<ProvisionedConfiguration>>,
    /// Original window outputs.
    outputs: Outputs,
    /// Ordinary sync/read invalidation.
    changes: watch::Sender<()>,
    /// Owning session cancellation.
    stop: CancellationToken,
}

impl Runtime {
    /// Run with the initial concrete selection, retaining captured sessions through replacement.
    async fn serve(
        self,
        mut commands: mpsc::Receiver<LookupCommand>,
        mut selected: anyhow::Result<Option<Selected>>,
    ) {
        let Self {
            store,
            mut binding,
            mut installed,
            outputs,
            changes,
            stop,
        } = self;
        let mut entries = BTreeMap::<QsoId, Entry>::new();
        let mut work = JoinSet::new();
        let mut binding_open = true;
        loop {
            let due = entries.values().filter_map(|entry| entry.due).min();
            tokio::select! {
                biased;
                _ = stop.cancelled() => break,
                changed = binding.changed(), if binding_open => {
                    if changed.is_err() { binding_open = false; continue; }
                    let current_binding = binding.borrow_and_update().clone();
                    let current_installation = installed.borrow().clone();
                    selected = selection(current_binding.as_ref(), current_installation.as_ref()).await;
                    for entry in entries.values_mut().filter(|entry| entry.success.is_none()) { entry.due = Some(Instant::now()); }
                }
                changed = installed.changed() => {

                    if changed.is_err() { break; }
                    let next = installed.borrow_and_update().clone();
                    if let (Ok(Some(current)), Some(next)) = (&selected, &next) && next.material.as_ref() == Some(&current.material) && next.configuration.selected == Some(current.provider) { continue; }
                    let current_binding = binding.borrow().clone();
                    selected = selection(current_binding.as_ref(), next.as_ref()).await;
                    for entry in entries.values_mut().filter(|entry| entry.success.is_none()) { entry.due = Some(Instant::now()); }
                }
                command = commands.recv() => match command {
                    None => break,
                    Some(LookupCommand::CallChanged { qso, call }) => {
                        observe(&mut entries, qso, &call);
                        if !entries.contains_key(&qso) { outputs.publish(qso, None, LookupState::Cleared, false, &stop).await; }
                    }
                    Some(LookupCommand::Deleted { qso }) => { entries.remove(&qso); }
                    Some(LookupCommand::Saved { qso }) => {
                        if let Some(snapshot) = entries.get(&qso).and_then(|entry| entry.success.clone()) && persist(&store, qso, snapshot.clone(), &changes).await { outputs.publish(qso, Some(snapshot.key().clone()), LookupState::Found(Box::new(snapshot)), true, &stop).await; }
                    }
                    Some(LookupCommand::Retry { qso }) => {
                        if selected.is_err() {
                            let current_binding = binding.borrow().clone();
                            let current_installation = installed.borrow().clone();
                            selected = selection(current_binding.as_ref(), current_installation.as_ref()).await;
                        }
                        if let Ok(Some(selected)) = &mut selected {
                            if selected.paused { selected.session.reset_authentication(); }
                            selected.paused = false;
                        }
                        if let Some(entry) = entries.get_mut(&qso) { entry.due = Some(Instant::now()); }
                    }
                },
                _ = async { if let Some(due) = due { tokio::time::sleep_until(due).await } else { std::future::pending().await } } => {
                    for (&qso, entry) in entries.iter_mut().filter(|(_, entry)| entry.due.is_some_and(|due| due <= Instant::now())) {
                        entry.due = None;
                        let selected = match &selected {
                            Ok(Some(selected)) if selected.retry_after.is_some_and(|(started, delay)| started.elapsed() < delay) => { outputs.publish(qso, Some(entry.key.clone()), LookupState::Failed("Directory rate limit reached; retry later.".into()), false, &stop).await; continue; }
                            Ok(Some(selected)) if !selected.paused => selected,
                            Ok(Some(_)) => { outputs.publish(qso, Some(entry.key.clone()), LookupState::PausedAuthentication, false, &stop).await; continue; }
                            Ok(None) => { outputs.publish(qso, Some(entry.key.clone()), LookupState::Cleared, false, &stop).await; continue; }
                            Err(_) => { outputs.publish(qso, Some(entry.key.clone()), LookupState::Failed("Directory configuration could not be used; retry directory storage.".into()), false, &stop).await; continue; }
                        };
                        let session = selected.session.clone();
                        let provider = selected.provider;
                        let key = entry.key.clone();
                        let lookup = LookupId::new();
                        outputs.publish(qso, Some(key.clone()), LookupState::Pending, false, &stop).await;
                        let attempt = Attempt { provider: provider.as_str(), lookup, qso, started: std::time::Instant::now(), outcome: "cancelled" };
                        let request = LookupRequest { qso, key, lookup_id: lookup, material: selected.material.clone(), provider };
                        work.spawn(fetch(session, request, attempt).in_current_span().with_current_subscriber());
                    }
                }
                Some(result) = work.join_next(), if !work.is_empty() => {
                    let Ok((LookupRequest { qso, key, lookup_id, material, provider }, result, completed)) = result else { continue; };
                    let mut changed = false;
                    let state = match result {
                        Ok(ProviderLookup::Found(facts)) => {
                            let mut facts = *facts;
                            if let Ok(settings) = clacks_client_store::settings::snapshot(&store).await {
                                facts.references = facts.references.into_iter().filter_map(|reference| {
                                    let definition = settings.groups.iter().filter_map(|row| match row.working_state() { Some(clacks_sync::record::RecordState::Live(clacks_sync::record::RecordValue::Group(group))) => Some(group.as_ref()), _ => None }).find(|definition| definition.id() == reference.group_id);
                                    clacks_model::group::normalize_reference(&reference, definition).ok()
                                }).collect();
                            }
                            let snapshot = DirectorySnapshot::new(DirectorySnapshotFields { provider, key: key.clone(), lookup_time: completed, lookup_id, facts });
                            match snapshot {
                                Ok(snapshot) => {
                                    changed = persist(&store, qso, snapshot.clone(), &changes).await;
                                    if let Some(entry) = entries.get_mut(&qso).filter(|entry| entry.key == key)
                                        && entry.success.as_ref().is_none_or(|current| current.compare_lookup(&snapshot).is_lt()) { entry.success = Some(snapshot.clone()); }
                                    LookupState::Found(Box::new(snapshot))
                                }
                                Err(_) => LookupState::Failed("Directory returned invalid facts".into()),
                            }
                        }
                        Ok(ProviderLookup::NotFound) => LookupState::NotFound,
                        Err(ProviderError::Authentication) => {
                            tracing::warn!(event="directory.authentication.rejected", qso_id=%qso, lookup_id=%lookup_id, provider=provider.as_str());
                            authentication_rejected(&mut selected, provider, &material)
                        }
                        Err(ProviderError::RateLimited { retry_after }) => {
                            if let Ok(Some(selected)) = &mut selected
                                && selected.provider == provider && selected.material == material
                                && let Some(delay) = retry_after
                            {
                                let remaining = selected.retry_after.map(|(started, delay)| delay.saturating_sub(started.elapsed())).unwrap_or_default();
                                if delay > remaining {
                                    selected.retry_after = Some((Instant::now(), delay));
                                }
                            }
                            tracing::warn!(event="directory.lookup.rate_limited", qso_id=%qso, lookup_id=%lookup_id, provider=provider.as_str());
                            LookupState::Failed("Directory rate limit reached; retry later.".into())
                        }
                        Err(error) => {
                            tracing::warn!(event="directory.lookup.unavailable", qso_id=%qso, lookup_id=%lookup_id, provider=provider.as_str(), reason=%error);
                            LookupState::Failed(error.to_string())
                        }
                    };
                    outputs.publish(qso, Some(key), state, changed, &stop).await;
                }
            }
        }
        work.abort_all();
        while work.join_next().await.is_some() {}
    }
}

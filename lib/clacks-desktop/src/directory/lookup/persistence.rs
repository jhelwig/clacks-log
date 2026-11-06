//! Caller-owned saved-contact application, independent of provider request ownership.
use super::{LookupClient, LookupDemand, LookupHandle, LookupState};
use clacks_client_store::{directory::snapshots, store::ClientStore};
use clacks_model::{directory::values::DirectorySnapshot, ids::QsoId};
use tokio::sync::{oneshot, watch};

/// One actual save's ready information or gate for a late result.
pub(crate) struct PreparedLookup {
    /// Already ready success can be saved atomically with the contact.
    snapshot: Option<Box<DirectorySnapshot>>,
    /// Only a known successful save permits asynchronous application.
    saved: Option<oneshot::Sender<bool>>,
}

impl PreparedLookup {
    /// Borrow complete successful information available before the save starts.
    pub fn ready_snapshot(&self) -> Option<Box<DirectorySnapshot>> {
        self.snapshot.clone()
    }

    /// Settle from the actual store outcome, separately from following UI refresh.
    pub fn settle(mut self, saved: bool) {
        if let Some(sender) = self.saved.take() {
            let _ = sender.send(saved);
        }
    }
}

/// Retain the submitted lookup demand and deadline without delaying the contact save.
pub(crate) fn prepare(
    client: LookupClient,
    store: ClientStore,
    qso: QsoId,
    demand: LookupDemand,
    changes: watch::Sender<()>,
) -> PreparedLookup {
    if let Some(LookupState::Found(snapshot)) = demand
        .handle
        .as_ref()
        .and_then(LookupHandle::ready)
        .as_deref()
    {
        return PreparedLookup {
            snapshot: Some(snapshot.clone()),
            saved: None,
        };
    }
    let (saved, gate) = oneshot::channel();
    let inner = client.inner.clone();
    let Ok(state) = inner.state.lock() else {
        return PreparedLookup {
            snapshot: None,
            saved: None,
        };
    };
    if state.closed {
        return PreparedLookup {
            snapshot: None,
            saved: None,
        };
    }
    let stop = inner.stop.clone();
    let context = inner.context.clone();
    inner.tasks.spawn_on(async move {
        let result = async {
            tokio::time::sleep_until(demand.due).await;
            let handle = demand.handle.unwrap_or_else(|| client.request(demand.key));
            handle.wait().await
        };
        let committed = async { if gate.await == Ok(true) { Ok(()) } else { Err(()) } };
        let outcome = tokio::select! {
            _ = stop.cancelled() => return,
            outcome = async { tokio::try_join!(committed, async { Ok::<_, ()>(result.await) }) } => outcome,
        };
        let Ok(((), outcome)) = outcome else { return; };
        if let LookupState::Found(snapshot) = outcome.as_ref() {
            apply(&store, qso, snapshot.as_ref().clone(), &changes, &context).await;
        }
    }, &inner.executor);
    drop(state);
    PreparedLookup {
        snapshot: None,
        saved: Some(saved),
    }
}

/// Guard late application against the current live contact/key and notify normal synchronization.
#[tracing::instrument(skip_all, name="desktop.directory.snapshot",fields(qso_id=%qso))]
async fn apply(
    store: &ClientStore,
    qso: QsoId,
    snapshot: DirectorySnapshot,
    changes: &watch::Sender<()>,
    context: &std::sync::OnceLock<eframe::egui::Context>,
) {
    let lookup_id = snapshot.lookup_id();
    tracing::debug!(event="directory.snapshot.applying",qso_id=%qso,lookup_id=%lookup_id);
    match snapshots::apply(store, qso, snapshot).await {
        Ok(snapshots::SnapshotOutcome::Applied { .. }) => {
            changes.send_replace(());
            if let Some(context) = context.get() {
                context.request_repaint();
            }
        }
        Ok(_) => {}
        Err(error) => {
            tracing::warn!(event="directory.snapshot.failed",qso_id=%qso,error.type=crate::telemetry::failure::error_class(&error));
        }
    }
}

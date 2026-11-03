//! One request/data owner shared by the Worked Before pane and native window.

use crate::session::settings::{
    SettingsAction, SettingsRequest, SettingsResponse,
    contacts::{ContactAction, ContactResponse},
};
use clacks_client_store::{
    logbooks::selection::SelectedView,
    read::{
        contacts::{ContactOrder, ContactPage},
        worked_before::WorkedBeforeQuery,
    },
};
use clacks_model::scalar::UtcSecond;
use eframe::egui;
use tokio::sync::{mpsc, oneshot};

/// A reply remains bound to its exact prefix, view and complete ordering.
struct PendingRead {
    /// Inputs admitted to the runtime.
    query: WorkedBeforeQuery,
    /// Runtime acknowledgment for this read only.
    receiver: oneshot::Receiver<SettingsResponse>,
}

/// Shared data model; viewport focus and scroll state remain in egui.
#[derive(Default)]
pub(in crate::ui) struct WorkedBeforeModel {
    /// Current normalized prefix and selected scope.
    pub query: WorkedBeforeQuery,
    /// Complete accepted page; absent immediately after a prefix/view change.
    pub page: Option<ContactPage>,
    /// Actionable read error without disclosing query contents in diagnostics.
    pub error: Option<String>,
    /// Current UTC calendar-day number, supplied by the application clock.
    pub day: i64,
    /// Actual originating request, discarded on changed inputs.
    pending: Option<PendingRead>,
    /// A real mutation or changed request requires another bounded read.
    dirty: bool,
}

impl WorkedBeforeModel {
    /// Follow the shared call and accept only replies for current inputs.
    pub fn update(
        &mut self,
        context: &egui::Context,
        call: &str,
        view: SelectedView,
        order: ContactOrder,
        now: UtcSecond,
        requests: &mpsc::Sender<SettingsRequest>,
    ) {
        let prefix = call.trim().to_uppercase();
        if self.query.call_prefix != prefix || self.query.view != view {
            self.query.call_prefix = prefix;
            self.query.view = view;
            self.query.offset = 0;
            self.page = None;
            self.error = None;
            self.dirty = true;
        }
        if self.query.order != order {
            self.query.order = order;
            self.query.offset = 0;
            self.dirty = true;
        }
        self.day = now.unix_seconds().div_euclid(86_400);
        let seconds = 86_400 - now.unix_seconds().rem_euclid(86_400);
        context.request_repaint_after(std::time::Duration::from_secs(seconds as u64));
        if self.query.call_prefix.is_empty() {
            self.pending = None;
            self.page = None;
            self.dirty = false;
            return;
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|p| p.query != self.query || self.dirty)
        {
            self.pending = None;
        }
        if let Some(pending) = &mut self.pending {
            match pending.receiver.try_recv() {
                Ok(SettingsResponse::Contacts(ContactResponse::WorkedBefore(page))) => {
                    self.page = Some(page);
                    self.pending = None;
                    self.error = None;
                }
                Ok(SettingsResponse::Failed(message)) => {
                    self.error = Some(message);
                    self.pending = None;
                }
                Ok(_) | Err(oneshot::error::TryRecvError::Closed) => {
                    self.error = Some("Worked Before read ended without a usable response.".into());
                    self.pending = None;
                }
                Err(oneshot::error::TryRecvError::Empty) => {}
            }
        }
        if self.dirty && self.pending.is_none() {
            let (reply, receiver) = oneshot::channel();
            let query = self.query.clone();
            if requests
                .try_send(SettingsRequest {
                    action: SettingsAction::Contacts(ContactAction::WorkedBefore {
                        query: query.clone(),
                    }),
                    reply,
                })
                .is_ok()
            {
                self.pending = Some(PendingRead { query, receiver });
                self.dirty = false;
            }
        }
        if self.pending.is_some() {
            context.request_repaint_after(std::time::Duration::from_millis(25));
        }
    }

    /// Mark actual saved-data changes without discarding usable rows while reloading.
    pub fn refresh(&mut self) {
        self.dirty = true;
    }

    /// Move within the current prefix's complete ordered results.
    pub fn next_page(&mut self, forward: bool) {
        self.query.offset = if forward {
            self.query.offset.saturating_add(self.query.limit)
        } else {
            self.query.offset.saturating_sub(self.query.limit)
        };
        self.dirty = true;
    }
}

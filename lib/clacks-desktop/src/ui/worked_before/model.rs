//! One query/data owner shared by the Worked Before pane and native window.

use crate::{
    session::settings::{
        SettingsAction, SettingsRequest, SettingsResponse,
        contacts::{ContactAction, ContactResponse},
    },
    ui::contact_table::data::{PAGE_SIZE, TableData},
};
use clacks_client_store::{
    logbooks::selection::SelectedView,
    read::{contacts::ContactOrder, worked_before::WorkedBeforeQuery},
};
use clacks_model::scalar::UtcSecond;
use eframe::egui;
use std::collections::BTreeSet;
use tokio::sync::{mpsc, oneshot};

/// A reply remains bound to its exact prefix, view and complete ordering.
struct PendingRead {
    /// Inputs admitted to the runtime.
    query: WorkedBeforeQuery,
    /// First reply after invalidation replaces old cached rows.
    replace: bool,
    /// Runtime acknowledgment for this read only.
    receiver: oneshot::Receiver<SettingsResponse>,
}

/// Shared query/rows with separate viewport demands for the tab and window.
#[derive(Default)]
pub(in crate::ui) struct WorkedBeforeModel {
    /// Current normalized prefix and selected scope.
    pub query: WorkedBeforeQuery,
    /// Bounded viewport rows and complete logical counts.
    pub data: TableData,
    /// Pages needed by the workbench tab.
    pub tab_needed: BTreeSet<u32>,
    /// Pages needed by the dedicated window; empty while closed.
    pub window_needed: BTreeSet<u32>,
    /// Actionable read error, retained until explicit retry or invalidation.
    pub error: Option<String>,
    /// Current UTC calendar-day number, supplied by the application clock.
    pub day: i64,
    /// Actual originating request, discarded on changed query inputs.
    pending: Option<PendingRead>,
    /// A mutation or changed query requires a replacement read.
    dirty: bool,
}

impl WorkedBeforeModel {
    /// Follow the shared call and accept only replies for the current query.
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
        if self.query.call_prefix != prefix || self.query.view != view || self.query.order != order
        {
            self.query.call_prefix = prefix;
            self.query.view = view;
            self.query.order = order;
            self.query.offset = 0;
            self.data.clear();
            self.tab_needed = [0].into();
            self.window_needed.clear();
            self.error = None;
            self.dirty = true;
        }
        self.day = now.unix_seconds().div_euclid(86_400);
        let seconds = 86_400 - now.unix_seconds().rem_euclid(86_400);
        context.request_repaint_after(std::time::Duration::from_secs(seconds as u64));
        if self.query.call_prefix.is_empty() {
            self.pending = None;
            self.data.clear();
            self.dirty = false;
            return;
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|p| p.query != self.query || self.dirty)
        {
            tracing::debug!("Discarded obsolete Worked Before request");
            self.pending = None;
        }
        if let Some(pending) = &mut self.pending {
            match pending.receiver.try_recv() {
                Ok(SettingsResponse::Contacts(ContactResponse::WorkedBefore(page))) => {
                    self.data
                        .accept(pending.query.offset, page, pending.replace);
                    self.pending = None;
                    self.error = None;
                }
                Ok(SettingsResponse::Failed(message)) => {
                    self.dirty |= pending.replace;
                    self.error = Some(message);
                    self.pending = None;
                }
                Ok(_) | Err(oneshot::error::TryRecvError::Closed) => {
                    self.dirty |= pending.replace;
                    self.error = Some("Worked Before read ended without a usable response.".into());
                    self.pending = None;
                }
                Err(oneshot::error::TryRecvError::Empty) => {}
            }
        }
        if self.pending.is_none() && self.error.is_none() {
            let needed = self
                .tab_needed
                .union(&self.window_needed)
                .copied()
                .collect();
            let offset = if self.dirty {
                Some(
                    self.tab_needed
                        .first()
                        .or(self.window_needed.first())
                        .copied()
                        .unwrap_or(0),
                )
            } else {
                self.data.require(&needed)
            };
            if let Some(offset) = offset {
                let (reply, receiver) = oneshot::channel();
                self.query.offset = offset;
                self.query.limit = PAGE_SIZE;
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
                    self.pending = Some(PendingRead {
                        query,
                        replace: self.dirty,
                        receiver,
                    });
                    self.dirty = false;
                }
            }
        }
        if self.pending.is_some() {
            context.request_repaint_after(std::time::Duration::from_millis(25));
        }
    }

    /// Invalidate rows after a saved-data change, retaining usable rows while reloading.
    pub fn refresh(&mut self) {
        self.dirty = true;
        self.error = None;
    }
}

//! Workspace-owned UDP reception and completed-contact capture.

pub(crate) mod interfaces;
pub(crate) mod runtime;
pub(crate) mod socket;

use clacks_client_store::ws_suite::ReceiverSettings;
use std::sync::{Arc, OnceLock};
use tokio::sync::{oneshot, watch};

/// One request admitted by the owning workspace session.
#[derive(Debug)]
pub(crate) struct ReceiverRequest {
    /// Interface refresh or receiver configuration apply.
    pub action: ReceiverAction,
    /// Reply retained even when the dialog is dismissed.
    pub reply: oneshot::Sender<ReceiverResponse>,
}

/// UI receiver operations, without network or database work on the native thread.
#[derive(Debug)]
pub(crate) enum ReceiverAction {
    /// Enumerate current local addresses.
    Interfaces,
    /// Persist intent and activate it, or retry an unchanged failed activation.
    Apply {
        /// Proposed receiver preference.
        next: ReceiverSettings,
    },
}

/// Settled receiver request outcome.
#[derive(Debug)]
pub(crate) enum ReceiverResponse {
    /// Current OS interface choices, or an actionable enumeration failure.
    Interfaces(Result<Vec<interfaces::InterfaceChoice>, String>),
    /// Committed settings and separate activation outcome.
    Applied(ReceiverView),
    /// Validation/persistence failure; previous listener remains owned.
    Failed(String),
    /// Workspace no longer admits receiver operations.
    Closing,
}

/// Internal lifecycle outcome; only failures need a separate product message.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum ReceptionState {
    /// Reception deliberately disabled.
    #[default]
    Off,
    /// Socket construction and required membership succeeded.
    Listening,
    /// Reception could not start or continue.
    Failed(String),
}

/// Current receiver intent and lifecycle outcome, loaded by the runtime.
#[derive(Clone, Debug, Default)]
pub(crate) struct ReceiverView {
    /// Committed settings, absent until successfully read.
    pub settings: Option<ReceiverSettings>,
    /// Separate reception result; not a normal-state status display.
    pub reception: ReceptionState,
    /// Capture/conversion feedback, independent of whether reception remains active.
    pub capture_notice: Option<String>,
}

/// Reuse the workspace client identity to scope ordered entry delivery.
pub(crate) struct EntryCall {
    /// Workspace identity used to reject entry updates from a closed workspace.
    pub workspace: crate::directory::lookup::LookupClient,
    /// Only the field supplied by Status.
    pub status: protocol::StatusCall,
}

/// Publish outcomes and wake native presentation independently of painting.
#[derive(Clone)]
pub(crate) struct Outputs {
    /// Latest loaded preference and reception failure.
    pub updates: watch::Sender<ReceiverView>,
    /// Ordered entry calls; completion capture never consumes these.
    pub calls: tokio::sync::mpsc::UnboundedSender<EntryCall>,
    /// Renderer wake handle, attached after startup.
    pub context: Arc<OnceLock<eframe::egui::Context>>,
}

impl Outputs {
    /// Replace current receiver view and wake an attached native window.
    pub fn publish(&self, view: ReceiverView) {
        self.updates.send_replace(view);
        if let Some(context) = self.context.get() {
            context.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests;

pub(crate) mod contact;
pub(crate) mod pairing;
pub(crate) mod protocol;

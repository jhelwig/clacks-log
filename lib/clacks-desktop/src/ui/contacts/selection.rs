//! Cancellable history selection preparation, separate from contact editing.

use super::*;

/// One Select all intent and its original view, applied filter and selected identities.
pub(super) struct PendingSelection {
    /// Scope at command admission.
    view: SelectedView,
    /// Applied expression, independent of unfinished/desired text.
    expression: String,
    /// Selection before enumeration, including hidden contacts.
    selected: BTreeSet<QsoId>,
    /// Dropping the receiver cancels the ordinary read worker.
    reply: oneshot::Receiver<SettingsResponse>,
}

impl ContactsUi {
    /// Start Select all without publishing a partial selection or blocking redraws.
    pub fn select_all(&mut self, requests: &mpsc::Sender<SettingsRequest>) {
        if self.selection.is_some() {
            return;
        }
        let expression = self.filter.state.applied.clone();
        let parsed = clacks_model::recognition::RecognitionCatalog::adif_318()
            .map_err(anyhow::Error::from)
            .and_then(|catalog| {
                clacks_model::query::parse(&expression, catalog).map_err(anyhow::Error::from)
            });
        let filter = match parsed {
            Ok(filter) => filter,
            Err(error) => {
                self.notice = Some(format!("Cannot select contacts: {error}"));
                self.problem = true;
                return;
            }
        };
        let query = ContactPageQuery {
            view: self.query.view,
            filter,
            ..Default::default()
        };
        let (reply, receiver) = oneshot::channel();
        match requests.try_send(SettingsRequest {
            action: SettingsAction::Contacts(ContactAction::SelectAll { query }),
            reply,
        }) {
            Ok(()) => {
                self.selection = Some(PendingSelection {
                    view: self.query.view,
                    expression,
                    selected: self.selected.clone(),
                    reply: receiver,
                })
            }
            Err(error) => {
                self.notice = Some(format!("Could not select contacts: {error}"));
                self.problem = true;
            }
        }
    }

    /// Selection consumers must wait until the complete new set has been acknowledged.
    pub fn selection_pending(&self) -> bool {
        self.selection.is_some()
    }

    /// Admit history commands only for this table's actual native focus owner.
    pub fn history_has_focus(&self, context: &egui::Context) -> bool {
        self.history_focus
            .is_some_and(|id| context.memory(|memory| memory.focused() == Some(id)))
            && self.filter.desired.is_some()
    }

    /// Settle only the original Select all intent; newer edits keep their own selection.
    pub fn poll_selection(&mut self) {
        let Some(pending) = &mut self.selection else {
            return;
        };
        if pending.view != self.query.view
            || pending.expression != self.filter.state.applied
            || pending.selected != self.selected
        {
            self.selection = None;
            return;
        }
        match pending.reply.try_recv() {
            Ok(SettingsResponse::Contacts(ContactResponse::SelectedAll(ids))) => {
                self.selected.extend(ids);
                self.selection = None;
                self.dirty = true;
            }
            Ok(response) => {
                self.notice = Some(match response {
                    SettingsResponse::Failed(message) => message,
                    _ => "Contact selection ended without a usable result.".into(),
                });
                self.problem = true;
                self.selection = None;
            }
            Err(oneshot::error::TryRecvError::Closed) => {
                self.notice = Some("Contact selection ended without acknowledgment.".into());
                self.problem = true;
                self.selection = None;
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
        }
    }
}

//! Retain operation outcomes without confusing committed effects with view refreshes.

use super::*;

impl ContactsUi {
    /// Poll only the actual originating page/operation receivers.
    pub(super) fn poll(&mut self) {
        if let Some(pending) = &mut self.page_request {
            match pending.receiver.try_recv() {
                Ok(response) => {
                    let query = pending.query.clone();
                    let expression = pending.expression.clone();
                    self.page_request = None;
                    if query == self.query && !self.dirty {
                        if matches!(
                            &response,
                            SettingsResponse::Contacts(ContactResponse::Page(_))
                        ) {
                            self.filter.applied(expression);
                        }
                        self.response(response);
                    }
                }
                Err(oneshot::error::TryRecvError::Empty) => {}
                Err(oneshot::error::TryRecvError::Closed) => {
                    self.page_request = None;
                    self.notice = Some(
                        "Contact list read ended without a response. Refresh contacts to retry."
                            .into(),
                    );
                }
            }
        }
        if let Some(pending) = &mut self.operation {
            match pending.try_recv() {
                Ok(response) => {
                    self.operation = None;

                    self.response(response);
                }
                Err(oneshot::error::TryRecvError::Empty) => {}
                Err(oneshot::error::TryRecvError::Closed) => {
                    self.operation = None;

                    self.notice = Some("The operation ended without acknowledgment. Reload current contact before retrying.".into());
                }
            }
        }
    }

    /// Unwrap refresh errors while preserving the authoritative domain result.
    fn response(&mut self, response: SettingsResponse) {
        self.problem = true;
        match response {
            SettingsResponse::Contacts(value) => self.accept(value),
            SettingsResponse::RefreshFailed { outcome, message } => {
                self.response(*outcome);
                self.problem = true;
                self.notice = Some(message);
            }
            SettingsResponse::Failed(message) => self.notice = Some(message),
            SettingsResponse::Closing => {
                self.notice = Some("The workspace is closing. No further work was admitted.".into())
            }
            _ => self.notice = Some("Unexpected contact response; refresh before retrying.".into()),
        }
    }

    /// Apply actual stored outcomes without discarding stale editor input.
    pub(super) fn accept(&mut self, response: ContactResponse) {
        self.problem = false;
        match response {
            ContactResponse::QueryState(_) | ContactResponse::QueryStateSaved(_) => {
                self.notice = Some("Unexpected history preference response".into());
            }
            ContactResponse::Page(page) => self.page = Some(page),
            ContactResponse::Loaded(Some(record)) => {
                match editor::ContactEditor::from_record(record) {
                    Ok(value) => {
                        self.editor = Some(value);
                        self.current_record = None;
                        self.notice = None;
                    }
                    Err(error) => self.notice = Some(error.to_string()),
                }
            }
            ContactResponse::Loaded(None) => {
                self.notice =
                    Some("This contact is no longer available locally. Refresh contacts.".into())
            }
            ContactResponse::Saved(SaveOutcome::Saved(_) | SaveOutcome::Unchanged(_)) => {
                self.editor = None;
                self.current_record = None;
                self.refresh(self.query.view);
                self.notice = Some("Changes saved locally".into());
            }
            ContactResponse::Deleted(SaveOutcome::Saved(_) | SaveOutcome::Unchanged(_)) => {
                if let Some(deletion) = &self.deletion
                    && let clacks_sync::record::RecordId::Qso(id) = deletion.expected.key.id
                {
                    self.selected.remove(&id);
                }
                self.editor = None;
                self.deletion = None;
                self.current_record = None;
                self.refresh(self.query.view);
                self.notice = Some("Contact deleted across all logbooks".into());
            }
            ContactResponse::Saved(SaveOutcome::Stale { current })
            | ContactResponse::Deleted(SaveOutcome::Stale { current }) => {
                self.problem = true;
                self.current_record = current;
                self.deletion = None;
                self.notice = Some(if self.editor.is_some() { "The stored contact changed. Your edits are preserved; reload the current contact before saving again." } else { "The stored contact changed. The stale request made no changes." }.into());
            }
        }
    }
}

//! Settle the submitted contact independently from any later view refresh.

use super::*;
use crate::session::settings::contacts::ContactResponse;
use clacks_client_store::save::SaveOutcome;

impl LoggingUi {
    /// Preserve truthful save outcomes even when the subsequent refresh failed.
    pub(super) fn poll(&mut self) {
        let Some(receiver) = &mut self.pending else {
            return;
        };
        let result = match receiver.try_recv() {
            Ok(value) => value,
            Err(oneshot::error::TryRecvError::Empty) => return,
            Err(oneshot::error::TryRecvError::Closed) => {
                self.pending = None;
                self.freeze_after_failure();
                self.notice = Some(
                    "Save acknowledgment unavailable. Retry uses this contact's existing identity."
                        .into(),
                );
                return;
            }
        };
        self.pending = None;
        let (result, refresh) = match result {
            SettingsResponse::RefreshFailed { outcome, message } => (*outcome, Some(message)),
            other => (other, None),
        };
        match result {
            SettingsResponse::Contacts(ContactResponse::Saved(
                SaveOutcome::Saved(_) | SaveOutcome::Unchanged(_),
            )) => {
                self.entry
                    .reset_after_save(&self.active, self.view, time::now());
                self.baseline = self.entry.form.clone();
                self.submitted = None;
                self.focus_call = true;
                self.changed = true;
                self.validation.invalidate();
                self.problem = refresh.is_some();
                self.notice = Some(refresh.unwrap_or_else(|| "Saved locally".into()));
            }
            SettingsResponse::Failed(message) => {
                self.freeze_after_failure();
                self.notice = Some(message);
            }
            SettingsResponse::Closing => {
                self.freeze_after_failure();
                self.notice =
                    Some("The workspace is closing; the contact was not admitted.".into());
            }
            SettingsResponse::Contacts(ContactResponse::Saved(SaveOutcome::Stale { .. })) => {
                self.freeze_after_failure();
                self.notice = Some("This contact ID has newer stored information. Open it in history before retrying changes.".into());
            }
            _ => {
                self.freeze_after_failure();
                self.notice = Some("Unexpected contact response. Refresh before retrying.".into());
            }
        }
    }

    /// Preserve exact submitted timestamp input until the operator deliberately resumes clocks.
    fn freeze_after_failure(&mut self) {
        self.problem = true;
        self.entry.form.start.lock();
        self.entry.form.end.lock();
    }
}

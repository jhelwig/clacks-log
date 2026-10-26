//! Exact acknowledgments survive dialog dismissal and failed following reads.

use super::*;

impl LogbooksUi {
    /// Settle each admitted result once without reissuing its mutation.
    pub(super) fn poll(&mut self) {
        if let Some(response) = take_response(&mut self.selection_response) {
            self.selecting = None;
            self.accept(response, None);
        }
        if let Some(mut frame) = self.management.take() {
            if let Some(response) = take_response(&mut frame.response) {
                self.accept(response, Some(&mut frame));
            }
            self.management = Some(frame);
        }
        for mut frame in std::mem::take(&mut self.dismissed) {
            if let Some(response) = take_response(&mut frame.response) {
                self.accept(response, Some(&mut frame));
            }
            if frame.response.is_some() {
                self.dismissed.push(frame);
            } else if let Some(notice) = frame.notice {
                self.error = Some(notice);
            }
        }
    }

    /// Preserve domain results independently from a failed catalog refresh.
    fn accept(&mut self, response: SettingsResponse, mut frame: Option<&mut Management>) {
        let response = match response {
            SettingsResponse::RefreshFailed { outcome, message } => {
                self.error = Some(message);
                *outcome
            }
            response => response,
        };
        match response {
            SettingsResponse::Refresh(snapshot) => {
                self.observe(&snapshot.logbooks, None);
                self.acknowledged_view = None;
                self.acknowledged_tags.clear();
                self.deleted_tags.clear();
                self.error = None;
            }
            SettingsResponse::Logbooks(LogbookResponse::Selected(view)) => {
                self.acknowledged_view = Some(view)
            }
            SettingsResponse::Logbooks(LogbookResponse::Saved(
                SaveOutcome::Saved(record) | SaveOutcome::Unchanged(record),
            )) => {
                if let RecordId::Tag(id) = record.key.id {
                    self.acknowledged_tags.insert(id, record);
                }
                if let Some(frame) = &mut frame {
                    frame.form = None;
                    frame.notice = None;
                }
            }
            SettingsResponse::Logbooks(LogbookResponse::Saved(SaveOutcome::Stale { .. })) => {
                if let Some(frame) = &mut frame {
                    if let Some(form) = &mut frame.form {
                        form.stale = true;
                    }
                    frame.notice = Some(
                        "The logbook changed. No save was applied; reload and review it.".into(),
                    );
                }
            }
            SettingsResponse::Failed(message) => {
                if let Some(frame) = &mut frame {
                    frame.notice = Some(message);
                } else {
                    self.error = Some(message);
                }
            }
            SettingsResponse::Closing => {
                self.error = Some("The application is closing; this request did not start.".into())
            }
            _ => {
                self.error = Some("Unexpected logbook response. Refresh before continuing.".into())
            }
        }
    }
}

/// Read a response nonblockingly, retaining an admitted request until settlement.
fn take_response(
    receiver: &mut Option<oneshot::Receiver<SettingsResponse>>,
) -> Option<SettingsResponse> {
    let result = receiver.as_mut()?.try_recv();
    match result {
        Ok(value) => {
            *receiver = None;
            Some(value)
        }
        Err(oneshot::error::TryRecvError::Empty) => None,
        Err(oneshot::error::TryRecvError::Closed) => {
            *receiver = None;
            Some(SettingsResponse::Failed(
                "The request ended without acknowledgment. Refresh before retrying.".into(),
            ))
        }
    }
}

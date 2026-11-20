//! Settle logbook requests and distinguish committed edits from catalog-refresh failures.

use super::*;

impl LogbooksUi {
    /// Handle completed logbook requests without repeating their writes.
    pub(in crate::ui) fn poll(&mut self) {
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
    }

    /// Preserve the store result when the following catalog refresh fails.
    fn accept(&mut self, response: SettingsResponse, mut frame: Option<&mut Management>) {
        let response = match response {
            SettingsResponse::RefreshFailed { outcome, message } => {
                self.error = Some(message);
                *outcome
            }
            response => response,
        };
        match response {
            SettingsResponse::Logbooks(LogbookResponse::DeletionPreview(preview)) => {
                if let Some(frame) = &mut frame {
                    frame.form = None;
                    frame.deletion = Some(super::deletion::DeletionReview {
                        preview,
                        choice: None,
                        stale: false,
                        focus_cancel: true,
                    });
                    frame.notice = None;
                }
            }
            SettingsResponse::Logbooks(LogbookResponse::Deleted(
                clacks_client_store::logbooks::deletion::TagDeleteOutcome::Applied {
                    plan,
                    selected_view,
                    fallback_from,
                },
            )) => {
                self.acknowledged_view = Some(selected_view);
                if let Some(frame) = &mut frame {
                    if let Some(review) = frame.deletion.take()
                        && let RecordId::Tag(id) = review.preview.tag.key().id
                    {
                        self.deleted_tags.insert(id);
                        self.acknowledged_tags.remove(&id);
                    }
                    frame.form = None;
                    frame.notice = None;
                }
                self.completion_notice = Some(format!(
                    "Deleted {} contacts; removed this logbook from {} retained contacts.",
                    plan.mark_deleted.len(),
                    plan.remove_membership.len()
                ));
                if let Some(id) = fallback_from {
                    self.fallback(id);
                }
            }
            SettingsResponse::Logbooks(LogbookResponse::Deleted(
                clacks_client_store::logbooks::deletion::TagDeleteOutcome::Stale { .. },
            )) => {
                if let Some(frame) = &mut frame {
                    if let Some(review) = &mut frame.deletion {
                        review.stale = true;
                    }
                    frame.notice = Some("The logbook or its contacts changed. Nothing was deleted. Reload the deletion review and confirm again.".into());
                }
            }
            SettingsResponse::Refresh(snapshot) => {
                self.observe(&snapshot.logbooks, None);
                self.acknowledged_view = None;
                self.acknowledged_tags.clear();
                self.deleted_tags.clear();
                self.error = None;
            }
            SettingsResponse::Logbooks(LogbookResponse::Selected(view)) => {
                self.acknowledged_view = Some(view);
                self.fallback_notice = None;
            }
            SettingsResponse::Logbooks(LogbookResponse::Saved(outcome)) => match *outcome {
                SaveOutcome::Saved(record) | SaveOutcome::Unchanged(record) => {
                    if let RecordId::Tag(id) = record.key().id {
                        self.acknowledged_tags.insert(id, record);
                    }
                    if let Some(frame) = &mut frame {
                        frame.form = None;
                        frame.notice = None;
                    }
                }
                SaveOutcome::Stale { .. } => {
                    if let Some(frame) = &mut frame {
                        if let Some(form) = &mut frame.form {
                            form.stale = true;
                        }
                        frame.notice = Some(
                            "The logbook changed. No save was applied; reload and review it."
                                .into(),
                        );
                    }
                }
            },
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

/// Receive a completed request's result without blocking the UI.
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

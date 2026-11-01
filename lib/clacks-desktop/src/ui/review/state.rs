//! Request ownership and exact comparison refresh for the review window.

use super::ReviewUi;
use crate::{
    review::comparison::ComparisonBasis,
    session::settings::{
        SettingsAction, SettingsRequest, SettingsResponse,
        review::{ReviewAction, ReviewResponse},
    },
};
use clacks_client_store::{
    read::review::{ReviewQuery, ReviewSection},
    save::SaveOutcome,
    settings::SettingsSnapshot,
};
use clacks_sync::record::{RecordId, RecordState, RecordValue};
use tokio::sync::{mpsc, oneshot};

impl ReviewUi {
    /// Admit an operation without blocking the native UI thread.
    pub(super) fn request(
        &mut self,
        action: ReviewAction,
        requests: &mpsc::Sender<SettingsRequest>,
    ) -> bool {
        if self.reply.is_some() {
            return false;
        }
        let (reply, response) = oneshot::channel();
        match requests.try_send(SettingsRequest {
            action: SettingsAction::Review(action),
            reply,
        }) {
            Ok(()) => {
                self.reply = Some(response);
                self.notice = None;
                true
            }
            Err(_) => {
                self.notice = Some("The workspace is busy. Try again.".into());
                false
            }
        }
    }

    /// Request exactly the current section and type restriction.
    pub(super) fn read_page(
        &mut self,
        section: ReviewSection,
        after: Option<RecordId>,
        requests: &mpsc::Sender<SettingsRequest>,
    ) {
        self.request(
            ReviewAction::ReadPage {
                query: ReviewQuery {
                    section: Some(section),
                    kind: self.kind,
                    after,
                    limit: 100,
                },
            },
            requests,
        );
    }

    /// Receive an originating result and report whether the window needs repainting.
    pub(super) fn poll(&mut self) -> bool {
        let response = match self.reply.as_mut().map(oneshot::Receiver::try_recv) {
            Some(Ok(response)) => response,
            Some(Err(oneshot::error::TryRecvError::Closed)) => {
                self.reply = None;
                self.saving = None;
                self.notice = Some(
                    "The operation result is unavailable. Refresh current state before retrying."
                        .into(),
                );
                return true;
            }
            _ => return false,
        };
        self.reply = None;
        self.accept(response);
        true
    }

    /// Keep an authoritative committed outcome even when its subsequent read fails.
    fn accept(&mut self, response: SettingsResponse) {
        match response {
            SettingsResponse::Review(ReviewResponse::Page {
                query,
                page,
                settings,
            }) if query.kind == self.kind => {
                let section = match query.section {
                    Some(ReviewSection::Conflicts) => &mut self.conflicts,
                    Some(ReviewSection::PendingSync) => &mut self.pending_sync,
                    None => return,
                };
                if query.after.is_none() {
                    section.rows.clear();
                }
                for row in page.rows {
                    if let Some(existing) = section
                        .rows
                        .iter_mut()
                        .find(|old| old.record.key() == row.record.key())
                    {
                        *existing = row;
                    } else {
                        section.rows.push(row);
                    }
                }
                section.next = page.next;
                self.counts = Some(page.counts);
                self.catalog(settings);
            }
            SettingsResponse::Review(ReviewResponse::Record { key, row, settings }) => {
                if let Some(row) = &row {
                    if let Some(review) = &row.review {
                        let basis = ComparisonBasis {
                            record: row.record.clone(),
                            review: review.clone(),
                        };
                        if let Some(editor) = self.drafts.get_mut(&key) {
                            if self.refresh_for == Some(key) {
                                if let Err(error) = editor.draft.refresh(basis) {
                                    self.notice = Some(error.to_string());
                                }
                                editor.invalidate();
                            } else {
                                editor.draft.observe(&basis);
                            }
                        } else if self.open
                            && self.selected == Some(key)
                            && matches!(
                                row.record.key().id,
                                RecordId::Qso(_)
                                    | RecordId::Profile(_)
                                    | RecordId::Equipment(_)
                                    | RecordId::Tag(_)
                            )
                            && (matches!(basis.review.local, RecordState::Live(_))
                                || matches!(&basis.review.shared, clacks_sync::record::Observation::Accepted(value) if matches!(value.state(), RecordState::Live(_)))
                                || basis
                                    .review
                                    .retained
                                    .iter()
                                    .any(|value| matches!(value.state(), RecordState::Live(_))))
                        {
                            match super::comparison::ReviewEditor::new(basis) {
                                Ok(editor) => {
                                    self.drafts.insert(key, editor);
                                }
                                Err(error) => self.notice = Some(error.to_string()),
                            }
                        }
                    } else if let Some(editor) = self.drafts.get_mut(&key) {
                        editor.settled = true;
                    }
                } else if let Some(editor) = self.drafts.get_mut(&key) {
                    editor.settled = true;
                }
                if self.refresh_for == Some(key) {
                    self.refresh_for = None;
                }
                if self.selected == Some(key) {
                    self.detail = row.map(|row| *row);
                    self.catalog(settings);
                }
            }
            SettingsResponse::Review(ReviewResponse::References {
                key,
                after,
                mut page,
            }) => {
                if self.selected == Some(key) {
                    if after.is_some()
                        && let Some(existing) = self.references.take()
                        && existing.key == key
                    {
                        let mut rows = existing.page.rows;
                        for row in page.rows {
                            if let Some(old) = rows
                                .iter_mut()
                                .find(|old| old.record.key() == row.record.key())
                            {
                                *old = row;
                            } else {
                                rows.push(row);
                            }
                        }
                        page.rows = rows;
                    }
                    self.references = Some(super::references::References { key, page });
                }
            }
            SettingsResponse::Review(ReviewResponse::Retried { key, outcome }) => {
                self.saving = None;
                use clacks_client_store::synchronization::RetryDeletionOutcome;
                match *outcome {
                    RetryDeletionOutcome::Ready { record } => {
                        self.accept(SettingsResponse::Review(ReviewResponse::Resolved {
                            key,
                            outcome: Box::new(SaveOutcome::Saved(*record)),
                        }))
                    }
                    RetryDeletionOutcome::StillReferenced { page } => {
                        if self.selected == Some(key) {
                            self.references = Some(super::references::References { key, page });
                        }
                        self.notice =
                            Some("Deletion remains blocked by accepted referring records.".into());
                    }
                    RetryDeletionOutcome::Stale { current } => {
                        self.accept(SettingsResponse::Review(ReviewResponse::Resolved {
                            key,
                            outcome: Box::new(SaveOutcome::Stale {
                                current: current.map(|row| row.record),
                            }),
                        }))
                    }
                }
            }
            SettingsResponse::Review(ReviewResponse::Resolved { key, outcome }) => {
                self.saving = None;
                match *outcome {
                    SaveOutcome::Saved(record) | SaveOutcome::Unchanged(record) => {
                        self.drafts.remove(&key);
                        if self.selected == Some(key) {
                            self.detail = Some(clacks_client_store::read::review::ReviewRow {
                                record,
                                review: None,
                                authorization_hold: None,
                            });
                        }
                        self.notice =
                            Some("Resolution saved locally; pending synchronization.".into());
                        self.invalidate();
                    }
                    SaveOutcome::Stale { .. } => {
                        if let Some(editor) = self.drafts.get_mut(&key) {
                            editor.draft.require_refresh();
                        }
                        self.notice = Some("The comparison changed; your draft is preserved. Refresh it before saving.".into());
                        self.detail_dirty = true;
                    }
                }
            }
            SettingsResponse::RefreshFailed { outcome, message } => {
                self.accept(*outcome);
                self.notice = Some(message);
            }
            SettingsResponse::Failed(message) => {
                self.saving = None;
                self.notice = Some(message);
            }
            SettingsResponse::Closing => {
                self.saving = None;
                self.notice = Some("The workspace is closing.".into());
            }
            _ => {}
        }
    }

    /// Replace catalog labels and invalidate the selected editor's normal validation.
    fn catalog(&mut self, settings: Box<SettingsSnapshot>) {
        let tags = settings
            .logbooks
            .tags
            .iter()
            .filter_map(|row| match row.working_state() {
                Some(RecordState::Live(RecordValue::Tag(tag))) => Some(tag.as_ref()),
                _ => None,
            })
            .collect::<Vec<_>>();
        self.tag_labels = crate::ui::logbooks::labels::labels(&tags);
        if self.settings.as_ref() != Some(&settings) {
            for editor in self.drafts.values_mut() {
                editor.invalidate();
            }
        }
        self.settings = Some(settings);
    }
}

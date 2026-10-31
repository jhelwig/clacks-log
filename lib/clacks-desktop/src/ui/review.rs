//! Non-modal account-wide pending-work presentation on the normal session path.

#[cfg(test)]
mod tests;

use super::{feedback, theme};
use crate::session::settings::{
    SettingsAction, SettingsRequest, SettingsResponse,
    review::{ReviewAction, ReviewResponse},
};
use clacks_client_store::{
    read::review::{RecordKind, ReviewCounts, ReviewQuery, ReviewRow, ReviewSection},
    settings::SettingsSnapshot,
};
use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};
use eframe::egui;
use tokio::sync::{mpsc, oneshot};

/// One independently continued section, retaining already loaded native rows.
#[derive(Default)]
struct Section {
    /// Loaded identities; drafts will remain separate from this page cache.
    rows: Vec<ReviewRow>,
    /// Exclusive continuation when more matching records exist.
    next: Option<RecordId>,
}

/// Window-owned selection and requests, independent of logging inputs.
#[derive(Default)]
pub(super) struct ReviewUi {
    /// Whether its native viewport should be shown.
    open: bool,
    /// Current record-type restriction; no logbook scope applies.
    kind: Option<RecordKind>,
    /// Loaded review-required page.
    conflicts: Section,
    /// Loaded ordinary unpublished page.
    pending_sync: Section,
    /// Counts from the last consistent read.
    counts: Option<ReviewCounts>,
    /// Current record identity, stable across section movement.
    selected: Option<RecordKey>,
    /// Exact comparison and catalogs currently displayed.
    detail: Option<ReviewRow>,
    /// Source definitions and available reusable records for the selected editor.
    settings: Option<Box<SettingsSnapshot>>,
    /// Catalog-based captions, disambiguating duplicate names only.
    tag_labels: std::collections::BTreeMap<clacks_model::ids::TagId, String>,
    /// Exact selected detail needs a fresh bounded read.
    detail_dirty: bool,
    /// Originating request receiver, never a replacement request's result.
    reply: Option<oneshot::Receiver<SettingsResponse>>,
    /// Sections awaiting bounded rereads.
    refresh: Vec<ReviewSection>,
    /// Actual operation or read diagnostic.
    notice: Option<String>,
}

impl ReviewUi {
    /// Offer account-wide review without replacing the selected history or entry.
    pub fn action(&mut self, ui: &mut egui::Ui) {
        if ui.button("Unsynchronized changes").clicked() {
            self.open = true;
            self.invalidate();
        }
    }

    /// Request bounded fresh pages after committed workspace changes.
    pub fn invalidate(&mut self) {
        if self.open {
            self.detail_dirty = true;
            self.refresh = vec![ReviewSection::PendingSync, ReviewSection::Conflicts];
        }
    }

    /// Draw a separately usable native window using standard viewport ownership.
    pub fn show_window(
        &mut self,
        context: &egui::Context,
        requests: &mpsc::Sender<SettingsRequest>,
    ) {
        self.poll();
        if !self.open {
            return;
        }
        context.show_viewport_immediate(
            egui::ViewportId::from_hash_of("clacks.unsynchronized_changes"),
            egui::ViewportBuilder::default()
                .with_title("Clacks Log · Unsynchronized changes")
                .with_inner_size([1120.0, 720.0]),
            |ui, _class| {
                if ui.input(|input| input.viewport().close_requested()) {
                    self.close();
                }
                egui::CentralPanel::default()
                    .frame(theme::section_frame(ui))
                    .show(ui, |ui| {
                        self.show_contents(ui, requests);
                    });
            },
        );
        if self.reply.is_none()
            && let Some(section) = self.refresh.pop()
        {
            self.read_page(section, None, requests);
        }
        if self.reply.is_none()
            && self.detail_dirty
            && let Some(key) = self.selected
        {
            self.detail_dirty = false;
            self.request(ReviewAction::ReadRecord { key }, requests);
        }
        if self.reply.is_some() {
            context.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }

    /// Render the actual list/detail controls independently of viewport creation.
    fn show_contents(&mut self, ui: &mut egui::Ui, requests: &mpsc::Sender<SettingsRequest>) {
        ui.horizontal_wrapped(|ui| {
            ui.heading("Unsynchronized changes");
            ui.label("Account-wide");
            let previous = self.kind;
            egui::ComboBox::from_label("Record type")
                .selected_text(self.kind.map(kind_label).unwrap_or("All types"))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.kind, None, "All types");
                    for kind in [
                        RecordKind::Qso,
                        RecordKind::Tag,
                        RecordKind::Profile,
                        RecordKind::Equipment,
                    ] {
                        ui.selectable_value(&mut self.kind, Some(kind), kind_label(kind));
                    }
                });
            if self.kind != previous {
                self.conflicts = Section::default();
                self.pending_sync = Section::default();
                self.refresh = vec![ReviewSection::PendingSync, ReviewSection::Conflicts];
            }
            if ui.button("Refresh").clicked() {
                self.invalidate();
            }
            if ui.button("Close").clicked() {
                self.close();
            }
        });
        if let Some(notice) = &self.notice {
            feedback::problem(ui, notice);
        }
        ui.separator();
        if ui.available_width() < 720.0 {
            self.show_list(ui, requests);
            ui.separator();
            self.show_detail(ui);
        } else {
            egui::Panel::left("review.records")
                .default_size(290.0)
                .resizable(true)
                .frame(theme::section_frame(ui))
                .show(ui, |ui| self.show_list(ui, requests));
            egui::CentralPanel::default()
                .frame(theme::section_frame(ui))
                .show(ui, |ui| self.show_detail(ui));
        }
    }

    /// Render independently expandable lists with their own bounded continuations.
    fn show_list(&mut self, ui: &mut egui::Ui, requests: &mpsc::Sender<SettingsRequest>) {
        egui::ScrollArea::vertical()
            .id_salt("review.sections")
            .show(ui, |ui| {
                for section in [ReviewSection::Conflicts, ReviewSection::PendingSync] {
                    let (title, count, rows, next) = match section {
                        ReviewSection::Conflicts => (
                            "Conflicts",
                            self.counts.as_ref().map(|c| c.conflicts),
                            &self.conflicts.rows,
                            self.conflicts.next,
                        ),
                        ReviewSection::PendingSync => (
                            "Pending sync",
                            self.counts.as_ref().map(|c| c.pending_sync),
                            &self.pending_sync.rows,
                            self.pending_sync.next,
                        ),
                    };
                    let heading = count
                        .map(|n| format!("{title} ({n})"))
                        .unwrap_or_else(|| title.into());
                    let mut select = None;
                    let mut more = false;
                    egui::CollapsingHeader::new(heading)
                        .id_salt(title)
                        .default_open(true)
                        .show(ui, |ui| {
                            for row in rows {
                                let key = row.record.key();
                                if ui
                                    .selectable_label(
                                        self.selected == Some(key),
                                        caption(row, &self.tag_labels),
                                    )
                                    .on_hover_text(key.id.to_string())
                                    .clicked()
                                {
                                    select = Some(key);
                                }
                            }
                            if rows.is_empty() {
                                ui.weak("No records in this section.");
                            }
                            if next.is_some() {
                                more = ui
                                    .add_enabled(
                                        self.reply.is_none(),
                                        egui::Button::new(match section {
                                            ReviewSection::Conflicts => "Load more conflicts",
                                            ReviewSection::PendingSync => {
                                                "Load more pending changes"
                                            }
                                        }),
                                    )
                                    .clicked();
                            }
                        });
                    if let Some(key) = select {
                        self.selected = Some(key);
                        self.detail = None;
                        self.detail_dirty = true;
                    } else if more {
                        self.read_page(section, next, requests);
                    }
                }
            });
    }

    /// Identify real local work without presenting absent evidence as a deletion.
    fn show_detail(&mut self, ui: &mut egui::Ui) {
        let Some(row) = &self.detail else {
            ui.label("Select a record to review.");
            return;
        };
        ui.heading(caption(row, &self.tag_labels));
        ui.label(format!("ID: {}", row.record.key().id));
        if let Some(review) = &row.review {
            ui.label(reason_label(&review.reason));
        } else if row.authorization_hold.is_some() || !row.record.retained().is_empty() {
            ui.label("Waiting for comparison data. Local work is preserved.");
        } else {
            ui.label(
                if matches!(row.record.working_state(), Some(RecordState::Deleted)) {
                    "Deletion saved locally; pending synchronization."
                } else {
                    "Saved locally; pending synchronization."
                },
            );
        }
    }

    /// Close only this review window, leaving committed work and logging untouched.
    fn close(&mut self) {
        self.open = false;
        self.detail = None;
        self.settings = None;
    }

    /// Admit a bounded read without blocking the egui thread.
    fn request(&mut self, action: ReviewAction, requests: &mpsc::Sender<SettingsRequest>) {
        if self.reply.is_some() {
            return;
        }
        let (reply, response) = oneshot::channel();
        match requests.try_send(SettingsRequest {
            action: SettingsAction::Review(action),
            reply,
        }) {
            Ok(()) => {
                self.reply = Some(response);
                self.notice = None;
            }
            Err(_) => self.notice = Some("The workspace is busy. Try again.".into()),
        }
    }

    /// Request a page for exactly the current section and type filter.
    fn read_page(
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

    /// Accept the originating reply while ignoring obsolete view selections.
    fn poll(&mut self) {
        let response = match self.reply.as_mut().map(oneshot::Receiver::try_recv) {
            Some(Ok(response)) => response,
            Some(Err(oneshot::error::TryRecvError::Closed)) => {
                self.reply = None;
                self.notice = Some("The review read stopped; refresh to retry.".into());
                return;
            }
            _ => return,
        };
        self.reply = None;
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
                section.rows.extend(page.rows);
                section.next = page.next;
                self.counts = Some(page.counts);
                self.catalog(settings);
            }
            SettingsResponse::Review(ReviewResponse::Record { key, row, settings })
                if self.selected == Some(key) =>
            {
                self.detail = row.map(|value| *value);
                self.catalog(settings);
            }
            SettingsResponse::Failed(message) => self.notice = Some(message),
            SettingsResponse::Closing => self.notice = Some("The workspace is closing.".into()),
            _ => {}
        }
    }

    /// Replace names and definitions from a completed consistent source read.
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
        self.tag_labels = super::logbooks::labels::labels(&tags);
        self.settings = Some(settings);
    }
}

/// Produce an operator-facing caption from real working or retained facts.
fn caption(
    row: &ReviewRow,
    tags: &std::collections::BTreeMap<clacks_model::ids::TagId, String>,
) -> String {
    let value = row
        .record
        .working_state()
        .filter(|state| matches!(state, RecordState::Live(_)))
        .or_else(|| row.record.pending().and_then(|p| p.original.state()))
        .or_else(|| match row.record.latest() {
            clacks_sync::record::Observation::Accepted(v) => Some(v.state()),
            _ => None,
        });
    match value {
        Some(RecordState::Live(RecordValue::Qso(qso))) => format!(
            "{} · {}",
            qso.contacted_station().callsign.as_str(),
            crate::logging::time::format(qso.start())
        ),
        Some(RecordState::Live(RecordValue::Tag(tag))) => {
            format!("Logbook · {}", tags.get(&tag.id).unwrap_or(&tag.name))
        }
        Some(RecordState::Live(RecordValue::Profile(profile))) => {
            format!("Station profile · {}", profile.name())
        }
        Some(RecordState::Live(RecordValue::Equipment(equipment))) => {
            format!("Equipment · {}", equipment.name())
        }
        _ => row.record.key().id.to_string(),
    }
}

/// Explain the domain reason without exposing implementation variant names.
fn reason_label(reason: &clacks_sync::client::ReviewReason) -> &'static str {
    use clacks_sync::client::ReviewReason;
    match reason {
        ReviewReason::BothDifferent => "Local and shared versions changed differently.",
        ReviewReason::MissingOriginal => {
            "Original values are unavailable; change attribution cannot be shown."
        }
        ReviewReason::OrderingUnknown(_) => {
            "Available ordering evidence cannot establish which version follows the other."
        }
        ReviewReason::MissingReference(_) => {
            "A referenced logbook or equipment configuration is unavailable."
        }
        ReviewReason::Referenced { .. } => "Surviving references prevent this deletion.",
        ReviewReason::ExcludedActor(_) => {
            "Work from the previous client authorization requires explicit review."
        }
    }
}

/// Name the existing operator-editable record categories.
fn kind_label(kind: RecordKind) -> &'static str {
    match kind {
        RecordKind::Qso => "QSO",
        RecordKind::Tag => "Logbook",
        RecordKind::Profile => "Station profile",
        RecordKind::Equipment => "Equipment",
        _ => "Other record",
    }
}

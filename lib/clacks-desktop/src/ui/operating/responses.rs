//! Typed outcomes stay attached to the originating screen and never retry writes implicitly.

use super::*;
use clacks_client_store::{operating::OperatingSaveOutcome, settings::EquipmentDeleteOutcome};

impl SettingsUi {
    /// Settle admitted requests, including dialogs the operator already dismissed.
    pub(in crate::ui) fn poll(&mut self) {
        let dismissed = std::mem::take(&mut self.dismissed);
        for mut frame in dismissed {
            self.poll_frame(&mut frame);
            if frame.response.is_some() {
                self.dismissed.push(frame);
            } else if let Some(notice) = frame.notice {
                self.completion_notice = Some(notice);
            }
        }
        let Some(mut frame) = self.stack.pop() else {
            return;
        };
        let result = self.poll_frame(&mut frame);
        if !result.complete || self.stack.is_empty() {
            self.stack.push(frame);
        } else if let Some(opener) = frame.opener {
            self.restore_focus = Some(opener);
        }
        if let Some(screen) = result.next {
            self.stack.push(Frame::new(screen));
        }
    }

    /// Preserve each typed acknowledgment without retrying an admitted write.
    fn poll_frame(&mut self, frame: &mut Frame) -> PollResult {
        let mut pop = false;
        let mut deleted = None;
        let mut next = None;
        let Some(receiver) = &mut frame.response else {
            return PollResult::default();
        };
        let response = match receiver.try_recv() {
            Ok(response) => response,
            Err(oneshot::error::TryRecvError::Empty) => return PollResult::default(),
            Err(oneshot::error::TryRecvError::Closed) => {
                frame.response = None;
                frame.notice = Some("The background request ended without an acknowledgment. Refresh before retrying.".into());
                return PollResult::default();
            }
        };
        frame.response = None;
        let refresh_failed = matches!(&response, SettingsResponse::RefreshFailed { .. });
        let response = if let SettingsResponse::RefreshFailed { outcome, message } = response {
            self.refresh_error = Some(message);
            *outcome
        } else {
            if !matches!(
                response,
                SettingsResponse::Failed(_) | SettingsResponse::Closing
            ) {
                self.refresh_error = None;
            }
            response
        };
        match response {
            SettingsResponse::Logbooks(_) | SettingsResponse::Contacts(_) => {},
            SettingsResponse::Refresh(_) => frame.notice = None,
            SettingsResponse::Apply(OperatingSaveOutcome::Saved(state) | OperatingSaveOutcome::Unchanged(state)) => {
                self.acknowledged = Some(state);
                pop = true;
            },
            SettingsResponse::Apply(OperatingSaveOutcome::Stale(current)) => {
                frame.stale_operating = Some(current);
                frame.notice = Some("Applied settings changed while you edited. Your text is preserved. Review the current applied state before retrying.".into());
            }
            SettingsResponse::SaveProfile(result) => {
                match result.outcome {
                    SaveOutcome::Saved(record) | SaveOutcome::Unchanged(record) => {
                        if refresh_failed {
                            self.acknowledged_records.insert(record.key().id, record.clone());
                            self.completion_notice = Some("Profile saved locally. Refresh settings to recover the remaining view; do not repeat the save.".into());
                        }
                        if result.missing_equipment.is_some() {
                            if let Screen::Profile(editor) = &mut frame.screen { editor.expected = Some(record); }
                            frame.notice = Some("Profile saved locally with a missing equipment reference. Choose a replacement or None and save again; it has not been applied.".into());
                        } else { pop = true; }
                    }
                    SaveOutcome::Stale { current } => stale(frame, current),
                }
            }
            SettingsResponse::SaveEquipment(outcome) => match outcome {
                SaveOutcome::Saved(record) | SaveOutcome::Unchanged(record) => {
                    if refresh_failed {
                        self.acknowledged_records.insert(record.key().id, record);
                        self.completion_notice = Some("Equipment saved locally. Refresh settings to recover the remaining view; do not repeat the save.".into());
                    }
                    pop = true;
                },
                SaveOutcome::Stale { current } => stale(frame, current),
            },
            SettingsResponse::DeleteProfile(outcome) => match outcome {
                SaveOutcome::Saved(record) | SaveOutcome::Unchanged(record) => { deleted = Some(record.key().id); pop = true; }
                SaveOutcome::Stale { current } => stale(frame, current),
            },
            SettingsResponse::PreviewEquipmentDeletion(preview) => next = Some(Screen::DeleteEquipment(Box::new(DeletionReview { preview, choices: BTreeMap::new() }))),
            SettingsResponse::DeleteEquipment(EquipmentDeleteOutcome::Applied) => {
                if let Screen::DeleteEquipment(review) = &frame.screen { deleted = Some(review.preview.equipment.key().id); }
                pop = true;
            },
            SettingsResponse::DeleteEquipment(EquipmentDeleteOutcome::Stale(preview)) => {
                if let Screen::DeleteEquipment(review) = &mut frame.screen {
                    review.choices.retain(|id, choice| preview.referring_profiles.iter().filter_map(profile).any(|p| p.id() == *id) && choice.is_none_or(|id| preview.replacement_equipment.iter().filter_map(configuration).any(|e| e.id() == id)));
                    review.preview = preview;
                }
                frame.notice = Some("The deletion review changed; nothing was deleted. Review every current profile and confirm again.".into());
            }
            SettingsResponse::DeleteEquipment(EquipmentDeleteOutcome::Unavailable { .. }) => frame.notice = Some("The configuration is no longer available. No deletion or profile repair was applied. Cancel and refresh.".into()),
            SettingsResponse::Failed(message) => frame.notice = Some(message),
            SettingsResponse::Closing => frame.notice = Some("The application is closing; this request did not start.".into()),
            SettingsResponse::Review(_) => frame.notice = Some("Unexpected review response in settings.".into()),
            SettingsResponse::RefreshFailed { .. } => frame.notice = Some("A refresh failed. Refresh settings before continuing.".into()),
        }
        if let Some(id) = deleted {
            self.known_deletions.insert(id);
            for caller in &mut self.stack {
                if let Some(editor) = caller.screen.operating_mut() {
                    editor.forget_source(id);
                }
            }
        }
        PollResult {
            complete: pop,
            next,
        }
    }
}

/// Navigation consequences of settling one originating request.
#[derive(Default)]
struct PollResult {
    /// The visible form has completed successfully.
    complete: bool,
    /// A preview result opens its explicit review screen.
    next: Option<Screen>,
}

/// Retain typed values and the competing saved version for explicit user review.
fn stale(frame: &mut Frame, current: Option<ClientRecord>) {
    frame.stale = Some(current);
    frame.notice = Some("The saved target changed; nothing was overwritten. Your text is preserved. Review the saved version or cancel.".into());
}

/// Allow a deliberate retry only after presenting the actual competing saved facts.
pub(super) fn stale_review(ui: &mut egui::Ui, frame: &mut Frame) {
    if let Some(current) = &frame.stale {
        ui.heading("Current saved version");
        match current.as_ref().and_then(ClientRecord::working_state) {
            Some(RecordState::Live(RecordValue::Profile(p))) => {
                ui.label(profile_label(p));
                let mut form = crate::operating::station::StationForm::from_snapshot(p.station());
                ui.add_enabled_ui(false, |ui| {
                    fields::station(
                        ui,
                        "stale.profile",
                        &mut form,
                        &DefinitionSet::default(),
                        &[],
                        None,
                    )
                });
                ui.label(format!("Default equipment: {:?}", p.default_equipment()));
                if ui
                    .button("Keep my values against this saved version")
                    .clicked()
                {
                    if let Screen::Profile(editor) = &mut frame.screen
                        && current.as_ref().is_some_and(|r| {
                            r.key().id == clacks_sync::record::RecordId::Profile(editor.id)
                        })
                    {
                        editor.expected = current.clone();
                    }
                    frame.stale = None;
                }
            }
            Some(RecordState::Live(RecordValue::Equipment(e))) => {
                ui.label(equipment_label(e));
                let mut form =
                    crate::operating::equipment::EquipmentForm::from_values(Some(e.values()));
                ui.add_enabled_ui(false, |ui| {
                    fields::equipment(ui, "stale.equipment", &mut form, &[], None)
                });
                if ui
                    .button("Keep my values against this saved version")
                    .clicked()
                {
                    if let Screen::EquipmentEdit(editor) = &mut frame.screen
                        && current.as_ref().is_some_and(|r| {
                            r.key().id == clacks_sync::record::RecordId::Equipment(editor.id)
                        })
                    {
                        editor.expected = current.clone();
                    }
                    frame.stale = None;
                }
            }
            _ => {
                ui.label("This saved target is deleted or unavailable. Choose Create new to save a distinct record, or cancel.");
            }
        }
    }
    if let Some(current) = &frame.stale_operating {
        ui.heading("Current applied settings to review");
        fields::active_summary(ui, current);
        if ui
            .button("Keep my pending values against these applied settings")
            .clicked()
        {
            if let Some(editor) = frame.screen.operating_mut() {
                editor.expected = current.clone();
            }
            frame.stale_operating = None;
        }
    }
}

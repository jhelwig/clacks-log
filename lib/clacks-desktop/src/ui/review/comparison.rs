//! Field comparison and per-record editor state for the non-modal review window.

use super::{ReviewUi, caption, reason_label};
use crate::{
    review::{
        comparison::{ComparisonBasis, ComparisonSource},
        draft::ResolutionDraft,
        fields::{FieldId, QsoField},
        input::FieldInput,
    },
    ui::{
        actions, feedback,
        tag_picker::{self, MembershipPicker},
        validation::ValidationState,
    },
};
use clacks_client_store::{read::review::ReviewRow, settings::SettingsSnapshot};
use clacks_model::{group::DefinitionSet, ids::TagId};
use clacks_sync::record::{Observation, RecordState};
use eframe::egui;
use std::collections::{BTreeMap, BTreeSet};

/// One visited conflict's unsaved choices and standard native validation state.
pub(super) struct ReviewEditor {
    /// Exact native comparison and candidate facts.
    pub draft: ResolutionDraft,
    /// Shared debounce state, independent of network availability.
    validation: ValidationState,
    /// Last complete native validation result.
    errors: Vec<crate::operating::FieldError>,
    /// Include equal fields as context when explicitly requested.
    show_unchanged: bool,
    /// Searchable membership choices for this one resolved QSO.
    membership: Option<MembershipEditor>,
    /// Record has settled or become unavailable since opening this draft.
    pub settled: bool,
}

/// A single-target picker and the opening set needed for its explicit intent.
struct MembershipEditor {
    /// Shared catalog/search/control model, never a two-contact mixed selection.
    picker: MembershipPicker,
    /// Set used when calculating the picker's explicit changes.
    opening: BTreeSet<TagId>,
    /// Last set reflected in the picker, including explicit checkbox edits.
    current: BTreeSet<TagId>,
}

/// A user action; saving is executed through the existing session, not in rendering.
pub(super) enum ComparisonAction {
    /// No write or refresh was requested.
    None,
    /// Complete validated chosen record.
    Save { chosen: RecordState },
    /// Explicitly replace compared inputs while retaining the draft.
    Refresh,
}

impl ReviewEditor {
    /// Start a concrete editor only for a usable native comparison.
    pub fn new(basis: ComparisonBasis) -> anyhow::Result<Self> {
        Ok(Self {
            draft: ResolutionDraft::new(basis)?,
            validation: Default::default(),
            errors: vec![],
            show_unchanged: false,
            membership: None,
            settled: false,
        })
    }

    /// Revalidate after installing an explicitly refreshed comparison or definitions.
    pub fn invalidate(&mut self) {
        self.validation.invalidate();
    }

    /// Render genuine comparison sources and the direct resolved-value controls.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        settings: &SettingsSnapshot,
        tags: &BTreeMap<TagId, String>,
        busy: bool,
    ) -> ComparisonAction {
        let definitions = DefinitionSet::new(
            settings
                .groups
                .iter()
                .filter_map(|row| match row.working_state() {
                    Some(RecordState::Live(clacks_sync::record::RecordValue::Group(value))) => {
                        Some(value.as_ref().clone())
                    }
                    _ => None,
                })
                .collect(),
        );
        let definitions = match definitions {
            Ok(definitions) => definitions,
            Err(error) => {
                feedback::problem(ui, &format!("Cannot validate references: {error}"));
                return ComparisonAction::None;
            }
        };
        let now = ui.input(|input| input.time);
        if self.validation.due(now) {
            self.errors = self.draft.build(&definitions).err().unwrap_or_default();
            self.validation.validated();
        }
        self.validation.request_repaint(ui.ctx(), now);
        let mut action = ComparisonAction::None;
        if self.settled {
            feedback::problem(
                ui,
                "This conflict has settled or is no longer available. Your unfinished inputs are preserved; they have not been saved.",
            );
        } else if self.draft.is_stale() {
            feedback::problem(
                ui,
                "Newer values are available. Refresh the comparison before saving; your draft will be preserved.",
            );
            if ui
                .add_enabled(!busy, egui::Button::new("Refresh comparison"))
                .clicked()
            {
                action = ComparisonAction::Refresh;
            }
        }
        ui.add_enabled_ui(!busy && !self.settled, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (label, source) in [
                    ("Use all local", ComparisonSource::Local),
                    ("Use all shared", ComparisonSource::Shared),
                ] {
                    if ui
                        .add_enabled(
                            self.draft.basis.source(&source).is_ok(),
                            egui::Button::new(label),
                        )
                        .clicked()
                    {
                        match self.draft.select_all(source) {
                            Ok(()) => self.validation.edited(now),
                            Err(error) => {
                                self.errors = vec![crate::operating::issue("Comparison", error)]
                            }
                        }
                    }
                }
                ui.checkbox(&mut self.show_unchanged, "Show unchanged fields");
            });
            egui::ScrollArea::vertical()
                .id_salt("review.comparison.scroll")
                .show(ui, |ui| {
                    for field in self.draft.fields() {
                        if !self.show_unchanged && !self.draft.differs(field) {
                            continue;
                        }
                        ui.push_id(field.label(), |ui| {
                            let sources = self
                                .draft
                                .basis
                                .review
                                .retained
                                .iter()
                                .map(|value| ComparisonSource::Retained {
                                    stamp: value.stamp.clone(),
                                })
                                .collect::<Vec<_>>();
                            if ui.available_width() >= 650.0 {
                                ui.columns(4, |columns| {
                                    if let Some(column) = columns.first_mut() {
                                        column.strong(field.label());
                                        column.weak(self.draft.attribution(field));
                                    }
                                    for (column, source) in columns
                                        .iter_mut()
                                        .skip(1)
                                        .take(2)
                                        .zip([ComparisonSource::Local, ComparisonSource::Shared])
                                    {
                                        self.source(column, field, source, tags, &definitions, now);
                                    }
                                    if let Some(column) = columns.last_mut() {
                                        self.resolved(column, field, settings, &definitions, now);
                                    }
                                });
                            } else {
                                ui.strong(field.label());
                                ui.weak(self.draft.attribution(field));
                                ui.horizontal_wrapped(|ui| {
                                    self.source(
                                        ui,
                                        field,
                                        ComparisonSource::Local,
                                        tags,
                                        &definitions,
                                        now,
                                    );
                                    self.source(
                                        ui,
                                        field,
                                        ComparisonSource::Shared,
                                        tags,
                                        &definitions,
                                        now,
                                    );
                                });
                                self.resolved(ui, field, settings, &definitions, now);
                            }
                            for source in sources {
                                self.source(ui, field, source, tags, &definitions, now);
                            }
                            ui.separator();
                        });
                    }
                });
        });
        let missing = self
            .draft
            .fields()
            .into_iter()
            .filter(|field| {
                self.draft.needs_choice(*field) || self.draft.needs_confirmation(*field)
            })
            .count();
        if missing > 0 {
            ui.label(format!("Choose or confirm {missing} differing field(s)."));
        }
        for error in self
            .errors
            .iter()
            .filter(|error| error.field == "Record" || error.field == "Comparison")
        {
            feedback::problem(ui, &error.message);
        }
        if actions::primary(
            ui,
            "Save resolution",
            !busy
                && !self.settled
                && !self.draft.is_stale()
                && self.validation.can_submit(&self.errors),
        )
        .clicked()
        {
            match self.draft.build(&definitions) {
                Ok(chosen) => action = ComparisonAction::Save { chosen },
                Err(errors) => self.errors = errors,
            }
        }
        action
    }

    /// Offer one native source without parsing its displayed text.
    fn source(
        &mut self,
        ui: &mut egui::Ui,
        field: FieldId,
        source: ComparisonSource,
        tags: &BTreeMap<TagId, String>,
        definitions: &DefinitionSet,
        now: f64,
    ) {
        let input = self.draft.source_input(field, &source);
        let label = match &source {
            ComparisonSource::Local => "Use local".into(),
            ComparisonSource::Shared => "Use shared".into(),
            ComparisonSource::Retained { stamp } => format!(
                "Use retained {}:{}",
                stamp.accepted_at().timeline,
                stamp.accepted_at().revision
            ),
        };
        let selected = self.draft.selected_source(field) == Some(&source);
        if ui
            .add_enabled(input.is_ok(), egui::Button::selectable(selected, label))
            .clicked()
        {
            match self.draft.select(field, source) {
                Ok(()) => self.validation.edited(now),
                Err(error) => self.errors = vec![crate::operating::issue(&field.label(), error)],
            }
        }
        match input {
            Ok(FieldInput::Membership { tags: values }) => {
                ui.label(
                    values
                        .iter()
                        .map(|id| {
                            tags.get(id)
                                .cloned()
                                .unwrap_or_else(|| format!("Unavailable logbook · {id}"))
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                );
            }
            Ok(FieldInput::References { rows }) => {
                ui.label(
                    rows.iter()
                        .map(|row| {
                            format!(
                                "{}{}",
                                row.group
                                    .and_then(|id| definitions.get(id).map(|g| g.name().to_owned()))
                                    .unwrap_or_else(|| "Unrecognized group".into()),
                                if row.identifier.is_empty() {
                                    String::new()
                                } else {
                                    format!(": {}", row.identifier)
                                }
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                );
            }
            Ok(input) => {
                ui.label(input.display());
            }
            Err(_) => {
                ui.weak("No live values available");
            }
        }
    }

    /// Edit the actual resolved field, with no extra selection-confirmation action.
    fn resolved(
        &mut self,
        ui: &mut egui::Ui,
        field: FieldId,
        settings: &SettingsSnapshot,
        definitions: &DefinitionSet,
        now: f64,
    ) {
        if let FieldId::Qso(QsoField::Tags) = field {
            let current = self.draft.membership().cloned().unwrap_or_default();
            let state = self.membership.get_or_insert_with(|| MembershipEditor {
                picker: MembershipPicker::new(&settings.logbooks, std::slice::from_ref(&current)),
                opening: current.clone(),
                current: current.clone(),
            });
            if state.current != current {
                state.picker.replace_single(&settings.logbooks, &current);
                state.opening = current.clone();
                state.current = current;
            }
            let mut sources = vec![ComparisonSource::Local, ComparisonSource::Shared];
            sources.extend(self.draft.basis.review.retained.iter().map(|value| {
                ComparisonSource::Retained {
                    stamp: value.stamp.clone(),
                }
            }));
            for source in sources {
                if let Ok(FieldInput::Membership { tags }) = self.draft.source_input(field, &source)
                {
                    state.picker.include_context(tags);
                }
            }
            ui.label("Resolved membership");
            if tag_picker::show_contents(ui, &mut state.picker) {
                let intent = state.picker.intent();
                let mut tags: BTreeSet<_> =
                    state.opening.difference(&intent.remove).copied().collect();
                tags.extend(intent.add);
                state.current = tags.clone();
                self.draft
                    .set_custom(field, FieldInput::Membership { tags });
                self.validation.edited(now);
            }
        } else if let Ok(mut input) = self.draft.input(field) {
            let label = format!("Resolved {}", field.label());
            if super::editors::show(
                ui,
                &label,
                &mut input,
                definitions,
                settings,
                if self.draft.needs_choice(field) {
                    &[]
                } else {
                    &self.errors
                },
                self.draft.selected_source(field).is_some(),
            ) {
                self.draft.set_custom(field, input);
                self.validation.edited(now);
            }
        }
        if !self.draft.needs_choice(field) {
            for error in self
                .errors
                .iter()
                .filter(|error| error.field == field.label())
            {
                feedback::problem(ui, &error.message);
            }
        }
        if self.draft.needs_confirmation(field) && ui.button("Keep draft value").clicked() {
            self.draft.confirm(field);
            self.validation.edited(now);
        } else if self.draft.needs_choice(field) {
            ui.weak("Choose a source or enter a value.");
        }
    }
}

impl ReviewUi {
    /// Show the pinned comparison while newer saved roles remain separate.
    pub(super) fn show_detail(&mut self, ui: &mut egui::Ui) {
        let Some(row) = &self.detail else {
            ui.label("Select a record to review.");
            return;
        };
        let key = row.record.key();
        let displayed = self
            .drafts
            .get(&key)
            .map(|editor| &editor.draft.basis.record)
            .unwrap_or(&row.record);
        ui.heading(caption(displayed, &self.tag_labels));
        ui.label(format!("ID: {}", key.id));
        if let Some(review) = &row.review {
            ui.label(reason_label(&review.reason));
        }
        let busy = self.reply.is_some() || self.detail_dirty;
        let action =
            if let (Some(editor), Some(settings)) = (self.drafts.get_mut(&key), &self.settings) {
                editor.show(ui, settings, &self.tag_labels, busy)
            } else {
                show_pending(ui, row);
                ComparisonAction::None
            };
        match action {
            ComparisonAction::Refresh => {
                self.refresh_for = Some(key);
                self.detail_dirty = true;
            }
            ComparisonAction::Save { chosen } => {
                if let Some(editor) = self.drafts.get(&key) {
                    self.submit = Some(super::SaveRequest {
                        expected: Box::new(editor.draft.basis.record.clone()),
                        shown: Box::new(editor.draft.basis.review.clone()),
                        chosen,
                    });
                }
            }
            ComparisonAction::None => {}
        }
        if self.saving == Some(key) {
            ui.label("Saving…");
        }
    }
}

/// Describe actual unpublished and blocked work without offering a fake resolution.
fn show_pending(ui: &mut egui::Ui, row: &ReviewRow) {
    if row.authorization_hold.is_some() || !row.record.retained().is_empty() {
        ui.label("Waiting for comparison data. Local work is preserved.");
    } else if row.review.is_none() {
        ui.label(
            if matches!(row.record.working_state(), Some(RecordState::Deleted)) {
                "Deletion saved locally; pending synchronization."
            } else {
                "Saved locally; pending synchronization."
            },
        );
    } else if matches!(row.record.latest(), Observation::Unavailable { .. }) {
        ui.label("Waiting for usable comparison data. Local work is preserved.");
    }
}

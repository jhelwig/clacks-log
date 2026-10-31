//! Field comparison and per-record editor state for the non-modal review window.

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
use clacks_client_store::settings::SettingsSnapshot;
use clacks_model::{group::DefinitionSet, ids::TagId};
use clacks_sync::record::RecordState;
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
            self.errors = self
                .validate(&definitions, settings)
                .err()
                .unwrap_or_default();
            self.validation.validated();
        }
        ui.label(match self.draft.deletion_choice() {
            Some(true) => "Resolved record: Deleted",
            Some(false) => "Resolved record: Live",
            None => "Choose the resolved record state using the source shortcuts.",
        });
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
            let retained = self
                .draft
                .basis
                .review
                .retained
                .iter()
                .map(|value| value.stamp.clone())
                .collect::<Vec<_>>();
            for stamp in retained {
                let label = format!(
                    "Use all retained values from {}:{}",
                    stamp.accepted_at().timeline,
                    stamp.accepted_at().revision
                );
                if ui.button(label).clicked() {
                    match self.draft.select_all(ComparisonSource::Retained { stamp }) {
                        Ok(()) => self.validation.edited(now),
                        Err(error) => {
                            self.errors = vec![crate::operating::issue("Comparison", error)]
                        }
                    }
                }
            }
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
                                        self.source(
                                            column,
                                            field,
                                            source,
                                            tags,
                                            &definitions,
                                            settings,
                                        );
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
                                        settings,
                                    );
                                    self.source(
                                        ui,
                                        field,
                                        ComparisonSource::Shared,
                                        tags,
                                        &definitions,
                                        settings,
                                    );
                                });
                                self.resolved(ui, field, settings, &definitions, now);
                            }
                            for source in sources {
                                self.source(ui, field, source, tags, &definitions, settings);
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
            match self.validate(&definitions, settings) {
                Ok(chosen) => action = ComparisonAction::Save { chosen },
                Err(errors) => self.errors = errors,
            }
        }
        self.validation.request_repaint(ui.ctx(), now);
        action
    }

    /// Validate the chosen native record and its currently available local dependencies.
    fn validate(
        &self,
        definitions: &DefinitionSet,
        settings: &SettingsSnapshot,
    ) -> Result<RecordState, Vec<crate::operating::FieldError>> {
        let chosen = self.draft.build(definitions)?;
        if matches!(chosen, RecordState::Deleted)
            && matches!(
                self.draft.basis.review.reason,
                clacks_sync::client::ReviewReason::Referenced { .. }
            )
        {
            return Err(vec![crate::operating::issue(
                "Record",
                "Use Retry deletion to recheck accepted references",
            )]);
        }
        let mut errors = Vec::new();
        for key in
            clacks_sync::acceptance::required_references(self.draft.basis.record.key(), &chosen)
        {
            let (records, field) = match key.id {
                clacks_sync::record::RecordId::Tag(_) => {
                    (&settings.logbooks.tags, "Logbook membership")
                }
                clacks_sync::record::RecordId::Equipment(_) => {
                    (&settings.equipment, "Default equipment")
                }
                _ => continue,
            };
            if !records.iter().any(|record| {
                record.key() == key && matches!(record.working_state(), Some(RecordState::Live(_)))
            }) {
                errors.push(crate::operating::issue(
                    field,
                    format!(
                        "Referenced record {} is unavailable or deleted; remove or replace it",
                        key.id
                    ),
                ));
            }
        }
        if errors.is_empty() {
            Ok(chosen)
        } else {
            Err(errors)
        }
    }

    /// Offer one native source without parsing its displayed text.
    fn source(
        &mut self,
        ui: &mut egui::Ui,
        field: FieldId,
        source: ComparisonSource,
        tags: &BTreeMap<TagId, String>,
        definitions: &DefinitionSet,
        settings: &SettingsSnapshot,
    ) {
        let now = ui.input(|input| input.time);
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
            match self.draft.select(field, source.clone()) {
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
                                    .map(|id| definitions
                                        .get(id)
                                        .map(|group| group.name().to_owned())
                                        .unwrap_or_else(|| format!("Unrecognized group · {id}")))
                                    .unwrap_or_else(|| "No group selected".into()),
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
            Ok(FieldInput::Equipment { selected }) => {
                let caption = selected
                    .map(|id| {
                        settings
                            .equipment
                            .iter()
                            .find_map(|row| match row.working_state() {
                                Some(RecordState::Live(
                                    clacks_sync::record::RecordValue::Equipment(value),
                                )) if value.id() == id => Some(format!("{} · {id}", value.name())),
                                _ => None,
                            })
                            .unwrap_or_else(|| format!("Unavailable equipment · {id}"))
                    })
                    .unwrap_or_else(|| "No default equipment".into());
                ui.label(caption);
            }
            Ok(input) => {
                let text = egui::RichText::new(input.display());
                ui.label(if field.radio_data() {
                    text.monospace()
                } else {
                    text
                });
            }
            Err(_) => {
                ui.weak(
                    if matches!(self.draft.basis.source(&source), Ok(RecordState::Deleted)) {
                        "Deleted"
                    } else {
                        "No live values available"
                    },
                );
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
                crate::ui::fields::FieldPresentation {
                    edited: self.draft.edited(field),
                    read_only: !ui.is_enabled(),
                    errors: if self.draft.needs_choice(field) {
                        &[]
                    } else {
                        &self.errors
                    },
                    validation_label: Some(&field.label()),
                    radio_data: field.radio_data(),
                },
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

//! Explicit choices and transient edited inputs over an exact saved comparison.

use super::{
    comparison::{ComparisonBasis, ComparisonSource},
    fields::{FieldId, QsoField, record::NativeFields},
    input::FieldInput,
};
use crate::operating::{FieldError, issue};
use clacks_model::{group::DefinitionSet, ids::TagId};
use clacks_sync::{
    client::OriginalBasis,
    record::{Observation, RecordState},
};
use std::collections::{BTreeMap, BTreeSet};

/// An unfinished native record resolution; no intermediate input is a saved record.
#[derive(Clone, Debug)]
pub(crate) struct ResolutionDraft {
    /// Exact saved roles shown and required by the guarded save.
    pub basis: ComparisonBasis,
    /// Native source-selected fields; invalid cross-field combinations remain editable.
    seed: NativeFields,
    /// Explicitly edited text/control inputs, parsed only when building a result.
    custom: BTreeMap<FieldId, FieldInput>,
    /// Fields with an explicit resolved choice against the current comparison.
    confirmed: BTreeSet<FieldId>,
    /// Source names for choice presentation, not a causal ordering mechanism.
    sources: BTreeMap<FieldId, ComparisonSource>,
    /// Retained choices requiring reconsideration after their inputs changed.
    reconsider: BTreeSet<FieldId>,
    /// Current shown basis differs from a subsequently observed saved basis.
    stale: bool,
    /// Explicit record-state choice when a live version competes with deletion.
    deleted: Option<bool>,
}

impl ResolutionDraft {
    /// Start from available native facts, leaving differing scalar choices unselected.
    pub fn new(basis: ComparisonBasis) -> anyhow::Result<Self> {
        anyhow::ensure!(
            basis.record.key() == basis.review.shared.key(),
            "Comparison belongs to another record"
        );
        let seed = NativeFields::from_state(&basis.review.local)
            .or_else(|_| NativeFields::from_state(basis.source(&ComparisonSource::Shared)?))
            .or_else(|_| {
                basis
                    .review
                    .retained
                    .iter()
                    .find_map(|value| NativeFields::from_state(value.state()).ok())
                    .ok_or_else(|| anyhow::anyhow!("This comparison has no live editable values"))
            })?;
        let deleted = if matches!(basis.review.local, RecordState::Deleted)
            || matches!(
                basis.source(&ComparisonSource::Shared),
                Ok(RecordState::Deleted)
            ) {
            None
        } else {
            Some(false)
        };
        Ok(Self {
            basis,
            seed,
            custom: BTreeMap::new(),
            confirmed: [FieldId::Qso(QsoField::Tags)].into(),
            sources: BTreeMap::new(),
            reconsider: BTreeSet::new(),
            stale: false,
            deleted,
        })
    }

    /// Borrow the checked membership that directly defines the resolved set.
    pub fn membership(&self) -> Option<&BTreeSet<TagId>> {
        match self.custom.get(&FieldId::Qso(QsoField::Tags)) {
            Some(FieldInput::Membership { tags }) => Some(tags),
            _ => match &self.seed {
                NativeFields::Qso(value) => Some(&value.tags),
                _ => None,
            },
        }
    }

    /// Return native fields in their owning editor’s presentation order.
    pub fn fields(&self) -> Vec<FieldId> {
        if self.deleted == Some(true) {
            Vec::new()
        } else {
            self.seed.fields()
        }
    }

    /// Identify an actual local/shared difference without parsing displayed text.
    pub fn differs(&self, field: FieldId) -> bool {
        !same(
            &self.basis.review.local,
            self.basis.source(&ComparisonSource::Shared).ok(),
            field,
        )
    }

    /// Describe changes only against a genuine available original.
    pub fn attribution(&self, field: FieldId) -> &'static str {
        if matches!(
            self.basis.review.original,
            OriginalBasis::Unavailable { .. }
        ) {
            return "Original values unavailable";
        }
        let original = self.basis.review.original.state();
        if original.is_none() {
            return "No original recorded values";
        }
        let local_same = original.is_some_and(|v| same(v, Some(&self.basis.review.local), field));
        let shared_same = original
            .is_some_and(|v| same(v, self.basis.source(&ComparisonSource::Shared).ok(), field));
        match (local_same, shared_same) {
            (true, false) => "Changed in shared log",
            (false, true) => "Changed locally",
            (false, false) => "Both changed",
            (true, true) => "Unchanged",
        }
    }

    /// Copy a selected existing native field without a text round trip.
    pub fn select(&mut self, field: FieldId, source: ComparisonSource) -> anyhow::Result<()> {
        let from = NativeFields::from_state(self.basis.source(&source)?)?;
        self.seed.copy(&from, field)?;
        self.custom.remove(&field);
        self.confirmed.insert(field);
        self.reconsider.remove(&field);
        self.sources.insert(field, source);
        Ok(())
    }

    /// Populate all choices from one existing native version without saving.
    pub fn select_all(&mut self, source: ComparisonSource) -> anyhow::Result<()> {
        if matches!(self.basis.source(&source)?, RecordState::Deleted) {
            self.deleted = Some(true);
            self.custom.clear();
            self.reconsider.clear();
            return Ok(());
        }
        self.seed = NativeFields::from_state(self.basis.source(&source)?)?;
        self.deleted = Some(false);

        self.custom.clear();
        self.reconsider.clear();
        for field in self.fields() {
            self.confirmed.insert(field);
            self.sources.insert(field, source.clone());
        }
        Ok(())
    }

    /// Retain explicit raw input, including invalid or deliberately cleared values.
    pub fn set_custom(&mut self, field: FieldId, input: FieldInput) {
        self.custom.insert(field, input);
        self.confirmed.insert(field);
        self.reconsider.remove(&field);
        self.sources.remove(&field);
    }

    /// Recover the current control input without treating a preview as an edit.
    pub fn input(&self, field: FieldId) -> anyhow::Result<FieldInput> {
        if let Some(input) = self.custom.get(&field) {
            return Ok(input.clone());
        }
        self.seed.input(field)
    }

    /// Format a source only for comparison display; copying uses native values.
    pub fn source_input(
        &self,
        field: FieldId,
        source: &ComparisonSource,
    ) -> anyhow::Result<FieldInput> {
        let value = NativeFields::from_state(self.basis.source(source)?)?;
        value.input(field)
    }

    /// Borrow a chosen source for standard selected-control presentation.
    pub fn selected_source(&self, field: FieldId) -> Option<&ComparisonSource> {
        self.sources.get(&field)
    }

    /// Highlight explicit edits and native results that differ from local working facts.
    pub fn edited(&self, field: FieldId) -> bool {
        self.custom.contains_key(&field)
            || NativeFields::from_state(&self.basis.review.local)
                .is_ok_and(|local| !self.seed.same(&local, field))
    }

    /// Whether this differing field still needs the operator's explicit choice.
    pub fn needs_choice(&self, field: FieldId) -> bool {
        self.differs(field) && !self.confirmed.contains(&field)
    }

    /// Whether a preserved result needs approval against newly shown inputs.
    pub fn needs_confirmation(&self, field: FieldId) -> bool {
        self.reconsider.contains(&field)
    }

    /// Confirm a retained resolved input after viewing its refreshed comparison.
    pub fn confirm(&mut self, field: FieldId) {
        self.reconsider.remove(&field);
        self.confirmed.insert(field);
    }

    /// Mark a changed exact basis without replacing visible comparison or inputs.
    pub fn observe(&mut self, latest: &ComparisonBasis) {
        self.stale |= &self.basis != latest;
    }

    /// Require explicit refresh after the guarded store rejects an outdated basis.
    pub fn require_refresh(&mut self) {
        self.stale = true;
    }

    /// Whether a refreshed comparison is required before saving this draft.
    pub fn is_stale(&self) -> bool {
        self.stale
    }

    /// Identify the explicit live/deleted choice, absent until the operator chooses.
    pub fn deletion_choice(&self) -> Option<bool> {
        self.deleted
    }

    /// Install explicitly refreshed evidence while preserving edited results.
    pub fn refresh(&mut self, next: ComparisonBasis) -> anyhow::Result<()> {
        anyhow::ensure!(
            next.record.key() == self.basis.record.key(),
            "Refresh belongs to another record"
        );
        for field in self.fields() {
            let changed = !same(&self.basis.review.local, Some(&next.review.local), field)
                || match (
                    self.basis.source(&ComparisonSource::Shared),
                    next.source(&ComparisonSource::Shared),
                ) {
                    (Ok(a), Ok(b)) => !same(a, Some(b), field),
                    _ => self.basis.review.shared != next.review.shared,
                };
            if changed {
                if self.confirmed.remove(&field) || self.custom.contains_key(&field) {
                    self.reconsider.insert(field);
                    self.sources.remove(&field);
                } else if let Ok(from) = NativeFields::from_state(&next.review.local) {
                    self.seed.copy(&from, field)?;
                }
            }
        }
        let was_deleted = (
            matches!(self.basis.review.local, RecordState::Deleted),
            matches!(
                self.basis.source(&ComparisonSource::Shared),
                Ok(RecordState::Deleted)
            ),
        );
        let now_deleted = (
            matches!(next.review.local, RecordState::Deleted),
            matches!(
                next.source(&ComparisonSource::Shared),
                Ok(RecordState::Deleted)
            ),
        );
        if was_deleted != now_deleted {
            self.deleted = None;
        }
        self.basis = next;

        self.stale = false;
        Ok(())
    }

    /// Validate one complete chosen record without publishing or altering saved work.
    pub fn build(&self, definitions: &DefinitionSet) -> Result<RecordState, Vec<FieldError>> {
        if self.stale {
            return Err(vec![issue(
                "Comparison",
                "Refresh the comparison before saving",
            )]);
        }
        if matches!(self.basis.review.shared, Observation::Unavailable { .. }) {
            return Err(vec![issue(
                "Comparison",
                "Waiting for usable shared comparison data",
            )]);
        }
        match self.deleted {
            Some(true) => return Ok(RecordState::Deleted),
            None => {
                return Err(vec![issue(
                    "Record",
                    "Choose whether the resolved record is live or deleted",
                )]);
            }
            Some(false) => {}
        }
        let mut errors = Vec::new();

        for field in self.fields() {
            if self.needs_choice(field) || self.needs_confirmation(field) {
                errors.push(issue(
                    &field.label(),
                    "Choose or confirm the resolved value",
                ));
            }
        }
        let mut result = self.seed.clone();
        for (field, input) in &self.custom {
            if let Err(error) = result.edit(*field, input, definitions) {
                errors.push(issue(&field.label(), error));
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        result.build().map_err(|error| vec![issue("Record", error)])
    }
}

/// Compare one field only when both actual source values are available.
fn same(a: &RecordState, b: Option<&RecordState>, field: FieldId) -> bool {
    match (
        NativeFields::from_state(a),
        b.map(NativeFields::from_state).transpose(),
    ) {
        (Ok(a), Ok(Some(b))) => a.same(&b, field),
        _ => false,
    }
}

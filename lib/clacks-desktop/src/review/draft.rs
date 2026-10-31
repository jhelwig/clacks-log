//! Explicit choices and transient edited inputs over an exact saved comparison.

use super::{
    comparison::{ComparisonBasis, ComparisonSource},
    fields::{self, FieldId, QsoField},
    input::FieldInput,
};
use crate::operating::{FieldError, issue};
use clacks_model::{contact::input::QsoFields, group::DefinitionSet, ids::TagId};
use clacks_sync::{
    client::OriginalBasis,
    record::{Observation, RecordState, RecordValue},
};
use std::collections::{BTreeMap, BTreeSet};

/// An unfinished native contact resolution; no intermediate input is a saved record.
#[derive(Clone, Debug)]
pub(crate) struct ResolutionDraft {
    /// Exact saved roles shown and required by the guarded save.
    pub basis: ComparisonBasis,
    /// Native source-selected fields; invalid cross-field combinations remain editable.
    seed: QsoFields,
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
}

impl ResolutionDraft {
    /// Start from local native facts, leaving differing scalar choices unselected.
    pub fn new(basis: ComparisonBasis) -> anyhow::Result<Self> {
        anyhow::ensure!(
            basis.record.key() == basis.review.shared.key(),
            "Comparison belongs to another record"
        );
        let seed = match &basis.review.local {
            RecordState::Live(RecordValue::Qso(qso)) => qso.clone().into_fields(),
            _ => match basis.source(&ComparisonSource::Shared)? {
                RecordState::Live(RecordValue::Qso(qso)) => qso.clone().into_fields(),
                _ => anyhow::bail!("This comparison has no live contact values"),
            },
        };
        Ok(Self {
            basis,
            seed,
            custom: BTreeMap::new(),
            confirmed: [FieldId::Qso(QsoField::Tags)].into(),
            sources: BTreeMap::new(),
            reconsider: BTreeSet::new(),
            stale: false,
        })
    }

    /// Borrow the checked membership that directly defines the resolved set.
    pub fn membership(&self) -> Option<&BTreeSet<TagId>> {
        match self.custom.get(&FieldId::Qso(QsoField::Tags)) {
            Some(FieldInput::Membership { tags }) => Some(tags),
            _ => Some(&self.seed.tags),
        }
    }

    /// Return native fields in normal contact presentation order.
    pub fn fields(&self) -> Vec<FieldId> {
        FieldId::qso_fields()
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
        let from = qso_fields(self.basis.source(&source)?)?;
        let FieldId::Qso(qso_field) = field;
        fields::qso::copy(&mut self.seed, &from, qso_field)?;
        self.custom.remove(&field);
        self.confirmed.insert(field);
        self.reconsider.remove(&field);
        self.sources.insert(field, source);
        Ok(())
    }

    /// Populate all choices from one existing native version without saving.
    pub fn select_all(&mut self, source: ComparisonSource) -> anyhow::Result<()> {
        self.seed = qso_fields(self.basis.source(&source)?)?;
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
        let FieldId::Qso(field) = field;
        Ok(fields::qso::input(&self.seed, field))
    }

    /// Format a source only for comparison display; copying uses native values.
    pub fn source_input(
        &self,
        field: FieldId,
        source: &ComparisonSource,
    ) -> anyhow::Result<FieldInput> {
        let value = qso_fields(self.basis.source(source)?)?;
        let FieldId::Qso(field) = field;
        Ok(fields::qso::input(&value, field))
    }

    /// Borrow a chosen source for standard selected-control presentation.
    pub fn selected_source(&self, field: FieldId) -> Option<&ComparisonSource> {
        self.sources.get(&field)
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
                } else if let Ok(from) = qso_fields(&next.review.local) {
                    let FieldId::Qso(qso_field) = field;
                    fields::qso::copy(&mut self.seed, &from, qso_field)?;
                }
            }
        }
        self.basis = next;
        self.stale = false;
        Ok(())
    }

    /// Validate one complete chosen contact without publishing or altering saved work.
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
            let FieldId::Qso(qso_field) = *field;
            if let Err(error) = fields::qso::edit(&mut result, qso_field, input, definitions) {
                errors.push(issue(&field.label(), error));
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let qso = result
            .try_into()
            .map_err(|error: anyhow::Error| vec![issue("Contact", error)])?;
        Ok(RecordState::Live(RecordValue::Qso(Box::new(qso))))
    }
}

/// Recover editable native contact members, refusing incompatible sources.
fn qso_fields(state: &RecordState) -> anyhow::Result<QsoFields> {
    match state {
        RecordState::Live(RecordValue::Qso(value)) => Ok(value.clone().into_fields()),
        _ => anyhow::bail!("This source has no live contact fields"),
    }
}

/// Compare one field only when both actual source values are available.
fn same(a: &RecordState, b: Option<&RecordState>, field: FieldId) -> bool {
    match (qso_fields(a), b.map(qso_fields).transpose()) {
        (Ok(a), Ok(Some(b))) => {
            let FieldId::Qso(field) = field;
            fields::qso::same(&a, &b, field)
        }
        _ => false,
    }
}

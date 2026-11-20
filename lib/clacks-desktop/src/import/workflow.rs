//! Track import review choices and build the final QSO for each row.

pub(crate) mod operation;

#[cfg(test)]
pub(crate) mod tests;

use super::{
    adi::{Diagnostic, PreparedSource, SourceMetadata},
    mapping::{Choice, PreparedRow, RowProjection},
    matching::{Candidate, CandidateIndex},
};
use crate::review::{
    fields::{FieldId, record::NativeFields},
    input::FieldInput,
};
use clacks_client_store::{read::projection::ReadContext, settings::SettingsSnapshot};
use clacks_model::{
    contact::Qso,
    ids::{QsoId, TagId},
    scalar::RecordedCallsign,
};
use clacks_sync::{
    client::ClientRecord,
    record::{RecordState, RecordValue},
};
use std::{collections::BTreeMap, path::PathBuf};
use tokio::sync::{oneshot, watch};
use tokio_util::sync::CancellationToken;

pub(crate) struct PrepareRequest {
    pub path: PathBuf,
    pub tag: TagId,
    pub stop: CancellationToken,
    pub review: oneshot::Sender<ReviewSession>,
    pub completion: oneshot::Sender<Result<Report, String>>,
    pub progress: watch::Sender<usize>,
}

impl std::fmt::Debug for PrepareRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PrepareRequest")
            .finish_non_exhaustive()
    }
}

pub(crate) struct ReviewSession {
    pub prepared: PreparedImport,
    pub apply: oneshot::Sender<PreparedImport>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Action {
    Add,
    Update(QsoId),
    Drop,
}

/// Readiness of a row under its current choices, before any write.
pub(crate) enum RowReadiness {
    /// Source or saved-contact assignment choices remain.
    NeedsAssignment,
    /// One or more differing fields still need a choice.
    NeedsFields,
    Dropped,
    /// The selected values form a valid QSO for Apply.
    Ready {
        /// The saved QSO being updated, or none for a new contact.
        target: Option<QsoId>,
        qso: Box<Qso>,
    },
}

/// One source row, its saved candidates, and the operator's choices.
pub(crate) struct RowReview {
    pub source: PreparedRow,
    new_id: QsoId,
    projection: RowProjection,
    pub candidates: Vec<Candidate>,
    pub choices: BTreeMap<usize, Choice>,
    pub historical_call: String,
    pub fields: BTreeMap<FieldId, bool>,
    explicit_action: Option<Action>,
}

impl RowReview {
    /// A new QSO needs an explicit historical callsign when the source omits ours.
    pub fn needs_station(&self) -> bool {
        self.projection.own_call.is_none()
    }

    pub fn worked_call(&self) -> Option<&RecordedCallsign> {
        self.projection.worked_call.as_ref()
    }

    pub fn source_resolved(&self) -> bool {
        self.source.source_resolved(&self.choices)
    }
}

pub(crate) struct Difference {
    pub field: FieldId,
    pub saved: FieldInput,
    pub incoming: FieldInput,
}

pub(crate) struct PreparedImport {
    pub metadata: SourceMetadata,
    pub header_warnings: Vec<Diagnostic>,
    /// Terminal source framing error, when only a prefix was readable.
    pub terminal: Option<Diagnostic>,
    /// Number of readable rows that contain each unsupported field.
    pub unsupported: BTreeMap<String, usize>,
    pub settings: SettingsSnapshot,
    pub rows: Vec<RowReview>,
    pub destination: String,
    pub file: String,
    tag: TagId,
    source_identity: String,
    catalogs: ReadContext,
    index: CandidateIndex,
    contention: BTreeMap<QsoId, usize>,
}

#[derive(Default)]
pub(crate) struct Report {
    pub created: usize,
    pub changed: usize,
    pub memberships: usize,
    pub assertions: usize,
    pub unchanged: usize,
    pub dropped: usize,
    pub unprocessed: usize,
    pub cancelled: bool,
    pub terminal: Option<Diagnostic>,
    pub header_warnings: Vec<Diagnostic>,
    pub warnings: Vec<(usize, Diagnostic)>,
    pub unsupported: BTreeMap<String, usize>,
    pub failures: Vec<(usize, String)>,
    pub skipped: Vec<(usize, &'static str)>,
}

impl PreparedImport {
    fn new(
        source: PreparedSource,
        file: String,
        source_identity: String,
        tag: TagId,
        snapshot: (SettingsSnapshot, ReadContext, Vec<ClientRecord>),
        stop: &CancellationToken,
    ) -> anyhow::Result<Self> {
        let (settings, catalogs, records) = snapshot;
        let destination = tags(&settings)
            .remove(&tag)
            .ok_or_else(|| anyhow::anyhow!("The selected logbook is no longer available"))?;
        let index = CandidateIndex::new(records, stop)?;
        let PreparedSource {
            rows,
            unsupported,
            terminal,
            header_warnings,
            metadata,
        } = source;
        let rows = rows
            .into_iter()
            .map(|source| {
                anyhow::ensure!(!stop.is_cancelled(), "Import cancelled");
                let choices = BTreeMap::new();
                let projection = source.resolve(&choices);
                Ok(RowReview {
                    source,
                    new_id: QsoId::new(),
                    candidates: index.candidates(&projection),
                    projection,
                    choices,
                    historical_call: String::new(),
                    fields: BTreeMap::new(),
                    explicit_action: None,
                })
            })
            .collect::<anyhow::Result<_>>()?;
        let mut prepared = Self {
            metadata,
            header_warnings,
            terminal,
            unsupported,
            settings,
            rows,
            destination,
            file,
            tag,
            source_identity,
            catalogs,
            index,
            contention: BTreeMap::new(),
        };
        prepared.update_contention();
        Ok(prepared)
    }

    pub fn catalogs(&self) -> &ReadContext {
        &self.catalogs
    }

    pub fn saved(&self, id: QsoId) -> Option<&Qso> {
        self.index.saved(id).map(|(_, qso)| qso)
    }

    /// Return the selected or inferred action. Unresolved assignment yields `None`.
    pub fn action(&self, row: usize) -> Option<Action> {
        self.action_for(self.rows.get(row)?)
    }

    /// Infer reuse only for a unique, uncontested match requiring no explicit selection.
    fn action_for(&self, state: &RowReview) -> Option<Action> {
        if state.explicit_action == Some(Action::Drop) {
            return Some(Action::Drop);
        }
        if !state.source_resolved() {
            return None;
        }
        state
            .explicit_action
            .or_else(|| match state.candidates.as_slice() {
                [] => Some(Action::Add),
                [candidate]
                    if !candidate.requires_selection()
                        && self.contention.get(&candidate.id) == Some(&1) =>
                {
                    Some(Action::Update(candidate.id))
                }
                _ => None,
            })
    }

    /// Return the one-based source record number of another row assigned to the target.
    pub fn other_assignment(&self, row: usize, target: QsoId) -> Option<usize> {
        self.rows.iter().enumerate().find_map(|(other, state)| {
            (row != other && self.action_for(state) == Some(Action::Update(target)))
                .then_some(state.source.number)
        })
    }

    /// Choose a row action. Reject a saved target that is ineligible or already assigned,
    /// and discard field choices when the action changes.
    pub fn select(&mut self, row: usize, action: Action) {
        let Some(state) = self.rows.get(row) else {
            return;
        };
        if let Action::Update(target) = action
            && (self.other_assignment(row, target).is_some()
                || !state
                    .candidates
                    .iter()
                    .any(|candidate| candidate.id == target))
        {
            return;
        }
        let changed = self.action_for(state) != Some(action);
        if let Some(state) = self.rows.get_mut(row) {
            state.explicit_action = Some(action);
            if changed {
                state.fields.clear();
            }
        }
    }

    /// Recompute candidates after a source choice. Discard an explicit assignment
    /// if its candidate changes and retain field choices only for unchanged values
    /// on the same target.
    pub fn resolve(&mut self, row: usize, dispute: usize, choice: Choice) {
        let Some(state) = self.rows.get(row) else {
            return;
        };
        if state.choices.get(&dispute) == Some(&choice) {
            return;
        }
        let target = match self.action_for(state) {
            Some(Action::Update(target)) => Some(target),
            _ => None,
        };
        let before = target.and_then(|target| self.comparison(state, target).ok());
        let Some(state) = self.rows.get_mut(row) else {
            return;
        };
        let previous_candidates = std::mem::take(&mut state.candidates);
        state.choices.insert(dispute, choice);
        state.projection = state.source.resolve(&state.choices);
        state.candidates = self.index.candidates(&state.projection);
        if let Some(Action::Update(target)) = state.explicit_action
            && !state
                .candidates
                .iter()
                .any(|candidate| candidate.id == target && previous_candidates.contains(candidate))
        {
            state.explicit_action = None;
        }
        self.update_contention();

        let Some(target) = target else {
            return;
        };
        let after = self
            .rows
            .get(row)
            .filter(|state| self.action_for(state) == Some(Action::Update(target)))
            .and_then(|state| self.comparison(state, target).ok());
        if let Some(state) = self.rows.get_mut(row) {
            if let (Some((_, before)), Some((_, after))) = (before, after) {
                state.fields.retain(|field, _| before.same(&after, *field));
            } else {
                state.fields.clear();
            }
        }
    }

    fn update_contention(&mut self) {
        self.contention.clear();
        for row in &self.rows {
            for candidate in &row.candidates {
                *self.contention.entry(candidate.id).or_default() += 1;
            }
        }
    }

    /// Return the full saved contact and the proposed fields that differ.
    pub fn review_comparison(
        &self,
        row: usize,
        target: QsoId,
    ) -> anyhow::Result<(NativeFields, Vec<Difference>)> {
        let state = self
            .rows
            .get(row)
            .ok_or_else(|| anyhow::anyhow!("The source row is unavailable"))?;
        let (saved, import) = self.comparison(state, target)?;
        let differences = state
            .projection
            .present()
            .into_iter()
            .filter(|field| !saved.same(&import, *field))
            .map(|field| {
                Ok(Difference {
                    field,
                    saved: saved.input(field)?,
                    incoming: import.input(field)?,
                })
            })
            .collect::<anyhow::Result<_>>()?;
        Ok((saved, differences))
    }

    fn comparison(
        &self,
        row: &RowReview,
        target: QsoId,
    ) -> anyhow::Result<(NativeFields, NativeFields)> {
        let mut incoming = self
            .saved(target)
            .ok_or_else(|| anyhow::anyhow!("The reviewed saved candidate is unavailable"))?
            .clone()
            .into_fields();
        let saved = NativeFields::Qso(Box::new(incoming.clone()));
        row.projection.apply(&mut incoming)?;
        Ok((saved, NativeFields::Qso(Box::new(incoming))))
    }

    /// Validate the combined saved and incoming values using the same path as Apply.
    pub fn readiness(&self, row: usize) -> anyhow::Result<RowReadiness> {
        let review = self
            .rows
            .get(row)
            .ok_or_else(|| anyhow::anyhow!("The source row is unavailable"))?;
        self.row_readiness(review)
    }

    fn row_readiness(&self, review: &RowReview) -> anyhow::Result<RowReadiness> {
        let Some(action) = self.action_for(review) else {
            return Ok(RowReadiness::NeedsAssignment);
        };
        let (target, mut fields) =
            match action {
                Action::Drop => return Ok(RowReadiness::Dropped),
                Action::Add => {
                    let historical = &review.historical_call;
                    let own = (!historical.trim().is_empty())
                        .then(|| RecordedCallsign::new(historical))
                        .transpose()?;
                    (None, review.projection.create(review.new_id, own)?)
                }
                Action::Update(target) => {
                    let (mut saved, import) = self.comparison(review, target)?;
                    if review.projection.present().iter().any(|field| {
                        !review.fields.contains_key(field) && !saved.same(&import, *field)
                    }) {
                        return Ok(RowReadiness::NeedsFields);
                    }

                    for (&field, &use_import) in &review.fields {
                        if use_import {
                            saved.copy(&import, field)?;
                        }
                    }
                    let NativeFields::Qso(fields) = saved else {
                        anyhow::bail!("Import comparison has no contact fields");
                    };
                    (Some(target), *fields)
                }
            };
        fields.tags.insert(self.tag);
        Ok(RowReadiness::Ready {
            target,
            qso: Box::new(Qso::try_from(fields)?),
        })
    }
}

pub(crate) fn tags(settings: &SettingsSnapshot) -> BTreeMap<TagId, String> {
    settings
        .logbooks
        .tags
        .iter()
        .filter_map(|row| match row.working_state() {
            Some(RecordState::Live(RecordValue::Tag(tag))) => Some((tag.id, tag.name.clone())),
            _ => None,
        })
        .collect()
}

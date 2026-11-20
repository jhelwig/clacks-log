//! Source field access, diagnostics and alternatives for one prepared record.

use super::{Dispute, Edit, PreparedRow, SubmissionPark};
use crate::import::adi::{Alternative, Diagnostic, decode_diagnostics};
use qso_io::adif::adi::parser::framing::field::PermissiveField;
use std::{collections::BTreeSet, ops::Range, sync::Arc};

/// Accumulate edits and diagnostics for a single framed source record.
pub(in crate::import) struct RowInput<'row, 'source> {
    fields: &'row [PermissiveField<'source>],
    /// Supported names, including occurrences whose value could not be decoded.
    handled: BTreeSet<String>,
    /// Unsupported subfield labels within otherwise supported source fields.
    unsupported: BTreeSet<String>,
    prepared: PreparedRow,
}

impl<'row, 'source> RowInput<'row, 'source> {
    pub(in crate::import) fn new(
        fields: &'row [PermissiveField<'source>],
        span: Range<usize>,
        number: usize,
        provenance: Arc<str>,
    ) -> Self {
        let warnings = decode_diagnostics(fields).collect();
        Self {
            fields,
            handled: BTreeSet::new(),
            unsupported: BTreeSet::new(),
            prepared: PreparedRow {
                number,
                span,
                provenance,
                warnings,
                disputes: Vec::new(),
                edits: Vec::new(),
                alternatives: Vec::new(),
            },
        }
    }

    /// Exclude this name from unsupported-field reporting.
    pub(in crate::import) fn handle(&mut self, name: &str) {
        self.handled.insert(name.to_owned());
    }

    /// Return decoded occurrences in source order, including duplicates, and
    /// mark the field name as supported.
    pub(in crate::import) fn values(&mut self, name: &str) -> Vec<Alternative> {
        self.handle(name);
        self.fields
            .iter()
            .filter(|field| field.name().eq_ignore_ascii_case(name))
            .filter_map(|field| {
                field
                    .text()
                    .ok()
                    .map(|value| Alternative::from_field(field, value))
            })
            .collect()
    }

    /// Whether a descriptor was supplied, including empty or undecodable values.
    pub(in crate::import) fn supplied(&self, name: &str) -> bool {
        self.fields
            .iter()
            .any(|field| field.name().eq_ignore_ascii_case(name))
    }

    /// An omitted field or only decoded whitespace counts as absent or empty.
    pub(super) fn absent_or_empty(&self, name: &str) -> bool {
        self.fields
            .iter()
            .filter(|field| field.name().eq_ignore_ascii_case(name))
            .all(|field| field.text().is_ok_and(|value| value.trim().is_empty()))
    }

    pub(super) fn present_nonempty(&self, name: &str) -> bool {
        self.fields.iter().any(|field| {
            field.name().eq_ignore_ascii_case(name)
                && field.text().is_ok_and(|value| !value.trim().is_empty())
        })
    }

    /// Record a warning with the supplied text and source byte range.
    pub(in crate::import) fn warn(&mut self, source: &Alternative, reason: impl std::fmt::Display) {
        self.prepared.warnings.push(Diagnostic {
            message: format!(
                "{} {:?} at bytes {}–{} · {reason}",
                source.source, source.value, source.span.start, source.span.end,
            ),
            span: source.span.clone(),
        });
    }

    /// Report an unsupported application subfield separately from its descriptor.
    pub(in crate::import) fn disclose(&mut self, name: String) {
        self.unsupported.insert(name);
    }

    /// Store one distinct edit directly. Require review when distinct edits disagree.
    pub(in crate::import) fn record(
        &mut self,
        fact: impl Into<String>,
        values: Vec<(Alternative, Edit)>,
    ) {
        self.record_dispute(fact, values, None);
    }

    /// Deduplicate by typed edit and return the index if a dispute is created.
    /// A POTA submission alternative can settle a separate qualifier dispute.
    pub(super) fn record_dispute(
        &mut self,
        fact: impl Into<String>,
        values: Vec<(Alternative, Edit)>,
        unless_submission_selects: Option<SubmissionPark>,
    ) -> Option<usize> {
        let mut distinct = Vec::<(Alternative, Edit)>::new();
        for (source, value) in values {
            if !distinct.iter().any(|(_, old)| *old == value) {
                distinct.push((source, value));
            }
        }
        if distinct.len() > 1 {
            let index = self.prepared.disputes.len();
            let (alternatives, edits) = distinct.into_iter().unzip();
            self.prepared.disputes.push(Dispute {
                fact: fact.into(),
                alternatives,
                unless_submission_selects,
            });
            self.prepared.alternatives.push(edits);
            return Some(index);
        }
        if let Some((_, value)) = distinct.pop() {
            self.prepared.edits.push(value);
        }
        None
    }

    /// Map one source field independently, retaining conflicts between duplicate values.
    pub(in crate::import) fn map(
        &mut self,
        name: &str,
        parse: impl Fn(&str) -> anyhow::Result<Edit>,
    ) {
        let next = self.parsed(name, parse);
        self.record(name, next);
    }

    /// Return usable values with their source locations and warn about parse failures.
    pub(super) fn parsed<T>(
        &mut self,
        name: &str,
        parse: impl Fn(&str) -> anyhow::Result<T>,
    ) -> Vec<(Alternative, T)> {
        let mut next = Vec::new();
        for input in self.values(name) {
            match parse(&input.value) {
                Ok(value) => next.push((input, value)),
                Err(error) => self.warn(&input, error),
            }
        }
        next
    }

    /// Represent a decoded empty or whitespace-only value as `None`. Invalid values
    /// produce warnings and are excluded from the returned alternatives.
    pub(super) fn parsed_optional<T>(
        &mut self,
        name: &str,
        parse: impl Fn(&str) -> anyhow::Result<T>,
    ) -> Vec<(Alternative, Option<T>)> {
        self.parsed(name, |text| {
            let text = text.trim();
            (!text.is_empty()).then(|| parse(text)).transpose()
        })
    }

    /// Return unique unsupported fields so each row contributes at most one occurrence.
    pub(in crate::import) fn unsupported(&self) -> BTreeSet<String> {
        self.fields
            .iter()
            .map(|field| field.name().to_ascii_uppercase())
            .filter(|name| !self.handled.contains(name))
            .chain(self.unsupported.iter().cloned())
            .collect()
    }

    pub(in crate::import) fn finish(self) -> PreparedRow {
        self.prepared
    }
}

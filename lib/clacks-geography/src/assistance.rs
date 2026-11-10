//! Shared native geographic population and position-bound correction rules.

use crate::{
    artifact::Field,
    lookup::{Coverage, FieldLookup, LookupResult},
};
use clacks_model::{location::RecordedLocationFields, position::Position};
use std::collections::BTreeSet;

/// Replace position evidence and expire geographic facts only on a geographic change.
///
/// Call after validating an actual position edit, never for incomplete input or
/// snapshot loading. Same-point display precision preserves facts and protected
/// absence. A grid area is distinct from its center point. Returns whether old
/// location corrections were expired; unrelated station facts are not inputs.
pub fn change_position(
    fields: &mut RecordedLocationFields,
    protected: &mut BTreeSet<Field>,
    next: Option<Position>,
) -> bool {
    let same = match (&fields.position, &next) {
        (Some(previous), Some(next)) => previous.same_geographic_position(next),
        (None, None) => true,
        _ => false,
    };
    fields.position = next;
    if !same {
        fields.dxcc = None;
        fields.primary_subdivision = None;
        fields.secondary_subdivision = None;
        fields.cq_zone = None;
        fields.itu_zone = None;
        protected.clear();
    }
    !same
}

/// Populate only eligible absent fields with a single completely covered value.
///
/// Existing values and protected absence always survive. Callers supply manual,
/// supplied/rejected-field and snapshot protection and ensure the result belongs
/// to the current operation/position. Candidates remain on `lookup` for manual
/// choice; this returns only fields actually changed, without modifying position.
pub fn populate(
    fields: &mut RecordedLocationFields,
    protected: &BTreeSet<Field>,
    lookup: &LookupResult,
) -> BTreeSet<Field> {
    let mut changed = BTreeSet::new();
    if fill(&mut fields.dxcc, Field::Dxcc, protected, &lookup.dxcc) {
        changed.insert(Field::Dxcc);
    }
    if fill(
        &mut fields.primary_subdivision,
        Field::PrimarySubdivision,
        protected,
        &lookup.primary_subdivision,
    ) {
        changed.insert(Field::PrimarySubdivision);
    }
    if fill(
        &mut fields.secondary_subdivision,
        Field::SecondarySubdivision,
        protected,
        &lookup.secondary_subdivision,
    ) {
        changed.insert(Field::SecondarySubdivision);
    }
    if fill(
        &mut fields.cq_zone,
        Field::CqZone,
        protected,
        &lookup.cq_zone,
    ) {
        changed.insert(Field::CqZone);
    }
    if fill(
        &mut fields.itu_zone,
        Field::ItuZone,
        protected,
        &lookup.itu_zone,
    ) {
        changed.insert(Field::ItuZone);
    }
    changed
}

/// Apply the independent candidate/coverage decision for one native field.
fn fill<T: Clone>(
    target: &mut Option<T>,
    field: Field,
    protected: &BTreeSet<Field>,
    lookup: &FieldLookup<T>,
) -> bool {
    if target.is_some() || protected.contains(&field) || lookup.coverage != Coverage::Complete {
        return false;
    }
    let [candidate] = lookup.candidates.as_slice() else {
        return false;
    };
    *target = Some(candidate.value.clone());
    true
}

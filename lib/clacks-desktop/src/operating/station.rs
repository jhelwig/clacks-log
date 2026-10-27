//! Own-station form values and explicit reference/position edits.

use super::{FieldError, issue, optional, position::PositionForm};
use clacks_model::{
    contact::StationSnapshot,
    group::{DefinitionSet, normalize_reference},
    ids::GroupId,
    location::{CqZone, ItuZone, RecordedLocation},
    position::{GridLength, Position},
    reference::ReferenceAssociation,
    scalar::RecordedCallsign,
};
use std::collections::BTreeSet;

/// An existing association or an explicitly edited/new reference row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReferenceForm {
    /// Selected account group; absent while creating a row.
    pub group: Option<GroupId>,
    /// Raw optional identifier text.
    pub identifier: String,
    /// Exact saved association used until an explicit edit.
    original: Option<ReferenceAssociation>,
    /// Whether the user has explicitly edited this row.
    pub edited: bool,
}

impl ReferenceForm {
    /// Identify a pending association change using current group rules and its recorded baseline.
    pub fn is_changed(&self, definitions: &DefinitionSet) -> bool {
        match (&self.original, self.build(definitions)) {
            (Some(original), Ok(current)) => original != &current,
            _ => true,
        }
    }

    /// Start an incomplete reference row without inventing a group association.
    pub fn new() -> Self {
        Self {
            group: None,
            identifier: String::new(),
            original: None,
            edited: true,
        }
    }

    /// Keep existing recorded values independent of later definition recognition.
    fn from_association(value: &ReferenceAssociation) -> Self {
        Self {
            group: Some(value.group_id),
            identifier: value.identifier.clone().unwrap_or_default(),
            original: Some(value.clone()),
            edited: false,
        }
    }

    /// Apply group rules only to new or explicitly changed associations.
    fn build(&self, definitions: &DefinitionSet) -> Result<ReferenceAssociation, FieldError> {
        if !self.edited
            && let Some(original) = &self.original
        {
            return Ok(original.clone());
        }
        let group_id = self
            .group
            .ok_or_else(|| issue("Reference group", "Select a group"))?;
        normalize_reference(
            &ReferenceAssociation {
                group_id,
                identifier: optional(&self.identifier),
            },
            definitions.get(group_id),
        )
        .map_err(|e| issue("Reference identifier", e))
    }
}

/// Complete editable station facts, without profile identity or equipment ownership.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StationForm {
    /// Recorded station callsign text.
    pub callsign: String,
    /// Position evidence and derived display text.
    pub position: PositionForm,
    /// Recorded DX Century Club entity identifier.
    pub dxcc: String,
    /// Recorded state/province identifier.
    pub primary_subdivision: String,
    /// Recorded county/second subdivision identifier.
    pub secondary_subdivision: String,
    /// CQ operating zone text.
    pub cq_zone: String,
    /// International Telecommunication Union zone text.
    pub itu_zone: String,
    /// Station-side references in editable order.
    pub references: Vec<ReferenceForm>,
    /// Last valid position used to retain or clear manual geographic corrections.
    last_position: Option<Position>,
}

impl StationForm {
    /// Start uninitialized station entry without a placeholder callsign.
    pub fn empty() -> Self {
        Self {
            callsign: String::new(),
            position: PositionForm::from_position(None),
            dxcc: String::new(),
            primary_subdivision: String::new(),
            secondary_subdivision: String::new(),
            cq_zone: String::new(),
            itu_zone: String::new(),
            references: vec![],
            last_position: None,
        }
    }

    /// Copy a complete station snapshot, including absent fields and original references.
    pub fn from_snapshot(station: &StationSnapshot) -> Self {
        let mut form = Self::empty();
        form.callsign = station.callsign.as_str().to_owned();
        if let Some(location) = &station.location {
            form.position = PositionForm::from_position(location.position.as_ref());
            form.last_position = location.position.clone();
            form.dxcc = location.dxcc.clone().unwrap_or_default();
            form.primary_subdivision = location.primary_subdivision.clone().unwrap_or_default();
            form.secondary_subdivision = location.secondary_subdivision.clone().unwrap_or_default();
            form.cq_zone = location
                .cq_zone
                .map(|v| v.get().to_string())
                .unwrap_or_default();
            form.itu_zone = location
                .itu_zone
                .map(|v| v.get().to_string())
                .unwrap_or_default();
        }
        form.references = station
            .references
            .iter()
            .map(ReferenceForm::from_association)
            .collect();
        form
    }

    /// Change grid-area evidence and reset corrections only after a valid position change.
    pub fn edit_grid(&mut self, grid: String) {
        self.position.edit_grid(grid);
        self.position_changed();
    }

    /// Change explicit point evidence without borrowing precision from a grid center.
    pub fn edit_coordinates(&mut self, latitude: String, longitude: String) {
        self.position.edit_coordinates(latitude, longitude);
        self.position_changed();
    }

    /// Adjust display-locator precision while preserving an unchanged point's corrections.
    pub fn edit_precision(&mut self, length: GridLength) {
        self.position.edit_precision(length);
        self.position_changed();
    }

    /// Drop facts belonging to an old position, without guessing new geographic values.
    fn position_changed(&mut self) {
        let Ok(next) = self.position.build() else {
            return;
        };
        let same = match (&self.last_position, &next) {
            (Some(a), Some(b)) => a.same_geographic_position(b),
            (None, None) => true,
            _ => false,
        };
        if !same {
            self.dxcc.clear();
            self.primary_subdivision.clear();
            self.secondary_subdivision.clear();
            self.cq_zone.clear();
            self.itu_zone.clear();
        }
        self.last_position = next;
    }

    /// Validate station entry while preserving existing reference values verbatim.
    pub fn build(&self, definitions: &DefinitionSet) -> Result<StationSnapshot, Vec<FieldError>> {
        self.build_with_position(definitions, self.position.build())
    }

    /// Build recorded station facts, preserving native coarse historical areas.
    pub fn build_recorded(
        &self,
        definitions: &DefinitionSet,
    ) -> Result<StationSnapshot, Vec<FieldError>> {
        self.build_with_position(definitions, self.position.build_recorded())
    }

    /// Validate station fields with the position policy selected by the caller.
    fn build_with_position(
        &self,
        definitions: &DefinitionSet,
        position: Result<Option<Position>, String>,
    ) -> Result<StationSnapshot, Vec<FieldError>> {
        let mut errors = Vec::new();
        let callsign = match RecordedCallsign::new(&self.callsign) {
            Ok(v) => Some(v),
            Err(e) => {
                errors.push(issue("Station callsign", e));
                None
            }
        };
        let position = match position {
            Ok(v) => v,
            Err(e) => {
                errors.push(issue("Position", e));
                None
            }
        };
        let cq_zone = match zone(&self.cq_zone, "CQ zone", CqZone::new) {
            Ok(v) => v,
            Err(e) => {
                errors.push(e);
                None
            }
        };
        let itu_zone = match zone(&self.itu_zone, "ITU zone", ItuZone::new) {
            Ok(v) => v,
            Err(e) => {
                errors.push(e);
                None
            }
        };
        let mut references = BTreeSet::new();
        for row in &self.references {
            match row.build(definitions) {
                Ok(v) => {
                    references.insert(v);
                }
                Err(e) => errors.push(e),
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let Some(callsign) = callsign else {
            return Err(vec![issue("Station callsign", "A callsign is required")]);
        };
        let location = RecordedLocation {
            position,
            dxcc: optional(&self.dxcc),
            primary_subdivision: optional(&self.primary_subdivision),
            secondary_subdivision: optional(&self.secondary_subdivision),
            cq_zone,
            itu_zone,
        }
        .normalized();
        let station = StationSnapshot {
            callsign,
            location,
            references,
        };
        station.validate().map_err(|e| vec![issue("Station", e)])?;
        Ok(station)
    }
}

/// Parse an independently optional bounded zone with field-local feedback.
fn zone<T>(
    text: &str,
    field: &str,
    validate: impl FnOnce(u8) -> Result<T, clacks_model::error::ValueError>,
) -> Result<Option<T>, FieldError> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let value = text.trim().parse::<u8>().map_err(|e| issue(field, e))?;
    validate(value).map(Some).map_err(|e| issue(field, e))
}

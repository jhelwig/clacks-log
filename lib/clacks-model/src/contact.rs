//! Recorded contact snapshots, band/frequency authority and save-boundary validation.

use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A record invariant or profile-equipment reference that prevents acceptance.
///
/// Record operations propagate these concrete diagnostics through `anyhow::Error`;
/// callers may downcast when field-specific handling is needed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RecordError {
    /// Neither a frequency nor a band was supplied for a required pair.
    #[error("a frequency or band is required")]
    MissingBandAndFrequency,
    /// The contact end precedes its start.
    #[error("end {} precedes start {}", .end.unix_seconds(), .start.unix_seconds())]
    EndBeforeStart {
        /// Recorded contact start.
        start: UtcSecond,
        /// Rejected contact end.
        end: UtcSecond,
    },
    /// A present optional record contains no meaningful values.
    #[error("empty {field} must be normalized to absence before saving")]
    EmptyOptionalAggregate {
        /// Optional aggregate that must normalize to absence.
        field: String,
    },
    /// A present optional text value is blank instead of absent.
    #[error("blank optional {field} must be absent")]
    InvalidOptionalText {
        /// Field needing optional-text normalization.
        field: String,
    },
    /// A profile’s selected equipment configuration is unavailable.
    #[error("equipment configuration {id} is unavailable")]
    MissingEquipment {
        /// Configuration required by the profile.
        id: EquipmentConfigurationId,
    },
    /// Supplied equipment does not satisfy the profile’s reference.
    #[error("expected equipment {expected}, supplied {supplied}")]
    EquipmentIdMismatch {
        /// Configuration required by the profile.
        expected: EquipmentConfigurationId,
        /// Identity of the supplied configuration.
        supplied: EquipmentConfigurationId,
    },
}

/// A recorded frequency and/or band for one contact direction.
///
/// A newly supplied frequency determines the band using the selected catalog.
/// Deserialization preserves recorded values; it does not reinterpret old bands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BandFrequency {
    /// Separately recorded whole-hertz frequency, absent for a band-only pair.
    frequency: Option<Hertz>,
    /// Recorded band label, preserved independently of later recognition changes.
    band: Option<Band>,
}

impl BandFrequency {
    /// Create a primary pair or receive override from entry/import values.
    ///
    /// At least one value is required. A supplied frequency takes precedence over
    /// the band, including clearing the band if the catalog cannot classify it.
    /// Import adapters must report any correction to a conflicting supplied band.
    pub fn new(
        frequency: Option<Hertz>,
        band: Option<Band>,
        catalog: &RecognitionCatalog,
    ) -> anyhow::Result<Self> {
        let band = frequency
            .map(|f| Band::for_frequency(f, catalog))
            .unwrap_or(band);
        let value = Self { frequency, band };
        value.validate()?;
        Ok(value)
    }

    /// Return the separately recorded frequency, without inventing one for a band-only value.
    pub fn frequency(&self) -> Option<Hertz> {
        self.frequency
    }

    /// Borrow the recorded band label, which may be unfamiliar to the current catalog.
    pub fn band(&self) -> Option<&Band> {
        self.band.as_ref()
    }

    /// Apply a frequency edit and replace the band with its current catalog classification.
    ///
    /// An unclassified frequency clears the band. Use for an explicit edit, not
    /// when merely refreshing recognition of a saved record.
    pub fn set_frequency(&mut self, value: Hertz, catalog: &RecognitionCatalog) {
        self.frequency = Some(value);
        self.band = Band::for_frequency(value, catalog);
    }

    /// Apply a band-only edit and clear the previous frequency.
    ///
    /// Use when the operator specifies a band without a new frequency; retaining
    /// the old frequency would contradict that edit.
    pub fn set_band(&mut self, value: Band) {
        self.frequency = None;
        self.band = Some(value);
    }

    /// Select a whole receive override, or this primary pair when no override exists.
    ///
    /// A band-only override does not borrow the primary frequency, even if both
    /// bands match. Use this to interpret the effective receive direction.
    pub fn effective_receive<'a>(&'a self, override_pair: Option<&'a Self>) -> &'a Self {
        override_pair.unwrap_or(self)
    }

    /// Check that this pair has a frequency or a nonblank band.
    ///
    /// Use before accepting deserialized data. Existing band/frequency differences
    /// are preserved rather than recalculated against a newer recognition catalog.
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.frequency.is_none() && self.band.is_none() {
            return Err(RecordError::MissingBandAndFrequency.into());
        }
        if let Some(b) = &self.band {
            Band::from_recorded_label(b.label())?;
        }
        Ok(())
    }
}

/// Recorded station identity, operating location and participation for one side of a contact.
///
/// The same shape describes own and contacted stations. Directory facts are
/// separate and must not overwrite these recorded values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StationSnapshot {
    /// Required recorded station callsign, including any portable components.
    pub callsign: RecordedCallsign,
    /// Recorded operating-location details, selected as one source during fallback.
    pub location: Option<RecordedLocation>,
    /// Unordered, deduplicated participation/reference associations for this station.
    pub references: BTreeSet<ReferenceAssociation>,
}

/// Optional on-air operator identity, separate from station identity and directory names.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorInformation {
    /// Operator callsign, independently optional from the recorded name.
    pub callsign: Option<RecordedCallsign>,
    /// Operator name supplied by the operator or on-air exchange, not directory data.
    pub recorded_name: Option<String>,
}

impl OperatorInformation {
    /// Normalize optional operator text and collapse an all-absent operator to `None`.
    ///
    /// Use before saving prepared entry data; a callsign or nonblank name alone
    /// is sufficient to retain the operator snapshot.
    pub fn normalized(mut self) -> Option<Self> {
        self.recorded_name = self.recorded_name.as_deref().and_then(optional_text);
        (self.callsign.is_some() || self.recorded_name.is_some()).then_some(self)
    }
}

/// Own operator, transmit power and copied equipment associated with a contact.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnOperatingContext {
    /// Recorded own operator information, when known.
    pub operator: Option<OperatorInformation>,
    /// Recorded positive transmit power at one-milliwatt resolution.
    pub transmit_power: Option<Milliwatts>,
    /// Copied equipment facts; later configuration edits do not change this snapshot.
    pub equipment: Option<EquipmentValues>,
}

/// An account-owned saved contact with copied operating context and its complete tag set.
///
/// Constructing or deserializing this record is not a save operation. Prepare
/// optional values and validate before acceptance. Equality compares recorded
/// values as a whole, including tags and references, without consulting catalogs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Qso {
    /// Stable account-wide contact identity, independent of tags.
    pub id: QsoId,
    /// Required whole-second UTC start timestamp.
    pub start: UtcSecond,
    /// Optional whole-second UTC end, no earlier than start.
    pub end: Option<UtcSecond>,
    /// Own station snapshot captured for this contact.
    pub own_station: StationSnapshot,
    /// Contacted station snapshot captured for this contact.
    pub contacted_station: StationSnapshot,
    /// Own operator, power and equipment for this contact.
    pub own_context: OwnOperatingContext,
    /// Separately recorded contacted operator information, when known.
    pub contacted_operator: Option<OperatorInformation>,
    /// Primary frequency/band pair, used in both directions absent a receive override.
    pub primary: BandFrequency,
    /// Whole receive-direction override; band-only values do not borrow primary frequency.
    pub receive_override: Option<BandFrequency>,
    /// Recorded mode hierarchy and labels independent of recognition.
    pub mode: Mode,
    /// Optional sent signal report; report syntax is not mode-restricted here.
    pub sent_report: Option<String>,
    /// Optional received signal report; report syntax is not mode-restricted here.
    pub received_report: Option<String>,
    /// Optional recorded comment, preserving nonblank text.
    pub comment: Option<String>,
    /// Complete unordered set of logbook memberships; an empty set leaves the contact untagged.
    pub tags: BTreeSet<TagId>,
}

/// Reject present blank text at a record boundary without modifying recorded input.
pub(crate) fn validate_optional_text(value: Option<&str>, field: &str) -> anyhow::Result<()> {
    if value.is_some_and(|s| s.trim().is_empty()) {
        return Err(RecordError::InvalidOptionalText {
            field: field.into(),
        }
        .into());
    }
    Ok(())
}

impl StationSnapshot {
    /// Validate recorded location/reference text before saving the station snapshot.
    ///
    /// Reject empty present locations and blank optional identifiers. This does not
    /// require optional geography, validate group syntax, or infer directory facts.
    pub fn validate(&self) -> anyhow::Result<()> {
        if let Some(l) = &self.location {
            if !l.has_values() {
                return Err(RecordError::EmptyOptionalAggregate {
                    field: "location".into(),
                }
                .into());
            }
            for (name, text) in [
                ("dxcc", &l.dxcc),
                ("primary subdivision", &l.primary_subdivision),
                ("secondary subdivision", &l.secondary_subdivision),
            ] {
                validate_optional_text(text.as_deref(), name)?;
            }
        }
        for r in &self.references {
            validate_optional_text(r.identifier.as_deref(), "reference identifier")?;
        }
        Ok(())
    }
}

impl Qso {
    /// Check saved-contact invariants and nested values without modifying the contact.
    ///
    /// Use after entry/import preparation and before durable acceptance. Equal start
    /// and end times and contacts spanning midnight are valid. Returns concrete
    /// record/value diagnostics through `anyhow::Error`; account ownership, tag
    /// existence and service associations require separate application checks.
    pub fn validate(&self) -> anyhow::Result<()> {
        if let Some(end) = self.end
            && end < self.start
        {
            return Err(RecordError::EndBeforeStart {
                start: self.start,
                end,
            }
            .into());
        }
        self.primary.validate()?;
        if let Some(r) = &self.receive_override {
            r.validate()?;
        }
        self.own_station.validate()?;
        self.contacted_station.validate()?;
        Mode::from_recorded_identity(self.mode.identity())?;
        for (name, value) in [
            ("sent report", self.sent_report.as_deref()),
            ("received report", self.received_report.as_deref()),
            ("comment", self.comment.as_deref()),
        ] {
            validate_optional_text(value, name)?;
        }
        for operator in [&self.own_context.operator, &self.contacted_operator]
            .into_iter()
            .flatten()
        {
            if operator.callsign.is_none() && operator.recorded_name.is_none() {
                return Err(RecordError::EmptyOptionalAggregate {
                    field: "operator information".into(),
                }
                .into());
            }
            validate_optional_text(operator.recorded_name.as_deref(), "operator name")?;
        }
        if let Some(e) = &self.own_context.equipment {
            e.validate()?;
        }
        Ok(())
    }

    /// Compare the complete recorded working versions for whole-record reconciliation.
    ///
    /// Tag/reference order is irrelevant because those fields are sets. No provider
    /// lookup, catalog interpretation or automatic field merge participates.
    pub fn same_recorded_values(&self, other: &Self) -> bool {
        self == other
    }

    /// Prepare optional locations, operators, receive override and report/comment text for saving.
    ///
    /// Empty locations/operators collapse to absence; an empty receive override
    /// returns to primary-pair fallback. Nonblank text and explicit recorded facts
    /// remain unchanged. Equipment/reference strings must be prepared separately
    /// with `optional_text`; call `validate` afterward to enforce the full save boundary.
    pub fn normalize_optional_aggregates(&mut self) {
        if self
            .receive_override
            .as_ref()
            .is_some_and(|r| r.frequency.is_none() && r.band.is_none())
        {
            self.receive_override = None;
        }
        for station in [&mut self.own_station, &mut self.contacted_station] {
            station.location = station
                .location
                .take()
                .and_then(RecordedLocation::normalized);
        }
        self.own_context.operator = self
            .own_context
            .operator
            .take()
            .and_then(OperatorInformation::normalized);
        self.contacted_operator = self
            .contacted_operator
            .take()
            .and_then(OperatorInformation::normalized);
        for value in [
            &mut self.sent_report,
            &mut self.received_report,
            &mut self.comment,
        ] {
            *value = value.as_deref().and_then(optional_text);
        }
    }
}

impl EquipmentValues {
    /// Validate copied optional equipment text before accepting it into a saved record.
    ///
    /// Blank present strings fail validation; use `optional_text` when preparing
    /// entry values. This does not require any particular equipment field.
    pub fn validate(&self) -> anyhow::Result<()> {
        for (name, value) in [
            ("equipment", &self.equipment_description),
            ("antenna", &self.antenna_description),
            ("equipment notes", &self.notes),
        ] {
            validate_optional_text(value.as_deref(), name)?;
        }
        Ok(())
    }
}

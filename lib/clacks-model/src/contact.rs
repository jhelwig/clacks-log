//! Valid saved contact snapshots and explicit frequency/band edits.

pub mod input;

use crate::equipment::EquipmentValues;
use crate::ids::{EquipmentConfigurationId, QsoId, TagId};
use crate::location::RecordedLocation;
use crate::recognition::RecognitionCatalog;
use crate::recognition::band::Band;
use crate::recognition::mode::Mode;
use crate::reference::ReferenceAssociation;
use crate::scalar::{Hertz, Milliwatts, RecordedCallsign, UtcSecond, optional_text};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A record invariant or profile-equipment reference that prevents acceptance.
///
/// Variants carry the field or entity context needed for a targeted diagnostic.
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
/// Its serialized representation contains the recorded pair without a catalog
/// dependency. For construction and edits, see [`Self::new`],
/// [`Self::set_frequency`] and [`Self::set_band`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "BandFrequencyFields")]
pub struct BandFrequency {
    /// Separately recorded whole-hertz frequency, absent for a band-only pair.
    frequency: Option<Hertz>,
    /// Recorded band label, preserved independently of later recognition changes.
    band: Option<Band>,
}

impl BandFrequency {
    /// Restore a recorded pair without reclassifying it against a recognition catalog.
    ///
    /// Use for durable native values whose labels must survive catalog changes.
    /// At least one component is required; entry and import frequency authority
    /// belongs to [`Self::new`].
    pub fn from_recorded_pair(
        frequency: Option<Hertz>,
        band: Option<Band>,
    ) -> anyhow::Result<Self> {
        let value = Self { frequency, band };
        value.validate()?;
        Ok(value)
    }

    /// Create a primary pair or receive override from entry/import values.
    ///
    /// At least one value is required. A supplied frequency takes precedence over
    /// the band, including clearing the band if the catalog cannot classify it.
    /// The supplied band is not retained separately when a frequency is present.
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
    fn validate(&self) -> anyhow::Result<()> {
        if self.frequency.is_none() && self.band.is_none() {
            return Err(RecordError::MissingBandAndFrequency.into());
        }
        Ok(())
    }
}

/// Editable input checked before becoming a saved OperatorInformation.
#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorInformationFields {
    /// Operator callsign, independently optional from the recorded name.
    pub callsign: Option<RecordedCallsign>,
    /// Operator name supplied by the operator or on-air exchange, not directory data.
    pub recorded_name: Option<String>,
}

/// Optional on-air operator identity, separate from station identity and directory names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "OperatorInformationFields")]
pub struct OperatorInformation {
    /// Operator callsign, independently optional from the recorded name.
    callsign: Option<RecordedCallsign>,
    /// Operator name supplied by the operator or on-air exchange, not directory data.
    recorded_name: Option<String>,
}

impl OperatorInformation {
    /// Check that a present operator snapshot contains a callsign or nonblank recorded name.
    ///
    /// Use before saving a complete optional operator value; this does not normalize text.
    fn validate(&self) -> anyhow::Result<()> {
        if self.callsign.is_none() && self.recorded_name.is_none() {
            return Err(RecordError::EmptyOptionalAggregate {
                field: "operator information".into(),
            }
            .into());
        }
        validate_optional_text(self.recorded_name.as_deref(), "operator name")
    }
}

/// An account-owned saved contact with copied operating context and its complete tag set.
///
/// Construct through [`input::QsoFields`]; edits are checked on conversion back.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "input::QsoFields")]
pub struct Qso {
    /// Stable account-wide contact identity, independent of tags.
    id: QsoId,
    /// Required whole-second UTC start timestamp.
    start: UtcSecond,
    /// Optional whole-second UTC end, no earlier than start.
    end: Option<UtcSecond>,
    /// Own station snapshot captured for this contact.
    own_station: StationSnapshot,
    /// Contacted station snapshot captured for this contact.
    contacted_station: StationSnapshot,
    /// Own operator, power and equipment for this contact.
    own_context: OwnOperatingContext,
    /// Separately recorded contacted operator information, when known.
    contacted_operator: Option<OperatorInformation>,
    /// Recorded primary frequency/band pair.
    primary: BandFrequency,
    /// Optional separate receive pair; see [`crate::contact::BandFrequency::effective_receive`].
    receive_override: Option<BandFrequency>,
    /// Recorded mode hierarchy and labels independent of recognition.
    mode: Mode,
    /// Optional sent signal report; report syntax is not mode-restricted here.
    sent_report: Option<String>,
    /// Optional received signal report; report syntax is not mode-restricted here.
    received_report: Option<String>,
    /// Optional recorded comment text.
    comment: Option<String>,
    /// Complete unordered set of logbook memberships; an empty set leaves the contact untagged.
    tags: BTreeSet<TagId>,
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

impl Qso {
    /// Recover editable input for a complete, subsequently checked replacement.
    pub fn into_fields(self) -> input::QsoFields {
        self.into()
    }

    /// Stable account-wide contact identity, independent of tags.
    pub fn id(&self) -> QsoId {
        self.id
    }

    /// Required whole-second UTC start timestamp.
    pub fn start(&self) -> UtcSecond {
        self.start
    }

    /// Optional whole-second UTC end, no earlier than start.
    pub fn end(&self) -> Option<UtcSecond> {
        self.end
    }

    /// Own station snapshot captured for this contact.
    pub fn own_station(&self) -> &StationSnapshot {
        &self.own_station
    }

    /// Contacted station snapshot captured for this contact.
    pub fn contacted_station(&self) -> &StationSnapshot {
        &self.contacted_station
    }

    /// Own operator, power and equipment for this contact.
    pub fn own_context(&self) -> &OwnOperatingContext {
        &self.own_context
    }

    /// Separately recorded contacted operator information, when known.
    pub fn contacted_operator(&self) -> &Option<OperatorInformation> {
        &self.contacted_operator
    }

    /// Recorded primary frequency/band pair.
    pub fn primary(&self) -> &BandFrequency {
        &self.primary
    }

    /// Optional separate receive pair; see [`crate::contact::BandFrequency::effective_receive`].
    pub fn receive_override(&self) -> &Option<BandFrequency> {
        &self.receive_override
    }

    /// Recorded mode hierarchy and labels independent of recognition.
    pub fn mode(&self) -> &Mode {
        &self.mode
    }

    /// Optional sent signal report; report syntax is not mode-restricted here.
    pub fn sent_report(&self) -> &Option<String> {
        &self.sent_report
    }

    /// Optional received signal report; report syntax is not mode-restricted here.
    pub fn received_report(&self) -> &Option<String> {
        &self.received_report
    }

    /// Optional recorded comment text.
    pub fn comment(&self) -> &Option<String> {
        &self.comment
    }

    /// Complete unordered set of logbook memberships; an empty set leaves the contact untagged.
    pub fn tags(&self) -> &BTreeSet<TagId> {
        &self.tags
    }

    /// Reject inconsistent timestamps or blank optional contact text at construction.
    ///
    /// Use after entry/import preparation and before durable acceptance. Equal start
    /// and end times and contacts spanning midnight are valid. Returns concrete
    /// record/value diagnostics through `anyhow::Error`; account ownership, tag
    /// existence and service associations require separate application checks.
    fn validate(&self) -> anyhow::Result<()> {
        if let Some(end) = self.end
            && end < self.start
        {
            return Err(RecordError::EndBeforeStart {
                start: self.start,
                end,
            }
            .into());
        }

        for (name, value) in [
            ("sent report", self.sent_report.as_deref()),
            ("received report", self.received_report.as_deref()),
            ("comment", self.comment.as_deref()),
        ] {
            validate_optional_text(value, name)?;
        }

        Ok(())
    }
}

/// Recorded pair input validated without consulting a recognition catalog.
#[derive(Deserialize)]
struct BandFrequencyFields {
    /// Recorded frequency, if independently supplied.
    frequency: Option<Hertz>,
    /// Recorded band label, without catalog reinterpretation.
    band: Option<Band>,
}

impl TryFrom<BandFrequencyFields> for BandFrequency {
    type Error = anyhow::Error;

    fn try_from(fields: BandFrequencyFields) -> anyhow::Result<Self> {
        Self::from_recorded_pair(fields.frequency, fields.band)
    }
}

impl OperatorInformation {
    /// Recover editable input for a subsequently checked replacement.
    pub fn into_fields(self) -> OperatorInformationFields {
        self.into()
    }

    /// Operator callsign, independently optional from the recorded name.
    pub fn callsign(&self) -> &Option<RecordedCallsign> {
        &self.callsign
    }

    /// Operator name supplied by the operator or on-air exchange, not directory data.
    pub fn recorded_name(&self) -> &Option<String> {
        &self.recorded_name
    }
}

impl TryFrom<OperatorInformationFields> for OperatorInformation {
    type Error = anyhow::Error;

    fn try_from(fields: OperatorInformationFields) -> anyhow::Result<Self> {
        let value = Self {
            callsign: fields.callsign,
            recorded_name: fields.recorded_name,
        };
        value.validate()?;
        Ok(value)
    }
}

impl From<OperatorInformation> for OperatorInformationFields {
    fn from(value: OperatorInformation) -> Self {
        Self {
            callsign: value.callsign,
            recorded_name: value.recorded_name,
        }
    }
}

impl OperatorInformationFields {
    /// Normalize optional operator text and collapse an all-absent operator to `None`.
    ///
    /// Use before saving prepared entry data; a callsign or nonblank name alone
    /// is sufficient to retain the operator snapshot.
    pub fn normalized(mut self) -> Option<Self> {
        self.recorded_name = self.recorded_name.as_deref().and_then(optional_text);
        (self.callsign.is_some() || self.recorded_name.is_some()).then_some(self)
    }
}

/// Recorded station identity, operating location and participation for one side of a contact.
///
/// The same shape represents own and contacted stations. It contains recorded
/// station facts, not directory observations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StationSnapshot {
    /// Required recorded station callsign, including any portable components.
    pub callsign: RecordedCallsign,
    /// Optional recorded operating-location details.
    pub location: Option<RecordedLocation>,
    /// Unordered, deduplicated participation/reference associations for this station.
    pub references: BTreeSet<ReferenceAssociation>,
}

/// Own operator, transmit power and copied equipment associated with a contact.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnOperatingContext {
    /// Recorded own operator information, when known.
    pub operator: Option<OperatorInformation>,
    /// Recorded positive transmit power at one-milliwatt resolution.
    pub transmit_power: Option<Milliwatts>,
    /// Owned equipment facts with no live configuration reference.
    pub equipment: Option<EquipmentValues>,
}

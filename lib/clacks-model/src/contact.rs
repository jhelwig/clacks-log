use crate::*;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordError {
    MissingBandAndFrequency,
    EndBeforeStart {
        start: UtcSecond,
        end: UtcSecond,
    },
    EmptyOptionalAggregate {
        field: String,
    },
    InvalidOptionalText {
        field: String,
    },
    InvalidValue(ValueError),
    MissingEquipment {
        id: EquipmentConfigurationId,
    },
    EquipmentIdMismatch {
        expected: EquipmentConfigurationId,
        supplied: EquipmentConfigurationId,
    },
}

impl fmt::Display for RecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBandAndFrequency => write!(f, "a frequency or band is required"),
            Self::EndBeforeStart { start, end } => write!(
                f,
                "end {} precedes start {}",
                end.unix_seconds(),
                start.unix_seconds()
            ),
            Self::EmptyOptionalAggregate { field } => write!(
                f,
                "empty {field} must be normalized to absence before saving"
            ),
            Self::InvalidOptionalText { field } => {
                write!(f, "blank optional {field} must be absent")
            }
            Self::InvalidValue(e) => e.fmt(f),
            Self::MissingEquipment { id } => {
                write!(f, "equipment configuration {id} is unavailable")
            }
            Self::EquipmentIdMismatch { expected, supplied } => {
                write!(f, "expected equipment {expected}, supplied {supplied}")
            }
        }
    }
}

impl std::error::Error for RecordError {}

impl From<ValueError> for RecordError {
    fn from(value: ValueError) -> Self {
        Self::InvalidValue(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BandFrequency {
    frequency: Option<Hertz>,
    band: Option<Band>,
}

impl BandFrequency {
    pub fn new(
        frequency: Option<Hertz>,
        band: Option<Band>,
        catalog: &RecognitionCatalog,
    ) -> Result<Self, RecordError> {
        let band = frequency
            .map(|f| Band::for_frequency(f, catalog))
            .unwrap_or(band);
        let value = Self { frequency, band };
        value.validate()?;
        Ok(value)
    }

    pub fn frequency(&self) -> Option<Hertz> {
        self.frequency
    }

    pub fn band(&self) -> Option<&Band> {
        self.band.as_ref()
    }

    pub fn set_frequency(&mut self, value: Hertz, catalog: &RecognitionCatalog) {
        self.frequency = Some(value);
        self.band = Band::for_frequency(value, catalog);
    }

    pub fn set_band(&mut self, value: Band) {
        self.frequency = None;
        self.band = Some(value);
    }

    pub fn effective_receive<'a>(&'a self, override_pair: Option<&'a Self>) -> &'a Self {
        override_pair.unwrap_or(self)
    }

    pub fn validate(&self) -> Result<(), RecordError> {
        if self.frequency.is_none() && self.band.is_none() {
            return Err(RecordError::MissingBandAndFrequency);
        }
        if let Some(b) = &self.band {
            Band::from_recorded_label(b.label(), RecognitionCatalog::adif_318())?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StationSnapshot {
    pub callsign: RecordedCallsign,
    pub location: Option<RecordedLocation>,
    pub references: BTreeSet<ReferenceAssociation>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorInformation {
    pub callsign: Option<RecordedCallsign>,
    pub recorded_name: Option<String>,
}

impl OperatorInformation {
    pub fn normalized(mut self) -> Option<Self> {
        self.recorded_name = self.recorded_name.as_deref().and_then(optional_text);
        (self.callsign.is_some() || self.recorded_name.is_some()).then_some(self)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnOperatingContext {
    pub operator: Option<OperatorInformation>,
    pub transmit_power: Option<Milliwatts>,
    pub equipment: Option<EquipmentValues>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Qso {
    pub id: QsoId,
    pub start: UtcSecond,
    pub end: Option<UtcSecond>,
    pub own_station: StationSnapshot,
    pub contacted_station: StationSnapshot,
    pub own_context: OwnOperatingContext,
    pub contacted_operator: Option<OperatorInformation>,
    pub primary: BandFrequency,
    pub receive_override: Option<BandFrequency>,
    pub mode: Mode,
    pub sent_report: Option<String>,
    pub received_report: Option<String>,
    pub comment: Option<String>,
    pub tags: BTreeSet<TagId>,
}

pub(crate) fn validate_optional_text(value: Option<&str>, field: &str) -> Result<(), RecordError> {
    if value.is_some_and(|s| s.trim().is_empty()) {
        return Err(RecordError::InvalidOptionalText {
            field: field.into(),
        });
    }
    Ok(())
}

impl StationSnapshot {
    pub fn validate(&self) -> Result<(), RecordError> {
        if let Some(l) = &self.location {
            if !l.has_values() {
                return Err(RecordError::EmptyOptionalAggregate {
                    field: "location".into(),
                });
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
    pub fn validate(&self) -> Result<(), RecordError> {
        if let Some(end) = self.end
            && end < self.start
        {
            return Err(RecordError::EndBeforeStart {
                start: self.start,
                end,
            });
        }
        self.primary.validate()?;
        if let Some(r) = &self.receive_override {
            r.validate()?;
        }
        self.own_station.validate()?;
        self.contacted_station.validate()?;
        Mode::from_recorded_identity(self.mode.identity(), RecognitionCatalog::adif_318())?;
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
                });
            }
            validate_optional_text(operator.recorded_name.as_deref(), "operator name")?;
        }
        if let Some(e) = &self.own_context.equipment {
            e.validate()?;
        }
        Ok(())
    }

    pub fn same_recorded_values(&self, other: &Self) -> bool {
        self == other
    }

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
    pub fn validate(&self) -> Result<(), RecordError> {
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

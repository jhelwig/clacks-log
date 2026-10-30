//! Recorded geographic details independent of directory and dataset metadata.

use crate::error::ValueError;
use crate::position::Position;
use serde::{Deserialize, Serialize};

/// Define a bounded operating-zone number with validated deserialization.
macro_rules! zone {
    ($name:ident,$max:literal,$description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(try_from = "u8", into = "u8")]
        pub struct $name(u8);

        impl $name {
            /// Validate an operating-zone number before recording it.
            ///
            /// Values outside this zone system’s range return `ValueError::InvalidZone`.
            pub fn new(value: u8) -> Result<Self, ValueError> {
                if (1..=$max).contains(&value) {
                    Ok(Self(value))
                } else {
                    Err(ValueError::InvalidZone {
                        system: stringify!($name).into(),
                        value,
                    })
                }
            }

            /// Return the zone number for display or interchange output.
            pub fn get(self) -> u8 {
                self.0
            }
        }

        impl TryFrom<u8> for $name {
            type Error = ValueError;

            fn try_from(value: u8) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$name> for u8 {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

zone!(
    CqZone,
    40,
    "A CQ operating-zone number in the inclusive range 1–40."
);
zone!(
    ItuZone,
    90,
    "An International Telecommunication Union (ITU) operating-zone number from 1 through 90."
);

/// Editable input checked before becoming a saved RecordedLocation.
#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedLocationFields {
    /// Grid-area or explicit-point evidence for the operating location.
    pub position: Option<Position>,
    /// DX Century Club (DXCC) entity identifier, when recorded.
    pub dxcc: Option<String>,
    /// Recorded first-level administrative subdivision, such as a U.S. state.
    pub primary_subdivision: Option<String>,
    /// Recorded second-level administrative subdivision, such as a U.S. county.
    pub secondary_subdivision: Option<String>,
    /// Recorded CQ operating zone.
    pub cq_zone: Option<CqZone>,
    /// Recorded International Telecommunication Union (ITU) operating zone.
    pub itu_zone: Option<ItuZone>,
}

/// A station’s recorded geographic details, selected as a whole during fallback.
///
/// Individual details may be unknown; missing fields do not authorize filling
/// from a different source. Catalog coordinates do not constitute this record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RecordedLocationFields")]
pub struct RecordedLocation {
    /// Grid-area or explicit-point evidence for the operating location.
    position: Option<Position>,
    /// DX Century Club (DXCC) entity identifier, when recorded.
    dxcc: Option<String>,
    /// Recorded first-level administrative subdivision, such as a U.S. state.
    primary_subdivision: Option<String>,
    /// Recorded second-level administrative subdivision, such as a U.S. county.
    secondary_subdivision: Option<String>,
    /// Recorded CQ operating zone.
    cq_zone: Option<CqZone>,
    /// Recorded International Telecommunication Union (ITU) operating zone.
    itu_zone: Option<ItuZone>,
}

impl RecordedLocation {
    /// Test whether any position, nonblank geographic identifier or zone is present.
    ///
    /// Use to distinguish an absent location from meaningful but incomplete details;
    /// this does not validate identifiers against geographic datasets.
    fn has_values(&self) -> bool {
        self.position.is_some()
            || [
                &self.dxcc,
                &self.primary_subdivision,
                &self.secondary_subdivision,
            ]
            .iter()
            .any(|v| v.as_ref().is_some_and(|s| !s.trim().is_empty()))
            || self.cq_zone.is_some()
            || self.itu_zone.is_some()
    }
}

impl RecordedLocation {
    /// Recover editable input for a subsequently checked replacement.
    pub fn into_fields(self) -> RecordedLocationFields {
        self.into()
    }

    /// Grid-area or explicit-point evidence for the operating location.
    pub fn position(&self) -> &Option<Position> {
        &self.position
    }

    /// DX Century Club (DXCC) entity identifier, when recorded.
    pub fn dxcc(&self) -> &Option<String> {
        &self.dxcc
    }

    /// Recorded first-level administrative subdivision, such as a U.S. state.
    pub fn primary_subdivision(&self) -> &Option<String> {
        &self.primary_subdivision
    }

    /// Recorded second-level administrative subdivision, such as a U.S. county.
    pub fn secondary_subdivision(&self) -> &Option<String> {
        &self.secondary_subdivision
    }

    /// Recorded CQ operating zone.
    pub fn cq_zone(&self) -> Option<CqZone> {
        self.cq_zone
    }

    /// Recorded International Telecommunication Union (ITU) operating zone.
    pub fn itu_zone(&self) -> Option<ItuZone> {
        self.itu_zone
    }
}

impl TryFrom<RecordedLocationFields> for RecordedLocation {
    type Error = anyhow::Error;

    fn try_from(fields: RecordedLocationFields) -> anyhow::Result<Self> {
        let value = Self {
            position: fields.position,
            dxcc: fields.dxcc,
            primary_subdivision: fields.primary_subdivision,
            secondary_subdivision: fields.secondary_subdivision,
            cq_zone: fields.cq_zone,
            itu_zone: fields.itu_zone,
        };
        if !value.has_values() {
            return Err(crate::contact::RecordError::EmptyOptionalAggregate {
                field: "location".into(),
            }
            .into());
        }
        for (name, text) in [
            ("dxcc", &value.dxcc),
            ("primary subdivision", &value.primary_subdivision),
            ("secondary subdivision", &value.secondary_subdivision),
        ] {
            crate::contact::validate_optional_text(text.as_deref(), name)?;
        }

        Ok(value)
    }
}

impl From<RecordedLocation> for RecordedLocationFields {
    fn from(value: RecordedLocation) -> Self {
        Self {
            position: value.position,
            dxcc: value.dxcc,
            primary_subdivision: value.primary_subdivision,
            secondary_subdivision: value.secondary_subdivision,
            cq_zone: value.cq_zone,
            itu_zone: value.itu_zone,
        }
    }
}

impl RecordedLocationFields {
    /// Remove blank geographic identifiers and return `None` when no details remain.
    ///
    /// Use when preparing entry/import data before saving. Nonblank strings, position
    /// meaning and manual geographic corrections are preserved.
    pub fn normalized(mut self) -> Option<Self> {
        for value in [
            &mut self.dxcc,
            &mut self.primary_subdivision,
            &mut self.secondary_subdivision,
        ] {
            if value.as_ref().is_some_and(|s| s.trim().is_empty()) {
                *value = None;
            }
        }
        (self.position.is_some()
            || self.dxcc.is_some()
            || self.primary_subdivision.is_some()
            || self.secondary_subdivision.is_some()
            || self.cq_zone.is_some()
            || self.itu_zone.is_some())
        .then_some(self)
    }
}

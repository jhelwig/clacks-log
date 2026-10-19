//! Recorded geographic details independent of directory and dataset metadata.

use crate::{Position, ValueError};
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

/// A station’s recorded geographic details, selected as a whole during fallback.
///
/// Individual details may be unknown; missing fields do not authorize filling
/// from a different source. Catalog coordinates do not constitute this record.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedLocation {
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

impl RecordedLocation {
    /// Test whether any position, nonblank geographic identifier or zone is present.
    ///
    /// Use to distinguish an absent location from meaningful but incomplete details;
    /// this does not validate identifiers against geographic datasets.
    pub fn has_values(&self) -> bool {
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
        self.has_values().then_some(self)
    }
}

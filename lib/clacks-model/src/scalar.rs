//! Validated station text, physical quantities and UTC timestamps.

use crate::error::ValueError;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

/// A nonblank recorded callsign, trimmed and uppercased while retaining slash components.
///
/// This is the contact’s recorded identity, not a base callsign for directory lookup.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RecordedCallsign(String);

impl RecordedCallsign {
    /// Create a recorded callsign from entry or import text.
    ///
    /// Rejects blank input. Prefixes, suffixes and slash components are preserved;
    /// this does not validate licensing or derive a directory lookup key.
    pub fn new(value: &str) -> Result<Self, ValueError> {
        let value = value.trim().to_uppercase();
        if value.is_empty() {
            return Err(ValueError::EmptyText {
                field: "callsign".into(),
            });
        }
        Ok(Self(value))
    }

    /// Borrow the normalized recorded callsign for display or interchange output.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RecordedCallsign {
    type Error = ValueError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl From<RecordedCallsign> for String {
    fn from(value: RecordedCallsign) -> Self {
        value.0
    }
}

/// Convert blank optional entry text to absence and preserve every character otherwise.
///
/// Use when preparing native optional strings. This does not distinguish an
/// omitted import field from an explicit clear; both need separate input-state
/// representation when that distinction matters.
pub fn optional_text(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_owned())
}

/// Define a positive integer physical quantity with validated deserialization.
macro_rules! quantity {
    ($name:ident, $unit:literal) => {
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(try_from = "u64", into = "u64")]
        #[doc = concat!("A positive whole-number quantity measured in ", $unit, ". Zero is invalid.")]
        pub struct $name(NonZeroU64);

        impl $name {
            /// Validate a whole-unit measurement before recording it.
            ///
            /// Returns `ValueError::ZeroQuantity` for zero; no scaling or rounding is performed.
            pub fn new(value: u64) -> Result<Self, ValueError> {
                NonZeroU64::new(value)
                    .map(Self)
                    .ok_or(ValueError::ZeroQuantity { unit: $unit.into() })
            }

            /// Return the measurement in the type’s named base unit.
            pub fn get(self) -> u64 {
                self.0.get()
            }
        }

        impl TryFrom<u64> for $name {
            type Error = ValueError;

            fn try_from(value: u64) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$name> for u64 {
            fn from(value: $name) -> Self {
                value.get()
            }
        }
    };
}

quantity!(Hertz, "hertz");
quantity!(Milliwatts, "milliwatts");

/// A contact timestamp in whole Unix seconds relative to 1970-01-01 UTC.
///
/// Negative values represent times before the epoch. This value carries no
/// subsecond precision or causal ordering information.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UtcSecond(i64);

impl UtcSecond {
    /// Represent an already interpreted UTC instant as whole Unix seconds.
    ///
    /// Use for contact start/end times; all `i64` values are accepted. Calendar
    /// parsing and time-zone conversion belong to the caller.
    pub fn from_unix_seconds(value: i64) -> Self {
        Self(value)
    }

    /// Return signed seconds since the Unix epoch for formatting or interchange conversion.
    pub fn unix_seconds(self) -> i64 {
        self.0
    }
}

/// An operational observation timestamp with a fractional second.
///
/// Use for provider lookups and catalog acquisition, not contact times.
/// The stored resolution does not imply an accurately synchronized clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "TimestampFields", into = "TimestampFields")]
pub struct UtcTimestamp {
    /// Signed whole Unix seconds at the start of this timestamp’s fractional second.
    seconds: i64,
    /// Nonnegative fractional nanoseconds, validated to remain below one second.
    nanoseconds: u32,
}

impl UtcTimestamp {
    /// Create an operational timestamp from Unix seconds and a nonnegative fraction.
    ///
    /// Nanoseconds must be less than 1,000,000,000; excess values return
    /// `ValueError::InvalidTimestamp` rather than carrying into the seconds.
    pub fn new(seconds: i64, nanoseconds: u32) -> Result<Self, ValueError> {
        if nanoseconds >= 1_000_000_000 {
            return Err(ValueError::InvalidTimestamp { nanoseconds });
        }
        Ok(Self {
            seconds,
            nanoseconds,
        })
    }
}

/// Serialized operational timestamp fields validated by `UtcTimestamp::new`.
#[derive(Serialize, Deserialize)]
struct TimestampFields {
    /// Signed whole Unix seconds at the start of this timestamp’s fractional second.
    seconds: i64,
    /// Nonnegative fractional nanoseconds, validated to remain below one second.
    nanoseconds: u32,
}

impl TryFrom<TimestampFields> for UtcTimestamp {
    type Error = ValueError;

    fn try_from(value: TimestampFields) -> Result<Self, Self::Error> {
        Self::new(value.seconds, value.nanoseconds)
    }
}

impl From<UtcTimestamp> for TimestampFields {
    fn from(value: UtcTimestamp) -> Self {
        Self {
            seconds: value.seconds,
            nanoseconds: value.nanoseconds,
        }
    }
}

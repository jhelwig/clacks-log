use crate::ValueError;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RecordedCallsign(String);

impl RecordedCallsign {
    pub fn new(value: &str) -> Result<Self, ValueError> {
        let value = value.trim().to_uppercase();
        if value.is_empty() {
            return Err(ValueError::EmptyText {
                field: "callsign".into(),
            });
        }
        Ok(Self(value))
    }

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

pub fn optional_text(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_owned())
}

macro_rules! quantity {
    ($name:ident, $unit:literal) => {
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(try_from = "u64", into = "u64")]
        pub struct $name(NonZeroU64);

        impl $name {
            pub fn new(value: u64) -> Result<Self, ValueError> {
                NonZeroU64::new(value)
                    .map(Self)
                    .ok_or(ValueError::ZeroQuantity { unit: $unit.into() })
            }

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UtcSecond(i64);

impl UtcSecond {
    pub fn from_unix_seconds(value: i64) -> Self {
        Self(value)
    }

    pub fn unix_seconds(self) -> i64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "TimestampFields", into = "TimestampFields")]
pub struct UtcTimestamp {
    seconds: i64,
    nanoseconds: u32,
}

impl UtcTimestamp {
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

#[derive(Serialize, Deserialize)]
struct TimestampFields {
    seconds: i64,
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

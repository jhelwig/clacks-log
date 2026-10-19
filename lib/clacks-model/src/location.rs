use crate::{Position, ValueError};
use serde::{Deserialize, Serialize};

macro_rules! zone {
    ($name:ident,$max:literal) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(try_from = "u8", into = "u8")]
        pub struct $name(u8);

        impl $name {
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

zone!(CqZone, 40);
zone!(ItuZone, 90);

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedLocation {
    pub position: Option<Position>,
    pub dxcc: Option<String>,
    pub primary_subdivision: Option<String>,
    pub secondary_subdivision: Option<String>,
    pub cq_zone: Option<CqZone>,
    pub itu_zone: Option<ItuZone>,
}

impl RecordedLocation {
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

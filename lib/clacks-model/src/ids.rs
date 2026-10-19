use crate::ValueError;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

macro_rules! entity_ids {
    ($($name:ident),+ $(,)?) => {$(
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(ulid::Ulid);

        impl $name {
            pub fn new() -> Self { Self(ulid::Ulid::generate()) }

            pub fn from_u128(value: u128) -> Self { Self(value.into()) }
        }

        impl Default for $name {
            fn default() -> Self { Self::new() }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) }
        }

        impl FromStr for $name {
            type Err = ValueError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                value.parse().map(Self).map_err(|_| ValueError::InvalidId { value: value.into() })
            }
        }

        impl TryFrom<String> for $name {
            type Error = ValueError;

            fn try_from(value: String) -> Result<Self, Self::Error> { value.parse() }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self { value.to_string() }
        }
    )+};
}

entity_ids!(
    QsoId,
    TagId,
    ProfileId,
    EquipmentConfigurationId,
    GroupId,
    CatalogId,
    OutputProfileId
);

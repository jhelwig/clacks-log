//! Distinct stable entity IDs with string serialization; ordering is not causality.

use crate::error::ValueError;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

/// Define noninterchangeable entity identities with common generation and string conversion.
macro_rules! entity_ids {
    ($( $(#[$meta:meta])* $name:ident),+ $(,)?) => {$(
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(ulid::Ulid);

        impl $name {
            /// Generate a fresh identity for a new native entity.
            ///
            /// Use the existing ID when editing or synchronizing an entity. Sort order
            /// must not be used to establish causal edit order.
            pub fn new() -> Self { Self(ulid::Ulid::generate()) }
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
    /// Stable identity of an account-owned contact.
    QsoId,
    /// Stable identity of a logbook tag independent of its display name.
    TagId,
    /// Stable identity of a saved station-location profile.
    ProfileId,
    /// Stable identity of a reusable equipment configuration.
    EquipmentConfigurationId,
    /// Stable identity of a participation/reference group.
    GroupId,
    /// Stable identity of a reference dataset independent of its upstream version.
    CatalogId,
    /// Stable identity of an export output profile.
    OutputProfileId
);

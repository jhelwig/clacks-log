//! Public remote collections and exact external-record identity.

use super::error::ServiceError;
use crate::{
    ids::DestinationId,
    scalar::{RecordedCallsign, UtcSecond},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::{cmp::Ordering, fmt, str::FromStr};

/// Define a validated decimal provider identity without imposing a machine-width limit.
macro_rules! qrz_id {
    ($name:ident, $allow_zero:literal, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            /// Parse decimal digits and remove leading zeros without changing numeric identity.
            ///
            /// Reject signs, whitespace, fractions and empty input. No integer-width limit applies.
            pub fn new(value: &str) -> Result<Self, ServiceError> {
                if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(ServiceError::Invalid {
                        detail: format!("invalid decimal QRZ identity {value:?}"),
                    });
                }
                let digits = value.trim_start_matches('0');
                let digits = if digits.is_empty() { "0" } else { digits };
                if !$allow_zero && digits == "0" {
                    return Err(ServiceError::Invalid {
                        detail: "QRZ book ID must be positive".into(),
                    });
                }
                Ok(Self(digits.into()))
            }

            /// Borrow canonical decimal digits for provider requests and lossless persistence.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Ord for $name {
            fn cmp(&self, other: &Self) -> Ordering {
                self.0
                    .len()
                    .cmp(&other.0.len())
                    .then_with(|| self.0.cmp(&other.0))
            }
        }

        impl PartialOrd for $name {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = ServiceError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }

        impl TryFrom<String> for $name {
            type Error = ServiceError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(&value)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

qrz_id!(
    QrzBookId,
    false,
    "A positive decimal QRZ logbook identity, independent of access credentials."
);
qrz_id!(
    QrzRecordId,
    true,
    "A decimal QRZ record identity with numeric ordering and lossless pagination."
);

impl QrzRecordId {
    /// Compute the next inclusive pagination threshold after a returned record ID.
    ///
    /// Use after selecting the greatest returned ID. Decimal carry preserves values beyond `u64`.
    pub fn next_for_pagination(&self) -> Self {
        let mut carry = true;
        let mut digits: Vec<u8> = self
            .0
            .bytes()
            .rev()
            .map(|digit| {
                if !carry {
                    digit
                } else if digit == b'9' {
                    b'0'
                } else {
                    carry = false;
                    digit + 1
                }
            })
            .collect();
        if carry {
            digits.push(b'1');
        }
        Self(digits.into_iter().rev().map(char::from).collect())
    }
}

/// External logbook provider, independent of directory providers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Service {
    /// Logbook of The World.
    Lotw,
    /// A QRZ logbook.
    Qrz,
}

/// Public remote collection binding; contains no access secrets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteCollection {
    /// A configured LoTW account, not a signing certificate.
    Lotw {
        /// Public account identifier supplied by configuration.
        account: String,
    },
    /// A QRZ logbook whose provider identity may not yet be established.
    Qrz {
        /// Known provider book ID; absence does not imply a different book.
        book_id: Option<QrzBookId>,
    },
}

/// Historical public destination description retained independently of credentials.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Destination {
    /// Stable identity of the remote namespace.
    pub id: DestinationId,
    /// Binding to the actual remote collection.
    pub collection: RemoteCollection,
    /// Human-readable context; not a uniqueness key.
    pub description: String,
}

impl Destination {
    /// Identify the provider without consulting credentials or display names.
    pub fn service(&self) -> Service {
        match self.collection {
            RemoteCollection::Lotw { .. } => Service::Lotw,
            RemoteCollection::Qrz { .. } => Service::Qrz,
        }
    }
}

/// Exact retained LoTW identity using concrete submitted service values.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LotwRecordKey {
    /// Full own station identity, including slash components.
    pub own_call: RecordedCallsign,
    /// Full contacted station identity.
    pub worked_call: RecordedCallsign,
    /// Start instant to the second.
    pub start: UtcSecond,
    /// Primary service band.
    pub band: String,
    /// Effective receive service band.
    pub receive_band: String,
    /// Actual mapped mode, not its award group.
    pub mode: String,
}

/// Provider identity within a collection; matching windows are not identities.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RemoteRecordId {
    /// Exact LoTW composite key.
    Lotw(LotwRecordKey),
    /// QRZ provider-assigned LOGID.
    Qrz(QrzRecordId),
}

/// Destination-qualified external identity; account qualification is supplied by its owner.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ExternalRecordKey {
    /// Stable destination namespace.
    pub destination: DestinationId,
    /// Provider record identity.
    pub record: RemoteRecordId,
}

/// Validate an account registry before treating its destinations as distinct namespaces.
///
/// Unknown QRZ book identities require configuration reconciliation when another
/// QRZ destination exists. Repeated identical descriptors are harmless; unequal
/// descriptors for the same Clacks ID and duplicate established collections fail.
pub fn validate_destinations(destinations: &[Destination]) -> anyhow::Result<()> {
    let mut unique = BTreeMap::new();
    for d in destinations {
        if let Some(old) = unique.insert(d.id, d)
            && old != d
        {
            return Err(ServiceError::Invalid {
                detail: format!("destination {} has conflicting descriptions", d.id),
            }
            .into());
        }
        match &d.collection {
            RemoteCollection::Lotw { account } if account.trim().is_empty() => {
                return Err(ServiceError::Invalid {
                    detail: format!("destination {} has blank LoTW account", d.id),
                }
                .into());
            }
            _ => {}
        }
    }
    for (i, a) in unique.values().enumerate() {
        for b in unique.values().skip(i + 1) {
            let conflict = match (&a.collection, &b.collection) {
                (RemoteCollection::Lotw { account: a }, RemoteCollection::Lotw { account: b }) => {
                    a.trim().eq_ignore_ascii_case(b.trim())
                }
                (RemoteCollection::Qrz { book_id: a }, RemoteCollection::Qrz { book_id: b }) => {
                    a.is_none() || b.is_none() || a == b
                }
                _ => false,
            };
            if conflict {
                return Err(ServiceError::Invalid {
                    detail: format!(
                        "destinations {} and {} do not establish distinct collections",
                        a.id, b.id
                    ),
                }
                .into());
            }
        }
    }
    Ok(())
}

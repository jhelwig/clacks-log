//! Public remote collections and exact external-record identity.

use super::error::ServiceError;
use crate::{
    ids::DestinationId,
    scalar::{RecordedCallsign, UtcSecond},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
        book_id: Option<u64>,
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
    Qrz(u64),
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
        if let Some(old) = unique.insert(d.id, d) {
            if old != d {
                return Err(ServiceError::Invalid {
                    detail: format!("destination {} has conflicting descriptions", d.id),
                }
                .into());
            }
        }
        match &d.collection {
            RemoteCollection::Lotw { account } if account.trim().is_empty() => {
                return Err(ServiceError::Invalid {
                    detail: format!("destination {} has blank LoTW account", d.id),
                }
                .into());
            }
            RemoteCollection::Qrz { book_id: Some(0) } => {
                return Err(ServiceError::Invalid {
                    detail: format!("destination {} has zero QRZ book ID", d.id),
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

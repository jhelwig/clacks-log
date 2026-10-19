//! Station-side participation associations with data-defined group behavior.

use crate::GroupId;
use serde::{Deserialize, Serialize};

/// Participation in one reference group, optionally qualified by an identifier.
///
/// Use in own/contacted station sets. A missing identifier denotes group-only
/// participation. The value contains no resolved definition or catalog metadata;
/// ordering and equality use its recorded group ID and identifier.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ReferenceAssociation {
    /// Stable group identity used to find behavior in account-shared definitions.
    pub group_id: GroupId,
    /// Optional recorded reference value; syntax and normalization are group-defined.
    pub identifier: Option<String>,
}

/// The station whose recorded facts or reference set an operation addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum StationSide {
    /// The operator’s own station.
    Own,
    /// The station worked during the contact.
    Contacted,
}

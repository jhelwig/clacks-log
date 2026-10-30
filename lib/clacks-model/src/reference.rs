//! Station-side participation associations with data-defined group behavior.

use crate::ids::GroupId;
use serde::{Deserialize, Serialize};

/// Participation in one reference group, optionally qualified by an identifier.
///
/// Use in own/contacted station sets. A missing identifier denotes group-only
/// participation. The value contains no resolved definition or catalog metadata;
/// ordering and equality use its recorded group ID and identifier.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "ReferenceFields")]
pub struct ReferenceAssociation {
    /// Stable group identity used to find behavior in account-shared definitions.
    pub group_id: GroupId,
    /// Optional recorded reference value; syntax and normalization are group-defined.
    identifier: Option<String>,
}

/// The station whose recorded facts or reference set an operation addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum StationSide {
    /// The operator’s own station.
    Own,
    /// The station worked during the contact.
    Contacted,
}

impl ReferenceAssociation {
    /// Record a group association without interpreting or normalizing its identifier.
    ///
    /// A present identifier must be nonblank; group-specific rules are applied
    /// separately for explicit entry and edits.
    pub fn new(group_id: GroupId, identifier: Option<String>) -> anyhow::Result<Self> {
        crate::contact::validate_optional_text(identifier.as_deref(), "reference identifier")?;
        Ok(Self {
            group_id,
            identifier,
        })
    }

    /// Borrow the exact recorded identifier, or absence for group-only participation.
    pub fn identifier(&self) -> &Option<String> {
        &self.identifier
    }
}

/// Native reference input checked at deserialization.
#[derive(Deserialize)]
struct ReferenceFields {
    /// Recorded reference group identity.
    group_id: GroupId,
    /// Optional recorded identifier, checked for nonblank text.
    identifier: Option<String>,
}

impl TryFrom<ReferenceFields> for ReferenceAssociation {
    type Error = anyhow::Error;

    fn try_from(fields: ReferenceFields) -> anyhow::Result<Self> {
        Self::new(fields.group_id, fields.identifier)
    }
}

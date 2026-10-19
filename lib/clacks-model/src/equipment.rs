//! Equipment facts copied into operating defaults and historical contacts.

use serde::{Deserialize, Serialize};

/// Equipment descriptions copied independently of reusable configuration identities.
///
/// A contact retains these facts even if its source configuration is renamed,
/// edited or deleted. Configuration display names are not part of the snapshot.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentValues {
    /// Optional rig/equipment description.
    pub equipment_description: Option<String>,
    /// Optional antenna description, independently optional from equipment.
    pub antenna_description: Option<String>,
    /// Optional equipment notes, preserving nonblank text.
    pub notes: Option<String>,
}

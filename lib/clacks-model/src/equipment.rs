//! Equipment facts copied into operating defaults and historical contacts.

use serde::{Deserialize, Serialize};

/// Equipment descriptions copied independently of reusable configuration identities.
///
/// These are owned descriptions, with no configuration identity, display name
/// or live reference to a reusable setup.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentValues {
    /// Optional rig/equipment description.
    pub equipment_description: Option<String>,
    /// Optional antenna description, independently optional from equipment.
    pub antenna_description: Option<String>,
    /// Optional equipment notes.
    pub notes: Option<String>,
}

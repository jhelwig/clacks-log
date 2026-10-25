//! Reusable station/equipment values and copied client-local operating facts.

use crate::contact::{OperatorInformation, StationSnapshot};
use crate::equipment::EquipmentValues;
use crate::ids::{EquipmentConfigurationId, ProfileId};
use crate::scalar::Milliwatts;
use serde::{Deserialize, Serialize};

/// An account-shared reusable own-station snapshot with an optional equipment default.
///
/// This is an owned profile value, not an active link to operating defaults or
/// contact history. Its station and equipment reference are owned reusable values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedLocationProfile {
    /// Stable profile identity, independent of its display name.
    pub id: ProfileId,
    /// User-facing profile name.
    pub name: String,
    /// Complete saved own-station snapshot.
    pub station: StationSnapshot,
    /// Optional identity of the profile’s preferred equipment configuration.
    pub default_equipment: Option<EquipmentConfigurationId>,
}

/// An account-shared named equipment setup.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentConfiguration {
    /// Stable reusable equipment identity.
    pub id: EquipmentConfigurationId,
    /// User-facing configuration name.
    pub name: String,
    /// Equipment facts belonging to this reusable setup.
    pub values: EquipmentValues,
}

/// Client-local station, operator, power and equipment defaults for contact entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentOperatingValues {
    /// Current complete own-station defaults.
    pub station: StationSnapshot,
    /// Current own operator information.
    pub operator: Option<OperatorInformation>,
    /// Current positive transmit power.
    pub transmit_power: Option<Milliwatts>,
    /// Current owned equipment values, with no live configuration reference.
    pub equipment: Option<EquipmentValues>,
}

impl CurrentOperatingValues {
    /// Check complete station, operator and copied equipment defaults before saving them locally.
    pub fn validate(&self) -> anyhow::Result<()> {
        self.station.validate()?;
        if let Some(operator) = &self.operator {
            operator.validate()?;
        }
        if let Some(equipment) = &self.equipment {
            equipment.validate()?;
        }
        Ok(())
    }
}

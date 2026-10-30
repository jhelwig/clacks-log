//! Reusable station/equipment values and copied client-local operating facts.

use crate::contact::{OperatorInformation, StationSnapshot};
use crate::equipment::EquipmentValues;
use crate::ids::{EquipmentConfigurationId, ProfileId};
use crate::scalar::Milliwatts;
use serde::{Deserialize, Serialize};

/// Editable station-profile input checked before saved-value construction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedLocationProfileFields {
    /// Stable profile identity, independent of its display name.
    pub id: ProfileId,
    /// User-facing profile name.
    pub name: String,
    /// Complete saved own-station snapshot.
    pub station: crate::contact::StationSnapshot,
    /// Optional identity of the profile’s preferred equipment configuration.
    pub default_equipment: Option<EquipmentConfigurationId>,
}

/// An account-shared reusable own-station snapshot with an optional equipment default.
///
/// This is an owned profile value, not an active link to operating defaults or
/// contact history. Its station and equipment reference are owned reusable values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SavedLocationProfileFields")]
pub struct SavedLocationProfile {
    /// Stable profile identity, independent of its display name.
    id: ProfileId,
    /// User-facing profile name.
    name: String,
    /// Complete saved own-station snapshot.
    station: StationSnapshot,
    /// Optional identity of the profile’s preferred equipment configuration.
    default_equipment: Option<EquipmentConfigurationId>,
}

/// Editable equipment-configuration input checked before saved-value construction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentConfigurationFields {
    /// Stable reusable equipment identity.
    pub id: EquipmentConfigurationId,
    /// User-facing configuration name.
    pub name: String,
    /// Equipment facts belonging to this reusable setup.
    pub values: crate::equipment::EquipmentValuesFields,
}

/// An account-shared named equipment setup.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "EquipmentConfigurationFields")]
pub struct EquipmentConfiguration {
    /// Stable reusable equipment identity.
    id: EquipmentConfigurationId,
    /// User-facing configuration name.
    name: String,
    /// Equipment facts belonging to this reusable setup.
    values: EquipmentValues,
}

impl SavedLocationProfile {
    /// Recover editable input for a subsequently checked replacement.
    pub fn into_fields(self) -> SavedLocationProfileFields {
        self.into()
    }

    /// Stable profile identity, independent of its display name.
    pub fn id(&self) -> ProfileId {
        self.id
    }

    /// User-facing profile name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Complete saved own-station snapshot.
    pub fn station(&self) -> &StationSnapshot {
        &self.station
    }

    /// Optional identity of the profile’s preferred equipment configuration.
    pub fn default_equipment(&self) -> Option<EquipmentConfigurationId> {
        self.default_equipment
    }
}

impl TryFrom<SavedLocationProfileFields> for SavedLocationProfile {
    type Error = anyhow::Error;

    fn try_from(fields: SavedLocationProfileFields) -> anyhow::Result<Self> {
        let value = Self {
            id: fields.id,
            name: fields.name,
            station: fields.station,
            default_equipment: fields.default_equipment,
        };
        if value.name.trim().is_empty() {
            return Err(crate::error::ValueError::EmptyText {
                field: "profile name".into(),
            }
            .into());
        }
        Ok(value)
    }
}

impl From<SavedLocationProfile> for SavedLocationProfileFields {
    fn from(value: SavedLocationProfile) -> Self {
        Self {
            id: value.id,
            name: value.name,
            station: value.station,
            default_equipment: value.default_equipment,
        }
    }
}

impl EquipmentConfiguration {
    /// Recover editable input for a subsequently checked replacement.
    pub fn into_fields(self) -> EquipmentConfigurationFields {
        self.into()
    }

    /// Stable reusable equipment identity.
    pub fn id(&self) -> EquipmentConfigurationId {
        self.id
    }

    /// User-facing configuration name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Equipment facts belonging to this reusable setup.
    pub fn values(&self) -> &EquipmentValues {
        &self.values
    }
}

impl TryFrom<EquipmentConfigurationFields> for EquipmentConfiguration {
    type Error = anyhow::Error;

    fn try_from(fields: EquipmentConfigurationFields) -> anyhow::Result<Self> {
        let value = Self {
            id: fields.id,
            name: fields.name,
            values: fields.values.try_into()?,
        };
        if value.name.trim().is_empty() {
            return Err(crate::error::ValueError::EmptyText {
                field: "equipment configuration name".into(),
            }
            .into());
        }
        Ok(value)
    }
}

impl From<EquipmentConfiguration> for EquipmentConfigurationFields {
    fn from(value: EquipmentConfiguration) -> Self {
        Self {
            id: value.id,
            name: value.name,
            values: value.values.into_fields(),
        }
    }
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

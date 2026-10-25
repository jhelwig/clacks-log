//! Reusable station/equipment values and explicit client-local profile loading.

use crate::contact::{OperatorInformation, RecordError, StationSnapshot};
use crate::equipment::EquipmentValues;
use crate::ids::{EquipmentConfigurationId, ProfileId};
use crate::scalar::Milliwatts;
use serde::{Deserialize, Serialize};

/// An account-shared reusable own-station snapshot with an optional equipment default.
///
/// This is an owned profile value, not an active link to operating defaults or
/// contact history. For explicit loading, see [`crate::profile::load_profile`].
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

/// Copy a saved profile into current defaults while preserving operator and transmit power.
///
/// Replace the station snapshot as a whole, clearing absent values. A profile
/// with no equipment default preserves current equipment; otherwise supply the
/// matching configuration. Invalid station/equipment values or a missing/mismatched
/// configuration return an error. The returned defaults own their station and
/// equipment values; input values are not mutated.
#[tracing::instrument(
    skip_all,
    name = "model.profile.load",
    level = "debug",
    fields(
        profile_id = %profile.id,
    ),
)]
pub fn load_profile(
    current: &CurrentOperatingValues,
    profile: &SavedLocationProfile,
    equipment: Option<&EquipmentConfiguration>,
) -> anyhow::Result<CurrentOperatingValues> {
    profile.station.validate()?;
    let mut loaded = current.clone();
    loaded.station = profile.station.clone();
    if let Some(expected) = profile.default_equipment {
        let equipment = equipment.ok_or(RecordError::MissingEquipment { id: expected })?;
        if expected != equipment.id {
            return Err(RecordError::EquipmentIdMismatch {
                expected,
                supplied: equipment.id,
            }
            .into());
        }
        equipment.values.validate()?;
        loaded.equipment = Some(equipment.values.clone());
        tracing::debug!(
            event = "profile.equipment_selected",
            equipment_id = %equipment.id,
        );
    }
    Ok(loaded)
}

/// Copy a configuration’s equipment values into defaults while preserving other defaults.
///
/// Use for explicit equipment selection. This operation does not validate the
/// configuration; validate at the saved-configuration or contact acceptance boundary.
pub fn load_equipment(
    current: &CurrentOperatingValues,
    equipment: &EquipmentConfiguration,
) -> CurrentOperatingValues {
    let mut loaded = current.clone();
    loaded.equipment = Some(equipment.values.clone());
    loaded
}

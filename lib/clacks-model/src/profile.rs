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

/// Copied profile and local values used to detect changes since an explicit load.
///
/// Capture it after loading; it is not a live reference to the saved profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadedProfileBaseline {
    /// Saved profile as it existed at load time.
    pub profile: SavedLocationProfile,
    /// Local defaults immediately after that load.
    pub operating_values: CurrentOperatingValues,
}

/// Independent indications that local station/equipment or the saved profile has changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProfileDifference {
    /// Local station or equipment differs from the loaded defaults.
    pub local_station_or_equipment_changed: bool,
    /// The supplied saved profile differs from the loaded profile copy.
    pub saved_profile_changed: bool,
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

/// Compare local and saved values with the baseline captured at profile load.
///
/// Use to indicate modified defaults or an updated saved profile without
/// automatically reloading either. Operator/power-only edits do not mark the
/// local station/equipment comparison as changed; supply the same profile identity.
pub fn compare_loaded_profile(
    current: &CurrentOperatingValues,
    latest: &SavedLocationProfile,
    baseline: &LoadedProfileBaseline,
) -> ProfileDifference {
    ProfileDifference {
        local_station_or_equipment_changed: current.station != baseline.operating_values.station
            || current.equipment != baseline.operating_values.equipment,
        saved_profile_changed: latest != &baseline.profile,
    }
}

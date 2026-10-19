use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedLocationProfile {
    pub id: ProfileId,
    pub name: String,
    pub station: StationSnapshot,
    pub default_equipment: Option<EquipmentConfigurationId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentConfiguration {
    pub id: EquipmentConfigurationId,
    pub name: String,
    pub values: EquipmentValues,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentOperatingValues {
    pub station: StationSnapshot,
    pub operator: Option<OperatorInformation>,
    pub transmit_power: Option<Milliwatts>,
    pub equipment: Option<EquipmentValues>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadedProfileBaseline {
    pub profile: SavedLocationProfile,
    pub operating_values: CurrentOperatingValues,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProfileDifference {
    pub local_station_or_equipment_changed: bool,
    pub saved_profile_changed: bool,
}

pub fn load_profile(
    current: &CurrentOperatingValues,
    profile: &SavedLocationProfile,
    equipment: Option<&EquipmentConfiguration>,
) -> Result<CurrentOperatingValues, RecordError> {
    profile.station.validate()?;
    let mut loaded = current.clone();
    loaded.station = profile.station.clone();
    if let Some(expected) = profile.default_equipment {
        let equipment = equipment.ok_or(RecordError::MissingEquipment { id: expected })?;
        if expected != equipment.id {
            return Err(RecordError::EquipmentIdMismatch {
                expected,
                supplied: equipment.id,
            });
        }
        equipment.values.validate()?;
        loaded.equipment = Some(equipment.values.clone());
    }
    Ok(loaded)
}

pub fn load_equipment(
    current: &CurrentOperatingValues,
    equipment: &EquipmentConfiguration,
) -> CurrentOperatingValues {
    let mut loaded = current.clone();
    loaded.equipment = Some(equipment.values.clone());
    loaded
}

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

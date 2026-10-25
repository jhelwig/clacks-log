use super::id;
use clacks_client_store::operating::OperatingState;
use clacks_model::{
    contact::StationSnapshot,
    equipment::EquipmentValues,
    profile::{CurrentOperatingValues, EquipmentConfiguration, SavedLocationProfile},
    scalar::{Milliwatts, RecordedCallsign},
};

pub fn equipment(value: u128, name: &str) -> EquipmentConfiguration {
    EquipmentConfiguration {
        id: id(value),
        name: name.into(),
        values: EquipmentValues {
            equipment_description: Some("KX3".into()),
            antenna_description: Some("Vertical".into()),
            notes: None,
        },
    }
}

pub fn profile(value: u128, default: Option<u128>) -> SavedLocationProfile {
    SavedLocationProfile {
        id: id(value),
        name: "Home".into(),
        station: StationSnapshot {
            callsign: RecordedCallsign::new("W7HEX").unwrap(),
            location: None,
            references: Default::default(),
        },
        default_equipment: default.map(id),
    }
}

pub fn operating(
    profile: &SavedLocationProfile,
    equipment: &EquipmentConfiguration,
) -> OperatingState {
    OperatingState {
        current: Some(CurrentOperatingValues {
            station: profile.station.clone(),
            operator: None,
            transmit_power: Some(Milliwatts::new(5000).unwrap()),
            equipment: Some(equipment.values.clone()),
        }),
        selected_profile: Some(profile.clone()),
        selected_equipment: Some(equipment.clone()),
    }
}

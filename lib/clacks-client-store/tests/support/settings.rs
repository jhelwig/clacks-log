use crate::support::id;
use clacks_client_store::operating::OperatingState;
use clacks_model::{
    profile::{EquipmentConfiguration, SavedLocationProfile},
    scalar::{Milliwatts, RecordedCallsign},
};

pub fn equipment(value: u128, name: &str) -> EquipmentConfiguration {
    (clacks_model::profile::EquipmentConfigurationFields {
        id: id(value),
        name: name.into(),
        values: clacks_model::equipment::EquipmentValuesFields {
            equipment_description: Some("KX3".into()),
            antenna_description: Some("Vertical".into()),
            notes: None,
        },
    })
    .try_into()
    .expect("valid native fixture")
}

pub fn profile(value: u128, default: Option<u128>) -> SavedLocationProfile {
    (clacks_model::profile::SavedLocationProfileFields {
        id: id(value),
        name: "Home".into(),
        station: clacks_model::contact::StationSnapshot {
            callsign: RecordedCallsign::new("W7HEX").unwrap(),
            location: None,
            references: Default::default(),
        },
        default_equipment: default.map(id),
    })
    .try_into()
    .expect("valid native fixture")
}

pub fn operating(
    profile: &SavedLocationProfile,
    equipment: &EquipmentConfiguration,
) -> OperatingState {
    OperatingState {
        current: Some(clacks_model::profile::CurrentOperatingValues {
            station: (profile.station().clone()),
            operator: None,
            transmit_power: Some(Milliwatts::new(5000).unwrap()),
            equipment: Some(
                ((equipment.values().clone()).into_fields())
                    .try_into()
                    .expect("valid native fixture"),
            ),
        }),
        selected_profile: Some(profile.clone()),
        selected_equipment: Some(equipment.clone()),
    }
}

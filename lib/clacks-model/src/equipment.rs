use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentValues {
    pub equipment_description: Option<String>,
    pub antenna_description: Option<String>,
    pub notes: Option<String>,
}

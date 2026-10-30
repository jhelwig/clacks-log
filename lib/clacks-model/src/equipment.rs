//! Equipment facts copied into operating defaults and historical contacts.

use serde::{Deserialize, Serialize};

/// Editable input checked before becoming a saved EquipmentValues.
#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentValuesFields {
    /// Optional rig/equipment description.
    pub equipment_description: Option<String>,
    /// Optional antenna description, independently optional from equipment.
    pub antenna_description: Option<String>,
    /// Optional equipment notes.
    pub notes: Option<String>,
}

/// Equipment descriptions copied independently of reusable configuration identities.
///
/// These are owned descriptions, with no configuration identity, display name
/// or live reference to a reusable setup.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "EquipmentValuesFields")]
pub struct EquipmentValues {
    /// Optional rig/equipment description.
    equipment_description: Option<String>,
    /// Optional antenna description, independently optional from equipment.
    antenna_description: Option<String>,
    /// Optional equipment notes.
    notes: Option<String>,
}

impl EquipmentValues {
    /// Recover editable input for a subsequently checked replacement.
    pub fn into_fields(self) -> EquipmentValuesFields {
        self.into()
    }

    /// Optional rig/equipment description.
    pub fn equipment_description(&self) -> &Option<String> {
        &self.equipment_description
    }

    /// Optional antenna description, independently optional from equipment.
    pub fn antenna_description(&self) -> &Option<String> {
        &self.antenna_description
    }

    /// Optional equipment notes.
    pub fn notes(&self) -> &Option<String> {
        &self.notes
    }
}

impl TryFrom<EquipmentValuesFields> for EquipmentValues {
    type Error = anyhow::Error;

    fn try_from(fields: EquipmentValuesFields) -> anyhow::Result<Self> {
        let value = Self {
            equipment_description: fields.equipment_description,
            antenna_description: fields.antenna_description,
            notes: fields.notes,
        };
        value.validate()?;
        Ok(value)
    }
}

impl From<EquipmentValues> for EquipmentValuesFields {
    fn from(value: EquipmentValues) -> Self {
        Self {
            equipment_description: value.equipment_description,
            antenna_description: value.antenna_description,
            notes: value.notes,
        }
    }
}

impl EquipmentValues {
    /// Validate copied optional equipment text before accepting it into a saved record.
    ///
    /// Blank present strings fail validation. No particular equipment field is
    /// required. For preparing optional text, see [`crate::scalar::optional_text`].
    fn validate(&self) -> anyhow::Result<()> {
        for (name, value) in [
            ("equipment", &self.equipment_description),
            ("antenna", &self.antenna_description),
            ("equipment notes", &self.notes),
        ] {
            crate::contact::validate_optional_text(value.as_deref(), name)?;
        }
        Ok(())
    }
}

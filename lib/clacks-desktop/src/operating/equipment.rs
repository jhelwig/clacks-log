//! Editable equipment descriptions independent of configuration identity.

use super::{FieldError, issue, optional};
use clacks_model::equipment::EquipmentValues;

/// Copied equipment text, including local unsaved adjustments.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct EquipmentForm {
    /// Rig and other equipment description.
    pub description: String,
    /// Antenna description.
    pub antenna: String,
    /// Additional copied equipment notes.
    pub notes: String,
}

impl EquipmentForm {
    /// Populate copied text without retaining a saved configuration association.
    pub fn from_values(values: Option<&EquipmentValues>) -> Self {
        match values {
            Some(values) => Self {
                description: values.equipment_description.clone().unwrap_or_default(),
                antenna: values.antenna_description.clone().unwrap_or_default(),
                notes: values.notes.clone().unwrap_or_default(),
            },
            None => Self::default(),
        }
    }

    /// Normalize optional text and validate it before saving or applying equipment facts.
    pub fn build(&self) -> Result<Option<EquipmentValues>, Vec<FieldError>> {
        let value = EquipmentValues {
            equipment_description: optional(&self.description),
            antenna_description: optional(&self.antenna),
            notes: optional(&self.notes),
        };
        value.validate().map_err(|e| vec![issue("Equipment", e)])?;
        if value == EquipmentValues::default() {
            Ok(None)
        } else {
            Ok(Some(value))
        }
    }
}

//! Concrete saved-source fields, independent of active operating copies.

use super::{EquipmentField, FieldId, ProfileField, StationField, TagField, station};
use crate::{operating::optional, review::input::FieldInput};
use clacks_model::{
    group::DefinitionSet,
    profile::{EquipmentConfigurationFields, SavedLocationProfileFields},
    reference::StationSide,
    tag::LogbookTag,
};

/// Editable native aggregates owned by one transient resolution draft.
#[derive(Clone, Debug)]
pub(crate) enum NativeFields {
    /// Complete contact facts.
    Qso(Box<clacks_model::contact::input::QsoFields>),
    /// Saved profile facts.
    Profile(SavedLocationProfileFields),
    /// Reusable equipment facts.
    Equipment(EquipmentConfigurationFields),
    /// Named-tag facts.
    Tag(LogbookTag),
}

impl NativeFields {
    /// Copy editable native facts from an actual live supported record.
    pub fn from_state(state: &clacks_sync::record::RecordState) -> anyhow::Result<Self> {
        use clacks_sync::record::{RecordState, RecordValue};
        Ok(match state {
            RecordState::Live(RecordValue::Qso(value)) => {
                Self::Qso(Box::new(value.clone().into_fields()))
            }
            RecordState::Live(RecordValue::Profile(value)) => {
                Self::Profile(value.clone().into_fields())
            }
            RecordState::Live(RecordValue::Equipment(value)) => {
                Self::Equipment(value.clone().into_fields())
            }
            RecordState::Live(RecordValue::Tag(value)) => Self::Tag(value.as_ref().clone()),
            _ => anyhow::bail!("This source has no supported live editable values"),
        })
    }

    /// List only fields belonging to this native record kind.
    pub fn fields(&self) -> Vec<FieldId> {
        match self {
            Self::Qso(_) => FieldId::qso_fields(),
            Self::Profile(_) => {
                let mut fields = vec![FieldId::Profile(ProfileField::Name)];
                fields.extend(
                    [
                        StationField::Callsign,
                        StationField::Position,
                        StationField::Dxcc,
                        StationField::State,
                        StationField::County,
                        StationField::Cq,
                        StationField::Itu,
                        StationField::References,
                    ]
                    .into_iter()
                    .map(|field| FieldId::Profile(ProfileField::Station(field))),
                );
                fields.push(FieldId::Profile(ProfileField::DefaultEquipment));
                fields
            }
            Self::Equipment(_) => [
                EquipmentField::Name,
                EquipmentField::Description,
                EquipmentField::Antenna,
                EquipmentField::Notes,
            ]
            .into_iter()
            .map(FieldId::Equipment)
            .collect(),
            Self::Tag(_) => [TagField::Name, TagField::Lotw, TagField::Qrz]
                .into_iter()
                .map(FieldId::Tag)
                .collect(),
        }
    }

    /// Copy one source member without parsing a presentation string.
    pub fn copy(&mut self, source: &Self, field: FieldId) -> anyhow::Result<()> {
        match (self, source, field) {
            (Self::Qso(to), Self::Qso(from), FieldId::Qso(field)) => {
                super::qso::copy(to, from, field)?
            }
            (Self::Profile(to), Self::Profile(from), FieldId::Profile(field)) => match field {
                ProfileField::Name => to.name = from.name.clone(),
                ProfileField::DefaultEquipment => to.default_equipment = from.default_equipment,
                ProfileField::Station(field) => {
                    station::copy(&mut to.station, &from.station, field)?
                }
            },
            (Self::Equipment(to), Self::Equipment(from), FieldId::Equipment(field)) => {
                match field {
                    EquipmentField::Name => to.name = from.name.clone(),
                    EquipmentField::Description => {
                        to.values.equipment_description = from.values.equipment_description.clone()
                    }
                    EquipmentField::Antenna => {
                        to.values.antenna_description = from.values.antenna_description.clone()
                    }
                    EquipmentField::Notes => to.values.notes = from.values.notes.clone(),
                }
            }
            (Self::Tag(to), Self::Tag(from), FieldId::Tag(field)) => match field {
                TagField::Name => to.name = from.name.clone(),
                TagField::Lotw => to.automatic_upload.lotw = from.automatic_upload.lotw,
                TagField::Qrz => to.automatic_upload.qrz = from.automatic_upload.qrz,
            },
            _ => anyhow::bail!("Field and source record do not match"),
        }
        Ok(())
    }

    /// Compare native members, including unknown labels and reference identities.
    pub fn same(&self, other: &Self, field: FieldId) -> bool {
        match (self, other, field) {
            (Self::Qso(a), Self::Qso(b), FieldId::Qso(field)) => super::qso::same(a, b, field),
            (Self::Profile(a), Self::Profile(b), FieldId::Profile(field)) => match field {
                ProfileField::Name => a.name == b.name,
                ProfileField::DefaultEquipment => a.default_equipment == b.default_equipment,
                ProfileField::Station(field) => station::same(&a.station, &b.station, field),
            },
            (Self::Equipment(a), Self::Equipment(b), FieldId::Equipment(field)) => match field {
                EquipmentField::Name => a.name == b.name,
                EquipmentField::Description => {
                    a.values.equipment_description == b.values.equipment_description
                }
                EquipmentField::Antenna => {
                    a.values.antenna_description == b.values.antenna_description
                }
                EquipmentField::Notes => a.values.notes == b.values.notes,
            },
            (Self::Tag(a), Self::Tag(b), FieldId::Tag(field)) => match field {
                TagField::Name => a.name == b.name,
                TagField::Lotw => a.automatic_upload.lotw == b.automatic_upload.lotw,
                TagField::Qrz => a.automatic_upload.qrz == b.automatic_upload.qrz,
            },
            _ => false,
        }
    }

    /// Supply the existing control representation for an explicit edit.
    pub fn input(&self, field: FieldId) -> anyhow::Result<FieldInput> {
        let text = |value: &str| FieldInput::Text {
            value: value.into(),
        };
        Ok(match (self, field) {
            (Self::Qso(value), FieldId::Qso(field)) => super::qso::input(value, field),
            (Self::Profile(value), FieldId::Profile(field)) => match field {
                ProfileField::Name => text(&value.name),
                ProfileField::DefaultEquipment => FieldInput::Equipment {
                    selected: value.default_equipment,
                },
                ProfileField::Station(field) => station::input(&value.station, field),
            },
            (Self::Equipment(value), FieldId::Equipment(field)) => match field {
                EquipmentField::Name => text(&value.name),
                EquipmentField::Description => text(
                    value
                        .values
                        .equipment_description
                        .as_deref()
                        .unwrap_or_default(),
                ),
                EquipmentField::Antenna => text(
                    value
                        .values
                        .antenna_description
                        .as_deref()
                        .unwrap_or_default(),
                ),
                EquipmentField::Notes => text(value.values.notes.as_deref().unwrap_or_default()),
            },
            (Self::Tag(value), FieldId::Tag(field)) => match field {
                TagField::Name => text(&value.name),
                TagField::Lotw => FieldInput::Policy {
                    value: value.automatic_upload.lotw,
                },
                TagField::Qrz => FieldInput::Policy {
                    value: value.automatic_upload.qrz,
                },
            },
            _ => anyhow::bail!("Field does not belong to this record"),
        })
    }

    /// Apply only explicit inputs using the owning domain validation rules.
    pub fn edit(
        &mut self,
        field: FieldId,
        input: &FieldInput,
        definitions: &DefinitionSet,
    ) -> anyhow::Result<()> {
        match (self, field, input) {
            (Self::Qso(value), FieldId::Qso(field), input) => {
                super::qso::edit(value, field, input, definitions)?
            }
            (Self::Profile(value), FieldId::Profile(ProfileField::Station(field)), input) => {
                station::edit(
                    &mut value.station,
                    StationSide::Own,
                    field,
                    input,
                    definitions,
                )?
            }
            (
                Self::Profile(value),
                FieldId::Profile(ProfileField::Name),
                FieldInput::Text { value: text },
            ) => value.name = text.clone(),
            (
                Self::Profile(value),
                FieldId::Profile(ProfileField::DefaultEquipment),
                FieldInput::Equipment { selected },
            ) => value.default_equipment = *selected,
            (
                Self::Equipment(value),
                FieldId::Equipment(field),
                FieldInput::Text { value: text },
            ) => match field {
                EquipmentField::Name => value.name = text.clone(),
                EquipmentField::Description => value.values.equipment_description = optional(text),
                EquipmentField::Antenna => value.values.antenna_description = optional(text),
                EquipmentField::Notes => value.values.notes = optional(text),
            },
            (Self::Tag(value), FieldId::Tag(TagField::Name), FieldInput::Text { value: text }) => {
                value.name = text.clone()
            }
            (
                Self::Tag(value),
                FieldId::Tag(TagField::Lotw),
                FieldInput::Policy { value: policy },
            ) => value.automatic_upload.lotw = *policy,
            (
                Self::Tag(value),
                FieldId::Tag(TagField::Qrz),
                FieldInput::Policy { value: policy },
            ) => value.automatic_upload.qrz = *policy,
            _ => anyhow::bail!("This field needs its matching editor"),
        }
        Ok(())
    }

    /// Validate and construct the one complete native saved result.
    pub fn build(self) -> anyhow::Result<clacks_sync::record::RecordState> {
        use clacks_sync::record::{RecordState, RecordValue};
        Ok(RecordState::Live(match self {
            Self::Qso(value) => RecordValue::Qso(Box::new((*value).try_into()?)),
            Self::Profile(value) => RecordValue::Profile(Box::new(value.try_into()?)),
            Self::Equipment(value) => RecordValue::Equipment(Box::new(value.try_into()?)),
            Self::Tag(value) => {
                anyhow::ensure!(
                    !value.name.trim().is_empty(),
                    "Logbook name cannot be blank"
                );
                RecordValue::Tag(Box::new(value))
            }
        }))
    }
}

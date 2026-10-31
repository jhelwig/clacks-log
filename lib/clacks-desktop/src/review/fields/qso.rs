//! Explicit native contact-field copying, comparison and edited-input conversion.

use super::{QsoField, station};
use crate::{
    logging::{radio::RadioForm, time::ContactTimestamp},
    operating::{optional, power},
    review::input::FieldInput,
};
use clacks_model::{
    contact::{OperatorInformation, OperatorInformationFields, StationSnapshot, input::QsoFields},
    equipment::EquipmentValuesFields,
    group::DefinitionSet,
    reference::StationSide,
    scalar::RecordedCallsign,
};

/// Copy exactly one selected native contact field into an editable native aggregate.
pub(in crate::review) fn copy(
    target: &mut QsoFields,
    source: &QsoFields,
    field: QsoField,
) -> anyhow::Result<()> {
    match field {
        QsoField::Start => target.start = source.start,
        QsoField::End => target.end = source.end,
        QsoField::Primary => target.primary = source.primary.clone(),
        QsoField::Receive => target.receive_override = source.receive_override.clone(),
        QsoField::Mode => target.mode = source.mode.clone(),
        QsoField::Power => target.own_context.transmit_power = source.own_context.transmit_power,
        QsoField::Sent => target.sent_report = source.sent_report.clone(),
        QsoField::Received => target.received_report = source.received_report.clone(),
        QsoField::Comment => target.comment = source.comment.clone(),
        QsoField::Tags => target.tags = source.tags.clone(),
        QsoField::Station { side, field } => {
            station::copy(station_mut(target, side), station_ref(source, side), field)?
        }
        QsoField::OwnOperatorCall
        | QsoField::OwnOperatorName
        | QsoField::ContactOperatorCall
        | QsoField::ContactOperatorName => {
            let mut values = operator_fields(target, field);
            let from = operator_fields(source, field);
            match field {
                QsoField::OwnOperatorCall | QsoField::ContactOperatorCall => {
                    values.callsign = from.callsign
                }
                _ => values.recorded_name = from.recorded_name,
            }
            set_operator(target, field, values)?;
        }
        QsoField::Equipment | QsoField::Antenna | QsoField::EquipmentNotes => {
            let mut values = equipment_fields(target);
            let from = equipment_fields(source);
            match field {
                QsoField::Equipment => values.equipment_description = from.equipment_description,
                QsoField::Antenna => values.antenna_description = from.antenna_description,
                _ => values.notes = from.notes,
            }
            set_equipment(target, values)?;
        }
    }
    Ok(())
}

/// Compare the selected native facts without recognition or display formatting.
pub(in crate::review) fn same(a: &QsoFields, b: &QsoFields, field: QsoField) -> bool {
    match field {
        QsoField::Start => a.start == b.start,
        QsoField::End => a.end == b.end,
        QsoField::Primary => a.primary == b.primary,
        QsoField::Receive => a.receive_override == b.receive_override,
        QsoField::Mode => a.mode == b.mode,
        QsoField::Power => a.own_context.transmit_power == b.own_context.transmit_power,
        QsoField::Sent => a.sent_report == b.sent_report,
        QsoField::Received => a.received_report == b.received_report,
        QsoField::Comment => a.comment == b.comment,
        QsoField::Tags => a.tags == b.tags,
        QsoField::Station { side, field } => {
            station::same(station_ref(a, side), station_ref(b, side), field)
        }
        QsoField::OwnOperatorCall | QsoField::ContactOperatorCall => {
            operator_fields(a, field).callsign == operator_fields(b, field).callsign
        }
        QsoField::OwnOperatorName | QsoField::ContactOperatorName => {
            operator_fields(a, field).recorded_name == operator_fields(b, field).recorded_name
        }
        QsoField::Equipment => {
            equipment_fields(a).equipment_description == equipment_fields(b).equipment_description
        }
        QsoField::Antenna => {
            equipment_fields(a).antenna_description == equipment_fields(b).antenna_description
        }
        QsoField::EquipmentNotes => equipment_fields(a).notes == equipment_fields(b).notes,
    }
}

/// Prepare concrete controls; their text is not used unless the operator edits it.
pub(in crate::review) fn input(value: &QsoFields, field: QsoField) -> FieldInput {
    match field {
        QsoField::Station { side, field } => station::input(station_ref(value, side), field),
        QsoField::Tags => FieldInput::Membership {
            tags: value.tags.clone(),
        },
        QsoField::Primary => FieldInput::Radio {
            form: RadioForm::from_pair(Some(&value.primary)),
        },
        QsoField::Receive => FieldInput::Radio {
            form: RadioForm::from_pair(value.receive_override.as_ref()),
        },
        QsoField::Mode => match value.mode.identity() {
            clacks_model::recognition::mode::ModeIdentity::Parent { label } => FieldInput::Mode {
                label,
                parent: String::new(),
            },
            clacks_model::recognition::mode::ModeIdentity::Specialization { label, parent } => {
                FieldInput::Mode {
                    label,
                    parent: parent.unwrap_or_default(),
                }
            }
        },
        _ => FieldInput::Text {
            value: match field {
                QsoField::Start => crate::logging::time::format(value.start),
                QsoField::End => value
                    .end
                    .map(crate::logging::time::format)
                    .unwrap_or_default(),
                QsoField::Power => power::format_watts(value.own_context.transmit_power),
                QsoField::Sent => value.sent_report.clone().unwrap_or_default(),
                QsoField::Received => value.received_report.clone().unwrap_or_default(),
                QsoField::Comment => value.comment.clone().unwrap_or_default(),
                QsoField::OwnOperatorCall | QsoField::ContactOperatorCall => {
                    operator_fields(value, field)
                        .callsign
                        .map(|call| call.as_str().to_owned())
                        .unwrap_or_default()
                }
                QsoField::OwnOperatorName | QsoField::ContactOperatorName => {
                    operator_fields(value, field)
                        .recorded_name
                        .unwrap_or_default()
                }
                QsoField::Equipment => equipment_fields(value)
                    .equipment_description
                    .unwrap_or_default(),
                QsoField::Antenna => equipment_fields(value)
                    .antenna_description
                    .unwrap_or_default(),
                QsoField::EquipmentNotes => equipment_fields(value).notes.unwrap_or_default(),
                _ => String::new(),
            },
        },
    }
}

/// Parse only explicit contact edits using normal logging conversions.
pub(in crate::review) fn edit(
    target: &mut QsoFields,
    field: QsoField,
    input: &FieldInput,
    definitions: &DefinitionSet,
) -> anyhow::Result<()> {
    match (field, input) {
        (QsoField::Station { side, field }, input) => {
            station::edit(station_mut(target, side), side, field, input, definitions)?
        }
        (QsoField::Tags, FieldInput::Membership { tags }) => target.tags = tags.clone(),
        (QsoField::Primary, FieldInput::Radio { form }) => {
            target.primary = form
                .build()?
                .ok_or_else(|| anyhow::anyhow!("Enter a frequency or band"))?
        }
        (QsoField::Receive, FieldInput::Radio { form }) => {
            target.receive_override = form.build()?
        }
        (QsoField::Mode, FieldInput::Mode { label, parent }) => {
            target.mode = crate::logging::form::parse_mode(label, parent)?
        }
        (_, FieldInput::Text { value }) => match field {
            QsoField::Start | QsoField::End => {
                let mut time = ContactTimestamp::recorded(None);
                time.text = value.clone();
                let next = time.value()?;
                if field == QsoField::Start {
                    target.start =
                        next.ok_or_else(|| anyhow::anyhow!("Enter UTC start date and time"))?;
                } else {
                    target.end = next;
                }
            }
            QsoField::Power => {
                target.own_context.transmit_power =
                    power::parse_watts(value).map_err(anyhow::Error::msg)?
            }
            QsoField::Sent => target.sent_report = optional(value),
            QsoField::Received => target.received_report = optional(value),
            QsoField::Comment => target.comment = optional(value),
            QsoField::OwnOperatorCall
            | QsoField::OwnOperatorName
            | QsoField::ContactOperatorCall
            | QsoField::ContactOperatorName => {
                let mut values = operator_fields(target, field);
                match field {
                    QsoField::OwnOperatorCall | QsoField::ContactOperatorCall => {
                        values.callsign = optional(value)
                            .map(|call| RecordedCallsign::new(&call))
                            .transpose()?
                    }
                    _ => values.recorded_name = optional(value),
                }
                set_operator(target, field, values)?;
            }
            QsoField::Equipment | QsoField::Antenna | QsoField::EquipmentNotes => {
                let mut values = equipment_fields(target);
                match field {
                    QsoField::Equipment => values.equipment_description = optional(value),
                    QsoField::Antenna => values.antenna_description = optional(value),
                    _ => values.notes = optional(value),
                }
                set_equipment(target, values)?;
            }
            _ => anyhow::bail!("This field needs its matching editor"),
        },
        _ => anyhow::bail!("This field needs its matching editor"),
    }
    Ok(())
}

/// Borrow the station snapshot selected by its native side.
fn station_ref(value: &QsoFields, side: StationSide) -> &StationSnapshot {
    match side {
        StationSide::Own => &value.own_station,
        StationSide::Contacted => &value.contacted_station,
    }
}

/// Edit one station side without changing the other snapshot.
fn station_mut(value: &mut QsoFields, side: StationSide) -> &mut StationSnapshot {
    match side {
        StationSide::Own => &mut value.own_station,
        StationSide::Contacted => &mut value.contacted_station,
    }
}

/// Recover the independent recorded operator fields for the requested side.
fn operator_fields(value: &QsoFields, field: QsoField) -> OperatorInformationFields {
    match field {
        QsoField::OwnOperatorCall | QsoField::OwnOperatorName => value.own_context.operator.clone(),
        _ => value.contacted_operator.clone(),
    }
    .map(OperatorInformation::into_fields)
    .unwrap_or_default()
}

/// Normalize an explicitly changed optional operator aggregate before construction.
fn set_operator(
    value: &mut QsoFields,
    field: QsoField,
    next: OperatorInformationFields,
) -> anyhow::Result<()> {
    let next = next.normalized().map(TryInto::try_into).transpose()?;
    match field {
        QsoField::OwnOperatorCall | QsoField::OwnOperatorName => value.own_context.operator = next,
        _ => value.contacted_operator = next,
    }
    Ok(())
}

/// Recover copied equipment fields without introducing a saved configuration link.
fn equipment_fields(value: &QsoFields) -> EquipmentValuesFields {
    value
        .own_context
        .equipment
        .clone()
        .map(|value| value.into_fields())
        .unwrap_or_default()
}

/// Collapse wholly empty copied equipment after an explicit field edit.
fn set_equipment(value: &mut QsoFields, next: EquipmentValuesFields) -> anyhow::Result<()> {
    value.own_context.equipment = if next == EquipmentValuesFields::default() {
        None
    } else {
        Some(next.try_into()?)
    };
    Ok(())
}

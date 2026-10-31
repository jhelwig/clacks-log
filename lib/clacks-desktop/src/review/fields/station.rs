//! Copying and explicitly editing station facts without directory fallback.

use super::StationField;
use crate::{
    operating::{optional, position::PositionForm, station::StationForm},
    review::input::FieldInput,
};
use clacks_model::{
    contact::StationSnapshot,
    group::DefinitionSet,
    location::{RecordedLocation, RecordedLocationFields},
    reference::StationSide,
    scalar::RecordedCallsign,
};

/// Copy just one native station member, preserving all other recorded facts.
pub(super) fn copy(
    target: &mut StationSnapshot,
    source: &StationSnapshot,
    field: StationField,
) -> anyhow::Result<()> {
    let mut next = location(target);
    let from = location(source);
    match field {
        StationField::Callsign => {
            target.callsign = source.callsign.clone();
            return Ok(());
        }
        StationField::References => {
            target.references = source.references.clone();
            return Ok(());
        }
        StationField::Position => next.position = from.position,
        StationField::Dxcc => next.dxcc = from.dxcc,
        StationField::State => next.primary_subdivision = from.primary_subdivision,
        StationField::County => next.secondary_subdivision = from.secondary_subdivision,
        StationField::Cq => next.cq_zone = from.cq_zone,
        StationField::Itu => next.itu_zone = from.itu_zone,
    }
    target.location = next.normalized().map(TryInto::try_into).transpose()?;
    Ok(())
}

/// Compare actual native members instead of their formatted/recognized display.
pub(super) fn same(a: &StationSnapshot, b: &StationSnapshot, field: StationField) -> bool {
    let x = location(a);
    let y = location(b);
    match field {
        StationField::Callsign => a.callsign == b.callsign,
        StationField::References => a.references == b.references,
        StationField::Position => x.position == y.position,
        StationField::Dxcc => x.dxcc == y.dxcc,
        StationField::State => x.primary_subdivision == y.primary_subdivision,
        StationField::County => x.secondary_subdivision == y.secondary_subdivision,
        StationField::Cq => x.cq_zone == y.cq_zone,
        StationField::Itu => x.itu_zone == y.itu_zone,
    }
}

/// Prepare the existing concrete input control for an explicitly editable field.
pub(super) fn input(station: &StationSnapshot, field: StationField) -> FieldInput {
    let value = location(station);
    match field {
        StationField::Position => FieldInput::Position {
            form: PositionForm::from_position(value.position.as_ref()),
        },
        StationField::References => FieldInput::References {
            rows: StationForm::from_snapshot(station).references,
        },
        _ => FieldInput::Text {
            value: match field {
                StationField::Callsign => station.callsign.as_str().to_owned(),
                StationField::Dxcc => value.dxcc.unwrap_or_default(),
                StationField::State => value.primary_subdivision.unwrap_or_default(),
                StationField::County => value.secondary_subdivision.unwrap_or_default(),
                StationField::Cq => value
                    .cq_zone
                    .map(|v| v.get().to_string())
                    .unwrap_or_default(),
                StationField::Itu => value
                    .itu_zone
                    .map(|v| v.get().to_string())
                    .unwrap_or_default(),
                _ => String::new(),
            },
        },
    }
}

/// Parse only the station member the user explicitly edited.
pub(super) fn edit(
    target: &mut StationSnapshot,
    side: StationSide,
    field: StationField,
    input: &FieldInput,
    definitions: &DefinitionSet,
) -> anyhow::Result<()> {
    match (field, input) {
        (StationField::Callsign, FieldInput::Text { value }) => {
            target.callsign = RecordedCallsign::new(value)?
        }
        (StationField::References, FieldInput::References { rows }) => {
            target.references = rows
                .iter()
                .map(|row| {
                    row.build(definitions)
                        .map_err(|error| anyhow::anyhow!(error.message))
                })
                .collect::<anyhow::Result<_>>()?;
        }
        (StationField::Position, FieldInput::Position { form }) => {
            let mut next = location(target);
            next.position = if side == StationSide::Own {
                form.build()
            } else {
                form.build_recorded()
            }
            .map_err(anyhow::Error::msg)?;
            target.location = next.normalized().map(TryInto::try_into).transpose()?;
        }
        (_, FieldInput::Text { value }) => {
            let mut next = location(target);
            match field {
                StationField::Dxcc => next.dxcc = optional(value),
                StationField::State => next.primary_subdivision = optional(value),
                StationField::County => next.secondary_subdivision = optional(value),
                StationField::Cq => {
                    next.cq_zone = optional(value)
                        .map(|v| v.trim().parse::<u8>())
                        .transpose()?
                        .map(clacks_model::location::CqZone::new)
                        .transpose()?
                }
                StationField::Itu => {
                    next.itu_zone = optional(value)
                        .map(|v| v.trim().parse::<u8>())
                        .transpose()?
                        .map(clacks_model::location::ItuZone::new)
                        .transpose()?
                }
                _ => anyhow::bail!("This station field needs its matching editor"),
            }
            target.location = next.normalized().map(TryInto::try_into).transpose()?;
        }
        _ => anyhow::bail!("This station field needs its matching editor"),
    }
    Ok(())
}

/// Recover native optional geographic fields without introducing fallback values.
fn location(station: &StationSnapshot) -> RecordedLocationFields {
    station
        .location
        .clone()
        .map(RecordedLocation::into_fields)
        .unwrap_or_default()
}

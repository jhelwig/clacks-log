//! Shared interpretation of the two providers' explicitly supported station fields.

use clacks_model::{
    directory::values::DirectoryFacts,
    location::{CqZone, ItuZone, RecordedLocation, RecordedLocationFields},
    position::{Coordinates, Locator, Position},
    reference::ReferenceAssociation,
    scalar::RecordedCallsign,
};

/// Textual station values decoded from the concrete provider schema, never persisted as raw data.
#[derive(Default)]
pub(super) struct StationFields {
    /// Selected on-air name.
    pub name: Option<String>,
    /// Returned provider identity, separate from the query.
    pub callsign: Option<String>,
    /// Reported locator at its supplied precision.
    pub grid: Option<String>,
    /// Reported latitude, not a derived grid centre.
    pub latitude: Option<String>,
    /// Reported longitude.
    pub longitude: Option<String>,
    /// Provider's coordinate-origin classification.
    pub coordinate_source: Option<String>,
    /// Station entity, distinct from its postal address.
    pub dxcc: Option<String>,
    /// Primary subdivision.
    pub state: Option<String>,
    /// County before state qualification.
    pub county: Option<String>,
    /// Optional CQ zone.
    pub cq: Option<String>,
    /// Optional ITU zone.
    pub itu: Option<String>,
    /// Island association, separate from QSO facts.
    pub iota: Option<String>,
    /// Provider locality, distinct from geographic identifiers.
    pub locality: Option<String>,
}

/// Treat omitted and whitespace-only optional text as absent, preserving supplied nonblank text.
pub(super) fn text(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
}

/// Check optional fields independently and preserve the provider's position-source meaning.
pub(super) fn project(fields: StationFields, prefer_grid: bool) -> anyhow::Result<DirectoryFacts> {
    let grid = fields
        .grid
        .as_deref()
        .and_then(|value| Locator::parse(value.trim()).ok());
    let coordinates = fields
        .latitude
        .as_deref()
        .zip(fields.longitude.as_deref())
        .and_then(|(latitude, longitude)| {
            Coordinates::from_decimal_degrees(latitude.trim(), longitude.trim()).ok()
        });
    let point = coordinates.map(|coordinates| Position::ExplicitPoint {
        coordinates,
        grid_length: grid.as_ref().map(Locator::length).unwrap_or_default(),
    });
    let grid_position = grid.clone().map(Position::GridArea);
    let position = if prefer_grid {
        grid_position.or(point)
    } else {
        point.or(grid_position)
    };
    let dxcc = fields
        .dxcc
        .as_deref()
        .and_then(|value| value.trim().parse::<u16>().ok())
        .filter(|value| *value != 0)
        .map(|value| value.to_string());
    let secondary = fields
        .state
        .as_ref()
        .zip(fields.county.as_ref())
        .map(|(state, county)| {
            if county.starts_with(&format!("{state},")) {
                county.clone()
            } else {
                format!("{state},{county}")
            }
        });
    let location_fields = RecordedLocationFields {
        position,
        dxcc,
        primary_subdivision: fields.state,
        secondary_subdivision: secondary,
        cq_zone: fields
            .cq
            .as_deref()
            .and_then(|value| value.trim().parse().ok())
            .and_then(|value| CqZone::new(value).ok()),
        itu_zone: fields
            .itu
            .as_deref()
            .and_then(|value| value.trim().parse().ok())
            .and_then(|value| ItuZone::new(value).ok()),
    };
    let has_location = location_fields.position.is_some()
        || location_fields.dxcc.is_some()
        || location_fields.primary_subdivision.is_some()
        || location_fields.secondary_subdivision.is_some()
        || location_fields.cq_zone.is_some()
        || location_fields.itu_zone.is_some();
    let location = if has_location {
        Some(RecordedLocation::try_from(location_fields)?)
    } else {
        None
    };
    let mut references = std::collections::BTreeSet::new();
    if let Some(iota) = fields.iota {
        references.insert(ReferenceAssociation::new(
            "01M5QZFPZZ9GPDY819J1WMTJAK".parse()?,
            Some(iota),
        )?);
    }
    Ok(DirectoryFacts {
        name: fields.name,
        location,
        references,
        locality: fields.locality,
        returned_callsign: fields
            .callsign
            .as_deref()
            .and_then(|value| RecordedCallsign::new(value).ok()),
        supplied_locator: grid,
        supplied_coordinates: coordinates,
        coordinate_source: fields.coordinate_source,
    })
}

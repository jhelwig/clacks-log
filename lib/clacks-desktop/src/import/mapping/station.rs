//! Recorded geography and activity references for the two stations in an ADI row.

use super::{
    Edit, IOTA_GROUP_ID, POTA_GROUP_ID, PositionClear, RowInput, SOTA_GROUP_ID, SubmissionPark,
};
use crate::import::adi::Alternative;
use clacks_model::{
    group::{DefinitionSet, normalize_reference},
    ids::GroupId,
    location::{CqZone, ItuZone},
    position::{Coordinates, GridLength, Locator, Position},
    reference::{ReferenceAssociation, StationSide},
};
use std::collections::{BTreeMap, BTreeSet};

/// Add recorded location and reference edits to the row for `side`.
/// The own-station pass also records group-only contest participation.
pub(super) fn prepare(row: &mut RowInput<'_, '_>, side: StationSide, definitions: &DefinitionSet) {
    let name = |own, contacted| match side {
        StationSide::Own => own,
        StationSide::Contacted => contacted,
    };
    let dxcc = name("MY_DXCC", "DXCC");
    row.map(dxcc, |value| {
        Ok(Edit::Dxcc(
            side,
            optional_id(value, dxcc)?.map(|id| id.to_string()),
        ))
    });
    row.map(name("MY_STATE", "STATE"), |value| {
        Ok(Edit::State(side, trimmed(value)))
    });
    row.map(name("MY_CNTY", "CNTY"), |value| {
        Ok(Edit::County(side, trimmed(value)))
    });
    row.map(name("MY_CQ_ZONE", "CQZ"), |value| {
        Ok(Edit::Cq(side, zone(value, CqZone::new)?))
    });
    row.map(name("MY_ITU_ZONE", "ITUZ"), |value| {
        Ok(Edit::Itu(side, zone(value, ItuZone::new)?))
    });
    position(
        row,
        side,
        name("MY_GRIDSQUARE", "GRIDSQUARE"),
        name("MY_GRIDSQUARE_EXT", "GRIDSQUARE_EXT"),
        name("MY_LAT", "LAT"),
        name("MY_LON", "LON"),
    );

    references(
        row,
        side,
        name("MY_SOTA_REF", "SOTA_REF"),
        SOTA_GROUP_ID,
        definitions,
    );
    references(
        row,
        side,
        name("MY_IOTA", "IOTA"),
        IOTA_GROUP_ID,
        definitions,
    );
    parks(
        row,
        side,
        name("MY_POTA_REF", "POTA_REF"),
        name("MY_SIG", "SIG"),
        name("MY_SIG_INFO", "SIG_INFO"),
        definitions,
    );
    if side == StationSide::Own {
        contest(row, definitions);
    }
}

fn trimmed(value: &str) -> Option<String> {
    clacks_model::scalar::optional_text(value.trim())
}

fn optional_id(value: &str, name: &str) -> anyhow::Result<Option<u32>> {
    trimmed(value)
        .map(|value| {
            anyhow::ensure!(
                value.bytes().all(|byte| byte.is_ascii_digit()),
                "{name} requires a numeric identifier"
            );
            Ok(value.parse()?)
        })
        .transpose()
}

fn zone<T>(
    value: &str,
    read: impl Fn(u8) -> Result<T, clacks_model::error::ValueError>,
) -> anyhow::Result<Option<T>> {
    trimmed(value)
        .map(|value| Ok(read(value.parse()?)?))
        .transpose()
}

/// Interpret a coordinate in ADIF's hemisphere, degrees and decimal minutes syntax.
fn angle(value: &str, north_east: char, south_west: char, max: u32) -> anyhow::Result<i32> {
    let value = value.trim().to_ascii_uppercase();
    let polarity = if value.starts_with(north_east) {
        1
    } else if value.starts_with(south_west) {
        -1
    } else {
        anyhow::bail!("Invalid coordinate hemisphere");
    };
    let (degrees, minutes) = value
        .get(1..)
        .and_then(|rest| rest.split_once(' '))
        .ok_or_else(|| anyhow::anyhow!("Expected degrees and decimal minutes"))?;
    let (whole, fraction) = minutes.split_once('.').unwrap_or((minutes, ""));
    anyhow::ensure!(
        !degrees.is_empty()
            && !whole.is_empty()
            && fraction.len() <= 9
            && degrees
                .bytes()
                .chain(whole.bytes())
                .chain(fraction.bytes())
                .all(|byte| byte.is_ascii_digit()),
        "Invalid degree or minute components"
    );
    let degrees = degrees.parse::<u32>()?;
    let whole = whole.parse::<u32>()?;
    let scale = 10u64.pow(fraction.len() as u32);
    let fraction = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<u64>()?
    };
    let minutes = u64::from(whole) * scale + fraction;
    anyhow::ensure!(
        minutes < 60 * scale && (degrees < max || degrees == max && minutes == 0),
        "Coordinate is outside its geographic range"
    );
    let divisor = 60 * scale;
    let microdegrees =
        u64::from(degrees) * 1_000_000 + ((minutes * 1_000_000 + divisor / 2) / divisor);
    Ok(i32::try_from(microdegrees)? * polarity)
}

/// A complete coordinate pair takes precedence over a disagreeing locator
/// while retaining its precision. A locator alone supplies a grid area.
fn position(
    row: &mut RowInput<'_, '_>,
    side: StationSide,
    grid_field: &str,
    ext_field: &str,
    lat_field: &str,
    lon_field: &str,
) {
    let grids = row.parsed_optional(grid_field, |value| {
        anyhow::ensure!(
            value.len() <= 8,
            "Use the separate locator extension for characters after the eighth"
        );
        Ok(Locator::parse(value)?)
    });
    let extensions = row.values(ext_field);
    let latitude = row.parsed_optional(lat_field, |value| angle(value, 'N', 'S', 90));
    let longitude = row.parsed_optional(lon_field, |value| angle(value, 'E', 'W', 180));
    let mut located = Vec::new();
    for (grid_input, grid) in &grids {
        let Some(grid) = grid else { continue };
        let mut combined = false;
        for extension in extensions
            .iter()
            .filter(|value| !value.value.trim().is_empty())
        {
            if grid.length().get() != 8 {
                row.warn(
                    extension,
                    "A locator extension requires an eight-character base locator",
                );
                continue;
            }
            let suffix = extension.value.trim();
            if !matches!(suffix.len(), 2 | 4) {
                row.warn(
                    extension,
                    "A locator extension needs two or four characters",
                );
                continue;
            }
            match Locator::parse(&format!("{grid}{suffix}")) {
                Ok(value) => {
                    located.push((grid_input.join(extension), value));
                    combined = true;
                }
                Err(error) => row.warn(extension, error),
            }
        }
        if !combined {
            located.push((grid_input.clone(), grid.clone()));
        }
    }
    if located.is_empty() {
        for extension in extensions
            .iter()
            .filter(|value| !value.value.trim().is_empty())
        {
            row.warn(
                extension,
                "A locator extension requires a usable base locator",
            );
        }
    }

    let mut values = Vec::new();
    for (lat_source, lat) in &latitude {
        for (lon_source, lon) in &longitude {
            let (Some(lat), Some(lon)) = (lat, lon) else {
                continue;
            };
            let source = lat_source.join(lon_source);
            match Coordinates::from_microdegrees(*lat, *lon) {
                Ok(coordinates) => {
                    for (grid_source, grid) in &located {
                        let derived = coordinates.locator(grid.length());
                        if derived != *grid {
                            row.warn(
                                grid_source,
                                format!("Explicit coordinates supply locator {derived}"),
                            );
                        }
                        values.push((
                            source.join(grid_source),
                            Edit::Position(
                                side,
                                Position::ExplicitPoint {
                                    coordinates,
                                    grid_length: grid.length(),
                                },
                            ),
                        ));
                    }
                    if located.is_empty() {
                        values.push((
                            source,
                            Edit::Position(
                                side,
                                Position::ExplicitPoint {
                                    coordinates,
                                    grid_length: GridLength::default(),
                                },
                            ),
                        ));
                    }
                }
                Err(error) => row.warn(&source, error),
            }
        }
    }
    if values.is_empty() {
        for (source, _) in latitude
            .iter()
            .filter(|(_, value)| value.is_some())
            .chain(longitude.iter().filter(|(_, value)| value.is_some()))
        {
            row.warn(
                source,
                "A recorded point requires both latitude and longitude",
            );
        }
        values.extend(
            located
                .into_iter()
                .map(|(source, grid)| (source, Edit::Position(side, Position::GridArea(grid)))),
        );
    }
    if values.is_empty() {
        let empty_grid = extensions
            .iter()
            .all(|extension| extension.value.trim().is_empty())
            .then(|| {
                grids
                    .iter()
                    .find_map(|(source, value)| value.is_none().then_some(source))
            })
            .flatten();
        let empty_lat = latitude
            .iter()
            .find_map(|(source, value)| value.is_none().then_some(source));
        let empty_lon = longitude
            .iter()
            .find_map(|(source, value)| value.is_none().then_some(source));
        let empty_point = empty_lat.zip(empty_lon).map(|(lat, lon)| lat.join(lon));
        let clear = match (empty_grid, empty_point) {
            (Some(grid), Some(point)) => Some((grid.join(&point), PositionClear::Both)),
            (Some(grid), None) => Some((grid.clone(), PositionClear::Grid)),
            (None, Some(point)) => Some((point, PositionClear::Point)),
            (None, None) => None,
        };
        if let Some((source, clear)) = clear {
            values.push((source, Edit::ClearPosition(side, clear)));
        }
    }
    row.record(grid_field, values);
}

/// Normalize the supplied reference with its account group definition. An
/// unavailable definition prevents import of the reference.
fn association(
    raw: &str,
    group: GroupId,
    definitions: &DefinitionSet,
) -> anyhow::Result<ReferenceAssociation> {
    let definition = definitions
        .get(group)
        .ok_or_else(|| anyhow::anyhow!("This account lacks the corresponding reference group"))?;
    Ok(normalize_reference(
        &ReferenceAssociation::new(group, Some(raw.to_owned()))?,
        Some(definition),
    )?)
}

fn references(
    row: &mut RowInput<'_, '_>,
    side: StationSide,
    name: &str,
    group: &str,
    definitions: &DefinitionSet,
) {
    row.map(name, |value| {
        let group = group.parse()?;
        if value.trim().is_empty() {
            anyhow::ensure!(
                definitions.get(group).is_some(),
                "This account lacks the corresponding reference group"
            );
            return Ok(Edit::ClearReference(side, group));
        }
        Ok(Edit::Reference(
            side,
            association(value, group, definitions)?,
        ))
    });
}

/// Extract the park identity independently of its optional activation-location qualifier.
pub(super) fn park_identity(reference: &ReferenceAssociation) -> Option<&str> {
    if reference.group_id != POTA_GROUP_ID.parse::<GroupId>().ok()? {
        return None;
    }
    let full = reference.identifier().as_deref()?;
    Some(full.split_once('@').map_or(full, |(park, _)| park))
}

/// Combine park lists and single-park submissions by park identity. Conflicting
/// qualifiers for a park require one resolution.
fn parks(
    row: &mut RowInput<'_, '_>,
    side: StationSide,
    list_field: &str,
    sig_field: &str,
    info_field: &str,
    definitions: &DefinitionSet,
) {
    let Ok(group) = POTA_GROUP_ID.parse::<GroupId>() else {
        return;
    };
    let lists = row.values(list_field);
    let sigs = row.values(sig_field);
    let info = row.values(info_field);
    let mut parks = BTreeMap::<String, Vec<(Alternative, ReferenceAssociation)>>::new();

    for input in &lists {
        for park in input
            .value
            .split(',')
            .filter(|park| !park.trim().is_empty())
        {
            insert_park(row, input, park, group, definitions, &mut parks);
        }
    }
    let is_pota = |sig: &Alternative| sig.value.trim().eq_ignore_ascii_case("POTA");
    let pota = sigs.iter().find(|sig| is_pota(sig));
    let other = sigs.iter().any(|sig| !is_pota(sig));
    if other {
        row.disclose(sig_field.to_owned());
    }
    if !info.is_empty() && (pota.is_none() || other) {
        row.disclose(info_field.to_owned());
    }

    let mut submission = BTreeMap::new();
    if let Some(sig) = pota {
        if info.is_empty() {
            row.warn(sig, format!("POTA needs {info_field}"));
        }
        for reference in &info {
            let input = sig.join(reference);
            insert_park(
                row,
                &input,
                &reference.value,
                group,
                definitions,
                &mut submission,
            );
        }
    }
    let (disputed_submission, shared_parks) = if submission.len() > 1 {
        let shared_parks: BTreeSet<_> = submission
            .keys()
            .filter(|park| parks.contains_key(*park))
            .cloned()
            .collect();
        let dispute = row.record_dispute(
            info_field,
            submission
                .into_iter()
                .flat_map(|(park, mut values)| {
                    // Include any competing location qualifier for this same park.
                    if let Some(listed) = parks.get(&park) {
                        values.extend(listed.iter().cloned());
                    }
                    park_edits(side, values)
                })
                .collect(),
            None,
        );
        (dispute, shared_parks)
    } else {
        for (park, values) in submission {
            parks.entry(park).or_default().extend(values);
        }
        (None, BTreeSet::new())
    };

    if parks.is_empty()
        && disputed_submission.is_none()
        && lists.iter().all(|source| source.value.trim().is_empty())
        && let Some(source) = lists.first()
    {
        if definitions.get(group).is_some() {
            row.record(
                list_field,
                vec![(source.clone(), Edit::ClearReference(side, group))],
            );
        } else {
            row.warn(
                source,
                "This account lacks the corresponding reference group",
            );
        }
    }
    for (park, values) in parks {
        let unless_submission_selects = disputed_submission
            .filter(|_| shared_parks.contains(&park))
            .map(|dispute| SubmissionPark {
                dispute,
                park: park.clone(),
            });
        row.record_dispute(
            format!("{list_field} {park}"),
            park_edits(side, values).collect(),
            unless_submission_selects,
        );
    }
}

/// Suppress the bare park when a qualified reference describes the same park.
fn park_edits(
    side: StationSide,
    values: Vec<(Alternative, ReferenceAssociation)>,
) -> impl Iterator<Item = (Alternative, Edit)> {
    let qualified = |reference: &ReferenceAssociation| {
        reference
            .identifier()
            .as_deref()
            .is_some_and(|value| value.contains('@'))
    };
    let has_qualified = values.iter().any(|(_, reference)| qualified(reference));
    values
        .into_iter()
        .filter(move |(_, reference)| !has_qualified || qualified(reference))
        .map(move |(source, reference)| (source, Edit::Reference(side, reference)))
}

fn insert_park(
    row: &mut RowInput<'_, '_>,
    source: &Alternative,
    park: &str,
    group: GroupId,
    definitions: &DefinitionSet,
    parks: &mut BTreeMap<String, Vec<(Alternative, ReferenceAssociation)>>,
) {
    match association(park, group, definitions) {
        Ok(reference) => {
            if let Some(identity) = park_identity(&reference) {
                parks.entry(identity.to_owned()).or_default().push((
                    Alternative {
                        value: park.to_owned(),
                        ..source.clone()
                    },
                    reference,
                ));
            }
        }
        Err(error) => row.warn(source, error),
    }
}

/// Accept group-only participation when the source supplies only a contest identifier.
fn contest(row: &mut RowInput<'_, '_>, definitions: &DefinitionSet) {
    const EXCHANGE: &[&str] = &["STX", "SRX", "STX_STRING", "SRX_STRING"];
    if EXCHANGE.iter().any(|name| row.present_nonempty(name)) {
        return;
    }
    let mut choices = Vec::new();
    for source in row.values("CONTEST_ID") {
        if source.value.trim().is_empty() {
            continue;
        }
        match crate::ws_suite::contact::contest(&source.value, definitions) {
            Ok(reference) => {
                let code = source.value.trim().to_ascii_uppercase();
                choices.push((source, Edit::Contest(reference, code)));
            }
            Err(error) => row.warn(&source, error),
        }
    }
    row.record("Contest", choices);
}

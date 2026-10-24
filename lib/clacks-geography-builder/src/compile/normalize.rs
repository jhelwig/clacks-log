//! Continuity-preserving longitude normalization and polygon validation.

use anyhow::{Context, Result, ensure};
use geo::{BooleanOps, BoundingRect, MapCoords};
use geo_types::{Coord, LineString, MultiPolygon, Polygon, Rect};

#[cfg(test)]
mod tests;

/// Normalize polygon longitude strips without inferring polar caps or repairing topology.
pub(super) fn normalize(input: &MultiPolygon<f64>) -> Result<MultiPolygon<f64>> {
    let mut result = Vec::new();
    for polygon in &input.0 {
        let exterior = unwrap_ring(polygon.exterior())?;
        let west = exterior.0.iter().map(|c| c.x).fold(f64::INFINITY, f64::min);
        let east = exterior
            .0
            .iter()
            .map(|c| c.x)
            .fold(f64::NEG_INFINITY, f64::max);
        let mut holes = Vec::new();
        for ring in polygon.interiors() {
            let mut hole = unwrap_ring(ring)?;
            let first = hole.0.first().context("empty hole")?;
            let shift = 360. * (((west + east) / 2. - first.x) / 360.).round();
            for point in &mut hole.0 {
                point.x += shift;
            }
            holes.push(hole);
        }
        let lifted = MultiPolygon(vec![Polygon::new(exterior, holes)]);
        clacks_geography::geometry::validate_topology(&lifted)?;
        let bounds = lifted.bounding_rect().context("empty polygon")?;
        if bounds.min().x >= -180. && bounds.max().x <= 180. {
            result.extend(lifted.0);
            continue;
        }
        let first_strip = ((bounds.min().x + 180.) / 360.).floor() as i32;
        let last_strip = ((bounds.max().x + 180.) / 360.).floor() as i32;
        ensure!(last_strip - first_strip <= 2, "unsupported longitude span");
        for strip in first_strip..=last_strip {
            let shift = 360. * f64::from(strip);
            let window = Rect::new(
                Coord {
                    x: -180. + shift,
                    y: -90.,
                },
                Coord {
                    x: 180. + shift,
                    y: 90.,
                },
            )
            .to_polygon();
            let clipped = lifted.intersection(&MultiPolygon(vec![window]));
            result.extend(
                clipped
                    .map_coords(|point| Coord {
                        // Restore the exact clipping-window constraints after overlay quantization.
                        // Source longitude is unwrapped above, never clamped to choose its side.
                        x: point.x.clamp(-180. + shift, 180. + shift) - shift,
                        y: point.y.clamp(-90., 90.),
                    })
                    .0,
            );
        }
    }
    let output = MultiPolygon(result);
    ensure!(!output.0.is_empty(), "normalization produced no geometry");
    clacks_geography::geometry::validate_topology(&output)?;
    Ok(output)
}

/// Lift a closed ring using explicit longitude continuity and reject ambiguous winding.
fn unwrap_ring(ring: &LineString<f64>) -> Result<LineString<f64>> {
    ensure!(ring.0.len() >= 4, "undersized ring");
    ensure!(ring.0.first() == ring.0.last(), "unexpected open ring");
    ensure!(
        ring.0
            .iter()
            .all(|p| p.x.is_finite() && p.y.is_finite() && p.y.abs() <= 90. && p.x.abs() <= 540.),
        "invalid ring coordinate"
    );
    let explicit = ring.0.iter().any(|p| p.x.abs() > 180.);
    let mut output = Vec::with_capacity(ring.0.len());
    for point in &ring.0 {
        let mut point = *point;
        if let Some(previous) = output.last() {
            let previous: &Coord<f64> = previous;
            let delta = point.x - previous.x;
            if !explicit {
                point.x -= 360. * (delta / 360.).round();
            }
            ensure!(
                (point.x - previous.x).abs() < 180.,
                "ambiguous half-world edge"
            );
        }
        output.push(point);
    }
    ensure!(
        output.first() == output.last(),
        "unsupported whole-world ring closure"
    );
    Ok(LineString(output))
}

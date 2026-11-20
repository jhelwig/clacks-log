//! Explicit source-coordinate assignment and per-operation transformation.

use super::{SeamReport, coordinates::Projection, normalize::normalize, overlay::overlay};
use crate::{
    recipe::{Assignment, CoordinateMethod, CoordinateOperation},
    source::{DecodedSources, SourceFeature, check_cancelled},
};
use anyhow::{Context, Result, ensure};
use clipper2_rust::engine::ClipType;
use geo::{Contains, CoordsIter, Relate, unary_union};
use geo_types::{Coord, Geometry, LineString, MultiPolygon, Point, Polygon, Rect};
use rayon::prelude::*;
use sha2_rsa::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::AtomicBool,
};

#[cfg(test)]
mod tests;

/// Geometry assigned to one exact operation before or after transformation.
pub(super) struct OperationPart {
    /// Applied coordinate and source-representation conversion identities.
    pub operation_ids: Vec<String>,
    /// Valid geometry in the stage's coordinate system.
    pub geometry: MultiPolygon<f64>,
}

/// Transformed parts remain separate so operation seams are neither hidden nor repaired.
pub(super) struct TransformedFeature {
    /// Individually valid pieces, retaining operation provenance.
    pub parts: Vec<OperationPart>,
    /// Cross-operation boundary displacement observations.
    pub seams: Vec<SeamReport>,
}

/// An executable operation with its explicit source-coordinate mask.
struct PreparedOperation<'a> {
    /// Recipe definition supplying identity, applicability and provenance.
    definition: &'a CoordinateOperation,
    /// Source-coordinate mask; absent for entire/residual assignments.
    mask: Option<MultiPolygon<f64>>,
    /// Required numerical conversion; absent for identity/no-shift assignments.
    projection: Option<Projection>,
}

/// Validated executable operation definitions in recipe order.
pub(super) struct PreparedOperations<'a> {
    /// Operations whose masks and mandatory numerical resources are ready.
    operations: Vec<PreparedOperation<'a>>,
}

/// Prepare exact assignment masks and required grids without interpreting failures as no-shift.
pub(super) fn prepare_operations<'a>(
    definitions: &'a [CoordinateOperation],
    sources: &DecodedSources,
) -> Result<PreparedOperations<'a>> {
    let mut operations = Vec::new();
    let mut ids = BTreeSet::new();
    for definition in definitions {
        ensure!(
            !definition.id.is_empty() && ids.insert(&definition.id),
            "duplicate/empty operation ID"
        );
        ensure!(
            !definition.source_ids.is_empty()
                && !definition.description.is_empty()
                && !definition.citations.is_empty(),
            "operation lacks applicability/provenance"
        );
        let mask = match &definition.assignment {
            Assignment::Entire => None,
            Assignment::Bounds(bounds) => Some(rectangle(*bounds)?),
            Assignment::SourceMask {
                identities,
                source_ids,
                components,
                extent,
            } => {
                let mut selected = Vec::new();
                for identity in identities {
                    let source = sources
                        .features
                        .iter()
                        .find(|f| &f.identity == identity)
                        .context("mask feature not selected")?;
                    selected.extend(source_polygon(source)?.0);
                }
                for id in source_ids {
                    let matching: Vec<_> = sources
                        .features
                        .iter()
                        .filter(|f| &f.identity.source_id == id)
                        .collect();
                    ensure!(!matching.is_empty(), "mask source has no selected geometry");
                    for source in matching {
                        selected.extend(source_polygon(source)?.0);
                    }
                }
                for component in components {
                    let source = sources
                        .features
                        .iter()
                        .find(|f| f.identity == component.feature)
                        .context("component mask feature not selected")?;
                    let [x, y] = component.contains_point;
                    ensure!(
                        x.is_finite() && y.is_finite(),
                        "non-finite component anchor"
                    );
                    let matching: Vec<_> = source_polygon(source)?
                        .0
                        .into_iter()
                        .filter(|p| p.contains(&Point::new(x, y)))
                        .collect();
                    ensure!(
                        matching.len() == 1,
                        "component anchor does not identify exactly one source polygon"
                    );
                    selected.extend(matching);
                }
                ensure!(!selected.is_empty(), "empty source mask");
                let mut geometry = unary_union(selected.iter());
                if let Some(bounds) = extent {
                    geometry = intersection(&geometry, &rectangle(*bounds)?)?;
                }
                ensure!(!geometry.0.is_empty(), "source mask outside applicability");
                Some(geometry)
            }
            Assignment::Remainder {
                excluded_operation_ids,
            } => {
                ensure!(
                    !excluded_operation_ids.is_empty(),
                    "unbounded residual assignment"
                );
                for id in excluded_operation_ids {
                    let prior: &PreparedOperation<'_> = operations
                        .iter()
                        .find(|op: &&PreparedOperation<'_>| &op.definition.id == id)
                        .context("residual refers to missing/later operation")?;
                    ensure!(
                        prior.mask.is_some(),
                        "residual may exclude only explicit masks"
                    );
                    ensure!(
                        definition
                            .source_ids
                            .iter()
                            .all(|source| prior.definition.source_ids.contains(source)),
                        "residual excludes mask with different target sources"
                    );
                }
                None
            }
        };
        if let Some(mask) = &mask {
            valid(mask)?;
        }
        let projection = match &definition.method {
            CoordinateMethod::Identity | CoordinateMethod::NoShift => None,
            CoordinateMethod::Translation { x_m, y_m, z_m } => {
                Some(Projection::translation([*x_m, *y_m, *z_m])?)
            }
            CoordinateMethod::Grid { source_id } => {
                let bytes = sources
                    .grids
                    .get(source_id)
                    .context("required grid absent")?;
                Some(Projection::grid(
                    bytes,
                    &hex::encode(Sha256::digest(bytes)),
                )?)
            }
        };
        operations.push(PreparedOperation {
            definition,
            mask,
            projection,
        });
    }
    Ok(PreparedOperations { operations })
}

/// Partition one source feature without projecting its shared boundaries independently.
fn partition_feature<'a, 'b>(
    feature: &SourceFeature,
    prepared: &'a PreparedOperations<'b>,
    cancelled: &AtomicBool,
) -> Result<Vec<(&'a PreparedOperation<'b>, MultiPolygon<f64>)>> {
    check_cancelled(cancelled)?;
    let input = source_polygon(feature).with_context(|| {
        format!(
            "normalize source {}:{}",
            feature.identity.source_id, feature.identity.feature_id
        )
    })?;
    let operations: Vec<_> = prepared
        .operations
        .iter()
        .filter(|op| {
            op.definition
                .source_ids
                .contains(&feature.identity.source_id)
        })
        .collect();
    ensure!(
        !operations.is_empty(),
        "source has no coordinate assignment"
    );
    let masks: BTreeSet<_> = operations
        .iter()
        .filter(|operation| operation.mask.is_some())
        .map(|operation| &operation.definition.id)
        .collect();
    let complete_residual = operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.definition.assignment,
                Assignment::Remainder { .. }
            )
        })
        .count()
        == 1
        && operations.len() == masks.len() + 1
        && operations
            .iter()
            .any(|operation| match &operation.definition.assignment {
                Assignment::Remainder {
                    excluded_operation_ids,
                } => excluded_operation_ids.iter().collect::<BTreeSet<_>>() == masks,
                _ => false,
            });
    let mut source_parts: Vec<(&PreparedOperation<'_>, MultiPolygon<f64>)> = Vec::new();
    for operation in operations {
        let piece = match &operation.definition.assignment {
            Assignment::Entire => input.clone(),
            Assignment::Remainder {
                excluded_operation_ids,
            } => {
                let mut remaining = input.clone();
                for id in excluded_operation_ids {
                    let excluded = prepared
                        .operations
                        .iter()
                        .find(|op| &op.definition.id == id)
                        .context("missing residual mask")?;
                    if includes_whole_source(excluded, feature, &input)? {
                        remaining = MultiPolygon(vec![]);
                        break;
                    }
                    let mask = excluded.mask.as_ref().context("missing residual mask")?;
                    remaining = subtract(&remaining, mask)?;
                }
                remaining
            }
            _ if includes_whole_source(operation, feature, &input)? => input.clone(),
            _ => intersection(
                &input,
                operation.mask.as_ref().context("missing operation mask")?,
            )?,
        };
        if piece.0.is_empty() {
            continue;
        }
        valid(&piece)?;
        for (other_operation, other) in &source_parts {
            if complete_residual
                && (matches!(
                    operation.definition.assignment,
                    Assignment::Remainder { .. }
                ) || matches!(
                    other_operation.definition.assignment,
                    Assignment::Remainder { .. }
                ))
            {
                continue;
            }
            ensure!(
                intersection(&piece, other)?.0.is_empty(),
                "overlapping source-coordinate assignments"
            );
        }
        source_parts.push((operation, piece));
    }
    if !complete_residual {
        let mut remaining = input;
        for (_, piece) in &source_parts {
            remaining = subtract(&remaining, piece)?;
        }
        ensure!(
            remaining.0.is_empty(),
            "uncovered source-coordinate assignment"
        );
    }
    Ok(source_parts)
}

/// Transform a batch after consistently noding shared source boundaries within each operation.
pub(super) fn transform_features(
    features: &[&SourceFeature],
    prepared: &PreparedOperations<'_>,
    cancelled: &AtomicBool,
) -> Result<BTreeMap<clacks_geography::artifact::SourceIdentity, TransformedFeature>> {
    // Preserve feature order for shared-edge numbering and the projection loop.
    let mut partitions = features
        .par_iter()
        .map(|feature| {
            partition_feature(feature, prepared, cancelled).with_context(|| {
                format!(
                    "partition source {}:{}",
                    feature.identity.source_id, feature.identity.feature_id
                )
            })
        })
        .collect::<Result<Vec<_>>>()?;
    node_boundaries(&mut partitions, cancelled)?;
    let mut result = BTreeMap::new();
    for (feature, source_parts) in features.iter().zip(partitions) {
        let transformed =
            transform_partition(feature, source_parts, cancelled).with_context(|| {
                format!(
                    "transform source {}:{}",
                    feature.identity.source_id, feature.identity.feature_id
                )
            })?;
        ensure!(
            result
                .insert(feature.identity.clone(), transformed)
                .is_none(),
            "duplicate transformed source identity"
        );
    }
    Ok(result)
}

/// Project already-noded parts, retaining each exact operation and its seam diagnostics.
fn transform_partition(
    feature: &SourceFeature,
    source_parts: Vec<(&PreparedOperation<'_>, MultiPolygon<f64>)>,
    cancelled: &AtomicBool,
) -> Result<TransformedFeature> {
    let seams = measure_seams(feature, &source_parts)?;
    let mut parts = Vec::new();
    let mut remaining_vertices = 1_000_000usize;
    for (operation, geometry) in source_parts {
        check_cancelled(cancelled)?;
        let geometry = if let Some(projection) = &operation.projection {
            let transformed =
                transform_polygon(&geometry, projection, cancelled, &mut remaining_vertices)?;
            normalize(&transformed)
                .with_context(|| format!("validate operation {} output", operation.definition.id))?
        } else {
            remaining_vertices = remaining_vertices
                .checked_sub(geometry.coords_iter().count())
                .context("feature vertex budget exhausted")?;
            geometry
        };
        parts.push(OperationPart {
            operation_ids: vec![operation.definition.id.clone()],
            geometry,
        });
    }
    Ok(TransformedFeature { parts, seams })
}

/// Recognize exact set inclusion when an assignment mask explicitly contains the entire source.
fn includes_whole_source(
    operation: &PreparedOperation<'_>,
    feature: &SourceFeature,
    input: &MultiPolygon<f64>,
) -> Result<bool> {
    let Assignment::SourceMask {
        identities,
        source_ids,
        extent,
        ..
    } = &operation.definition.assignment
    else {
        return Ok(false);
    };
    if !identities.contains(&feature.identity) && !source_ids.contains(&feature.identity.source_id)
    {
        return Ok(false);
    }
    Ok(match extent {
        Some(bounds) => rectangle(*bounds)?.relate(input).is_covers(),
        None => true,
    })
}

/// Convert supported original polygons or explicitly closed source linework to normalized areas.
pub(super) fn source_polygon(feature: &SourceFeature) -> Result<MultiPolygon<f64>> {
    let polygons = match &feature.geometry {
        Geometry::Polygon(p) => MultiPolygon(vec![p.clone()]),
        Geometry::MultiPolygon(p) => p.clone(),
        Geometry::LineString(line) => MultiPolygon(vec![Polygon::new(line.clone(), vec![])]),
        _ => anyhow::bail!("source is not polygon linework"),
    };
    normalize(&polygons)
}

/// Validate a finite applicability rectangle, explicitly splitting a date-line interval.
fn rectangle([west, south, east, north]: [f64; 4]) -> Result<MultiPolygon<f64>> {
    ensure!(
        [west, south, east, north].into_iter().all(f64::is_finite)
            && south >= -90.
            && north <= 90.
            && south < north
            && (-180. ..=180.).contains(&west)
            && (-180. ..=180.).contains(&east)
            && west != east,
        "invalid assignment bounds"
    );
    let ranges = if west < east {
        vec![(west, east)]
    } else {
        vec![(west, 180.), (-180., east)]
    };
    Ok(MultiPolygon(
        ranges
            .into_iter()
            .map(|(w, e)| {
                Rect::new(Coord { x: w, y: south }, Coord { x: e, y: north }).to_polygon()
            })
            .collect(),
    ))
}

/// Reject invalid area topology before overlay.
fn valid(geometry: &MultiPolygon<f64>) -> Result<()> {
    clacks_geography::geometry::validate_topology(geometry)?;
    Ok(())
}

/// Preserve exact contained geometry and use overlay only when clipping is required.
fn intersection(a: &MultiPolygon<f64>, b: &MultiPolygon<f64>) -> Result<MultiPolygon<f64>> {
    if a.relate(b).is_covers() {
        Ok(b.clone())
    } else if b.relate(a).is_covers() {
        Ok(a.clone())
    } else if a.relate(b).is_disjoint() {
        Ok(MultiPolygon(vec![]))
    } else {
        overlay(a, b, ClipType::Intersection)
    }
}

/// Preserve disjoint geometry without quantization during subtraction.
fn subtract(a: &MultiPolygon<f64>, b: &MultiPolygon<f64>) -> Result<MultiPolygon<f64>> {
    if b.relate(a).is_covers() {
        Ok(MultiPolygon(vec![]))
    } else if a.relate(b).is_disjoint() {
        Ok(a.clone())
    } else {
        overlay(a, b, ClipType::Difference)
    }
}

/// Transform all rings with a bounded shared-edge subdivision rule.
fn transform_polygon(
    input: &MultiPolygon<f64>,
    projection: &Projection,
    cancelled: &AtomicBool,
    budget: &mut usize,
) -> Result<MultiPolygon<f64>> {
    let mut ring = |line: &LineString<f64>| -> Result<LineString<f64>> {
        let mut output = Vec::new();
        for pair in line.0.windows(2) {
            check_cancelled(cancelled)?;
            let [a, b] = pair else {
                anyhow::bail!("source edge missing endpoint")
            };
            let segment = projection.segment(*a, *b, *budget)?;
            if output.is_empty() {
                output.extend(segment);
            } else {
                output.extend(segment.into_iter().skip(1));
            }
            ensure!(output.len() <= *budget, "feature vertex budget exhausted");
        }
        *budget = budget
            .checked_sub(output.len())
            .context("feature vertex budget exhausted")?;
        Ok(LineString(output))
    };
    let mut result = Vec::new();
    for polygon in &input.0 {
        let exterior = ring(polygon.exterior())?;
        let holes = polygon
            .interiors()
            .iter()
            .map(&mut ring)
            .collect::<Result<Vec<_>>>()?;
        result.push(Polygon::new(exterior, holes));
    }
    Ok(MultiPolygon(result))
}

/// Measure exact-operation disagreement at vertices shared by the source partition.
fn measure_seams(
    feature: &SourceFeature,
    parts: &[(&PreparedOperation<'_>, MultiPolygon<f64>)],
) -> Result<Vec<SeamReport>> {
    let mut points: BTreeMap<(u64, u64), Vec<&PreparedOperation<'_>>> = BTreeMap::new();
    for (operation, geometry) in parts {
        for point in geometry.coords_iter() {
            let key = (point.x.to_bits(), point.y.to_bits());
            let owners = points.entry(key).or_default();
            if !owners
                .iter()
                .any(|owner| owner.definition.id == operation.definition.id)
            {
                owners.push(operation);
            }
        }
    }
    let mut seams: BTreeMap<[String; 2], SeamReport> = BTreeMap::new();
    for ((x, y), owners) in points {
        let point = Coord {
            x: f64::from_bits(x),
            y: f64::from_bits(y),
        };
        for (i, a) in owners.iter().enumerate() {
            for b in owners.iter().skip(i + 1) {
                let first = if let Some(projection) = &a.projection {
                    projection.apply(point)?
                } else {
                    point
                };
                let second = if let Some(projection) = &b.projection {
                    projection.apply(point)?
                } else {
                    point
                };
                let displacement = ((first.x - second.x) * point.y.to_radians().cos())
                    .hypot(first.y - second.y)
                    * 111_320.;
                let mut ids = [a.definition.id.clone(), b.definition.id.clone()];
                ids.sort();
                let report = seams.entry(ids.clone()).or_insert_with(|| SeamReport {
                    source_identity: feature.identity.clone(),
                    operation_ids: ids,
                    sampled_vertices: 0,
                    max_sampled_displacement_m: 0.,
                });
                report.sampled_vertices += 1;
                report.max_sampled_displacement_m =
                    report.max_sampled_displacement_m.max(displacement);
            }
        }
    }
    Ok(seams.into_values().collect())
}

/// Source edge identity used to insert shared collinear endpoints before projection.
#[derive(Clone)]
struct SharedEdge {
    /// Batch-wide edge position for rebuilding rings without changing source coordinates.
    id: usize,
    /// Unmodified source-coordinate segment.
    segment: geo_types::Line<f64>,
}

impl geo::algorithm::sweep::Cross for SharedEdge {
    type Scalar = f64;

    fn line(&self) -> geo_types::Line<f64> {
        self.segment
    }
}

/// Split overlapping source edges at a common set of exact endpoints per operation.
fn node_boundaries(
    partitions: &mut [Vec<(&PreparedOperation<'_>, MultiPolygon<f64>)>],
    cancelled: &AtomicBool,
) -> Result<()> {
    use geo::algorithm::{line_intersection::LineIntersection, sweep::Intersections};
    let mut groups: BTreeMap<&str, Vec<SharedEdge>> = BTreeMap::new();
    let mut next_id = 0;
    for parts in partitions.iter() {
        for (operation, geometry) in parts {
            for polygon in &geometry.0 {
                for ring in std::iter::once(polygon.exterior()).chain(polygon.interiors()) {
                    for segment in ring.lines() {
                        groups
                            .entry(&operation.definition.id)
                            .or_default()
                            .push(SharedEdge {
                                id: next_id,
                                segment,
                            });
                        next_id += 1;
                    }
                }
            }
        }
    }
    let mut splits: BTreeMap<usize, Vec<Coord<f64>>> = BTreeMap::new();
    for edges in groups.into_values() {
        for (a, b, intersection) in Intersections::from_iter(edges) {
            check_cancelled(cancelled)?;
            if let LineIntersection::Collinear { intersection } = intersection {
                for edge in [a, b] {
                    for point in [intersection.start, intersection.end] {
                        if point != edge.segment.start && point != edge.segment.end {
                            splits.entry(edge.id).or_default().push(point);
                        }
                    }
                }
            }
        }
    }
    let mut edge_id = 0;
    for parts in partitions {
        let mut remaining_vertices = 1_000_000usize;
        for (_, geometry) in parts {
            for polygon in &mut geometry.0 {
                let mut rebuild = |ring: &mut LineString<f64>| -> Result<()> {
                    let mut output = Vec::new();
                    for segment in ring.lines() {
                        output.push(segment.start);
                        if let Some(nodes) = splits.get_mut(&edge_id) {
                            nodes.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
                            nodes.dedup();
                            if segment
                                .start
                                .x
                                .total_cmp(&segment.end.x)
                                .then(segment.start.y.total_cmp(&segment.end.y))
                                .is_gt()
                            {
                                nodes.reverse();
                            }
                            output.extend(nodes.iter().copied());
                        }
                        edge_id += 1;
                    }
                    if let Some(first) = output.first().copied() {
                        output.push(first);
                    }
                    remaining_vertices = remaining_vertices
                        .checked_sub(output.len())
                        .context("noded feature vertex budget exhausted")?;
                    ring.0 = output;
                    Ok(())
                };
                polygon.try_exterior_mut(&mut rebuild)?;
                polygon.try_interiors_mut(|rings| {
                    for ring in rings {
                        rebuild(ring)?;
                    }
                    Ok::<_, anyhow::Error>(())
                })?;
                check_cancelled(cancelled)?;
            }
        }
    }
    Ok(())
}

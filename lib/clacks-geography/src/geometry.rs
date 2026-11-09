//! Polygon validation shared by source compilation and artifact storage.

use crate::error::{GeographyError, invalid};
use geo::{BoundingRect, CoordsIter};
use geo_types::MultiPolygon;

/// Reject malformed or unnormalized geometry before library topology operations.
pub(crate) fn validate(geometry: &MultiPolygon<f64>) -> Result<[f64; 4], GeographyError> {
    if geometry.0.is_empty()
        || geometry.coords_iter().any(|p| {
            !p.x.is_finite()
                || !p.y.is_finite()
                || !(-180. ..=180.).contains(&p.x)
                || !(-90. ..=90.).contains(&p.y)
        })
    {
        return Err(invalid("empty, non-finite or out-of-domain geometry"));
    }
    validate_topology(geometry)?;
    let bounds = geometry
        .bounding_rect()
        .ok_or_else(|| invalid("empty geometry bounds"))?;
    if bounds.min().x >= bounds.max().x || bounds.min().y >= bounds.max().y {
        return Err(invalid("geometry has no area"));
    }
    Ok([
        bounds.min().x,
        bounds.min().y,
        bounds.max().x,
        bounds.max().y,
    ])
}

/// Merge nonempty feature extents without introducing rounded coordinates.
pub(crate) fn extend(extent: &mut Option<[f64; 4]>, next: [f64; 4]) {
    let [west, south, east, north] = next;
    *extent = Some(match *extent {
        None => next,
        Some([w, s, e, n]) => [w.min(west), s.min(south), e.max(east), n.max(north)],
    });
}

/// Validate finite polygon topology using spatial intersection candidates.
///
/// Accepts longitude-unwrapped producer geometry as well as artifact geometry;
/// coordinate-reference and domain validation remain the caller's responsibility.
/// Rejects non-simple rings, crossing/nested holes and overlapping or edge-sharing
/// polygon members. Uses `geo` intersection predicates without altering coordinates.
pub fn validate_topology(geometry: &MultiPolygon<f64>) -> Result<(), GeographyError> {
    use geo::coordinate_position::CoordPos;
    use geo::dimensions::Dimensions;
    use geo::{Intersects, PreparedGeometry, Relate};
    use geo_types::Polygon;
    if geometry.0.is_empty() {
        return Err(invalid("empty geometry"));
    }
    for (index, polygon) in geometry.0.iter().enumerate() {
        for ring in std::iter::once(polygon.exterior()).chain(polygon.interiors()) {
            validate_ring(ring)?;
        }
        let shell = Polygon::new(polygon.exterior().clone(), vec![]);
        let prepared = PreparedGeometry::from(&shell);
        for (hole_index, hole) in polygon.interiors().iter().enumerate() {
            let hole = Polygon::new(hole.clone(), vec![]);
            let relationship = prepared.relate(&hole);
            if !relationship.is_contains()
                || relationship.get(CoordPos::OnBoundary, CoordPos::OnBoundary)
                    == Dimensions::OneDimensional
            {
                return Err(invalid("hole is not contained or shares a shell edge"));
            }
            for other in polygon.interiors().iter().skip(hole_index + 1) {
                let other = Polygon::new(other.clone(), vec![]);
                if hole
                    .bounding_rect()
                    .ok_or_else(|| invalid("empty hole bounds"))?
                    .intersects(
                        &other
                            .bounding_rect()
                            .ok_or_else(|| invalid("empty hole bounds"))?,
                    )
                {
                    let relationship = hole.relate(&other);
                    if relationship.get(CoordPos::Inside, CoordPos::Inside)
                        == Dimensions::TwoDimensional
                        || relationship.get(CoordPos::OnBoundary, CoordPos::OnBoundary)
                            == Dimensions::OneDimensional
                    {
                        return Err(invalid("holes overlap or share an edge"));
                    }
                }
            }
        }
        validate_connected_interior(polygon)?;
        let bounds = polygon
            .bounding_rect()
            .ok_or_else(|| invalid("empty polygon bounds"))?;
        let mut prepared_polygon = None;
        for other in geometry.0.iter().skip(index + 1) {
            if bounds.intersects(
                &other
                    .bounding_rect()
                    .ok_or_else(|| invalid("empty polygon bounds"))?,
            ) {
                let relationship = prepared_polygon
                    .get_or_insert_with(|| PreparedGeometry::from(polygon))
                    .relate(other);
                if relationship.get(CoordPos::Inside, CoordPos::Inside)
                    == Dimensions::TwoDimensional
                    || relationship.get(CoordPos::OnBoundary, CoordPos::OnBoundary)
                        == Dimensions::OneDimensional
                {
                    return Err(invalid("polygon members overlap or share an edge"));
                }
            }
        }
    }
    Ok(())
}

/// A ring edge carrying its original adjacency position for intersection validation.
#[derive(Clone)]
struct RingEdge {
    /// Position among nondegenerate edges.
    index: usize,
    /// Unmodified source segment.
    segment: geo_types::Line<f64>,
}

impl geo::algorithm::sweep::Cross for RingEdge {
    type Scalar = f64;
    fn line(&self) -> geo_types::Line<f64> {
        self.segment
    }
}

/// Reject self-intersections using the dependency's sweep rather than every segment pair.
fn validate_ring(ring: &geo_types::LineString<f64>) -> Result<(), GeographyError> {
    use geo::algorithm::{line_intersection::LineIntersection, sweep::Intersections};
    if ring.0.len() < 4
        || ring.0.first() != ring.0.last()
        || ring
            .0
            .iter()
            .any(|point| !point.x.is_finite() || !point.y.is_finite())
    {
        return Err(invalid("undersized, open or non-finite ring"));
    }
    let edges: Vec<_> = ring
        .lines()
        .filter(|edge| edge.start != edge.end)
        .enumerate()
        .map(|(index, segment)| RingEdge { index, segment })
        .collect();
    if edges.len() < 3 {
        return Err(invalid("ring has fewer than three nondegenerate edges"));
    }
    let count = edges.len();
    for (first, second, intersection) in Intersections::from_iter(edges) {
        let adjacent = first.index.abs_diff(second.index) == 1
            || first.index.abs_diff(second.index) == count - 1;
        match intersection {
            LineIntersection::SinglePoint { intersection, .. }
                if adjacent
                    && (intersection == first.segment.start
                        || intersection == first.segment.end)
                    && (intersection == second.segment.start
                        || intersection == second.segment.end) => {}
            _ => return Err(invalid("ring self-intersection")),
        }
    }
    Ok(())
}

/// Ring identity attached to an unchanged segment for contact analysis.
#[derive(Clone)]
struct ContactEdge {
    /// Shell is zero; holes follow in their original order.
    ring: usize,
    /// Unmodified boundary segment.
    segment: geo_types::Line<f64>,
}

impl geo::algorithm::sweep::Cross for ContactEdge {
    type Scalar = f64;
    fn line(&self) -> geo_types::Line<f64> {
        self.segment
    }
}

/// Reject cycles in the ring/contact incidence graph that disconnect the area interior.
fn validate_connected_interior(polygon: &geo_types::Polygon<f64>) -> Result<(), GeographyError> {
    use geo::algorithm::{line_intersection::LineIntersection, sweep::Intersections};
    use std::collections::{BTreeMap, BTreeSet};
    if polygon.interiors().is_empty() {
        return Ok(());
    }
    let rings: Vec<_> = std::iter::once(polygon.exterior())
        .chain(polygon.interiors())
        .collect();
    let edges = rings.iter().enumerate().flat_map(|(ring, line)| {
        line.lines()
            .filter(|line| line.start != line.end)
            .map(move |segment| ContactEdge { ring, segment })
    });
    let mut contacts: BTreeMap<(u64, u64), BTreeSet<usize>> = BTreeMap::new();
    for (first, second, intersection) in Intersections::from_iter(edges) {
        if first.ring == second.ring {
            continue;
        }
        let LineIntersection::SinglePoint { intersection, .. } = intersection else {
            return Err(invalid("rings share an edge"));
        };
        let key = (
            if intersection.x == 0. {
                0
            } else {
                intersection.x.to_bits()
            },
            if intersection.y == 0. {
                0
            } else {
                intersection.y.to_bits()
            },
        );
        contacts
            .entry(key)
            .or_default()
            .extend([first.ring, second.ring]);
    }
    let mut graph: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for (index, contact) in contacts.into_values().enumerate() {
        let point = rings.len() + index;
        for ring in contact {
            graph.entry(ring).or_default().insert(point);
            graph.entry(point).or_default().insert(ring);
        }
    }
    let mut visited = BTreeSet::new();
    for start in graph.keys() {
        if visited.contains(start) {
            continue;
        }
        let mut stack = vec![(*start, None)];
        while let Some((node, parent)) = stack.pop() {
            if !visited.insert(node) {
                return Err(invalid("ring contacts disconnect polygon interior"));
            }
            for neighbor in graph.get(&node).into_iter().flatten() {
                if Some(*neighbor) != parent {
                    stack.push((*neighbor, Some(node)));
                }
            }
        }
    }
    Ok(())
}

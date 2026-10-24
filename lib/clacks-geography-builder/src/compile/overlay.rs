//! Checked producer overlays that retain original boundary and intersection coordinates.

use anyhow::{Context, Result, ensure};
use clipper2_rust::{
    FillRule,
    clipper::poly_tree_to_faces64,
    core::{Path64, Paths64, Point64},
    engine::ClipType,
    engine_public::{Clipper64, PolyTree64},
};
use geo::{BoundingRect, Orient, orient::Direction};
use geo_types::{Coord, LineString, MultiPolygon, Polygon};
use std::collections::BTreeMap;

/// Maximum binary scale, retaining safe integer arithmetic at geographic extents.
const MAX_SCALE: f64 = 281_474_976_710_656.;

/// Integer magnitude below Clipper64's 2^61 limit and within i128 determinant arithmetic.
const MAX_COORDINATE: f64 = 1e17;

/// Perform a nonzero-fill overlay without silently collapsing distinct boundary nodes.
///
/// Original vertices and calculated intersections are restored after integer clipping.
/// Unresolvable nodes and unrecognized output intersections stop compilation.
pub(super) fn overlay(
    a: &MultiPolygon<f64>,
    b: &MultiPolygon<f64>,
    kind: ClipType,
) -> Result<MultiPolygon<f64>> {
    use geo::algorithm::{line_intersection::LineIntersection, sweep::Intersections};
    let a = a.orient(Direction::Default);
    let b = b.orient(Direction::Default);
    let bounds = a.bounding_rect().context("empty overlay subject")?;
    let other = b.bounding_rect().context("empty overlay clip")?;
    let origin = Coord {
        x: bounds.min().x.min(other.min().x),
        y: bounds.min().y.min(other.min().y),
    };
    let extent = (bounds.max().x.max(other.max().x) - origin.x)
        .max(bounds.max().y.max(other.max().y) - origin.y);
    ensure!(
        extent.is_finite() && extent > 0.,
        "invalid producer overlay extent"
    );
    let scale = 2_f64
        .powf((MAX_COORDINATE / extent).log2().floor())
        .min(MAX_SCALE);
    let mut lattice = Lattice {
        origin,
        scale,
        nodes: BTreeMap::new(),
    };
    let mut subject = lattice.paths(&a)?;
    let mut clip = lattice.paths(&b)?;
    let edges =
        a.0.iter()
            .chain(&b.0)
            .flat_map(|polygon| std::iter::once(polygon.exterior()).chain(polygon.interiors()))
            .flat_map(|ring| ring.lines())
            .enumerate()
            .map(|(id, segment)| OverlayEdge { id, segment });
    let mut splits: BTreeMap<usize, Vec<Coord<f64>>> = BTreeMap::new();
    for (a, b, intersection) in Intersections::from_iter(edges) {
        let nodes = match intersection {
            LineIntersection::SinglePoint { intersection, .. } => vec![intersection],
            LineIntersection::Collinear { intersection } => {
                vec![intersection.start, intersection.end]
            }
        };
        for node in nodes {
            lattice.insert(node)?;
            for edge in [&a, &b] {
                if node != edge.segment.start && node != edge.segment.end {
                    splits.entry(edge.id).or_default().push(node);
                }
            }
        }
    }
    // Make every crossing an input vertex in both paths. Clipping then selects
    // registered nodes instead of independently rounding the same intersection.
    let mut edge_id = 0;
    for path in subject.iter_mut().chain(&mut clip) {
        let mut noded = Vec::new();
        for pair in path.windows(2) {
            let [a, b] = pair else {
                anyhow::bail!("missing producer edge endpoint")
            };
            noded.push(*a);
            if let Some(nodes) = splits.get_mut(&edge_id) {
                nodes.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
                nodes.dedup();
                if (a.x, a.y) > (b.x, b.y) {
                    nodes.reverse();
                }
                for point in nodes {
                    noded.push(lattice.insert(*point)?);
                }
            }
            edge_id += 1;
        }
        noded.push(*path.last().context("empty producer path")?);
        *path = noded;
    }
    let mut clipper = Clipper64::new();
    clipper.set_preserve_collinear(true);
    clipper.add_subject(&subject);
    clipper.add_clip(&clip);
    let mut tree = PolyTree64::new();
    ensure!(
        clipper.execute_tree(kind, FillRule::NonZero, &mut tree, &mut Paths64::new()),
        "producer overlay failed ({})",
        clipper.error_code()
    );
    let polygons = poly_tree_to_faces64(&tree)
        .into_iter()
        .map(|face| {
            Ok(Polygon::new(
                lattice.restore(&face.outer)?,
                face.holes
                    .iter()
                    .map(|ring| lattice.restore(ring))
                    .collect::<Result<_>>()?,
            ))
        })
        .collect::<Result<_>>()?;
    let result = MultiPolygon(polygons);
    if !result.0.is_empty() {
        clacks_geography::geometry::validate_topology(&result)
            .context("invalid producer overlay result")?;
    }
    Ok(result)
}

/// One shared coordinate lattice and its exact source/intersection-node dictionary.
struct Lattice {
    /// Local origin limiting integer magnitudes.
    origin: Coord<f64>,
    /// Binary units per degree.
    scale: f64,
    /// Unrounded coordinates for every recognized integer node.
    nodes: BTreeMap<(i64, i64), Coord<f64>>,
}

impl Lattice {
    /// Quantize one node, rejecting unresolved neighbors before clipping can erase them.
    fn insert(&mut self, point: Coord<f64>) -> Result<Point64> {
        let x = ((point.x - self.origin.x) * self.scale).round();
        let y = ((point.y - self.origin.y) * self.scale).round();
        ensure!(
            x.is_finite()
                && y.is_finite()
                && x.abs() <= MAX_COORDINATE
                && y.abs() <= MAX_COORDINATE,
            "producer overlay coordinate range exceeded"
        );
        let key = (x as i64, y as i64);
        for dx in [-1, 0, 1] {
            for dy in [-1, 0, 1] {
                if let Some(previous) = self.nodes.get(&(key.0 + dx, key.1 + dy)) {
                    ensure!(
                        *previous == point,
                        "distinct boundary nodes unresolved at producer overlay precision"
                    );
                }
            }
        }
        self.nodes.insert(key, point);
        Ok(Point64::new(key.0, key.1))
    }

    /// Preserve consistent shell/hole winding and every representable boundary turn.
    fn paths(&mut self, geometry: &MultiPolygon<f64>) -> Result<Paths64> {
        use geo::kernels::{Kernel, Orientation, RobustKernel};
        let oriented = geometry.orient(Direction::Default);
        let mut paths = Vec::new();
        for polygon in &oriented.0 {
            for ring in std::iter::once(polygon.exterior()).chain(polygon.interiors()) {
                let path = ring
                    .0
                    .iter()
                    .map(|point| self.insert(*point))
                    .collect::<Result<Vec<_>>>()?;
                let count = ring.0.len().saturating_sub(1);
                let mut nodes = ring.0.iter().zip(&path).take(count).cycle();
                let mut previous = nodes.next().context("empty producer ring")?;
                let mut current = nodes.next().context("empty producer ring")?;
                for next in nodes.take(count) {
                    let original = RobustKernel::orient2d(*previous.0, *current.0, *next.0);
                    let abx = i128::from(current.1.x) - i128::from(previous.1.x);
                    let aby = i128::from(current.1.y) - i128::from(previous.1.y);
                    let acx = i128::from(next.1.x) - i128::from(previous.1.x);
                    let acy = i128::from(next.1.y) - i128::from(previous.1.y);
                    let converted = match (abx * acy - aby * acx).cmp(&0) {
                        std::cmp::Ordering::Less => Orientation::Clockwise,
                        std::cmp::Ordering::Equal => Orientation::Collinear,
                        std::cmp::Ordering::Greater => Orientation::CounterClockwise,
                    };
                    ensure!(
                        original == converted,
                        "boundary turn unresolved at producer overlay precision"
                    );
                    previous = current;
                    current = next;
                }
                paths.push(path);
            }
        }
        Ok(paths)
    }

    /// Rebuild a closed ring using exact, uniquely identified boundary nodes.
    fn restore(&self, path: &Path64) -> Result<LineString<f64>> {
        let mut ring = Vec::with_capacity(path.len() + 1);
        for point in path {
            let restored = if let Some(original) = self.nodes.get(&(point.x, point.y)) {
                *original
            } else {
                // Intersection arithmetic can round by one unit in either engine.
                // Only a unique registered node is safe to restore.
                let nearby: Vec<_> = [-1, 0, 1]
                    .into_iter()
                    .flat_map(|dx| {
                        [-1, 0, 1]
                            .into_iter()
                            .filter_map(move |dy| self.nodes.get(&(point.x + dx, point.y + dy)))
                    })
                    .collect();
                let [original] = nearby.as_slice() else {
                    anyhow::bail!("unrecognized intersection at producer overlay precision")
                };
                **original
            };
            if ring.last() != Some(&restored) {
                ring.push(restored);
            }
        }
        let first = *ring.first().context("empty producer output ring")?;
        ring.push(first);
        Ok(LineString(ring))
    }
}

/// Original boundary edge carrying its path position through intersection noding.
#[derive(Clone)]
struct OverlayEdge {
    /// Global edge position in subject/clip ring order.
    id: usize,
    /// Unmodified original segment.
    segment: geo_types::Line<f64>,
}

impl geo::algorithm::sweep::Cross for OverlayEdge {
    type Scalar = f64;

    fn line(&self) -> geo_types::Line<f64> {
        self.segment
    }
}

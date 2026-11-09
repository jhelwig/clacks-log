//! Exact-node conversion from Clipper's touching contours to simple polygon rings.

use super::Lattice;
use anyhow::{Context, Result, ensure};
use clipper2_rust::{
    clipper::poly_tree_to_faces64,
    core::{Path64, Point64},
    engine_public::PolyTree64,
};
use geo::{Relate, Winding, winding_order::WindingOrder};
use geo_types::{MultiPolygon, Polygon};
use std::collections::BTreeMap;

/// Separate contour cycles at identical integer nodes, preserving every boundary edge.
///
/// Clipper faces can contain a ring visiting a touching node more than once.
/// This changes only ring ownership: each resulting cycle retains its coordinates,
/// winding and area, including arbitrarily small classified components.
pub(super) fn restore_faces(tree: &PolyTree64, lattice: &Lattice) -> Result<MultiPolygon<f64>> {
    let mut polygons = Vec::new();
    for face in poly_tree_to_faces64(tree) {
        let mut shells = Vec::new();
        let mut holes = Vec::new();
        for path in std::iter::once(&face.outer).chain(&face.holes) {
            for cycle in cycles(path)? {
                let ring = lattice.restore(&cycle)?;
                match ring
                    .winding_order()
                    .context("degenerate producer contour cycle")?
                {
                    WindingOrder::CounterClockwise => shells.push(Polygon::new(ring, vec![])),
                    WindingOrder::Clockwise => holes.push(ring),
                }
            }
        }
        // A single shell has no ownership choice; the final topology check
        // validates its holes without reconstructing the same relationship graph.
        if let [shell] = shells.as_mut_slice() {
            for hole in holes {
                shell.interiors_push(hole);
            }
            polygons.extend(shells);
            continue;
        }
        for hole in holes {
            let hole_polygon = Polygon::new(hole.clone(), vec![]);
            let mut owner: Option<usize> = None;
            for (index, shell) in shells.iter().enumerate() {
                if !shell.relate(&hole_polygon).is_covers() {
                    continue;
                }
                if let Some(previous) = owner {
                    let other = shells
                        .get(previous)
                        .context("missing producer contour shell")?;
                    let previous_contains = other.relate(shell).is_covers();
                    let next_contains = shell.relate(other).is_covers();
                    ensure!(
                        previous_contains != next_contains,
                        "ambiguous producer contour hole ownership"
                    );
                    if previous_contains {
                        owner = Some(index);
                    }
                } else {
                    owner = Some(index);
                }
            }
            shells
                .get_mut(owner.context("producer contour hole has no shell")?)
                .context("missing producer contour shell")?
                .interiors_push(hole);
        }
        polygons.extend(shells);
    }
    Ok(MultiPolygon(polygons))
}

/// Extract closed traversal cycles at exact repeated nodes without creating or removing edges.
fn cycles(path: &Path64) -> Result<Vec<Path64>> {
    let first = path.first().context("empty producer contour")?;
    let closed = path.len() > 1 && path.first() == path.last();
    let count = path.len().saturating_sub(usize::from(closed));
    let mut positions = BTreeMap::new();
    let mut walk: Vec<Point64> = Vec::new();
    let mut result = Vec::new();
    for point in path.iter().take(count).chain(std::iter::once(first)) {
        let key = (point.x, point.y);
        if let Some(start) = positions.get(&key).copied() {
            let cycle = walk.split_off(start);
            ensure!(cycle.len() >= 3, "degenerate producer contour cycle");
            for node in &cycle {
                positions.remove(&(node.x, node.y));
            }
            result.push(cycle);
        }
        positions.insert(key, walk.len());
        walk.push(*point);
    }
    ensure!(walk.len() == 1, "unclosed producer contour traversal");
    Ok(result)
}

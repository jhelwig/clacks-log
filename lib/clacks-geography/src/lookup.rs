//! Independent per-field candidates and coverage for native point or whole-grid queries.

/// Metric name for complete geographic lookup.
pub const LOOKUP_DURATION_METRIC: &str = "clacks_geography_lookup_duration_seconds";

use crate::{
    artifact::{
        Field, ParentScope, SourceIdentity,
        read::{Bundle, decode_feature},
    },
    error::{GeographyError, invalid},
};
use clacks_model::{
    location::{CqZone, ItuZone},
    position::{LookupGeometry, RationalDegree},
};
use flatgeobuf::{FallibleStreamingIterator, FgbReader};
use geo::{
    BoundingRect, Intersects, Orient, Relate, coordinate_position::CoordPos,
    dimensions::Dimensions, orient::Direction,
};
use geo_types::{Coord, MultiPolygon, Point, Polygon, Rect};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{BufReader, Seek},
    time::Instant,
};

/// Proportion of a query supported by known classifications for one field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coverage {
    /// All requested geometry has at least one known classification.
    Complete,
    /// Some positive area remains unclassified.
    Partial,
    /// No requested point or positive area has a known classification.
    None,
}

/// A distinct scoped geographic value and its supporting source evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate<T> {
    /// Explicit recordable value, not inferred from a label.
    pub value: T,
    /// Context distinguishing equal subdivision codes.
    pub parent_scope: ParentScope,
    /// Human-readable source/mapping label.
    pub display_label: String,
    /// All contributing original features.
    pub source_identities: Vec<SourceIdentity>,
    /// Supporting derivation records in this release.
    pub derivation_keys: Vec<String>,
}

/// Candidates and independent coverage for one geographic field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldLookup<T> {
    /// Distinct values in deterministic scoped-value order.
    pub candidates: Vec<Candidate<T>>,
    /// Coverage of the entire input geometry for this field.
    pub coverage: Coverage,
}

/// Geographic candidates and coverage associated with one immutable data release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LookupResult {
    /// Data release used for every field in this result.
    pub release_id: String,
    /// DXCC candidates.
    pub dxcc: FieldLookup<String>,
    /// State/province candidates.
    pub primary_subdivision: FieldLookup<String>,
    /// County/equivalent candidates.
    pub secondary_subdivision: FieldLookup<String>,
    /// CQ operating-zone candidates.
    pub cq_zone: FieldLookup<CqZone>,
    /// ITU operating-zone candidates.
    pub itu_zone: FieldLookup<ItuZone>,
}

/// Validated native query geometry with its indexed search rectangles.
enum Query {
    /// Point, including equivalent longitude representations at seams/poles.
    Point(Point<f64>),
    /// Full native locator footprint, without microdegree rounding.
    Area(Polygon<f64>),
}

impl Bundle {
    /// Query a recorded point or the full area of a grid-derived position.
    ///
    /// One candidate with complete coverage is eligible for autofill. Multiple
    /// candidates or partial coverage require operator judgment. Invalid bounds,
    /// unreadable artifacts or a lost numerical decision return errors, never
    /// successful partial results. This method does not update native snapshots.
    #[tracing::instrument(skip_all,name="geography.lookup",level="debug",fields(release_id = %self.manifest.release_id))]
    pub fn lookup(&mut self, geometry: LookupGeometry) -> Result<LookupResult, GeographyError> {
        let started = Instant::now();
        let query_kind = match geometry {
            LookupGeometry::Point(_) => "point",
            LookupGeometry::Area(_) => "area",
        };
        let result = self.lookup_fields(geometry);
        metrics::histogram!(LOOKUP_DURATION_METRIC,"query_kind"=>query_kind,"outcome"=>if result.is_ok(){"ok"}else{"error"}).record(started.elapsed().as_secs_f64());
        if let Ok(result) = &result {
            for (field, coverage, count) in [
                ("dxcc", result.dxcc.coverage, result.dxcc.candidates.len()),
                (
                    "primary_subdivision",
                    result.primary_subdivision.coverage,
                    result.primary_subdivision.candidates.len(),
                ),
                (
                    "secondary_subdivision",
                    result.secondary_subdivision.coverage,
                    result.secondary_subdivision.candidates.len(),
                ),
                (
                    "cq_zone",
                    result.cq_zone.coverage,
                    result.cq_zone.candidates.len(),
                ),
                (
                    "itu_zone",
                    result.itu_zone.coverage,
                    result.itu_zone.candidates.len(),
                ),
            ] {
                let coverage = match coverage {
                    Coverage::Complete => "complete",
                    Coverage::Partial => "partial",
                    Coverage::None => "none",
                };
                let count = match count {
                    0 => "zero",
                    1 => "one",
                    _ => "multiple",
                };
                metrics::counter!("clacks_geography_field_lookups_total","field"=>field,"coverage"=>coverage,"candidate_class"=>count).increment(1);
            }
        }
        result
    }

    /// Resolve all mandatory roles before making any field result observable.
    fn lookup_fields(&mut self, geometry: LookupGeometry) -> Result<LookupResult, GeographyError> {
        let query = Query::from_native(geometry)?;
        Ok(LookupResult {
            release_id: self.manifest.release_id.clone(),
            dxcc: self.lookup_field(Field::Dxcc, &query, Ok)?,
            primary_subdivision: self.lookup_field(Field::PrimarySubdivision, &query, Ok)?,
            secondary_subdivision: self.lookup_field(Field::SecondarySubdivision, &query, Ok)?,
            cq_zone: self.lookup_field(Field::CqZone, &query, |s| {
                CqZone::new(s.parse().map_err(|_| invalid("invalid CQ value"))?)
                    .map_err(|_| invalid("invalid CQ value"))
            })?,
            itu_zone: self.lookup_field(Field::ItuZone, &query, |s| {
                ItuZone::new(s.parse().map_err(|_| invalid("invalid ITU value"))?)
                    .map_err(|_| invalid("invalid ITU value"))
            })?,
        })
    }

    /// Read indexed candidates, retain topology and classify the requested coverage.
    fn lookup_field<T>(
        &mut self,
        field: Field,
        query: &Query,
        parse: impl Fn(String) -> Result<T, GeographyError>,
    ) -> Result<FieldLookup<T>, GeographyError> {
        let layer = self
            .manifest
            .layers
            .iter()
            .find(|l| l.field == field)
            .ok_or_else(|| invalid("missing mandatory layer"))?;
        if layer.feature_count == 0 {
            return Ok(FieldLookup {
                candidates: vec![],
                coverage: Coverage::None,
            });
        }
        let file = self
            .files
            .get_mut(&field)
            .ok_or_else(|| invalid("missing retained layer handle"))?;
        let path = self.root.join(&layer.path);
        let mut grouped: BTreeMap<(ParentScope, String), Candidate<String>> = BTreeMap::new();
        let mut parts = BTreeSet::new();
        let mut known_areas = Vec::new();
        for [west, south, east, north] in query.search_bounds() {
            file.rewind().map_err(|source| GeographyError::Read {
                path: path.clone(),
                source,
            })?;
            let mut selected = FgbReader::open(BufReader::new(&mut *file))
                .and_then(|r| r.select_bbox(west, south, east, north))
                .map_err(|source| GeographyError::Decode {
                    path: path.clone(),
                    source: source.into(),
                })?;
            while let Some(feature) = selected.next().map_err(|source| GeographyError::Decode {
                path: path.clone(),
                source: source.into(),
            })? {
                let feature = decode_feature(feature).map_err(|source| GeographyError::Decode {
                    path: path.clone(),
                    source,
                })?;
                if !parts.insert(feature.part_id.clone()) || !query.accepts(&feature.geometry) {
                    continue;
                }
                let value = self
                    .manifest
                    .values
                    .iter()
                    .find(|v| v.key == feature.value_key)
                    .ok_or_else(|| invalid("missing feature value"))?;
                let derivation = self
                    .manifest
                    .derivations
                    .iter()
                    .find(|d| d.key == feature.derivation_key)
                    .ok_or_else(|| invalid("missing feature derivation"))?;
                let candidate = grouped
                    .entry((value.parent_scope.clone(), value.recordable_value.clone()))
                    .or_insert_with(|| Candidate {
                        value: value.recordable_value.clone(),
                        parent_scope: value.parent_scope.clone(),
                        display_label: value.display_label.clone(),
                        source_identities: vec![],
                        derivation_keys: vec![],
                    });
                candidate
                    .source_identities
                    .extend(derivation.source_identities.iter().cloned());
                candidate.derivation_keys.push(derivation.key.clone());
                if let Query::Area(_) = query {
                    known_areas.push(feature.geometry);
                }
            }
        }
        let coverage = if grouped.is_empty() {
            Coverage::None
        } else {
            match query {
                Query::Point(_) => Coverage::Complete,
                Query::Area(area) => area_coverage(area, &known_areas, field)?,
            }
        };
        let candidates = grouped
            .into_values()
            .map(|mut c| {
                c.source_identities.sort();
                c.source_identities.dedup();
                c.derivation_keys.sort();
                c.derivation_keys.dedup();
                Ok(Candidate {
                    value: parse(c.value)?,
                    parent_scope: c.parent_scope,
                    display_label: c.display_label,
                    source_identities: c.source_identities,
                    derivation_keys: c.derivation_keys,
                })
            })
            .collect::<Result<_, GeographyError>>()?;
        Ok(FieldLookup {
            candidates,
            coverage,
        })
    }
}

/// Classify the union of known areas on a shared high-resolution integer lattice.
fn area_coverage(
    area: &Polygon<f64>,
    known: &[MultiPolygon<f64>],
    field: Field,
) -> Result<Coverage, GeographyError> {
    use clipper2_rust::{
        core::{FillRule, Paths64},
        engine::ClipType,
        engine_public::Clipper64,
    };
    if known
        .iter()
        .any(|geometry| geometry.relate(area).is_covers())
    {
        return Ok(Coverage::Complete);
    }
    let window = area
        .bounding_rect()
        .ok_or_else(|| invalid("empty query bounds"))?;
    use geo::CoordsIter;
    let extent = std::iter::once(area)
        .chain(known.iter().flat_map(|geometry| &geometry.0))
        .flat_map(|polygon| polygon.coords_iter())
        .map(|point| {
            (point.x - window.min().x)
                .abs()
                .max((point.y - window.min().y).abs())
        })
        .fold(0., f64::max);
    // A power-of-two scale preserves representable binary coordinates where the
    // engine's safe coordinate range permits it, rather than introducing decimal rounding.
    let scale = 2_f64
        .powf((1e15 / extent).log2().floor())
        .min(MAX_COVERAGE_SCALE);
    let mut points = BTreeMap::new();
    let subject = coverage_paths(
        &MultiPolygon(vec![area.clone()]),
        window,
        scale,
        &mut points,
        field,
    )?;
    let mut clips = Vec::new();
    for geometry in known {
        clips.extend(coverage_paths(geometry, window, scale, &mut points, field)?);
    }
    use geo::algorithm::{line_intersection::LineIntersection, sweep::Intersections};
    let edges = std::iter::once(area)
        .chain(known.iter().flat_map(|geometry| &geometry.0))
        .flat_map(|polygon| std::iter::once(polygon.exterior()).chain(polygon.interiors()))
        .flat_map(|ring| ring.lines())
        .filter(|line| line.bounding_rect().intersects(&window));
    for (_, _, intersection) in Intersections::from_iter(edges) {
        match intersection {
            LineIntersection::SinglePoint { intersection, .. } => {
                coverage_node(intersection, window, scale, &mut points, field)?;
            }
            LineIntersection::Collinear { intersection } => {
                coverage_node(intersection.start, window, scale, &mut points, field)?;
                coverage_node(intersection.end, window, scale, &mut points, field)?;
            }
        }
    }
    let mut clipper = Clipper64::new();
    clipper.set_preserve_collinear(true);
    clipper.add_subject(&subject);
    clipper.add_clip(&clips);
    let mut uncovered = Paths64::new();
    if !clipper.execute(
        ClipType::Difference,
        FillRule::NonZero,
        &mut uncovered,
        None,
    ) {
        return Err(GeographyError::NumericalFailure {
            message: format!(
                "{field:?}: coverage clipping failed ({})",
                clipper.error_code()
            ),
        });
    }
    Ok(if uncovered.is_empty() {
        Coverage::Complete
    } else {
        Coverage::Partial
    })
}

/// Maximum binary lattice scale (2^48 units/degree), bounded further by query/source extent.
const MAX_COVERAGE_SCALE: f64 = 281_474_976_710_656.;

/// Preserve winding while rejecting distinct vertices that collapse onto one lattice point.
fn coverage_paths(
    geometry: &MultiPolygon<f64>,
    window: Rect<f64>,
    scale: f64,
    points: &mut BTreeMap<(i64, i64), Coord<f64>>,
    field: Field,
) -> Result<clipper2_rust::core::Paths64, GeographyError> {
    let oriented = geometry.orient(Direction::Default);
    let mut paths = Vec::new();
    for polygon in &oriented.0 {
        for ring in std::iter::once(polygon.exterior()).chain(polygon.interiors()) {
            let mut path = Vec::with_capacity(ring.0.len());
            for point in &ring.0 {
                path.push(coverage_node(*point, window, scale, points, field)?);
            }
            // Separated vertices may still become collinear after rounding. Preserve
            // every local turn, including the closure, rather than erase a thin notch.
            use geo::kernels::{Kernel, Orientation, RobustKernel};
            let count = ring.0.len().saturating_sub(1);
            let mut nodes = ring.0.iter().zip(&path).take(count).cycle();
            let mut previous = nodes.next().ok_or_else(|| invalid("empty coverage ring"))?;
            let mut current = nodes.next().ok_or_else(|| invalid("empty coverage ring"))?;
            for next in nodes.take(count) {
                let original = RobustKernel::orient2d(*previous.0, *current.0, *next.0);
                let abx = i128::from(current.1.x) - i128::from(previous.1.x);
                let aby = i128::from(current.1.y) - i128::from(previous.1.y);
                let acx = i128::from(next.1.x) - i128::from(previous.1.x);
                let acy = i128::from(next.1.y) - i128::from(previous.1.y);
                // coverage_node bounds each coordinate to 1e15, well inside i128 products.
                let converted = match (abx * acy - aby * acx).cmp(&0) {
                    std::cmp::Ordering::Less => Orientation::Clockwise,
                    std::cmp::Ordering::Equal => Orientation::Collinear,
                    std::cmp::Ordering::Greater => Orientation::CounterClockwise,
                };
                if original != converted && window.intersects(&Point(*current.0)) {
                    return Err(GeographyError::NumericalFailure {
                        message: format!(
                            "{field:?}: boundary turn unresolved at coverage precision"
                        ),
                    });
                }
                previous = current;
                current = next;
            }
            paths.push(path);
        }
    }
    Ok(paths)
}

/// Reject vertices or intersection nodes too close for the clipping engine's integer resolution.
fn coverage_node(
    point: Coord<f64>,
    window: Rect<f64>,
    scale: f64,
    points: &mut BTreeMap<(i64, i64), Coord<f64>>,
    field: Field,
) -> Result<clipper2_rust::core::Point64, GeographyError> {
    let x = ((point.x - window.min().x) * scale).round();
    let y = ((point.y - window.min().y) * scale).round();
    if !x.is_finite() || !y.is_finite() || x.abs() > 1e15 || y.abs() > 1e15 {
        return Err(GeographyError::NumericalFailure {
            message: format!("{field:?}: coverage coordinate range exceeded"),
        });
    }
    let key = (x as i64, y as i64);
    if !window.intersects(&Point(point)) {
        return Ok(clipper2_rust::core::Point64::new(key.0, key.1));
    }
    // Clipper64 can discard a triangle containing an edge shorter than two integer units.
    // Fail on unresolved nodes instead of allowing that engine limit to erase positive area.
    for dx in [-1, 0, 1] {
        for dy in [-1, 0, 1] {
            if let Some(previous) = points.get(&(key.0 + dx, key.1 + dy))
                && *previous != point
            {
                return Err(GeographyError::NumericalFailure {
                    message: format!(
                        "{field:?}: distinct boundary nodes unresolved at coverage precision"
                    ),
                });
            }
        }
    }
    points.insert(key, point);
    Ok(clipper2_rust::core::Point64::new(key.0, key.1))
}

impl Query {
    /// Validate public native bounds without discarding their exact area meaning.
    fn from_native(geometry: LookupGeometry) -> Result<Self, GeographyError> {
        match geometry {
            LookupGeometry::Point(p) => Ok(Self::Point(Point::new(
                f64::from(p.longitude_microdegrees()) / 1e6,
                f64::from(p.latitude_microdegrees()) / 1e6,
            ))),
            LookupGeometry::Area(b) => {
                let west = degree(b.west)?;
                let south = degree(b.south)?;
                let east = degree(b.east)?;
                let north = degree(b.north)?;
                if west >= east
                    || south >= north
                    || west < -180.
                    || east > 180.
                    || south < -90.
                    || north > 90.
                {
                    return Err(GeographyError::InvalidQuery {
                        message: "inverted or out-of-domain area".into(),
                    });
                }
                Ok(Self::Area(
                    Rect::new(Coord { x: west, y: south }, Coord { x: east, y: north })
                        .to_polygon(),
                ))
            }
        }
    }

    /// Return search windows, including both date-line endpoints and all pole sectors.
    fn search_bounds(&self) -> Vec<[f64; 4]> {
        use geo::BoundingRect;
        match self {
            Self::Point(p) if p.y().abs() == 90. => vec![[-180., p.y(), 180., p.y()]],
            Self::Point(p) if p.x().abs() == 180. => {
                vec![[-180., p.y(), -180., p.y()], [180., p.y(), 180., p.y()]]
            }
            Self::Point(p) => vec![[p.x(), p.y(), p.x(), p.y()]],
            Self::Area(p) => p
                .bounding_rect()
                .map(|r| vec![[r.min().x, r.min().y, r.max().x, r.max().y]])
                .unwrap_or_default(),
        }
    }

    /// Apply closure membership to points and positive interior intersection to areas.
    fn accepts(&self, geometry: &MultiPolygon<f64>) -> bool {
        match self {
            Self::Point(point) if point.y().abs() == 90. => {
                use geo::CoordsIter;
                geometry.coords_iter().any(|p| p.y == point.y())
            }
            Self::Point(point) if point.x().abs() == 180. => {
                geometry.relate(point).is_covers()
                    || geometry
                        .relate(&Point::new(-point.x(), point.y()))
                        .is_covers()
            }
            Self::Point(point) => geometry.relate(point).is_covers(),
            Self::Area(area) => {
                geometry
                    .relate(area)
                    .get(CoordPos::Inside, CoordPos::Inside)
                    == Dimensions::TwoDimensional
            }
        }
    }
}

/// Convert an exact native rational without panics or premature coordinate rounding.
fn degree(value: RationalDegree) -> Result<f64, GeographyError> {
    if value.denominator == 0 {
        return Err(GeographyError::InvalidQuery {
            message: "zero bound denominator".into(),
        });
    }
    let result = value.numerator as f64 / value.denominator as f64;
    if !result.is_finite() {
        return Err(GeographyError::InvalidQuery {
            message: "non-finite bound".into(),
        });
    }
    Ok(result)
}

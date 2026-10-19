//! Maidenhead locator areas, explicit geographic points and exact grid geometry.

use crate::ValueError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Number of subdivisions per longitude/latitude pair at each supported locator level.
const RADICES: [i64; 6] = [18, 10, 24, 10, 24, 10];

/// A supported Maidenhead locator length: 2, 4, 6, 8, 10 or 12 characters.
///
/// Two-character precision represents a coarse historical area. For own-station
/// entry validation, see [`Position::validate_operating_entry`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct GridLength(u8);

impl GridLength {
    /// Validate a locator’s retained character count.
    ///
    /// Returns `ValueError::InvalidGridLength` for unsupported lengths. This does
    /// not pad a locator or impose the own-station entry minimum.
    pub fn new(length: u8) -> Result<Self, ValueError> {
        if [2, 4, 6, 8, 10, 12].contains(&length) {
            Ok(Self(length))
        } else {
            Err(ValueError::InvalidGridLength { length })
        }
    }

    /// Return the number of characters to retain when displaying a locator.
    pub fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for GridLength {
    type Error = ValueError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<GridLength> for u8 {
    fn from(value: GridLength) -> Self {
        value.0
    }
}

/// A validated uppercase Maidenhead locator retaining its supplied precision.
///
/// It denotes an area, not an explicit station coordinate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Locator(String);

impl Locator {
    /// Parse a two- through twelve-character Maidenhead locator, accepting ASCII case differences.
    ///
    /// The supplied length is retained. Invalid length, pair alphabet, or non-ASCII
    /// content returns `ValueError::InvalidLocator`; surrounding whitespace is not trimmed.
    pub fn parse(value: &str) -> Result<Self, ValueError> {
        let value = value.to_ascii_uppercase();
        let invalid = || ValueError::InvalidLocator {
            value: value.clone(),
        };
        GridLength::new(value.len().try_into().map_err(|_| invalid())?).map_err(|_| invalid())?;
        let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
        if !remainder.is_empty() {
            return Err(invalid());
        }
        for (i, ([longitude, latitude], radix)) in pairs.iter().zip(RADICES).enumerate() {
            let base = if i % 2 == 0 { b'A' } else { b'0' };
            for digit in [*longitude, *latitude] {
                if digit < base || i64::from(digit - base) >= radix {
                    return Err(invalid());
                }
            }
        }
        Ok(Self(value))
    }

    /// Return the locator’s original precision for display and derived-coordinate conversion.
    pub fn length(&self) -> GridLength {
        GridLength(self.0.len() as u8)
    }

    /// Return longitude/latitude cell indices and subdivisions per axis for exact grid geometry.
    fn indices(&self) -> (i64, i64, i64) {
        let (mut lon, mut lat, mut count) = (0, 0, 1);
        let (pairs, _) = self.0.as_bytes().as_chunks::<2>();
        for (i, ([longitude, latitude], radix)) in pairs.iter().zip(RADICES).enumerate() {
            let base = if i % 2 == 0 { b'A' } else { b'0' };
            lon = lon * radix + i64::from(*longitude - base);
            lat = lat * radix + i64::from(*latitude - base);
            count *= radix;
        }
        (lon, lat, count)
    }

    /// Return the entire grid cell as exact rational degree boundaries.
    ///
    /// Use these bounds when the full area, rather than a representative point,
    /// is the geographic evidence.
    pub fn bounds(&self) -> GridBounds {
        let (lon, lat, count) = self.indices();
        let degree = |index: i64, span: i64, origin: i64| RationalDegree {
            numerator: index * span - origin * count,
            denominator: count as u64,
        };
        GridBounds {
            south: degree(lat, 180, 90),
            north: degree(lat + 1, 180, 90),
            west: degree(lon, 360, 180),
            east: degree(lon + 1, 360, 180),
        }
    }

    /// Return the grid center rounded to integer millionths of a degree.
    ///
    /// Use as a representative display coordinate. It does not turn this area
    /// into evidence that the station occupied that particular point.
    pub fn center(&self) -> Coordinates {
        let (lon, lat, count) = self.indices();
        let midpoint = |index: i64, span: i64, origin: i64| {
            round_ratio(
                ((2 * index + 1) * span - 2 * origin * count) * 1_000_000,
                2 * count,
            ) as i32
        };
        Coordinates {
            latitude: midpoint(lat, 180, 90),
            longitude: midpoint(lon, 360, 180),
        }
    }
}

impl fmt::Display for Locator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl TryFrom<String> for Locator {
    type Error = ValueError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<Locator> for String {
    fn from(value: Locator) -> Self {
        value.0
    }
}

/// A degree value expressed as an exact numerator and denominator.
///
/// Grid calculations produce positive denominators to avoid premature coordinate rounding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RationalDegree {
    /// Signed degree numerator.
    pub numerator: i64,
    /// Degree denominator; grid-generated values are always positive.
    pub denominator: u64,
}

/// Geographic edges of a Maidenhead cell, retaining exact angular boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridBounds {
    /// Southern edge in degrees.
    pub south: RationalDegree,
    /// Northern edge in degrees.
    pub north: RationalDegree,
    /// Western edge in degrees.
    pub west: RationalDegree,
    /// Eastern edge in degrees.
    pub east: RationalDegree,
}

/// A geographic point in signed integer millionths of a degree.
///
/// Latitude is within ±90 degrees and longitude within ±180 degrees, inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "CoordinateFields", into = "CoordinateFields")]
pub struct Coordinates {
    /// Latitude in signed millionths of a degree.
    latitude: i32,
    /// Longitude in signed millionths of a degree, retaining the supplied date-line endpoint.
    longitude: i32,
}

impl Coordinates {
    /// Validate an explicit point supplied in millionths of a degree.
    ///
    /// Use when the source already has native integer resolution. Out-of-range
    /// latitude or longitude returns `ValueError::InvalidCoordinate`.
    pub fn from_microdegrees(latitude: i32, longitude: i32) -> Result<Self, ValueError> {
        for (axis, value, max) in [
            ("latitude", latitude, 90_000_000),
            ("longitude", longitude, 180_000_000),
        ] {
            if !(-max..=max).contains(&value) {
                return Err(ValueError::InvalidCoordinate {
                    axis: axis.into(),
                    value: value.to_string(),
                });
            }
        }
        Ok(Self {
            latitude,
            longitude,
        })
    }

    /// Parse decimal-degree latitude and longitude into native coordinate resolution.
    ///
    /// Validate the supplied ranges before rounding; round exact halfway values
    /// away from zero. Reject exponent notation, nonfinite values and malformed text.
    /// Use for explicit input, not to promote a grid-derived center into a point.
    pub fn from_decimal_degrees(latitude: &str, longitude: &str) -> Result<Self, ValueError> {
        Self::from_microdegrees(
            decimal(latitude, 90, "latitude")?,
            decimal(longitude, 180, "longitude")?,
        )
    }

    /// Return signed latitude in millionths of a degree for storage or formatting.
    pub fn latitude_microdegrees(self) -> i32 {
        self.latitude
    }

    /// Return signed longitude in millionths of a degree, retaining either date-line endpoint.
    pub fn longitude_microdegrees(self) -> i32 {
        self.longitude
    }

    /// Derive the containing grid cell at the requested supported precision.
    ///
    /// Use when displaying an explicit point. Positive 180° longitude selects the
    /// same cell as negative 180°; the north pole selects the final latitude cell.
    /// The stored point is not changed.
    pub fn locator(self, length: GridLength) -> Locator {
        let pair_count = usize::from(length.0 / 2);
        let count: i64 = RADICES.into_iter().take(pair_count).product();
        let lon = if self.longitude == 180_000_000 {
            -180_000_000
        } else {
            self.longitude
        };
        let mut x = (i64::from(lon) + 180_000_000) * count / 360_000_000;
        let mut y = ((i64::from(self.latitude) + 90_000_000) * count / 180_000_000).min(count - 1);
        let mut pairs = Vec::with_capacity(pair_count);
        for (i, radix) in RADICES.into_iter().take(pair_count).enumerate().rev() {
            let base = if i % 2 == 0 { b'A' } else { b'0' };
            pairs.push([
                char::from(base + (x % radix) as u8),
                char::from(base + (y % radix) as u8),
            ]);
            x /= radix;
            y /= radix;
        }
        Locator(pairs.into_iter().rev().flatten().collect())
    }
}

/// Serialized explicit-point components subject to native range validation.
#[derive(Serialize, Deserialize)]
struct CoordinateFields {
    /// Latitude in signed millionths of a degree.
    latitude: i32,
    /// Longitude in signed millionths of a degree, retaining the supplied date-line endpoint.
    longitude: i32,
}

impl TryFrom<CoordinateFields> for Coordinates {
    type Error = ValueError;

    fn try_from(v: CoordinateFields) -> Result<Self, Self::Error> {
        Self::from_microdegrees(v.latitude, v.longitude)
    }
}

impl From<Coordinates> for CoordinateFields {
    fn from(v: Coordinates) -> Self {
        Self {
            latitude: v.latitude,
            longitude: v.longitude,
        }
    }
}

/// Round a rational value to the nearest integer, with exact ties away from zero.
///
/// The denominator must be positive and intermediate values must fit `i64`.
fn round_ratio(n: i64, d: i64) -> i64 {
    n.signum() * ((n.abs() + d / 2) / d)
}

/// Parse one bounded geographic axis into millionths of a degree.
///
/// Reject invalid syntax or values beyond `max` degrees before rounding.
fn decimal(value: &str, max: u32, axis: &str) -> Result<i32, ValueError> {
    let malformed = || ValueError::MalformedDecimal {
        value: value.into(),
    };
    let negative = value.starts_with('-');
    let raw = value.strip_prefix(['-', '+']).unwrap_or(value);
    let mut parts = raw.split('.');
    let whole = parts.next().unwrap_or_default();
    let frac = parts.next().unwrap_or_default();
    if parts.next().is_some()
        || (whole.is_empty() && frac.is_empty())
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !frac.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(malformed());
    }
    let significant = whole.trim_start_matches('0');
    let whole_value = if significant.is_empty() {
        0
    } else {
        significant
            .parse::<u32>()
            .map_err(|_| ValueError::InvalidCoordinate {
                axis: axis.into(),
                value: value.into(),
            })?
    };
    if whole_value > max || (whole_value == max && frac.bytes().any(|b| b != b'0')) {
        return Err(ValueError::InvalidCoordinate {
            axis: axis.into(),
            value: value.into(),
        });
    }
    let mut micro = 0;
    for i in 0..6 {
        micro = micro * 10 + u32::from(frac.as_bytes().get(i).copied().unwrap_or(b'0') - b'0');
    }
    if frac.as_bytes().get(6).is_some_and(|b| *b >= b'5') {
        micro += 1;
    }
    let magnitude = (whole_value * 1_000_000 + micro) as i32;
    Ok(if negative { -magnitude } else { magnitude })
}

/// Recorded location evidence as either a grid area or an explicit point.
///
/// The forms remain distinct even when a point equals a grid center.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Position {
    /// A recorded grid cell whose entire area remains the location evidence.
    GridArea(Locator),
    /// An explicitly supplied point with independent display-locator precision.
    ExplicitPoint {
        /// Authoritative recorded coordinates, never replaced by the displayed grid center.
        coordinates: Coordinates,
        /// Retained locator precision for deriving a display grid; not point uncertainty.
        grid_length: GridLength,
    },
}

impl Position {
    /// Return an explicit point or a grid area’s representative center for display.
    ///
    /// For geographic queries, see [`Self::lookup_geometry`].
    pub fn display_coordinates(&self) -> Coordinates {
        match self {
            Self::GridArea(g) => g.center(),
            Self::ExplicitPoint { coordinates, .. } => *coordinates,
        }
    }

    /// Return the recorded area locator or derive a locator for an explicit point.
    ///
    /// Preserves the stored locator length without changing the position’s meaning.
    pub fn display_locator(&self) -> Locator {
        match self {
            Self::GridArea(g) => g.clone(),
            Self::ExplicitPoint {
                coordinates,
                grid_length,
            } => coordinates.locator(*grid_length),
        }
    }

    /// Check the four-character minimum for own-station operating-location entry.
    ///
    /// Apply this check to newly entered own-station locations, not to
    /// historical imports or contacted-station data.
    pub fn validate_operating_entry(&self) -> Result<(), ValueError> {
        let length = self.display_locator().length().get();
        if length < 4 {
            Err(ValueError::InvalidGridLength { length })
        } else {
            Ok(())
        }
    }

    /// Select an area or point query without treating grid-derived coordinates as explicit.
    ///
    /// Geographic adapters should use this instead of display coordinates.
    pub fn lookup_geometry(&self) -> LookupGeometry {
        match self {
            Self::GridArea(g) => LookupGeometry::Area(g.bounds()),
            Self::ExplicitPoint { coordinates, .. } => LookupGeometry::Point(*coordinates),
        }
    }

    /// Test whether geographic evidence is unchanged for retaining manual corrections.
    ///
    /// Explicit points ignore display-grid length; grid areas require the same
    /// locator and precision. A grid area never equals an explicit point here.
    pub fn same_geographic_position(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::GridArea(a), Self::GridArea(b)) => a == b,
            (
                Self::ExplicitPoint { coordinates: a, .. },
                Self::ExplicitPoint { coordinates: b, .. },
            ) => a == b,
            _ => false,
        }
    }
}

/// Geometry that a geographic adapter should query without losing area/point meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookupGeometry {
    /// Query the full locator footprint and retain ambiguous results.
    Area(GridBounds),
    /// Query the explicitly recorded geographic point.
    Point(Coordinates),
}

use crate::ValueError;
use serde::{Deserialize, Serialize};
use std::fmt;

const RADICES: [i64; 6] = [18, 10, 24, 10, 24, 10];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct GridLength(u8);

impl GridLength {
    pub fn new(length: u8) -> Result<Self, ValueError> {
        if [2, 4, 6, 8, 10, 12].contains(&length) {
            Ok(Self(length))
        } else {
            Err(ValueError::InvalidGridLength { length })
        }
    }

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Locator(String);

impl Locator {
    pub fn parse(value: &str) -> Result<Self, ValueError> {
        let value = value.to_ascii_uppercase();
        let invalid = || ValueError::InvalidLocator {
            value: value.clone(),
        };
        GridLength::new(value.len().try_into().map_err(|_| invalid())?).map_err(|_| invalid())?;
        for (i, b) in value.bytes().enumerate() {
            let radix = RADICES[i / 2];
            let base = if i / 2 % 2 == 0 { b'A' } else { b'0' };
            if b < base || i64::from(b - base) >= radix {
                return Err(invalid());
            }
        }
        Ok(Self(value))
    }

    pub fn length(&self) -> GridLength {
        GridLength(self.0.len() as u8)
    }

    fn indices(&self) -> (i64, i64, i64) {
        let (mut lon, mut lat, mut count) = (0, 0, 1);
        for (i, pair) in self.0.as_bytes().chunks_exact(2).enumerate() {
            let base = if i % 2 == 0 { b'A' } else { b'0' };
            lon = lon * RADICES[i] + i64::from(pair[0] - base);
            lat = lat * RADICES[i] + i64::from(pair[1] - base);
            count *= RADICES[i];
        }
        (lon, lat, count)
    }

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RationalDegree {
    pub numerator: i64,
    pub denominator: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridBounds {
    pub south: RationalDegree,
    pub north: RationalDegree,
    pub west: RationalDegree,
    pub east: RationalDegree,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "CoordinateFields", into = "CoordinateFields")]
pub struct Coordinates {
    latitude: i32,
    longitude: i32,
}

impl Coordinates {
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

    pub fn from_decimal_degrees(latitude: &str, longitude: &str) -> Result<Self, ValueError> {
        Self::from_microdegrees(
            decimal(latitude, 90, "latitude")?,
            decimal(longitude, 180, "longitude")?,
        )
    }

    pub fn latitude_microdegrees(self) -> i32 {
        self.latitude
    }

    pub fn longitude_microdegrees(self) -> i32 {
        self.longitude
    }

    pub fn locator(self, length: GridLength) -> Locator {
        let count: i64 = RADICES[..usize::from(length.0 / 2)].iter().product();
        let lon = if self.longitude == 180_000_000 {
            -180_000_000
        } else {
            self.longitude
        };
        let mut x = (i64::from(lon) + 180_000_000) * count / 360_000_000;
        let mut y = ((i64::from(self.latitude) + 90_000_000) * count / 180_000_000).min(count - 1);
        let mut bytes = vec![0; usize::from(length.0)];
        for i in (0..usize::from(length.0 / 2)).rev() {
            let radix = RADICES[i];
            let base = if i % 2 == 0 { b'A' } else { b'0' };
            bytes[2 * i] = base + (x % radix) as u8;
            bytes[2 * i + 1] = base + (y % radix) as u8;
            x /= radix;
            y /= radix;
        }
        Locator(String::from_utf8(bytes).expect("locator alphabet is ASCII"))
    }
}

#[derive(Serialize, Deserialize)]
struct CoordinateFields {
    latitude: i32,
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

fn round_ratio(n: i64, d: i64) -> i64 {
    n.signum() * ((n.abs() + d / 2) / d)
}

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Position {
    GridArea(Locator),
    ExplicitPoint {
        coordinates: Coordinates,
        grid_length: GridLength,
    },
}

impl Position {
    pub fn display_coordinates(&self) -> Coordinates {
        match self {
            Self::GridArea(g) => g.center(),
            Self::ExplicitPoint { coordinates, .. } => *coordinates,
        }
    }

    pub fn display_locator(&self) -> Locator {
        match self {
            Self::GridArea(g) => g.clone(),
            Self::ExplicitPoint {
                coordinates,
                grid_length,
            } => coordinates.locator(*grid_length),
        }
    }

    pub fn validate_operating_entry(&self) -> Result<(), ValueError> {
        let length = self.display_locator().length().get();
        if length < 4 {
            Err(ValueError::InvalidGridLength { length })
        } else {
            Ok(())
        }
    }

    pub fn lookup_geometry(&self) -> LookupGeometry {
        match self {
            Self::GridArea(g) => LookupGeometry::Area(g.bounds()),
            Self::ExplicitPoint { coordinates, .. } => LookupGeometry::Point(*coordinates),
        }
    }

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookupGeometry {
    Area(GridBounds),
    Point(Coordinates),
}

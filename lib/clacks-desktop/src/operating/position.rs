//! Editable grid-area and explicit-point input with native coordinate conversion.

use clacks_model::position::{Coordinates, GridLength, Locator, Position};

/// Which user input supplies geographic evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PositionKind {
    /// The locator area is authoritative; coordinates show its center.
    Grid,
    /// Entered coordinates are authoritative; the locator is derived.
    Coordinates,
}

/// Raw position text, including incomplete input that is not yet a native value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PositionForm {
    /// User-selected authoritative representation.
    pub kind: PositionKind,
    /// Entered or derived locator text.
    pub grid: String,
    /// Entered or representative latitude in decimal degrees.
    pub latitude: String,
    /// Entered or representative longitude in decimal degrees.
    pub longitude: String,
    /// Precision used when deriving a locator from explicit coordinates.
    pub length: GridLength,
}

impl PositionForm {
    /// Populate a form from recorded evidence without changing its meaning.
    pub fn from_position(position: Option<&Position>) -> Self {
        let Some(position) = position else {
            return Self {
                kind: PositionKind::Grid,
                grid: String::new(),
                latitude: String::new(),
                longitude: String::new(),
                length: GridLength::default(),
            };
        };
        let locator = position.display_locator();
        let coordinates = position.display_coordinates();
        Self {
            kind: if matches!(position, Position::GridArea(_)) {
                PositionKind::Grid
            } else {
                PositionKind::Coordinates
            },
            grid: locator.to_string(),
            latitude: microdegrees(coordinates.latitude_microdegrees()),
            longitude: microdegrees(coordinates.longitude_microdegrees()),
            length: locator.length(),
        }
    }

    /// Accept locator text and update its representative coordinates only when decodable.
    pub fn edit_grid(&mut self, value: String) {
        self.kind = PositionKind::Grid;
        self.grid = value;
        if self.grid.trim().is_empty() {
            self.latitude.clear();
            self.longitude.clear();
        } else if let Ok(locator) = Locator::parse(self.grid.trim()) {
            if locator.length().get() >= 4 {
                self.length = locator.length();
            }
            let coordinates = locator.center();
            self.latitude = microdegrees(coordinates.latitude_microdegrees());
            self.longitude = microdegrees(coordinates.longitude_microdegrees());
        }
    }

    /// Accept explicit coordinate text while preserving partial invalid edits.
    pub fn edit_coordinates(&mut self, latitude: String, longitude: String) {
        self.kind = PositionKind::Coordinates;
        self.latitude = latitude;
        self.longitude = longitude;
        if self.latitude.trim().is_empty() && self.longitude.trim().is_empty() {
            self.grid.clear();
        } else if let Ok(coordinates) =
            Coordinates::from_decimal_degrees(self.latitude.trim(), self.longitude.trim())
        {
            self.grid = coordinates.locator(self.length).to_string();
        }
    }

    /// Change an explicit point's display precision without changing its coordinates.
    pub fn edit_precision(&mut self, length: GridLength) {
        self.length = length;
        if self.kind == PositionKind::Coordinates {
            self.edit_coordinates(self.latitude.clone(), self.longitude.clone());
        }
    }

    /// Validate own-station position input, retaining the area/point distinction.
    pub fn build(&self) -> Result<Option<Position>, String> {
        let position = self.build_recorded()?;
        if let Some(value) = &position {
            value
                .validate_operating_entry()
                .map_err(|e| e.to_string())?;
        }
        Ok(position)
    }

    /// Read recorded native positions without imposing own operating-entry precision.
    pub fn build_recorded(&self) -> Result<Option<Position>, String> {
        let position = match self.kind {
            PositionKind::Grid if self.grid.trim().is_empty() => return Ok(None),
            PositionKind::Grid => {
                Position::GridArea(Locator::parse(self.grid.trim()).map_err(|e| e.to_string())?)
            }
            PositionKind::Coordinates
                if self.latitude.trim().is_empty() && self.longitude.trim().is_empty() =>
            {
                return Ok(None);
            }
            PositionKind::Coordinates => Position::ExplicitPoint {
                coordinates: Coordinates::from_decimal_degrees(
                    self.latitude.trim(),
                    self.longitude.trim(),
                )
                .map_err(|e| e.to_string())?,
                grid_length: self.length,
            },
        };
        Ok(Some(position))
    }
}

/// Format native integer millionths as decimal degrees without floating point.
fn microdegrees(value: i32) -> String {
    let magnitude = value.unsigned_abs();
    let sign = if value < 0 { "-" } else { "" };
    format!(
        "{sign}{}.{:06}",
        magnitude / 1_000_000,
        magnitude % 1_000_000
    )
}

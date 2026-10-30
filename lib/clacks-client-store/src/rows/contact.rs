//! Relational contact rows and field groups shared within the client store.

use clacks_model::position::Position;

/// Nullable columns retaining a position's evidence form.
pub(crate) struct PositionFields {
    /// Grid-area or explicit-point discriminator.
    pub(crate) kind: Option<String>,
    /// Entire supplied grid area.
    pub(crate) locator: Option<String>,
    /// Explicit latitude in millionths of a degree.
    pub(crate) latitude: Option<i64>,
    /// Explicit longitude in millionths of a degree.
    pub(crate) longitude: Option<i64>,
    /// Display-grid precision for an explicit point.
    pub(crate) length: Option<i64>,
}

impl From<Option<&Position>> for PositionFields {
    /// Prepare relational bindings from an optional native position.
    fn from(value: Option<&Position>) -> Self {
        match value {
            None => Self {
                kind: None,
                locator: None,
                latitude: None,
                longitude: None,
                length: None,
            },
            Some(Position::GridArea(grid)) => Self {
                kind: Some("grid".into()),
                locator: Some(grid.to_string()),
                latitude: None,
                longitude: None,
                length: None,
            },
            Some(Position::ExplicitPoint {
                coordinates,
                grid_length,
            }) => Self {
                kind: Some("point".into()),
                locator: None,
                latitude: Some(i64::from(coordinates.latitude_microdegrees())),
                longitude: Some(i64::from(coordinates.longitude_microdegrees())),
                length: Some(i64::from(grid_length.get())),
            },
        }
    }
}

/// Native working-contact pointers selected by local readers.
#[derive(sqlx::FromRow)]
pub(crate) struct WorkingContactRow {
    /// Stable contact identity.
    pub record_id: String,
    /// Relational working payload pointer.
    pub payload_id: i64,
    /// Unpublished local state.
    pub pending: bool,
}

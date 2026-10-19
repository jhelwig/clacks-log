//! Preserved band labels and optional catalog-derived interpretation.

use super::{BandRow, RecognitionCatalog};
use crate::{Hertz, ValueError};
use serde::{Deserialize, Serialize};

/// A recorded label whose identity is independent of current catalog recognition.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Band(String);

impl Band {
    /// Preserve a supplied band label without requiring recognition.
    ///
    /// Reject blank text, but otherwise keep spelling and whitespace unchanged.
    /// Use for native decoding or recorded values rather than frequency-driven edits.
    pub fn from_recorded_label(label: &str) -> Result<Self, ValueError> {
        if label.trim().is_empty() {
            return Err(ValueError::EmptyText {
                field: "band".into(),
            });
        }
        Ok(Self(label.into()))
    }

    /// Borrow the preserved band label for display or serialization.
    pub fn label(&self) -> &str {
        &self.0
    }

    /// Look up this label with ASCII case-insensitive matching in the supplied catalog.
    ///
    /// Returns `None` for unfamiliar labels. Use for current metadata; the stored
    /// label and its equality/ordering remain unchanged.
    pub fn interpret<'a>(&self, catalog: &'a RecognitionCatalog) -> Option<&'a BandRow> {
        catalog
            .bands
            .iter()
            .find(|row| row.code.eq_ignore_ascii_case(self.label()))
    }

    /// Derive the first band whose inclusive catalog range contains a frequency.
    ///
    /// Returns `None` if no row applies. Use for entry/import frequency authority,
    /// not to reinterpret and rewrite an existing saved band automatically.
    pub fn for_frequency(freq: Hertz, catalog: &RecognitionCatalog) -> Option<Self> {
        catalog
            .bands
            .iter()
            .find(|row| freq.get() >= row.lower_hz && row.upper_hz.is_none_or(|u| freq.get() <= u))
            .and_then(|row| Self::from_recorded_label(&row.code).ok())
    }
}

impl TryFrom<String> for Band {
    type Error = ValueError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_recorded_label(&value)
    }
}

impl From<Band> for String {
    fn from(value: Band) -> Self {
        value.0
    }
}

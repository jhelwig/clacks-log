use super::{BandRow, RecognitionCatalog};
use crate::{Hertz, ValueError};
use serde::{Deserialize, Serialize};

/// A recorded label whose identity is independent of current catalog recognition.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Band(String);

impl Band {
    pub fn from_recorded_label(label: &str) -> Result<Self, ValueError> {
        if label.trim().is_empty() {
            return Err(ValueError::EmptyText {
                field: "band".into(),
            });
        }
        Ok(Self(label.into()))
    }

    pub fn label(&self) -> &str {
        &self.0
    }

    pub fn interpret<'a>(&self, catalog: &'a RecognitionCatalog) -> Option<&'a BandRow> {
        catalog
            .bands
            .iter()
            .find(|row| row.code.eq_ignore_ascii_case(self.label()))
    }

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

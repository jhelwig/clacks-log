use super::{KnownBand, RecognitionCatalog};
use crate::{Hertz, ValueError};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Band {
    Known(KnownBand),
    Other(String),
}

impl Band {
    pub fn from_recorded_label(
        label: &str,
        catalog: &RecognitionCatalog,
    ) -> Result<Self, ValueError> {
        if label.trim().is_empty() {
            return Err(ValueError::EmptyText {
                field: "band".into(),
            });
        }
        Ok(catalog
            .bands
            .iter()
            .find(|r| r.code == label)
            .and_then(|_| KnownBand::from_label(label))
            .map(Self::Known)
            .unwrap_or_else(|| Self::Other(label.into())))
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Known(b) => b.label(),
            Self::Other(s) => s,
        }
    }

    pub fn interpret(&self, catalog: &RecognitionCatalog) -> Option<KnownBand> {
        catalog
            .bands
            .iter()
            .find(|r| r.code.eq_ignore_ascii_case(self.label()))
            .and_then(|r| KnownBand::from_label(&r.code))
    }

    pub fn for_frequency(freq: Hertz, catalog: &RecognitionCatalog) -> Option<Self> {
        catalog
            .bands
            .iter()
            .find(|r| freq.get() >= r.lower_hz && r.upper_hz.is_none_or(|u| freq.get() <= u))
            .and_then(|r| Self::from_recorded_label(&r.code, catalog).ok())
    }
}

impl PartialEq for Band {
    fn eq(&self, other: &Self) -> bool {
        self.label() == other.label()
    }
}

impl Eq for Band {}

impl Ord for Band {
    fn cmp(&self, other: &Self) -> Ordering {
        self.label().cmp(other.label())
    }
}

impl PartialOrd for Band {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl TryFrom<String> for Band {
    type Error = ValueError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_recorded_label(&value, RecognitionCatalog::adif_318())
    }
}

impl From<Band> for String {
    fn from(value: Band) -> Self {
        value.label().into()
    }
}

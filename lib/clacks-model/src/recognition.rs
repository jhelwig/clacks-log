use crate::ValueError;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, sync::OnceLock};

mod band;
mod mode;

pub use band::*;
pub use mode::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecognitionCatalog {
    pub release: String,
    pub bands: Vec<BandRow>,
    pub modes: Vec<ModeRow>,
    pub submodes: Vec<SubmodeRow>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BandRow {
    pub code: String,
    pub lower_hz: u64,
    pub upper_hz: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeRow {
    pub code: String,
    pub deprecated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmodeRow {
    pub code: String,
    pub parent: String,
    pub deprecated: bool,
}

impl RecognitionCatalog {
    pub fn adif_318() -> &'static Self {
        static CATALOG: OnceLock<RecognitionCatalog> = OnceLock::new();
        CATALOG.get_or_init(|| {
            Self::from_json(include_str!("../data/adif-3.1.8.json"))
                .expect("validated bundled catalog")
        })
    }

    pub fn from_json(json: &str) -> Result<Self, ValueError> {
        let invalid = |reason: String| ValueError::InvalidRecognitionCatalog { reason };
        let v: Self = serde_json::from_str(json).map_err(|e| invalid(e.to_string()))?;
        for codes in [
            v.bands.iter().map(|r| r.code.as_str()).collect::<Vec<_>>(),
            v.modes.iter().map(|r| r.code.as_str()).collect(),
            v.submodes.iter().map(|r| r.code.as_str()).collect(),
        ] {
            if codes.iter().any(|s| s.trim().is_empty())
                || codes.iter().copied().collect::<BTreeSet<_>>().len() != codes.len()
            {
                return Err(invalid("blank or duplicate catalog labels".into()));
            }
        }
        if v.bands
            .iter()
            .any(|r| r.lower_hz == 0 || r.upper_hz.is_some_and(|u| u < r.lower_hz))
            || v.submodes
                .iter()
                .any(|r| !v.modes.iter().any(|m| m.code == r.parent))
        {
            return Err(invalid("invalid range or missing parent".into()));
        }
        Ok(v)
    }

    pub fn without_modes_and_submodes(&self, labels: &[&str]) -> Self {
        let mut v = self.clone();
        v.modes.retain(|r| !labels.contains(&r.code.as_str()));
        v.submodes
            .retain(|r| !labels.contains(&r.code.as_str()) && !labels.contains(&r.parent.as_str()));
        v
    }
}

//! Data-driven band and mode interpretation that preserves recorded identity.

use crate::error::ValueError;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, sync::OnceLock};

pub mod band;
pub mod mode;

/// Band/mode recognition rows supplied independently of recorded contact values.
///
/// Its public rows and derived deserialization are unconstrained storage; the
/// type itself does not enforce catalog consistency. For validated loading,
/// see [`Self::from_json`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecognitionCatalog {
    /// Source release label describing this catalog.
    pub release: String,
    /// Ordered band definitions.
    pub bands: Vec<BandRow>,
    /// Parent-mode definitions.
    pub modes: Vec<ModeRow>,
    /// Specialization rows with exact references to parent-mode labels.
    pub submodes: Vec<SubmodeRow>,
}

/// A band label and its inclusive frequency range in whole hertz.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BandRow {
    /// Band label declared by the catalog.
    pub code: String,
    /// Inclusive positive lower frequency bound in hertz.
    pub lower_hz: u64,
    /// Inclusive upper bound; `None` denotes an open-ended range.
    pub upper_hz: Option<u64>,
}

/// Recognition metadata for a parent mode label, including historical labels.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeRow {
    /// Recognized parent-mode label.
    pub code: String,
    /// Whether the source marks this label deprecated.
    pub deprecated: bool,
}

/// Recognition metadata linking a specialization label to its parent mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmodeRow {
    /// Recognized specialization label.
    pub code: String,
    /// Exact code of this submode’s parent row.
    pub parent: String,
    /// Whether the source marks this specialization deprecated.
    pub deprecated: bool,
}

impl RecognitionCatalog {
    /// Borrow the bundled Amateur Data Interchange Format (ADIF) 3.1.8 recognition baseline.
    ///
    /// Use as the initial offline catalog; it includes deprecated historical labels
    /// and performs no network acquisition. Malformed or inconsistent bundled
    /// data returns `ValueError::InvalidRecognitionCatalog`.
    pub fn adif_318() -> Result<&'static Self, ValueError> {
        /// Shared immutable copy of the bundled baseline.
        static CATALOG: OnceLock<Result<RecognitionCatalog, ValueError>> = OnceLock::new();
        CATALOG
            .get_or_init(|| Self::from_json(include_str!("../data/adif-3.1.8.json")))
            .as_ref()
            .map_err(Clone::clone)
    }

    /// Decode and validate a supplied recognition catalog before using it for interpretation.
    ///
    /// Reject malformed JSON, blank or ASCII-case-colliding labels, invalid ranges
    /// and missing exact parent references with `ValueError::InvalidRecognitionCatalog`.
    /// This does not validate provenance, prohibit overlapping ranges or rewrite labels.
    #[tracing::instrument(skip_all, name = "model.recognition.load", level = "debug")]
    pub fn from_json(json: &str) -> Result<Self, ValueError> {
        let invalid = |reason: String| ValueError::InvalidRecognitionCatalog { reason };
        let v: Self = serde_json::from_str(json).map_err(|e| invalid(e.to_string()))?;
        for codes in [
            v.bands.iter().map(|r| r.code.as_str()).collect::<Vec<_>>(),
            v.modes.iter().map(|r| r.code.as_str()).collect(),
            v.submodes.iter().map(|r| r.code.as_str()).collect(),
        ] {
            if codes.iter().any(|s| s.trim().is_empty())
                || codes
                    .iter()
                    .map(|code| code.to_ascii_lowercase())
                    .collect::<BTreeSet<_>>()
                    .len()
                    != codes.len()
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
}

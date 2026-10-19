//! Preserved mode hierarchies with catalog-derived recognition views.

use crate::error::ValueError;
use crate::recognition::{ModeRow, RecognitionCatalog, SubmodeRow};
use serde::{Deserialize, Serialize};

/// Recorded parent/specialization hierarchy and supplied labels.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ModeIdentity {
    /// A parent-level label recorded without a specialization.
    Parent {
        /// Preserved parent-mode text, whether currently recognized or not.
        label: String,
    },
    /// A specialization with optional recorded parent context.
    Specialization {
        /// Preserved specialization text.
        label: String,
        /// Optional supplied parent label; retained even if current catalog interpretation differs.
        parent: Option<String>,
    },
}

/// A nonblank recorded mode hierarchy whose identity survives recognition changes.
///
/// Serialization and equality contain only this preserved hierarchy and its
/// labels, with no catalog-derived metadata. See [`Self::from_recorded_identity`]
/// and [`Self::interpret`] for construction and recognition.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "ModeIdentity", into = "ModeIdentity")]
pub struct Mode(ModeIdentity);

/// Borrowed catalog rows comprising one recognized mode interpretation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecognizedMode<'a> {
    /// Parent-mode row belonging to this interpretation.
    pub mode: &'a ModeRow,
    /// Matched specialization row, absent for a parent-only identity.
    pub submode: Option<&'a SubmodeRow>,
}

/// Current catalog interpretation of a preserved mode identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModeInterpretation<'a> {
    /// Matching rows from the supplied recognition catalog.
    Known(RecognizedMode<'a>),
    /// Unrecognized identity, preserving all recorded labels and parent context.
    Other(ModeIdentity),
}

impl Mode {
    /// Validate nonblank labels while preserving a supplied mode identity exactly.
    ///
    /// Unknown labels and unknown parent context are valid. Returns `ValueError::EmptyText`
    /// for a blank mode or present blank parent; no catalog normalization is performed.
    pub fn from_recorded_identity(identity: ModeIdentity) -> Result<Self, ValueError> {
        let (label, parent) = match &identity {
            ModeIdentity::Parent { label } => (label, None),
            ModeIdentity::Specialization { label, parent } => (label, parent.as_ref()),
        };
        if label.trim().is_empty() || parent.is_some_and(|p| p.trim().is_empty()) {
            return Err(ValueError::EmptyText {
                field: "mode or parent context".into(),
            });
        }
        Ok(Self(identity))
    }

    /// Copy the preserved hierarchy for serialization, comparison or unrelated record edits.
    ///
    /// Use this rather than reconstructing a mode from its current interpretation.
    pub fn identity(&self) -> ModeIdentity {
        self.0.clone()
    }

    /// Borrow matching recognition rows without modifying recorded identity.
    ///
    /// Labels match ASCII case-insensitively. A recognized specialization uses its
    /// catalog parent for interpretation, even if recorded parent context differs.
    /// An unavailable match returns `Other` with the preserved identity.
    pub fn interpret<'a>(&self, catalog: &'a RecognitionCatalog) -> ModeInterpretation<'a> {
        let recognized = match &self.0 {
            ModeIdentity::Parent { label } => catalog
                .modes
                .iter()
                .find(|row| row.code.eq_ignore_ascii_case(label))
                .map(|mode| RecognizedMode {
                    mode,
                    submode: None,
                }),
            ModeIdentity::Specialization { label, .. } => catalog
                .submodes
                .iter()
                .find(|row| row.code.eq_ignore_ascii_case(label))
                .and_then(|submode| {
                    catalog
                        .modes
                        .iter()
                        .find(|row| row.code == submode.parent)
                        .map(|mode| RecognizedMode {
                            mode,
                            submode: Some(submode),
                        })
                }),
        };
        recognized
            .map(ModeInterpretation::Known)
            .unwrap_or_else(|| ModeInterpretation::Other(self.identity()))
    }
}

impl TryFrom<ModeIdentity> for Mode {
    type Error = ValueError;

    fn try_from(value: ModeIdentity) -> Result<Self, Self::Error> {
        Self::from_recorded_identity(value)
    }
}

impl From<Mode> for ModeIdentity {
    fn from(value: Mode) -> Self {
        value.0
    }
}

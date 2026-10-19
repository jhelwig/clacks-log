use super::{ModeRow, RecognitionCatalog, SubmodeRow};
use crate::ValueError;
use serde::{Deserialize, Serialize};

/// Recorded hierarchy and labels. Recognition never rewrites this identity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ModeIdentity {
    Parent {
        label: String,
    },
    Specialization {
        label: String,
        parent: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "ModeIdentity", into = "ModeIdentity")]
pub struct Mode(ModeIdentity);

/// A borrowed interpretation from the supplied catalog, never a stored mode value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecognizedMode<'a> {
    pub mode: &'a ModeRow,
    pub submode: Option<&'a SubmodeRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModeInterpretation<'a> {
    Known(RecognizedMode<'a>),
    Other(ModeIdentity),
}

impl Mode {
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

    pub fn identity(&self) -> ModeIdentity {
        self.0.clone()
    }

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

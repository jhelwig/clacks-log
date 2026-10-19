use super::{KnownMode, RecognitionCatalog};
use crate::ValueError;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OtherMode {
    Parent {
        label: String,
    },
    Specialization {
        label: String,
        parent: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "ModeIdentity", into = "ModeIdentity")]
pub enum Mode {
    Known(KnownMode),
    Other(OtherMode),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModeInterpretation {
    Known(KnownMode),
    Other(OtherMode),
}

impl OtherMode {
    fn from_identity(id: ModeIdentity) -> Self {
        match id {
            ModeIdentity::Parent { label } => Self::Parent { label },
            ModeIdentity::Specialization { label, parent } => {
                Self::Specialization { label, parent }
            }
        }
    }

    fn identity(&self) -> ModeIdentity {
        match self {
            Self::Parent { label } => ModeIdentity::Parent {
                label: label.clone(),
            },
            Self::Specialization { label, parent } => ModeIdentity::Specialization {
                label: label.clone(),
                parent: parent.clone(),
            },
        }
    }
}

impl Mode {
    pub fn from_recorded_identity(
        id: ModeIdentity,
        catalog: &RecognitionCatalog,
    ) -> Result<Self, ValueError> {
        let (label, parent) = match &id {
            ModeIdentity::Parent { label } => (label, None),
            ModeIdentity::Specialization { label, parent } => (label, parent.as_ref()),
        };
        if label.trim().is_empty() || parent.is_some_and(|p| p.trim().is_empty()) {
            return Err(ValueError::EmptyText {
                field: "mode or parent context".into(),
            });
        }
        let other = Self::Other(OtherMode::from_identity(id.clone()));
        if let ModeInterpretation::Known(k) = other.interpret(catalog)
            && k.identity() == id
        {
            return Ok(Self::Known(k));
        }
        Ok(other)
    }

    pub fn identity(&self) -> ModeIdentity {
        match self {
            Self::Known(k) => k.identity(),
            Self::Other(o) => o.identity(),
        }
    }

    pub fn interpret(&self, catalog: &RecognitionCatalog) -> ModeInterpretation {
        let id = self.identity();
        let known = match &id {
            ModeIdentity::Parent { label } => catalog
                .modes
                .iter()
                .find(|r| r.code.eq_ignore_ascii_case(label))
                .and_then(|r| KnownMode::from_parent(&r.code)),
            ModeIdentity::Specialization { label, .. } => catalog
                .submodes
                .iter()
                .find(|r| r.code.eq_ignore_ascii_case(label))
                .and_then(|r| KnownMode::from_specialization(&r.code, &r.parent)),
        };
        known
            .map(ModeInterpretation::Known)
            .unwrap_or_else(|| ModeInterpretation::Other(OtherMode::from_identity(id)))
    }
}

impl PartialEq for Mode {
    fn eq(&self, other: &Self) -> bool {
        self.identity() == other.identity()
    }
}

impl Eq for Mode {}

impl Ord for Mode {
    fn cmp(&self, other: &Self) -> Ordering {
        self.identity().cmp(&other.identity())
    }
}

impl PartialOrd for Mode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl TryFrom<ModeIdentity> for Mode {
    type Error = ValueError;

    fn try_from(value: ModeIdentity) -> Result<Self, Self::Error> {
        Self::from_recorded_identity(value, RecognitionCatalog::adif_318())
    }
}

impl From<Mode> for ModeIdentity {
    fn from(value: Mode) -> Self {
        value.identity()
    }
}

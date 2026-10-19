use crate::*;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupDefinition {
    pub id: GroupId,
    pub name: String,
    pub identifier: IdentifierDefinition,
    pub bears_location_context: bool,
    pub catalog: Option<CatalogBinding>,
    pub export_mappings: GroupExportMappings,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentifierDefinition {
    pub required: bool,
    pub normalization: Vec<TextNormalization>,
    pub syntax: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextNormalization {
    Trim,
    Uppercase,
    Lowercase,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DefinitionError {
    DuplicateGroupId {
        id: GroupId,
    },
    MissingIdentifier {
        group_id: GroupId,
    },
    InvalidIdentifier {
        group_id: GroupId,
        value: String,
    },
    InvalidDefinition {
        group_id: GroupId,
        path: String,
        reason: String,
    },
}

impl fmt::Display for DefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateGroupId { id } => write!(f, "competing definitions for group {id}"),
            Self::MissingIdentifier { group_id } => {
                write!(f, "group {group_id} requires an identifier")
            }
            Self::InvalidIdentifier { group_id, value } => {
                write!(f, "identifier {value:?} does not match group {group_id}")
            }
            Self::InvalidDefinition {
                group_id,
                path,
                reason,
            } => write!(f, "group {group_id} at {path}: {reason}"),
        }
    }
}

impl std::error::Error for DefinitionError {}

#[derive(Clone, Debug, Default)]
pub struct DefinitionSet(BTreeMap<GroupId, GroupDefinition>);

impl DefinitionSet {
    pub fn new(definitions: Vec<GroupDefinition>) -> Result<Self, DefinitionError> {
        let mut map = BTreeMap::new();
        for d in definitions {
            if let Some(previous) = map.insert(d.id, d.clone())
                && previous != d
            {
                return Err(DefinitionError::DuplicateGroupId { id: d.id });
            }
        }
        Ok(Self(map))
    }

    pub fn get(&self, id: GroupId) -> Option<&GroupDefinition> {
        self.0.get(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &GroupDefinition> {
        self.0.values()
    }
}

pub(crate) fn full_regex(pattern: &str) -> Result<regex::Regex, regex::Error> {
    regex::Regex::new(&format!(r"\A(?:{pattern})\z"))
}

pub fn normalize_reference(
    association: &ReferenceAssociation,
    definition: Option<&GroupDefinition>,
) -> Result<ReferenceAssociation, DefinitionError> {
    let Some(d) = definition else {
        return Ok(association.clone());
    };
    if d.id != association.group_id {
        return Err(DefinitionError::InvalidDefinition {
            group_id: d.id,
            path: "identifier".into(),
            reason: "definition ID does not match association".into(),
        });
    }
    let mut value = association.identifier.clone();
    for operation in &d.identifier.normalization {
        value = value.map(|v| match operation {
            TextNormalization::Trim => v.trim().into(),
            TextNormalization::Uppercase => v.to_uppercase(),
            TextNormalization::Lowercase => v.to_lowercase(),
        });
    }
    value = value.filter(|v| !v.trim().is_empty());
    if d.identifier.required && value.is_none() {
        return Err(DefinitionError::MissingIdentifier { group_id: d.id });
    }
    if let Some(pattern) = &d.identifier.syntax {
        let re = full_regex(pattern).map_err(|e| DefinitionError::InvalidDefinition {
            group_id: d.id,
            path: "identifier.syntax".into(),
            reason: e.to_string(),
        })?;
        if let Some(v) = &value
            && !re.is_match(v)
        {
            return Err(DefinitionError::InvalidIdentifier {
                group_id: d.id,
                value: v.clone(),
            });
        }
    }
    Ok(ReferenceAssociation {
        group_id: d.id,
        identifier: value,
    })
}

pub fn interpret_reference<'a>(
    association: &ReferenceAssociation,
    definitions: &'a DefinitionSet,
) -> Option<&'a GroupDefinition> {
    definitions.get(association.group_id)
}

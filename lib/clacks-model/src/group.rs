use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DefinitionError {
    #[error("competing definitions for group {id}")]
    DuplicateGroupId { id: GroupId },
    #[error("group {group_id} requires an identifier")]
    MissingIdentifier { group_id: GroupId },
    #[error("identifier {value:?} does not match group {group_id}")]
    InvalidIdentifier { group_id: GroupId, value: String },
    #[error("group {group_id} at {path}: {reason}")]
    InvalidDefinition {
        group_id: GroupId,
        path: String,
        reason: String,
    },
}

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
    match definition {
        Some(d) => compile_definition(d)?.normalize_reference(association),
        None => Ok(association.clone()),
    }
}

pub fn interpret_reference<'a>(
    association: &ReferenceAssociation,
    definitions: &'a DefinitionSet,
) -> Option<&'a GroupDefinition> {
    definitions.get(association.group_id)
}

//! Account-shared reference-group behavior and explicit identifier preparation.

use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Data-defined participation/reference behavior shared by an account.
///
/// Default and future user-defined groups use the same engine. Group names do
/// not select privileged behavior, and definition changes do not rewrite saved associations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupDefinition {
    /// Stable account group identity.
    pub id: GroupId,
    /// User-facing group name, not a discriminator for behavior.
    pub name: String,
    /// Identifier entry/edit rules, including group-only participation.
    pub identifier: IdentifierDefinition,
    /// Whether participation itself prevents falling back to a lower-priority location.
    pub bears_location_context: bool,
    /// Optional descriptive dataset binding, separate from recorded contact facts.
    pub catalog: Option<CatalogBinding>,
    /// Ordinary interchange mappings and profile-specific targeted plans.
    pub export_mappings: GroupExportMappings,
}

/// Rules for preparing an explicitly entered or edited group identifier.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentifierDefinition {
    /// Whether an association needs a nonblank identifier instead of group-only participation.
    pub required: bool,
    /// Transformations applied in order on entry or explicit edits.
    pub normalization: Vec<TextNormalization>,
    /// Optional whole-value regex checked after normalization.
    pub syntax: Option<String>,
}

/// An explicit text transformation applied in listed order during identifier preparation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextNormalization {
    /// Remove surrounding whitespace from explicit input.
    Trim,
    /// Apply Unicode uppercase conversion to explicit input.
    Uppercase,
    /// Apply Unicode lowercase conversion to explicit input.
    Lowercase,
}

/// A group definition or explicit identifier edit that cannot be accepted.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DefinitionError {
    /// Differing definitions claim one stable group identity.
    #[error("competing definitions for group {id}")]
    DuplicateGroupId {
        /// Identity shared by the conflicting definitions.
        id: GroupId,
    },
    /// An explicit association lacks a required identifier.
    #[error("group {group_id} requires an identifier")]
    MissingIdentifier {
        /// Group requiring a value.
        group_id: GroupId,
    },
    /// A prepared identifier fails its group’s whole-value syntax.
    #[error("identifier {value:?} does not match group {group_id}")]
    InvalidIdentifier {
        /// Group whose syntax was applied.
        group_id: GroupId,
        /// Rejected value after configured normalization.
        value: String,
    },
    /// A group schema, mapping or expression cannot be used.
    #[error("group {group_id} at {path}: {reason}")]
    InvalidDefinition {
        /// Group containing the invalid definition.
        group_id: GroupId,
        /// Definition location associated with the diagnostic.
        path: String,
        /// Reason the definition or evaluation is invalid.
        reason: String,
    },
}

/// Group definitions indexed by stable ID, independent of display names.
///
/// Construction rejects differing definitions for one ID but does not compile
/// their syntax or mappings; use `compile_definition` when those are needed.
#[derive(Clone, Debug, Default)]
pub struct DefinitionSet(BTreeMap<GroupId, GroupDefinition>);

impl DefinitionSet {
    /// Index account group definitions, accepting repeated identical definitions.
    ///
    /// Differing definitions with the same ID return `DefinitionError::DuplicateGroupId`.
    /// This does not seed an account or merge conflicting shared records.
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

    /// Find a group by stable identity without interpreting or rewriting associations.
    ///
    /// `None` means the definition is unavailable, not that recorded participation is invalid.
    pub fn get(&self, id: GroupId) -> Option<&GroupDefinition> {
        self.0.get(&id)
    }

    /// Borrow definitions in stable ID order for selection or account-data presentation.
    pub fn iter(&self) -> impl Iterator<Item = &GroupDefinition> {
        self.0.values()
    }
}

/// Compile a pattern that must match the entire reference value, not a substring.
pub(crate) fn full_regex(pattern: &str) -> Result<regex::Regex, regex::Error> {
    regex::Regex::new(&format!(r"\A(?:{pattern})\z"))
}

/// Prepare a new or explicitly edited association using its supplied group definition.
///
/// Apply configured text transformations, requiredness and whole-value syntax
/// validation. An unavailable definition preserves the association unchanged.
/// Do not call merely because definitions were refreshed; that would rewrite history.
pub fn normalize_reference(
    association: &ReferenceAssociation,
    definition: Option<&GroupDefinition>,
) -> Result<ReferenceAssociation, DefinitionError> {
    match definition {
        Some(d) => compile_definition(d)?.normalize_reference(association),
        None => Ok(association.clone()),
    }
}

/// Borrow the available definition for a recorded association without normalizing it.
///
/// Unknown groups return `None` and remain preserved. Use this read-only path
/// when interpreting a saved contact or checking location context.
pub fn interpret_reference<'a>(
    association: &ReferenceAssociation,
    definitions: &'a DefinitionSet,
) -> Option<&'a GroupDefinition> {
    definitions.get(association.group_id)
}

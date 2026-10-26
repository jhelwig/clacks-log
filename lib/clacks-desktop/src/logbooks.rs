//! Editable logbook fields independent of committed records and view selection.

use crate::operating::FieldError;
use clacks_model::{
    ids::TagId,
    tag::{AutomaticUploadPolicies, LogbookTag},
};

/// Unapplied name and automatic-upload policies for one stable tag identity.
pub(crate) struct TagEditor {
    /// Existing or newly allocated native identity.
    pub id: TagId,
    /// Supplied nonblank name, without uniqueness or trimming changes.
    pub name: String,
    /// Independent service policies.
    pub automatic_upload: AutomaticUploadPolicies,
}

impl TagEditor {
    /// Start a new named logbook with neutral upload policies.
    pub fn new(id: TagId) -> Self {
        Self {
            id,
            name: String::new(),
            automatic_upload: Default::default(),
        }
    }

    /// Copy a saved tag for editing without changing its identity.
    pub fn from_tag(tag: &LogbookTag) -> Self {
        Self {
            id: tag.id,
            name: tag.name.clone(),
            automatic_upload: tag.automatic_upload.clone(),
        }
    }

    /// Validate required text and return the exact desired native values.
    pub fn build(&self) -> Result<LogbookTag, Vec<FieldError>> {
        if self.name.trim().is_empty() {
            return Err(vec![FieldError {
                field: "Logbook name".into(),
                message: "Enter a logbook name.".into(),
            }]);
        }
        Ok(LogbookTag {
            id: self.id,
            name: self.name.clone(),
            automatic_upload: self.automatic_upload.clone(),
        })
    }
}

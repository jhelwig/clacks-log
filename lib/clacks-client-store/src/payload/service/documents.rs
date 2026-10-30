//! Versioned frozen service maps and field-coverage sets.

use clacks_model::service::adif::AdifFields;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// One immutable semantic field map, including an explicitly empty map.
#[derive(Serialize, Deserialize)]
pub(super) struct FieldMapDocument {
    version: u8,
    fields: AdifFields,
}

impl FieldMapDocument {
    /// Encode exact frozen values without rerunning provider normalization.
    pub(super) fn encode(fields: &AdifFields) -> anyhow::Result<String> {
        Ok(serde_json::to_string(&Self {
            version: 1,
            fields: fields.clone(),
        })?)
    }

    /// Decode a frozen map only under the supported document version.
    pub(super) fn decode(text: &str) -> anyhow::Result<AdifFields> {
        let document: Self = serde_json::from_str(text)?;
        anyhow::ensure!(
            document.version == 1,
            "unsupported service map storage version"
        );
        Ok(document.fields)
    }
}

/// A complete requested or reported field set.
#[derive(Serialize, Deserialize)]
pub(super) struct FieldSetDocument {
    version: u8,
    fields: BTreeSet<String>,
}

impl FieldSetDocument {
    /// Encode the complete set without changing its membership semantics.
    pub(super) fn encode(fields: &BTreeSet<String>) -> anyhow::Result<String> {
        Ok(serde_json::to_string(&Self {
            version: 1,
            fields: fields.clone(),
        })?)
    }

    /// Restore coverage only under the supported document version.
    pub(super) fn decode(text: &str) -> anyhow::Result<BTreeSet<String>> {
        let document: Self = serde_json::from_str(text)?;
        anyhow::ensure!(
            document.version == 1,
            "unsupported service field-set storage version"
        );
        Ok(document.fields)
    }
}

//! Descriptive reference metadata and read-only association lookup.

use crate::export::rules::{
    ExpressionContext, ExpressionValue, ValueExpression, compile_definition,
};
use crate::group::{DefinitionError, DefinitionSet};
use crate::ids::CatalogId;
use crate::reference::ReferenceAssociation;
use crate::scalar::UtcTimestamp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A descriptive JSON value, never an authoritative recorded contact field.
pub type MetadataValue = serde_json::Value;
/// A metadata attribute key for selecting descriptive catalog values.
pub type MetadataField = String;
/// Reference datasets keyed by their own `ReferenceCatalog::id`.
///
/// Callers must keep registry keys consistent with embedded identities.
pub type CatalogSet = BTreeMap<CatalogId, ReferenceCatalog>;

/// A group’s data-driven lookup and display projection for a reference dataset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogBinding {
    /// Dataset identity to find in the supplied catalog registry.
    pub dataset_id: CatalogId,
    /// Scalar key expression evaluated against this association’s identifier.
    pub lookup_key: ValueExpression,
    /// Metadata key supplying the main descriptive name.
    pub display_name: MetadataField,
    /// Additional labeled metadata fields for presentation.
    pub details: Vec<MetadataDisplayBinding>,
}

/// A user-facing label and the descriptive catalog field to display beneath it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataDisplayBinding {
    /// Display label for the metadata value.
    pub label: String,
    /// Catalog key whose value is displayed.
    pub field: MetadataField,
}

/// One descriptive dataset with provenance and reference-keyed entries.
///
/// Its entries are descriptive data, separate from recorded contact associations
/// and operating-location evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceCatalog {
    /// Stable dataset identity, independent of refresh/version information.
    pub id: CatalogId,
    /// Upstream attribution and acquisition information.
    pub provenance: SourceInformation,
    /// Descriptive entries keyed by the group binding’s derived lookup value.
    pub entries: BTreeMap<String, CatalogEntry>,
}

/// Descriptive facts about a reference, kept outside saved contact values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogEntry {
    /// Descriptive attributes such as a reference name; not on-air QSO facts.
    pub metadata: BTreeMap<String, MetadataValue>,
}

/// Dataset provenance without a separate application-local version scheme.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceInformation {
    /// Name identifying the upstream dataset.
    pub dataset_name: String,
    /// Optional publisher attribution.
    pub publisher: Option<String>,
    /// Optional acquisition/source URL, not a fetch instruction.
    pub source_url: Option<String>,
    /// Operational timestamp when this dataset copy was obtained.
    pub obtained_at: UtcTimestamp,
    /// Version supplied by the publisher, if any; no local version is invented.
    pub upstream_version: Option<String>,
}

/// A nonmutating reference lookup outcome, distinguishing unavailable data from absence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogFinding {
    /// No definition or catalog binding is available for the association.
    NotConfigured,
    /// The configured dataset is not present locally.
    Unavailable {
        /// Dataset needed for lookup.
        catalog_id: CatalogId,
    },
    /// The association yielded no lookup value.
    NoLookupKey,
    /// A usable key has no entry in the available dataset.
    NotListed {
        /// Derived key that was not found.
        key: String,
    },
    /// Descriptive metadata is available for the derived key.
    Found {
        /// Key used to select the entry.
        key: String,
        /// Copied descriptive attributes, independent of the recorded association.
        entry: CatalogEntry,
    },
}

/// Look up descriptive metadata for one recorded association using supplied local data.
///
/// Availability and listing outcomes are returned without rejecting or editing
/// the contact. Definition or key-evaluation failures propagate their concrete
/// diagnostics through `anyhow::Error`. Keys use association-local context only.
/// This operation performs no download or normalization.
pub fn lookup_reference(
    association: &ReferenceAssociation,
    definitions: &DefinitionSet,
    catalogs: &CatalogSet,
) -> anyhow::Result<CatalogFinding> {
    let Some(definition) = definitions.get(association.group_id) else {
        return Ok(CatalogFinding::NotConfigured);
    };
    let Some(binding) = &definition.catalog else {
        return Ok(CatalogFinding::NotConfigured);
    };
    let Some(catalog) = catalogs.get(&binding.dataset_id) else {
        return Ok(CatalogFinding::Unavailable {
            catalog_id: binding.dataset_id,
        });
    };
    let compiled = compile_definition(definition)?;
    let bindings = BTreeMap::new();
    let context = ExpressionContext {
        qso: None,
        current_identifier: association.identifier().as_deref(),
        bindings: &bindings,
    };
    let key = match compiled.evaluate(&binding.lookup_key, &context)? {
        ExpressionValue::Scalar(key) => key,
        ExpressionValue::Absent => return Ok(CatalogFinding::NoLookupKey),
        ExpressionValue::Collection(_) => {
            return Err(DefinitionError::InvalidDefinition {
                group_id: association.group_id,
                path: "catalog.lookup_key".into(),
                reason: "catalog key must be scalar".into(),
            }
            .into());
        }
    };
    Ok(match catalog.entries.get(&key) {
        Some(entry) => CatalogFinding::Found {
            key,
            entry: entry.clone(),
        },
        None => CatalogFinding::NotListed { key },
    })
}

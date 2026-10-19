use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type MetadataValue = serde_json::Value;
pub type MetadataField = String;
pub type CatalogSet = BTreeMap<CatalogId, ReferenceCatalog>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogBinding {
    pub dataset_id: CatalogId,
    pub lookup_key: ValueExpression,
    pub display_name: MetadataField,
    pub details: Vec<MetadataDisplayBinding>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataDisplayBinding {
    pub label: String,
    pub field: MetadataField,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceCatalog {
    pub id: CatalogId,
    pub provenance: SourceInformation,
    pub entries: BTreeMap<String, CatalogEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub metadata: BTreeMap<String, MetadataValue>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceInformation {
    pub dataset_name: String,
    pub publisher: Option<String>,
    pub source_url: Option<String>,
    pub obtained_at: UtcTimestamp,
    pub upstream_version: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogFinding {
    NotConfigured,
    Unavailable { catalog_id: CatalogId },
    NoLookupKey,
    InvalidBinding { reason: String },
    NotListed { key: String },
    Found { key: String, entry: CatalogEntry },
}

pub fn lookup_reference(
    association: &ReferenceAssociation,
    definitions: &DefinitionSet,
    catalogs: &CatalogSet,
) -> CatalogFinding {
    let Some(binding) = definitions
        .get(association.group_id)
        .and_then(|d| d.catalog.as_ref())
    else {
        return CatalogFinding::NotConfigured;
    };
    let Some(catalog) = catalogs.get(&binding.dataset_id) else {
        return CatalogFinding::Unavailable {
            catalog_id: binding.dataset_id,
        };
    };
    let definition = definitions
        .get(association.group_id)
        .expect("binding came from definition");
    let compiled = match compile_definition(definition) {
        Ok(c) => c,
        Err(e) => {
            return CatalogFinding::InvalidBinding {
                reason: e.to_string(),
            };
        }
    };
    let bindings = BTreeMap::new();
    let context = ExpressionContext {
        qso: None,
        current_identifier: association.identifier.as_deref(),
        bindings: &bindings,
    };
    let key = match compiled.evaluate(&binding.lookup_key, &context) {
        Ok(ExpressionValue::Scalar(key)) => key,
        Ok(ExpressionValue::Absent) => return CatalogFinding::NoLookupKey,
        Ok(ExpressionValue::Collection(_)) => {
            return CatalogFinding::InvalidBinding {
                reason: "catalog key must be scalar".into(),
            };
        }
        Err(e) => {
            return CatalogFinding::InvalidBinding {
                reason: e.to_string(),
            };
        }
    };
    match catalog.entries.get(&key) {
        Some(entry) => CatalogFinding::Found {
            key,
            entry: entry.clone(),
        },
        None => CatalogFinding::NotListed { key },
    }
}

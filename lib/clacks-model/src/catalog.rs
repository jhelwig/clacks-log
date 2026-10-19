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
    let key = match catalog_key(&binding.lookup_key, association.identifier.as_deref()) {
        Ok(Some(key)) => key,
        Ok(None) => return CatalogFinding::NoLookupKey,
        Err(reason) => return CatalogFinding::InvalidBinding { reason },
    };
    match catalog.entries.get(&key) {
        Some(entry) => CatalogFinding::Found {
            key,
            entry: entry.clone(),
        },
        None => CatalogFinding::NotListed { key },
    }
}

fn catalog_key(expr: &ValueExpression, identifier: Option<&str>) -> Result<Option<String>, String> {
    match expr {
        ValueExpression::Literal(v) => Ok(Some(v.clone())),
        ValueExpression::CurrentIdentifier => Ok(identifier.map(str::to_owned)),
        ValueExpression::Capture {
            value,
            pattern,
            name,
        } => {
            let re = crate::group::full_regex(pattern).map_err(|e| e.to_string())?;
            if !re.capture_names().flatten().any(|n| n == name) {
                return Err(format!("missing capture {name}"));
            }
            Ok(catalog_key(value, identifier)?.and_then(|v| {
                re.captures(&v)
                    .and_then(|c| c.name(name).map(|m| m.as_str().to_owned()))
            }))
        }
        _ => Err("catalog keys require an association-local scalar expression".into()),
    }
}

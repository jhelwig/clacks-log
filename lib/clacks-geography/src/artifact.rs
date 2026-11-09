//! Geographic bundle manifests, layer metadata and compiled polygon features.

use crate::error::{GeographyError, invalid};
use clacks_model::location::{CqZone, ItuZone};
use geo_types::MultiPolygon;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path},
};

pub mod read;
pub mod validate;
pub mod write;

/// A separately resolved geographic field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    /// DX Century Club entity.
    Dxcc,
    /// State, province or equivalent primary subdivision.
    PrimarySubdivision,
    /// County or equivalent secondary subdivision.
    SecondarySubdivision,
    /// CQ operating zone.
    CqZone,
    /// International Telecommunication Union operating zone.
    ItuZone,
}

impl Field {
    /// The complete schema-1 role set, including layers without features.
    pub const ALL: [Self; 5] = [
        Self::Dxcc,
        Self::PrimarySubdivision,
        Self::SecondarySubdivision,
        Self::CqZone,
        Self::ItuZone,
    ];

    /// Return the schema-defined relative payload path for this role.
    pub(crate) fn path(self) -> &'static str {
        match self {
            Self::Dxcc => "layers/dxcc.fgb",
            Self::PrimarySubdivision => "layers/primary_subdivision.fgb",
            Self::SecondarySubdivision => "layers/secondary_subdivision.fgb",
            Self::CqZone => "layers/cq_zone.fgb",
            Self::ItuZone => "layers/itu_zone.fgb",
        }
    }
}

/// Parent context that distinguishes equal subdivision codes in different places.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParentScope {
    /// The value is globally scoped.
    Global,
    /// Primary subdivision within the supplied DXCC entity.
    Entity(String),
    /// Secondary subdivision within an entity and primary subdivision.
    Subdivision {
        /// Parent entity's recordable value.
        entity: String,
        /// Parent primary subdivision's recordable value.
        primary: String,
    },
}

/// A source-defined feature identity, separate from its display label.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SourceIdentity {
    /// Recipe-local pinned source ID.
    pub source_id: String,
    /// Source-defined key or reviewed revision-bound selector.
    pub feature_id: String,
}

/// A classified value that may be offered to the operator for recording.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueEntry {
    /// Manifest-local feature reference.
    pub key: String,
    /// Geographic field supplied by this value.
    pub field: Field,
    /// Geographic scope for identity and deduplication.
    pub parent_scope: ParentScope,
    /// Explicitly authored interchange/native recorded value.
    pub recordable_value: String,
    /// Human-readable label, not an identity or recordable value.
    pub display_label: String,
}

/// Provenance for the geometry and value carried by a feature.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Derivation {
    /// Manifest-local derivation key.
    pub key: String,
    /// All contributing original source identities.
    pub source_identities: Vec<SourceIdentity>,
    /// Coordinate operations used for this feature.
    pub operation_ids: Vec<String>,
    /// Reviewed complete replacements applied to this feature.
    pub correction_ids: Vec<String>,
    /// Authored geographic-value assignments.
    pub mapping_ids: Vec<String>,
    /// Compiled exclusion/replacement rules.
    pub exception_ids: Vec<String>,
}

/// Source publication and redistribution evidence retained with a bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRecord {
    /// Recipe-local source identity.
    pub id: String,
    /// Original data publisher.
    pub publisher: String,
    /// Source's published release or pinned commit.
    pub upstream_version: String,
    /// SHA-256 of the acquired source bytes.
    pub sha256: String,
    /// Original coordinate declaration, including known limitations.
    pub coordinate_declaration: String,
    /// Supporting source/terms references.
    pub citations: Vec<String>,
    /// Required bundled attribution/license files.
    pub notice_paths: Vec<String>,
}

/// Evidence for an authored operation, correction, mapping or exception.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceRecord {
    /// Category-local rule identity.
    pub id: String,
    /// Human-readable purpose, application rationale and known limits.
    pub description: String,
    /// Resolved rule parameters, excluding the shared descriptive fields above/below.
    /// This is provenance, never executable reader policy.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub definition: serde_json::Value,
    /// Sources whose features this rule may affect.
    pub source_ids: Vec<String>,
    /// Supporting definitions or authorship evidence.
    pub citations: Vec<String>,
    /// Required notices for the rule's inputs.
    pub notice_paths: Vec<String>,
}

/// An intentionally unsupported part of a bundle's declared coverage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewedOmission {
    /// Stable recipe-local omission identity.
    pub id: String,
    /// Field whose coverage is limited.
    pub field: Field,
    /// Affected source features, when source-defined.
    pub source_identities: Vec<SourceIdentity>,
    /// Reviewed reason, including the affected geography when not a whole feature.
    pub reason: String,
    /// Supporting source and review references.
    pub citations: Vec<String>,
}

/// Integrity record for a confined payload or notice file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDigest {
    /// Bundle-relative path using forward slashes.
    pub path: String,
    /// Exact encoded byte length.
    pub bytes: u64,
    /// Lowercase SHA-256 digest.
    pub sha256: String,
}

/// Geometry storage declaration for a mandatory geographic role.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayerDescriptor {
    /// Geographic field represented by the layer.
    pub field: Field,
    /// Schema-defined relative FlatGeobuf path.
    pub path: String,
    /// Number of encoded features, including zero for an empty layer.
    pub feature_count: u64,
    /// Min longitude/latitude and max longitude/latitude, absent only when empty.
    pub extent_or_empty: Option<[f64; 4]>,
}

/// Explicit representative release scope; absence outside it is never inferred coverage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scope {
    /// Human-readable selected geography and limitations.
    pub description: String,
}

/// Complete immutable bundle metadata, independent of the application's version.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// Format contract version; this reader supports exactly version 1.
    pub schema_version: u32,
    /// Build-input identity, independent of the application version.
    pub release_id: String,
    /// Digest of the complete resolved build recipe.
    pub recipe_sha256: String,
    /// Every payload and notice, excluding the manifest itself.
    pub files: Vec<FileDigest>,
    /// Exactly one descriptor per mandatory field.
    pub layers: Vec<LayerDescriptor>,
    /// Classified values referenced by encoded features.
    pub values: Vec<ValueEntry>,
    /// Feature provenance references.
    pub derivations: Vec<Derivation>,
    /// Acquired source publication and rights evidence.
    pub sources: Vec<SourceRecord>,
    /// Coordinate operation evidence.
    pub operations: Vec<ProvenanceRecord>,
    /// Complete geometry replacement evidence.
    pub corrections: Vec<ProvenanceRecord>,
    /// Geographic-value assignment evidence.
    pub mappings: Vec<ProvenanceRecord>,
    /// Compiled exclusion/replacement evidence.
    pub exceptions: Vec<ProvenanceRecord>,
    /// Reviewed unsupported geography and its reasons.
    pub omissions: Vec<ReviewedOmission>,
    /// Declared selected coverage.
    pub scope: Scope,
    /// Accuracy, source and numerical limits.
    pub limitations: Vec<String>,
}

/// Manifest identity selected by a release-store pointer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentRelease {
    /// Selected immutable directory component.
    pub release_id: String,
    /// Digest of the selected manifest bytes.
    pub manifest_sha256: String,
}

/// A normalized feature ready for artifact encoding.
#[derive(Clone, Debug)]
pub struct CompiledFeature {
    /// Unique part identity within its layer.
    pub part_id: String,
    /// Reference to the manifest's classified value.
    pub value_key: String,
    /// Reference to its provenance record.
    pub derivation_key: String,
    /// Finite valid 2D geometry in the artifact coordinate convention.
    pub geometry: MultiPolygon<f64>,
}

/// Compiled input for one mandatory layer, including intentionally empty layers.
#[derive(Clone, Debug)]
pub struct LayerInput {
    /// Geographic role of the supplied features.
    pub field: Field,
    /// Features whose parts are encoded deterministically.
    pub features: Vec<CompiledFeature>,
}

/// A producer's complete bundle before payload hashes are generated.
#[derive(Clone, Debug)]
pub struct BundleContents {
    /// Declared values/provenance; the writer fills file and layer descriptors.
    pub manifest: Manifest,
    /// Exactly one feature collection per mandatory role.
    pub layers: Vec<LayerInput>,
    /// Bundle-relative notice paths and their original text.
    pub notices: BTreeMap<String, String>,
}

/// Check a release identity before using it as a path component.
pub(crate) fn validate_release(id: &str) -> Result<(), GeographyError> {
    if id.is_empty()
        || id.len() > 128
        || matches!(id, "." | "..")
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    {
        return Err(invalid("unsafe or empty release identity"));
    }
    Ok(())
}

/// Require one unambiguous, portable confined relative path.
pub(crate) fn validate_path(path: &str) -> Result<(), GeographyError> {
    if path.is_empty()
        || path.contains(['\\', ':'])
        || Path::new(path)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        || path.split('/').any(|c| c.is_empty() || c == ".")
    {
        return Err(invalid(format!("unsafe relative path {path:?}")));
    }
    Ok(())
}

/// Validate the encoding and length of an integrity digest.
pub(crate) fn validate_digest(value: &str) -> Result<(), GeographyError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid("invalid SHA-256 digest"));
    }
    Ok(())
}

impl Manifest {
    /// Validate metadata references and schema invariants before accepting any features.
    pub(crate) fn validate(&self) -> Result<(), GeographyError> {
        if self.schema_version != 1 {
            return Err(GeographyError::UnsupportedSchema(self.schema_version));
        }
        validate_release(&self.release_id)?;
        validate_digest(&self.recipe_sha256)?;
        if self.scope.description.is_empty() {
            return Err(invalid("invalid bundle provenance"));
        }
        let paths = unique(self.files.iter().map(|r| r.path.as_str()), "file")?;
        for file in &self.files {
            validate_path(&file.path)?;
            validate_digest(&file.sha256)?;
            if file.path == "manifest.json" {
                return Err(invalid("manifest cannot digest itself"));
            }
        }
        let roles: BTreeSet<_> = self.layers.iter().map(|l| l.field).collect();
        if self.layers.len() != 5 || roles != Field::ALL.into_iter().collect() {
            return Err(invalid("missing or duplicated layer role"));
        }
        for layer in &self.layers {
            if layer.path != layer.field.path() || !paths.contains(layer.path.as_str()) {
                return Err(invalid("layer path missing or inconsistent with role"));
            }
            match (layer.feature_count, layer.extent_or_empty) {
                (0, None) => (),
                (n, Some([west, south, east, north]))
                    if n > 0
                        && [west, south, east, north].iter().all(|n| n.is_finite())
                        && west >= -180.
                        && east <= 180.
                        && south >= -90.
                        && north <= 90.
                        && west < east
                        && south < north => {}
                _ => return Err(invalid("layer count/extent inconsistency")),
            }
        }
        let source_ids = unique(self.sources.iter().map(|r| r.id.as_str()), "source")?;
        for source in &self.sources {
            validate_digest(&source.sha256)?;
            if source.publisher.is_empty()
                || source.upstream_version.is_empty()
                || source.coordinate_declaration.is_empty()
                || source.citations.is_empty()
            {
                return Err(invalid("incomplete source evidence"));
            }
            check_notices(&source.notice_paths, &paths)?;
        }
        let operations = rules(&self.operations, &source_ids, &paths)?;
        let corrections = rules(&self.corrections, &source_ids, &paths)?;
        let mappings = rules(&self.mappings, &source_ids, &paths)?;
        let exceptions = rules(&self.exceptions, &source_ids, &paths)?;
        unique(self.values.iter().map(|r| r.key.as_str()), "value")?;
        let mut scoped = BTreeMap::new();
        for value in &self.values {
            if value.recordable_value.is_empty() || value.display_label.is_empty() {
                return Err(invalid("empty classified value or label"));
            }
            let valid_scope = match (&value.field, &value.parent_scope) {
                (Field::PrimarySubdivision, ParentScope::Entity(entity)) => !entity.is_empty(),
                (Field::SecondarySubdivision, ParentScope::Subdivision { entity, primary }) => {
                    !entity.is_empty() && !primary.is_empty()
                }
                (Field::Dxcc | Field::CqZone | Field::ItuZone, ParentScope::Global) => true,
                _ => false,
            };
            if !valid_scope {
                return Err(invalid("classified value has wrong parent scope"));
            }
            if value.field == Field::CqZone {
                let zone = CqZone::new(
                    value
                        .recordable_value
                        .parse()
                        .map_err(|_| invalid("invalid CQ zone"))?,
                )
                .map_err(|_| invalid("invalid CQ zone"))?;
                if zone.get().to_string() != value.recordable_value {
                    return Err(invalid("noncanonical CQ zone value"));
                }
            }
            if value.field == Field::ItuZone {
                let zone = ItuZone::new(
                    value
                        .recordable_value
                        .parse()
                        .map_err(|_| invalid("invalid ITU zone"))?,
                )
                .map_err(|_| invalid("invalid ITU zone"))?;
                if zone.get().to_string() != value.recordable_value {
                    return Err(invalid("noncanonical ITU zone value"));
                }
            }
            let key = (value.field, &value.parent_scope, &value.recordable_value);
            if scoped.insert(key, &value.display_label).is_some() {
                return Err(invalid("duplicate scoped geographic value"));
            }
        }
        unique(
            self.derivations.iter().map(|r| r.key.as_str()),
            "derivation",
        )?;
        for row in &self.derivations {
            if row.source_identities.is_empty() {
                return Err(invalid("derivation without source identity"));
            }
            check_identities(&row.source_identities, &source_ids)?;
            check_refs(&row.operation_ids, &operations)?;
            check_refs(&row.correction_ids, &corrections)?;
            check_refs(&row.mapping_ids, &mappings)?;
            check_refs(&row.exception_ids, &exceptions)?;
        }
        unique(self.omissions.iter().map(|r| r.id.as_str()), "omission")?;
        for row in &self.omissions {
            if row.reason.is_empty() || row.citations.is_empty() {
                return Err(invalid("omission lacks reason or evidence"));
            }
            check_identities(&row.source_identities, &source_ids)?;
        }
        for path in &paths {
            if !self.layers.iter().any(|l| l.path == *path) && !path.starts_with("notices/") {
                return Err(invalid("unrecognized payload role"));
            }
        }
        Ok(())
    }
}

/// Collect unique nonempty local keys, rejecting aliasing within a category.
fn unique<'a>(
    keys: impl Iterator<Item = &'a str>,
    category: &str,
) -> Result<BTreeSet<&'a str>, GeographyError> {
    let mut set = BTreeSet::new();
    for key in keys {
        if key.is_empty() || !set.insert(key) {
            return Err(invalid(format!("empty or duplicate {category} key")));
        }
    }
    Ok(set)
}

/// Validate required attribution paths against declared payloads.
fn check_notices(notices: &[String], paths: &BTreeSet<&str>) -> Result<(), GeographyError> {
    for path in notices {
        if !path.starts_with("notices/") || !paths.contains(path.as_str()) {
            return Err(invalid(format!("missing notice {path}")));
        }
    }
    Ok(())
}

/// Validate identities against the pinned sources.
fn check_identities(
    identities: &[SourceIdentity],
    sources: &BTreeSet<&str>,
) -> Result<(), GeographyError> {
    let mut seen = BTreeSet::new();
    for row in identities {
        if row.feature_id.is_empty()
            || !sources.contains(row.source_id.as_str())
            || !seen.insert(row)
        {
            return Err(invalid("missing or duplicate source feature identity"));
        }
    }
    Ok(())
}

/// Validate a rule category and retain its available keys.
fn rules<'a>(
    rows: &'a [ProvenanceRecord],
    sources: &BTreeSet<&str>,
    paths: &BTreeSet<&str>,
) -> Result<BTreeSet<&'a str>, GeographyError> {
    let keys = unique(rows.iter().map(|r| r.id.as_str()), "provenance")?;
    for row in rows {
        if row.description.is_empty() || row.citations.is_empty() {
            return Err(invalid("rule lacks rationale or citations"));
        }
        check_refs(&row.source_ids, sources)?;
        check_notices(&row.notice_paths, paths)?;
    }
    Ok(keys)
}

/// Require every referenced key to exist exactly once.
fn check_refs(refs: &[String], keys: &BTreeSet<&str>) -> Result<(), GeographyError> {
    unique(refs.iter().map(String::as_str), "reference")?;
    for key in refs {
        if !keys.contains(key.as_str()) {
            return Err(invalid(format!("missing provenance reference {key}")));
        }
    }
    Ok(())
}

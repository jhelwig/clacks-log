//! Pinned build inputs, source selectors and reviewed compilation rules.

use anyhow::{Context, ensure};
use clacks_geography::artifact::{Field, ParentScope, ReviewedOmission, SourceIdentity};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

/// Limits for downloaded and expanded source data.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceLimits {
    /// Maximum downloaded or cached source size.
    pub download_bytes: u64,
    /// Maximum uncompressed size of one member.
    pub member_bytes: u64,
    /// Maximum sum of uncompressed member bytes.
    pub expanded_bytes: u64,
    /// Maximum number of archive entries.
    pub members: usize,
}

/// Concrete source encodings understood by this producer.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum SourceKind {
    /// Shapefile and sidecars, with independently pinned publisher/declarations.
    Shapefile,
    /// Pinned JSON collection embedded in a JavaScript assignment; never executed.
    Zones {
        /// Exact expected assignment identifier.
        assignment: String,
        /// Required number of original features before selection.
        expected_features: usize,
        /// Source identities whose open rings are explicitly accepted for later closure.
        open_rings: BTreeSet<String>,
    },
    /// Pinned coordinate correction grid retained without source-feature decoding.
    Grid,
}

/// A pinned acquisition with its interpretation, archive inventory and source rights.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcePin {
    /// Recipe-local source identity.
    pub id: String,
    /// Original publisher.
    pub publisher: String,
    /// Published version or Git commit.
    pub upstream_version: String,
    /// Source and redistribution references.
    pub citation_urls: Vec<String>,
    /// Explicit download URL, used only by fetch.
    pub download_url: String,
    /// SHA-256 of the acquired bytes.
    pub archive_sha256: String,
    /// Exact source `.prj` text or declared format convention.
    pub expected_coordinate_declaration: String,
    /// Explicit supported character encoding.
    pub text_encoding: String,
    /// Complete expected archive inventory; empty for nonarchives.
    pub archive_members: Vec<String>,
    /// Paths of bundled source notices.
    pub notice_paths: Vec<String>,
    /// Optional source-declared attribute domains checked before selecting records.
    #[serde(default)]
    pub attribute_domains: BTreeMap<String, BTreeSet<String>>,
    /// Source-specific decoder selection.
    pub kind: SourceKind,
    /// Bounded acquisition and archive expansion.
    pub limits: SourceLimits,
}

/// An exact-count source selection with source-defined identity fields.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSelection {
    /// Pinned source to select from.
    pub source_id: String,
    /// Source key fields; empty means a revision-bound feature digest identity.
    pub identity_fields: Vec<String>,
    /// AND across fields, membership within each supplied allowed-value set.
    pub properties: BTreeMap<String, BTreeSet<String>>,
    /// Optional revision-bound geometry/attribute guard for sources without a unique key.
    #[serde(default)]
    pub feature_sha256: Option<String>,
    /// Required count, preventing unnoticed omission/expansion on source changes.
    pub expected_count: usize,
}

/// Source-coordinate region assigned to one coordinate operation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum Assignment {
    /// All selected target-source geometry.
    Entire,
    /// An explicitly authored longitude/latitude rectangle in the source datum.
    Bounds([f64; 4]),
    /// Union of selected source polygons, optionally intersected with registry extent.
    SourceMask {
        /// All selected support features from these source pins.
        #[serde(default)]
        source_ids: Vec<String>,
        /// Exact source polygon components selected by an interior anchor.
        #[serde(default)]
        components: Vec<ComponentMask>,
        /// Selected support features in the same source coordinate system.
        identities: Vec<SourceIdentity>,
        /// Published operation applicability rectangle, including date-line wrapping.
        extent: Option<[f64; 4]>,
    },
    /// Explicitly reviewed residual after the named assignment masks are subtracted.
    Remainder {
        /// Previously declared masks excluded from this assignment, not failed operations.
        excluded_operation_ids: Vec<String>,
    },
}

/// An explicit selected operation, with parameters rather than inferred datum aliases.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum CoordinateMethod {
    /// Already-WGS84 input; only representation normalization applies.
    Identity,
    /// D259 approximation preserving supplied angular coordinates.
    NoShift,
    /// Geographic two-dimensional geocentric translation from GRS80 to WGS84.
    Translation {
        /// Earth-centered X displacement in metres.
        x_m: f64,
        /// Earth-centered Y displacement in metres.
        y_m: f64,
        /// Earth-centered Z displacement in metres.
        z_m: f64,
    },
    /// Mandatory pinned GeoTIFF correction grid.
    Grid {
        /// Source pin containing the required grid bytes.
        source_id: String,
    },
}

/// Geographic applicability and evidence for a selected coordinate operation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoordinateOperation {
    /// Recipe-local operation ID.
    pub id: String,
    /// Target source IDs whose selected geometry is partitioned.
    pub source_ids: Vec<String>,
    /// Parameterized execution method.
    pub method: CoordinateMethod,
    /// Source-coordinate assignment mask.
    pub assignment: Assignment,
    /// Registry identity, accuracy statement and approximation limits.
    pub description: String,
    /// Supporting operation/source references.
    pub citations: Vec<String>,
}

/// A complete reviewed replacement guarded by original source identity and bytes.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryCorrection {
    /// Correction identity.
    pub id: String,
    /// Exact source feature to replace.
    pub feature: SourceIdentity,
    /// Required acquired-source digest.
    pub expected_source_digest: String,
    /// Required original semantic feature digest.
    pub expected_feature_sha256: String,
    /// Confined recipe-relative complete GeoJSON geometry.
    pub replacement_geojson_path: String,
    /// Required replacement file digest.
    pub replacement_sha256: String,
    /// Intended geography and explanation of the source defect.
    pub rationale: String,
    /// Supporting source/review references.
    pub supporting_citations: Vec<String>,
    /// Required attribution files.
    pub notice_paths: Vec<String>,
}

/// An authored recorded value for exact selected source features.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValueMapping {
    /// Mapping identity, also used for the manifest value key.
    pub id: String,
    /// Original source feature identities.
    pub source_feature_selectors: Vec<SourceIdentity>,
    /// Original polygon components selected by source-coordinate interior anchors.
    /// Components retain their source identity and prescribed coordinate operations.
    #[serde(default)]
    pub source_components: Vec<ComponentMask>,
    /// Output role.
    pub field: Field,
    /// Geographic identity context.
    pub parent_scope: ParentScope,
    /// Explicit recorded value.
    pub recordable_value: String,
    /// Human-readable label.
    pub display_label: String,
    /// Factual value/definition evidence.
    pub citations: Vec<String>,
}

/// Optional classified replacement for an excluded area.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MappingReplacement {
    /// Complete recipe-relative replacement geometry file.
    pub geometry_path: String,
    /// Pinned geometry digest.
    pub sha256: String,
    /// Explicit classified value and its factual sources.
    pub mapping: ValueMapping,
}

/// A compiled subtraction and optional classified replacement; no runtime precedence rule.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MappingException {
    /// Rule identity.
    pub id: String,
    /// Mappings from which the exclusion is subtracted.
    pub affected_mapping_ids: Vec<String>,
    /// Unmodified base mappings supplying an exact compiled exclusion mask.
    /// Mutually exclusive with an external exclusion geometry file.
    #[serde(default)]
    pub exclusion_mapping_ids: Vec<String>,
    /// Confined recipe-relative exclusion geometry file.
    pub exclusion_geometry_path: String,
    /// Pinned exclusion digest.
    pub exclusion_sha256: String,
    /// Supported replacement; absent means uncovered geography.
    pub replacement: Option<MappingReplacement>,
    /// Geographic meaning and known coverage sacrifices.
    pub rationale: String,
    /// Supporting factual/review evidence.
    pub citations: Vec<String>,
    /// Required notices for geometry/definitions.
    pub notice_paths: Vec<String>,
}

/// Native query input for an independently authored acceptance case.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum AcceptanceGeometry {
    /// Explicit native point, latitude then longitude in integer microdegrees.
    Point {
        /// Signed latitude microdegrees.
        latitude_microdegrees: i32,
        /// Signed longitude microdegrees.
        longitude_microdegrees: i32,
    },
    /// The entire supplied native locator cell.
    Grid(String),
}

/// Expected geographic coverage in a recipe acceptance case.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum ExpectedCoverage {
    /// Entire point/area classified.
    Complete,
    /// Some positive area uncovered.
    Partial,
    /// No point/area classified.
    None,
}

/// One expected scoped candidate; a display label is not its identity.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedCandidate {
    /// Geographic identity context.
    pub parent_scope: ParentScope,
    /// Exact authored recorded value.
    pub value: String,
}

/// Independently justified expectations for a field.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedField {
    /// Required coverage.
    pub coverage: ExpectedCoverage,
    /// Exact candidate set, ignoring order.
    pub candidates: Vec<ExpectedCandidate>,
}

/// A real-source or analytical acceptance obligation checked before publication.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceCase {
    /// Diagnostic case identity.
    pub id: String,
    /// Native point or full locator area.
    pub query: AcceptanceGeometry,
    /// Field-specific expectations; unspecified fields make no claim.
    pub expected: BTreeMap<Field, ExpectedField>,
    /// Independent source/rule/analytical justification.
    pub citations: Vec<String>,
}

/// Complete build input, independent from cache/output locations and current time.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildRecipe {
    /// Recipe schema, currently 1.
    pub schema_version: u32,
    /// Original source pins and source rights.
    pub sources: Vec<SourcePin>,
    /// Explicit source-defined selected features.
    pub selections: Vec<SourceSelection>,
    /// Coordinate partition, parameters and evidence.
    pub operations: Vec<CoordinateOperation>,
    /// Guarded complete geometry replacements.
    pub corrections: Vec<GeometryCorrection>,
    /// Classified values and their exact source correspondences.
    pub mappings: Vec<ValueMapping>,
    /// Compiled exclusions/replacements.
    pub exceptions: Vec<MappingException>,
    /// Reviewed unsupported source/geography records.
    pub omissions: Vec<ReviewedOmission>,
    /// Independently authored query expectations.
    pub acceptance_cases: Vec<AcceptanceCase>,
    /// Original attribution/license text under confined notice paths.
    pub notices: BTreeMap<String, String>,
    /// Declared selected geography.
    pub scope: String,
    /// Source, accuracy and coverage limits.
    pub limitations: Vec<String>,
    /// Resolved base directory for guarded recipe-relative geometry files.
    #[serde(skip)]
    pub(crate) directory: PathBuf,
}

/// Load a recipe and validate source identities, limits and confined file references.
///
/// No sources are fetched here. Interpretation and pin changes fail explicitly
/// instead of accepting an apparent successful empty source selection.
pub fn read_recipe(path: &Path) -> anyhow::Result<BuildRecipe> {
    let bytes = fs::read(path).with_context(|| format!("read recipe {}", path.display()))?;
    let mut recipe: BuildRecipe =
        serde_json::from_slice(&bytes).context("decode geographic recipe")?;
    recipe.directory = path.parent().context("recipe parent")?.to_owned();
    validate_recipe(&recipe)?;
    Ok(recipe)
}

/// Revalidate mutable recipe rows at each processing boundary before using paths or pins.
pub(crate) fn validate_recipe(recipe: &BuildRecipe) -> anyhow::Result<()> {
    ensure!(
        recipe.schema_version == 1,
        "unsupported recipe schema {}",
        recipe.schema_version
    );
    ensure!(!recipe.scope.is_empty(), "recipe lacks scope");
    let mut ids = BTreeSet::new();
    for source in &recipe.sources {
        ensure!(
            !source.id.is_empty() && ids.insert(&source.id),
            "empty or duplicate source ID"
        );
        validate_digest(&source.archive_sha256)?;
        ensure!(
            !source.publisher.is_empty()
                && !source.upstream_version.is_empty()
                && !source.citation_urls.is_empty()
                && !source.expected_coordinate_declaration.is_empty(),
            "source {} lacks provenance/declaration",
            source.id
        );
        ensure!(
            source.text_encoding == "UTF-8",
            "unsupported source encoding for {}",
            source.id
        );
        ensure!(
            source.download_url.starts_with("https://")
                || source.download_url.starts_with("http://127.0.0.1:")
                || source.download_url.starts_with("http://localhost:"),
            "source requires HTTPS (or loopback test transport)"
        );
        let limits = &source.limits;
        ensure!(
            limits.download_bytes > 0
                && limits.download_bytes <= 512 * 1024 * 1024
                && limits.member_bytes > 0
                && limits.member_bytes <= 1024 * 1024 * 1024
                && limits.expanded_bytes > 0
                && limits.expanded_bytes <= 2 * 1024 * 1024 * 1024
                && limits.members > 0
                && limits.members <= 128,
            "source {} exceeds resource limits",
            source.id
        );
        let mut members = BTreeSet::new();
        for member in &source.archive_members {
            validate_relative(member)?;
            ensure!(
                members.insert(member),
                "duplicate archive member declaration"
            );
        }
        for notice in &source.notice_paths {
            ensure!(
                recipe.notices.contains_key(notice),
                "source {} lacks notice {notice}",
                source.id
            );
        }
    }
    for path in recipe.notices.keys() {
        validate_relative(path)?;
        ensure!(
            path.starts_with("notices/"),
            "notice outside notices directory"
        );
    }
    for selection in &recipe.selections {
        if let Some(digest) = &selection.feature_sha256 {
            validate_digest(digest)?;
        }
        ensure!(
            ids.contains(&selection.source_id) && selection.expected_count > 0,
            "invalid source selection"
        );
        ensure!(
            selection.properties.values().all(|s| !s.is_empty()),
            "empty selection predicate"
        );
    }
    Ok(())
}

/// Validate a portable recipe-relative path without accepting traversal or alternate separators.
pub(crate) fn validate_relative(path: &str) -> anyhow::Result<()> {
    use std::path::Component;
    ensure!(
        !path.is_empty()
            && !path.contains(['\\', ':'])
            && !path.split('/').any(|p| p.is_empty() || p == ".")
            && Path::new(path)
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "unsafe relative input path"
    );
    Ok(())
}

/// Check digest spelling before using it as a cache key.
pub(crate) fn validate_digest(digest: &str) -> anyhow::Result<()> {
    ensure!(
        digest.len() == 64
            && digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "invalid SHA-256 digest"
    );
    Ok(())
}

/// Check the schema's immutable directory component before publication.
pub(crate) fn validate_release(id: &str) -> anyhow::Result<()> {
    ensure!(
        !id.is_empty()
            && id.len() <= 128
            && !matches!(id, "." | "..")
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)),
        "unsafe release identity"
    );
    Ok(())
}

/// A source polygon component selected without fabricating an administrative boundary.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentMask {
    /// Exact selected original feature.
    pub feature: SourceIdentity,
    /// Longitude/latitude interior anchor in the original source coordinate system.
    pub contains_point: [f64; 2],
}

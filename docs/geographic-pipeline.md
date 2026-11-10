# Geographic data production and lookup

Specification for [issue #5](https://github.com/jhelwig/clacks-log/issues/5).
D239 and D257–D269 supply the accepted direction. This document defines the
production and integration contracts; it is not a claim of complete geographic coverage.
The [product contract](product.md), [native model](native-model.md) and
[observability contract](observability.md) remain applicable.
The [reader guide](../lib/clacks-geography/README.md) and
[producer guide](../lib/clacks-geography-builder/README.md) document concrete
interfaces, commands and numerical limits. The
[representative recipe](../references/geography/representative.json) carries
the pinned inputs and acceptance cases. Issue #23 adds the separately maintained
[shipping recipe](../references/geography/shipping.json), shared population rules
and desktop consumers. The [build procedure](../references/geography/BUILDING.md)
describes the cacheable data dependency: exact hits reuse validated output and
misses generate it automatically from the current checkout. Server-provided replacements are future work.

DX Century Club is abbreviated DXCC; International Telecommunication Union is
ITU. CQ refers to CQ magazine's operating zones. Geographic JavaScript Object
Notation is GeoJSON; its underlying data notation is JSON. North American Datum
of 1983 is NAD83; World Geodetic System 1984 is WGS84. A datum defines a reference
frame for positions, a realization implements that frame, and a coordinate epoch
identifies the time to which coordinates refer. A dataset's publication year is
not its coordinate epoch. SHA-256 means Secure Hash Algorithm with a 256-bit digest.
GeoTIFF is an image/grid format carrying geographic metadata; here it contains
coordinate corrections rather than a display map.

## 1. Outcome and boundaries

Deliver the production Rust source-ingestion, transformation, correction,
mapping, artifact-writing and artifact-reading/query components. Issue #23
extends the same pipeline's coverage and integrates offline operating-location
assistance; it does not replace a demonstration environment or alternate reader.

Issue #5 delivers representative real-source artifacts exercising every selected
mechanism, reproducible source pins, provenance/notices and meaningful acceptance
evidence. Complete U.S. coverage belongs to #23. Worldwide DXCC/CQ/ITU coverage is
desired, with incomplete fidelity permitted; no complete-world release gate is
introduced. A successful build means its declared scope and exclusions are true,
not that every geographic location can be classified.

Excluded here: application UI, automatic application of suggestions, changes to
saved profiles/QSOs, source-directory lookup, an online mapping service, a spatial
database server, a standalone data updater and new native geographic-ID types.
Existing recorded snapshots and manual corrections retain their established
lifetime. Geographic lookup never changes the supplied position.

## 2. Selected approaches and ownership

| Choice | Selected approach and tradeoff |
| --- | --- |
| Artifact | Indexed FlatGeobuf layers plus a manifest/notices. Established Rust reading/writing fits immutable layers; multiple files require one coherent release. GeoPackage offers single-file relational packaging, but its Rust implementation is younger and adds SQLite-binding coordination. |
| Geometry | `geo` planar predicates, sweep-based validation, unions and longitude-strip clipping; `clipper2-rust` for checked assignment/mapping overlays and shared-lattice area-coverage difference. Preserve source coordinate-space edge semantics; date-line handling is explicit. Native geographic stacks add build/data dependencies without fixing missing source evidence. |
| Coordinates | `proj4rs` for identified regional operations; documented no-shift approximation for the remaining components. No inferred legacy island datum or survey-accuracy claim. |
| Exceptions | Compile authored DXCC mappings/exclusions/replacements before publication. Readers query resulting features rather than interpret source-specific rule precedence. |
| Repairs | Complete reviewed replacement geometries in ordinary GeoJSON, with source guards and rationale. Larger diffs are accepted instead of a custom vertex-edit language. |
| Lookup | Per-field candidates plus coverage, with independent field resolution. |
| Compatibility | One bundle-schema version, separate from data-release identity. No per-layer capability negotiation. |

Production dependency direction:

```text
clacks-geography -> clacks-model
clacks-geography-builder -> clacks-geography + clacks-model
clacks-geography-build executable -> clacks-geography-builder
clacks-desktop -> clacks-geography + clacks-model
```

| Workspace component | Owning modules and responsibilities |
| --- | --- |
| `lib/clacks-geography` | `artifact`: manifest/value/provenance contracts, validation and read-only bundle access. `lookup`: point/area queries and typed results. `geometry` supplies shared topology validation for normalized and longitude-unwrapped producer geometry. |
| `lib/clacks-geography-builder` | `source`: acquisition/cache and source decoding. `compile`: coordinate operations, corrections and mapping compilation. `publish`: staging, reader-based validation and publication. `application`: command parsing, diagnostics and telemetry composition. |
| `bin/clacks-geography-build` | Thin main delegating to the builder's application entry point. |

These module boundaries are engineering choices in this draft. No additional
shared crate, backend/plugin framework, generic task dispatcher or xtask is needed.
Readers do not depend on downloads, shapefile decoding or coordinate transformation
libraries. The builder uses the explicit artifact validator for production acceptance.

Use Rust 1.99.1, workspace-inherited dependencies/metadata, ordinary Serde types,
named module files and owning-module APIs. Follow repository documentation,
error-handling and Rust-only tooling requirements. No public interface exists solely
for tests. Use typed `thiserror` domain errors where callers need categories and
`anyhow` context at processing/application boundaries without erasing causes.

## 3. Dependencies and source evidence

The capability assessment inspected published APIs/source/manifests, not a compiled
integration. Implementation must lock and verify the actual transitive graph.

| Dependency | Intended use and feature boundary |
| --- | --- |
| `geo` 0.33.1 / compatible `geo-types` | Planar geometry, validity and topological relationships. Leave the native `proj` feature off. Use explicit union semantics for overlapping inputs; do not accidentally cancel overlaps with an even/odd fill operation. |
| `flatgeobuf` 6.0.1 | Indexed local-file reading/writing; disable default networking features. Write two-dimensional `MultiPolygon` features and indexes explicitly. |
| `geozero` 0.15.1 if conversion needs it | Only the geometry conversion features actually used; no native geographic bindings. |
| `shapefile` 0.9 | Census/Natural Earth geometry and attributes; verify text encoding and coordinate declarations separately. |
| `proj4rs` 0.2.1 | Producer only, explicit three-parameter operations and pinned GeoTIFF correction grids through `tiff`. Its grid support is experimental and requires independent checks. |
| Rustls-backed `ureq` 3.5 | Explicit source fetching; no OpenSSL-backed transport. No network dependency in the reader. |
| `zip` with an explicitly Rust-backed Deflate feature selection | Source archives. Disable unrelated default compression/encryption features. |
| Existing Serde/SHA-256 dependencies; `tempfile` | Structured metadata, digest checking and same-filesystem staging/atomic pointer replacement. |

Source pins, digests, redistribution evidence and known defects are recorded in
[the source research](../references/location-lookup.md),
[historical candidate audit](../references/geographic-data-audit.json), and
[coordinate investigation](../references/geographic-coordinate-operations.md).
The historical audit used external research tools; it is not a Rust fixture
generator or acceptance oracle. Reproduce delivered evidence using Rust tooling.

Sources:

- Census 2026 state/county archives. Remove Connecticut's nine planning-region
  records and use its eight actual county records from the pinned 2021 archive.
  Keep the modern state geometry and report/check vintage seams. Planning regions
  are not counties. Preserve source identifiers and Census attribution.
- HA8TKS CQ and ITU repositories at the researched commits. Extract the one
  expected JSON feature collection from the pinned JavaScript assignment using
  bounded, string-aware extraction and JSON parsing. The pinned assignments use
  trailing separators: remove those only outside quoted strings before strict
  JSON decoding. Verify feature identities
  and counts; reject an unexpected wrapper. Never execute JavaScript. Preserve
  the author's MIT license (named for the Massachusetts Institute of Technology)
  and modification notices.
- Natural Earth subunits 5.1.1 and minor islands 4.1.0. These public-domain inputs
  supply geometry, not a DXCC classification. Clacks authors the correspondence
  and exceptions; retain source acknowledgment and mapping citations.

No Python fixture generation or project tooling, native geographic command-line
utilities, browser execution, or executed TrustedQSL (TQSL) reference code is introduced.
The existing model catalogs may supply recorded vocabulary through their public
contracts; geographic libraries must not depend on the signing crate.

## 4. Build recipe, corrections and mappings

The following is the concrete recipe schema. Identifiers in these records are
recipe-local references, not new account/QSO identities or promised permanent
identifiers across arbitrary source changes.

```text
BuildRecipe
  schema_version = 1
  sources: SourcePin[]
  selections: SourceSelection[]
  operations: CoordinateOperation[]
  corrections: GeometryCorrection[]
  mappings: ValueMapping[]
  exceptions: MappingException[]
  omissions: ReviewedOmission[]
  acceptance_cases: AcceptanceCase[]

SourcePin
  id, publisher, upstream_version_or_commit, citation_urls, download_url
  archive_sha256, expected_coordinate_declaration, text_encoding
  archive_members, download_and_expansion_limits, notice_ids

GeometryCorrection
  id, source_id, expected_source_digest, feature_selector
  replacement_geojson_path, replacement_sha256
  rationale, supporting_citations, notice_ids

ValueMapping
  id, source_feature_selectors, optional source_components, field, parent_scope
  recordable_value, display_label, citations

MappingException
  id, affected_mapping_ids, exclusion_geometry_reference
  optional replacement_geometry_reference_and_value_mapping
  rationale, citations, notice_ids

ReviewedOmission
  id, affected_field, source_selectors_or_geometry_reference
  reason, citations
```

`source_components` contains `ComponentMask` entries with an exact original
feature identity and an interior longitude/latitude anchor in that source's
coordinate convention. Each anchor must identify exactly one original polygon.
Selected components pass through the same prescribed coordinate operations as
whole-feature mappings, retaining source provenance. This supports distinct
DXCC islands within one administrative feature without copying untransformed
source coordinates into target-frame replacement files. Whole-feature and
component selections for the same original feature cannot overlap in one
mapping. Existing mapped exclusions subtract the selected transformed geometry.
This is producer recipe information; runtime artifact schema and lookup rules
remain unchanged.

Selectors use source-defined keys, such as the Census geographic identifier
field `GEOID`, not source file row
offsets or display labels alone. For a source without a suitable key, use a
reviewed selector plus its expected source revision and feature digest; fail on
zero/multiple matches. No nearest-place or guessed label matching is allowed.

Correction files are ordinary GeoJSON geometries/features in WGS84 longitude/
latitude. For an input in another datum, execute its declared operation first;
the replacement then supplies the entire corrected normalized geometry. A source
guard still binds that correction to the original input. Do not label NAD83
coordinates GeoJSON and thereby imply they were already WGS84.

Execute exclusions by subtraction from affected parent mappings; add supported
replacement features with their own values. Do not interpret source ordering as
precedence. Conflicting exception definitions, missing references and cycles are
errors. Validated exclusions without replacements leave uncovered areas. Unknown
or operating-context-dependent DXCC classifications remain unknown; a point is
not authority to create an arbitrary island polygon or circle.

Mappings provide recordable values explicitly: for example Washington `WA`, and
King County `WA,King`, alongside readable labels and original source identities.
Do not derive values by stripping administrative suffixes. Vocabulary mismatches
need an authored mapping or reviewed omission, not a silent best guess. Existing
manual/imported text is outside this compilation process.

## 5. Coordinate operations and geometry compilation

The artifact convention is WGS84 longitude first, latitude second, in degrees,
using finite `f64` coordinates. Maintain source geometry precision; native query
points retain the model's microdegree precision. Geometry operations do not
rewrite native positions or claim accuracy implied by decimal places.

Use the regional operations and parameters pinned in the coordinate investigation:

| Region | Operation |
| --- | --- |
| Contiguous states/Alaska mainland | EPSG:1188, explicit zero geocentric translation; stated operation accuracy 4 m. |
| Aleutians, within applicability | EPSG:1251, X=−2, Y=0, Z=+4 m; stated accuracy 8 m. |
| Main Hawaiian islands, within applicability | EPSG:1252, X=+1, Y=+1, Z=−1 m; stated accuracy 4 m. |
| Puerto Rico/U.S. Virgin Islands, within applicability | EPSG:1731 using the pinned `us_noaa_pvhpgn.tif`; stated accuracy 2 m. |
| Remaining Pacific/outlying/offshore components | D259's explicit no-shift approximation; accuracy unestablished. |
| Natural Earth/HA8TKS geographic coordinates | No datum conversion; retain the documented interpretation and source limitations. |

EPSG names the geodetic parameter registry originally maintained by the European
Petroleum Survey Group. The numbers identify operations, not a universal accuracy
guarantee. WGS84/NAD83 realization differences can be metre-scale, and legacy
errors could be much larger. Boundary-source error is independent of these shifts.

The recipe assigns operations to source components/regions explicitly, using
regions expressed in the original source coordinate system with that declaration
retained; do not encode a NAD83 assignment region as WGS84 GeoJSON. A polygon
crossing assignments is split consistently before applying operations. Shared edges within the same operation
use identical deterministic transformations. Across different operations, retain each
operation exactly and report the resulting seams, including gaps or overlaps (D269).
Do not adjust the no-shift boundary, blend operations or snap geometry to force agreement. Do not choose an operation from the first
overlapping bounding box. Include the eight older Connecticut counties in the
mainland assignment. Keep datum-assignment regions separate from DXCC boundaries.

Pass angles to/from `proj4rs` in radians. Execute geographic two-dimensional
operations using zero ellipsoidal height internally; discard transformed height
instead of inventing recorded elevation. Missing/out-of-area required grids and
failed/non-finite transformations are errors. The no-shift recipe assignment is
not error recovery for these failures. Pin all grid files; no implicit network
fetch or optional-grid fallback occurs during compilation.

Processing order:

1. Verify source pins, coordinate declarations, encoding and selected identities.
2. Decode rings/parts while retaining identities and source longitude continuity.
   Close the known HA8TKS open rings explicitly as a recorded conversion; reject
   unexpected structural defects instead of blindly closing arbitrary inputs.
3. Establish source edge interpretation and operation assignments; transform valid
   segments, splitting/densifying where necessary. Invalid source linework destined
   for a reviewed full replacement is retained for its source guard rather than
   passed to polygon overlay as though valid.
4. Apply guarded reviewed replacement geometry and validate the result.
5. Normalize/split date-line geometry, preserve holes and validate topology.
6. Apply value mappings, exclusions/replacements and explicit Census-derived CQ
   state assignments where published definitions apply. A coarse world zone must
   not override a detailed U.S. state assignment; compile this precedence here.
7. Validate compiled geometry and value/provenance references, then write layers.

### Date line, poles and numerical behavior

Longitude wrapping is geometry processing, not clamping. Preserve explicit
source longitudes outside ±180° as continuity evidence. For wrapped rings, lift
successive vertices to a continuous longitude interval before clipping against
360° world strips, translate resulting fragments into [−180°, +180°], and preserve
parent/hole associations. A half-world edge, ambiguous ring closure or apparent
whole-world wrap needs an explicit source interpretation/replacement; do not
guess a small polygon from its bounds. Persist intentional date-line seam edges.
Follow [GeoJSON's antimeridian guidance](https://www.rfc-editor.org/rfc/rfc7946.html#section-3.1.9)
when authoring replacement polygons.

Queries at −180° and +180° examine both equivalent seams and deduplicate geographic
values. A grid abutting that seam is not expanded or wrapped into a different
cell. Split/translate its query pieces consistently with artifact fragments.

Do not extend a source stopping at latitude 85° to a pole or infer a polar cap
from a line closing in longitude. Unsupported polar portions are disclosed
omissions. Explicit polar-cap geometry requires reviewed semantics and a valid
split representation; a true pole point queries all applicable longitude
representations and aggregates their values, since its longitude cannot choose
a geographic sector. Missing polar support yields uncovered results rather than
nearest-zone inference.

Use library topological predicates and polygon union/difference rather than
summing overlapping polygon areas or comparing approximate area percentages.
No positive-area overlap is discarded as an application-level small-sliver
threshold. No buffer, snapping to recorded grids, source simplification or
largest-overlap rule is introduced.

Engineering numerical acceptance: transformations subdivide nonlinear segments
until quarter/midpoint deviation from the output segment is at most 0.01 m,
with maximum subdivision depth 24 and a shared budget of one million vertices
per source feature. Failure to meet the criterion within these limits stops the
build. This is a computational approximation budget, not source accuracy. Preserve straight
source-coordinate edges when no nonlinear operation applies. Test the chosen
overlay engine at global/date-line extents and two- through twelve-character
grid sizes, including narrow representable known overlaps/gaps. A library operation
that erases a known test sliver is not grounds to silently relax D260; use a
more suitable local coordinate window/library facility or report failure. The
library guides document the numerical limits and verification cases.

Assignment partition intersections/differences and mapping subtraction use a
shared binary lattice, capped at 2^48 units/degree and 10^17 coordinate magnitude.
Original vertices and source intersection coordinates are restored after clipping;
unresolved nodes, changed turns or ambiguous intersection correspondence fail the
build. Shared partition coordinates therefore remain comparable for seam reporting.
Before projection, each operation's selected features are noded at common exact
collinear endpoints, so differing source segmentation cannot introduce a seam
within that operation. Added nodes consume the existing feature vertex budget.
The engine still applies each distinct coordinate operation independently under
D269; retaining source boundary correspondence does not blend transformed results.

## 6. Bundle schema version 1

Engineering choice: a bundle is an immutable directory. An optional outer
distribution archive does not change its contents or reader contract.

```text
manifest.json
layers/dxcc.fgb
layers/primary_subdivision.fgb
layers/secondary_subdivision.fgb
layers/cq_zone.fgb
layers/itu_zone.fgb
notices/<notice-id>.txt
```

All five roles exist, including explicitly empty layers when a representative
release supplies no values for a role. The manifest makes that coverage limit
explicit. The reader never interprets an absent/corrupt layer as empty coverage.

```text
Manifest
  schema_version: 1
  release_id: generated build-input identity, independent of application version
  recipe_sha256
  files: {relative_path, bytes, sha256}[]
  layers: {field, path, feature_count, extent_or_empty}[]
  values: {key, field, parent_scope, recordable_value, display_label}[]
  derivations: {key, source_identities, operation_ids,
                correction_ids, mapping_ids, exception_ids}[]
  sources: source versions/commits, archive digests, declarations,
           publisher/citations and notice references
  operations, corrections, mappings, exceptions: provenance records
    (shared identity/rationale/citations/notices appear once; definition holds rule parameters)
  omissions: reviewed reason and affected feature/geometry references
  scope: representative coverage description
  limitations: source/accuracy/polar/geometry limits
```

All relative paths are confined regular files: reject absolute paths, traversal,
duplicates and symlinks. `files` covers every payload/notices file, not the
manifest itself. Checksums establish integrity relative to the supplied manifest,
not publisher authentication. Application packaging supplies the trusted bundle;
cryptographic release signing is not introduced by this contract.

Generated manifests use compact JSON. Coordinate interpretation is fixed by
this schema and the FlatGeobuf CRS84 headers; it is not a separately configurable
manifest field. Source identities belong to output derivations and specific
omissions, not an inventory of every intermediate input feature.

Each FlatGeobuf feature has fixed string properties `part_id`, `value_key` and
`derivation_key`, referencing manifest entries. Part IDs are unique within a
layer/release; they are not recorded QSO identities. Store indexed, finite,
two-dimensional `MultiPolygon` geometry. Header coordinate metadata and actual
axis order must agree with the explicit artifact convention. Validate roles,
columns, references, counts, bounds and geometry; require the standard 16-way
spatial index for
nonempty layers. Empty layers have verified zero features and no tree to query;
return no candidates without constructing an empty index search.

Candidate identity is `(field, parent_scope, recordable_value)`, not its label
or source-row number. Scope is global for zones/DXCC, DXCC entity for primary
subdivisions, and entity/primary subdivision for secondary subdivisions. This
draft's scoped key prevents unrelated subdivisions with the same short code
from being collapsed. Labels must be consistent per key; multiple source parts
retain all contributing provenance. Scope does not add field-dependency merges
or rewrite native stored values.

Readers support schema 1 explicitly. Mandatory roles, value interpretations or
property changes require another schema version. Optional descriptive metadata
may be ignored within a supported schema, but unknown required roles or value
kinds are errors. A new source vintage or corrected polygon alone changes the
release identity, not the schema. IDs, input timestamps and recipe metadata are
supplied/pinned rather than populated from the current clock during a rebuild.
Use deterministic source/feature ordering and deterministic writer settings.
Release IDs used as directory components must contain only ASCII letters,
digits, dots, hyphens or underscores, be at most 128 characters, and not be empty,
`.` or `..`. This is a path-safety restriction, not a new global identity scheme.

## 7. Reader and lookup contracts

Public responsibilities, with concrete Rust naming left to the implementation
plan:

```text
read.open_bundle(path, expected_manifest_sha256) -> Result<Bundle>
validate.open_bundle(path) -> Result<Bundle>  # production acceptance
Bundle.lookup(LookupGeometry) -> Result<LookupResult>

LookupResult
  release_id
  dxcc: FieldLookup<String>
  primary_subdivision: FieldLookup<String>
  secondary_subdivision: FieldLookup<String>
  cq_zone: FieldLookup<CqZone>
  itu_zone: FieldLookup<ItuZone>

FieldLookup<T>
  candidates: Candidate<T>[]
  coverage: Complete | Partial | None

Candidate<T>
  value: T
  parent_scope
  display_label
  source_identities and contributing derivation references
```

Use the native model's `Position::lookup_geometry`, `LookupGeometry`, grid bounds,
`Coordinates`, `CqZone` and `ItuZone`; do not create a competing locator parser,
timestamp/coordinate representation or a second station-location model.
Validate supplied grid bounds at the lookup boundary: denominators must be
nonzero, conversion must produce finite coordinates, and bounds must describe
a nonempty area within the permitted latitude/longitude range. Invalid query
geometry returns an ordinary error rather than panicking or producing coverage.
Selection supplies an expected manifest digest independently of the selected
directory: packaged data uses the checked-in release descriptor, and an external
release uses its explicitly configured digest (or the packaged digest when using
the same release elsewhere). Runtime opening hashes that same manifest handle
before interpreting it, checks supported schema, verifies declared payload hashes
and lengths, and retains their file handles. It performs no full metadata,
feature, topology, index or directory-inventory validation. Subsequent queries
remain bound to those immutable files, not a repeatedly resolved current pointer.
Ordinary decoding guards and I/O errors apply as data is used; failed reads must
not become successful unknown coverage.

Exhaustive acceptance belongs to `artifact::validate::open_bundle`: manifest
semantics, feature references, coordinate bounds, polygon topology, framing,
canonical indexes, counts/extents and complete inventory. Build, explicit verify
operations use that production path before caching an artifact.
Native packaging verifies its build-provided manifest and payload identities;
it does not repeat geographic production acceptance.

For each field:

1. Use the index for bounding-box candidates for every normalized query piece.
2. Resolve actual topology. A point accepts features whose closure covers it;
   boundaries are included. An area candidate requires a two-dimensional
   intersection, excluding edge/corner-only touches.
3. Group by scoped geographic value, merge contributing coverage/provenance and
   deduplicate date-line fragments. Sort results deterministically by scoped value.
4. For points, coverage is complete when any known candidate covers the point,
   otherwise none. Points cannot have partial coverage.
5. For areas, union the known candidate intersections. Coverage is none when
   there is no area, complete when the requested area minus this union has no
   two-dimensional remainder, and partial otherwise. Uncovered holes and excluded regions count as uncovered;
   line-only residuals do not create area or introduce another candidate.

One candidate with complete coverage is eligible for autofill. Several candidates
are ambiguous even when one occupies most of the area. Partial coverage never
permits autofill, even with one candidate. No candidates/none means unknown.
Do not discard ambiguity in one field to force a jointly consistent location;
fields resolve independently under D184. Shared `assistance` rules populate
eligible native fields; editor adapters record explicit choices. The reader
never fills or modifies a `RecordedLocation` itself.

## 8. Publication, reproducibility and command-line boundary

Engineering choice: use immutable release directories and one small current-release
pointer file under a caller-selected output root:

```text
output/releases/<release-id>/...
output/current.json = {release_id, manifest_sha256}
```

Acquire a standard exclusive file lock for output publication. Build in a private
staging directory on that filesystem. Compile every selected input, write payloads,
compute hashes, write the manifest, then open it with the production reader and
run the declared structural/acceptance checks. Only after success rename the
staging directory to its new immutable release path. Never overwrite an existing
release with different bytes; identical validated content may be reused.

Flush completed files before publication. Write the pointer into a same-directory
temporary file, flush it and use `tempfile::NamedTempFile::persist`'s atomic replace
facility. That successful replacement is the publication commit point. Earlier
failure/cancellation leaves the old pointer and release unchanged. Later cleanup
problems are reported as published-with-cleanup-warning, not a failed publication
claiming unchanged output. Verify real Windows replacement behavior; if a sharing
violation prevents replacement, return failure with the previous pointer intact.
Do not promise power-loss durability beyond the filesystem/platform's guarantees.

A crash before pointer publication can leave unreferenced staging/new-release
directories; they are never selected implicitly. Restart uses the normal build
and validation path. No job/session-resumption framework or automatic removal of
old in-use releases is required. Readers pin a release for their lifetime.

The command-line interface (CLI) supplies standard `--help` and documented Cargo invocation. Proposed
commands are `fetch --recipe ... --cache ...`, `build --recipe ... --cache ...
--output ...`, and `verify --bundle ... --recipe ...`. Fetch verifies digest and
limits before installing source-cache entries. Build/verify perform no network
requests. Reject archive traversal, duplicate member names and expansion beyond
recipe limits. Failures identify source/feature/operation and retain causes.
Expected reviewed omissions are reported distinctly from unexpected failures.

## 9. Observability

Follow the [existing contract](observability.md); no telemetry abstraction or
manual lifecycle machinery is introduced. Libraries emit, application composition
configures. Use `#[instrument(skip_all, ...)]`, module targets and direct safe
fields; preserve normal control flow and error propagation.

| Instrumentation | Diagnostic purpose and level |
| --- | --- |
| `geography.build` / `geography.publish` spans | `info`: operator-requested build/publication attempts, release identity and recipe digest. Span presence does not claim success. |
| `geography.source.fetch` span | `info`: acquisition of a pinned source, source ID and expected digest; no URL query/authentication dumps. |
| Compile/verify/open spans | `debug`: identify failing source/layer/operation and expensive work; include release/source keys where available. |
| `geography.lookup` span | `debug`: release identity and query form only; omit coordinates, locators and result payload dumps. |
| `geography.bundle_published` event | `info` only after pointer replacement: release/manifest identity and committed feature/omission counts. |
| `geography.reviewed_omissions_applied` event | `info` summary: release identity, counts by fixed field and stable omission IDs. Expected accepted omissions are not repeatedly warned per feature. |
| `geography.bundle_rejected` event | Handling-boundary `error`: safe category, source/layer/feature/operation references, before returning failure. No repeated lower-level logging of the same error. |

Domain metrics: `clacks_geography_build_runs_total{outcome}` counts handled build
invocations once (`published`, `unchanged`, `failed`, `cancelled`);
`clacks_geography_build_duration_seconds{outcome}` measures that workflow;
`clacks_geography_lookup_duration_seconds{query_kind,outcome}` measures queries
(`point`/`area`, `ok`/`error`); and
`clacks_geography_field_lookups_total{field,coverage,candidate_class}` counts
completed field decisions (`zero`/`one`/`multiple`). Do not emit partial-result
metrics for a query returning error. Build completion is owned by the builder's
workflow boundary; standalone verify/fetch are not counted as builds.

No release IDs, coordinates, paths, labels, arbitrary source IDs or diagnostics
are metric dimensions. Use framework span timing and ordinary histogram recording
at their owning boundaries; no generic observer/Completion struct or synthetic
span-exit events. Record errors once with deliberate safe diagnostics, preserving
underlying error chains for the caller without logging credentials or arbitrary
provider bodies. Instrumentation must distinguish staged data from publication.

## 10. Acceptance evidence

| Area | Required representative evidence |
| --- | --- |
| Administration | Adjacent counties and state borders; Connecticut's eight real county features from 2021 alongside 2026 inputs. |
| Coordinates | Each selected operation, explicit no-shift components, grid edges and date-line equivalents; independent numerical evidence rather than round trips alone. |
| Corrections | At least one reviewed replacement for a real defective ITU polygon. All other included defects corrected or explicitly excluded. Source-guard mismatch stops build. |
| DXCC | Ordinary mapping, a sourced exclusion/replacement case and a truthful gap; the excluded region never silently inherits its parent. |
| Queries | Shared-boundary points, edge/corner-only contacts, small positive-area overlap, holes, multiple same-value fragments and uncovered portions at all supported locator sizes. |
| Publication | Injected parse/grid/geometry/mapping/checksum/write/replacement failures preserve the existing selected release. Unsupported schema and mixed bundles reject. |
| Observability | Real builds/queries report truthful outcomes and useful safe context; filtered debug spans do not erase publication/rejection context. |

Real-source cases traverse ingestion, compilation, FlatGeobuf output and production
reading/querying. Select representative features with enough neighbors to exercise
the intended borders; do not create apparently real administrative boundaries by
cropping them to artificial test rectangles. The recipe declares the selected
scope; outside it, coverage is not invented. Independently authored Rust fixtures
supplement exact numerical/topological edge cases. Expected values require cited
source rules or independently calculated geometry, not the production answer
copied into a fixture. No Python generation, executed TQSL, native geographic
information system utility or
alternate test-only reader is used.

Standard tests are offline. Source-cache-dependent acceptance runs use documented
Cargo tests/CLI commands and the pinned downloads; they are mandatory delivery
evidence even if excluded from ordinary fast tests. Record dependency/features,
licenses/notices and actual representative coverage in crate documentation when
implemented. Preserve measured limitations instead of claiming this specification
establishes compatibility, complete crosswalks or source accuracy.

## 11. Engineering boundaries

The pipeline uses manifest indirection, scoped value keys, immutable
directory/pointer publication, offline build versus explicit fetch commands,
`f64` geometry and the stated numerical subdivision criterion. These are
engineering choices within the accepted product requirements. Private module/type
names and bounded resource limits may evolve without changing caller-visible contracts.

The [representative recipe](../references/geography/representative.json) names
source selections, guarded replacements, explicit mappings, operation masks and
independently justified acceptance cases. Its [data documentation](../references/geography/README.md)
records coverage and authorship. The shipping recipe expands this representative
scope; unsupported areas remain explicit omissions in its maintained notes.

Provenance records retain human-readable descriptions and structured `definition`
parameters separately. These describe resolved production rules; the reader does
not execute them.

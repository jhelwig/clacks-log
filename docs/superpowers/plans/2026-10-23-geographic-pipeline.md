# Geographic Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` for native execution, or `superpowers:subagent-driven-development` if the user selects that method. Implement task-by-task; read this plan and its specification before execution. Steps use checkboxes for tracking.

**Goal:** Deliver issue #5's reusable Rust geographic-data producer, coherent FlatGeobuf bundles, production point/area reader, and representative real-source acceptance evidence.

**Architecture:** `clacks-geography` owns artifact contracts and read-only queries; `clacks-geography-builder` owns pinned-source ingestion, compilation, validation and publication. A thin `clacks-geography-build` application delegates to the builder. Issue #23 extends these components and coverage rather than replacing them.

**Tech stack:** Rust 1.99.1; Serde; `geo` 0.33.1; FlatGeobuf 6.0.1; `geozero` 0.15.1; `shapefile` 0.9; `proj4rs` 0.2.1; Rustls-backed source fetching; Rust-backed archive decoding; standard filesystem locking and `tempfile` publication.

**Spec:** [Geographic data production and lookup](../../geographic-pipeline.md). Source evidence lives in [the source comparison](../../../references/location-lookup.md) and [coordinate operations](../../../references/geographic-coordinate-operations.md). The historical audit is source provenance, not an executable oracle.

## Global constraints

- Implement production components only. No alternate demonstration reader, test-only public API, plugin framework or general task dispatcher.
- Rust 1.99.1, edition 2024; library crates under `lib/`, thin applications under `bin/`; inherit workspace package/dependency settings. No `mod.rs` outside tests.
- Project-owned tooling and nontrivial fixture generation use Rust. Do not execute Python, native geographic utilities, JavaScript source wrappers or TrustedQSL reference sources.
- Keep the reader free of network, source-archive and transformation dependencies. No native PROJ, geographic bindings, OpenSSL-backed fetching or external spatial database.
- Use whole grid footprints and explicit coordinate points from `clacks-model`; never substitute grid centers. Keep candidates and complete/partial/none coverage independent per field.
- Preserve boundary-point ambiguity, ignore edge/corner-only area intersections, retain every positive-area alternative, and never infer a value across uncovered area. Do not add a largest-overlap or sliver threshold.
- Native snapshots, recorded positions and manual corrections are untouched. This issue supplies lookup results; #23 owns application integration and complete U.S. coverage.
- Source rights/notices remain distinct from the application's Elastic License 2.0. Every included transformation/correction/mapping/omission has provenance.
- Bundle schema version is `1`; release identity is separate. All five layer roles exist, including verified empty layers. Publication changes the current pointer only after production-reader verification.
- Follow `AGENTS.md` and the [observability contract](../../observability.md), including standalone rustdoc, normal error propagation, `thiserror` categories, `anyhow` context, no unchecked production indexing/panics, and no telemetry lifecycle abstractions.
- This plan is documentation, not evidence that dependencies have compiled or geographic expectations have passed. Do not close #5 before real-source acceptance and platform verification.

## Review focus

Each risk below has an owning test task; these are engineering checks, not new product decisions.

1. Numerical overlay loses a narrow overlap/gap when a large feature meets a twelve-character grid: Task 2 must preserve the decision or return an explicit numerical failure, never confident wrong coverage.
2. Mixed datum-operation regions create seams or double-transform shared vertices: Task 4 checks shared borders, operation assignments and grid-edge failures.
3. A reviewed correction applies to changed upstream coordinates: Tasks 3/5 reject source/feature-guard mismatch before geometry processing.
4. An old reader is active while publication fails or succeeds: Task 7 preserves its release and changes new readers only at the pointer commit point, including Windows replacement failure.
5. A checksum-valid but semantically malformed artifact looks like empty coverage: Tasks 1/2 reject inconsistent counts, references, geometry and indexes instead of returning unknown.

## File and dependency map

Create these files when their owning task introduces behavior; do not add empty placeholders.

```text
lib/clacks-geography/
  Cargo.toml, README.md
  src/lib.rs, error.rs
  src/artifact.rs             schema, provenance and semantic validation
  src/artifact/read.rs        verified immutable bundle handles
  src/artifact/write.rs       deterministic artifact encoding used by the producer
  src/geometry.rs             shared producer/reader topology validation
  src/lookup.rs               native query conversion and per-field results
  tests/artifact.rs, lookup.rs, observability.rs
  tests/support/mod.rs        private Rust-authored geometry fixtures
lib/clacks-geography-builder/
  Cargo.toml, README.md
  src/lib.rs
  src/recipe.rs               build inputs, selectors and validation
  src/source.rs
  src/source/cache.rs, archives.rs, shapes.rs, zones.rs
  src/compile.rs
  src/compile/coordinates.rs, normalize.rs, operations.rs, overlay.rs, corrections.rs, mappings.rs
  src/publish.rs              staging, lock and pointer commit
  src/application.rs          CLI composition, cancellation and workflow telemetry
  tests/sources.rs, publication.rs, real_sources.rs
  src/compile/tests.rs        private compilation regression coverage
  src/compile/*/tests.rs      private numerical/assignment tests
  src/application/tests.rs   command composition and reporting
  tests/support/             Rust source/geometry fixture constructors
bin/clacks-geography-build/Cargo.toml, src/main.rs
references/geography/
  representative.json        pinned recipe; explicit representative scope
                             includes mappings, operations and acceptance cases
  itu-2.json                 complete reviewed replacement, not edit instructions
  athos-review-area.json     conservative authored exclusion
  README.md                  source/correction/omission rationale and reproduction
                             source notices are literal recipe data
```

Modify root `Cargo.toml`/`Cargo.lock`, `.gitignore` for local source/output directories, README commands, and owning source/spec notes where delivered behavior needs clarification. Do not change native model types merely to ease fixture construction. Do not add completed-status copies of GitHub issues.

`artifact/write.rs` is a real producer API in the same format-owning crate; it avoids duplicating format invariants in the builder. It is not a test fixture writer facade. Private construction helpers remain private. Decode/encode types use ordinary Serde derives; keep public APIs in owning modules without facade re-exports.

## Interface vocabulary and execution limits

Concrete names below are plan choices. Metadata row fields are those in specification sections 4/6, not a second competing schema. Use sorted maps/sets or explicit sorting wherever encoded ordering affects hashes.

- `artifact::Field::{Dxcc, PrimarySubdivision, SecondarySubdivision, CqZone, ItuZone}`; typed zone result values use existing `location::{CqZone, ItuZone}`.
- `artifact::Manifest`, `ValueEntry`, `Derivation`, `SourceIdentity`, `LayerDescriptor`, `FileDigest`, and `CurrentRelease` correspond directly to the specification. Scope is structured (`Global`, `Entity(String)`, `Subdivision { entity, primary }`), not a concatenated string requiring later reparsing.
- `artifact::CompiledFeature { part_id, value_key, derivation_key, geometry: geo_types::MultiPolygon<f64> }`; `LayerInput { field, features }`; `BundleContents { manifest, layers, notices }`. Geometry is already normalized; the writer rejects invalid content rather than repairing it.
- `artifact::read::Bundle` retains the validated manifest and pinned release directory/files. `lookup::LookupResult` has `release_id` and five typed `FieldLookup<T>` members; `Coverage::{Complete, Partial, None}`; `Candidate<T> { value, parent_scope, display_label, source_identities, derivation_keys }`.
- `recipe::BuildRecipe` includes typed source selection, coordinate-operation, correction, mapping, exception, omission and acceptance rows. `SourceSelection` has an explicit source ID, identity predicate and expected match count. No display-name-only or positional matching.
- `source::SourceFeature { identity, properties, geometry, coordinate_declaration, feature_sha256 }`; `DecodedSources` contains selected features keyed by source identity. Feature guards hash compact deterministic JSON of the original geometry/properties before normalization, with sorted property keys and preserved coordinate order.
- `compile::Compilation` owns contents ready for encoding and diagnostic seam reports; the recipe owns acceptance cases. `publish::BuildReport` distinguishes `Published`, `Unchanged`, and `PublishedWithCleanupWarning`; cancellation is a classified error before publication, never an unchanged claim after publication.
- Reader failures use `error::GeographyError` categories: unsupported schema, invalid artifact, invalid query, numerical failure and read failure. Category variants include useful field/path/reference context and source errors where applicable. Producer boundaries return `anyhow::Result` with source/feature/operation context; use a small typed build-cancelled error where classification is needed. Do not add forwarding enum variants for unrelated dependencies.
- A borrowed `&AtomicBool` cancellation flag belongs to real fetch/build workflows. Check before expensive stages, during bounded record/segment loops, and immediately before committing the pointer. No cancellation framework or persisted job state.

Set explicit recipe limits: per download 512 MiB, each expanded member 1 GiB, archive total 2 GiB, 128 members, geometry nesting 8, selected coordinates 20 million, subdivision depth 24 and at most one million generated vertices per feature. Reject over-limit inputs with contextual errors. These are resource limits, not geometry simplification allowances; exceeding one does not turn data into an accepted omission. The plan's representative recipe can use lower limits per source. Query overlays are bounded by selected feature/coordinate totals; do not silently truncate candidates.

## Representative real-source cases

These define the delivery target. Record exact digests/selected feature guards through the Rust source path in Task 3; keep the already researched archive hashes and upstream commits unchanged. Names below aid review; selectors use source keys.

| Purpose | Selected input/case | Expected evidence |
| --- | --- | --- |
| Adjacent counties/state border | 2026 Washington `STATEFP=53`, Oregon `41`; Washington counties King `53033`, Pierce `53053`, Snohomish `53061`, plus Clark `53011` and Oregon Multnomah `41051` | Seattle point → `WA` and `WA,King`; neighboring county interiors resolve independently. A real shared-edge query exercises candidates without invented clipped county boundaries. |
| Connecticut | 2026 state `09`; all eight 2021 county `GEOID`s `09001` through `09015` in odd increments; exclude all 2026 Connecticut county-layer rows | New Haven → `CT,New Haven`; exactly eight old county identities, no planning-region candidates; vintage seam/gap measured and disclosed. |
| Mainland/Alaska | Washington and Anchorage `02020` | EPSG:1188; independent zero-translation calculation, full original state/county parts retained. |
| Aleutian/date line | Alaska state `02`, Aleutians West `02016`, with Unalaska and islands on both sides of 180° | EPSG:1251 only within reviewed onshore assignment; declared no-shift elsewhere; no seam duplication of values. |
| Hawaii | Hawaii state `15`, Honolulu `15003` | EPSG:1252 main-island extent; named remote components use D259 approximation. Honolulu resolves its actual subdivision. |
| Caribbean | Puerto Rico `72`/San Juan `72127`; Virgin Islands `78`/St. Thomas `78030` | EPSG:1731 onshore, independently evaluated grid nodes/cells, explicit offshore approximation, no optional-grid fallback. |
| Pacific approximation | Guam state `66`, Northern Mariana Islands `69`, American Samoa `60` | Numerically unchanged angular coordinates with explicit source datum and unestablished-accuracy provenance. |
| Zones | All 40 source CQ records; selected ITU `1`, repaired `2`, `6`, `8`, `27`, `45`, `56`, `59`, `60`, `61`, `62` | Seattle 3/6, New Haven 5/8; Auckland 32/60, Suva 32/56, Apia 32/62 exercise date-line geometry. Omit unsupported other ITU records explicitly, including known defective records. |
| Ordinary DXCC | Natural Earth `SU_A3=USB`, `USK`, `USH`, `ENG`, `SCT`, `GRC` | Interior points map to the existing catalog's U.S., Alaska, Hawaii, England, Scotland and Greece values; mapping guards verify actual pinned keys before compiling. |
| Exclusion/gap | Greece mapping minus the authored Athos review region described below | A Mount Athos point is unknown, never inherited Greece; an Athens interior remains Greece. Runtime has no exception-rule engine. |
| Island source path | Pinned Natural Earth minor-islands source, selected through original attributes plus geometry digest | Parse and retain its real source identity; an unmapped selected island is a reviewed omission, never guessed from proximity or a nearest parent. |

Do not require complete world or national coverage to finish #5. Outside the selected scope, omit values honestly. A representative source is not permission to crop administrative polygons into false borders. Small synthetic geometries below supplement real source cases; they do not replace them.

### Concrete data-authorship choices

**ITU 2:** The pinned source begins `[-141.00403,80.00002]` and ends `[-141.00403,79.99907]`. Closing the latter back to the former introduces the recorded northern self-intersection. Author the complete replacement ring by retaining all original vertices except that last near-start vertex, then closing directly to the original first vertex. Preserve the other original coordinates; do not run a generic `make_valid` operation. Cite the pinned source and record both the original digest and the rationale for removing this closure overshoot. Task 5 must verify that the replacement is topologically valid, preserves the intended interior, and does not make an unsupported boundary claim; failure stops this case for review rather than silently choosing another repair. Other included defective polygons require their own reviewed replacements or explicit omission.

**Athos:** The [ARRL statement](https://www.arrl.org/news/arrl-dxcc-statement-on-mount-athos-operation-sv1ga-a) establishes that Mount Athos is a separate entity; the [UNESCO property/map page](https://whc.unesco.org/en/list/454/maps/) locates the peninsula. Author a deliberately conservative exclusion rectangle, longitude `[23.9,24.5]`, latitude `[40.1,40.6]`, subtracting its intersection with Greece. This is an explicitly disclosed *review area with no classification*, not the Mount Athos jurisdiction/entity boundary and not a polygon assigning DXCC 180. It intentionally sacrifices surrounding Greece coverage until a supported boundary/replacement exists. Do not copy the UNESCO map artwork or trace its polygon without separately establishing redistribution rights. Use its published location as verification context; the simple uncertainty mask is Clacks-authored. Verify `(40.266670,24.216670)` returns no DXCC and Athens `(37.983810,23.727540)` remains Greece. Supplemental synthetic cases prove the optional replacement path; no invented real entity replacement is shipped.

**Operation assignments:** Registry bounding rectangles alone cannot distinguish onshore/offshore. For representative Aleutian and Caribbean counties, use Census 2026 topological-face polygons in the same declared NAD83 coordinate system as support data: `FACES/tl_2026_02016_faces.zip`, `tl_2026_72127_faces.zip`, `tl_2026_78030_faces.zip`. The [technical documentation](https://www2.census.gov/geo/pdfs/maps-data/data/tiger/tgrshp2026/TGRSHP2026_TechDoc.pdf), section 4.21 and Appendix R, defines `TFID` and `LWFLAG`. Verify the land flag encoding against the actual source metadata before selecting land faces; reject unknown flag values. These are additional Census-family build inputs for assignment geometry, not new output subdivisions or an alternative geographic source family. Pin their declarations/digests/notices through Task 3. Intersect land faces with reviewed operation applicability; build a disjoint source-coordinate assignment partition and name remaining approximation regions explicitly. Preserve the complete original administrative polygon, including water. No WGS84 GeoJSON is relabeled NAD83. If inspected face metadata cannot establish the assignment, stop that assignment for source review; do not extend an onshore operation offshore.

Pin the following ordinary point cases as native microdegrees in the recipe's `acceptance_cases`: Seattle `(47.606200,-122.332100)`, New Haven `(41.308300,-72.927900)`, Honolulu `(21.306900,-157.858300)`, Anchorage `(61.218100,-149.900300)`, Auckland `(-36.848500,174.763300)`, Suva `(-18.124800,178.450100)`, Apia `(-13.833300,-171.750000)`, London `(51.507400,-0.127800)` and Edinburgh `(55.953300,-3.188300)`. These pairs are **latitude, longitude**, matching the native constructor; geometry coordinates remain **longitude, latitude**. Assert only fields for which the selected sources/rules establish the expectation; coastal zone probes do not imply a Natural Earth land match. Include native `CN87` as the agreed area case (CQ 3/ITU 6) and derive smaller supported cells through the native locator parser, not a competing encoder.

## Task 1: Validated bundle encoding and immutable reading

**Files:** Root manifests; reader crate manifest, `src/lib.rs`, `error.rs`, `artifact.rs`, `artifact/read.rs`, `artifact/write.rs`, `geometry.rs`; `tests/artifact.rs`, `tests/support/mod.rs`; crate README.

**Interfaces:** Produces:

```rust
artifact::write::write_bundle(root: &Path, contents: &BundleContents)
    -> anyhow::Result<Manifest>
artifact::read::open_bundle(root: &Path) -> Result<Bundle, GeographyError>
Bundle::manifest(&self) -> &Manifest
```

The writer fills payload hashes/sizes and writes the manifest last. It never publishes a current pointer. Reject an existing nonempty destination; publication supplies a private staging directory. The reader uses verified open files for its lifetime; queries seek/read those handles, rather than reopening mutable paths. Serialize access as needed through ordinary borrowing (`lookup` below takes `&mut self`); no custom pool/cache layer. Retaining handles plus immutable release discipline protects ordinary publication; do not claim protection against hostile concurrent in-place file mutation by an administrator.

- [x] **Step 1:** Add failing artifact tests: deterministic round trip with all five roles; zero-feature layers; unknown schema; wrong coordinate convention; duplicate/absent roles, values, parts and provenance references; unsupported columns/type/dimensions; NaN/infinite/invalid polygon; missing index on a nonempty layer; wrong counts/bounds; truncated files; checksum mismatch; missing notice; absolute/traversing/symlink paths; colliding release IDs. Assertions distinguish `InvalidArtifact` from empty valid coverage and retain `source()` for read errors.
- [x] **Step 2:** Run `cargo +1.99.1 test -p clacks-geography --test artifact`; confirm tests fail because behavior is missing. Add real crate behavior, declarations and dependencies together, not a placeholder-only commit.
- [x] **Step 3:** Implement the schema, writer and reader. Use finite 2D `MultiPolygon`, exact property names from spec, explicit longitude-first metadata and an index for every nonempty layer. Validate the full feature stream on open, including geometry and reference invariants, then retain seekable handles. Empty files bypass index search. Normalize winding consistently before nonzero-fill overlays; do not repair invalid input here.
- [x] **Step 4:** Declare narrow workspace dependencies: `geo` defaults off; `flatgeobuf` defaults off; `geozero` defaults off with `with-geo`; compatible `geo-types`; SHA-256 using the existing workspace dependency alias appropriate to the 0.10 API. Select a current compatible `tempfile` in the lockfile. Inspect `cargo tree -e features` to confirm no source/network/native geographic dependencies in the reader. Copy no dependency implementation into Clacks.
- [x] **Step 5:** Run artifact tests with `--locked --offline`, formatting and crate Clippy. Record schema/usage/error guarantees in reader README/rustdoc. Commit with a subject and body explaining why bundles are validated and pinned as a unit.

## Task 2: Production point/area queries and numerical acceptance

**Files:** `src/lookup.rs`, `geometry.rs`, reader module declarations; `tests/lookup.rs`, support and README.

**Interfaces:** Consumes `Bundle`; produces:

```rust
Bundle::lookup(&mut self, geometry: clacks_model::position::LookupGeometry)
    -> Result<LookupResult, GeographyError>
```

Private geometry helpers convert rational bounds and perform scoped unions/intersections. No new public coordinate types or synthetic feature query API.

- [x] **Step 1:** Write failing read-back tests for boundary points, area edge/corner contacts, small positive overlaps, uncovered holes, overlapping same-value fragments, equal labels in different scopes and duplicate date-line fragments. Build fixtures through the production writer. Assert the exact candidates/coverage from spec, never summed-area percentages.
- [x] **Step 2:** Add native locator tests for lengths `2,4,6,8,10,12` at equator, high latitude and ±180°. Invalid publicly constructible rational bounds (zero denominator, inverted/out-of-range edges) return `InvalidQuery`. Pole points aggregate equivalent longitude sectors; an unsupported cap returns unknown. Failed layer reads return errors without partial field metrics.
- [x] **Step 3:** Run `cargo +1.99.1 test -p clacks-geography --test lookup`; confirm failure. Implement indexed bounding-box selection followed by `geo::Relate` closure/2D-intersection tests, explicit `FillRule::NonZero` unions and remainder coverage. Group by field/scope/value and sort deterministically; collect all contributing provenance. Do not use `contains` alone for boundary points or default even/odd fill for overlapping fragments.
- [x] **Step 4:** Pin numerical tests with analytical rectangles: at longitude 179°, a large containing polygon and a local twelve-character cell, add/remove strips of widths `1e-6`, `1e-8` and `1e-10` degrees. Also test a same-value overlap of those widths. Use query-local translated/windowed coordinates for overlays where necessary, preserving exact source segments without simplification. Compare source topology (`Relate`) with overlay output to detect a lost intersection/gap; return `NumericalFailure` on a contradiction rather than guessing complete coverage. Record observed supported resolution and any conservative failure cases in the README. If `geo` cannot implement the required result even with a suitable local window, stop and report that concrete limitation before changing libraries or weakening D260.
- [x] **Step 5:** Run artifact/lookup tests and Clippy with locked offline dependencies. Add the lookup span and result metrics from spec using normal control flow; Task 8 verifies reporting through real operations. Commit the working production query path with context in the message body.

## Task 3: Pinned recipes, acquisition and source decoding

**Files:** Builder manifest, `src/lib.rs`, `recipe.rs`, `source.rs` and children; `tests/sources.rs`; recipe/source/notices files under `references/geography/`; thin binary and initial `application.rs` fetch command.

**Interfaces:** Produces:

```rust
recipe::read_recipe(path: &Path) -> anyhow::Result<BuildRecipe>
source::fetch_sources(recipe: &BuildRecipe, cache: &Path, cancelled: &AtomicBool)
    -> anyhow::Result<FetchReport>
source::decode_sources(recipe: &BuildRecipe, cache: &Path, cancelled: &AtomicBool)
    -> anyhow::Result<DecodedSources>
application::run() -> anyhow::Result<()>
```

`FetchReport` reports installed/reused source counts. `application::run` reads command arguments; private argument types need no public testing constructor. Cache blobs are digest-named regular files installed only after checking bytes/hash. Decoding validates pins again; cached existence is not proof of integrity.

- [x] **Step 1:** Write failing tests for valid small Rust-authored shapefile/attribute input, multipart/hole preservation, mismatched shape/attribute counts, wrong `.prj`/encoding, absent/duplicate selectors, truncated streams and unexpected feature kinds. Test zone-wrapper braces inside strings, escapes, trailing code, a second assignment and changed collection counts. Wrapper extraction never evaluates JavaScript.
- [x] **Step 2:** Add archive/cache tests for duplicate member names, traversal, absolute/symlink entries, unlisted shapefile sidecars, byte/count expansion limits, bad digest, interrupted fetch and cache corruption. Run `cargo +1.99.1 test -p clacks-geography-builder --test sources`; confirm failure.
- [x] **Step 3:** Implement bounded ZIP/attribute decoding and the exact pinned `var cqZones =`/`var ituZones =` collection extraction with a string-aware brace scanner plus Serde JSON. Decode source keys/properties before filtering; retain source coordinate declarations and canonical feature guards. Known unclosed HA8TKS rings are listed explicitly from the pinned input; do not close all malformed lines indiscriminately.
- [x] **Step 4:** Add Rustls-only `ureq` 3.5 and a compatible `zip` release with defaults off and a Rust Deflate backend; lock actual versions/features and verify no OpenSSL/native geographic linkage. Use `shapefile` 0.9 with only the encoding feature required by inspected files. Fetch is explicit, bounded, timed out and digest-checked. `build`/`verify` later never invoke it. Use local loopback HTTP only in acquisition tests; ordinary tests require no public network.
- [x] **Step 5:** Populate the representative recipe from the previously researched pins/digests and acquire source metadata for the additional three Census face archives. Record the exact `LWFLAG` encoding, original feature IDs, member lists, hashes, declarations and notices through this Rust path. Preserve MIT notices for HA8TKS, Census attribution, Natural Earth acknowledgment and NOAA grid metadata. Record unselected/known defective ITU IDs as explicit omissions. Missing source evidence is an error, not a guessed default.
- [x] **Step 6:** Provide standard `--help` and `fetch --recipe <path> --cache <path>` via `clap` in application composition; thin `main` calls only `application::run`. Document Cargo invocation. Run source/application tests and locked feature-graph checks; commit the usable ingestion/fetch path and pinned inputs with a substantive message.

## Task 4: Coordinate operations, assignment partition and normalization

**Files:** `src/compile.rs`, `src/compile/coordinates.rs`, `normalize.rs`, `operations.rs` and private unit-test modules; operation rows in `references/geography/representative.json` and source rationale.

**Interfaces:** Produces private compilation entry points:

```rust
prepare_operations(definitions: &[CoordinateOperation], sources: &DecodedSources)
    -> anyhow::Result<PreparedOperations>
transform_features(features: &[&SourceFeature], operations: &PreparedOperations,
                   cancelled: &AtomicBool)
    -> anyhow::Result<BTreeMap<SourceIdentity, TransformedFeature>>
```

`TransformedFeature` retains its identity/guard, normalized geometry and operation derivations; it does not assign output values. `PreparedOperations` owns a validated disjoint assignment partition in the source coordinate system.

- [x] **Step 1:** Write numerical tests at Seattle `(-122.33,47.60)`, Unalaska `(-166.53,53.87)`, Honolulu `(-157.86,21.31)`, San Juan `(-66.10,18.46)` and the corresponding applicable regions. Independently evaluate geodetic-to-geocentric translation using GRS80/WGS84 ellipsoids and zero input ellipsoidal height; compare 1188 `(0,0,0)`, 1251 `(-2,0,4)`, 1252 `(1,1,-1)` within `1e-9` degrees. Test axis order, radians and translation sign explicitly; round trips alone are insufficient.
- [x] **Step 2:** Independently decode the pinned GeoTIFF's sample grid through the Rust `tiff` API in test support, identify an interior node and four-node cell containing San Juan, and calculate node/bilinear expectations from metadata/sample values. Verify channel order, arc-second units, positive-east sign and shift direction against source documentation; compare `proj4rs` output within `1e-9` degrees. Include exact grid edge, outside-grid, missing/corrupt grid and required-grid lookup failures. This is independent expected-value arithmetic, not a second production transform path.
- [x] **Step 3:** Run coordinate tests to confirm failure. Add `proj4rs` 0.2.1 defaults off with `tiff`, using explicit projection parameters. Load only the verified grid bytes via `nadgrids::catalog::with_catalog` and `nadgrids::formats::read`, keyed by digest. Do not use the environment-searching file finder, optional grid names or `@null`. Register the pinned grid once with standard synchronization and immutable digest keys; concurrent builds cannot replace it. Verify that repeated and differently ordered builds cannot use another grid of the same filename.
- [x] **Step 4:** Author the source-coordinate assignment partition using the named Census faces and applicable regional extents. Apply 1188 to supported mainland components; 1252 inside main-island applicability; 1251/1731 only to the reviewed onshore masks. Assign the specifically identified remaining components/areas D259 no-shift in advance. Test zero/multiple assignment, overlaps, uncovered assignment area, shared vertices and mismatch across adjacent features. A failed required operation never falls through to no-shift.
- [x] **Step 5:** Implement splitting at assignment boundaries and adaptive segment subdivision. Use quarter/midpoint output deviation at most `0.01 m`, max depth `24` and the stated vertex bound; no-shift/identity preserves straight source edges. Batch partitioning must node shared collinear source edges at common endpoints before projection. Shared source edges within one operation must use the same deterministic subdivision decisions independent of feature direction and original segmentation. Noding consumes the existing feature vertex budget. Across different operations preserve each result exactly and report the resulting seams under D269, without forcing coincident boundaries. Test reversed and differently segmented shared edges, native twelve-character coverage across the common boundary, fractional assignment borders with seam reporting, iteration-order independence and failure when the budget cannot be met.
- [x] **Step 6:** Normalize date-line geometry by continuity-preserving unwrap, strip clipping and translation; keep holes attached. Test source longitude outside ±180°, both seams, ambiguous half-world/closure rejection, documented source ring closure and unsupported polar limits. Non-finite transforms and invalid transformed topology stop compilation. Run coordinates/source tests and commit, documenting computational tolerances separately from source accuracy.

## Task 5: Reviewed corrections, value mappings and compiled exceptions

**Files:** `compile/corrections.rs`, `compile/mappings.rs`, private `compile/tests.rs`; authored recipe/correction/exclusion files and source rationale.

**Interfaces:** Produces:

```rust
prepare_corrections(recipe: &BuildRecipe, features: &[SourceFeature])
    -> anyhow::Result<Vec<Replacement>>
compile_mappings(recipe: &BuildRecipe,
                 features: &BTreeMap<SourceIdentity, TransformedFeature>,
                 corrections: &BTreeMap<SourceIdentity, String>,
                 cancelled: &AtomicBool) -> anyhow::Result<MappedContents>
```

Invalid originals remain `SourceFeature` inputs until their guarded target-WGS84 replacement is selected. They never become a supposedly valid `TransformedFeature` before replacement and never reach source-datum conversion or overlays.

- [x] **Step 1:** Write failing tests for full replacement, original guard mismatch, absent/multiple source matches, invalid replacement, missing/cyclic exception references, invalid zone values and inconsistent scoped labels. Assert source identity/operation/correction/rule references survive into derivations. Test two nonoverlapping same-label counties in different parent scopes remain separate candidates.
- [x] **Step 2:** Add an analytical synthetic parent square with an interior exclusion: exclusion-only creates a hole/partial coverage; a supported replacement assigns another value; overlapping exceptions cannot silently depend on input order. Do not publish this square as a real DXCC dataset.
- [x] **Step 3:** Run compilation tests to confirm failure. Implement explicit mappings and guarded geometry replacement/subtraction/addition using valid geometry, normalized winding and nonzero unions. Use checked nonzero-fill overlays for mapping subtractions and assignment intersections/differences: restore original vertices and source intersection nodes from the binary lattice; reject unresolved nodes, changed turns and ambiguous output intersections. Verify narrow exclusions through production point queries and confirm an unresolvable exclusion preserves the selected release. Compile Census-derived CQ assignments where the published written state definitions apply; exclude the overridden detailed U.S. state area from the coarse CQ source before inserting it. Preserve unrelated world-zone geometry. No runtime precedence rule remains.
- [x] **Step 4:** Author the complete ITU-2 replacement and Athos conservative exclusion specified above. Include rationale, source guards, citations and notices; validate the replacement and retain independent interior/border expectations. Record the mask's knowingly broader omission area. Do not label the Athos uncertainty rectangle a jurisdiction boundary or assign entity 180 from it.
- [x] **Step 5:** Populate the exact representative Census/zone/DXCC mappings. Verify expected Natural Earth selectors against real attributes; zero/multiple matches stop the build rather than falling back to a display name. Derive factual DXCC values from the existing catalog without executing reference code; do not depend on `clacks-lotw`. Check the selected minor-island path preserves identity and reports an unmapped omission honestly.
- [x] **Step 6:** Run compilation/coordinate/query regression tests. Commit the production rule compilation and reviewed inputs with a body explaining unsupported-area handling and conservative coverage loss.

## Task 6: End-to-end compilation and recipe acceptance

**Files:** `compile.rs`, `recipe.rs`, builder README; `tests/real_sources.rs`, acceptance rows in `references/geography/representative.json`, supporting source/rationale files.

**Interfaces:** Produces:

```rust
compile::compile_bundle(recipe: &BuildRecipe, cache: &Path,
                        cancelled: &AtomicBool) -> anyhow::Result<Compilation>
compile::verify_bundle(bundle: &mut Bundle, recipe: &BuildRecipe)
    -> anyhow::Result<AcceptanceReport>
```

`AcceptanceCase` supports native point/grid queries and explicit expected candidates/coverage for each specified field. Point coordinates are native integer microdegrees; locator expectations use native parsing. Cases can also require included/omitted source identities and recorded operation/correction references. `AcceptanceReport` reports checked-case and reviewed-omission counts; failure retains the case ID and actual/expected safe diagnostics.

- [x] **Step 1:** Add a tiny offline source-backed Rust fixture scenario traversing decoding → operations → corrections/mappings → production writer → reader → queries. This tests composition, not a fake substitute for the mandatory external-cache source run. Add fail-before-output cases for invalid recipes, operation/mapping/correction failures and cancellation.
- [x] **Step 2:** Run compilation integration tests to confirm failure. Implement the exact processing order in spec section 5, then encode and reopen through the production reader. Verify recipe references/resource limits before expensive work. Deterministically order feature/value/derivation/notice records; exclude current time, cache/output absolute paths and random IDs from bundle bytes.
- [x] **Step 3:** Author real acceptance cases for the named table, including source internal points and published administrative identities, shared-edge/whole-grid cases, the documented zone city expectations, CT eight-county count, no planning regions, all selected operations, and omission provenance. Pin the actual case IDs, source feature IDs and independent numerical expectations in the checked-in acceptance file. Do not copy query output into expected data or use historical Python results as the sole authority. For a historical sample, independently justify its expected value from source/rules before accepting it.
- [x] **Step 4:** Add `#[ignore = "requires pinned geographic source cache"]` real-source tests reading `CLACKS_GEOGRAPHY_CACHE`. They must fail with a useful missing-cache message when explicitly run without inputs. Run the exact real-source suite with the explicit cache and offline dependencies. Verify that complete features are retained and output coverage outside declared selection is not invented. Investigate source-vintage seams; a genuine gap produces partial/none rather than buffered geometry.
- [x] **Step 5:** Build twice with identical pins/release metadata but different input iteration and cache/output directories. Assert all manifest/payload bytes match. Run all reader/builder tests; commit the end-to-end production compilation and acceptance expectations with reproducibility context.

## Task 7: Atomic publication and complete command-line workflow

**Files:** `publish.rs`, `application.rs`, thin executable; `tests/publication.rs`, `tests/application.rs`; root command documentation.

**Interfaces:** Produces:

```rust
publish::build_and_publish(recipe: &BuildRecipe, cache: &Path, output: &Path,
                          cancelled: &AtomicBool) -> anyhow::Result<BuildReport>
artifact::read::open_current(output: &Path) -> Result<Bundle, GeographyError>
```

`open_current` resolves a single pointer, checks its manifest digest and delegates to verified bundle opening. It never chooses an unreferenced directory. It is a real packaged-data convenience, not a second artifact reader.

- [x] **Step 1:** Write failure-preservation tests against an existing good release: parse, missing grid, invalid geometry/mapping, acceptance failure, encoding/write error, checksum mismatch and pointer replacement failure all retain the old pointer bytes and query results. Use private test injection only at filesystem write/commit boundaries where real permissions cannot deterministically exercise the branch; do not introduce a public filesystem/backend interface for testing.
- [x] **Step 2:** Add tests for identical release reuse (`Unchanged`), changed bytes under the same release ID (error), two concurrent publishers (one serialized lock boundary), stale staging/unreferenced releases, cancellation at stage boundaries, already-open old reader after success, and cleanup failure after commit (`PublishedWithCleanupWarning`). Test cancellation observed after the commit cannot misreport failure or roll back the pointer.
- [x] **Step 3:** Run publication tests to confirm failure. Implement `File::lock` on a stable lock file, same-filesystem `tempfile` staging, production-reader/recipe verification, immutable release install, flush, and same-directory temporary `current.json` plus atomic `persist`. Do not remove/recreate the lock file or auto-delete previous releases. A failed commit can leave an unreferenced new release; the normal next build can validate/reuse it.
- [x] **Step 4:** Complete `build --recipe --cache --output` and `verify --bundle --recipe`, with `--help`, explicit human-readable diagnostics and nonzero failure exit. `verify` uses the recipe to check recipe digest/declared scope and acceptance, not only the embedded manifest. Install Ctrl-C handling in application composition with a standard Rust signal crate and the borrowed cancellation flag; no signal handler in reader/domain modules. Existing files are unchanged before commit on cancellation. Build/verify are network-free.
- [x] **Step 5:** Verify actual publication/replacement behavior on Windows, including an open reader and a deliberately sharing-locked pointer; failure must preserve the old selection. Cross-compilation alone does not establish filesystem semantics. Run native publication/application tests, document platform guarantees and commit with the commit-point rationale.

## Task 8: Observability, production verification and integration documentation

**Files:** Owning operation modules, both `tests/observability.rs`; crate READMEs, root README and relevant source/spec notes; Windows CI job if a suitable repository runner is being introduced.

**Interfaces:** Existing operations only; no new public telemetry interfaces. Applications configure subscribers/recorders, libraries emit. No generic observer, completion struct, span wrapper or lifecycle timer.

- [x] **Step 1:** Add focused failing observability tests through real lookup/build/fetch operations: published versus unchanged versus cancelled/failed outcomes; committed counts; omission summary; no success before pointer commit; query-error absence of partial result counters; publication/error correlation when debug spans are filtered. Check bounded labels and exclusion of coordinates/locators, full source bodies, authentication data and arbitrary paths from metrics. Do not duplicate the entire functional suite with telemetry off.
- [x] **Step 2:** Run the focused tests, then complete the spec's instrumentation at the owning boundaries. `#[instrument(skip_all, ...)]` uses direct fields and default module targets. Keep library compile/open/lookup work at debug; explicit fetch/build/publication attempts and actual publication/omission milestones at info; handling-boundary failures once at error. Framework spans own lifecycle; normal workflow boundaries own metric histograms. Fixed-label dimensions are only those in the specification. Ensure elapsed histograms include failures/cancellation through ordinary workflow result handling, without wrapping every helper or altering domain control flow.
- [x] **Step 3:** Finish crate documentation: public APIs, bundle/recipe versioning, source acquisition and offline commands, notice/licensing basis, exact representative scope, reviewed omissions, no-shift limitations, measured numerical behavior, publication/recovery guarantees and #23 integration obligations. Replace speculative capability wording only where execution established a result; retain honest source-accuracy limits. Do not add a review-history or issue-status document.
- [x] **Step 4:** Run the final commands below. Inspect the locked feature graph and source for prohibited dependencies/tooling, production panic/indexing, test-only APIs, facade re-exports, unexplained public types, manual tracing formatting and redundant telemetry. Check every spec section has delivered behavior/tests, not just a corresponding heading. Fix defects in their owning modules and rerun affected checks.
- [x] **Step 5:** Commit the verification/documentation changes with a substantive body. Keep meaningful functional commits; fold corrections into the task that introduced them when preparing the branch. Preserve user-authored changes. Do not push, create a PR or close #5 without the applicable user authorization.

## Final verification and evidence

After the explicit fetch has installed verified cache entries, all build/test commands below are offline. Replace `<cache>` with the user's local source cache; do not check it or generated large bundles into Git. Standard tests construct small inputs with Rust support code; checked-in data is reviewed source/recipe/correction material, not opaque generated expectations.

```sh
cargo +1.99.1 fmt --all --check
cargo +1.99.1 test --workspace --all-targets --locked --offline
cargo +1.99.1 test --workspace --doc --locked --offline
cargo +1.99.1 clippy --workspace --all-targets --locked --offline -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo +1.99.1 doc --workspace --no-deps --document-private-items --locked --offline
cargo +1.99.1 tree -p clacks-geography -e features --locked --offline
cargo +1.99.1 tree -p clacks-geography-builder -e features --locked --offline
CLACKS_GEOGRAPHY_CACHE=<cache> cargo +1.99.1 test --release -p clacks-geography-builder --test real_sources --locked --offline -- --ignored
cargo +1.99.1 run -p clacks-geography-build --locked --offline -- build --recipe references/geography/representative.json --cache <cache> --output <output>
cargo +1.99.1 run -p clacks-geography-build --locked --offline -- verify --bundle <output>/releases/<release-id> --recipe references/geography/representative.json
```

Acquire sources separately with `cargo +1.99.1 run -p clacks-geography-build --locked --offline -- fetch --recipe references/geography/representative.json --cache <cache>`; Cargo stays offline while the explicit command performs downloads. Windows delivery evidence includes `cargo test` for artifact/lookup/publication/application behavior on a real Windows runner and a full workspace check on `x86_64-pc-windows-msvc`. Do not claim these passed from macOS-only tests or an unexecuted workflow file.

Expected evidence: all ordinary workspace checks pass; explicit real-source cases pass through emitted FlatGeobuf and the production reader; two builds reproduce identical bytes; old selected releases survive injected failures; actual Windows replacement behavior passes; transitive features contain no forbidden native geographic/network stack in the reader or OpenSSL-backed fetching. Large coverage expansion and application autofill remain #23 work.

## Dependency and specification coverage check

Task order is **1 → 2 → 3 → 4 → 5 → 6 → 7 → 8**. Every task adds usable production behavior; early synthetic inputs exercise the eventual production path. No step creates an environment later tasks discard.

| Specification | Owning tasks |
| --- | --- |
| Boundaries, Rust layout, dependencies and source rights (§1–3) | 1, 3, 8 |
| Recipe/guards/mappings/exceptions/omissions (§4) | 3, 5, 6 |
| Transformations, geometry, date line, poles and numerical behavior (§5) | 2, 4, 5 |
| Bundle schema, hashes, roles and reproducibility (§6) | 1, 6 |
| Query candidates/coverage and invalid input (§7) | 2 |
| Publication, commands and cancellation (§8) | 3, 7 |
| Meaningful observability (§9) | 2, 3, 7, 8 |
| Real-source acceptance and honest limitations (§10–11) | Named cases above; 4–8 |

Implementation starts only after review of this plan. Planning completion is not delivery completion.

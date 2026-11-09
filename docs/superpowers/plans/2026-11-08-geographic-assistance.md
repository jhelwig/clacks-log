# Geographic Assistance Implementation Plan

**Goal:** Deliver #23's bundled offline geographic assistance through shared
native functionality used by desktop editors and automatic live capture.

**Architecture:** Extend the existing geographic producer/reader and native
location values. Build geography as a cached dependency of native packaging;
both platforms consume one validated directory from current source inputs.
Runtime composition owns one reader, while shared native rules apply eligible
values for both interactive and captured contacts.

**Tech Stack:** Rust 1.99.1, locked `geo`/FlatGeobuf/Serde/ZIP dependencies,
egui/eframe/egui_kittest, Tokio, existing SQLite saves, tracing and metrics,
GitHub Actions cache/artifacts, existing MSI and signed/notarized DMG packaging.

**Spec:** [Revised geographic-assistance design](../specs/2026-11-08-geographic-assistance-design.md).

**Scope:** Update distribution in the existing branch while preserving shared
assistance behavior. Reuse existing code and tests rather than recreating them.
This plan does not include publication, merging or a server updater.

## Global Constraints

- Use Rust 1.99.1, locked dependencies and existing crate boundaries. All
  project-owned data preparation/packaging tooling is Rust; no Python or native
  geographic stack. Never execute the supplied TrustedQSL reference sources.
- Required U.S. coverage; desired worldwide DXCC/CQ/ITU coverage with explicit
  limitations. Retain Census, Natural Earth and HA8TKS source choices and notices.
  DXCC means DX Century Club; ITU means International Telecommunication Union;
  CQ refers to CQ magazine's operating zones.
- Grid input queries the entire area; explicit coordinates query a point.
  Autofill requires one distinct candidate and complete coverage, per field.
- Preserve same-position corrections, manual clearing, supplied/defaulted
  capture values and rejected optional-field absence. Do not persist origin flags.
- Geography is a cacheable build dependency. Reuse exact matching validated
  output; build automatically on a miss. No separately published prerequisite,
  Git/LFS payload, manual producer pin or committed output hash is required.
- No new database tables, migrations, synchronization records/messages, updater,
  server endpoints, hot reload, implicit restore/export enrichment or extra save
  confirmation. Server-provided updates remain future work using output from
  the same producer.
- Read installed resources directly. Do not copy data into workspaces or modify
  signed/macOS or installer-owned/Windows resources for independent replacement.
- Preserve ordinary guarded saves and existing capture pairing/duplicate rules.
  Lookup work holds no database transaction and blocks neither UI nor reception.
- Tests target project-owned mistakes and use production operations/temporary
  workspaces. Justify ordinary CI execution independently of test usefulness.
  Native geography checks use explicitly isolated workspace/config/log paths
  and the user-owned machine inventory, never the normal application workspace.
- Give each implementation commit a coherent subject and substantive body in a
  message file, inspect its actual 72-column body wrapping, and fold corrections
  into their owning commits before review.

## Review Focus

- Cache inputs include transitive producer catalogs, corrections and notices;
  unrelated application source does not invalidate the output (Task 1).
- An exact hit reuses validated output; a miss automatically builds from this
  checkout without a release, registry upload or manual pin (Task 1).
- Both native binaries embed the common output's expected identity, including
  after an incremental native build with a changed digest (Task 6).
- Missing packaged identity leaves developer manual use available but cannot
  produce a release package that silently trusts its own manifest (Tasks 4/6).
- Snapshot/correction, capture and editor behavior remain unchanged; preserve
  current checks without replaying them through a new distribution test layer.

## Files and dependency order

All paths below are repository-relative. Inspect existing implementations first.
Keep any required production edits focused; register
new tests in the existing suite/module lists rather than adding one executable
per scenario. Tasks are implementation units of one capability, not separate
delivery milestones. Do not declare completion with dormant helpers.

| Task | Production files and ownership | Focused verification |
| --- | --- | --- |
| 1 | Existing builder recipe/compile/application, build CLI, manifest model; `.github/workflows/desktop-release.yml` | Existing producer acceptance; actual cache miss/hit and invalidation runs |
| 2 | `references/geography/shipping.json`, `shipping/` inputs; existing recipe/compile modules only for demonstrated ingestion gaps | `tests/real_sources.rs`, shared support; shipping CLI verify |
| 3 | `lib/clacks-geography/src/assistance.rs`, `lib.rs` | `tests/assistance.rs` using existing fixture support |
| 4 | `lib/clacks-desktop/src/geography.rs` and `geography/runtime.rs`, `application.rs`, `session.rs`, config modules, `ws_suite/{contact,pairing,runtime}.rs` | Geography/config child tests; existing capture/session scenarios |
| 5 | `operating/{station,editor}.rs`, `logging/defaults.rs`; existing UI operating/profile/logging/contact-editor consumers; `ui/geography.rs` | Existing operating/logging/component suites |
| 6 | Existing packaging geography/CLI, desktop runtime identity, native workflow/install inputs; remove `deploy/desktop/geography.json` | Focused identity wiring checks and actual payload inspection |
| 7 | Owning geographic/native/operating/manual/capture contracts and affected READMEs; focused integration evidence | Existing functional coverage and any missing packaged startup wiring |

Tasks 1–2 establish a reusable validated build output. Task 3's shared native
interface feeds Tasks 4–5; retain existing reviewed functionality where unchanged.
Task 6 consumes Task 1's common output and generated identity. Task 7 verifies
assembled behavior. These dependencies do not require separate PRs or manual
publication between tasks.

## Task 1: Make geography a normal cached build dependency

**Files:** Modify existing builder `src/{recipe,compile,application,publish}.rs`,
build CLI `bin/clacks-geography-build/src/main.rs`, geographic manifest model and
validation where identity fields change, and `.github/workflows/desktop-release.yml`.
Remove `.github/workflows/geography-release.yml`, builder `src/release.rs`, its
export/CLI archive command and archive-only `tests/release.rs`. Remove archive-only
dependencies if no remaining code uses them. Update builder usage documentation
and replace `references/geography/BUILDING.md` with build/cache instructions.

**Interfaces:** `BuildRecipe` holds maintained geographic inputs, not manually
selected producer/release identities. The build operation accepts a generated
input fingerprint (`--build-id`) separately from the recipe; it uses that value
as the existing manifest data identity. Remove the redundant producer commit
field rather than adding a compatibility layer or schema bump for this PR-only
format. Keep recipe/source/payload digests, schema and coverage metadata needed
by the existing pipeline. Ordinary explicit verification uses the actual bundle
and recipe, without requiring an already committed producer revision.

Build output consists of a validated bundle directory and its producer-computed
expected manifest digest outside that directory. This is generated output, not a
second maintained descriptor. Existing package evidence may carry this digest;
no new report schema is required. Do not hash the currently selected bundle at
runtime to establish its own expected identity.

- [ ] **1. Trace build inputs and existing checks before changes.** Read the
  producer's Cargo dependencies, `include_str!`/embedded data and shipping recipe
  referents. The current local chain is build CLI, builder, geography and model;
  include model catalogs under `lib/clacks-model/data`, not just Rust source.
  Use standard workflow `hashFiles` inputs, with Cargo manifests/lockfile,
  toolchain, relevant Cargo configuration and the producer invocation/settings.
  Exclude unrelated desktop/server source, Git SHAs and current time. Hashing the
  whole root manifest/lockfile may conservatively rebuild on unrelated dependency
  edits; do not build a custom dependency cache system to avoid that cost.
- [ ] **2. Remove manually authored identity fields and release-only operations.**
  Update maintained recipes and authoring output, builder validation, manifest
  construction and existing fixture support together. Derive data identity from
  the supplied build-input fingerprint. Record provenance through that fingerprint
  and existing recipe/source metadata, without a separate producer revision
  requiring a commit before generation. Preserve existing production compilation,
  exhaustive validation and recipe cases. Reuse the current build/publish library
  operations with a fresh output directory; do not add another publisher or change
  its pre-existing pointer/locking behavior solely for this cache integration.
- [ ] **3. Wire cache restore/build/save into the common geography job.** Use
  standard Actions cache restore/save, exact match only, and existing trusted
  workflow permissions/scoping. When `cache-hit != 'true'`, build the producer
  from this checkout, fetch verified pinned originals with its existing source
  cache and build/validate once. An unsuccessful build is not saved or packaged.
  Hits restore the validated output and expected digest, using ordinary reader
  integrity checks without polygon/index/inventory acceptance. Cache absence,
  eviction or branch isolation causes generation, not a manual recovery gate.
  Do not require a registry, bucket, release, tag or upload step from the operator.
- [ ] **4. Transfer the directory and generated identity to both native jobs.**
  Use the existing same-run upload/download artifact steps; they already support
  directories and compression. No standalone ZIP command/asset is needed.
  Keep source archives and compiled output outside Git and do not add evidence
  or measurement assets. No per-platform compilation of geographic data.
- [ ] **5. Verify changed project behavior proportionately.** Adapt existing
  producer/reader tests to generated identity and retain only tests that detect
  wrong identity propagation or acceptance failure reaching successful output.
  Do not add tests of hashFiles, cache matching, ZIP compression, declarative YAML
  gates or mocked GitHub commands. Run affected Rust tests and CLI help. Exercise
  one actual cold build, then an exact hit that skips generation, a relevant input
  change that misses, and an unrelated app-source edit that hits. A small
  supported production recipe
  can check orchestration; required shipping coverage still needs real production
  acceptance. Record actual workflow evidence and cache-scope limitations.

## Task 2: Maintain and verify shipping coverage inputs

**Files:** Existing `references/geography/shipping.json`, reviewed `shipping/`
inputs and authoring module; existing producer original-source acceptance support.

**Interfaces:** Keep supported authoring separate from compilation. Remove manual
producer revision and data-release ID authoring; the build supplies identity.
Keep all source pins, mappings, corrections, notices and acceptance cases needed
for required coverage. Do not regenerate unchanged geographic payload merely to
revise documentation. The first build under changed identity semantics may need
new output; this planning task performs no build.

- [ ] **1. Preserve actual required coverage and reviewed source facts.** Retain
  Census state/county selections including Connecticut counties, regional masks,
  coordinate operations, DXCC mappings and CQ/ITU corrections. Required U.S.
  coverage remains mandatory; worldwide limits stay explicit. Source-vintage
  changes require reviewing the originals rather than reusing stale counts.
- [ ] **2. Remove only release-process metadata from maintained inputs.** Update
  `author-shipping` and its usage so it does not restore manual producer pins or
  release names. No fixture generator, full-source authoring or acquisition runs
  during ordinary input hashing. Keep notices/corrections referenced by the
  shipping recipe in the cache's input set.
- [ ] **3. Use existing production acceptance on cache misses.** Preserve required
  region, whole-grid/point, border, offshore and seam cases with independent
  expected values. Existing real-source checks remain the authority for audited
  source inventories; do not add another golden manifest/inventory or timing test.
  Source-independent tests do not claim shipping geographic acceptance.
- [ ] **4. Record capacity only when needed for implementation acceptance.** Use
  ordinary process/file tools for build memory/time, installed/package size and
  startup/query observations. No timing assertions, permanent measurement command
  or report asset. Keep results in PR/issue evidence, not maintained build inputs.

## Task 3: Preserve shared native assistance

**Files:** Existing `lib/clacks-geography/src/assistance.rs`,
`tests/assistance.rs` and crate README. No distribution logic belongs here.

**Interfaces:** Retain `change_position` and `populate` with native
`RecordedLocationFields`, `Position`, `BTreeSet<Field>` and `LookupResult`.
The design's independent field eligibility, complete unique candidate,
position-bound correction and protected-absence rules remain authoritative.

- [ ] **1. Inspect the existing implementation and tests for identity coupling.**
  Adapt only a data-identity fixture or assertion affected by Task 1. Do not
  recreate modules, retest geometry libraries or restore previously deleted tests.
- [ ] **2. Preserve the shared API and domain semantics.** Generated build
  identity is provenance, not a change to eligibility or correction lifetime.
  Existing native-rule tests already cover these rules. Extend them only for a
  demonstrated new project-owned gap, with its requirement and failure stated
  before adding a test. Run the existing affected target if this code changes.

## Task 4: Adapt runtime identity without replacing composition

**Files:** Existing desktop `src/geography.rs`, `geography/runtime.rs`,
configuration, session/application composition and `ws_suite` capture consumers.
Task 6 owns replacement of the packaged descriptor with build-provided identity.

**Interfaces:** Retain `geography::Selection` (directory plus independent expected
manifest digest), the existing client/status and finite session-owned operations.
Explicit external selection retains its directory/digest configuration. One
runtime reader continues serving UI and capture through existing Tokio facilities.

- [ ] **1. Preserve the established loading boundary.** Manifest identity, schema
  and payload integrity are runtime checks. Exhaustive feature/index/inventory
  and polygon validation stays in producer acceptance. No change introduces
  implicit geographic work into constructors, persistence, synchronization,
  reload or conflict resolution.
- [ ] **2. Adapt existing loading checks only where identity sourcing changes.**
  Keep manual work and capture available when data is unavailable. A build without
  packaged identity must not silently trust the installed bundle. Reuse current
  configuration/runtime scenarios; do not add another mock reader, task-drain
  suite or telemetry suite. Task 6 owns actual embedded-identity verification.
- [ ] **3. Retain capture and lifecycle behavior.** Supplied/defaulted fields,
  rejected optional-field absence, format pairing, duplicate handling and save
  boundaries remain unchanged. Keep blocking I/O off UI/receiver loops and use
  existing session task ownership. No new queue, worker or shutdown protocol is
  needed for a cached build artifact. Run changed code's existing checks only.

## Task 5: Preserve editor behavior and proportional coverage

**Files:** Existing `operating/{station,editor}.rs`, `logging/defaults.rs`,
`ui/geography.rs` and operating/profile/logging/contact-editor consumers.

- [ ] **1. Confirm consumers remain independent of artifact production.**
  Editors use shared native assistance and runtime status. Cache keys, generation,
  release selection and source acquisition must not enter UI controls.
- [ ] **2. Retain existing operation boundaries.** Same-position manual edits and
  clearing remain protected; genuine position changes expire geographic choices.
  Old replies cannot affect moved/closed/submitted operations. Quick DX-grid and
  details share state; snapshot loading, directory facts and explicit review
  fields remain preserved. Ordinary parent saves commit the submitted snapshot.
- [ ] **3. Reuse existing focused tests.** No UI behavior change is planned, so
  no new UI tests or repeated native interaction matrix is justified. If actual
  integration exposes a regression, identify its project-owned failure and add
  only the missing check at the owning layer. Do not reintroduce deleted tests
  merely because an earlier plan named them.

## Task 6: Package the common build output on both platforms

**Files:** Modify existing `lib/clacks-desktop-packaging/src/geography.rs`, its
CLI/metadata and focused tests, desktop `src/geography/runtime.rs`, native
workflow, MSI inputs and platform READMEs. Delete `deploy/desktop/geography.json`
and remote release-selection/archive extraction code and tests without another
consumer. Remove dependencies needed only by those deleted paths.

**Interfaces:** Packaging takes a prepared directory and expected manifest digest,
not repository/tag/asset/archive coordinates. Reuse the existing geography-inspect
operation with `--bundle PATH --manifest-sha256 HASH`; return existing package
identity evidence. Native compilation receives
`CLACKS_GEOGRAPHY_MANIFEST_SHA256` from that same build output, read with Rust's
`option_env!`; absence keeps data-free developer builds usable, while packaging
fails without a valid supplied identity. Explicit external selection still
requires its independent expected manifest digest.

- [ ] **1. Replace the checked-in pin and release retrieval.** Remove descriptor
  parsing from packaged identity selection. Pass the generated expected digest
  into both native Cargo builds and packaging inspections. Do not silently derive
  it from each platform's installed bundle. Preserve reader manifest identity,
  supported-schema and payload-integrity checks without production acceptance.
- [ ] **2. Retain only focused identity-wiring regression checks.** Adapt existing
  package inspection tests to show that correct output is accepted and a bundle
  from another build fails the expected digest. Remove archive/pin tests whose
  production path is gone; do not replace them with mocks, YAML assertions or
  tests that reproduce framework behavior. Missing required packaging identity
  is checked at the actual command boundary, not through another configuration
  test suite.
- [ ] **3. Include ordinary files using existing package facilities.** Windows
  includes the common tree beneath installation `geography`; macOS copies it to
  `Contents/Resources/geography` before signing/notarization. Retain all declared
  notices. Use fresh staging output so another bundle's files cannot linger.
- [ ] **4. Verify actual products and existing evidence.** Inspect both package
  payloads against the common expected manifest and declared digests. Confirm the
  application embedded identity matches that output and macOS signatures cover
  the resource tree. Retain identity in existing package evidence; the installers
  already retain shipped data, so a separate geographic release asset is not
  required. Keep existing application release eligibility and signing gates.
- [ ] **5. Verify affected tests, CLI help and documentation.** Run native builds
  only for actual integration/native acceptance when prerequisites are available.
  Local commands describe automatic miss/build behavior and explicit data
  overrides without requiring manual release generation, upload or pin edits.

## Task 7: Verify assembled behavior and reconcile owning contracts

**Files:** Update distribution clauses in `docs/geographic-pipeline.md` and
affected crate/platform READMEs. Inspect native, operating, manual-entry and
capture contracts and update only conflicting clauses. Existing tests need edits
only for an uncovered failure in changed integration. Keep delivery status and
measurement/review evidence on #23/its PR.

- [ ] **1. Review preservation through existing operations.** Confirm the diff
  changes distribution/identity wiring without adding enrichment on save/reload,
  changing stored snapshots or altering explicit field-resolution behavior.
  Use existing regression coverage; add no duplicate save/reload scenario solely
  because reference-data production moved into packaging.
- [ ] **2. Close only geography-specific evidence gaps.** Review existing package
  inspection, normal-startup and functional evidence for bundled resource
  discovery, independent expected identity, offline assistance and nonfatal
  invalid selection. Reuse existing snapshot, correction, editor and capture
  coverage. If normal packaged startup is not covered, run that focused check
  with isolated paths and observe bundled-data readiness; do not repeat the
  already established query/UI behavior through another full interaction matrix.
  Follow the user-owned machine inventory and verify executable/isolation
  arguments. #23 adds no clean-machine installation or version-upgrade suite;
  broader platform release acceptance remains with #35/#70.
- [ ] **3. Complete proportional automated verification after integrations.**
  Run `cargo +1.99.1 fmt --all --check`, affected package tests, and
  `cargo +1.99.1 clippy -p clacks-geography -p clacks-geography-builder -p clacks-desktop -p clacks-desktop-packaging -p clacks-desktop-package --all-targets --locked --offline -- -D warnings`.
  Use workspace compilation through normal CI. Run wider required CI checks once for the completed branch;
  do not repeatedly execute full-source data builds or the whole workspace for
  every documentation edit. Native Windows behavior is not proved by a cross-check.
- [ ] **4. Reconcile contracts around shared functionality.** Geographic rules
  belong to native assistance APIs; UI contracts describe presentation only.
  Document capture integration, protected absence, exact snapshot-preserving
  consumers and the cacheable build dependency with common-output packaging.
  Keep server update protocol/selection/download implementation future work.
- [ ] **5. Review instrumentation and tests separately from functional success.**
  Inspect changed identity/cache diagnostics for truthful outcome reporting and
  exclusion of coordinates/callsigns/secrets. Unchanged query/capture reporting
  needs no repeated exercise solely for this distribution revision. Retain
  only checks detecting a meaningful Clacks integration mistake; do not add
  library-conformance tests or make full-source acceptance ordinary app CI.
- [ ] **6. Record evidence and remaining platform/data limits in #23/PR.**
  Identify actual source/data/package revisions and hashes, measured resources,
  native observations and unavailable checks. Review coherent branch history and
  message wrapping before any authorized push. Keep #23 open until the production
  capability and required acceptance are delivered; planning is not delivery.

## Verification evidence

This plan defines implementation and verification work; it does not record
execution results. Delivered behavior, completed checks and remaining acceptance
belong in #23 and its PR.

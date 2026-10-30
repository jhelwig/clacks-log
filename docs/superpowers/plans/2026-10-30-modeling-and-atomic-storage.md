# Modeling and Atomic Storage Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans for established native execution. Work task-by-task using the checkboxes. This plan does not authorize implementation, database resets, committing or pushing.

**Goal:** Complete PR #58's checked modeling and atomic storage work without
redundant representations, dormant production components or dead_code allowances.

**Architecture:** Keep queryable application facts and relationships relational.
Use checked native construction for intrinsic invariants, ordinary Serde for
complete native values, and the small versioned encrypted-value codec for existing
enrollment receipts and account keys. Future service credentials arrive with
their first production workflow rather than as a generic store here.

**Tech stack:** Rust 1.99.1, Serde/serde_json, SQLx 0.9, PostgreSQL, SQLite, existing
RustCrypto AES-256-GCM encryption, zeroize, Tokio, egui/eframe and selected
authorization libraries. No new serialization/encryption framework or application.

**Spec:** The agreed target contract below and the
[credential-protection proposal](../../credential-protection-proposal.md).
Preserve the owning [native model](../../native-model.md),
[synchronization model](../../synchronization-model.md),
[service evidence](../../service-evidence.md),
[operating settings](../../operating-settings.md),
[enrollment](../../client-enrollment-proposal.md) and
[desktop UI](../../desktop-ui.md) contracts.

**Execution baseline:** Continue the existing PR #58 branch, inspecting its
current state. Much checked modeling already exists; retain it rather than
reimplementing it. Unchecked steps below describe remaining implementation or
reverification under the agreed scope; they do not claim the current code already
matches it. The [#13 plan](2026-10-29-credential-protection.md) owns removal of
service storage and preservation of the real enrollment/key-maintenance path.

## Global constraints

- Preserve matching, effective/recorded fallback, confirmation applicability,
  whole-record reconciliation, upload policy, enrollment security and UI behavior.
  No new product decisions, synchronization algorithm or protocol is introduced.
- Schemas have no active consumers outside development. Edit original migrations
  in place; add no forward-copy migrations, dual readers, historical-format
  adapters, compatibility notes or database-recreation instructions.
- Verification uses disposable stores. Do not reset persistent development state.
  Future deployed-schema changes still follow normal migration policy.
- Preserve unknown mode/band/reference text, locator precision, whole-second
  times, whole-hertz frequency, milliwatt power and signed-64-bit storage bounds.
- Retain authorization, current-state checks, conditional saves, transactions,
  keys, uniqueness and account-qualified relational references.
- Keep fixed SQL in checked query files with committed offline metadata.
  Raw decoder rows may be incomplete; ordinary writes consume checked native
  variants directly, using named fields rather than anonymous tuples.
- No generic AtomicObject trait, custom key protocol, extra version wrappers,
  serializer dependency, write queue, job registry or telemetry abstraction.
- No Clacks-owned dead_code allow/expect attributes, blanket suppression, dummy
  reads/callers/tests or visibility widened to evade warnings. Vendored code is
  outside this cleanup. Clap's selected derive macros also emit dependency-owned
  allowances; forbid rejects their expansion. Use workspace dead_code = "deny"
  without Clacks-owned suppressions and inherit it in
  owned members without overriding it in their targets.
- Keep Rust-only tooling, thin bin/ applications, focused lib/ modules and
  standalone rustdoc. No mod.rs outside tests, ordinary production unwrap/expect,
  unchecked indexing, facade re-exports or test-only production interfaces.
- Follow the observability and testing contracts. Preserve normal control flow,
  actual domain events, truthful committed metrics and secret exclusion.
  Do not test dependencies or add matrices solely for coverage.

## Agreed target contract

### Checked native models

Keep intrinsic invariants behind checked construction/deserialization and
coherent transitions. Existing QsoFields, GroupDefinitionFields, StampFields,
AcceptedFields, WitnessFields and frozen LoTW submission input types enforce
real boundaries and stay. Do not replace them with freely mutable saved records.

Draft entry/import state remains separate from saved QSOs. Preserve partial
provider evidence, Reported omission/value/clear meanings and independently
optional issued access/refresh references. Rust validation cannot replace
current authorization, external outcomes or relational existence checks.

AcquisitionPhase directly represents Active, Completed with time, or Failed
with time/reason. Serialize that current representation directly; remove the
adapter maintaining redundant historical phase/finished_at/complete fields.
Keep used computed accessors. GrantState retains Provisional with deadline,
Active with actor, and unit Revoked. Historical SQL facts remain retained even
when revoked runtime decisions need no copied origin.

### Atomic values and serialization

| Value | Storage and consumer |
| --- | --- |
| Account keys and consumed-code receipts | Complete packed encrypted values, with expected ownership/context authenticated. Key identities and code lifecycle remain relational. |
| Group identifier configuration, optional catalog binding, export bindings/plans | Ordinary Serde serialization of existing native values; group/profile ownership stays relational. |
| Frozen service field maps and field sets | Ordinary native serialization directly on the owning projection/assertion/observation; no independent map identity or retention root. |
| Exact remote record key | Ordinary Serde serialization of RemoteRecordId. Account/destination remain columns, preserving external_id references and qualified uniqueness. |
| RecordVector and Coverage | Complete native serialized values on qualified owners; stamps/checkpoints/lineage/retention edges remain relational. |
| Output-context fields and local enrollment attempt | Ordinary serialization without an unused per-document version. Preserve actual validation and coherent state. |
| QSO/station/operator/equipment/directory facts | Independently queryable relational fields, retaining effective/recorded query behavior. |
| Memberships, activities and evidence dependencies | Relational edges used by matching, joins, installation and garbage collection. |

Remove IdentifierDocument, CatalogDocument, BindingsDocument, PlanDocument,
FieldMapDocument, FieldSetDocument, VectorDocument, CoverageDocument and
ContextFieldsDocument. These version wrappers have no independent consumer.
Coverage serialization must still validate the native Coverage: some construction
paths do not establish its checked revision bounds. Keep empty, omitted and
explicitly cleared values distinct.

Existing RemoteRecordId and LotwRecordKey already express native identity.
Use serde_json::to_string/from_str directly, preserving exact native labels,
complete QRZ decimal strings and native LoTW fields. Remove StoredKey,
QrzLabel/LotwLabel and positional array encodings. No replacement wire enum,
hash-only equality proof or public codec API. Writer-produced serialization and
the existing qualified unique key provide database equality; do not guess a
different identity from current recognition catalogs.

Maps have one owner and no independent semantics. Put complete field-map text
on submission projections, imported assertions and incomplete observations,
removing map IDs and owner/role lookup rows. Preserve evidence IDs, dependency
links, omission/clear semantics and meaningful provider facts.

Remove PositionEncoding's wrapper around PositionFields. Keep one named
conversion for SQL binding and checked raw decoding. This is not permission to
pack queryable position facts or replace them with anonymous column tuples.

### Encryption and database responsibilities

Retain the receipt/account-key packed forms, authenticated codec header and
owner context in the credential proposal. Remove service-credential storage,
codec and rotation branches in concert with the #13 plan. No new encryption
dependency or algorithm negotiation. Use generated key-reference projections
without independently writable copies.

Remove SQL checks duplicating Rust enums, domain ranges, prepared text and
coupled variants. Keep native column types/nullability, primary/foreign keys,
account-qualified ownership, set/identity uniqueness and active-key uniqueness.
Retain fixed-family qualifications needed by composite references and actual
physical representation rules. Do not remove indexes indiscriminately.

## Review focus

- Ordinary Serde must preserve native external-key distinctions, including LoTW
  receive band/mode and QRZ IDs beyond signed-64-bit range (Task 3).
- Wrapper removal must retain Coverage validation and omitted/clear/empty
  distinctions, not silently accept invalid saved values (Task 3).
- Map inlining must retain evidence ownership, association uniqueness and
  recovery/collection links (Task 3).
- Credential removal must not strand actual enrollment, encrypted receipts or
  either operator rotation command (Task 2 and the #13 plan).
- Removing telemetry support must preserve assertions that actually use targets,
  parent relationships, counts and secret exclusion in their owning suites
  (Task 4).

## Task 1: Preserve checked domain and workflow modeling

**Files:** Existing model contact/shared/service modules, synchronization stamp/
accepted/witness modules, client operating/enrollment mappers, server OAuth grant
decoding and their existing behavior tests.

- [x] Retain implemented checked constructors/Serde conversion for saved QSO,
  location, operator, equipment/profile/group values, accepted contexts,
  witnesses and frozen LoTW submissions. Keep valid-input behavior unchanged.
- [x] Retain coherent operating selections and enrollment recovery states.
  Preserve copied operating values, source baselines and pending work.
- [x] Derive normal serialization for AcquisitionPhase; remove PhaseFields and
  AcquisitionFields historical-shape adapters. Update actual acquisition
  persistence consumers and meaningful fixed expectations together.
- [x] Replace GrantOrigin and Revoked payload with unit Revoked. Update all
  eligibility/activation/replay decisions exhaustively, retaining stored history.
- [x] Run existing model, sync, LoTW and enrollment/operating scenarios.
  Remove assertions for accidental old representations without adding tests of
  Rust privacy or Serde itself.

## Task 2: Narrow server storage to current enrollment consumers

**Files and interfaces:** Owned by Tasks 1–3 of the
[#13 plan](2026-10-29-credential-protection.md).

- [x] Execute that plan's removal of unused service credentials, schema, codec,
  grants, metadata and exclusive tests together. Retain live receipt/key formats.
- [x] Preserve both command-line rotation operations on actual issuer-created
  receipts/keys, including resume, report accuracy and exporter shutdown.
- [x] Reconcile source grants/queries and regenerate/check server SQLx metadata.
  Do not claim #13 complete until its narrowed outcome is verified.

## Task 3: Simplify client persistence representations

**Files:** `lib/clacks-client-store/src/payload/definitions/documents.rs`,
`payload/service/{documents,identity/key,projection}.rs`,
`metadata/clock/document.rs`, `payload/shared.rs`,
`enrollment/attempt.rs`, `rows/contact.rs`, affected callers, migrations
`0003_shared_values.sql`/`0004_service_values.sql` and matching SQL files.

- [x] Replace all nine document wrappers and enrollment's extra version with
  native serde_json serialization. Retain explicit validation where needed,
  especially Coverage, and contextual errors for malformed stored data.
- [x] Replace the external key codec with native RemoteRecordId serialization.
  Keep account/destination columns, external_id references and qualified
  uniqueness. Preserve exact recorded callsigns/labels and decimal QRZ IDs.
- [x] Inline field-map text on actual owning rows. Remove service_field_map,
  artificial map identities, owner/role indirection, exclusive queries and orphan
  traversal. Keep queryable provider facts and evidence relationships.
- [x] Remove PositionEncoding, retaining named binding fields and checked
  restoration. Do not add another one-use wrapper or lossy fallback.
- [x] Adapt existing persistence/export/catalog/service/compaction scenarios to
  the simpler representation. Assert actual matching, interpretation, reopening,
  recovered dependencies and comparison outcomes. Preserve distinct-key cases;
  avoid adding generic serialization round trips.
- [x] Remove deleted-query metadata and regenerate/check SQLite metadata using
  the production migrations on one disposable database.

## Task 4: Remove all owned dead_code allowances and their causes

**Files:** Server grant/credential modules handled above; workspace/member Cargo
lint configuration; model/sync/LoTW/client-store test support; geography
observability tests, geographic-builder application tests, and web/desktop
shared OIDC provider fixtures.

- [x] Remove obsolete support allowances in sync/LoTW and client-store
  support/logbooks: their retained items already have consumers.
- [x] Remove model capture's unused target, client-store capture's unused
  name/target/parent fields and span/event_count methods. Preserve target/parent
  data in suites whose actual assertions use it.
- [x] Replace geography/builder imports of oversized telemetry support with
  focused test-local capture for their meaningful assertions. No new generic
  capture library, feature matrix or dummy assertions.
- [x] Separate the registration-count probe needed by web registration tests
  from the sign-in fixture used by desktop. Keep real registration/sign-in
  behavior; no production/test interface added to consume unused fields.
- [x] Remove all 17 Clacks-owned allowances. Enable inherited
  dead_code = "deny" for every owned workspace member/target. Build all targets
  and resolve any further genuine unused code without suppressions or visibility
  changes. Exclude vendor sources from this cleanup.
- [x] Run affected existing suites. Check assertions separately for actual Clacks
  requirements; do not preserve deferred APIs because their tests exist.

## Task 5: Reconcile contracts and verify the integrated application

**Files:** Owning native/sync/service/client-store/enrollment/credential guides,
roadmap and the GitHub capability issues/milestones.

- [x] Make the documents describe current target product/architecture, not review
  narration. Ensure no version-wrapper/tuple-key prescriptions, unowned moved
  requirements, generic secret CRUD promise or completed status for pending
  simplifications remains.
- [x] Keep directory setup/storage/provisioning/lookup together in #17; QRZ in
  #31; LoTW extending QRZ in #30. Essential controls belong to each provider;
  #33 holds only remaining combined diagnostics. Use the reconciled roadmap.
- [x] Review every changed test for its Clacks requirement and failure mode.
  Remove dependency-only tests and scaffold-only production interfaces.
- [x] Run the checks below against actual production migrations with disposable
  state. SQL metadata jobs run sequentially. Record exact results and unavailable
  native/hosted acceptance; previous verification counts do not apply.
- [x] Reconcile PR #58's description with implemented scope and evidence before
  claiming readiness. Preserve accepted product behavior and limitations.

## Verification commands

Follow the owning crate guides for one disposable PostgreSQL database with real
roles and one temporary SQLite database. Set SQLX_OFFLINE and LEPTOS_OUTPUT_NAME
as documented. Do not add a separate query-only database or touch persistent
development accounts.

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo test -p clacks-model -p clacks-sync -p clacks-client-store -p clacks-lotw --locked
cargo test -p clacks-server --lib --locked -- --test-threads=1
cargo test --workspace --all-targets --locked -- --test-threads=1
cargo test --workspace --doc --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --document-private-items --locked
cargo leptos build --release
cargo build -p clacks-log -p clacks-admin -p clacks-server-app --release --locked
git diff --check
```

From the server crate, prepare/check SQLx metadata with SQLX_OFFLINE=false and the
disposable migrator URL as DATABASE_URL. From the workspace root, use one fresh
temporary SQLite URL with production client migrations and scope preparation
to clacks-client-store. Remove metadata for deleted queries. Do not run the two
preparations concurrently.

Use the existing native macOS enrollment path and bounded relevant application
checks, not interactive automated harnesses. Windows CI remains disabled under
the user's instruction; do not claim native Windows or hosted acceptance unless
it was actually performed. Commit/push/fold actions require the user's requested
delivery instructions; this plan does not choose a new commit layout itself.

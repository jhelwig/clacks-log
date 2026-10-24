# Durable Offline Client Storage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for native execution, or superpowers:subagent-driven-development if the user selects that method. Implement this plan task-by-task; checkbox steps track execution. Read both the specification and this plan before changing production code.

**Goal:** Deliver issue #6's production SQLite client store, preserving native records, local working values, operating defaults and synchronization evidence through restart and interruption.

**Architecture:** Add `lib/clacks-client-store`, using SQLx and fully relational tables shared by accepted/working/original/prepared payload roles. Purpose-specific transactional operations call the existing `clacks-sync` core; ordinary snapshot reads serve later desktop workflows. Native libraries retain domain validation, SQLite owns transaction atomicity, and runtime composition owns authentication, network work and telemetry configuration.

**Tech Stack:** Rust 1.99.1, SQLx SQLite/macros/migrations, bundled SQLite, Tokio, existing native/sync crates, `thiserror`, `anyhow`, `tracing`, `metrics`; Rust-only test tooling.

**Spec:** [Durable offline client storage](../../client-store.md), with [native values](../../native-model.md), [synchronization metadata](../../synchronization-model.md), [service evidence](../../service-evidence.md) and [production telemetry](../../observability.md).

## Global constraints

- Libraries live in `lib/<crate-name>`; thin applications live in `bin/<application-name>`. No new production executable is needed here. Inherit workspace package/dependency declarations.
- Rust 1.99.1 is the toolchain source of truth. Windows and Linux must run the real store tests; macOS/Linux/Windows architecture remains portable.
- One database per local account workspace; tags are overlapping views, not partitions or child ownership. Every identity/reference remains account-qualified.
- Frequency in whole hertz and power in whole milliwatts use positive signed-64-bit-range integers. Clacks-controlled counters use the nonnegative signed range with existing zero/nonzero rules and checked exhaustion.
- QRZ book/record IDs are canonical decimal strings; Rust handles numeric pagination. No fixed-width padding, SQL numeric casts, high/low columns or invented upstream bit-width restriction.
- Fully relational values, maps, sets, enum variants and mapping expressions; no authoritative serialized record/vector/expression blobs or application-maintained QSO query projections.
- WAL (write-ahead logging), `synchronous=FULL`, standard automatic checkpointing, `BEGIN IMMEDIATE` writes, snapshot reads; no custom writer queue or checkpoint worker. Network/user interaction stays outside transactions.
- Fixed SQL belongs in query files, included through `sqlx::query_file!` / `sqlx::query_file_as!`. Commit offline metadata. Bind dynamic values and test dynamic structure against the real schema.
- Startup migrations are embedded, forward-only and transactional per migration. Unsupported newer schemas are rejected without database changes. Preserve enrollment, local data, pending work and comparison evidence.
- Check stale edit/review bases within the committing transaction. No additional local record/save/checkpoint generation counters; retain existing core membership and transfer-progress generations.
- No per-edit history, generic repository framework, temporary replacement store, SQL console, built-in backup/restore, automatic clone detection or enrollment-time merge of unrelated populated logs.
- Rust-only executable maintenance and fixture tooling. No Python or xtask. TrustedQSL (TQSL) sources remain strictly reference-only: never compile, execute, link, wrap or use their outputs as a test oracle.
- No production unchecked indexing, `unwrap` or `expect`; use meaningful checked conversions/errors. Named module files, no `mod.rs` outside tests, no facade/glob re-exports, standalone rustdoc on public and private documented items.
- Use `thiserror` for meaningful leaf errors and `anyhow` for contextual propagation, retaining error sources. Expected stale/review/unchanged outcomes are result data.
- Follow ordinary `#[instrument(skip_all, ...)]` and framework lifecycle facilities, default targets, safe direct fields, truthful commit events and bounded metric dimensions. No telemetry-only wrappers, generic completion machinery or duplicate error logging.
- Preserve all pre-existing work, including unrelated desktop-toolkit notes. Keep commits independently meaningful with substantive message bodies. Do not push/create a PR until authorized; capability #6 remains open until delivered.

## Review focus

1. An editor or resolution view saves after a background update: reject stale input without a lost update or a new local counter (Tasks 8–9).
2. A valid pending QSO/profile references a deleted tag/equipment record: retain it and its original evidence while unrelated work proceeds (Tasks 5, 8–10).
3. A newer-schema or unrelated file is opened, including simultaneous startup: do not migrate, reinitialize or change its data (Task 3).
4. A staged service unit or original witness outlives its former current envelope: preserve dependencies and do not certify partial account receipt (Tasks 6–7, 10–11).
5. A large QRZ decimal ID, unknown recognition label, null set element or midnight-spanning contact reaches persistence: round-trip without truncation, normalization drift or duplicate rows (Tasks 1, 4–6, 12).

## Execution preparation and dependency decision

Use the worktree skill when execution is authorized. Capture the current status and preserve the documentation already in progress; the plan does not authorize discarding or sweeping those changes into unrelated commits. Record baseline workspace verification before production changes.

Start dependency resolution with these explicit candidates, using `default-features = false`:

- `sqlx = "=0.9.0"`, features `runtime-tokio`, `sqlite-bundled`, `macros`, `migrate`.
- `tokio = "=1.54.1"`, library features `rt`, `sync`, `time`; store dev-dependency additionally enables `macros`, `rt-multi-thread`.
- Standard `sqlx-cli` 0.9.0 installed with `--locked --no-default-features --features sqlite` for metadata preparation; not a workspace task dispatcher or application dependency.
- Reuse workspace `tempfile`, `serde`, `thiserror`, `anyhow`, `tracing`, `metrics` and test collectors where their owning tests need them. Do not add a general serialization/storage framework.

The selected versions build under the repository toolchain. Task 3 verifies Rust compatibility, resolution, bundled SQLite, native C compilation, notices and Windows/Linux operation before relying on them. Do not infer an OpenSSL requirement from SQLite's native C implementation. No PostgreSQL binding is selected here.

Sources: [SQLx features](https://docs.rs/crate/sqlx/0.9.0/features), [CLI features](https://docs.rs/crate/sqlx-cli/0.9.0/features), [Tokio release](https://docs.rs/crate/tokio/1.54.1). Use SQLite and standard SQLx per-migration transactions for contention; no separate startup lock.

## File and responsibility map

All paths below are repository-relative. Additional SQL files follow the concrete table/operation naming convention below; do not consolidate unrelated operations into a generic dispatcher.

| Path | Responsibility |
| --- | --- |
| `lib/clacks-model/src/scalar.rs`, `error.rs`, `service/destination.rs` | Quantity bound, decimal QRZ identities, production timestamp accessors needed for relational mapping. |
| `lib/clacks-sync/src/clock.rs`, `record.rs`, `lineage.rs`, `account.rs`, `membership.rs`, `client.rs`, `transfer.rs`, `compaction.rs`, `recovery.rs` | Enforce the selected counter range at actual input/transition boundaries. Reuse existing core operations. |
| `lib/clacks-client-store/Cargo.toml`, `README.md`, `build.rs`, `src/lib.rs` | Production package, documented usage, SQL/migration change tracking and explicit module declarations. |
| `src/store.rs`, `src/error.rs`, `src/identity.rs` | Opening/settings, private pool, account binding and meaningful storage errors. |
| `src/migration.rs` | One read-only preflight and embedded migration execution. |
| `src/payload.rs`, `src/payload/contact.rs`, `shared.rs`, `definitions.rs`, `definitions/expression.rs`, `definitions/plan.rs`, `service.rs` | Typed relational domain mappings and strict hydration; expression chains and output plans have separate mapping modules. |
| `src/metadata.rs`, `src/metadata/clock.rs`, `lineage.rs`, `client.rs`, `client/observation.rs`, `client/original.rs`, `client/prepared.rs`, `transfer.rs` | Relational metadata, role composition, shared observations, original comparison evidence, frozen proposals and transfer state. |
| `src/save.rs`, `src/operating.rs`, `src/synchronization.rs`, `src/transfer.rs`, `src/retention.rs` | Concrete real-transaction workflows, not another planner. |
| `src/read.rs`, `src/read/history.rs`, `review.rs` | Snapshot lifetime, local-working queries, typed filtering and account-wide pending review. |
| `migrations/0001_workspace.sql` through `0008_query_indexes.sql` | Ordered append-only production schema, described below. |
| `sql/<owner>/<operation>.sql` | Individual checked SQL statements; e.g. `sql/payload/insert.sql`, `sql/client/select.sql`, `sql/window/append_installed.sql`. SQL files, not Rust inline strings, own fixed queries. |
| `.sqlx/query-*.json` at workspace root | Standard SQLx offline metadata, generated against the migrations. |
| `tests/support/mod.rs`, `tests/support/telemetry.rs` | Test-only native recipes, real temporary databases and collectors; no public test constructors. |
| `tests/opening.rs`, `native_values.rs`, `service_values.rs`, `local_saves.rs`, `operating.rs`, `preparation.rs`, `transfers.rs`, `transfers/`, `recovery.rs`, `reads.rs`, `interruptions.rs`, `observability.rs` | Public production-operation acceptance tests. Private mapper tests stay beside their owning modules until public operations can exercise them. |
| `.gitattributes`, `.github/workflows/rust.yml`, `README.md`, owning docs/crate guides | SQL line endings, offline metadata checks, real Windows/Linux validation and accurate integration guidance. |

### Migration ownership and structural rules

Migrations introduce real tables in dependency order, not a fake database replaced later:

1. `0001_workspace.sql`: `store_account`, `record_identity`, `payload`.
2. `0002_contact_values.sql`: QSO, station/location, station-reference, QSO-tag and copied-equipment tables.
3. `0003_shared_values.sql`: tags, profiles, equipment configurations, groups/catalog bindings, output profiles and relational export expressions/plans.
4. `0004_service_values.sql`: all existing service payload/report/query/projection/map tables, exact external keys and support child structures. Introduce the record-qualified stamp/vector structural parents needed by support here; a reference to a baseline ID does not assert that its interpretation is already installed.
5. `0005_sync_state.sql`: coverage/lineage/baseline interpretations, accepted envelopes, observations/originals/pending/prepared roles, local counters, membership, windows and progress, reusing the stamp/vector parents from 0004.
6. `0006_operating_and_views.sql`: local defaults/loaded-profile snapshots, local-working views and current external-binding relation.
7. `0007_local_review.sql`: durable typed core-produced review context, sharing the existing payload/original/observation/envelope reference tables.
8. `0008_query_indexes.sql`: measured history/review/reference/dependency access indexes.

Root payload mapping and role mapping are separate responsibilities; an early task may test a private mapper in a real SQLx transaction without adding a public API solely for tests. At every committed task boundary the complete migration sequence so far must apply cleanly with foreign keys enabled. If a later referenced table must exist earlier for correct constraints, create that structural parent in its first dependent migration; do not insert through temporarily disabled constraints.

Use SQLite `STRICT` tables, qualified foreign keys, complete variant/null checks and domain constructors on hydration. A deleted payload has no live root; a live payload has exactly its kind's root. Structural checks that span tables are enforced by the production mapper and rejected on read; do not claim a SQL `CHECK` alone can validate arbitrary child trees. Root selection precedes pagination/joins to avoid duplicate QSOs.

## Production interfaces carried between tasks

Use the owning modules, without `pub use` facades. Signatures below fix responsibility and result distinctions; rustdoc supplies their exact preconditions. The store operations in this inventory are `async fn`s; `Result` denotes `anyhow::Result`, retaining concrete causes. Domain structs/enums are not async.

- `store::ClientStore::create(path: &Path, account: AccountId, settings: StoreSettings) -> Result<Self>`: exclusively initialize a new local-only workspace; never overwrite an existing file.
- `store::ClientStore::open(path: &Path, account: AccountId, settings: StoreSettings) -> Result<Self>`; `close(&self) -> Result<()>`.
- `store::StoreSettings { max_connections: u32, busy_timeout: Duration }`, default 4 and 5 seconds. Pool fields remain private; writes use SQLx transaction tracking.
- `identity::EnrollmentBinding { account: AccountId, server: String, actor: ClientActorId, credential_reference: String }`; `identity::bind(store: &ClientStore, expected: Option<&EnrollmentBinding>, binding: EnrollmentBinding) -> Result<()>` installs caller-supplied metadata under an exact existing-binding/account check. First enrollment requires the fresh workspace required by D56; explicit reauthorization of an already enrolled same-account workspace preserves existing values, old actor evidence and pending/frozen work. No authentication, automatic resubmission/relabeling or fabricated checkpoint; credentials remain protected-store references. Later enrollment composition owns the verified server/account context.
- `save::load(store: &ClientStore, key: RecordKey) -> Result<Option<ClientRecord>>`.
- `save::save(store: &ClientStore, key: RecordKey, expected: Option<&ClientRecord>, next: RecordState) -> Result<SaveOutcome>`; `SaveOutcome::{Saved(ClientRecord), Unchanged(ClientRecord), Stale { current: Option<ClientRecord> }}`. `None` is a local expected-no-row precondition, not certified shared absence.
- `save::change_tags(store: &ClientStore, expected: &[ClientRecord], add: &BTreeSet<TagId>, remove: &BTreeSet<TagId>) -> Result<Vec<SaveOutcome>>`; `save::delete_tag(store: &ClientStore, expected_tag: &ClientRecord, expected_members: &[ClientRecord], choice: TagDeletionChoice) -> Result<TagDeleteOutcome>`. `TagDeleteOutcome::{Applied(TagDeletionPlan), Stale { current: Option<Box<ClientRecord>> }}`. Deletion rechecks the complete membership read basis as well as the tag, so a changed affected set cannot silently broaden a previously previewed deletion.
- `operating::OperatingState { current: Option<CurrentOperatingValues>, loaded: Option<LoadedProfileBaseline> }`; `load(store: &ClientStore) -> Result<OperatingState>`; `save(store: &ClientStore, expected: &OperatingState, next: &OperatingState) -> Result<OperatingSaveOutcome>`; `load_profile(store: &ClientStore, expected: &OperatingState, profile: &ClientRecord, equipment: Option<&ClientRecord>) -> Result<OperatingSaveOutcome>`. `OperatingSaveOutcome::{Saved(OperatingState), Stale(OperatingState)}`.
- `synchronization::prepare(store: &ClientStore, key: RecordKey) -> Result<PrepareOutcome>`; `PrepareOutcome::{NotEnrolled, Core(Box<Preparation>)}`; `receive(store: &ClientStore, incoming: &AcceptedRecord) -> Result<ClientUpdate>`; `reject(store: &ClientStore, key: RecordKey, reason: &RejectionReason) -> Result<ClientUpdate>`; `resolve(store: &ClientStore, expected: &ClientRecord, review: &ReviewInput, chosen: RecordState) -> Result<SaveOutcome>`.
- `synchronization::observe_account(store, expected: Option<&(AccountMetadata, MembershipSnapshot)>, next: &AccountMetadata, membership: &MembershipSnapshot) -> Result<bool>` stores an authenticated coordinator's supplied observation under the exact prior-value guard. `false` means the basis changed. It neither fabricates metadata from manifests nor advances installed receipt. `ReadSnapshot::account_view` and `checkpoint` keep those two meanings distinct for runtime diagnostics.
- `transfer::remember(store: &ClientStore, manifest: &WindowManifest) -> Result<()>`; `progress(store: &ClientStore, source: &InstalledCheckpoint, mode: &WindowMode) -> Result<WindowProgress>`; `install(store: &ClientStore, source: &InstalledCheckpoint, mode: &WindowMode, incoming: &[AcceptedRecord]) -> Result<InstallBatch>`; `finish(store: &ClientStore, source: &InstalledCheckpoint, mode: &WindowMode) -> Result<InstalledCheckpoint>`. `finish` returns a checkpoint reloaded after commit, not the core's uncommitted update. Task 10 defines first full installation when no checkpoint exists.
- `read::snapshot(store: &ClientStore) -> Result<ReadSnapshot>` owns one SQLx transaction; `ReadSnapshot::record(&mut self, key: RecordKey)`, `history(&mut self, query: &HistoryQuery)`, `review(&mut self, query: &ReviewQuery)`, `operating(&mut self)` read within it; `close(self) -> Result<()>`. Exact row/result types are set in Task 12.

Private mappers use `&mut SqliteConnection` borrowed from the caller's SQLx transaction. Their functions never acquire another pool connection or commit. Typed inserts/readers return native values or internal row IDs; they do not expose arbitrary SQL execution. Do not build a public generic transaction callback to make import composition convenient; the later importer can extend the purpose-specific facade and reuse private writes in one transaction.

---

## Task 1: Native quantity bounds and decimal QRZ identities

**Files:** Modify `lib/clacks-model/src/scalar.rs`, `error.rs`, `service/destination.rs`, existing native/service tests and callers in `lib/clacks-sync/tests/`; add `lib/clacks-model/tests/qrz_ids.rs`. Update model/service crate guidance as needed.

**Interfaces:** Preserve `Hertz::new(u64)`, `Milliwatts::new(u64)` and getters, but reject above `i64::MAX`. In `service::destination`, add validated `QrzBookId` / `QrzRecordId` decimal-text types with `new(&str)`, `as_str()`, `Display`, string Serde and numeric `Ord`; only `QrzRecordId::next_for_pagination() -> Self` performs decimal increment without a machine-width limit. Book ID zero remains invalid; record ID follows existing domain rules. `RemoteCollection::Qrz.book_id` becomes `Option<QrzBookId>` and `RemoteRecordId::Qrz` carries `QrzRecordId`. Derive/implement ordering consistently with equality; do not compare numeric identities lexically through the parent enum.

- [ ] Write `quantities_fit_sqlite_integer` asserting 1 and 9,223,372,036,854,775,807 succeed; 0 and 9,223,372,036,854,775,808 fail through construction and deserialization, retaining concrete `ValueError` causes.
- [ ] Write `qrz_decimal_identity_and_pagination` asserting `"00010" == "10"`, 9 < 10 numerically, `"18446744073709551615".next_for_pagination()` is `"18446744073709551616"`, decimal carry across many nines, invalid sign/whitespace/fraction/empty input rejection, and known book zero rejection. Round-trip through actual destinations, observations and association keys, not only standalone scalars.
- [ ] Run `cargo test -p clacks-model --test values --test qrz_ids --locked --offline`; confirm the new expectations fail for current behavior before implementing.
- [ ] Implement the validated types and range error in their owning modules. Add `UtcTimestamp::unix_seconds()` and `subsecond_nanoseconds()` production accessors for relational mapping; retain all timestamp semantics. Adapt every QRZ literal/caller/fixture using explicit canonical strings; no unchecked convenience constructors.
- [ ] Run native and synchronization suites; existing projections/observation equality must remain unchanged except the intentional identity representation. Update existing serialization fixtures using project tooling/editor-authored text as appropriate; no Python or TQSL execution.
- [ ] Commit the domain change with a body explaining SQLite representability and why externally assigned QRZ IDs are not bounded by the same range.

## Task 2: Bound synchronization counters at native transition boundaries

**Files:** Modify `lib/clacks-sync/src/error.rs`, `clock.rs`, `record.rs`, `lineage.rs`, `account.rs`, `membership.rs`, `client.rs`, `transfer.rs`, `compaction.rs`, `recovery.rs`, service input validators where they consume account metadata; add `tests/counter_bounds.rs` and extend owning tests.

**Interfaces:** Keep existing transition signatures and `u64` fields where practical. Add owning-type `validate()` methods where currently missing (not a generic numeric wrapper), and a private checked-counter increment used by actual allocation sites. All composites supplied to transitions must be validated before comparison/shortcut/side effects. No SQLite dependency in `clacks-sync`.

- [ ] Write `counter_maximum_is_valid_but_cannot_advance`: max signed counter round-trips and compares; vector increment, account advance, membership change and window progress increment at that maximum return `MetadataError::Overflow` without changing input. Explicit zero vector components remain omitted; accepted revision zero is rejected; origin/checkpoint zero remains valid.
- [ ] Write `oversized_composites_cannot_enter_transitions`: direct public-field construction and deserialized account/baseline/stamp/coverage/membership/progress values above the signed maximum fail even through retry/covered/unchanged shortcuts. Include server components equaling accepted revision, required dependency stamps and retained witnesses.
- [ ] Run `cargo test -p clacks-sync --test counter_bounds --locked --offline` and confirm the existing unrestricted behavior fails the assertions.
- [ ] Implement range checks at existing owning constructors/validation and checked allocation sites. Inspect every `u64` occurrence in model/sync so no counter carrier bypasses the selected range; recognition band bounds are frequency data, grid rational denominators are derived geometry, not stored counter fields.
- [ ] Run `cargo test -p clacks-sync --all-targets --locked --offline` and native compatibility tests. Commit with context that the logical synchronization algorithms and causal interpretation are unchanged.

## Task 3: Real database opening, migrations and query-build integration

**Files:** Root manifests/lockfile; create store manifest/README/build script, `src/lib.rs`, `store.rs`, `identity.rs`, `error.rs`, `migration.rs`, `migrations/0001_workspace.sql`, `sql/opening/identity.sql`, `migration_history.sql`, `sql/workspace/create.sql`; `.sqlx/`; modify `.gitattributes` and CI; test `tests/opening.rs`.

**Interfaces:** Implement the create/open/close/settings and binding signatures above. `StoreError` has concrete incompatible-schema, wrong-account, invalid-database/value categories; SQLx/I/O errors retain their sources through `anyhow` rather than forwarding enum wrappers. No public pool/connection getter.

- [ ] Add tests `open_rejects_wrong_account_or_unrelated_file`, `create_never_overwrites`, `startup_uses_wal_full_and_foreign_keys`, `newer_schema_is_not_modified`, `concurrent_opens_of_the_current_schema_use_sqlite_contention_handling` and `binding_checks_account_and_prior_binding`. Assert actual pragma values on every acquired connection in private tests, no fabricated enrollment/checkpoint, and unchanged database bytes/contents on rejection (ephemeral shared-memory locking is not a user-data write). Task 9 adds reauthorization with genuine pending/frozen work.
- [ ] Resolve the candidate dependencies and record actual selected SQLite version/notice requirements. Run compile/tests under Rust 1.99.1. Check `cargo tree -e features -p clacks-client-store` for no default network backends or OpenSSL. Bundled SQLite is a verified native-build requirement, not a claim of Rust-only SQLite internals.
- [ ] Use one read-only existing-file preflight with an ordinary snapshot transaction over existing SQLx history and account. Do not set WAL or optimization pragmas before it. Use standard SQLite contention handling and SQLx per-migration transactions; concurrent migration attempts can fail and be retried by restarting against fresh history. Do not introduce a startup lock or migration-retry framework.
- [ ] Keep SQLx migration history as the sole schema-version record. Do not add application identifiers, workspace-version tables or user_version copies. Separate create from open; do not create empty missing files on normal open, delete corruption, or wrap all migrations in one outer transaction that would undo earlier successful migrations.
- [ ] Add `*.sql text eol=lf`, stable-Rust change tracking for migrations and SQL directories, and the first real offline metadata. Resolve any special SQLx migration bootstrap queries with normal SQLx facilities; variable SQL metadata checks use bound parameters/schema introspection without inventing a generic SQL loader.
- [ ] Introduce the CI metadata check at the same time as the first query macros. Install pinned CLI, create a temporary SQLite database from migrations, run `cargo sqlx prepare --check --workspace -- --all-targets`, then run normal checks with `SQLX_OFFLINE=true` and `DATABASE_URL` absent. Add `.sqlx/**` to existing PR/main path filters, never all-branch push triggers. Derive compiler from `rust-toolchain.toml`.
- [ ] Verify all opening tests and offline build, and commit the production opening/migration capability with SQLx metadata. Describe startup safety and failure preservation in the commit body.

## Task 4: Relational QSO, station, location and equipment snapshots

**Files:** `src/payload.rs`, `src/payload/contact.rs`, `migrations/0002_contact_values.sql`, `sql/payload/insert.sql`, `select.sql`, `sql/contact/insert.sql`, `select.sql`, `sql/station/insert.sql`, `select.sql`, `references.sql`, `sql/location/insert.sql`, `select.sql`, `sql/equipment/snapshot_insert.sql`, `snapshot_select.sql`, `sql/contact/tags.sql`; private mapper tests and `tests/native_values.rs` when public reads become available.

**Interfaces:** Private `payload::insert(conn: &mut SqliteConnection, key: RecordKey, state: &RecordState) -> Result<i64>` and `payload::load(conn: &mut SqliteConnection, key: RecordKey, payload_id: i64) -> Result<RecordState>` dispatch to owning typed mappers. The whole caller transaction owns their lifetime; no mapper commit/pool acquisition. Add remaining kind implementations in Tasks 5–6 rather than leaving public unsupported variants in the final store.

- [ ] Write mapper round-trip tests for both QSO sides, independently optional operators/names, frequency-only/unrecognized band, band-only receive override, explicit point endpoints, grid areas of lengths 2–12, two references in one group and group-only participation. Assert integer coordinates and original labels survive exactly; no directory/fallback material becomes recorded facts.
- [ ] Include sent/received reports, optional equipment fields/power, crossing-midnight/equal start/end, empty tags and duplicate-null-reference attempts. Assert 2 tag joins still represent one QSO payload, two-character imported areas survive, and owned child corruption/missing live roots fails hydration.
- [ ] Run mapper tests against a real migrated temporary file and observe the missing mapping/schema failures.
- [ ] Implement `STRICT` schema/qualified child foreign keys and typed query files, including separate uniqueness for null/non-null reference identifiers. Positive integer frequency/power checks apply to all snapshot columns. Hydrate through existing native constructors/validators without re-normalizing saved values against current catalogs.
- [ ] Query domain columns directly in a private integration check (range filtering on frequency/power, joining side-specific references), proving they are not serialized payloads. Regenerate offline metadata, run model/store tests, and commit the relational contact mapping.

## Task 5: Shared tags, profiles, equipment, groups and export definitions

**Files:** `src/payload/shared.rs`, `definitions.rs`, `migrations/0003_shared_values.sql`; query directories `sql/tag/`, `profile/`, `group/`, `export/`; extend native mapper tests.

**Interfaces:** Extend `payload::insert/load` to all non-service `RecordValue` variants. Persist export expressions as the existing `ValueExpression` variants/child links, not an application-wide property or expression framework. Use existing `compile_definition` after strict hydration.

- [ ] Write `shared_payloads_round_trip_all_roles` covering duplicate tag/profile names, every upload-policy state, profile optional equipment reference, copied equipment, normalization sequence order, unknown group associations, catalog display/detail binding and ordinary/targeted output mappings.
- [ ] Write expression tests covering every native expression/condition/partition/expansion variant, nested Capture/Join, ordered fields and unrecognized recorded reference groups. Assert malformed cycles/variant fields/duplicate keys fail with group/path context; a valid unknown association needs no fabricated definition.
- [ ] Run tests to see mapper failures, then implement typed tables/query files and strict native hydration. Bare identity foreign keys establish structural references only; deleting/receiving deleted equipment/tag state must not cascade into QSO/profile snapshots.
- [ ] Test known deleted/missing tag/equipment identities can still be referenced by pending payloads. No current catalog refresh rewrites a recorded mode/band/reference. Regenerate offline metadata and commit these production shared-value mappings.

## Task 6: Relational service payloads, support and exact external identities

**Files:** `src/payload/service.rs` with focused named child modules if needed, `src/metadata.rs`, `src/metadata/clock.rs` for support's stamp/vector mapping, `migrations/0004_service_values.sql`; query files under `sql/service/`, `report/`, `external/`, `stamp/`, `vector/`; `tests/service_values.rs` and private mapper tests.

**Interfaces:** Complete `payload::insert/load` for every existing `ServicePayload` variant. Add private public-descriptor, typed report, query and field-map readers/writers sharing owner-qualified storage, not serialized carriers. `external_key` uses QRZ text and exact LoTW composite identity. Account/namespace equality uses the existing native registry semantics.

- [ ] Add table-driven native recipes for Destination, Imported, Attempt, Result, Observation, Association, Disposition, Suppression, Download and Acquisition payloads. Test exact public destination copies, optional unknown book ID, huge decimal QRZ IDs, both services' frozen projections, source/mapping provenance, coverage sets, batch members and returned external-attempt correlation.
- [ ] Add `reported_states_remain_distinct`: NotReported/Clear/Value, explicit empty reference set, absent counterpart versus present all-unreported counterpart, minute/second start coverage, receipt/confirmation ordering scopes and optional result diagnostic payloads survive round-trip.
- [ ] Verify immutable native evidence equality before and after persistence, including LoTW mapping metadata, independently from `same_payload` comparison. No request/signing/normalization is rerun to reconstruct historic submitted values.
- [ ] Implement the native variant tables and children with ordinary query files. Implement support's record-qualified stamp/vector parents and their private typed mapping now; Task 7 extends the same modules with baseline interpretation/client roles. An unknown referenced baseline remains unknown, not a fabricated definition. Do not disable foreign keys or store credential/key/provider-response bodies as a shortcut.
- [ ] Test external identity equality differs from fuzzy candidate matching and that same-digit IDs in different destinations/accounts remain distinct. Regenerate metadata, run service/native/sync tests, and commit the service persistence capability.

## Task 7: Synchronization metadata, client roles and strict reconstruction

**Files:** `src/metadata.rs`, `src/metadata/{clock,lineage,client,transfer}.rs`, `migrations/0005_sync_state.sql`; `sql/stamp/`, `vector/`, `coverage/`, `baseline/`, `lineage/`, `client/`, `original/`, `prepared/`, `witness/`, `window/`; `tests/metadata.rs`.

**Interfaces:** Implement `save::load` and private `metadata::client::load(conn, key) -> Result<Option<ClientRecord>>`, `store(conn, row: &ClientRecord) -> Result<()>`; private metadata readers reconstruct `EvidenceView` backing values from owned `Lineage`/`Vec<Baseline>`. The returned borrowed view never outlives those values/transaction.

- [ ] Write `all_client_roles_reopen_independently` using genuine core-created pending/prepared/retained/witness values: every `Observation` and `OriginalBasis` variant, exact stamps, vectors, coverage zeros, membership acknowledgments, baseline lineage and timeline boundary positions must round-trip.
- [ ] Assert original/working/prepared can point to different complete payloads; later working updates do not mutate shared payloads. Account/kind/native-ID mismatches, incompatible duplicate baseline meanings, missing required children and malformed timestamps fail explicitly. No scalar cross-timeline ordering is introduced.
- [ ] Write `service_support_and_staged_dependencies_remain_reachable` covering ExactFact/CoveredEffect, original acquisition stamps, frozen destination copies, manifest coupled member stamps/deletion flags, staged envelopes and completed unit sets.
- [ ] Implement qualified tables and strict readers through ordinary SQLx transactions. Preserve map uniqueness, sequence ordinals and set uniqueness. Add no local record/save/checkpoint generation columns; existing membership/window generations are present.
- [ ] Expose only useful typed record reads, not a public arbitrary `store_client_record` bypass. Use private unit tests to exercise intermediate mapping, then reuse recipes through real operations below. Regenerate metadata, verify all mapper/core tests and commit.

## Task 8: Durable local saves, tags and operating defaults

**Files:** `src/save.rs`, `src/operating.rs`, `migrations/0006_operating_and_views.sql`; query files `sql/save/`, `operating/`, `views/`; tests `local_saves.rs`, `operating.rs`, `observability.rs`.

**Interfaces:** Implement the `save`, `change_tags`, `delete_tag` and operating signatures defined above. Expected read basis uses existing `ClientRecord` / `OperatingState` values; do not add another concurrency token type or local counter. Define `Unchanged` from native-state equality without deleting genuine existing pending/frozen work.

- [ ] Write `save_reopen_without_server`: create valid QSO/tag/profile/equipment, save/edit/delete/reopen; no local-only save fabricates an accepted stamp, actor or installed prefix. Initial local-only original/shared evidence is explicitly unavailable, not certified absence.
- [ ] Write `stale_edit_does_not_replace_intervening_save`: read E, commit B, attempt save using E, assert Stale/current B and no changed payload roles or mutation metric. Repeat for concurrent deletion and operating-default changes. No UI notification is required for this test; the transaction boundary is responsible.
- [ ] Write all three tag-deletion cases using `plan_tag_deletion`: unique/shared/untagged contacts, pending deletions, and complete QSO tag sets. Assert stale if the tag or any previewed member changed, or a new QSO acquired that tag after preview. A tag deletion plus its local contact effects commits together or rolls back together. Selected-group membership edits use bounded transactions, preserving completed units if later ones fail; no separate membership sync records.
- [ ] Write profile-load tests for complete station replacement, absent default equipment retaining current copies, operator/power unchanged, baseline containing complete loaded copies, restart preservation, and shared profile/equipment edits leaving defaults/historical QSOs unchanged.
- [ ] Implement immediate read/check/write operations. First local edits retain their original; subsequent edits change working values only. Missing/deleted references remain valid local pending work and later prepare/review handles them; never recreate targets or let a structural foreign key erase the work.
- [ ] Add significant `info` save/profile/tag operation spans and committed events, with default targets and no arguments/payload dumps. Count local QSO create/update/delete only after commit; unchanged/stale/rolled-back operations do not count as committed mutations. Test those decisions through real store methods.
- [ ] Regenerate metadata, run save/operating/readiness regressions and commit the durable offline write workflows.

## Task 9: Frozen preparation, responses and stale-safe resolution

**Files:** `src/synchronization.rs`, `src/metadata/client.rs`, `migrations/0007_local_review.sql`, `sql/prepare/`, `sql/review/`; tests `preparation.rs`, `observability.rs`.

**Interfaces:** Implement `prepare`, `receive`, `reject`, `resolve`. Core returns remain core types. Prepare derives mandatory QSO-tag/profile-equipment conditions from actual shared observations read in the same transaction; do not add another acceptance algorithm. If shared reference evidence is missing, return the core's review/unavailable outcome without fabricating conditions. Extend the core's owning reference-query API only if necessary for real adapter reuse; do not copy its logic into a telemetry helper or test-only public function.

- [ ] Write `prepared_counter_and_payload_commit_before_send`, `retry_reuses_prepared_values_after_restart`, `later_edit_survives_accepted_prepared_response`, `rejection_keeps_original_and_frozen_work` and `unenrolled_cannot_prepare`. Assert store methods never return a sendable change until commit, and counters are not reallocated on Retry.
- [ ] Write `stale_resolution_preserves_latest_row`: save a new working/shared/retained state or different core review context after review was read; resolution with the old basis returns Stale and current data. Compare both the current row and the shown review against saved state inside the transaction. Existing record-level conflict policy applies, with no automatic field/tag merge or new resolution UI.
- [ ] Run failing store tests, then implement each workflow by loading current state/evidence inside `BEGIN IMMEDIATE`, invoking the existing core function, persisting its result/counter together, and committing. Shared reference absence must come from complete installed evidence, not a bare identity-row lookup.
- [ ] Persist actual `ClientUpdate.review` and `Preparation::Review` values alongside their returned client rows. `local_review` is keyed by the record and stores the core's `ReviewReason` discriminator/variant fields, original basis, local payload, shared observation and ordered retained envelope references through the same relational families. It is local core-produced context, not another clock or synchronized fact. Do not turn a rejection with no core review into a fabricated conflict. Clear/refresh review only when a later core transition establishes the replacement outcome. Test exclusion/missing-reference review survives reopen with its exact values/context.
- [ ] Test explicit same-account reauthorization changes only the current enrollment binding. Old actor counters, originals, witnesses and frozen proposals remain; no automatic author rewrite or submission happens. Binding a separately populated local-only workspace still requires the D56 export/import flow.
- [ ] Emit explicit committed resolution milestones at `info`, routine prepare/retry detail at `debug`; do not label a persisted prepared proposal as an upload or acceptance. Test context remains useful when lower-level spans are filtered.
- [ ] Regenerate metadata, run client/core/store scenarios and commit guarded preparation/receipt/resolution.

## Task 10: Immutable windows, coupled installation and truthful checkpoints

**Files:** `src/transfer.rs`, `src/metadata/transfer.rs`, query files `sql/window/`, `sql/checkpoint/`, `sql/external/`; tests `transfers.rs` and scenario modules in `tests/transfers/`; extend `lib/clacks-sync/src/transfer.rs` and its tests for a first full window without a previous checkpoint.

**Interfaces:** Implement `remember/progress/install/finish` above. Derive complete metadata support from the persisted immutable manifest; `install` calculates its core batch from current rows, not caller-supplied precomputed updates. Maintain current accepted external-binding state as part of the same complete-unit installation.

- [ ] Write `manifest_identity_rejects_unequal_replacement`, `partial_unit_stages_without_binding_or_receipt`, `complete_unit_installs_atomically`, `window_completion_requires_every_requirement_and_interpretation`, `first_full_window_requires_complete_verification`, `lost_ack_reloads_committed_checkpoint` and `native_stale_proposal_cannot_race_progress`.
- [ ] Include the concrete QSO+association+disposition dependency closure, ExactFact/CoveredEffect distinctions, occupied external bindings and unknown namespace identities. Pending/recovered alternative associations must not take current ownership or reactivate superseded/deleted associations.
- [ ] Run failures, then implement short immediate batch transactions loading the actual current clients/progress/interpretation and invoking `plan_batch`. Commit accepted values, pending/original/witness updates, staged inputs, installed keys, completed units, external binding and generation together. Incomplete units remain staged.
- [ ] Implement `finish_window` verification in its transaction, persist the guarded checkpoint update, commit, then reload and return that checkpoint. An individual accepted response, high record revision or advertised source head cannot advance receipt.
- [ ] Support the initial full account installation without inventing an already-installed checkpoint. Factor the core's complete-window verification into a shared private operation; add a production `clacks_sync::transfer::finish_initial_window(manifest: &WindowManifest, progress: &WindowProgress, installed: &[AcceptedRecord], evidence: &EvidenceView<'_>) -> anyhow::Result<InstalledCheckpoint>` restricted to `WindowMode::Full`. It verifies all requirements, units and interpretation using the same checks as ordinary completion. The adapter guards absence of a current installed checkpoint, inserts the returned checkpoint only after verification in the same transaction, commits and reloads. An incremental first window or incomplete seed must fail; no temporary fabricated checkpoint, local server acceptance, actor or timeline is needed. Later enrollment/network code supplies genuine source metadata/manifests through these production operations.
- [ ] Check unchanged manifest/progress replays and stale core progress generations. Persist missing prerequisites as staged work or report explicit core failures, never a successful partial installation. Count installed records after commit separately from local QSO saves.
- [ ] Regenerate metadata and run the worked interruption/window cases on real SQLite. Commit transfer integration with a body explaining the acknowledgment boundary.

## Task 11: Baseline installation, recovery evidence and reachability-based cleanup

**Files:** `src/retention.rs`, `src/metadata/{client,lineage}.rs`, `src/transfer.rs`; query files `sql/baseline/`, `witness/`, `retention/`; tests `recovery.rs`.

**Interfaces:** Add `retention::install_interpretation(store: &ClientStore, expected: &InstalledCheckpoint, baselines: &[Baseline], lineage: &Lineage) -> Result<()>` for received interpretation. It validates account/current expectations, seals any required pending basis through `seal_basis` while old evidence is still readable, and preserves exact contexts. Checkpoint advancement still belongs to `transfer::finish`, not this method. Add `retention::collect_unreferenced(store: &ClientStore) -> Result<u64>` for private/local relational garbage only, returning the count of removed payload roots, never claiming server actor compaction.

- [ ] Write `pending_witness_survives_received_compaction`, `retired_actor_does_not_delete_accepted_data`, `recovery_preserves_original_stamps_and_pending_tail` and `collection_keeps_every_dependency_root`. Construct server decisions through the existing core in test recipes; production client code does not authorize a fork or choose the server's common boundary.
- [ ] Test multiple retained baseline/lineage snapshots with distinct meanings and boundaries established at later positions; no rewrite of an older baseline/witness interpretation. An incompatible received definition fails without releasing old evidence or advancing a checkpoint.
- [ ] Implement received interpretation/witness persistence in one guarded immediate transaction; use the core comparison evidence, not last-write-wins. Recovered accepted versions pass through the same receipt/window machinery under supplied authorized-server evidence.
- [ ] Test a pending original that cannot yet be sealed because its observed version is not covered by the installed checkpoint. Retain the old baseline/lineage roots and pending evidence rather than fabricate coverage, discard the edit, or claim collection succeeded for that evidence. Ordinary unrelated installation continues.
- [ ] Implement ordinary `NOT EXISTS`/foreign-key reachability deletion, preserving originals, frozen values, retained alternatives, staged inputs, immutable service facts/support, deletion memory, destinations and loaded profile snapshots. Do not add reference-count bookkeeping, per-edit history, age expiry or vacuum-on-save. Deleting only an obsolete envelope must not delete still-required semantic evidence.
- [ ] Keep history-regression warning and recovery context useful at normal telemetry levels; committed cleanup counts are scoped to removed local roots. Test rollback leaves every root intact.
- [ ] Regenerate metadata, run core compaction/recovery/service-retention and store tests, then commit persistence/reclamation with its recovery limitations documented.

## Task 12: Consistent working-state reads, filters and review

**Files:** `src/read.rs`, `src/read/history.rs`, `src/read/review.rs`, `migrations/0008_query_indexes.sql`; query files `sql/read/history.sql`, `worked_prefix.sql`, `pending.sql`, `record.sql`; `tests/reads.rs`.

**Interfaces:** `ReadSnapshot` owns a SQLx read transaction. `record` returns `Result<Option<ClientRecord>>`; `operating` returns `Result<OperatingState>`. `HistoryQuery` contains optional selected tag IDs, contacted-call prefix, inclusive UTC start/end bounds, primary band/mode labels and own callsign plus `after: Option<(UtcSecond, QsoId)>` and positive page limit. `history` returns `Result<Vec<HistoryRow>>`, where `HistoryRow { qso: Qso, pending: bool, review: Option<ReviewInput> }`, ordered by `(start, id)`. `ReviewQuery` filters optional tag/record kind and has `after: Option<RecordId>` plus positive page limit; `review` returns `Result<Vec<ReviewRow>>`, where `ReviewRow { record: ClientRecord, review: Option<ReviewInput> }`, including pending deleted/untagged records. All methods are async and return the result types inside `anyhow` as above.

Other agreed history filters (grid/reference and per-service status) extend these relational query operations in their owning feature work; implement the underlying columns/relationships now, not a hard-coded projection or a generic query language. No provider status is invented when evidence is absent. This task exposes the usable store queries needed by issue #6; the later history UI owns the complete D114 interaction.

- [ ] Write `working_view_has_one_qso_per_identity`, `pending_deletion_is_hidden_but_reviewable`, `prefix_matches_typed_callsign_and_escapes_wildcards`, `snapshot_pages_do_not_mix_commits` and `profile_defaults_read_in_same_snapshot`. Include duplicate tag names, multi-tag joins, unknown band/mode labels, group-only refs, same-second QSO ordering and before-1970 timestamps.
- [ ] Use `EXISTS` membership/reference predicates before paging, SQL local-working views rather than a second maintained projection, bound values and exact recorded labels. Define selected tag matching as any selected tag; an empty selection means account-wide. No native record is duplicated for selected tags/roles.
- [ ] Open one read transaction for all snapshot calls; establish its first read consistently before concurrent writer verification. Never reacquire the pool from within a snapshot. Close/cancellation releases the reader; demonstrate a writer commits while the existing snapshot continues reading the old consistent values.
- [ ] Inspect representative query plans for history, prefix and dependency lookups, introducing only ordinary indexes that optimize real queries. Dynamic filters use SQLx bound arguments and trusted static column fragments; no caller SQL strings.
- [ ] Regenerate metadata, run reads and prior save/transfer regressions, document intended snapshot lifetime/checkpoint effects and commit production read integration.

## Task 13: Interruption, migration recovery, CI and delivery verification

**Files:** `tests/interruptions.rs`, opening/observability tests and test-only support; update store README, workspace README/dependency notices, `.github/workflows/rust.yml`, relevant integration docs. No production crash harness executable or test-only public methods.

**Interfaces:** Use the actual store APIs and schema. Spawn the integration-test binary itself with a filtered child test and explicit test-only process protocol; do not add an xtask/maintenance dispatcher. Private unit tests may coordinate immediately around real commit boundaries; public integration cases kill a child with an actual transaction/window state in progress.

- [ ] Add parent/child interruption cases before a local commit, after commit before caller acknowledgment, after frozen preparation, during staged versus complete coupled installation and between window batches. After termination/reopen, assert either complete old or complete new state, durable counters/frozen values and no fabricated installed coverage. Synchronization uses core-created server envelopes, not a production mock server to be replaced later.
- [ ] Test a real migrated schema with successful migration N then failing N+1, restart/retry and modified applied checksum. Use test-owned migration sequences only; no deliberately broken production migrations. Exercise the same private startup path rather than a fake migration engine. Test unsupported newer schema remains unchanged.
- [ ] Test real SQLite busy timeout, cancelled reader/writer transaction cleanup, malformed live-child structures, read-only/full-page-limit database errors and corruption handling. For deterministic storage-full rollback, use SQLite page-limit facilities with deliberately large valid native text; do not fill the machine's actual disk. No save/ack success on these failures; preserve useful concrete causes.
- [ ] Test actual domain telemetry: committed mutations/installations counted once per successful operation path, stale/rolled-back/unchanged outcomes not counted as saves, no payload/secret/reference/path/SQL-bound-value leakage, bounded metric dimensions and useful committed-resolution context with debug parents filtered. Do not retest subscriber formatting or duplicate the entire suite under enabled/disabled telemetry.
- [ ] Document normal developer commands, offline metadata regeneration/checking, startup compatibility behavior, consistent backup/closed-copy requirements and the absence of built-in restore. State precisely which APIs later desktop, enrollment, directory and WebSocket work extend. Retain source-only TQSL and Rust-only tooling policies.
- [ ] Run the full command set below and inspect failures/feature tree. Ensure native Windows and Linux CI include the new integration binaries, process interruption and metadata freshness; a cross-compile alone is insufficient. Commit the verification/documentation deliverable with substantive context. Do not leave commits repairing mistakes introduced by earlier commits; fold such fixes into their owning task commits before authorized publication, retaining meaningful implementation commits.

## Metadata and final verification commands

Use the normal Rust CLI, with a temporary database outside source control:

```sh
cargo install sqlx-cli --version 0.9.0 --locked --no-default-features --features sqlite
```

Set `DATABASE_URL` to a platform-correct `sqlite://...` temporary-file URL. From the workspace root:

```sh
cargo sqlx database create
cargo sqlx migrate run --source lib/clacks-client-store/migrations
cargo sqlx prepare --workspace -- --all-targets
cargo sqlx prepare --check --workspace -- --all-targets
```

Use `SQLX_OFFLINE=false` for preparation if an inherited environment sets it. Normal build/test steps use `SQLX_OFFLINE=true` and no `DATABASE_URL`. Do not hand-edit generated `.sqlx` query metadata. CI uses the same migration source; create/clean temporary files with platform-appropriate shell steps rather than introducing a project script language or production utility for this setup.

```sh
cargo fmt --all --check
cargo test --workspace --all-targets --locked --offline
cargo test --workspace --doc --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo check --workspace --all-targets --target x86_64-pc-windows-msvc --locked --offline
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --document-private-items --locked --offline
cargo tree -e features -p clacks-client-store
git diff --check
```

The documented Rust toolchain is selected by `rust-toolchain.toml`; explicit `+1.99.1` remains available for diagnosis. Offline dependency commands require fetched locked packages. Run native Windows/Linux jobs after authorized branch publication, monitor them to completion and correct failures; do not claim planning-time documentation checks prove runtime compatibility. Final branch review checks this plan and the specification, especially stale-save guards, immutable contexts, all service variants, atomic units, schema rejection and telemetry boundaries.

## Coverage and handoff

| Specification sections | Owning tasks |
| --- | --- |
| 1–2: production boundary, file scope, encodings and native integration | 1–3 |
| 3: payload/role schema and structural versus live references | 3–9 |
| 4: vectors/ancestry/witnesses/windows | 2, 7, 9–11 |
| 5: service records and operating defaults | 5–8, 10 |
| 6: working-state read model and consistent snapshots | 8, 12 |
| 7–8: transaction obligations and worked interruptions | 8–11, 13 |
| 9: migrations, retention and failures | 3, 11, 13 |
| 10: dependency/observability boundary | 3, 8–13 |
| 11: acceptance verification | Every task's concrete cases, final Task 13 |

Plan writing does not authorize execution or close #6. Review this plan together with the specification before implementation. The user's execution-method selection governs the implementation skill. No additional product decision is needed merely because a task has ordinary SQL/Rust details.

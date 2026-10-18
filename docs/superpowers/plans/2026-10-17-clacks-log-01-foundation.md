# Domain and persistence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

A radio contact is a **QSO**; Amateur Data Interchange Format is **ADIF**; the minimum viable product is **MVP**; Universally Unique Lexicographically Sortable Identifier is **ULID**. Unicode text is native data, not an interchange encoding restriction.

**Goal:** Build the shared native model, durable local working state and transactional server storage used by the rest of Clacks Log.

**Architecture:** A Rust workspace separates pure domain rules from embedded SQLite client storage and PostgreSQL storage inside the single server application. Account-owned contacts carry their complete tag sets; recorded facts, provider data and publication evidence have separate identities and lifetimes.

**Tech Stack:** Rust, Serde, SQLite, PostgreSQL; choose compatible database-library versions in F1 rather than prescribing a custom write queue.

**Spec:** [product.md](../../../docs/product.md), [synchronization.md](../../../docs/synchronization.md), [server-persistence.md](../../../references/server-persistence.md), [adif-matching.md](../../../references/adif-matching.md). Read these with the [coordinating plan](2026-10-17-clacks-log.md).

## Global Constraints

This is a planned file/interface layout, not existing code or authorization to execute. All tasks also inherit the coordinating plan’s constraints and file/dependency contracts. Native records and protocol encodings are pinned by F1/S1 before downstream code; do not invent competing representations in an isolated task. No implementation checks below have run yet.

- Account ownership and whole-record membership semantics are fixed by D243–D251.
- The mode value uses the hierarchical enum with structured OtherMode fallback (D177); recognition differences must not create edits (D178).
- Current operator/defaults and power stay client-local; profiles/equipment are shared and contacts snapshot their values.

## Review Focus

- Crash between working-data and pending-evidence writes: F2 must restore both or neither.
- Old/new mode recognition: F1 must preserve the underlying value through an unrelated edit.
- Tag removal versus QSO deletion: F3 must preserve an active untagged contact in the first case.
- Concurrent membership/fact changes: F3 and S2 must not introduce automatic tag merging.
- Invalid optional imported values: F1 exposes diagnostics; I2 retains existing valid values.

## File structure

The Files entries below use `lib/<crate-name>` for libraries and `bin/<application-name>` for thin executable wrappers. Behavior, configuration/runtime composition and tests belong to libraries; binaries only delegate to the library entry point. The Files entries map the new implementation units and their tests. Brace groups expand to individual files in that directory. Crate manifests/module exports and required schema migrations belong to the task that first needs them; keep related source/tests together. No production file exists at these proposed paths yet.

---

### F1: Workspace, dependency checks and native value contracts

**Depends on:** None.

**Files:**

- Create/modify: `Cargo.toml`
- Create/modify: `Cargo.lock`
- Create/modify: `rust-toolchain.toml`
- Create/modify: `lib/clacks-domain/Cargo.toml`
- Create/modify: `lib/clacks-domain/src/{lib,ids,qso,mode,location,station,tags,evidence,profiles}.rs`
- Create/modify: `lib/clacks-domain/tests/native_values.rs`
- Create/modify: `docs/engineering/domain-contract.md`
- Create/modify: `.github/workflows/check.yml`

**Interfaces:**

- Consumes: Accepted product model and decision register; pinned upstream source remains reference material.
- Produces: AccountId, QsoId, TagId, ProfileId, EquipmentId; QsoRecord { id, recorded, station, tags, deletion }; TagRecord with per-service UploadPolicy::{Enable,Forbid,Neither}; Mode/OtherMode; Location with explicit point/grid-area basis. Define normalize_mode(input: ModeInput) -> Result<ModeValue, ValidationError> and effective_contact(recorded: &ContactFacts, sources: &EligibleSources) -> EffectiveContact. Keep source/provenance in the result.

- [ ] **Step 1: Add the named regression/acceptance tests and their fixtures.**

native_values::mode_recognition_does_not_edit; whole_location_precedence; grid_precision_and_basis; frequency_precedence; optional_reports_and_activity_scope. Freeze a field inventory for sent/received reports, times, own/counterpart facts, power, operator, references and comments from the product document. Write fixtures for unknown submode-only input and partial recorded location over a complete directory location.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert_eq!(unrelated_edit.mode.raw_value(), before.mode.raw_value());
assert_eq!(effective.location, partial_recorded_location);
assert_eq!(grid_from_point.precision(), 6);
assert_eq!(frequency_edit.band, derived_band);
```

- [ ] **Step 2: Run the focused tests and confirm the expected pre-implementation failure.**

Run each focused command listed in Step 4 after creating a compilable minimal target. Confirm failure comes from missing behavior or the explicitly missing module/type, rather than an unavailable database, provider or toolchain. For infrastructure/artifact tasks, use the named reproducible verification scenario instead of adding tests that merely mirror configuration.

- [ ] **Step 3: Implement the listed interfaces and behavior.**

Set workspace members to `lib/*` and `bin/*`, adding only packages needed by the current task. Production logic and application startup live in libraries; later application manifests use distinct package names and thin entry-point wrappers. Do not create an initial `crates/` directory. Select a supported toolchain and compatible crate versions at execution, record the actual choices and commit the lockfile. Freeze the public field/type inventory in `docs/engineering/domain-contract.md` before dependent tasks implement storage, mappings or UI. Use ordinary Serde representations and stable underlying mode values, not Rust discriminants as wire identity. Field validation must distinguish absent/invalid values and retain location basis. Choose a ULID-like identity implementation with no actor-count cap. Run feature/license checks on selected dependencies; document third-party notices and the Elastic License 2.0 application boundary. Establish Windows checks and Linux server checks without adding macOS/Linux release scope. Do not ship application code before adding the selected license/notice files.

- [ ] **Step 4: Verify the task deliverable.**

```text
cargo test -p clacks-domain --test native_values; cargo fmt --all -- --check; cargo clippy -p clacks-domain --all-targets -- -D warnings
```

**Expected:** all named tests/checks pass; Domain fixtures pass, dependency features are recorded and no native enum interpretation change mutates saved meaning.

- [ ] **Step 5: Review and commit the bounded change.**

Check the task diff against its specification and constraints; correct failures, then commit the listed implementation/tests and any contract updates with message `feat: workspace, dependency checks and native value contracts` (use `docs:` for a documentation-only contract). Update the task checkbox and verification evidence. Do not advance a failing gate or change an accepted policy to make a test pass.

### F2: Durable storage and migrations

**Depends on:** F1, S1.

**Files:**

- Create/modify: `lib/clacks-client/src/{lib,store,migrations}.rs`
- Create/modify: `lib/clacks-client/migrations/0001_local.sql`
- Create/modify: `lib/clacks-client/tests/local_durability.rs`
- Create/modify: `lib/clacks-server/src/{lib,store,transaction}.rs`
- Create/modify: `lib/clacks-server/migrations/0001_records.sql`
- Create/modify: `lib/clacks-server/tests/storage_atomicity.rs`

**Interfaces:**

- Consumes: F1 native records and S1 accepted-state/comparison/checkpoint types.
- Produces: ClientStore::save_working(account: AccountId, change: WorkingChange) -> Result<LocalRevision, StoreError>; ClientStore::working_view(account: AccountId, filter: ContactFilter) -> Result<WorkingView, StoreError>; ServerStore transactional access to AcceptedRecord, EvidenceRecord and AccountCheckpoint. WorkingChange includes originals and causal comparison evidence; LocalRevision is local bookkeeping, not a server ordering claim.

- [ ] **Step 1: Add the named regression/acceptance tests and their fixtures.**

local_durability::restart_preserves_pending_originals; migration_failure_keeps_old_database; local_operator_not_shared. storage_atomicity::no_acceptance_ack_before_commit; account_scoping; evidence_failure_rolls_back_record. Inject failures between each durable boundary and reopen the database. Use disposable PostgreSQL and real SQLite, not only mocks.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert_eq!(reopened.pending(qso_id).original, original);
assert!(failed_transaction.accepted_record(qso_id).is_none());
assert!(other_account.read(qso_id).is_err());
```

- [ ] **Step 2: Run the focused tests and confirm the expected pre-implementation failure.**

Run each focused command listed in Step 4 after creating a compilable minimal target. Confirm failure comes from missing behavior or the explicitly missing module/type, rather than an unavailable database, provider or toolchain. For infrastructure/artifact tasks, use the named reproducible verification scenario instead of adding tests that merely mirror configuration.

- [ ] **Step 3: Implement the listed interfaces and behavior.**

Implement ordinary library transactions, schema migrations and contention retries. Persist native values, working/accepted separation, deletion state, profiles/equipment, tag definitions, independent provider snapshots/evidence and account checkpoints. Use account-scoped keys and reference constraints. Preserve native Unicode and versioned values. Tests control commit points; never acknowledge a failed commit. No external client storage service, custom writer queue or SQLite-file synchronization.

- [ ] **Step 4: Verify the task deliverable.**

```text
cargo test -p clacks-client --test local_durability; cargo test -p clacks-server --test storage_atomicity
```

**Expected:** all named tests/checks pass; Crash/reopen and migration tests prove durable originals, working state, evidence and isolation.

- [ ] **Step 5: Review and commit the bounded change.**

Check the task diff against its specification and constraints; correct failures, then commit the listed implementation/tests and any contract updates with message `feat: durable storage and migrations` (use `docs:` for a documentation-only contract). Update the task checkbox and verification evidence. Do not advance a failing gate or change an accepted policy to make a test pass.

### F3: Local contact, tag and operating-context commands

**Depends on:** F2.

**Files:**

- Create/modify: `lib/clacks-client/src/{contacts,tags,operating,profiles}.rs`
- Create/modify: `lib/clacks-client/tests/local_workflows.rs`
- Create/modify: `lib/clacks-domain/src/tag_deletion.rs`

**Interfaces:**

- Consumes: ClientStore; F1 profiles, equipment, contact facts and tag policies.
- Produces: ClientCommands::save_qso(input: QsoInput, context: OperatingContext) -> Result<QsoId, CommandError>; change_tags(ids: &[QsoId], delta: TagDelta) -> Result<ChangeReport, CommandError>; preview_tag_deletion(tag: TagId) -> Result<TagDeletionPreview, CommandError>; delete_tag(tag: TagId, choice: TagDeletionChoice, compared: TagDeletionPreview) -> Result<ChangeReport, CommandError>. TagDeletionChoice is LeaveAll/DeleteAll/DeleteUnique. Expose load_profile and explicit reload without automatically changing operator or power.

- [ ] **Step 1: Add the named regression/acceptance tests and their fixtures.**

local_workflows::overlapping_tags_share_one_qso; remove_last_tag_keeps_active; tag_delete_three_choices; profile_reload_is_explicit; equipment_delete_requires_reference_resolution. Seed 100 contacts, 30 shared with another tag, and revalidate a changed preview. Count distinct contacts.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert_eq!(delete_counts, [0, 100, 70]);
assert!(after_remove_last.tags.is_empty());
assert!(!after_remove_last.deletion.is_deleted());
assert_eq!(captured.station, context_at_save);
```

- [ ] **Step 2: Run the focused tests and confirm the expected pre-implementation failure.**

Run each focused command listed in Step 4 after creating a compilable minimal target. Confirm failure comes from missing behavior or the explicitly missing module/type, rather than an unavailable database, provider or toolchain. For infrastructure/artifact tasks, use the named reproducible verification scenario instead of adding tests that merely mirror configuration.

- [ ] **Step 3: Implement the listed interfaces and behavior.**

Save immediately and atomically with pending evidence. Snapshot station/profile/equipment values into each QSO; profile edits do not update current defaults or past contacts. Track last-loaded baseline for dirty/newer-profile indications. Keep transmit power separate and operator local across restart. Enforce tri-state defaults on offline-created tags. Deletion marks contacts deleted rather than merely untagging; retained contacts keep other tags and evidence. Require referenced-equipment reassignment/removal under D73/D183. Actual concurrent shared-state validation is S2, not a local bypass.

- [ ] **Step 4: Verify the task deliverable.**

```text
cargo test -p clacks-client --test local_workflows
```

**Expected:** all named tests/checks pass; Local logging and organization survive restart, preserve QSO identity and implement the accepted three deletion outcomes.

- [ ] **Step 5: Review and commit the bounded change.**

Check the task diff against its specification and constraints; correct failures, then commit the listed implementation/tests and any contract updates with message `feat: local contact, tag and operating-context commands` (use `docs:` for a documentation-only contract). Update the task checkbox and verification evidence. Do not advance a failing gate or change an accepted policy to make a test pass.

## Completion gate

Local logging and organization survive restart, preserve QSO identity and implement the accepted three deletion outcomes.
No later work is authorized merely because this plan is complete. See the coordinating plan for execution and release gates.

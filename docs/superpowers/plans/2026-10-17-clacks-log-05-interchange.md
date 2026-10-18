# Contact interchange Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

Amateur Data Interchange Format is **ADIF**; its text representation is **ADI** and its XML representation is **ADX**. A radio contact is a **QSO**; minimum viable product is **MVP**; Coordinated Universal Time is **UTC**; Parks on the Air is **POTA**. LoTW means Logbook of The World. FT4/MFSK are mode/submode names preserved and mapped by the domain contract.

**Goal:** Deliver safe prepared imports and consistent exports, including account-wide matching and optional POTA-compatible output.

**Architecture:** A native-independent parser and mapping layer feeds client commands. Preparation is read-only; application revalidates records and applies bounded durable changes, while exports serialize a captured local working view into staged destination files.

**Tech Stack:** Rust ADIF text format (ADI) parser/writer, domain validation, SQLite consistent read views.

**Spec:** [adif-matching.md](../../../references/adif-matching.md), [log4om-adif-import.md](../../../references/log4om-adif-import.md), [product.md](../../../docs/product.md). Read these with the [coordinating plan](2026-10-17-clacks-log.md).

## Global Constraints

This is a planned file/interface layout, not existing code or authorization to execute. All tasks also inherit the coordinating plan’s constraints and file/dependency contracts. Native records and protocol encodings are pinned by F1/S1 before downstream code; do not invent competing representations in an isolated task. No implementation checks below have run yet.

- ADI only in MVP; ADIF Extensible Markup Language (ADX) is deferred.
- Built-in mappings only; disclose unsupported fields then omit them if the user proceeds.
- ADIF is interchange, not a lossless native backup or a tag-preserving round trip.

## Review Focus

- Damaged lengths destroy reliable boundaries: I1 stops at the reliable complete-record prefix.
- An optional bad value is not a clear: I2 retains an existing valid value.
- A candidate changes after preparation: I2 revalidates before mutation.
- Cancellation/crash leaves completed records intact: I2 reports the remainder unprocessed.
- Export cancellation/failure: I3 leaves existing destination files untouched.

## File structure

The Files entries below use `lib/<crate-name>` for libraries and `bin/<application-name>` for thin executable wrappers. Behavior, configuration/runtime composition and tests belong to libraries; binaries only delegate to the library entry point. The Files entries map the new implementation units and their tests. Brace groups expand to individual files in that directory. Crate manifests/module exports and required schema migrations belong to the task that first needs them; keep related source/tests together. No production file exists at these proposed paths yet.

---

### I1: ADI preparation, mapping and candidate classification

**Depends on:** F1.

**Files:**

- Create/modify: `lib/clacks-adif/src/{lib,parser,mapping,prepare,diagnostics}.rs`
- Create/modify: `lib/clacks-domain/src/matching.rs`
- Create/modify: `lib/clacks-adif/tests/preparation.rs`
- Create/modify: `docs/engineering/adif-mappings.md`
- Create/modify: `tests/fixtures/adif/README.md`

**Interfaces:**

- Consumes: F1 mode/location/native values and stable equality; source LOG4OM field inventory.
- Produces: prepare_adi(input: &[u8], context: ImportContext) -> PreparedImport; PreparedImport { complete_records, structural_error, unsupported_fields, diagnostics }; classify_match(incoming: &MatchFacts, local: &MatchFacts) -> MatchClass::{Confident,Possible,NoMatch}. Document exact field mappings, supported lengths and precision-aware matching tables in this task.

- [ ] **Step 1: Add the named regression/acceptance tests and their fixtures.**

preparation::independent_field_sets; broken_length_stops_at_prefix; optional_invalid_is_warning; minimum_unknown_submode_valid; frequency_wins_band; coordinates_win_inconsistent_grid; possible_match_midnight_window. Include MODE=FT4, MODE=MFSK/SUBMODE=FT4, unknown submode-only, and incomplete identity at ±30 minutes.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert_eq!(prepared.complete_records.len(), reliable_prefix_count);
assert!(prepared.structural_error.is_some());
assert_eq!(incomplete_single_candidate, MatchClass::Possible);
assert_eq!(unknown_mode.source_value(), supplied_mode);
```

- [ ] **Step 2: Run the focused tests and confirm the expected pre-implementation failure.**

Run each focused command listed in Step 4 after creating a compilable minimal target. Confirm failure comes from missing behavior or the explicitly missing module/type, rather than an unavailable database, provider or toolchain. For infrastructure/artifact tasks, use the named reproducible verification scenario instead of adding tests that merely mirror configuration.

- [ ] **Step 3: Implement the listed interfaces and behavior.**

Parse the whole file before mutation, discovering fields across independent record sets. Expose structural errors and unsupported-field counts during preparation; do not guess boundaries beyond damaged lengths. Minimum new-contact facts are call, valid UTC start, band or usable frequency and preserved mode/submode information. Warn on invalid optional values and mode/grid/band corrections under selected precedence. Define deterministic transforms and confidence/possible classification using precision, not a universal automatic ±30-minute dedupe. Use sanitized synthetic fixtures; do not add a personal log export to the repository.

- [ ] **Step 4: Verify the task deliverable.**

```text
cargo test -p clacks-adif --test preparation
```

**Expected:** all named tests/checks pass; Prepared results contain every reliable complete record and upfront diagnostics without changing any local data.

- [ ] **Step 5: Review and commit the bounded change.**

Check the task diff against its specification and constraints; correct failures, then commit the listed implementation/tests and any contract updates with message `feat: adi preparation, mapping and candidate classification` (use `docs:` for a documentation-only contract). Update the task checkbox and verification evidence. Do not advance a failing gate or change an accepted policy to make a test pass.

### I2: Prepared import application and reports

**Depends on:** I1, F3, E1.

**Files:**

- Create/modify: `lib/clacks-client/src/import/{mod,review,apply,report}.rs`
- Create/modify: `lib/clacks-client/tests/import_application.rs`

**Interfaces:**

- Consumes: PreparedImport, F3 commands, E1 imported service-claim representation and comparison rules.
- Produces: apply_import(prepared: PreparedImport, choices: ImportChoices, target: TagId, cancel: &CancelToken) -> Result<ImportReport, ImportError>; ImportChoices includes Update/Duplicate/Skip and explicit ambiguous/possible candidate selection; ImportReport separately counts unchanged facts, memberships added, applied/skipped/failed/unprocessed records.

- [ ] **Step 1: Add the named regression/acceptance tests and their fixtures.**

import_application::account_wide_match_adds_tag; skip_does_not_retag; duplicate_new_id_no_occupied_external_binding; stale_candidate_recomparison; optional_invalid_preserves_existing; missing_vs_empty; crash_and_normal_reimport; cancellation_at_record_boundary. Include untagged candidates and positive/missing per-service statuses.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert_eq!(matched_qso_id, original_qso_id);
assert_eq!(skipped.tags, before.tags);
assert_eq!(report.memberships_added, 1);
assert_eq!(after_restart.completed_records, before_crash.completed_records);
```

- [ ] **Step 2: Run the focused tests and confirm the expected pre-implementation failure.**

Run each focused command listed in Step 4 after creating a compilable minimal target. Confirm failure comes from missing behavior or the explicitly missing module/type, rather than an unavailable database, provider or toolchain. For infrastructure/artifact tasks, use the named reproducible verification scenario instead of adding tests that merely mirror configuration.

- [ ] **Step 3: Implement the listed interfaces and behavior.**

Apply the selected tag to reused matches without copying identity. Revalidate preparation choices against current local working state and tag validity; preserve explicit update field choices and normal sync evidence. Atomically apply one contact plus its imported service claims. Invalid optional fields warn/omit for new contacts and preserve existing values on updates; omitted fields are not clears, explicit empty fields are optional user-selected clears. Positive imported statuses suppress uploads/confirm immediately without fabricated attempts; missing upload statuses are pending independently. Keep completed changes on cancellation/crash, stop at a safe boundary and use normal re-import for recovery. No dedicated session-resume system.

- [ ] **Step 4: Verify the task deliverable.**

```text
cargo test -p clacks-client --test import_application
```

**Expected:** all named tests/checks pass; Partial imports and reruns produce truthful reports, stable identities and no silent tag changes on Skip.

- [ ] **Step 5: Review and commit the bounded change.**

Check the task diff against its specification and constraints; correct failures, then commit the listed implementation/tests and any contract updates with message `feat: prepared import application and reports` (use `docs:` for a documentation-only contract). Update the task checkbox and verification evidence. Do not advance a failing gate or change an accepted policy to make a test pass.

### I3: Consistent ordinary and POTA-compatible export

**Depends on:** I2, F2.

**Files:**

- Create/modify: `lib/clacks-adif/src/{export,transliterate,pota}.rs`
- Create/modify: `lib/clacks-client/src/export.rs`
- Create/modify: `lib/clacks-client/tests/export_safety.rs`
- Create/modify: `lib/clacks-adif/tests/export_mapping.rs`

**Interfaces:**

- Consumes: F2 working_view snapshot and F1 effective_contact; native POTA reference scope and selected output mode.
- Produces: export_adi(snapshot: ExportSnapshot, options: ExportOptions, destination: ExportDestination, cancel: &CancelToken) -> Result<ExportReport, ExportError>; ExportOptions chooses Ordinary or PotaCompatible; ExportReport distinguishes native contacts from output entries/files and conversion warnings.

- [ ] **Step 1: Add the named regression/acceptance tests and their fixtures.**

export_safety::snapshot_stable_during_edits; unresolved_working_version_once; destination_unchanged_on_failure; cancellation_does_not_publish_partial_files. export_mapping::unicode_warning_original_preserved; grid_center_output; two_own_three_contacted_parks; tag_names_not_activity_fields.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert_eq!(ordinary_entries_for_shared_qso, 1);
assert_eq!(pota_file_count, 2);
assert_eq!(entries_per_pota_file, [3, 3]);
assert_eq!(destination_after_failure, destination_before);
```

- [ ] **Step 2: Run the focused tests and confirm the expected pre-implementation failure.**

Run each focused command listed in Step 4 after creating a compilable minimal target. Confirm failure comes from missing behavior or the explicitly missing module/type, rather than an unavailable database, provider or toolchain. For infrastructure/artifact tasks, use the named reproducible verification scenario instead of adding tests that merely mirror configuration.

- [ ] **Step 3: Implement the listed interfaces and behavior.**

Capture membership, facts and eligible provider snapshots at one local point in time, without requiring successful sync. Exclude pending deletions; do not emit competing conflict versions. Export grid/coordinates, preserving native area/point semantics even though ADIF may lose them. Transliterate human text best-effort with warnings; omit optional unrepresentable text while preserving native values. Ordinary output emits one entry per selected QSO identity; POTA output groups by actual own park/location and expands applicable contacted parks. Define multi-file staging/finalization so failure does not overwrite existing destinations; rollback/recovery of partially finalized output is part of this task’s contract, not permission for unsafe partial replacement.

- [ ] **Step 4: Verify the task deliverable.**

```text
cargo test -p clacks-client --test export_safety; cargo test -p clacks-adif --test export_mapping
```

**Expected:** all named tests/checks pass; Both export modes pass failure/cancellation tests, report conversions and leave native data unmodified.

- [ ] **Step 5: Review and commit the bounded change.**

Check the task diff against its specification and constraints; correct failures, then commit the listed implementation/tests and any contract updates with message `feat: consistent ordinary and pota-compatible export` (use `docs:` for a documentation-only contract). Update the task checkbox and verification evidence. Do not advance a failing gate or change an accepted policy to make a test pass.

## Completion gate

Both export modes pass failure/cancellation tests, report conversions and leave native data unmodified.
No later work is authorized merely because this plan is complete. See the coordinating plan for execution and release gates.

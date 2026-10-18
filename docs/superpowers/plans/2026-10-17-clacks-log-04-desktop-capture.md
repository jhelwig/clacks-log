# Desktop logging and WS Suite capture Milestone Outline

> **Status: milestone outline, not an executable implementation plan.** This document maps scope, dependencies and acceptance goals. Proposed files, interfaces and verification targets are provisional. Before coding, prepare a concrete plan for the selected milestone covering its data model, behavior, algorithms, edge cases and tests. Do not infer those choices while executing this outline.

A radio contact is a **QSO**; minimum viable product is **MVP**; user interface is **UI**; Coordinated Universal Time is **UTC**; User Datagram Protocol is **UDP**. WS Suite refers to the WSJT-X family of weak-signal applications. Amateur Data Interchange Format is **ADIF**, whose text representation is **ADI**. FT8 and CW denote an eight-tone digital mode and continuous-wave Morse operation; DX means the contacted station. Computer-aided transceiver control is **CAT**; high-DPI means high display pixel density.

**Goal:** Deliver the Windows MVP desktop for durable manual logging, tagged history, conflict review and one-client-per-source capture.

**Architecture:** egui renders views over client commands and local working state; asynchronous directory/network work never owns the UI thread. The client captures WS Suite messages and synchronizes contacts, while all service publication remains server-side.

**Tech Stack:** Rust, egui/eframe, SQLite client commands, WS Suite User Datagram Protocol (UDP) adapters.

**Spec:** [product.md](../../../docs/product.md), [ws-suite-udp.md](../../../references/ws-suite-udp.md), [gridtracker-selection.md](../../../references/gridtracker-selection.md), [desktop-toolkit.md](../../../references/desktop-toolkit.md). Read these with the [coordinating plan](2026-10-17-clacks-log.md).

## Global Constraints

This is a planned file/interface layout, not existing code or authorization to execute. All tasks also inherit the coordinating plan’s constraints and file/dependency contracts. Native records and protocol encodings are pinned by F1/S1 before downstream code; do not invent competing representations in an isolated task. No implementation checks below have run yet.

- Windows is the MVP release target; keep platform boundaries suitable for macOS/Linux later (D162).
- Both times return to live updating after successful save; a failed save does not reset the unfinished entry.
- One capturing Clacks client per WS Suite source; no tray mode or direct transceiver control in MVP.

## Review Focus

- Late lookup belongs to the original completed QSO: D3/G3 correlate by entry/QSO identity.
- Incoming empty or repeated WS call: D3 applies authority without unnecessary debounce restart.
- Type 5 plus type 12 represent one completion: D3 prevents double logging.
- Pending/conflicted/deleted contacts: D2 shows one working version and preserves review access.
- UI focus and rapid save: D1/D4 must retain entered data on validation/storage failure.

## File structure

The Files entries below use `lib/<crate-name>` for libraries and `bin/<application-name>` for thin executable wrappers. Behavior, configuration/runtime composition and tests belong to libraries; binaries only delegate to the library entry point. The Files entries map the new implementation units and their tests. Brace groups expand to individual files in that directory. Crate manifests/module exports and required schema migrations belong to the task that first needs them; keep related source/tests together. No production file exists at these proposed paths yet.

---

### D1: Desktop shell and manual entry

**Depends on:** F3.

**Files:**

- Create/modify: `lib/clacks-desktop/src/{lib,startup,app,entry,time_fields,operating_editor}.rs`
- Create/modify: `bin/clacks-log/{Cargo.toml,src/main.rs}`
- Create/modify: `lib/clacks-desktop/tests/manual_entry.rs`
- Create/modify: `docs/design/desktop-workflows.md`

**Interfaces:**

- Consumes: F3 ClientCommands and durable local defaults; F1 validation.
- Produces: clacks_desktop::main_entry() -> std::process::ExitCode; EntryController::save(commands: &ClientCommands) -> Result<QsoId, EntryError>; TimeField::{Live,Locked}; select_current_tag(tag: TagId) controls only views/default membership. Implement async event delivery separate from frame rendering.

**Acceptance scenarios to specify:**

manual_entry::independent_live_locks; save_resets_both; failed_save_keeps_entry; minimum_fields; no_timestamp_age_mode; operator_is_not_station_callsign. Use an injected clock and failing storage fixture, not wall-clock sleeps.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert!(after_success.start.is_live() && after_success.end.is_live());
assert_eq!(after_failed_save, before_failed_save);
assert_eq!(saved.station, context_at_save);
```

**Required outcomes:**

Keep desktop composition, configuration, event-loop startup and substantive error handling in `lib/clacks-desktop`; the `bin/clacks-log` executable delegates to its library entry point. Prepare entry/context/history mockups for review before implementing layout. Implement one manual workflow with current defaults and editable own/contact facts. Require call, valid UTC start, band and mode; end/reports/frequency/power/location are optional. Support FT8/phone/CW-compatible reports, comments and manually entered activity identifiers without contest-exchange machinery. Use grid/coordinate and frequency/band rules from F1. No mandatory operating sessions, persistent unfinished drafts, CAT or close-to-tray behavior.

**Candidate verification targets:**

```text
cargo test -p clacks-desktop --test manual_entry; cargo check -p clacks-log
```

**Expected:** all named tests/checks pass; A Windows user can save contacts offline, reopen saved state and operate locks/defaults without data loss.


### D2: Tagged history, Worked Before and account-wide review

**Depends on:** D1, S2.

**Files:**

- Create/modify: `lib/clacks-desktop/src/{history,worked_before,tags,pending_review,profiles}.rs`
- Create/modify: `lib/clacks-desktop/tests/contact_views.rs`

**Interfaces:**

- Consumes: F2 working_view; F3 membership/deletion/profile commands; S2 pending comparisons.
- Produces: HistoryQuery uses current tag plus selected filters; WorkedBeforeQuery uses the effective call prefix against local working contacts; PendingReviewQuery spans the account with tag/record-type filters. FieldResolution submits through S2, never directly overwrites server state.

**Acceptance scenarios to specify:**

contact_views::one_working_row_per_qso; pending_deletion_hidden_but_reviewable; untagged_accessible; shared_tag_views_same_identity; prefix_match; bulk_tag_changes; profile_baseline_indicators. Include a missing deleted tag not selectable in normal navigation.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert_eq!(rows_for_shared_qso.len(), 1);
assert!(!normal_history.contains(deleted_qso));
assert!(pending_review.contains(deleted_qso));
assert!(untagged_view.contains(active_untagged_qso));
```

**Required outcomes:**

Implement call/time/band-mode/own-call/grid-POTA/service-status filters; show pending/conflict indicators. Support individual/selected-group membership edits and the three-choice populated-tag deletion preview. Provide access to active untagged contacts without requiring a tag. Render field-level reconciliation with change-attribution distinctions and missing-reference choices. Profile reload is explicit; equipment reassignment/removal follows F3/S2. Do not infer park references from tag names.

**Candidate verification targets:**

```text
cargo test -p clacks-desktop --test contact_views
```

**Expected:** all named tests/checks pass; One shared contact behaves consistently across filtered views, including pending deletion and conflict review.


### D3: WS Suite source and completion handling

**Depends on:** D1, I1; integrate G3 lookup when available.

**Files:**

- Create/modify: `lib/clacks-client/src/ws/{mod,protocol,source,capture}.rs`
- Create/modify: `lib/clacks-client/tests/ws_capture.rs`
- Create/modify: `tests/fixtures/ws/README.md`

**Interfaces:**

- Consumes: Pinned WS Suite protocol findings, F3 save commands, I1 ADI decoding for type 12 and secondary ADI messages.
- Produces: decode_ws(datagram: &[u8]) -> Result<WsEvent, WsDecodeError>; CaptureController::handle(source: SourceId, event: WsEvent) -> Vec<CaptureEffect>. WsEvent separates Status and Completion; source association and duplicate-completion correlation are explicit, documented engineering rules.

**Acceptance scenarios to specify:**

ws_capture::status_before_completion; type5_and_type12_one_qso; repeated_completed_delivery; empty_authoritative_call; repeated_ws_value_overrides_local_edit; truncated_and_older_schema; supplied_frequency_not_offset_twice. Use synthetic protocol fixtures with provenance.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert_eq!(saved_qso_count_for_paired_completion, 1);
assert!(after_empty_call.lookup_display.is_empty());
assert_eq!(saved_frequency, supplied_frequency);
```

**Required outcomes:**

Implement source setup/reassignment and enforce one selected capturing client per source within known account state; document the limit that independently disconnected clients cannot negotiate a live lease, without inventing automatic capture failover. Preserve source-provided own-station context and completion time. Correlate binary/ADI completion variants before persistence; type 12 is not bare ADI. Incoming calls, including empty values, are authoritative. Preserve optional supplied fields; older shorter packets remain decodable when valid. Keep GridTracker Generate Messages status forwarding in the same flow. No outbound WS controls or automatic missed-contact recovery.

**Candidate verification targets:**

```text
cargo test -p clacks-client --test ws_capture
```

**Expected:** all named tests/checks pass; Recorded/replayed supported packet variants produce the intended call display and one durable completed contact.


### D4: Desktop integration and cross-platform boundaries

**Depends on:** D2, D3, S3, G3, I3, A4, E3.

**Files:**

- Create/modify: `lib/clacks-desktop/src/{service_status,import_export,lookup,sync_status,platform}.rs`
- Create/modify: `lib/clacks-desktop/tests/desktop_flows.rs`
- Create/modify: `docs/verification/windows-desktop.md`

**Interfaces:**

- Consumes: Local logging/views, capture, synchronization, directory and interchange APIs.
- Produces: Desktop commands orchestrate existing domain/client services; browser setup opens account-authorized management endpoints. PlatformServices owns browser opening, protected storage and file dialogs without OS-specific domain types.

**Acceptance scenarios to specify:**

desktop_flows::capture_during_sync_outage; lookup_during_server_outage; import_while_logging; close_stops_capture; setup_opens_web; slow_response_not_next_contact. Include keyboard navigation, display scaling and a large local log fixture.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert!(saved_contact_survives_restart);
assert_eq!(lookup_result.qso_id, original_qso_id);
assert!(closing_stops_background_capture);
```

**Required outcomes:**

Connect reviewed layouts to native workflows, cancellation/reporting and routine service/sync diagnostics. Keep network, database and geometry work from blocking frame interaction. Verify Windows keyboard/input/accessibility behavior, selection and high-DPI scaling; document measurements instead of inventing a maximum log size. Include platform abstraction compile checks without requiring macOS/Linux release packages.

**Candidate verification targets:**

```text
cargo test -p clacks-desktop --test desktop_flows; cargo test -p clacks-desktop; cargo build -p clacks-log --release
```

**Expected:** all named tests/checks pass; A packaged Windows candidate supports the complete offline-to-synchronized workflow; manual verification evidence records actual limitations.

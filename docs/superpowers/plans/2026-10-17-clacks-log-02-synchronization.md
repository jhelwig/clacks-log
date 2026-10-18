# Synchronization and recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

A radio contact is a **QSO**; the minimum viable product is **MVP**. Causal context summarizes known updates; timeline ancestry records exceptional recovery/coverage relationships. Account acknowledgment means durable local installation, not merely network receipt.

**Goal:** Implement account-wide, bidirectional catch-up and streaming with client-side record reconciliation and safe recovery.

**Architecture:** Use the researched vector-based model, sparse timeline ancestry and conservative whole-record comparison. A deterministic model precedes database/protocol implementation; accepted records and immutable service evidence reconcile without publishing unresolved client alternatives.

**Tech Stack:** Rust model and replication crate, SQLite client, PostgreSQL server, encrypted WebSockets in Axum.

**Spec:** [synchronization.md](../../../docs/synchronization.md), [recommended-model.md](../../../references/replication-research/recommended-model.md), [recovery-fit.md](../../../references/replication-research/recovery-fit.md), [synchronization-transport.md](../../../references/synchronization-transport.md). Read these with the [coordinating plan](2026-10-17-clacks-log.md).

## Global Constraints

This is a planned file/interface layout, not existing code or authorization to execute. All tasks also inherit the coordinating plan’s constraints and file/dependency contracts. Native records and protocol encodings are pinned by F1/S1 before downstream code; do not invent competing representations in an isolated task. No implementation checks below have run yet.

- One synchronization scope per account (D248); all enrolled clients receive all synchronized account data.
- Ordinary restarts create neither a fresh actor nor timeline. No per-record history graph or storage quorum is selected.
- Keep ordering evidence through the oldest authorized durable acknowledgment; no inactivity-based authorization expiry.

## Review Focus

- A restored server advances before a surviving client returns: S1/S4 must not equate reused positions.
- An authorized client returns below the transfer journal/baseline: S3/S4 must retain eligibility and pending work.
- Tag deletion races with membership or QSO edits: S2 preserves explicit missing-tag and ordinary record conflict behavior.
- Connection loss during baseline installation: S3 acknowledges only fully durable installation.
- Revoked actor data returns through recovery: S4 separates accepted history from excluded unsynchronized edits.

## File structure

The Files entries below use `lib/<crate-name>` for libraries and `bin/<application-name>` for thin executable wrappers. Behavior, configuration/runtime composition and tests belong to libraries; binaries only delegate to the library entry point. The Files entries map the new implementation units and their tests. Brace groups expand to individual files in that directory. Crate manifests/module exports and required schema migrations belong to the task that first needs them; keep related source/tests together. No production file exists at these proposed paths yet.

---

### S1: Executable reconciliation model and protocol contract

**Depends on:** F1.

**Files:**

- Create/modify: `lib/clacks-replication/src/{lib,comparison,causality,timeline,protocol}.rs`
- Create/modify: `lib/clacks-replication/tests/model_scenarios.rs`
- Create/modify: `docs/engineering/synchronization-contract.md`

**Interfaces:**

- Consumes: F1 account/record identities and stable native equality. Research describes semantics, not a ready-made protocol implementation.
- Produces: classify_record<T: Eq + Clone>(original: &T, local: &T, shared: &T) -> RecordComparison; RecordComparison::{Unchanged,LocalOnly,SharedOnly,EqualChanges,NeedsResolution}. Define ActorId, TimelinePosition, CausalContext, CoverageBoundary, AcceptedRecord, WorkingChange, ComparedState, AccountCheckpoint, SyncMessage and ProtocolVersion in this task’s contract. Specify exact wire encoding and compatibility before F2/S2/S3 depend on them.

- [ ] **Step 1: Add the named regression/acceptance tests and their fixtures.**

model_scenarios::three_way_cases; tag_and_fact_changes_conflict; old_timeline_coverage_is_not_future_coverage; restart_keeps_identity; repeated_delivery_is_idempotent. Model (T2,205) incorporating (T1,200), and prove it does not cover T1:201 or retroactively change earlier T2 coverage. Generate small event schedules with disconnect, restore and duplicate delivery.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert_eq!(classify_record(&0, &1, &2), RecordComparison::NeedsResolution);
assert_eq!(classify_record(&0, &1, &1), RecordComparison::EqualChanges);
assert!(!coverage.covers(t1_at_201));
```

- [ ] **Step 2: Run the focused tests and confirm the expected pre-implementation failure.**

Run each focused command listed in Step 4 after creating a compilable minimal target. Confirm failure comes from missing behavior or the explicitly missing module/type, rather than an unavailable database, provider or toolchain. For infrastructure/artifact tasks, use the named reproducible verification scenario instead of adding tests that merely mirror configuration.

- [ ] **Step 3: Implement the listed interfaces and behavior.**

Write a state-transition specification and executable pure model, drawing on the existing survey rather than inventing a new replication family. Define authored versus accepted state, vector lifecycle, timeline forks/coverage, comparison inputs, retry identities and evidence joins. Include two-way recovery when originals are missing and explained uncertainty when ordering is unavailable. Choose an encoding only after tests demonstrate required distinctions; record it as engineering design. Deletion is a record state. No automatic merge for different fields or tags. If a candidate representation fails an accepted property, stop its adoption, record the counterexample and refine the engineering model; do not silently weaken the property.

- [ ] **Step 4: Verify the task deliverable.**

```text
cargo test -p clacks-replication --test model_scenarios
```

**Expected:** all named tests/checks pass; The contract and executable model cover all listed counterexamples; downstream schemas/messages reference these exact types.

- [ ] **Step 5: Review and commit the bounded change.**

Check the task diff against its specification and constraints; correct failures, then commit the listed implementation/tests and any contract updates with message `feat: executable reconciliation model and protocol contract` (use `docs:` for a documentation-only contract). Update the task checkbox and verification evidence. Do not advance a failing gate or change an accepted policy to make a test pass.

### S2: Conditional server acceptance and client pending review

**Depends on:** F2, F3, S1, A1, E1.

**Files:**

- Create/modify: `lib/clacks-server/src/sync/{mod,accept,references}.rs`
- Create/modify: `lib/clacks-client/src/sync/{mod,reconcile,pending}.rs`
- Create/modify: `lib/clacks-server/tests/conditional_acceptance.rs`
- Create/modify: `lib/clacks-client/tests/reconciliation.rs`

**Interfaces:**

- Consumes: S1 ComparedState/WorkingChange; F2 stores; A1 authenticated AccountContext/ClientContext. Use fixtures implementing those contexts until A2 transport enrollment is complete.
- Produces: accept_change(ctx: &ClientContext, change: WorkingChange, compared: ComparedState) -> Result<Acceptance, AcceptError>; Acceptance::{Accepted,RefreshRequired,MissingReference,AssociationConflict}; reconcile_local(store: &ClientStore, incoming: AcceptedRecord) -> Result<ReconciliationResult, SyncError>. PendingReview is account-wide and includes hidden deletions and missing references.

- [ ] **Step 1: Add the named regression/acceptance tests and their fixtures.**

conditional_acceptance::changed_after_compare_requires_refresh; deleted_tag_stays_pending; cross_account_reference_rejected; stale_delete_preview_revalidated. reconciliation::disjoint_changes_require_resolution; pending_delete_hidden; one_sided_and_equal_skip_resolution. Include existing-QSO deletion versus edit separately from a new QSO with a deleted tag.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert!(matches!(stale_acceptance, Acceptance::RefreshRequired));
assert!(pending_original_qso_is_retained);
assert!(unrelated_change_is_accepted);
```

- [ ] **Step 2: Run the focused tests and confirm the expected pre-implementation failure.**

Run each focused command listed in Step 4 after creating a compilable minimal target. Confirm failure comes from missing behavior or the explicitly missing module/type, rather than an unavailable database, provider or toolchain. For infrastructure/artifact tasks, use the named reproducible verification scenario instead of adding tests that merely mirror configuration.

- [ ] **Step 3: Implement the listed interfaces and behavior.**

Reject stale compared state atomically without interpreting business differences on the server. Client re-runs whole-record comparison and retains originals/local/shared inputs. Use field-level choices inside the resolution UI contract. Enforce account references, tag existence and D251 association uniqueness; no accepted dangling tag references. Resolve missing tags only by explicit removal/replacement. Revalidate tag-deletion preview/effects under ordinary record rules; specification must make partial-conflict/transaction boundaries explicit before deletion is wired into the UI.

- [ ] **Step 4: Verify the task deliverable.**

```text
cargo test -p clacks-server --test conditional_acceptance; cargo test -p clacks-client --test reconciliation
```

**Expected:** all named tests/checks pass; Compare/accept races, deletion conflicts and unrelated progress are reproducible under concurrent transactions.

- [ ] **Step 5: Review and commit the bounded change.**

Check the task diff against its specification and constraints; correct failures, then commit the listed implementation/tests and any contract updates with message `feat: conditional server acceptance and client pending review` (use `docs:` for a documentation-only contract). Update the task checkbox and verification evidence. Do not advance a failing gate or change an accepted policy to make a test pass.

### S3: Durable catch-up, WebSocket streaming and compatibility

**Depends on:** S2, A2.

**Files:**

- Create/modify: `lib/clacks-server/src/sync/{transport,catchup,stream}.rs`
- Create/modify: `lib/clacks-client/src/sync/{transport,install,compatibility}.rs`
- Create/modify: `lib/clacks-server/tests/sync_transport.rs`
- Create/modify: `lib/clacks-client/tests/sync_install.rs`

**Interfaces:**

- Consumes: S1 SyncMessage/version; S2 acceptance; A2 per-client credentials and active revocation checks.
- Produces: Server sync route in the existing Axum application; SyncSession::run(ctx: ClientContext) -> Result<(), SyncError>; ClientSync::connect(endpoint: ServerEndpoint) -> Result<SyncStatus, SyncError>; install_batch(batch: SyncBatch) -> Result<AccountCheckpoint, SyncError>. Acknowledge only the returned durable checkpoint.

- [ ] **Step 1: Add the named regression/acceptance tests and their fixtures.**

sync_transport::snapshot_to_stream_has_no_gap; overlapping_batches_are_idempotent; bounded_backpressure; revoked_open_stream_stops. sync_install::crash_each_install_boundary; incompatible_upgrade_preserves_pending; server_can_request_missing_client_accepted_state. Change accepted records during catch-up and drop each message boundary.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert_eq!(after_reconnect.working(qso_id), expected_working);
assert!(durable_coverage.covers(&acknowledged_checkpoint));
assert!(upgrade_required.preserves_enrollment);
```

- [ ] **Step 2: Run the focused tests and confirm the expected pre-implementation failure.**

Run each focused command listed in Step 4 after creating a compilable minimal target. Confirm failure comes from missing behavior or the explicitly missing module/type, rather than an unavailable database, provider or toolchain. For infrastructure/artifact tasks, use the named reproducible verification scenario instead of adding tests that merely mirror configuration.

- [ ] **Step 3: Implement the listed interfaces and behavior.**

Use the same state negotiation both directions with bounded incremental batches and fuller-state fallback. Keep accepted recovery evidence distinct from local pending edits. Stream after an ordered catch-up boundary, with overlap idempotent. Persist records, originals, required causal evidence and checkpoint before acknowledgment. Negotiate incompatible versions without clearing data or enrollment. Expose only connection state, last successful synchronization and current error as required diagnostics. Do not tie catch-up to upload cooldown.

- [ ] **Step 4: Verify the task deliverable.**

```text
cargo test -p clacks-server --test sync_transport; cargo test -p clacks-client --test sync_install
```

**Expected:** all named tests/checks pass; Interrupted/repeated transfers converge without lost work, duplicate effects or premature acknowledgments.

- [ ] **Step 5: Review and commit the bounded change.**

Check the task diff against its specification and constraints; correct failures, then commit the listed implementation/tests and any contract updates with message `feat: durable catch-up, websocket streaming and compatibility` (use `docs:` for a documentation-only contract). Update the task checkbox and verification evidence. Do not advance a failing gate or change an accepted policy to make a test pass.

### S4: Compaction, actor retirement and restored-server recovery

**Depends on:** S3, E1.

**Files:**

- Create/modify: `lib/clacks-replication/src/{retention,recovery}.rs`
- Create/modify: `lib/clacks-server/src/sync/{compaction,recovery}.rs`
- Create/modify: `lib/clacks-client/src/sync/recovery.rs`
- Create/modify: `lib/clacks-server/tests/recovery_scenarios.rs`
- Create/modify: `docs/engineering/recovery-validation.md`

**Interfaces:**

- Consumes: S1 ancestry/causal contract; S3 durable checkpoints; E1 publication/observation evidence types. S4 follows E1 for full evidence recovery rather than testing contact rows alone.
- Produces: retention_boundary(participants: &[AuthorizedCheckpoint], ancestry: &TimelineCoverage) -> Result<CollectionBoundary, OrderingUncertain>; recover_state(input: RecoveryInput) -> RecoveryOutcome; Revocation closes old actor admission without deleting accepted contributions.

- [ ] **Step 1: Add the named regression/acceptance tests and their fixtures.**

recovery_scenarios::offline_client_pins_collection; revoke_advances_boundary; pending_original_survives_ack; restore_advance_then_old_client; below_compacted_baseline; new_actor_after_reauthorization; service_success_recovered_without_replay. Restore native database snapshots and simulate later revocation lost by rollback.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert_eq!(boundary_with_checkpoints_900_700_300, at_300);
assert_eq!(boundary_after_revoking_third, at_700);
assert!(recovered_success.covers_only(actual_sent_values));
```

- [ ] **Step 2: Run the focused tests and confirm the expected pre-implementation failure.**

Run each focused command listed in Step 4 after creating a compilable minimal target. Confirm failure comes from missing behavior or the explicitly missing module/type, rather than an unavailable database, provider or toolchain. For infrastructure/artifact tasks, use the named reproducible verification scenario instead of adding tests that merely mirror configuration.

- [ ] **Step 3: Implement the listed interfaces and behavior.**

Install interpretable baselines before collecting safe detail. Retain locally self-contained pending comparison evidence and surviving reference meaning. Offline unrevoked clients may pin ordering evidence indefinitely; transfer journals can still compact with fuller-state reconciliation. Reauthorization preserves data but excluded unsynchronized old-actor edits require manual reconciliation. Restore mode keeps uploads paused independently of old database flags until explicit operator resume; do not create a publication veto tied to an absent client’s unresolved conflict. Include conflicting recovered external associations as explicit resolution, never duplicate binding.

- [ ] **Step 4: Verify the task deliverable.**

```text
cargo test -p clacks-server --test recovery_scenarios; cargo test -p clacks-replication
```

**Expected:** all named tests/checks pass; Recovery report documents passed schedules and remaining limits; no claim to recover facts absent from every surviving copy.

- [ ] **Step 5: Review and commit the bounded change.**

Check the task diff against its specification and constraints; correct failures, then commit the listed implementation/tests and any contract updates with message `feat: compaction, actor retirement and restored-server recovery` (use `docs:` for a documentation-only contract). Update the task checkbox and verification evidence. Do not advance a failing gate or change an accepted policy to make a test pass.

## Completion gate

Recovery report documents passed schedules and remaining limits; no claim to recover facts absent from every surviving copy.
No later work is authorized merely because this plan is complete. See the coordinating plan for execution and release gates.

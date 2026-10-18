# Integrated validation, packaging and operations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

A radio contact is a **QSO**; Logbook of The World is **LoTW**; minimum viable product is **MVP**; OpenID Connect is **OIDC**; User Datagram Protocol is **UDP**; user interface is **UI**. These plans distinguish installed-artifact evidence from simulated provider success.

**Goal:** Produce a verified Windows desktop and Linux-container server release candidate with documented deployment, backup and recovery.

**Architecture:** Integrate completed subsystems in disposable multi-account deployments and packaged clients, then validate controlled real-service interoperability. Operational runbooks use established database/secret tools rather than built-in backup commands.

**Tech Stack:** Windows packaging, Linux containers, PostgreSQL, Rust workspace checks, browser and desktop verification.

**Spec:** [handoff.md](../../../docs/handoff.md), [architecture-review.md](../../../docs/architecture-review.md), [backup-recovery.md](../../../docs/backup-recovery.md), [observability.md](../../../docs/observability.md), [open-questions.md](../../../docs/open-questions.md). Read these with the [coordinating plan](2026-10-17-clacks-log.md).

## Global Constraints

This is a planned file/interface layout, not existing code or authorization to execute. All tasks also inherit the coordinating plan’s constraints and file/dependency contracts. Native records and protocol encodings are pinned by F1/S1 before downstream code; do not invent competing representations in an isolated task. No implementation checks below have run yet.

- Hosted subscription launch is not an MVP release gate.
- macOS/Linux desktop packages, built-in backup/restore and a separate geography updater are not MVP deliverables.
- Do not turn a documented integration risk into a claim of successful interoperability without evidence.

## Review Focus

- Multi-account hostile IDs across web/sync/jobs: R1 tests the complete deployment boundary.
- Native Windows input/secret storage and packaging differ from development: R2 validates installed artifacts.
- Restore old server while clients hold newer tags/evidence: R1/R3 drill the documented workflow.
- Missing wrapping keys: R3 fails explicitly without regenerating inaccessible credentials.
- Geography/signing bundle or licensing metadata omitted: R2 checks the actual release artifacts.

## File structure

The Files entries below use `lib/<crate-name>` for libraries and `bin/<application-name>` for thin executable wrappers. Behavior, configuration/runtime composition and tests belong to libraries; binaries only delegate to the library entry point. The Files entries map the new implementation units and their tests. Brace groups expand to individual files in that directory. Crate manifests/module exports and required schema migrations belong to the task that first needs them; keep related source/tests together. No production file exists at these proposed paths yet.

---

### R1: End-to-end failure and concurrency scenarios

**Depends on:** F3, S4, A4, D4, I3, G2, G3, E3.

**Files:**

- Create/modify: `lib/clacks-system-tests/Cargo.toml`
- Create/modify: `lib/clacks-system-tests/src/lib.rs`
- Create/modify: `lib/clacks-system-tests/tests/{offline_sync,accounts,publication,recovery}.rs`
- Create/modify: `docs/verification/mvp-scenarios.md`

**Interfaces:**

- Consumes: Completed subsystem APIs, disposable PostgreSQL/server, multiple real SQLite clients, local identity/directory/provider simulators.
- Produces: SystemHarness starts the one server application and independent clients with controllable network, clock and failure boundaries. Scenario reports identify fixture versions, injected failure and observed durable state; no private production credentials in fixtures.

- [ ] **Step 1: Add the named regression/acceptance tests and their fixtures.**

offline_sync::manual_and_ws_contacts_during_outage; all_tags_replicated; deleted_tag_pending_choice; concurrent_tag_fact_resolution; profile_reload_not_automatic. accounts::foreign_object_and_stream_denied. publication::occupied_association_race; pending_vs_accepted_payload. recovery::rollback_with_evidence_and_compaction; revoked_actor_manual_tail. Verify the package-wide Review Focus cases, not only successful CRUD.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert!(all_reachable_clients_converge_after_resolution);
assert!(offline_pending_originals_survive_crash);
assert_eq!(external_associations_for_one_key, 1);
assert!(no_secret_sentinel_in_logs_or_other_account_responses);
```

- [ ] **Step 2: Run the focused tests and confirm the expected pre-implementation failure.**

Run each focused command listed in Step 4 after creating a compilable minimal target. Confirm failure comes from missing behavior or the explicitly missing module/type, rather than an unavailable database, provider or toolchain. For infrastructure/artifact tasks, use the named reproducible verification scenario instead of adding tests that merely mirror configuration.

- [ ] **Step 3: Implement the listed interfaces and behavior.**

Add deterministic end-to-end scenarios plus generated interleavings around conditional acceptance and durable acknowledgment. Start real storage and protocol endpoints; use simulators only at external provider boundaries. Report performance measurements for representative imports, large filtered history, catch-up and geography queries without imposing an invented account/log-size cap. Review invariants with the actual implementation; do not accept test stubs as evidence of network, disk or provider behavior.

- [ ] **Step 4: Verify the task deliverable.**

```text
cargo test -p clacks-system-tests; cargo test --workspace --all-targets
```

**Expected:** all named tests/checks pass; All accepted MVP workflows have recorded end-to-end evidence and unresolved failures block release candidacy.

- [ ] **Step 5: Review and commit the bounded change.**

Check the task diff against its specification and constraints; correct failures, then commit the listed implementation/tests and any contract updates with message `feat: end-to-end failure and concurrency scenarios` (use `docs:` for a documentation-only contract). Update the task checkbox and verification evidence. Do not advance a failing gate or change an accepted policy to make a test pass.

### R2: Installed Windows client and provider interoperability

**Depends on:** R1, L2, G2.

**Files:**

- Create/modify: `packaging/windows/README.md`
- Create/modify: `packaging/windows/installer-config`
- Create/modify: `packaging/server/Dockerfile`
- Create/modify: `packaging/server/compose.yaml`
- Create/modify: `docs/verification/{windows-release,provider-interoperability,release-dependencies}.md`
- Create/modify: `.github/workflows/release-check.yml`

**Interfaces:**

- Consumes: Built desktop/server, geographic artifact manifest, selected dependency lockfile and signing test vectors.
- Produces: Reproducible build/packaging entry points produce a Windows installer/application and Linux server image. Record supported target/toolchain/library versions and actual artifact contents; packaging metadata filenames can follow the selected standard tooling without changing architecture.

- [ ] **Step 1: Add the named regression/acceptance tests and their fixtures.**

Verify installed Windows logging, protected secret storage, browser callbacks, WS UDP, keyboard/scaling and restart; verify container startup/migrations/OIDC/web/sync/jobs. Use controlled owner-approved service accounts/records for real QRZ/LoTW submission/query and compare actual acceptance/confirmation evidence. Authentication/signature verification alone is not a LoTW upload acceptance test.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert!(installed_artifact.contains_geographic_manifest_and_notices());
assert!(production_dependency_graph.contains_no_openssl_backend());
assert!(controlled_lotw_submission.has_explicit_service_success_evidence());
```

- [ ] **Step 2: Run the focused tests and confirm the expected pre-implementation failure.**

Run each focused command listed in Step 4 after creating a compilable minimal target. Confirm failure comes from missing behavior or the explicitly missing module/type, rather than an unavailable database, provider or toolchain. For infrastructure/artifact tasks, use the named reproducible verification scenario instead of adding tests that merely mirror configuration.

- [ ] **Step 3: Implement the listed interfaces and behavior.**

Select standard Windows packaging and Linux-container tooling, retain dependency/upstream/data notices and perform version/feature-specific license/advisory review. Verify pure-Rust TLS/cryptographic dependency features actually used by both targets; no backend substitution through a transitive default. Execute real provider tests only with authorized test material and record provider-normalized outcomes. Review desktop/web mockups before implementation and installed UI before release; do not add TS, tray behavior or native Windows server hosting.

- [ ] **Step 4: Verify the task deliverable.**

```text
cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace --all-targets; cargo build -p clacks-log --release; cargo build -p clacks-log-server --release
```

**Expected:** all named tests/checks pass; Installed Windows and Linux-container candidates pass their actual target checks; real-service evidence and artifact notices are recorded.

- [ ] **Step 5: Review and commit the bounded change.**

Check the task diff against its specification and constraints; correct failures, then commit the listed implementation/tests and any contract updates with message `feat: installed windows client and provider interoperability` (use `docs:` for a documentation-only contract). Update the task checkbox and verification evidence. Do not advance a failing gate or change an accepted policy to make a test pass.

### R3: Deployment, backup/restore drill and release handoff

**Depends on:** R2.

**Files:**

- Create/modify: `docs/operations/{deployment,backup,restore,upgrades,observability,account-recovery}.md`
- Create/modify: `docs/verification/restore-drill.md`
- Create/modify: `README.md`
- Create/modify: `docs/handoff.md`
- Create/modify: `docs/open-questions.md`

**Interfaces:**

- Consumes: A3 key-recovery procedures, S4 recovery model, R2 artifacts and measured system behavior.
- Produces: Operator runbooks specify database/configuration/key recovery sets, supported consistent SQLite/PostgreSQL backup paths, upgrade/rollback steps, observability wiring and administrator-assisted identity recovery. No built-in application backup/restore command is introduced.

- [ ] **Step 1: Add the named regression/acceptance tests and their fixtures.**

Execute documented steps from a fresh Linux-container deployment and Windows client: back up; create later records/tags/outcomes; restore an older server with uploads paused independently of saved flags; reconcile surviving clients; explicitly resume. Also test missing key versions, corrupted/incomplete backups, incompatible schema upgrade preservation and administrator identity recovery.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert!(restored_server.uploads_paused_before_jobs_start());
assert!(normal_sync_recovers_surviving_accepted_evidence());
assert!(missing_wrapping_key_stops_credential_use_explicitly());
assert!(upgrade_preserves_enrollment_pending_and_originals());
```

- [ ] **Step 2: Run the focused tests and confirm the expected pre-implementation failure.**

Run each focused command listed in Step 4 after creating a compilable minimal target. Confirm failure comes from missing behavior or the explicitly missing module/type, rather than an unavailable database, provider or toolchain. For infrastructure/artifact tasks, use the named reproducible verification scenario instead of adding tests that merely mirror configuration.

- [ ] **Step 3: Implement the listed interfaces and behavior.**

Document actual tested commands and secrets/configuration handling, including complete protected-key backups and consistent native client state. Explain that sync/ADIF do not recover unsynchronized data lost from every copy; copied client identity is not solved by assuming a new physical machine. Preserve D240’s explicit built-in client recovery process as post-MVP. State measured deployment limits and backup loss windows rather than invented service guarantees. Update handoff and remaining-work status from evidence, and produce release notes with known limitations. Hosted launch and lawyer-led contributor agreement stay outside MVP implementation gates.

- [ ] **Step 4: Verify the task deliverable.**

```text
Execute docs/operations runbooks in disposable environments; rerun cargo test -p clacks-system-tests --test recovery
```

**Expected:** all named tests/checks pass; A fresh operator can deploy, observe, back up, restore and upgrade the actual release candidate using the verified documentation.

- [ ] **Step 5: Review and commit the bounded change.**

Check the task diff against its specification and constraints; correct failures, then commit the listed implementation/tests and any contract updates with message `feat: deployment, backup/restore drill and release handoff` (use `docs:` for a documentation-only contract). Update the task checkbox and verification evidence. Do not advance a failing gate or change an accepted policy to make a test pass.

## Completion gate

A fresh operator can deploy, observe, back up, restore and upgrade the actual release candidate using the verified documentation.
No later work is authorized merely because this plan is complete. See the coordinating plan for execution and release gates.

# Integrated validation, packaging and operations Milestone Outline

> **Status: milestone outline, not an executable implementation plan.** This document maps scope, dependencies and acceptance goals. Proposed files, interfaces and verification targets are provisional. Before coding, prepare a concrete plan for the selected milestone covering its data model, behavior, algorithms, edge cases and tests. Do not infer those choices while executing this outline.

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

**Acceptance scenarios to specify:**

offline_sync::manual_and_ws_contacts_during_outage; all_tags_replicated; deleted_tag_pending_choice; concurrent_tag_fact_resolution; profile_reload_not_automatic. accounts::foreign_object_and_stream_denied. publication::occupied_association_race; pending_vs_accepted_payload. recovery::rollback_with_evidence_and_compaction; revoked_actor_manual_tail. Verify the package-wide Review Focus cases, not only successful CRUD.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert!(all_reachable_clients_converge_after_resolution);
assert!(offline_pending_originals_survive_crash);
assert_eq!(external_associations_for_one_key, 1);
assert!(no_secret_sentinel_in_logs_or_other_account_responses);
```

**Required outcomes:**

Add deterministic end-to-end scenarios plus generated interleavings around conditional acceptance and durable acknowledgment. Start real storage and protocol endpoints; use simulators only at external provider boundaries. Report performance measurements for representative imports, large filtered history, catch-up and geography queries without imposing an invented account/log-size cap. Review invariants with the actual implementation; do not accept test stubs as evidence of network, disk or provider behavior.

**Candidate verification targets:**

```text
cargo test -p clacks-system-tests; cargo test --workspace --all-targets
```

**Expected:** all named tests/checks pass; All accepted MVP workflows have recorded end-to-end evidence and unresolved failures block release candidacy.


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

**Acceptance scenarios to specify:**

Verify installed Windows logging, protected secret storage, browser callbacks, WS UDP, keyboard/scaling and restart; verify container startup/migrations/OIDC/web/sync/jobs. Use controlled owner-approved service accounts/records for real QRZ/LoTW submission/query and compare actual acceptance/confirmation evidence. Authentication/signature verification alone is not a LoTW upload acceptance test.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert!(installed_artifact.contains_geographic_manifest_and_notices());
assert!(production_dependency_graph.contains_no_openssl_backend());
assert!(controlled_lotw_submission.has_explicit_service_success_evidence());
```

**Required outcomes:**

Select standard Windows packaging and Linux-container tooling, retain dependency/upstream/data notices and perform version/feature-specific license/advisory review. Verify pure-Rust TLS/cryptographic dependency features actually used by both targets; no backend substitution through a transitive default. Execute real provider tests only with authorized test material and record provider-normalized outcomes. Review desktop/web mockups before implementation and installed UI before release; do not add TS, tray behavior or native Windows server hosting.

**Candidate verification targets:**

```text
cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace --all-targets; cargo build -p clacks-log --release; cargo build -p clacks-log-server --release
```

**Expected:** all named tests/checks pass; Installed Windows and Linux-container candidates pass their actual target checks; real-service evidence and artifact notices are recorded.


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

**Acceptance scenarios to specify:**

Execute documented steps from a fresh Linux-container deployment and Windows client: back up; create later records/tags/outcomes; restore an older server with uploads paused independently of saved flags; reconcile surviving clients; explicitly resume. Also test missing key versions, corrupted/incomplete backups, incompatible schema upgrade preservation and administrator identity recovery.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert!(restored_server.uploads_paused_before_jobs_start());
assert!(normal_sync_recovers_surviving_accepted_evidence());
assert!(missing_wrapping_key_stops_credential_use_explicitly());
assert!(upgrade_preserves_enrollment_pending_and_originals());
```

**Required outcomes:**

Document actual tested commands and secrets/configuration handling, including complete protected-key backups and consistent native client state. Explain that sync/ADIF do not recover unsynchronized data lost from every copy; copied client identity is not solved by assuming a new physical machine. Preserve D240’s explicit built-in client recovery process as post-MVP. State measured deployment limits and backup loss windows rather than invented service guarantees. Update handoff and remaining-work status from evidence, and produce release notes with known limitations. Hosted launch and lawyer-led contributor agreement stay outside MVP implementation gates.

**Candidate verification targets:**

```text
Execute docs/operations runbooks in disposable environments; rerun cargo test -p clacks-system-tests --test recovery
```

**Expected:** all named tests/checks pass; A fresh operator can deploy, observe, back up, restore and upgrade the actual release candidate using the verified documentation.

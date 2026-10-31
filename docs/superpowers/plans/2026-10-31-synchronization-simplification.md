# Synchronization Simplification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` for native execution when authorized, or `superpowers:subagent-driven-development` if selected by the user. Execute task-by-task. This plan does not authorize implementation, commits, pushing, or changes to GitHub issues.

**Goal:** Remove synchronization machinery without a demonstrated production purpose, including inherited code, while preserving the agreed synchronization behavior.

**Architecture:** Connection-owned receipt state records discovery obligations. SQLite and PostgreSQL transactions persist accepted state, meaningful incomplete input, interpretation and completed checkpoints. Shared dependency readiness operates on native accepted values; client reconciliation and server recovery retain their distinct responsibilities.

**Tech stack:** Rust 1.99.1, existing SQLx/PostgreSQL/SQLite adapters, Tokio, authenticated WebSockets, Serde and the selected authorization libraries. No new dependencies.

**Spec:** The user's necessity-review simplification request, implemented within [synchronization behavior](../../synchronization.md), [the native synchronization model](../../synchronization-model.md), [the integration contract](../../synchronization-integration-proposal.md), [client storage](../../client-store.md) and [service evidence](../../service-evidence.md). This plan supersedes their prescriptions for durable transfer attempts, unused account mirrors and unconsumed planners; it does not supersede their product behavior. Reconcile those prescriptions in the tasks below. The earlier [account synchronization plan](2026-10-30-account-synchronization.md) is not independent justification for retaining machinery.

## Global constraints

- Preserve one account scope, receive-first catch-up, record-level conflict detection, client-owned resolution, conditional acceptance, journal catch-up and fuller-state fallback.
- Preserve indefinite authorized offline operation, timeline ancestry, original accepted identities, deletion memory, exact service dependencies and genuinely coupled effects.
- Preserve pending originals, working values, frozen proposals, actor counters, enrollment holds and retained recovery alternatives. No new synchronization timestamp.
- A complete checkpoint certifies actual durable coverage, not the highest revision received. A page acknowledgment does not certify account coverage.
- No persistent exchange replacement, generation counter, pin manager, custom write queue, generic repository or second synchronization engine. Use normal connection ownership and the existing transactions.
- Keep SQL checked, bound and in external files. Keep backend-specific SQL and mapping. Preserve the fully relational business/evidence model and existing atomic-object encodings.
- No public APIs solely for tests, project-owned `dead_code` allowances, injected triggers/failpoints, dependency tests or replacement tests whose only claim is that removed code is absent.
- Keep native code idiomatic, named and documented; no production unchecked indexing, `unwrap`/`expect`, or `mod.rs` outside tests. Keep modules focused.
- Preserve meaningful spans, safe domain events and committed-effect metrics. Remove telemetry exclusive to removed machinery; do not add observability abstractions or log payloads, proofs or credentials.
- Production schemas have no external consumers. Edit existing development migrations directly; do not add upgrade, compatibility-marker or database-recreation workflows. Use disposable verification databases and preserve the persistent development environment.
- Preserve the Linux-only CI configuration and paused hosted-CI availability. Do not re-enable Windows tests or change transport. Report native/platform limits honestly.
- Use the existing worktree for execution. Read user-owned development guidance before server/database work; use the placement skill if another worktree becomes necessary.
- Any subsequently authorized commits need substantive messages with prose physically wrapped at 72 columns and inspected before committing. Do not leave intermediate compatibility scaffolding in the delivered branch.
- Fold branch-introduced corrections into the appropriate existing commits, including their tests, SQL metadata and documentation. Separate commits are permitted only for substantive simplifications of inherited code; task boundaries do not imply new commits.

## Review focus

Each item maps to a retained or revised scenario below, rather than a new fixture per assertion.

1. Interruption after committed pages: keep their data and pending work, keep the previous checkpoint, reconnect with a fresh receipt (Task 3).
2. Historical support beside a newer or divergent head: retain exact prerequisites without making them the current record or crediting unresolved recovery as complete coverage (Tasks 2–3).
3. Incomplete coupled input crosses connections: staging and descriptors survive, later support completes the effects atomically, unrelated work proceeds (Tasks 2–3).
4. Discovery runs while records change: retain the fixed target, valid successor substitution and gap-free subsequent catch-up (Tasks 3 and 5).
5. Removing unused implementations must not remove live comparison/witness rules, evidence interpretation, admission checks or service integrity rules (Tasks 1, 4 and 5).

## Ownership and interface changes

| Files | Resulting responsibility |
| --- | --- |
| `lib/clacks-sync/src/transfer.rs`, `transfer/{receipt,batch,discovery}.rs` | One receipt/completion contract; native dependency readiness; actual client installation planning |
| New `lib/clacks-sync/src/transfer/readiness.rs` | Shared accepted-value readiness, with real callers in both adapters; no client-record or database dependencies |
| `lib/clacks-sync/src/protocol/messages.rs` | Remove unused account/membership payloads and deferred-version list; retain ordered discovery, facts, acknowledgments and safe failures |
| `lib/clacks-client-store/src/transfer.rs` | Transaction-owning page application and final checkpoint installation using connection-owned receipt state |
| `lib/clacks-client-store/src/metadata/{receipt,transfer,account}.rs` | Remove receipt and unused account-mirror persistence; move only independently consumed staging/unit helpers to their existing semantic owners |
| `lib/clacks-client-store/src/metadata/units.rs`, `discovery/support.rs`, `metadata/client/observation.rs` | Durable coupled descriptors, exact envelopes and recovery support independent of an exchange |
| `lib/clacks-client-store/src/{retention,synchronization,read}.rs` | Keep used interpretation/witness and synchronization operations; remove test-only public alternatives and unintegrated collection |
| `lib/clacks-server/src/store/synchronization/{receipt,recovery}.rs` | Real recovery under account locking and savepoints, using shared readiness rather than synthetic client installation |
| `lib/clacks-{server,desktop}/src/synchronization/` | Connection ownership, committed-page acknowledgments, complete-coverage acknowledgments and ordinary reconnect |
| Client migration `0005_sync_state.sql`, checked SQL and both SQL metadata caches | Remove unused tables/queries; retain semantic storage and production-role constraints |
| Core `compaction.rs`, `service/{operation,download,recovery,retention}.rs` | Retain actually consumed types/rules; remove unconsumed execution and maintenance planners with their exclusive scaffolding |

Changes below are engineering simplifications, not new product decisions. Do not rename unrelated native types or redesign version/lineage encodings.

## Task 1: Remove the unused account/membership mirror and alternative installation APIs

**Files:** `clacks-sync/src/transfer/discovery.rs`, `protocol/messages.rs`, `transfer/receipt.rs`; server `store/discovery.rs`, `synchronization/connection.rs`; client `metadata/account.rs`, `metadata.rs`, `read.rs`, `synchronization.rs`, `retention.rs`, `transfer.rs`, `migrations/0005_sync_state.sql`, `sql/account/`, affected retention SQL; existing initialization/enrollment/recovery tests and crate READMEs.

**Interfaces:** Remove `AccountState`, `DiscoveryStart.account_state`, the account/membership fields of `ServerMessage::Hello`, `ReadSnapshot::account_view`, `synchronization::observe_account`, and `retention::install_interpretation`. Keep actual server account/membership reads needed by admission/recovery, not reads used only for these message fields. Keep internal `merge_lineage` and `seal_pending` and their real receipt callers.

- [ ] Inspect all direct readers of the three client mirror tables and all callers of the removed APIs. Record any newly found real consumer in the execution response before removing its required behavior; do not substitute a speculative future consumer.
- [ ] Rewrite meaningful tests currently setting interpretation through the alternative API to deliver a normal discovery page through `transfer`. Preserve assertions about pending original values and interpretation after reopening. Delete tests solely asserting mirror round trips, mirror absence or compare-and-swap behavior of `observe_account`.
- [ ] Remove the three client tables `account_metadata`, `membership_snapshot`, `client_membership`, their mapping module, query files, mirror-only collection roots and obsolete query metadata. Remove their transmitted payloads, validation and server membership reads. Keep baseline membership-generation data and authoritative PostgreSQL membership.
- [ ] Remove `install_interpretation`; normal receipt processing remains the installation path. Avoid exposing its private pieces just to repair tests.
- [ ] Run core integration tests plus affected client initialization/enrollment/recovery and server discovery/native enrollment scenarios. Regenerate/check client SQL metadata against fresh production migrations. Update the relevant contract sections and READMEs to remove the mirror and alternate API promises.

**Completion criterion:** Normal enrollment and discovery require no durable client copy of server membership; pending-evidence behavior remains exercised through real receipt installation. Deletion itself needs compilation/search checks, not a test for missing symbols.

## Task 2: Share dependency readiness without synthetic client state

**Files:** New core `transfer/readiness.rs`; `transfer.rs`, `transfer/batch.rs`; server `store/synchronization/receipt.rs`; existing `clacks-sync/tests/service_transfer.rs`, client `tests/transfers/{dependencies,service_effects}.rs`, server `store/tests/synchronization.rs` and its recovery modules.

**Interfaces:** In `transfer::readiness`, introduce `ReadinessInput<'a>` with named `account`, `candidates`, `heads`, `support` and `units` fields; accepted-value fields are `&'a [AcceptedRecord]`, units are `&'a [CoupledRequirement]`. `plan_readiness(input: &ReadinessInput<'_>, evidence: &EvidenceView<'_>) -> anyhow::Result<ReadinessPlan>` returns named `ready: Vec<AcceptedRecord>`, `staged: Vec<AcceptedRecord>` and `completed_units: BTreeSet<OperationId>` fields. `heads` supplies semantic current values; `support` supplies exact retained prerequisites without promoting them. This extracts existing rules, not a new merge or dependency policy.

- [ ] Preserve/extend the existing service-transfer scenarios: a partial coupled unit produces no partial working effect; missing exact support stages the dependent effect; a newer independent successor can proceed; historical support does not replace a head. Run focused tests before extraction; behavior-preserving extraction need not manufacture a failing test.
- [ ] Extract the existing baseline/dependency blocker and coupled-completeness calculation from `plan_batch`. Keep account/context validation and causal successor checks. Do not return client updates from readiness or treat equality of record IDs as equality of accepted versions.
- [ ] Make actual client installation call readiness and then `receive_accepted` for the ready values, preserving real pending/retained state.
- [ ] Change server receipt processing to supply actual available heads/support and staged candidates directly. Remove synthetic `WindowManifest`, zero-generation `WindowProgress`, fake `ClientRecordFields` and discarded client updates. Keep actual server `plan_recovered_install`, account lock, reference checks and savepoint rollback for coupled groups.
- [ ] Preserve `returned_coupled_effects_wait_for_support_and_keep_original_stamps`: unrelated accepted values proceed, an incomplete group remains staged, complete effects commit together with original acceptance stamps. Preserve the server recovery assertions preventing unresolved alternatives from certifying a recovered prefix.
- [ ] Run core service-transfer, affected client transfer and PostgreSQL recovery tests. Keep metrics tied to actual staged/committed outcomes, not duplicate planner calls. Document the narrower shared function and concrete adapter responsibilities.

**Completion criterion:** Both production adapters consume readiness; only the client constructs client reconciliation updates. Server grouping remains ordinary domain logic inside the existing transaction path, not a generic transaction framework.

## Task 3: Replace persisted transfer attempts with connection-owned receipt state

**Files:** Core `transfer.rs`, `transfer/{receipt,batch,discovery}.rs`, `protocol/messages.rs`; client `transfer.rs`, `metadata/{receipt,transfer,units}.rs`, `metadata.rs`, migration `0005_sync_state.sql`, `sql/{window,receipt,discovery}/`, `tests/incoming.rs` and transfer scenarios; server `synchronization/connection.rs`, `store/synchronization/receipt.rs`; desktop `synchronization/exchange.rs`; relevant tests, schemas and metadata caches.

**Native contract:** Keep `RecordRequirement` and `WindowMode`. Collapse public `WindowManifest`/`WindowProgress` layering into `ReceiptState` in `transfer::receipt`. It owns one source/mode, next expected page, accumulated exact requirements, required interpretation and coupled descriptors/completion evidence. Remove progress generations, repeated source/mode fields, full last-page retention and the separately durable sealed state. Keep only state actually used to verify this live exchange.

**Storage interfaces:** `transfer::begin(store: &ClientStore, start: &DiscoveryStart) -> anyhow::Result<ReceiptState>` establishes a fresh connection-owned receipt and preserves any pending evidence required before incoming interpretation. `install_page(store: &ClientStore, receipt: &mut ReceiptState, page: &DiscoveryPage) -> anyhow::Result<()>` commits real page effects before advancing the supplied receipt. `complete(store: &ClientStore, receipt: &ReceiptState, end: &DiscoveryEnd) -> anyhow::Result<InstalledCheckpoint>` validates source/page exhaustion and actual durable representation, and commits the checkpoint. Remove `ReceiptId`, the separate `seal` operation and optional identical-retry installation results.

**Completion evidence:** Reload actual accepted heads, exact supporting envelopes, semantic staging and interpretation inside the completion transaction. Retain receipt-local exact version references only where needed to identify those rows; do not retain entire payloads merely as proof that storage was written. Supporting facts do not become current heads. On server completion, retained unresolved alternatives cannot substitute for accepted effects. Preserve existing complete-unit validation, certified full-view absence and enrollment-hold handling.

- [ ] Rewrite `tests/transfers/windows.rs` interruption coverage to commit a page, discard the receipt, close/reopen the store and start a new receipt from its unchanged completed checkpoint. Assert saved accepted data, local pending/original values and any incomplete coupled inputs survive. Do not call completion on a pre-restart receipt ID.
- [ ] Adapt the existing `tests/incoming.rs` fixture to own the actual receipt and emit ordinary pages/end messages. Remove its manifest-first setup, stored ID and returned `InstallBatch` dependency. Assertions inspect committed Clacks state, not a batch result exposed only to tests. Keep this as a small message fixture, not a peer simulator or alternate synchronization implementation.
- [ ] Keep the existing missing-end/missing-obligation assertions. Change them to pass an actual `DiscoveryEnd` to completion: a wrong source or page count cannot certify coverage. Keep A92/B102 versus target100 and incomparable-successor assertions; remove tests of generation CAS and resuming/replacing stored windows.
- [ ] Implement the single native receipt API and adapters together so the delivered change has real callers and no compatibility wrappers. Validate a new page and its accumulated obligations, apply its data in the existing write transaction, then advance connection state only after commit. A failed operation abandons the exchange; retry is a fresh discovery, not a persisted attempt recovery.
- [ ] Remove `CheckpointUpdate`, `expected_progress_generation`, receipt generation fields, repeated plan source/mode fields, `ReceiptBatch.installation: Option<_>` and exact last-page replay handling. Return a verified checkpoint directly from native completion. Do not replace discarded guards with a new counter.
- [ ] Remove all twelve attempt tables: `transfer_window`, `window_requirement`, `window_boundary`, `window_baseline`, `window_unit`, `window_unit_member`, `window_progress`, `window_staged`, `window_completed_unit`, `receipt_state`, `receipt_last_envelope`, `window_received`. Delete their exclusive mapping/query files and cache entries. Retain `staged_accepted`, `coupled_unit`, `coupled_member`, accepted envelopes, exact support references and interpretation tables.
- [ ] Move any still-used semantic staging writes out of `metadata/transfer.rs` into their existing accepted/unit owners; delete the obsolete mapping modules. Use external checked SQL for final evidence reads. Do not create a replacement persistent receipt table, opaque receipt blob or global received-everything cache.
- [ ] Keep exact support stored independently of receipt lifetime and recoverable through the existing dependency closure when a real semantic record requires it. Preserve semantic roots rather than blanket-retaining every version by identity. With no active production collector, do not add leases or an in-flight pin manager to justify this change; Task 4 removes the unintegrated collector.
- [ ] Make desktop `inbound` own the mutable receipt. Send `PageAck` only after page installation returns, and `CoverageAck` only after completion commits. Preserve receive-first publication and normal reconnect. Server incoming discovery retains its connection-owned receipt and uses the same core sequencing/completion checks.
- [ ] Remove server `progress.received` payload accumulation and the clone-then-clear completion workaround. Return committed counts if needed for existing telemetry. Remove `Deferred.versions`; retain the payload-free `Deferred` outcome and its distinction from `CoverageAck`.
- [ ] Remove last-page identical-retry tests and full payload cloning used solely for that feature. Check only the next expected sequence within the ordered live exchange. Preserve idempotent semantic delivery across new exchanges and retries of frozen proposals.
- [ ] Run the revised core/client transfer tests and server recovery tests, then the existing authenticated two-session desktop scenario. Verify accepted-tail/retained-only recovery and partial-unit recovery before the first completed checkpoint remain intact. Regenerate/check both affected SQL metadata caches serially.
- [ ] Reconcile synchronization model section 5, integration-contract transaction/receipt sections and client-store transfer/retention sections in this same task. Remove stale `window_installed`, persisted-generation and old-receipt-resumption descriptions. Update the original #14 plan's conflicting interface descriptions to point to this replacement contract without adding review narration.

**Completion criterion:** No production persistence or test fixture resumes an exchange by stored ID. Interrupted exchanges retain useful committed domain data without advancing incomplete coverage. Discovery completeness still has explicit proof.

## Task 4: Remove unconsumed inherited planners and exclusive scaffolding

**Files:** Core `compaction.rs`, `service.rs`, `service/{operation,download,recovery,retention}.rs`; client `retention.rs`, exclusive `sql/retention/*`; core `tests/{compaction,scenarios,service_operations,service_downloads,service_recovery,service_retention,service_scenarios}.rs`; client `tests/{recovery,operating,settings_deletion}.rs`, module lists and shared fixtures; relevant READMEs/contracts.

**Retained interfaces:** `BasisWitness`, `EstablishedBasisRelation`, `seal_basis`, witness interpretation and the expanded-coverage helper actually used by discovery. Keep `CoupledRequirement` at its existing `service::operation` path, plus actually consumed evidence, comparison, dependency, combination and admission functions. Do not remove all of `compaction` or `service` by directory name.

- [ ] Recheck production callers after Tasks 1–3. Remove `CompactionInput`, `BaselinePlan`, `plan_baseline`, `common_coverage` and private projection helpers used only by that unconsumed planner. Keep their agreed retirement/retention requirements documented for the consuming maintenance workflow.
- [ ] Remove client `collect_unreferenced` and its exclusive physical-collection SQL. Do not remove queries also used by real pending preparation, transfer, basis preservation or discovery merely because they live under `sql/retention/`.
- [ ] Remove core `service::retention::{requirements, validate_collection}`, `RetentionRequirements` and their private helpers when the removed baseline planner is their last non-test caller. Keep the service support/dependency data and integrity checks used by real receipt/recovery; removing a collection planner does not remove the evidence it was supposed to protect.
- [ ] Remove `service::recovery::plan_recovered_operation` and its exclusive input/helper types. Retain the server's now-integrated recovery/readiness path and coupled descriptors.
- [ ] Remove unconsumed `plan_operation`, `plan_start`, `plan_observation`, `plan_progress`, their execution-only enums/inputs/commit plans/job updates and exclusive private helpers. Remove now-empty module declarations; do not relocate implementations or expose helpers to evade unused-code checks.
- [ ] Examine every assertion in the affected tests before deleting files. Remove scenarios solely exercising the removed planners, collection counts or hypothetical worker transactions. Preserve distinct assertions about live same-ID mutation rejection, namespace/admission guards, evidence ordering, service dependency completeness, original-basis interpretation and recovery. Place them with the existing actual owner (`service_admission`, `comparison`, `service_transfer`, `recovery`, model evidence tests or the real store scenario), without recreating fake jobs or a test-only planner.
- [ ] For pending-basis tests, supply received baseline/ancestry data through normal page installation. Keep the assertions that original values and their interpretable relationship survive checkpoint advancement and reopening; delete synthetic compaction execution from fixture setup. Missing evidence must retain conservative behavior.
- [ ] Run the affected core/client suites and workspace compilation. Search all project-owned source, docs and fixtures for removed entry points, unused allowances and newly orphaned helpers. Remove exclusive scaffolding rather than suppress warnings.
- [ ] Update crate READMEs, service-evidence/client-store integration obligations and roadmap delivery wording: document supported evidence semantics and pending consuming workflows, without claiming compaction, collection or external-service execution is currently integrated. Do not weaken the agreed future behavior or update GitHub without authorization.

**Completion criterion:** The removed execution/maintenance paths have no renamed or relocated replacement maintained only by tests. Retained shared rules have actual production consumers. Future capabilities retain their behavioral specifications, not these dormant execution frameworks.

## Task 5: Verify the integrated behavior and documentation

**Files:** Existing `clacks-desktop/src/synchronization/tests.rs` and enrollment integration owner; `clacks-server/src/store/tests/synchronization.rs`/recovery modules; the affected contracts, plan and crate READMEs. No new standalone harness or repository audit report.

- [ ] Run the existing two-client authenticated workflow. Preserve concrete assertions for receive-first behavior, offline save/reopen, convergence, repeated publication without another accepted edit, current authorization on an open connection and account isolation. Extend its interruption point only if the existing store scenario cannot establish the relevant transport wiring; do not duplicate database scenarios through the UI.
- [ ] Retain source tests showing a head changes during paging, a qualifying successor fulfills the original target, missing journal selects full discovery, and a following cycle catches later work. Retain reverse-reference tag-deletion assertions and unrelated progress. These do not depend on persisted receipt bookkeeping.
- [ ] Audit changed tests using the requirement-and-specific-Clacks-mistake rule. Record which meaningful assertion was preserved and which obsolete assertion was removed in the delivery response, not a checked-in inventory. Do not target a test count or coverage percentage.
- [ ] Check docs for persistent receipts/windows, progress generations, mirror metadata, last-page retries, unused planner examples and stale tables. Explain ordinary page acknowledgment, final coverage and reconnection separately. Preserve native-format, security, observational logging and deployment contracts.
- [ ] Prepare/check SQL metadata against fresh disposable production schemas, serially. Use the existing role-separated setup and grant scripts; do not add administrative runtime privileges, migration-history tests or legacy-account upgrade fixtures. SQLite cache stays at workspace `.sqlx/`; PostgreSQL cache stays at `lib/clacks-server/.sqlx/`.
- [ ] With the existing disposable database environment, run the verification commands below. Inspect actual assertions executed; zero matching tests and unavailable environment are not passing evidence.
- [ ] Confirm the worktree contains only authorized changes, no generated secrets/databases or test-only production helpers. Report remaining platform/hosted-CI limits and any genuinely uncovered behavior. Stop for review; do not push or rewrite PR history without authorization.

## Verification commands

Read the user-owned environment inventory before running server/database commands. Use its disposable test configuration; never print secret connection strings. `SQLX_OFFLINE=true` below uses the committed query caches, not the persistent development database.

```sh
cargo fmt --all --check
SQLX_OFFLINE=true cargo test -p clacks-sync --test integration --locked
SQLX_OFFLINE=true cargo test -p clacks-client-store --test integration --locked
SQLX_OFFLINE=true cargo test -p clacks-server --lib --locked
SQLX_OFFLINE=true cargo test -p clacks-desktop --lib --locked
SQLX_OFFLINE=true cargo test --workspace --all-targets --locked
SQLX_OFFLINE=true cargo test --workspace --doc --locked
SQLX_OFFLINE=true cargo clippy --workspace --all-targets --locked -- -D warnings
SQLX_OFFLINE=true RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --document-private-items --locked
cargo leptos build
git diff --check
```

Focused scenario filters from the tasks precede the workspace gate; do not repeat the full suite without a new change or unresolved failure. Check SQL metadata with the existing CI commands against fresh disposable schemas, not by accepting successful offline compilation alone. The real desktop enrollment/two-session acceptance owner supplies its existing protected local-provider setup; preserve its current automation boundary and report if that environment is unavailable.

## Delivery and commit boundaries

Tasks 1–4 are implementation and verification boundaries, not instructions to create a new commit per task. Task 3 deliberately includes core, adapters, schema and both transport consumers so no compatibility layer ships between them. Task 5 is integrated verification; corrections belong with their owning changes.

When execution and commits are authorized:

- Compare each change against the branch base and existing commit history to distinguish branch-introduced machinery from inherited code. A task can contain both; classify the changes rather than assigning the entire task to one category.
- Fold every branch-introduced correction into the appropriate existing commit, together with its meaningful regression coverage, SQL metadata and documentation. Rewrite affected later commits to use the corrected interfaces from the start. Do not leave fixup or cleanup commits in the delivered history.
- Create separate commits only for substantive simplifications of inherited code. Keep each such commit coherent, with its required consumer changes, tests and documentation; do not use inherited-code work as a vehicle for leaving branch-introduced corrections in later commits.
- Preserve meaningful existing commit boundaries rather than squashing the branch into one commit. Check that the rewritten commits build coherently and introduce no temporary compatibility scaffolding. Run focused verification for affected boundaries and the final integrated checks on the resulting branch.
- Do not create separate commits for review narration, verification reports or corrections to mistakes introduced by this branch. Inspect and hard-wrap each actual commit message before committing or amending.

History rewriting does not authorize pushing; pushing the rewritten branch still requires authorization.

Plan acceptance authorizes no additional product behavior. In particular, removing unconsumed compaction/service planners does not implement their consuming capabilities, change retention promises, or authorize publishing unrelated work.

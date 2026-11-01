# Synchronization retention and recovery implementation plan (#16)

> **For execution:** Read this plan and its specification together. Use native
> execution, completing and verifying each task before the next. Planning does
> not authorize implementation, pushing, or opening a pull request.

**Goal:** Deliver automatic, safe synchronization retention and demonstrate
indefinite authorized offline return and recovery from an older server database.

**Architecture:** Extend the existing native comparison, PostgreSQL account
transactions, SQLite receipt storage and desktop synchronization loop. Install
baseline conversions atomically; prune the incremental journal and reclaim
unreachable representations in separate transactions. Keep the existing protocol,
receive-first exchange, fuller-state fallback and client-side review.

**Tech stack:** Rust 1.99.1, Tokio, SQLx with checked query files and committed
offline metadata, PostgreSQL, SQLite, tracing and metrics. Standard PostgreSQL
snapshot tools are used only by the isolated automated restore scenario.

**Specification:** [Revised integration proposal](../../synchronization-retention-proposal.md).
Also read [synchronization](../../synchronization.md),
[the native model](../../synchronization-model.md),
[service-evidence retention](../../service-evidence.md#12-retention-and-collection),
[observability](../../observability.md#synchronization-retention-and-baseline-installation)
and [backup/recovery](../../backup-recovery.md).

## Global constraints

- Decisions D152–D156, D248 and D388–D390 are binding. Automatic server collection,
  a per-account journal entry-count policy and initially atomic baseline
  installation are agreed behavior, not alternatives to revisit.
- Offline absence never revokes a client. Retain genuine pending bases and
  witnesses before acknowledgment; uncertain ordering uses existing review.
- Keep accepted deletion/replay memory, distinct semantic service evidence and
  nonredundant lineage. Collection does not expire enrollment or credentials.
- Ordinary restarts retain identities. A witnessed rollback uses existing
  timeline recovery; do not add universal rollback detection or a history graph.
- Use the existing runtime role, account locks and shutdown lifecycle. No new
  repository framework, maintenance command, durable job/mark table, source
  lease, retention pin, protocol or alternate synchronization engine.
- Rust owns semantic validation and causal comparison. SQL owns account-qualified
  persistence and references. Do not duplicate enums/business rules in CHECK
  constraints or introduce serialization/encoding changes for collection.
- Use named fields, documented Rust items, ordinary module filenames and existing
  dependency facilities. No production panic shortcuts, unchecked indexing,
  dead-code allowances or test-only production interfaces.
- Tests must expose a concrete Clacks mistake. Do not test SQLx migration or
  database durability machinery, inject triggers/failpoints, add interactive
  tests, compile TrustedQSL, generate Python fixtures or chase coverage counts.
- Preserve unrelated changes. Before execution, use the user-owned worktree
  placement skill; read the user-owned development inventory before running a
  server. Use disposable databases and preserve persistent development accounts.
- Keep Windows CI disabled. Report unavailable platform checks and CI limits.
- Commit coherent production changes with substantive, physically wrapped
  72-column message bodies, checked before `git commit --file`. Do not push
  without authorization.

## Engineering defaults

These are starting implementation choices from the proposal, not new product
requirements or measured performance guarantees:

| Choice | Initial value / behavior |
| --- | --- |
| Server cadence | 300 seconds, Tokio interval with skipped missed ticks |
| Account discovery | Pages of 100, serial traversal through all pages |
| Journal target | 10,000 entries per account, deployment configurable |
| Physical cleanup | At most 1,000 owning rows per transaction |
| Busy accounts | Skip a locked account and revisit it; no worker leader |
| Authorized client without an acknowledgment | No baseline advancement beyond the existing baseline |
| No authorized clients | Bound common coverage by the server's own complete coverage |
| Local reclamation | After a completed cycle, once its source/receipt owners have finished |

Cadence/page/batch values are named internal defaults. Only the journal target
needs a new deployment setting. Existing configuration deserialization supplies
its default; do not build configuration machinery for every constant. Metric
names in the proposal are engineering defaults; their distinct workloads,
committed-effect semantics and bounded labels are requirements.

## Review focus

1. Multiple converted heads at one revision must survive discovery pagination;
   a checkpoint cannot silently pass skipped obligations. Task 1 owns this.
2. A historical body can be collectible while its journal obligation or ordering
   context remains necessary; deletion state must not depend on that body.
   Tasks 1 and 4 own this.
3. Covered effects/coupled members can use proven successors, but exact facts
   cannot. Unknown ordering or equal payloads must not become coverage. Task 2.
4. Pending originals, prepared changes and conflict drafts must survive compacted
   interpretation and an offline return below the journal floor. Task 5.
5. An older database can lack a baseline or enrollment known to a surviving
   client. Recovery must preserve immutable meanings and admission rules rather
   than silently relabel state or certify incomplete recovery. Task 6.

## File responsibility map

| Owner | Files and responsibility |
| --- | --- |
| Native journal obligations | `lib/clacks-sync/src/acceptance.rs`, `recovery.rs`: construct complete `RecordRequirement` descriptors |
| Native compaction | `lib/clacks-sync/src/compaction.rs` and new `compaction/installation.rs`: common coverage and equivalent context conversion |
| Native service readiness | `lib/clacks-sync/src/service/dependency.rs`: existing exact/covered semantics shared with actual storage callers |
| Server journal traversal | `lib/clacks-server/src/store/discovery.rs`, `store/synchronization.rs`, their SQL files: private append cursor, stored deletion state, truthful availability |
| Server persistence | New `lib/clacks-server/src/store/retention.rs` and `retention/{installation,journal,roots,reclaim}.rs`: concrete account transactions and reference closure |
| Server scheduling | New `lib/clacks-server/src/retention.rs`; existing `lib.rs`, `runtime.rs`, `config.rs`: runtime worker, lifecycle and deployment setting |
| Schema/grants | New server `migrations/0007_retention.sql`; `deploy/server/database-grants.sql`: needed descriptors/indexes and narrow deletion grants |
| Support adapters | Existing server/client `src/store/discovery/support.rs` / `src/discovery/support.rs`: select actual sufficient material without changing descriptors |
| Local retention | Existing `lib/clacks-client-store/src/retention.rs`, new child `retention/reclaim.rs` and checked `sql/retention/` queries |
| Local cleanup consumer | `lib/clacks-desktop/src/synchronization/exchange.rs`: completed-cycle invocation, no additional scheduler |
| Verification | Extend existing core, store and authenticated-session tests; isolate destructive restore in `lib/clacks-server/src/store/tests/synchronization/restore.rs` |

Split modules by these responsibilities as needed rather than extending a Rust
source beyond 600 lines. Child modules use `parent/child.rs`, not path attributes
or `mod.rs` outside tests. Regenerate query metadata in each owning crate.

## Task 1: Make journal obligations self-contained and page every installation

**Files:** Modify native `acceptance.rs` / `recovery.rs`; server
`store/synchronization.rs`, `store/discovery.rs`,
`sql/synchronization/journal_insert.sql`, `sql/discovery/journal.sql`;
create `migrations/0007_retention.sql`; extend
`lib/clacks-server/src/store/tests/synchronization/` with `retention.rs` and its
parent module declaration. Update existing journal constructors/tests.

**Interfaces:** Replace the duplicated journal key/stamp fields with the existing
native descriptor:

```rust
pub struct JournalChange {
    pub installed_at: Position,
    pub requirement: RecordRequirement,
}
```

All acceptance/recovery writers construct its deletion flag from the installed
`AcceptedRecord`. The wire protocol still transmits existing requirements;
`installation_order` stays private to the server discovery source.

- [ ] Add `retention_journal_pages_shared_installation_revision` to the existing
  real-store suite. Store more journal obligations at one installation revision
  than one page, then traverse the production discovery source. Assert every
  distinct key/deletion flag is emitted exactly once and completed coverage is
  unavailable until all pages/dependencies are received. Use existing store
  fixtures and source interfaces, not a second receipt simulator.
- [ ] Add native construction assertions to the existing acceptance/recovery
  cases: `assert_eq!(change.requirement.deleted, true);` for a deletion and
  `assert_eq!(change.requirement.deleted, false);` for a live value. Assert the
  descriptor stamp is the installed stamp, not a newly accepted version.
- [ ] Run `cargo test -p clacks-server retention_journal --locked`; expect failure
  on skipped same-revision entries before the fix. Run the affected native cases
  with `cargo test -p clacks-sync --locked`.
- [ ] Add `installation_order BIGINT GENERATED ALWAYS AS IDENTITY`, an
  `(account, installation_order)` index and `deleted BOOLEAN NOT NULL`. Keep the
  semantic primary key. Carry deletion state through all real writers/readers;
  do not add a historical-envelope lookup or payload-retention dependency.
  For rows present when the migration runs, obtain deletion state from their
  exact stamp-linked accepted payload before enforcing non-nullability; never
  supply a guessed `false` default. Initialize old append positions in a stable
  order preserving each timeline's revision order, without claiming a recovered
  cross-timeline history. Future entries use the generated append order.
- [ ] Change journal pagination to the private append-order cursor, retaining
  native lower/target coverage checks and the existing unavailable-source path.
  Read the deletion flag directly. Include both live and deleted descriptors.
- [ ] Prepare checked SQL metadata against a disposable migrated PostgreSQL
  database; rerun the focused tests and query preparation check. Expected: all
  obligations arrive across pages with unchanged native transfer messages.
- [ ] Commit: explain why installation order differs from causal order and why
  descriptors must survive independently of historical QSO bodies.

## Task 2: Preserve exact-fact versus covered-effect support semantics

**Files:** Modify `lib/clacks-sync/src/service/dependency.rs`, both existing
support adapters, and existing service transfer/recovery tests. Extend the
server retention tests and client integration cases using their real stores.

**Interfaces:** Factor the existing predicate into a production-consumed function
in `clacks_sync::service::dependency`:

```rust
pub fn represents(
    required: &SupportRequirement,
    actual: &RecordRequirement,
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<bool>;
```

`dependency::check` and both support loaders consume it. Coupled members use the
existing covered-effect requirement; do not introduce another dependency model.

- [ ] Extend the existing service-transfer scenario with
  `covered_support_uses_accepted_successor`: an actual accepted successor/deletion
  satisfies a covered requirement while the older body is absent. Assert
  `DependencyStatus::Complete`, the unchanged dependency descriptor and transfer
  of the actual successor. For an exact requirement on that older fact, assert
  incomplete until the genuine exact material is supplied.
- [ ] Add the distinct comparison assertions to the same scenario: an equal
  payload with unknown/divergent ordering does not satisfy the requirement;
  coupled-member support accepts only established coverage. Use existing
  accepted-record constructors and fixed native stamps, not invented clocks.
- [ ] Run `cargo test -p clacks-sync --test service_transfer --locked` and the
  owning server/client support cases. Expect the all-exact adapters to fail the
  successor case; establish which assertions already pass in the native engine.
- [ ] Extract the existing readiness comparison without changing semantics.
  Preserve account/key checks and genuine integrity errors. In the adapters,
  try sufficient current accepted material for covered effects, then historical
  support if needed; exact facts retain exact lookup. Unknown support remains
  incomplete/staged. Never reinstall an old effect over its successor.
- [ ] Rerun the focused native and real-store tests; verify both adapters use the
  predicate and still transmit sufficient baseline/lineage dependencies.
- [ ] Commit: describe the concrete adapter correction and the exact-fact
  behavior deliberately retained. No collection or evidence rewriting yet.

## Task 3: Install equivalent baselines through automatic server maintenance

**Files:** Native `compaction.rs` and new `compaction/installation.rs`;
server new `store/retention.rs`, `store/retention/installation.rs`,
`src/retention.rs`, corresponding checked SQL; existing metadata adapters,
`runtime.rs`, `config.rs`, `lib.rs`, deployment example and grants. Add native
`tests/compaction/installation.rs` through the existing test target, plus the
real-store retention cases from Task 1.

**Interfaces:** Native calculation receives metadata, not full QSO bodies:

```rust
pub fn plan_installation(
    baseline_id: BaselineId,
    account: &AccountMetadata,
    membership: &MembershipSnapshot,
    current: &[RecordRequirement],
    evidence: &EvidenceView<'_>,
) -> anyhow::Result<Option<BaselineInstallation>>;

pub struct BaselineInstallation {
    pub baseline: Baseline,
    pub account: AccountMetadata,
    pub conversions: Vec<ContextConversion>,
}

pub struct ContextConversion {
    pub original: RecordRequirement,
    pub projected: RecordRequirement,
}
```

The native planner owns its checked account transition; do not expose
`AccountMetadata::advance` just to bypass that boundary. The store operation
loads/plans/applies under the existing account lock, never applies a plan from
outside its transaction. Server-only installation is
`async fn install(store: &AccountStore, account: AccountId) -> anyhow::Result<()>`
in `store/retention/installation.rs`, visible only to its parent. It emits its
own result telemetry after confirmed commit. The runtime worker consumes
`pub(crate) async fn run(store: AccountStore, journal_entries: i64,
stop: CancellationToken)` from server `retention.rs`.

- [ ] Add `installation_respects_authorized_acknowledgments`: three native
  acknowledgments at different positions, one authorized client without an
  acknowledgment and explicit actor closure. Assert the exact intersected
  coverage, retained authorized components, closure eligibility and unchanged
  result for missing/incomparable interpretation. No numeric minimum across
  unrelated timelines. Cover the no-authorized-client default here too.
- [ ] Add `retention_installation_preserves_versions_and_is_idle_stable` using
  `NativeAccount` and actual acknowledgments/revocation. Assert unchanged QSO
  values, acceptance positions and service applicability, equivalent comparison,
  reused payload identity, one metadata revision and one journal descriptor per
  converted head. A repeat without new eligible work leaves baseline/head and
  committed-conversion count unchanged.
- [ ] Run the focused compaction and server retention cases. Expected: no
  installed collection behavior yet; record the failing assertions.
- [ ] Implement common coverage through real checkpoint/baseline/ancestry
  expansion. Require K to preserve the existing baseline. Remove only closed
  components whose closure is covered and whose projection is provably
  equivalent; preserve the server acceptance component. Keep uncertain and
  incomparable retained/staged contexts with their original interpretation.
- [ ] Insert a fresh immutable baseline only for actual eligible conversion or
  proved ordering reduction. Insert metadata-only envelopes reusing payload IDs;
  keep old exact representations. Repoint current heads/bindings as required,
  save account metadata and all journal descriptors atomically. Notify existing
  subscribers after commit. Do not call the payload-copying ordinary writer.
- [ ] Wire the worker into the existing runtime task ownership with the stated
  cadence/page defaults and existing cancellation facilities. Traverse every
  account page. Account-lock contention skips only that account. Shutdown stops
  admission and drains admitted work using the ordinary server policy; no
  network operation belongs inside an installation transaction.
- [ ] Add the journal target to existing deployment configuration with a typed
  default of 10,000 and positive signed-integer validation. Use named internal
  constants for 300 seconds, 100 accounts and 1,000 owning rows. No new daemon,
  queue, administrative command or per-instance leader.
- [ ] Instrument installation duration, lock wait/hold, examined contexts and
  confirmed conversions per the observability contract. Add normal-write lock
  wait measurements at the existing owning write boundaries. Use standard
  `#[instrument(skip_all, ...)]`, bounded outcomes and safe correlation; never
  account labels on metrics or custom span lifecycle wrappers.
- [ ] Extend the same store scenario with scoped telemetry assertions: committed
  conversions are reported once, idle repeats add none, and counts/diagnostics
  contain no native values or credentials. Exercise serialized same-account
  writes and another usable account; do not assert database lock timing.
- [ ] Run focused tests, SQL metadata checks and server configuration/build
  checks. Commit the native calculation with its real automatic consumer.

## Task 4: Prune the journal and reclaim only unreachable server material

**Files:** New server `store/retention/{journal,roots,reclaim}.rs`, checked
`sql/retention/` files; extend parent retention operation/worker, grants and
migration indexes; extend the existing retention test module.

**Interfaces:** Add parent-visible operations:

```rust
async fn prune(
    store: &AccountStore,
    account: AccountId,
    journal_entries: i64,
) -> anyhow::Result<bool>;

async fn reclaim(store: &AccountStore, account: AccountId) -> anyhow::Result<bool>;
```

`prune` and `reclaim` report whether another bounded pass may make progress. The actual worker
calls these after installation, checking shutdown between batches. Each operation
owns its account-locked transaction and domain metrics. Root-query helpers stay
private to this module tree; no generic repository or persisted mark set.

- [ ] Add `retention_prunes_installations_and_returns_below_floor`: use a target
  of 3 and more than one real timeline. Assert the retained count reaches 3,
  the monotonic floor contains the highest removed installation revisions and
  an older authorized client gets fuller-state discovery. Include partial
  pruning of a shared revision and an incremental source overtaken by pruning:
  assert source unavailable, never successful empty completion.
- [ ] Add `retention_reclaims_bodies_without_losing_obligations`: retain live and
  deleted journal descriptors but remove otherwise unneeded historical bodies.
  Assert discovery still emits the right flags and interpretable stamps.
  Assert current tombstones and record identities remain. Include metadata above
  K whose body is unused: reclaiming its body must not release required ordering.
- [ ] Extend Task 2's real-store scenario with actual collection. Assert the
  covered-effect old body is removed, its sufficient successor is transferred,
  a genuine exact-fact body stays, and distinct service facts stay. Do not
  manufacture a production API to inspect or delete rows; use private test SQL
  only for observing committed Clacks results against the production schema.
- [ ] Run `cargo test -p clacks-server retention_ --locked`; expect missing
  pruning/reclamation behavior. Preserve failures that identify actual lost
  meaning rather than implementation-specific row ordering.
- [ ] Implement prefix pruning by private installation order in batches at most
  1,000. Join each removed timeline's maximum installation revision into the
  existing floor and commit deletion/floor together. Count across the account,
  not separately per timeline. A temporary excess never rejects normal writes.
- [ ] Implement the complete root closure from proposal §7 in explicit checked
  queries and native comparisons: current heads/tombstones, staged/retained
  alternatives, journal descriptors, exact facts, sufficient covered effects,
  all distinct service evidence/bindings/coupled units, account metadata and
  authorized acknowledgment interpretation. Treat textual baseline IDs as real
  references. Protect ordering above K independently of journal retention.
- [ ] Recompute roots inside every cleanup transaction. Delete unreachable
  owning rows/children in schema order, then newly unreachable envelopes,
  payloads, stamps/vectors and interpretation. Bound total owning removals per
  transaction; shared payloads survive while either representation needs them.
  Do not remove enrollment rows, record identities, current tombstones or
  nonredundant ancestry. Add only missing indexes for actual query predicates.
- [ ] Add separate pruning/reclamation duration and committed-removal metrics,
  with family-owned counters rather than a generic arbitrary-row metric.
  Extend the owning scenarios with committed-count and no-op assertions.
- [ ] Exercise durable root selection after interrupted transfer and actual
  database restoration in Tasks 5–6; assert a repeated collection is a no-op.
  Do not add a committed-close durability test whose only possible failure is
  in PostgreSQL, or expose a production phase API solely for interruption tests.
- [ ] Verify restricted-runtime grants through these actual operations, regenerate
  offline metadata and rerun the retention/support suites. Commit journal/root
  reclamation as the substantive production deliverable.

## Task 5: Reclaim local copies without changing pending or review state

**Files:** Existing client `retention.rs`, new `retention/reclaim.rs`, checked
client queries and any demonstrated reference indexes in a new SQLite migration;
existing desktop `synchronization/exchange.rs`. Split the existing 624-line
`lib/clacks-client-store/tests/recovery.rs` by responsibility into child test
modules before adding recovery/retention assertions. Extend the existing
`lib/clacks-desktop/src/enrollment/tests/synchronization.rs` integration fixture.

**Interfaces:** Expose only the actual desktop-consumed entry point:

```rust
pub async fn reclaim_completed_cycle(store: &ClientStore) -> anyhow::Result<()>;
```

It lives in `clacks_client_store::retention`, uses the existing immediate write
transaction, and never advances coverage. Private cleanup helpers live in its
child module. Call it from the completed exchange path after receipt/source
ownership ends; an interrupted exchange does not call it as if complete.

- [ ] Extend the existing pending-basis case as
  `retention_preserves_pending_basis_and_reviews`: receive a projected baseline,
  preserve original/working/prepared values and a real review draft, complete
  the receipt, reclaim and reopen. Assert the original context/witness and review
  input are unchanged, equivalent accepted projection creates no conflict, and
  unrelated unreachable copies disappear. Insufficient sealing keeps old
  interpretation; it does not fabricate a witness or relabel an original.
- [ ] Assert current actor counter high-water marks and authorization-review
  holds survive. Include original/prepared/witness conditions, service support
  and current operating/saved-source references in the real rooted scenario.
  Do not add a second round-trip test for every table.
- [ ] Run `cargo test -p clacks-client-store --test integration retention_ --locked`.
  Expected: cleanup entry point/behavior absent, while existing preservation
  behavior remains passing.
- [ ] Implement local root closure from proposal §8 and bounded removal of
  unreachable relational copies. Reuse witness sealing and existing role
  writers. Do not delete actual comparison evidence when its self-contained
  witness is unavailable or insufficient. No local journal, scheduler, new
  generation counter or maintenance-state table.
- [ ] Wire the actual completed desktop cycle to reclamation. Reload the durable
  completed checkpoint for acknowledgment as before. Cleanup failure cannot
  invent completion or roll back an already committed receipt; report the
  owning error and use the existing reconnect path with safe retained data.
- [ ] Extend the existing two-session WebSocket scenario: authorize an offline
  client, retain its pending edit, compact/prune on the server, reconnect and
  complete full catch-up before publication. Assert unchanged pending basis,
  expected conflict/review outcome and unrelated accepted progress. Reuse #15
  review actions rather than duplicate its UI component matrix.
- [ ] Prepare/check SQLite query metadata; run the focused client/desktop cases
  and native comparison suites. Commit local reclamation with its actual caller.

## Task 6: Demonstrate real rollback recovery and finish verification

**Files:** New server `store/tests/synchronization/restore.rs` and parent test
module declaration; only needed private fixture additions under existing test
support. Reconcile `docs/synchronization.md`, `synchronization-model.md`,
`synchronization-retention-proposal.md`, `observability.md`, `backup-recovery.md`
and affected crate READMEs with delivered behavior. Keep planning/defaults
separate from binding product decisions; do not add a checked-in audit report.

**Interfaces:** Use existing production migration, admission, discovery, receipt
and review APIs. Private test support drives standard `pg_dump`/`pg_restore`
commands from Rust. No new production recovery endpoint or backup command.

- [ ] Add `retention_recovers_from_older_database` as an automated, noninteractive
  isolated restore scenario. It requires an exclusively owned disposable
  PostgreSQL instance and ordinary snapshot tools, so mark it ignored with that
  explicit reason in the normal parallel suite. This is not a manual harness.
  Use the existing migrator/runtime test connection variables; do not give
  ordinary application tests additional privileges or mutate shared databases.
- [ ] Snapshot real migrated state before baseline installation, then retain
  newer actual client stores while installing/collecting more server state.
  Stop/close the fixture's server and pools, restore the older database using
  ordinary PostgreSQL tools, reopen and synchronize. Assert immutable old/new
  baseline meanings, recovered accepted state, unchanged native payloads,
  correct timeline fork when witnessed and no false complete prefix.
- [ ] In the same fixture, take the later enrollment/revocation boundary needed
  to exercise both cases: an actor absent from the restored authorization state
  must reauthorize with a new identity; an authorized surviving client can return
  accepted old contributions without admitting revoked unsynchronized edits.
  Reapply known revocations as documented. Assert uncertainty/incomplete support
  remains staged or requires existing review while unrelated work progresses.
- [ ] Run the focused scenario against an explicitly disposable instance, alone:
  `cargo test -p clacks-server retention_recovers_from_older_database --locked -- --ignored --test-threads=1`.
  Expected: actual restore and production recovery succeed. Missing tools or
  database access are verification limits, not a passing recovery claim. Do not
  add Docker startup or repeated database creation to every workspace test.
- [ ] Keep external uploads and protected-secret recovery outside this fixture:
  document existing upload pause/explicit resume and required secret backups;
  do not implement future publication workflows or pretend QSO sync restores
  private credentials.
- [ ] Run final checks from the workspace with `SQLX_OFFLINE=true` and
  `LEPTOS_OUTPUT_NAME=clacks-web`: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked` and
  `cargo doc --workspace --no-deps --locked`. Run the existing web build and
  both stores' SQLx preparation checks using their documented commands against
  disposable migrated stores. Do not create an unnecessary metadata database.
- [ ] Inspect every new/changed test for its concrete Clacks mistake, duplicated
  assertions and runtime cost. Verify telemetry claims in owning scenarios,
  not another full telemetry test matrix. Inspect changed code for dormant
  APIs, public test helpers, panic shortcuts, inappropriate constraints and
  hidden custom lifecycle machinery.
- [ ] Reconcile documentation: distinguish what runs automatically, configured
  entry target versus ordering boundary, preserved service facts, interruption
  behavior and available measurements. Remove proposal-only claims once
  delivered. No legacy upgrade, compatibility/reset chapter or timestamp that
  claims to summarize synchronization completion.
- [ ] Commit the real recovery verification and maintained documentation. Report
  commands/results, restore evidence, observed installation/lock measurements
  with workload, platform limits and remaining acceptance. Do not invent a
  latency promise, require Windows CI, push or open a pull request implicitly.

## Execution and commit boundaries

Tasks 1–2 correct concrete existing consumers; Tasks 3–5 install production
maintenance; Task 6 proves recovery through those delivered paths. Each planned
commit must build and have real consumers. Adjust a commit boundary if required
for compilation rather than landing unused public APIs between tasks.

Fold corrections introduced by this branch into their owning commits. A separate
commit is appropriate for a substantive inherited simplification; do not append
cleanup commits for mistakes introduced by earlier tasks. Preserve unrelated
working-tree changes and inspect the actual commit-message file and physical
line lengths before each commit/amend.

Execution starts only after plan review. The standing execution preference is
native execution; no implementation, push or CI claim is implied by this plan.

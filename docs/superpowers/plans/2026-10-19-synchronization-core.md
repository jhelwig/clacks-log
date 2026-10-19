# Synchronization Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for native execution, or superpowers:subagent-driven-development if the user selects that method. Implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Read both this plan and its specification before execution.

**Goal:** Deliver the production synchronization metadata and transition logic for issue #2, ready for later storage and transport integration.

**Architecture:** Add `lib/clacks-sync`, consuming native records from `clacks-model`. Its production operations compare observed state and produce explicit state updates and acceptance/installation preconditions. Real database adapters later commit those updates atomically; the library does not implement a substitute database, simulated network or second synchronization engine.

**Tech Stack:** Rust 1.99.1; standard collections; existing workspace Serde, thiserror and anyhow dependencies; existing typed IDs and native records. Serde is Rust's serialization framework.

**Spec:** [Synchronization metadata and transitions](../specs/2026-10-18-synchronization-metadata-proposal.md), including its worked examples and hub-and-spoke corrections. The user's request to prepare this plan advances the written design to planning; implementation still requires plan review and execution authorization.

## Global constraints

- Rust **1.99.1**, edition **2024**, workspace resolver **3**, Elastic License **2.0**; inherit package metadata/dependencies using `workspace = true`.
- Libraries live in `lib/<crate-name>`, applications in `bin/<application-name>`. This issue adds a production library, not an application.
- One account synchronization scope; client↔server traffic only; all enrolled clients replicate the account. No client↔client connection.
- Whole-record comparison includes tag sets. Equal/one-sided changes bypass business resolution; differing two-sided changes require client-side field choices. No automatic field/tag merge.
- Preserve original, working and prepared values and actual comparison context. Local operating defaults stay outside synchronized records.
- Dynamic record version vectors include client and server components. Account coverage is a separate type. Ordinary restart preserves identities/timeline.
- Coalesce local edits; allocate checked **unsigned 64-bit** counters when preparing a version. Never wrap or reuse counters.
- No server-side unresolved-business-conflict queue, per-edit history graph, storage quorum, per-start identity renewal or custom database write queue. Standalone undelete and equipment restore/clone remain outside this issue; delete/edit conflicts still receive the agreed review.
- Tests exercise production transitions using native records. No disposable runtime, fake database or alternate model implementation.
- No schema for future service evidence is invented here; issue #3 extends production record/evidence handling, and #6/#14/#16 integrate this same library.
- No production `expect()`/`unwrap()`, direct collection indexing, public test helpers, glob imports or facade `pub use` re-exports. No `mod.rs` outside tests.
- Document public/private constants, fields, variants, types and functions. Keep each contract on its owning item. Separate Rust items with blank lines.
- Leaf errors derive `thiserror::Error`; aggregate operations return `anyhow::Result` preserving concrete causes. Expected review/retry outcomes are result data, not errors.
- Use ordinary Serde representations. Leaf containers validate on deserialization; composite envelopes and native values validate at transition/acceptance boundaries, following the existing model. Encode structured enum map keys as entry sequences for JSON compatibility.
- Commit subjects have substantive bodies explaining motivation/constraints and actual verification, not just restating a diff.
- Product/application code remains absent until execution is authorized. Planning does not close issue #2.

## Review focus

Each risk below has an owning test task.

1. Old or cross-account metadata must not impersonate a current record/checkpoint: Tasks 1–3 and 9.
2. A response lost before further local editing must not lose that edit or prompt about the client's own incorporated version: Tasks 4–5.
3. Receiving one high-numbered record, a stale page or only part of a manifest must not certify account coverage: Task 6.
4. Membership/reference changes during acceptance or baseline installation must invalidate the plan instead of racing past validation: Tasks 5 and 7.
5. Collected context, an older baseline or a recovered branch must produce only justified ordering, preserving pending/deletion data: Tasks 7–9.

## Delivery and transaction boundary

The library returns production row values and the state those values were computed
against. It cannot prove a database commit occurred. Store adapters must load the
compared rows, check preconditions and save the returned changes in one actual
transaction. An acknowledgment is built from a checkpoint **reloaded from the
committed store**, not from a proposed installation result. Do not add a public
`mark_durable()` flag/token that merely asserts persistence.

Unit tests can serialize/reload valid pre-commit and post-commit row sets to check
the production metadata. That demonstrates restart representation, not SQLite or
PostgreSQL crash atomicity. Actual transaction/interruption verification belongs
to the integrating issues. No fake storage engine is needed to test these rules.

## File structure and dependency order

All `src/` and `tests/` paths in tasks are relative to `lib/clacks-sync/`.

```text
Cargo.toml / Cargo.lock                  add workspace member/path dependency
lib/clacks-model/src/ids.rs              shared account/actor/timeline/baseline IDs
lib/clacks-sync/Cargo.toml               inherited metadata/dependencies
lib/clacks-sync/README.md                product entry points and transaction contract
lib/clacks-sync/src/lib.rs               owning public modules and crate lint policy
src/error.rs                            structured metadata errors
src/record.rs                           native payloads, keys, envelopes and observations
src/clock.rs                            scoped vectors and checked counter operations
src/lineage.rs                          position-dependent ancestry and prefix coverage
src/account.rs                          account head, baseline and installed checkpoint data
src/compare.rs                          causal comparison and value-change classification
src/client.rs                           pending/prepared state and original-value preservation
src/acceptance.rs                       admission/reference conditions and commit results
src/transfer.rs                         manifests, batch progress and checkpoint updates
src/membership.rs                       revocation/reauthorization metadata transitions
src/compaction.rs                       common boundary, witnesses and baseline projection
src/recovery.rs                         contradiction detection and restored-history installation
lib/clacks-sync/tests/                   matching integration tests plus scenarios.rs
lib/clacks-sync/tests/support/mod.rs     private native fixtures and deterministic parsed IDs
```

Task order: **1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9**. Tasks own their
verification and commits. Do not create empty modules or placeholder APIs for
later tasks. Add each module declaration only when its behavior is implemented.

## Interface vocabulary

These shapes are decisions for this plan; each is implemented by its owning task.
Opaque scalar/container types (`RecordVector`, `Coverage`, `Lineage`) keep their
storage private. Composite row types expose the listed fields, following the
existing native model; transition entry points validate their invariants before
use. Derive equality and ordinary Serde support for durable rows where
appropriate. Borrow read inputs; own returned row updates.

- Task 1 adds `AccountId`, `ClientActorId`, `TimelineId`, `BaselineId` to the existing model ID macro. Do not add integer/test-only constructors.
- `RecordId` variants wrap existing `QsoId`, `TagId`, `ProfileId`, `EquipmentConfigurationId`, `GroupId`, `OutputProfileId`; `RecordKey { account, id }`.
- `RecordValue` variants are `Qso(Box<Qso>)`, `Tag(Box<LogbookTag>)`, `Profile(Box<SavedLocationProfile>)`, `Equipment(Box<EquipmentConfiguration>)`, `Group(Box<GroupDefinition>)`, `OutputProfile(Box<OutputProfile>)`. Boxing bounds enum size without changing native serialization. No replacement native schema or arbitrary JSON payload.
- `RecordState::{Live(RecordValue), Deleted}`. Never-created/unavailable are observations, not saved live states.
- `ClockActor::{Client(ClientActorId), Server(TimelineId)}` and `RecordVector`: validated actor/counter entries. The containing key/baseline supplies its comparison domain.
- `Position { timeline, revision: u64 }`; revision zero is valid for an origin/checkpoint, not a newly accepted record. Do not derive scalar `Ord`/`PartialOrd` for positions; causal comparison goes through lineage. Keys/actor IDs may have deterministic collection ordering without causal meaning.
- `VersionStamp { accepted_at: Position, baseline: BaselineId, vector: RecordVector }`; `AcceptedRecord { key, state, stamp }`.
- `Coverage`: timeline/prefix map; `TimelineBoundary { at, includes: Coverage, kind: BoundaryKind }`, with `BoundaryKind::{ForkOrigin, RecoveredCoverage}`.
- `Baseline { id, account, coverage, lineage: Lineage, membership_generation: u64 }`; `InstalledCheckpoint { account, baseline, coverage }`.
- `AccountMetadata { account, head: Position, baseline: BaselineId, installed: Coverage, membership_generation: u64 }`.
- `EvidenceView<'a> { lineage: &'a Lineage, baselines: &'a [Baseline] }`: a read view of actual metadata, not a store/transport abstraction.
- `Observation::{Accepted(AcceptedRecord), Absent { key, checkpoint: InstalledCheckpoint }, Unavailable { key, reason: UnknownReason }}`.
- `UnknownReason::{MissingBaseline, MissingLineage, MissingOriginal, CollectedContext}` is ordinary diagnostic data in `error.rs`, available from Task 1. Callers retain the key/position inputs; do not duplicate them in an unused wrapper.
- `MetadataError` is the leaf error enum. Include structured account/key mismatch, invalid counter/overflow, inconsistent identity/state, missing/conflicting metadata, invalid lineage, duplicate/conflicting requirement and incomplete-coverage diagnostics. Do not wrap native errors in forwarding variants.

## Task 1: Scoped identities, native envelopes and explicit vectors

**Files:** Modify root `Cargo.toml`, `Cargo.lock`, `lib/clacks-model/src/ids.rs`; create the new crate manifest, `src/lib.rs`, `src/error.rs`, `src/record.rs`, `src/clock.rs`, and `tests/metadata.rs` with private `tests/support/mod.rs`.

**Consumes:** Existing model ID macro, native record types, `Qso::validate`, `StationSnapshot::validate`, `EquipmentValues::validate`, `compile_definition`.

**Produces:** Vocabulary above through `Position`/`VersionStamp`/`AcceptedRecord`, plus:

```rust
RecordVector::from_entries(entries: impl IntoIterator<Item = (ClockActor, u64)>)
    -> Result<RecordVector, MetadataError>
RecordVector::component(&self, actor: ClockActor) -> u64
RecordVector::compare_components(&self, other: &Self) -> VectorRelation
RecordVector::join(&self, other: &Self) -> RecordVector
RecordVector::increment(&self, actor: ClockActor, high_water: u64)
    -> Result<(RecordVector, u64), MetadataError>
AcceptedRecord::new(key: RecordKey, state: RecordState, stamp: VersionStamp)
    -> anyhow::Result<AcceptedRecord>
```

`VectorRelation::{Equal, Before, After, Concurrent}` describes raw compatible
components only; it is not the baseline-aware product decision. Zero entries are
canonicalized away; conflicting duplicate entries error. Encode structured
`ClockActor` entries as a sorted sequence through a private Serde representation.
Reserve the next counter above both supplied high-water and the vector's own
component using checked addition. Envelope construction checks embedded native
identity and existing save invariants; semantic lineage checks arrive in Task 3.
For profiles validate station/callsign and supplied equipment values through the
existing model; do not call profile loading or rewrite defaults. Validate group
definitions as a whole using the existing compiler. Do not invent new tag-name,
output-profile-name or catalog-recognition rules.

- [ ] **Step 1:** Add the new package manifest/minimal documented crate entry and workspace wiring needed to run this task's tests. Inherit existing dependency versions and add dev dependencies serde_json/ulid. Run `cargo +1.99.1 check --workspace --offline` to add the path package to the lockfile without upgrading existing dependencies. Do not create empty future modules.
- [ ] **Step 2:** Add failing `metadata.rs` tests using actual native fixtures. Pin scoped-key mismatch, round trips, actor-role distinction, `u64::MAX` overflow and raw vector algebra:

```rust
assert_eq!(a_3_b_2.compare_components(&a_2_b_3), VectorRelation::Concurrent);
assert_eq!(a_3_b_2.join(&a_2_b_3), a_3_b_3);
assert!(at_max.increment(client_a, u64::MAX).is_err());
assert_ne!(RecordKey { account: account_a, id }, RecordKey { account: account_b, id });
```

Fixture vectors contain the literal components in their names. Include a deleted
envelope and an invalid QSO whose end precedes start. JSON serialization must
round-trip client/server enum keys without a “key must be a string” failure.

- [ ] **Step 3:** Run `cargo +1.99.1 test -p clacks-sync --test metadata --locked --offline`; expect unresolved metadata operations or failing assertions, not a missing package/toolchain.
- [ ] **Step 4:** Implement the exact types, validation and methods above, with the existing crate's documentation/indexing/panic lint policy. The workspace path dependency is `clacks-model = { path = "lib/clacks-model" }`; the new member inherits it and serde/thiserror/anyhow.
- [ ] **Step 5:** Run `cargo +1.99.1 test -p clacks-sync --test metadata --locked --offline`; all named cases must pass, including concrete native errors remaining downcastable.
- [ ] **Step 6:** Commit the independently usable metadata/vector operations. Explain record versus account counter domains and inherited native validation in the body.

## Task 2: Account lineage, complete-prefix coverage and initialization

**Files:** Create `src/lineage.rs`, `src/account.rs`, `tests/lineage.rs`; extend owning declarations and metadata tests only as needed.

**Consumes:** Task 1 IDs, `Position`, `MetadataError`.

**Produces:** `Coverage`, `TimelineBoundary`, `BoundaryKind`, `Lineage`, `Baseline`, `InstalledCheckpoint`, `AccountMetadata`, `EvidenceView`, plus:

```rust
Coverage::from_entries(entries: impl IntoIterator<Item = (TimelineId, u64)>)
    -> Result<Coverage, MetadataError>
Lineage::new(account: AccountId, initial: TimelineId) -> Lineage
Lineage::add_boundary(&mut self, boundary: TimelineBoundary)
    -> Result<(), MetadataError>
Lineage::coverage_at(&self, position: Position) -> Result<Coverage, MetadataError>
Lineage::compare(&self, left: Position, right: Position) -> PositionRelation
AccountMetadata::initialize(account: AccountId, timeline: TimelineId, baseline: BaselineId)
    -> (AccountMetadata, Baseline)
```

`PositionRelation::{Equal, Before, After, Divergent, Unknown(UnknownReason)}`.
A root initializes at revision zero. Coverage expansion joins only boundaries
whose `at` position is not later than the queried position, recursively expanding
included prefixes. Use an iterative worklist; reject position-dependent cycles
and conflicting rewrites of an installed boundary. Do not reject valid history
solely because two timeline IDs occur in opposite directions at different cuts.
Repeated identical boundaries are idempotent. Missing interpretation yields
unknown, not proof of concurrency. No record API implicitly modifies coverage.

- [ ] **Step 1:** Add failing tests with root T1, fork `(T2:0 includes T1:160)`, then `(T2:41 includes T1:200)`:

```rust
assert!(!lineage.coverage_at(pos(t2, 40))?.covers(pos(t1, 199)));
assert!(lineage.coverage_at(pos(t2, 41))?.covers(pos(t1, 199)));
assert!(!lineage.coverage_at(pos(t2, 41))?.covers(pos(t1, 201)));
assert_eq!(unknown.compare(pos(t1, 199), pos(t2, 41)), PositionRelation::Unknown(UnknownReason::MissingLineage));
```

Also test conflicting duplicate boundaries, a true position cycle, deep valid
ancestry without recursive stack overflow, cross-account evidence and revision
zero initialization. Provide `Coverage::covers(Position) -> bool` in this task.

- [ ] **Step 2:** Run the `lineage` integration test; confirm failures identify missing behavior.
- [ ] **Step 3:** Implement coverage expansion and immutable-boundary validation. Account initialization returns the actual root baseline; seeding native definitions later uses ordinary acceptance, not this constructor.
- [ ] **Step 4:** Run `cargo +1.99.1 test -p clacks-sync --test lineage --locked --offline`; verify the T2:40/T2:41 distinction survives serialization/reload.
- [ ] **Step 5:** Commit with the non-retroactive coverage invariant and known missing-lineage outcome in the body.

## Task 3: Causal comparison and the settled value policy

**Files:** Create `src/compare.rs`, `tests/comparison.rs`; extend `record.rs` with `Observation` and declarations above.

**Consumes:** Accepted envelopes, explicit vectors, positions, baselines and `EvidenceView`.

**Produces:**

```rust
classify_changes<T: Eq>(original: &T, local: &T, shared: &T) -> ChangePattern
compare_records(left: &AcceptedRecord, right: &AcceptedRecord, evidence: &EvidenceView<'_>)
    -> anyhow::Result<RecordRelation>
```

`ChangePattern::{Unchanged, LocalOnly, SharedOnly, EqualResults, BothDifferent}`.
`RecordRelation::{SameVersion, Before, After, Divergent, Unknown(UnknownReason)}`.
Invalid contradictory metadata is a concrete error, not `Divergent`.

Before raw vector comparisons interpret both baselines. Combine positive record
and timeline evidence: a record vector can establish a predecessor omitted from
global prefix coverage. Missing a global edge is not opposite evidence. Compare
complete record contexts before claiming divergence. Same version/context with
different native states is invalid. Native equality still bypasses business
resolution when order is unknown; it does not fabricate lineage/coverage.

- [ ] **Step 1:** Add the complete original/local/shared truth table, including deletion, two independent tag edits, tag-versus-comment edits and equal results. Assert:

```rust
assert_eq!(classify_changes(&a, &b, &a), ChangePattern::LocalOnly);
assert_eq!(classify_changes(&a, &b, &b), ChangePattern::EqualResults);
assert_eq!(classify_changes(&a, &local_tag_edit, &shared_comment_edit), ChangePattern::BothDifferent);
```

Add record-specific T1:180 context under a T2 head whose global old-prefix coverage
is only 160; it must establish record succession without claiming prefix 180.
Also test absent baseline versus complete incomparable vectors, two different
baselines presented under one baseline ID, and a conflicting payload under the
same version identity. Conflicting baseline definitions error rather than
selecting the first match.

- [ ] **Step 2:** Run `comparison`; verify the expected classifications fail before implementation.
- [ ] **Step 3:** Implement these operations using native equality and metadata interpretation. Preserve input values for future field resolution; do not create a reflective field/patch engine. Call the same generic classifier on the whole `RecordState` to trigger review, then on individual field values for later field attribution. Field-level calls never decide whether to bypass whole-record review.
- [ ] **Step 4:** Run `cargo +1.99.1 test -p clacks-sync --test comparison --locked --offline`; all truth-table and evidence-conflict cases pass.
- [ ] **Step 5:** Commit; explain the distinction between native equality, causal order and coverage.

## Task 4: Durable pending/prepared state and own-version recognition

**Files:** Create `src/client.rs`, `tests/client_state.rs`.

**Consumes:** Task 3 observation/comparison and Task 1 checked vectors/native validation.

**Produces:**

- `OriginalBasis::{Accepted(AcceptedRecord), NeverCreated { key, checkpoint }, SubsumedOwn { key, state, vector, witness: VersionStamp }, Unavailable { key, reason }}`.
- `PendingRecord { key, original: OriginalBasis, working: RecordState, prepared: Option<PreparedChange> }`.
- `ClientRecord { key, latest: Observation, pending: Option<PendingRecord> }`; `LocalCounter { actor: ClientActorId, high_water: u64 }`.
- `VersionCondition::{Absent { key, checkpoint }, Is { key, stamp: VersionStamp }}` and `PreparedChange { key, state, vector, actor, expected: VersionCondition, dependencies: Vec<VersionCondition> }`.
- `ReviewReason::{BothDifferent, MissingOriginal, OrderingUnknown(UnknownReason), MissingReference(RecordKey), ExcludedActor(ClientActorId)}`; `ReviewInput { original: OriginalBasis, local: RecordState, shared: Observation, reason: ReviewReason }`.

```rust
receive_accepted(current: &ClientRecord, incoming: &AcceptedRecord, evidence: &EvidenceView<'_>)
    -> anyhow::Result<ClientUpdate>
prepare_change(current: &ClientRecord, actor: ClientActorId, counter: &LocalCounter,
    dependencies: &[VersionCondition], evidence: &EvidenceView<'_>)
    -> anyhow::Result<Preparation>
resolve_review(review: &ReviewInput, chosen: RecordState, evidence: &EvidenceView<'_>)
    -> anyhow::Result<ClientRecord>
```

`ClientUpdate { record: ClientRecord, review: Option<ReviewInput> }` owns the rows
the real client must save together. `Preparation` has `Ready { record, counter,
change }`, `Retry { record, change }`, `Review { record, input }`, `Unchanged {
record }` variants. `resolve_review` retains the compared shared version as the
next precondition and validates chosen native state; it does not publish it.

- [ ] **Step 1:** Write failing `lost_response_keeps_later_work`: A at S1:200; freeze B at `{S1:200,B:1}`; change working to C; receive B at `{S1:201,B:1}`. Assert working remains C, original becomes witnessed B, and no review is generated. Receiving later D must compare B/C/D. Round-trip all durable rows between steps. Also test that no acceptance position is invented for witnessed B.
- [ ] **Step 2:** Run `client_state` and observe failure.
- [ ] **Step 3:** Implement the functions and needed native constructors/accessors. `receive_accepted` installs shared state while preserving pending originals and hidden local deletions. One unsettled prepared version per record; edits change working only. Retry returns the identical prepared version without advancing counters. A terminal changed version precondition settles the old proposal; a lost acknowledgment alone does not. New counters exceed both local high-water and all observed own components.
- [ ] **Step 4:** Run `cargo +1.99.1 test -p clacks-sync --test client_state --locked --offline`; include no-B-acceptance, same-value supersession, pending deletion, missing-original review, counter overflow and fresh reauthorization actor with retained excluded work. Receipt logs and separate request IDs must be absent.
- [ ] **Step 5:** Commit; explain preserved original versus witnessed own version and the one-record prepared lifecycle.

## Task 5: Production acceptance conditions and commit updates

**Files:** Create `src/acceptance.rs`, `src/membership.rs`, `tests/acceptance.rs`.

**Consumes:** Prepared changes, accepted observations, native validation, account head and evidence.

**Produces:** `ClientMembership { actor, created_at, closed_at: Option<Position>, acknowledged: Option<InstalledCheckpoint> }`; `MembershipSnapshot { account, generation: u64, clients: Vec<ClientMembership> }`.

`AcceptanceInput<'a> { account: &'a AccountMetadata, membership: &'a MembershipSnapshot,
current: &'a Observation, references: &'a [Observation] }` supplies rows loaded
inside the integrating transaction. `AcceptanceCommit { expected_head: Position,
expected_membership_generation: u64, expected: VersionCondition, dependencies:
Vec<VersionCondition>, next_account: AccountMetadata, accepted: AcceptedRecord,
native_changed: bool, journal: JournalChange }`; `JournalChange { installed_at: Position, key, stamp }`.

```rust
plan_acceptance(change: &PreparedChange, input: &AcceptanceInput<'_>, evidence: &EvidenceView<'_>)
    -> anyhow::Result<AcceptanceDecision>
plan_server_acceptance(change: &ServerChange, input: &AcceptanceInput<'_>, evidence: &EvidenceView<'_>)
    -> anyhow::Result<AcceptanceDecision>
```

`ServerChange { key, state: RecordState, expected: VersionCondition,
dependencies: Vec<VersionCondition> }` is an internal-server-origin change input,
not a network client's replacement for admission. It reuses native/reference and
conditional-write checks, derives context from the compared accepted state and
adds only the server component. Initial group-definition seeding must work with
no enrolled clients and no fabricated client identity.

`AcceptanceDecision::{Commit(AcceptanceCommit), Covered(AcceptedRecord),
Stale(Observation), Rejected(RejectionReason)}`.
`RejectionReason::{ActorClosed { actor: ClientActorId, closed_at: Position },
ActorUnknown { actor: ClientActorId }, MissingReference { key: RecordKey }}`
represents normal admission/reference outcomes. Native invalid values remain
concrete errors. Extend `client.rs` with
`receive_rejection(current: &ClientRecord, reason: &RejectionReason) -> anyhow::Result<ClientUpdate>`:
keep local values, and turn closed-actor/reference failures into their explicit
review reasons without dropping the old prepared evidence. An adapter must match the emitted preconditions and save head,
envelope and journal atomically; stale plans cannot be applied unchanged.

- [ ] **Step 1:** Add failing tests: accept B against A produces S1:201 and retains B:1; repeat yields `Covered`, not another mutation. Change a tag/profile reference, head version or membership generation between planning inputs and require stale/rejection on re-evaluation. Include tag deletion and missing referenced equipment without silently rewriting pending values. From the Task 2 empty-account root, accept a native seeded group through `plan_server_acceptance`; assert server component S1:1 and no client component.
- [ ] **Step 2:** Run `acceptance` to confirm missing behavior.
- [ ] **Step 3:** Implement the exact acceptance inputs/outputs. Dependency conditions check native QSO tags and profile equipment; callers supply the compared dependency rows, not a fake lookup store. Do not treat missing group definitions as invalid references: unfamiliar associations are preserved. A metadata-only equal-result context join advances metadata but reports no native change. Emit `AcceptanceCommit.native_changed` from the compared native states, never vector differences; a context-only join sets it false without retaining a redundant old payload or adding a getter wrapper.
- [ ] **Step 4:** Run `cargo +1.99.1 test -p clacks-sync --test acceptance --locked --offline`; verify actual group compiler errors retain their source and revocation cannot be bypassed by a larger counter. Tests re-evaluate changed inputs; they must not claim this proves database atomicity.
- [ ] **Step 5:** Commit with the adapter's atomic preconditions and context-only/native-change distinction in its body.

## Task 6: Complete manifests, partial installation and durable checkpoint rows

**Files:** Create `src/transfer.rs`, `tests/transfer.rs`.

**Consumes:** Version stamps, comparison, client updates, checkpoints and lineage.

**Produces:** `RecordRequirement { key, stamp, deleted: bool }`; `WindowMode::{Full, Incremental { after: Coverage }}`; `WindowManifest { account, source: InstalledCheckpoint, mode, records: Vec<RecordRequirement>, boundaries: Vec<TimelineBoundary>, baselines: Vec<Baseline> }`; `WindowProgress { source: InstalledCheckpoint, mode: WindowMode, generation: u64, installed: BTreeSet<RecordKey> }`. Store the immutable manifest separately; do not clone its full record list into each progress update.

`InstallBatch { source: InstalledCheckpoint, mode: WindowMode,
expected_generation: u64, accepted: Vec<AcceptedRecord>, client_updates:
Vec<ClientUpdate>, installed_keys: Vec<RecordKey>, next_generation: u64 }`
describes one bounded store transaction. `CheckpointUpdate { expected:
InstalledCheckpoint, next: InstalledCheckpoint, completed_source:
InstalledCheckpoint, expected_progress_generation: u64 }` is a proposed durable row
update. The integrating transport sends an `InstalledCheckpoint` reloaded after
actual commit. Do not add a forwarding-only acknowledgment wrapper/API. New
unsupported record kinds must fail decoding before progress/coverage advances;
protocol negotiation and compatible schema migration arrive with integration.

```rust
WindowManifest::new(account: AccountId, source: InstalledCheckpoint, mode: WindowMode,
    records: Vec<RecordRequirement>, boundaries: Vec<TimelineBoundary>, baselines: Vec<Baseline>)
    -> anyhow::Result<WindowManifest>
plan_batch(manifest: &WindowManifest, progress: &WindowProgress,
    incoming: &[AcceptedRecord], local: &[ClientRecord],
    evidence: &EvidenceView<'_>) -> anyhow::Result<InstallBatch>
finish_window(manifest: &WindowManifest, progress: &WindowProgress,
    installed: &[AcceptedRecord],
    previous: &InstalledCheckpoint, evidence: &EvidenceView<'_>)
    -> anyhow::Result<CheckpointUpdate>
```

Completeness requires exact declared keys plus every required version/deletion or
an interpretable retained successor. Deduplicate identical descriptors; reject
conflicts. Source enumeration uses a consistent real-store view. For an
incremental window the real journal supplies affected keys; a full window
certifies complete enumeration. Do not turn a partial page into certified
absence. Manifest replacement may reuse already installed matching/successor
versions but cannot credit undeclared or incompatible versions. Match progress
to the manifest's immutable `(source checkpoint, mode)`; a changed requirement
set under that same consistent source is invalid metadata. Batch updates append
installed keys and advance the checked progress generation in one transaction,
rather than copying the entire manifest/installed set. Derive client updates
from rows loaded within that same transaction so a concurrent local edit is not
overwritten by a previously computed client update.

- [ ] **Step 1:** Write failing T1:40→60 QSO/tag/profile interruption tests: install Q at 60, serialize/reload progress, assert `finish_window` errors while H/P are absent. Install H/P, finish and assert checkpoint 60. An unresolved local Q conflict must not prevent accepted Q installation or completion after all records arrive.
- [ ] **Step 2:** Run `transfer` to observe the incomplete-window failures.
- [ ] **Step 3:** Implement batch planning and completion validation. Batches update only listed records plus their local comparison evidence. The transport later sends committed checkpoint rows by caller contract; no simulated durability flag or acknowledgment-constructor wrapper. Preserve both original acceptance stamps and separate journal installation positions. Record vectors never mutate account coverage.
- [ ] **Step 4:** Run `cargo +1.99.1 test -p clacks-sync --test transfer --locked --offline`; include lost acknowledgment, stale/reordered/duplicate pages, replaced manifest, tombstone omission, cross-account descriptors and incompatible baseline. Full coverage remains 40 after Q alone.
- [ ] **Step 5:** Commit with the consistent-source and committed-checkpoint adapter contracts.

## Task 7: Membership cutoff, basis witnesses and baseline projection

**Files:** Create `src/compaction.rs`, `tests/compaction.rs`; extend `membership.rs` and `client.rs`.

**Consumes:** Membership, installed coverage, accepted contexts, pending/prepared rows and lineage.

**Produces:** `MembershipUpdate { expected_head: Position, expected_generation: u64,
next: MembershipSnapshot, next_account: AccountMetadata }`; `BasisWitness { key, original: OriginalBasis,
observed: VersionStamp, checkpoint: InstalledCheckpoint, relation: RecordRelation,
lineage: Lineage }` retains original native values through its original basis.

`CompactionInput<'a> { account: &'a AccountMetadata, membership: &'a MembershipSnapshot,
records: &'a [AcceptedRecord], surviving: &'a [VersionStamp] }` comes from a
consistent store view. `BaselinePlan { expected_head, expected_membership_generation,
expected_records: Vec<VersionCondition>, baseline: Baseline, projected:
Vec<AcceptedRecord>, removable_clients: BTreeSet<ClientActorId> }`.

```rust
revoke(actor: ClientActorId, account: &AccountMetadata, membership: &MembershipSnapshot)
    -> anyhow::Result<MembershipUpdate>
common_coverage(server: &InstalledCheckpoint, membership: &MembershipSnapshot,
    evidence: &EvidenceView<'_>) -> anyhow::Result<Coverage>
seal_basis(pending: &PendingRecord, observed: &AcceptedRecord,
    checkpoint: &InstalledCheckpoint, evidence: &EvidenceView<'_>)
    -> anyhow::Result<BasisWitness>
plan_baseline(id: BaselineId, input: &CompactionInput<'_>, evidence: &EvidenceView<'_>)
    -> anyhow::Result<BaselinePlan>
```

Expand acknowledged ancestry before same-coordinate minima, including the
server's own coverage. An initializing member without a durable acknowledgment
cannot advance the current baseline. Only closed clients whose admitted history
is covered may be projected. Unrevoked clients never expire. Preserve active
client components needed to recognize prepared versions.

Project only a current accepted envelope proven to represent its key's common
prefix. Replace collectible closed components with baseline interpretation,
retain actual accepted positions and residual components. A private original is
sealed with its specific observed relationship, never changed into the current
head. Missing proof means retain the component/context, not guess. Old accepted
inputs covered by the baseline are known/superseded; inputs beyond it require
residual evidence. Keep nonredundant position-dependent lineage intervals.

- [ ] **Step 1:** Add failing tests for acknowledgments 900/700/300, server 950, pending original 200/local B/shared 600. Assert common coverage 300; revoking the client at 300 permits 700, while simple inactivity does not. With another writer closed at 80 and complete coverage 300, assert that writer's collectible component is actually removed while state and actual accepted position remain unchanged.
- [ ] **Step 2:** Run `compaction` and observe failures.
- [ ] **Step 3:** Implement closure, witness creation and projection under the specification's six-step transition. Emit expected head, membership generation and every touched version so actual installation revalidates them. New baseline must be durable before cleanup; projection plans themselves do not delete tombstones, evidence or payloads. No permanent clock-counter registry for removed writers is introduced.
- [ ] **Step 4:** Run `cargo +1.99.1 test -p clacks-sync --test compaction --locked --offline`; include missing witness context, pending prepared version, enrollment/revocation race, changed envelope, old baseline ID without its interpretation and multiple timeline coordinates. Assert preserved original 200 after acknowledgment 700 and no change to native/upload-relevant values.
- [ ] **Step 5:** Commit with the retention proof conditions, residual-context fallback and real-installation preconditions.

## Task 8: Recovery boundaries and retained accepted-state installation

**Files:** Create `src/recovery.rs`, `tests/recovery.rs`; extend lineage/transfer entry points only where specified here.

**Consumes:** Account metadata, record comparison, membership, complete-window updates and baseline interpretation.

**Produces:** `HistoryObservation { account, advertised: Position, remembered: Coverage }`; `HistoryCheck::{NoContradiction, Regression { advertised, remembered }, Unknown(UnknownReason)}`; `RecoveryBoundaryPlan { expected_head, expected_membership_generation, next_account: AccountMetadata, boundary: TimelineBoundary }`.

```rust
check_history(observation: &HistoryObservation, evidence: &EvidenceView<'_>)
    -> anyhow::Result<HistoryCheck>
plan_recovery_fork(new_timeline: TimelineId, account: &AccountMetadata,
    membership: &MembershipSnapshot, known_restored: &Coverage)
    -> anyhow::Result<RecoveryBoundaryPlan>
plan_recovered_install(incoming: &AcceptedRecord, current: &Observation,
    original: Option<&OriginalBasis>, returning_actor: ClientActorId,
    account: &AccountMetadata, membership: &MembershipSnapshot, evidence: &EvidenceView<'_>)
    -> anyhow::Result<RecoveredDecision>
plan_recovered_coverage(account: &AccountMetadata, completed: &InstalledCheckpoint,
    membership: &MembershipSnapshot, evidence: &EvidenceView<'_>)
    -> anyhow::Result<RecoveryBoundaryPlan>
```

`RecoveredDecision::{Install { expected_head: Position,
expected_membership_generation: u64, returning_actor: ClientActorId,
expected: VersionCondition, envelope: AcceptedRecord, journal: JournalChange,
next_account: AccountMetadata }, Covered, Review(ReviewInput)}`. Planning runs
on the client against the observed server rows; the server rechecks emitted
conditions and current returning-client admission before installation. Divergence
stays in client review; install never invents a
fresh author/version for a recovered accepted envelope. A recovered coverage
boundary can be planned only from a checkpoint reloaded after complete old-window
installation. A proposed `CheckpointUpdate` is not that evidence. The original
author's revocation does not reject data previously accepted and server-distributed to
the still-authorized returning client. Unaccepted work from that author remains
excluded; new reauthorization uses a fresh `ClientActorId` and preserves local
rows for explicit resolution.

- [ ] **Step 1:** Add failing cases for restored T1:160, witness through 200, fork T2:0, incomplete old window, then complete boundary at T2:41. Assert T1:199 is covered only after completion; T1:201 is not. A source-advertised 200 alone must not enable the boundary.
- [ ] **Step 2:** Run `recovery` to observe failures.
- [ ] **Step 3:** Implement regression checks and these conditional recovery transitions. Same-timeline counter regression, identity/state collision, boundary contradiction and falsely completed coverage are concrete diagnostics. Normal restart returns unchanged history. A below-baseline comparison without surviving interpretation returns explained unknown; genuine divergence without originals returns two-way review. Equal values do not require business resolution or fabricate global coverage.
- [ ] **Step 4:** Run `cargo +1.99.1 test -p clacks-sync --test recovery --locked --offline`; include A→server→C then C→restored server, revoked-A tail rejection, partial recovery with unrelated T2 progress, retained deletion memory and a backup before baseline 700. Include an explicit test documenting that an advertised revision already above the remembered one is **not** a universal rollback detector.
- [ ] **Step 5:** Commit with the detection domain, preserved original stamps and hub-and-spoke data path in its body.

## Task 9: Composition verification, public API audit and handoff

**Files:** Create `tests/scenarios.rs`, `lib/clacks-sync/README.md`, `docs/synchronization-core.md`; update `docs/handoff.md`, `docs/github-backlog.md`, `docs/mvp-roadmap.md`, and the design's status/implementation links to match actual delivery. Existing native-model scope remains accurate.

**Consumes:** All prior task APIs. Adds no simulation engine or replacement algorithms.

- [ ] **Step 1:** Write composed tests using those production functions and native QSO/tag/profile data. Exercise: prepare→accept→lost response→later edit→receive; partial install→serialized reload→completion; revoke→offline return→baseline projection; restore→partial accepted recovery→complete coverage→still-newer returning client. Use test-local values to supply inputs; do not implement a fake transaction manager or network scheduler.
- [ ] **Step 2:** Run `cargo +1.99.1 test -p clacks-sync --test scenarios --locked --offline` and confirm the assertions target the required transitions. Add any missing integration behavior in its owning module, with a focused regression there.
- [ ] **Step 3:** Audit all public items for actual production callers/purpose, no test constructors, no unused wrapper APIs, no panic/indexing paths and complete standalone rustdoc. Check that serialization preserves baselines, original/prepared contexts and actor roles. Explicitly document store transaction obligations and the absence of a complete arbitrary-rollback detector.
- [ ] **Step 4:** Run the full verification matrix below; all commands must succeed. Reconcile docs with the delivered subset, linking the library and precise downstream obligations. Keep issue #2 open until delivery evidence has actually been reviewed; do not equate this plan with delivery.
- [ ] **Step 5:** Commit with the composed evidence and remaining store/transport integration boundary. Follow the user's chosen execution method's branch-review workflow before publication.

## Verification matrix

Run with the pinned toolchain and inspect actual exit status/output:

```sh
cargo +1.99.1 fmt --all --check
cargo +1.99.1 test --workspace --all-targets --locked --offline
cargo +1.99.1 test --workspace --doc --locked --offline
cargo +1.99.1 clippy --workspace --all-targets --locked --offline -- -D warnings
cargo +1.99.1 check --workspace --all-targets --target x86_64-pc-windows-msvc --locked --offline
RUSTDOCFLAGS='-D warnings' cargo +1.99.1 doc --workspace --no-deps --document-private-items --locked --offline
git diff --check
```

Tests are not specified by an invented total count. Record the actual count,
platform and limitations. Check changed Markdown links/anchors, table structure
and unresolved placeholders. No new dependency, host service or network access
is required for these library tests. If the offline cache cannot resolve an
already-selected dependency, address that environment problem explicitly rather
than changing the design or claiming a successful run.

## Spec coverage and execution handoff

| Design section | Owning tasks |
| --- | --- |
| Requirements/researched composition | Global constraints; all tasks |
| Identities, envelopes and client metadata | 1–4 |
| Coverage, lineage, baselines, initialization | 2, 6–8 |
| Preparation, comparison and acceptance | 3–5 |
| Interrupted installation and acknowledgment | 6, 9 |
| Retirement, witnesses and compaction | 5, 7, 9 |
| Worked ordinary/interruption cases | 3–7, 9 |
| Rollback/restoration examples and limits | 8–9 |
| Production component and integration boundary | Delivery contract; 5–9 |

Review this plan before implementation. The earlier native execution choice was
for issue #1; it does not select execution for this new plan. Recommended method:
**native**, because these tasks share tightly coupled metadata interfaces and
form a sequential production library. Subagent-driven execution remains an
available choice if the user prefers independent review after each task.

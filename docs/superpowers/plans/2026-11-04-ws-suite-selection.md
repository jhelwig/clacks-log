# WS Suite Selection and Shared Lookup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver #21's authoritative WS callsign following, independent capture
directory lookup, shared transient directory results and Clear action.

**Architecture:** WS status changes the shared entry callsign; Worked Before
observes that field. Entry and independent WS capture obtain shared directory
handles by lookup key, while their save callers retain results with actual QSO
identities. Saving never waits for directory completion.

**Tech Stack:** Rust 1.99.1, egui/eframe and egui_kittest, Tokio 1.54.1,
tokio-util 0.7.16, futures-util 0.3.34 and existing SQLite/client-store operations.

WS Suite is the WSJT-X family of weak-signal applications. DX means the other
station; UDP is User Datagram Protocol, UTC is Coordinated Universal Time, HTTP
is Hypertext Transfer Protocol and API is application programming interface.

**Spec:** [Corrected #21 specification](../specs/2026-11-04-ws-suite-selection-design.md),
planning baseline commit `fc5098f`. Read both documents and repository `AGENTS.md`
before implementation. This plan authorizes no product behavior beyond that spec
and the explicit user decisions it records. This is planning, not implementation.

## Global Constraints

- Rust **1.99.1** and the existing lockfile; no dependency addition is planned.
- Entry lookup debounce is **500 ms** after an effective entry-call change.
- Successful transient results live **24 hours from successful completion**, with
  at most **128 reusable successes per workspace**. Hits do not extend age.
- Status updates the entry call only. Worked Before has no status input.
- Capture independently requests by its own completed contact's key: reuse,
  join or start on a miss, with no entry debounce and no wait before saving.
- Preserve separate snapshots, recorded-value precedence, whole-location
  selection, normalized-key/deletion guards and existing snapshot arbitration.
- Preserve #19 pairing, deduplication, supplied facts, default/tag capture and
  receiver behavior. Capture does not use or reset the manual form.
- No successful-result refresh, cache bypass, provider merging/fallback,
  persistent reusable cache, bulk catch-up, source selector or outbound WS control.
- New product behavior requires its original authority; engineering choices below
  are implementation decisions, not additions to the requirement register.
- Project-owned executable tooling and nontrivial fixtures use Rust. Extend
  existing test suites and production operations; no test SQL or failpoints.
- Before implementation, establish an isolated worktree using the worktree skills
  and user-owned placement preflight. Preserve the unrelated current modification
  to `references/desktop-toolkit.md`; do not copy it into this change.
- Before any server run or desktop validation, read the machine-local development
  inventory named by `AGENTS.md`. Use isolated workspace, configuration and log
  paths; tests use disposable data. No production deployment is authorized.
- Every commit uses a reviewed message file, with prose physically wrapped at
  **72 columns**. Do not merge or push as part of this plan without authorization.

## Review Focus

These boundary cases are included in the owning tasks' tests below:

1. `W7HEX` -> `W7HEX/P` changes entry/history input but retains the same directory
   key: restart 500 ms debounce and reuse directory data (Tasks 1–2).
2. Manual save before debounce expiry: preserve the original deadline and eventual
   saved-contact result without delaying the save acknowledgment (Task 1).
3. Capture with no status/entry lookup: a miss starts one lookup and the QSO is
   readable before its provider response is released (Task 3).
4. Status during manual saving: update the displayed call, preserve the submitted
   payload/key, and use the specified success-reset/failure behavior (Task 2).
5. Directory work during receiver reconfiguration or workspace shutdown: reception
   drains only its admitted saves, and directory tasks cannot use a closed store
   or leak results into another workspace (Task 3).

## Authority and engineering decisions

| Classification | Content and owner |
| --- | --- |
| Agreed requirements | The spec's section 1 authority table, including independent capture lookup, 24 hours, 128 results and Clear; implemented by Tasks 1–3. |
| Existing domain contracts | Snapshot eligibility/arbitration, synchronization notifications, immutable save submissions and offline persistence remain owned by current model/store operations. |
| Verifiable library constraints | `Shared` requires polling to progress; `TaskTracker::close()` does not prohibit new task registration; no data lock may span network/database awaits. Account for these in Task 1. |
| Engineering choices | Module/type names and signatures below, a short standard mutex for keyed state, lazy expiry and least-recently-used eviction, `TaskTracker` for workspace tasks, and an ordinary ordered channel for status. These implement the chosen shared-handle approach; they are not product requirements. |
| Unselected behavior | No refresh-success action, completion-first lookup requirement, delayed status application, persistent processing receipts or custom request broker is implied by this plan. |

One capability plan is appropriate: the lookup refactor is consumed by both entry
and capture. Tasks are sequential review units, not separate product projects.

## File and responsibility map

All paths below are relative to the Clacks repository.

| Files | Responsibility |
| --- | --- |
| Modify `lib/clacks-desktop/src/directory/lookup.rs`; create `directory/lookup/cache.rs`, `runtime.rs`, `persistence.rs` under the same `src` root | Keyed public-to-crate handles; transient cache; configured provider/task lifetime; caller-owned saved-result application. Keep QSO IDs out of provider requests and cache keys. |
| Modify `lib/clacks-desktop/src/application.rs`, `session.rs`, `session/settings.rs`, `session/settings/contacts.rs`, `ui.rs` | Publish one current workspace lookup client to the UI and receiver, integrate submitted save demand and shutdown, remove old QSO-keyed lookup commands/results. |
| Modify `lib/clacks-desktop/src/ui/logging.rs`, `ui/logging/directory.rs`, `ui/logging/responses.rs`, `ui/contacts.rs`, `ui/contacts/editor.rs`, `ui/contacts/responses.rs`, `ui/workbench.rs` | Caller-side debounce, display eligibility, submitted demand and entry actions, including lifecycle processing when panes are hidden. |
| Modify `lib/clacks-desktop/src/logging/defaults.rs` | One fresh-entry reset shared by Clear and successful manual save. |
| Modify `lib/clacks-desktop/src/ws_suite.rs`, `ws_suite/protocol.rs`, `ws_suite/protocol/binary.rs`, `ws_suite/runtime.rs` | Status decoding/delivery and capture's independent directory request. Leave pairing/contact conversion rules intact. |
| Modify existing `directory/lookup/tests.rs`, `directory/test_support.rs`, `ui/logging/tests.rs`, `ui/contacts/tests.rs`, `ui/workbench/tests.rs`, `ui/tests.rs`, `session/tests.rs`, `session/settings/tests.rs`, `session/settings/contacts/tests.rs`, `session/tests/ws_suite.rs`, `ws_suite/tests/{protocol,runtime}.rs`, `test_support/ws_suite.rs` | Migrate existing consumers/test construction and extend actual operation scenarios. Split an overlarge changed test module by scenario responsibility, retaining its module registration. |
| Modify `lib/clacks-desktop/src/telemetry/tests/{directory_login,ws_suite,session}.rs` and existing scenario registration in `telemetry/tests.rs` | Reuse actual scenarios to verify request counts, correlation and safe telemetry; migrate the session resource construction. |
| Modify the owning contracts listed in Task 4 | Integrate the agreed behavior and replace contradictory form-owned lifecycle wording. No schema, server protocol or new synchronized record family is planned. |

## Task 1: Replace form-owned lookup with shared keyed handles and migrate real consumers

**Files:** The directory, application/session and manual/saved-editor files above;
existing directory/UI/save tests. No dormant service-only delivery: complete the
manual and saved-editor migration within this task before its review/commit.

**Interfaces — engineering choices:**

```rust
// directory/lookup.rs; all items remain crate-internal.
#[derive(Clone)]
pub(crate) struct LookupClient { /* workspace resources, shared keyed state */ }
#[derive(Clone)]
pub(crate) struct LookupHandle { /* ready outcome or standard Shared future */ }
pub(crate) struct LookupRuntime { /* configuration watches and task ownership */ }
#[derive(Clone)]
pub(crate) struct LookupDemand {
    pub key: LookupKey,
    pub due: tokio::time::Instant,
    pub handle: Option<LookupHandle>,
}

impl LookupClient {
    pub fn request(&self, key: LookupKey) -> LookupHandle;
    pub fn retry(&self, key: LookupKey) -> LookupHandle;
    pub fn configuration_changes(&self) -> watch::Receiver<()>;
}
impl LookupHandle {
    pub fn ready(&self) -> Option<std::sync::Arc<LookupState>>;
    pub async fn wait(self) -> std::sync::Arc<LookupState>;
}
impl LookupRuntime {
    pub async fn new(
        store: ClientStore,
        binding: watch::Receiver<Option<EnrollmentBinding>>,
        installed: watch::Receiver<Option<ProvisionedConfiguration>>,
        context: Arc<OnceLock<eframe::egui::Context>>,
    ) -> (LookupClient, Self);
    pub async fn run(self, stop: CancellationToken) -> anyhow::Result<()>;
}

// directory/lookup/persistence.rs; a save caller owns this value.
pub(crate) struct PreparedLookup { /* optional ready snapshot and save gate */ }
pub(crate) fn prepare(
    client: LookupClient,
    store: ClientStore,
    qso: QsoId,
    demand: LookupDemand,
    changes: watch::Sender<()>,
) -> PreparedLookup;
impl PreparedLookup {
    pub fn ready_snapshot(&self) -> Option<Box<DirectorySnapshot>>;
    pub fn settle(self, saved: bool);
}
```

Retain the existing `LookupState` outcome vocabulary. `LookupHandle` makes a
cached result immediately inspectable; a cache hit must not require polling an
unstarted `ready().shared()` future before `ready()` can see it. Provider work
returns one `Arc<LookupState>` per admitted operation, with no QSO ID in that work.
`retry` preserves existing authentication recovery and quiet periods; it is not
a successful-result refresh or cache bypass.

Change `ContactAction::Save` from its `snapshot` field to
`directory: Option<LookupDemand>`. `None` remains valid for consumers with no
directory demand. Retain `expected` and `next` and the actual store save API.
Thread that consumer context through the two existing executors:

```rust
// session/settings.rs
pub(super) async fn execute(
    store: ClientStore, request: SettingsRequest,
    lookup: Option<LookupClient>, changes: watch::Sender<()>,
) -> Option<Completion>;
// session/settings/contacts.rs
pub(crate) async fn execute(
    store: &ClientStore, action: ContactAction,
    lookup: Option<LookupClient>, changes: watch::Sender<()>,
) -> anyhow::Result<ContactResponse>;
```

Other action variants ignore the directory context. Do not expose these types
from the model/store crates.

For the migrated UI consumer in `ui/logging/directory.rs`, use
`observe(&mut self, call: &str, now: Instant)`,
`advance(&mut self, context: &egui::Context, lookup: Option<&LookupClient>, now: Instant)`
and `submission(&self) -> Option<LookupDemand>`. `observe` owns effective-call
comparison and the deadline; `advance` owns due admission, configuration-change
recovery and result polling. Replace `saved(qso, call)` with `saved(call: &str)`
and retain its existing no-refresh-on-open behavior. Keep `retain`, fallback and
failure presentation in that same consumer; no QSO identity is needed for display.

- [ ] **1. Write requirement-based failing scenarios in the existing suites.**
  Replace `directory_lookup_debounces_actual_calls_and_preserves_saved_owner`,
  whose equal-key call-change expectation contradicts the spec. Add the named
  scenarios below with actual provider fixture requests and production store APIs:

  | Scenario | Required assertions |
  | --- | --- |
  | `shared_lookup_joins_and_reuses_one_result` | Concurrent same-key calls dispatch one lookup (login excluded); all results have identical lookup ID/time. An immediate later hit dispatches zero additional lookups. |
  | `shared_lookup_expires_and_evicts_successes` | At 24 hours from completion the next demand makes a new request; hits do not extend age. Inserting success 129 evicts the least-recently-used of 128; an evicted caller's retained handle still works. |
  | `configuration_changes_preserve_success_but_replace_dispatch_material` | A success retains provider/ID/time after material/selection changes; an uncached key uses current configuration. Preserve existing old-rejection/new-material coverage. |
  | `current_failed_entry_resumes_on_usable_configuration` | A current entry without success can request again when configured material becomes usable; a current success is not refreshed, and saved contacts are not retried as bulk catch-up. |
  | `manual_save_before_lookup_due_keeps_original_deadline` | Call at 0 ms, submit at 100 ms, commit/reset before 500 ms; no request at 499 ms, one at 500 ms, and late success belongs to the submitted QSO. |
  | `saved_editor_keeps_lookup_trigger_and_result_guards` | Opening unchanged contact makes zero queries; changed call queries after debounce; deletion/key change prevents late application. Preserve failure/not-found snapshot behavior. |

  Illustrative assertion values, not new test-only production APIs:

  ```rust
  assert_eq!(first.lookup_id(), joined.lookup_id());
  assert_eq!(first.lookup_time(), reused.lookup_time());
  assert_eq!(lookup_http_requests, 1); // exclude provider login
  // original due = first effective call change + 500 ms
  assert_eq!(requests_at_499_ms, 0);
  assert_eq!(requests_at_500_ms, 1);
  ```

  Extend `directory::test_support::Fixture` with `lookup_count(&self) -> usize`
  and `delayed_response() -> (Response, oneshot::Sender<&'static str>)` for the
  existing QRZ fixture path. Count captured actual lookup requests, not login;
  the delayed response uses the body-stream/oneshot pattern already used by
  `ownership_scenario`. Test helpers stay test-only; no production endpoint or
  fake provider service is introduced. Controlled-clock tests must let real HTTP
  I/O settle without unintentionally advancing the debounce clock.
- [ ] **2. Run and inspect the failing scenarios.**
  `cargo +1.99.1 test -p clacks-desktop --lib --locked --offline directory::lookup::tests::`
  and the affected `ui::contacts::tests::`/`ui::logging::tests::` filters. The new
  behavior should fail on absent keyed sharing/timing, not broken fixture setup.
- [ ] **3. Implement keyed lookup and workspace ownership.**
  In `cache.rs`, keep separate in-flight handles and successful entries keyed by
  `LookupKey`. Record successful completion `Instant` and last-use `Instant`;
  lazy-remove expired entries and evict the least recently used success when
  exceeding 128. At equal last-use times, use key ordering as a deterministic
  internal tie-break. Failures/not-found are terminal outcomes, not reusable
  negative entries. Keep actual successful lookup UTC time/ID unchanged.

  In `runtime.rs`, retain current protected configuration loading, provider
  sessions, auth pause/renewal, quiet-period logic, native reference normalization
  and existing safe error handling. An atomic short mutex section performs
  cache-hit/join/admit. No mutex spans an await. Use the existing runtime handle,
  `futures_util::Shared` and `tokio_util::task::TaskTracker` to drive and join work.
  Register admitted tasks under the same admission lock that shutdown uses to
  close admission: `TaskTracker::close()` alone does not reject new spawns. Do
  not build a command broker or consumer subscription registry.

  Completion stores one successful snapshot, removes that actual in-flight
  request, and wakes the UI. Actual material captured at dispatch determines
  whether an authentication error may pause current credentials. Cache reuse is
  independent of provider configuration changes; new dispatch uses current
  configuration. On stop, close admission, cancel unfinished lookup/consumer
  work, close/wait the standard tracker, then allow store closure.
  The async constructor resolves the initial available selection/material before
  publishing the client; protected-storage failure is a safe lookup-unavailable
  outcome, not a failed workspace open. This avoids an uninitialized-client race
  with the receiver. Subsequent configuration changes use the existing watches.
  Publish actual usable-configuration changes through the standard watch exposed
  by `configuration_changes()`. Only active entry/editor consumers without an
  eligible success resume their existing demand; preserve any still-future
  original debounce deadline. This retains existing configuration/authentication
  recovery, not a new saved-contact retry or successful-result refresh policy.
- [ ] **4. Implement caller-owned save application and migrate entry/editor.**
  `persistence::prepare` starts ordinary tracked consumer work before the database
  save. An already-ready eligible result is offered through `ready_snapshot()`
  for the existing atomic store save; that branch needs no late-result task.
  Otherwise the consumer independently waits until `demand.due`, obtains/joins
  the key's handle (or uses its captured handle), and retains the result while
  awaiting a save-success gate. `settle(true)` permits guarded snapshot application;
  `settle(false)` or dropping the save gate cancels still-unadmitted consumer work
  and never installs a snapshot. Already shared network work retains its own
  workspace lifetime. The save path never awaits this consumer's provider result.

  Prepare before `save::contacts::save`; settle from the actual store outcome,
  before a following settings-read refresh. `Saved`/`Unchanged` are successful
  saves; stale/failure is not. Applying a changed snapshot sends the existing
  change notification; report apply failure separately from committed-QSO success.
  Do not add a per-QSO pending-request table/map to the lookup service.

  `DirectoryView` keeps actual effective call text, key, due time and optional
  handle/success for that consumer. Observe trim/uppercase call changes separately
  from key equality. Its submission produces a cloned `LookupDemand`; clearing
  the display drops only its own pending interest. Preserve saved-editor snapshot
  presentation, field fallback and existing retry controls. Poll these consumer
  deadlines/results from the native lifecycle, not only visible form drawing;
  use the existing repaint mechanism for due times/completion.

  In `application.rs` replace QSO-tagged lookup update delivery with
  `watch::Receiver<Option<LookupClient>>` in `UiResources` and the corresponding
  sender in `SessionResources`. Publish a fresh client on actual workspace open
  and `None` on closure/transition; drop former UI consumers on that transition.
  Pass that same client to save consumers. Remove `LookupCommand`, `LookupUpdate`,
  the `SettingsAction::Directory` forwarding branch, QSO-keyed entry/request maps,
  `Saved`/`Deleted` notifications and form-close command flushing once migrated.
- [ ] **5. Preserve and update telemetry with the migrated real lookup scenario.**
  Keep actual HTTP counters at the adapters, and count logical operations once
  at admission/terminal outcome. A join/cache hit creates no new operation metric
  or lookup ID. Move QSO correlation into saved-result application; provider
  spans retain lookup/material-safe correlation without raw calls/facts. Update
  `ownership_scenario` and its existing telemetry consumer to use new handles,
  retaining its substantive persistence and data-exclusion assertions.
- [ ] **6. Run affected suites, then commit the complete real-consumer refactor.**
  Run `cargo +1.99.1 test -p clacks-desktop --lib --locked --offline` and
  `cargo +1.99.1 test -p clacks-client-store --test integration --locked --offline directory_snapshots::`.
  Expected: all relevant scenarios pass; obsolete command assertions are replaced
  by request/timing/persistence assertions. Review changed tests separately against
  the requirement and Clacks mistake they detect. Commit only this task's files
  using a reviewed 72-column message file.

## Task 2: Deliver status-driven shared entry and Clear

**Files:** WS protocol/output and application/UI files in the map;
`logging/defaults.rs`, `ui/logging/responses.rs`, existing protocol, logging,
workbench and window tests, and the test-only WS frame helpers.

**Interfaces — engineering choices:**

```rust
// ws_suite/protocol.rs
pub(crate) enum DecodedMessage {
    Status(StatusCall),
    Completion(DecodedCompletion),
}
pub(crate) struct StatusCall { pub call: String }
pub(crate) fn decode(bytes: &[u8], sender: SocketAddr)
    -> anyhow::Result<Option<DecodedMessage>>;

// logging/defaults.rs: rename reset_after_save and retain its current body.
pub fn reset(&mut self, active: &OperatingState, view: SelectedView, now: UtcSecond);
// ui/logging.rs
pub(super) fn incoming_call(&mut self, call: &str);
pub(super) fn clear(&mut self);
// ui/workbench.rs
pub(super) fn incoming_call(&mut self, call: &str);
```

- [ ] **1. Add failing wire, entry and Clear cases to existing suites.**
  `status_call_wire_fields` checks fixed schema-2/3 Status bytes, empty/null call,
  ignored appended data, structural truncation, zero frequency and blank/unknown
  mode without native-radio validation. Extend the independently authored
  `test_support::ws_suite` frame helpers with `status(call: &str) -> Vec<u8>`.

  `status_changes_only_entry_and_drives_existing_observers` starts with recorded
  name/grid/reports/comment and verifies they survive WS call changes. Use the
  existing Worked Before model and local store fixture to check the resulting
  entry prefix and selected-view rows without a provider. Inspect the wiring
  during review to confirm no direct status-to-Worked-Before path is added.

  `authoritative_repeats_and_equal_key_changes_keep_correct_debounce` changes the
  entry locally, reapplies the previous WS value, sends unchanged packets, then
  changes `W7HEX` to `W7HEX/P`. Pin the exact due times: each actual effective
  change moves due by 500 ms; unchanged packets do not. Directory hits can still
  reuse the shared `W7HEX` result. Empty/clear followed by an old result must leave
  both display and Worked Before empty.

  `clear_uses_fresh_entry_reset_without_save` uses egui_kittest semantic button
  actions: Clear works with invalid input, leaves zero Save requests, resets all
  spec section 7 fields/timestamps/membership and retains both radio pairs/mode.
  Verify Callsign focus and pending-save disablement. Extend the existing
  successful-save reset case to exercise the same reset owner.

  `incoming_status_does_not_rewrite_pending_submission` admits call A's save,
  applies status B, checks visible B and the original submitted A payload/key,
  then separately exercises successful reset and an ordinary failed admission
  response preserving B/other input. No synthetic SQL failure is needed.
- [ ] **2. Run the named tests and confirm the missing behavior fails.**
  Use the `ws_suite::tests::protocol::`, `ui::logging::tests::` and
  `ui::workbench::tests::` filters with the Task 1 Cargo command form. Keep existing
  completion decoding/pairing tests compiling as the decoded enum changes.
- [ ] **3. Implement decoding and ordered delivery to entry.**
  Decode/skip only the fixed fields needed to reach DX call using existing binary
  primitives; malformed input is not an empty call. Route Status separately from
  completion admission. Add a `tokio::sync::mpsc::UnboundedSender<StatusCall>` to
  `ws_suite::Outputs` and receiver to `UiResources`/`App`; use ordinary arrival
  order and the existing context wake. This channel choice keeps native event
  delivery from blocking UDP capture and uses standard lifecycle draining, not a
  custom event log. Do not deduplicate calls in the receiver.

  `App::update_lifecycle` drains received calls into `Workbench::incoming_call`,
  which delegates only to `LoggingUi::incoming_call`. Replace only callsign,
  follow its normal validation/directory observation, and retain an immutable
  admitted save even while status changes the current field. Worked Before keeps
  using `contact_call()`; add no new WS input to it. Poll lifecycle consumers and
  deadlines even when their panes are hidden. Drop pending calls with their
  workspace on an actual transition; do not replay one workspace into another.
- [ ] **4. Implement the shared reset and button.**
  Rename/extract `NewEntry::reset_after_save` to `reset`; update successful manual
  saving and add `LoggingUi::clear` to call it and reset directory presentation,
  baseline, submitted state, validation/errors/feedback and focus. In the fixed
  header, render Save QSO then Clear with the existing right-to-left layout so
  Clear is physically on its left. Only a pending save disables Clear. It sends
  no request, confirmation dialog, shortcut or WS command. On save failure, keep
  the newer authoritative call and preserve other submitted values; do not replay
  a queued status after the outcome.
- [ ] **5. Run affected tests and formatting, then commit.**
  Run the full desktop library suite and `cargo +1.99.1 fmt --all --check`.
  Inspect assertions for real call-change, callback and reset behavior, not only
  label presence. Inspect the new control's placement/focus in Task 4. Commit this
  functioning entry/status/Clear slice with the reviewed message-file process.

## Task 3: Integrate independent capture lookup and workspace lifetime

**Files:** `ws_suite/runtime.rs`, `session.rs`, existing session/WS runtime tests,
directory fixture support and affected session construction in telemetry tests.

**Consumes:** Task 1 `LookupClient::request`, `LookupDemand`, `PreparedLookup` and
`persistence::prepare`; Task 2 decoded completion/status variants.

**Produces:** Add `lookup: LookupClient` to `ws_suite::runtime::run` immediately
after its `store` parameter. No pairing-identity or native-contact API changes.

- [ ] **1. Add failing real capture cases using the controlled provider endpoint.**
  Extend `session/tests/ws_suite.rs` or its focused child scenario module, retaining
  the existing single suite/production receiver and save path:

  | Scenario | Required evidence |
  | --- | --- |
  | `capture_miss_saves_before_directory_response` | No status or entry lookup. Send a usable completion pair; observe exactly one directory lookup request and read the saved QSO while the response body is held. Release the response; read its separate snapshot on that same QSO. |
  | `capture_reuses_or_joins_entry_lookup` | A cached success adds no provider request and retains ID/time; a held in-flight lookup is joined without duplication and the capture still saves before response. Clear/change entry while held; later application still targets the saved QSO. |
  | `distinct_captures_share_station_information` | Two actual distinct completion identities save two QSO IDs, consume one result, and do not become duplicates merely because lookup keys match. |
  | `late_capture_lookup_respects_current_record` | Use actual delete/edit operations before response release; deletion or key change prevents application, while editing another recorded field preserves eligibility and recorded values. |
  | `capture_lookup_failure_keeps_saved_qso` | An ordinary fixture transport/auth/not-found outcome leaves the committed contact readable and does not report capture failure. |
  | `receiver_drain_does_not_wait_for_provider` | With provider response held, receiver reconfiguration/drain completes after admitted QSO saves; it retains normal deduplication state. Workspace close cancels directory work and joins it before store closure. |

  Reuse existing controlled signals and reads; do not use arbitrary sleeps to
  infer saving. No failpoints, SQL triggers, dropped tables or new fault-injection
  production interfaces. Report an observed socket-delivery failure with its
  platform and Clacks impact; untested platforms alone do not add acceptance gates.
- [ ] **2. Run and inspect failures with the session WS filter.**
  `cargo +1.99.1 test -p clacks-desktop --lib --locked --offline session::tests::ws_suite::`
  Expected initial failures demonstrate capture does not yet request/join and
  persist directory data, rather than failures of completion decoding.
- [ ] **3. Wire one lookup client into both receiver start/restart paths.**
  Create/publish the client before starting the receiver. For each usable
  finalized candidate, derive the key from `projection.qso.contacted_station()`
  and call `request` immediately. Use `LookupDemand` with due `Instant::now()`
  and that handle; prepare the caller-owned save operation and pass its ready
  snapshot to the existing save. Settle from the actual save result before any
  history refresh. Never await the handle before saving. Preserve supplied facts,
  context/tag capture, the candidate's QSO ID, #19 deadlines and repeat suppression.

  Finish the candidate when its QSO save settles. Late enrichment runs through
  the workspace lookup tracker, not the receiver's `FuturesUnordered`/pairing
  drain. Ready, joined and newly started lookups use the same result-application
  path and existing store guards. No form, last-status call or source-selection
  association participates in this operation.
- [ ] **4. Integrate stop/reopen ordering without broad lifecycle redesign.**
  On actual workspace close/replacement: close new admissions; signal receiver
  drain and lookup cancellation, finish admitted QSO saves, join provider and
  snapshot-application work, then close the store. A receiver still finishing a
  save can obtain a terminal unavailable lookup handle after lookup admission
  closes; that must not fail or block its QSO save. Do not await provider responses
  from inside an enrollment/receiver drain. Preserve lookup/cache lifetime across
  receiver-only Apply/retry; use a new service for an actual workspace reopen.
- [ ] **5. Verify, review and commit the integrated capture slice.**
  Run the desktop library suite, retaining prior #19 scenarios for paired/repeated
  notifications, reconfiguration, close and enrollment drain. Verify actual
  request counts and store reads with the held response still unreleased.
  Commit with the reviewed message-file process.

## Task 4: Consolidate contracts and collect completion evidence

**Files:** `docs/product.md`, `docs/manual-logging.md`, `docs/desktop-ui.md`,
`docs/superpowers/specs/2026-11-02-directory-lookup-design.md`,
`docs/superpowers/specs/2026-11-03-ws-suite-capture-design.md`,
`references/ws-suite-udp.md`, and the affected existing telemetry scenarios.
The roadmap remains phase-level; GitHub owns delivery status.

- [ ] **1. Reconcile owning documentation with the implemented corrected spec.**
  Document status -> entry -> Worked Before, preservation of other unsaved fields,
  Clear/reset, independent capture reuse/join/start on miss, 24-hour/128 transient
  cache and existing failure-only retries. Replace contradictory exact-draft/QSO
  lookup ownership wording, including #19's prior-association paragraph, rather
  than appending competing conclusions. Preserve research source/provenance and
  distinguish verified Clacks behavior from external application setup. Add no
  fresh decision merely for module names, TaskTracker or cache bookkeeping.
- [ ] **2. Extend the existing configured telemetry scenario with real shared work.**
  Exercise one actual lookup with multiple consumers and a cache hit, followed by
  a real late snapshot application. Assert actual request/logical-operation counts
  remain one, original lookup ID/time survive reuse, QSO ID is correlated at the
  application boundary, and outcome levels distinguish saved QSO from enrichment
  failure. Keep existing credential/call/raw-response exclusion checks and bounded
  metric labels. Do not create a new metric for every helper or test library
  internals. Run the existing telemetry suite and inspect its actual exports.
- [ ] **3. Run final repository checks once the focused suites pass.**
  Read the current server guide for disposable test database setup; do not use
  persistent development accounts. Run the required commands from the repository
  overview, including:

  ```sh
  cargo +1.99.1 fmt --all --check
  cargo +1.99.1 test --workspace --all-targets --locked --offline
  cargo +1.99.1 test --workspace --doc --locked --offline
  cargo +1.99.1 clippy --workspace --all-targets --locked --offline -- -D warnings
  cargo +1.99.1 check --workspace --all-targets --target x86_64-pc-windows-msvc --locked --offline
  RUSTDOCFLAGS='-D warnings' cargo +1.99.1 doc --workspace --no-deps --document-private-items --locked --offline
  ```

  Success means the commands actually pass with their required environment. If a
  target tooling or database prerequisite is missing, report that check as
  unavailable; do not replace it with a different claim. Do not resurrect explicitly waived
  checks unrelated to #21 or silently claim an existing platform gap is resolved.
- [ ] **4. Verify the affected production control and Clacks receiving path.**
  Follow machine-local isolation and verify executable/explicit launch arguments
  before attaching UI automation. Inspect Clear immediately left of Save QSO,
  using the existing shared styling, and Callsign focus after reset. These checks
  catch incorrect Clacks placement or focus wiring. Task 2's interaction cases
  establish invalid-input availability, pending-save disablement and reset of the
  shared entry; do not require a second manual check solely because it is absent.
  Record the inspected platform and relevant visual evidence for these claims.

  Exercise independently authored Status packets through the Clacks receiver to
  the entry and local Worked Before. Check the decoded-call mapping and that
  history follows the entry, including the dedicated window. Existing lifecycle
  cases must catch processing incorrectly tied to rendering a particular pane;
  they do not create a separate minimized-window acceptance gate. Task 3's held
  provider responses and failure scenarios establish capture's independence from
  directory access without orchestrating live provider/server outages.

  Installed GridTracker/WS forwarding settings are outside Clacks's responsibility
  and are not a delivery gate. Do not carry over #19 multicast/IPv6 or native
  platform checks automatically. Additional visual states, resizing and platforms
  require a specific Clacks concern and a gap in existing evidence before becoming
  #21 acceptance checks; their absence alone does not justify a caveat.
- [ ] **5. Review the composed behavior and tests, then prepare the review handoff.**
  Check spec sections 1–9 against Tasks 1–4 and the actual evidence. Verify no
  `LookupCommand`, form-keyed provider request owner, direct status-to-history
  input, success-refresh action or capture-miss omission remains. Review changed
  tests for the requirement and Clacks error they detect, not just passing status.
  Commit scoped final docs/verification changes with the reviewed message-file
  process. Report verified behavior and concrete defects or tooling blockers.
  Distinguish a failed build prerequisite from a defect in Clacks. Do not list
  unperformed checks as risks or gates without identifying the Clacks mistake
  they could expose and why existing evidence does not address it. Assess delivery
  against the specification's Clacks behavior checks; this plan's existence is not
  completion evidence. No merge/deployment is part of this plan.

## Plan self-review

Spec sections 1–2 map to the authority/global constraints; sections 3–4 to Task 1;
section 5 and Clear in section 7 to Task 2; independent capture and lifecycle in
section 6 to Tasks 1/3; section 8 to each task's focused evidence plus Task 4;
section 9 to Task 4's owning-document reconciliation. All five Review Focus cases
have named scenario assertions above. Interfaces use one keyed request operation
for both entry and capture, and all save-result application remains caller-owned.
No new product decision is required to execute this plan. Exact internal helper
bodies remain implementation work within these interfaces and constraints.

# WS Suite Capture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Deliver #19's configurable desktop UDP receiver and duplicate-safe
completed-contact capture through the existing local save/synchronization path.

**Architecture:** One workspace-owned receiver task uses standard socket options,
Tokio I/O and a small in-memory pairing buffer. The existing desktop menu,
dialog and asynchronous session patterns configure it; the existing local-state
table persists preferences. Incoming values become native QSOs saved by the
ordinary contact operation, without a second synchronization path.

**Tech Stack:** Rust 1.99.1; egui/eframe/egui_kittest 0.37.0; Tokio 1.54.1;
winnow 1.0.3; socket2 0.6.5 with `all`; network-interface 2.0.5; existing SQLx
0.9.0/SQLite, tracing and metrics facilities. Dependencies inherit workspace
declarations; no other dependency is selected by this plan.

**Spec:** [Approved #19 specification](../specs/2026-11-03-ws-suite-capture-design.md).
Read it and repository AGENTS.md before executing. WS Suite means the WSJT-X
family of weak-signal applications; UDP is User Datagram Protocol; ADIF is
Amateur Data Interchange Format, with length-framed ADI text; UTC is Coordinated
Universal Time. SQL is Structured Query Language. A radio contact is a QSO.

## Global Constraints

- Preserve Rust 1.99.1 and the existing library/application workspace layout.
- First type 5 or 12 starts a **100 ms pairing window**; both end it early;
  type 12 wins; repeated same-type messages do not restart the window.
- Keep the latest completed identity per source until the next successful
  completion, in memory only, including across receiver-only reconfiguration.
- Truncate decimal-watt power below milliwatt precision: `5.0007 W -> 5000 mW`.
- Resolve contest codes through eligible account definitions, never seed IDs.
- Use existing `local_state` and contact saves: no migration, capture receipts,
  durable packet queue, synchronization record or protocol change is planned.
- Core reception is configured through **Settings → WS Suite…**, not file edits.
- Settings persistence and listener activation have separate truthful outcomes;
  no staged listeners, seamless handoff or rollback protocol.
- Windows is the MVP runtime target; retain macOS/Linux foundations and report
  native platform evidence separately from compilation and automated checks.
- Tests are noninteractive, use temporary workspaces/disposable databases and
  existing production APIs; fixture generation, if needed, is Rust.
- Read the machine-local development inventory named by AGENTS.md before any
  server/desktop launch; use explicit isolated workspace/config/log paths.
- Implementation requires the user's explicit authorization. Do not infer
  permission for commits, pushes or PRs; any authorized commit uses a
  reviewed message file and physically checked 72-column prose wrapping.

## Review Focus

Each item is pinned by a named scenario below, not a separate test framework.

1. Off must remain applicable with stale invalid fields or a missing interface
   (Task 1: `off_bypasses_unavailable_fields`).
2. Saved settings plus a failed bind must remain visible and retryable without
   pretending reception works or rewriting unchanged settings
   (Task 1: `saved_settings_survive_bind_failure_and_retry`).
3. A counterpart/repeat while its save is running must retain the same QSO ID;
   expiration and early pairing must obey the same 100 ms model
   (Task 2: `pairing_window_and_inflight_identity`).
4. Extra decimal digits must truncate rather than reject valid supplied power
   (Task 2: `supplied_power_truncates_to_milliwatts`).
5. Contacted-only contest bindings must not create own associations or false
   ambiguity; repeated eligible bindings in one group count as one candidate
   (Task 2: `contest_lookup_respects_binding_conditions`).

---

## Delivery boundaries and file map

Execute in order. Task 1 delivers actual settings, persistence and socket
ownership, not contact logging. Task 2 extends that same production receiver
with capture. Task 3 supplies whole-workflow evidence and documentation. No
partial task closes #19 or counts test-only code as the delivered capability.

| Files | Responsibility |
| --- | --- |
| `lib/clacks-client-store/src/ws_suite.rs` (new), `src/lib.rs` | Typed receiver preferences, default/load/save through existing local-state queries. |
| `lib/clacks-client-store/tests/ws_suite.rs` (new), `tests/integration.rs` | Real preference save/reopen scenarios in the existing linked suite. |
| `lib/clacks-desktop/src/ws_suite.rs` (new), `src/lib.rs` | Receiver action/response/view types and focused child modules. |
| `lib/clacks-desktop/src/ws_suite/interfaces.rs`, `socket.rs` (new) | Standard interface enumeration/resolution and socket construction. |
| `lib/clacks-desktop/src/ws_suite/runtime.rs` (new) | Listener lifetime, Apply, I/O/deadlines, candidate reads/saves and outcomes. |
| `lib/clacks-desktop/src/ws_suite/protocol.rs`, `protocol/binary.rs`, `protocol/adi.rs` (new) | Wire decoding, not an import/session framework. |
| `lib/clacks-desktop/src/ws_suite/contact.rs`, `pairing.rs` (new) | Native conversion, exact correlation and transient pairing decisions. |
| `lib/clacks-desktop/src/ws_suite/tests.rs` and focused child test files (new) | Fixed wire cases, mapping/pairing cases and real configured-socket scenarios. |
| `lib/clacks-desktop/src/ui/ws_suite.rs`, `ui/tests/ws_suite.rs` (new) | Actual receiver dialog, pending form, responses, nonmodal failure access and component tests. |
| `src/menus.rs`, `src/ui.rs`, `src/ui/menu_commands.rs`, `src/ui/tests.rs`, `src/ui/tests/menu_commands.rs` in the desktop library | Wire the new action into the existing menus/modals; preserve unrelated forms. |
| `lib/clacks-desktop/src/application.rs`, `session.rs`, `session/tests.rs`, `session/tests/ws_suite.rs` (new) | Output composition, existing session admission and workspace lifetime integration. |
| `lib/clacks-desktop/src/enrollment/tests/synchronization.rs`, `src/synchronization/tests.rs` | Extend the existing authenticated test host for actual captured-contact dissemination. |
| `lib/clacks-desktop/src/telemetry/tests.rs`, `src/telemetry/tests/ws_suite.rs` (new) | One production telemetry scenario using existing isolated subscriber composition. |
| Root `Cargo.toml`, `Cargo.lock`, desktop `Cargo.toml` | Workspace-managed dependencies needed by delivered consumers. |
| `docs/desktop-ui.md`, `docs/integrations.md`, desktop/store READMEs | Update owning contracts and actual usage after implementation; keep execution evidence in delivery/PR. |

Keep production modules focused; use named module files, not `mod.rs`.
The structure is a responsibility map, not a reason to add abstractions or
production APIs solely to satisfy tests. Reuse existing scalars/native values.

## Task 1: Deliver UI-controlled receiver settings and socket ownership

**Files:** Store preferences/tests; desktop receiver types, interfaces, socket,
runtime and UI; menu/application/session integration; workspace dependencies.

**Interfaces:**

- Store module owns `ReceiverSettings { enabled: bool, connection:
  ReceiverConnection }`. `ReceiverConnection` has named `Unicast` fields for
  local `IpAddr`/port and `Multicast` fields for group `IpAddr`/port/interface.
  `InterfaceSelection` is `SystemDefault` or `Explicit { name: String,
  address: IpAddr }`. Default is Off, unicast `127.0.0.1:2237`.
- `ReceiverSettings::validate(&self) -> anyhow::Result<()>` checks intrinsic
  address/mode/port/scope rules, not changing OS interface availability.
  `load(store: &ClientStore) -> anyhow::Result<ReceiverSettings>` defaults on
  an absent key without writing; malformed saved values are errors.
  `save(store: &ClientStore, next: &ReceiverSettings) ->
  anyhow::Result<ReceiverSaveOutcome>` returns named `settings` and `changed`
  fields, committing `ws_suite.receiver` only for a semantic change.
- Desktop `InterfaceChoice { name: String, address: IpAddr, index: u32 }`
  is transient enumeration output. `interfaces::enumerate() ->
  anyhow::Result<Vec<InterfaceChoice>>` flattens library results for the UI.
  `socket::bind(connection: &ReceiverConnection, interfaces:
  &[InterfaceChoice]) -> anyhow::Result<tokio::net::UdpSocket>` resolves
  explicit choices and applies the spec's platform socket options.
- `ReceiverAction` is `Interfaces` or `Apply { next: ReceiverSettings }`.
  `ReceiverRequest { action, reply: oneshot::Sender<ReceiverResponse> }` uses
  the existing session command pattern: add `SessionCommand::WsSuite` carrying
  it, with the same closing/admission checks as other workspace operations.
- `ReceiverResponse` distinguishes interface results, successful settings
  application with its `settings`/`reception` values, failure and Closing.
  `ReceptionState` is Off, Listening, or Failed with a safe message.
  `ReceiverView` carries optional loaded settings and
  current reception state for lifecycle handling and failure feedback; unloaded
  controls cannot Apply. Off/Listening are internal outcomes,
  not an additional status display in the dialog.
- `Outputs { updates: watch::Sender<ReceiverView>, context:
  Arc<OnceLock<egui::Context>> }` follows existing directory outputs. Pass its
  receiver to `UiResources` and sender to `SessionResources`.
- `runtime::run(store: ClientStore, commands:
  mpsc::Receiver<ReceiverRequest>, outputs: Outputs,
  settings_updates: watch::Sender<Option<Arc<SettingsSnapshot>>>,
  changes: watch::Sender<()>, pairs: Pairing,
  stop: CancellationToken) -> anyhow::Result<Pairing>` owns the
  receiver for one workspace. Session uses an ordinary bounded control channel
  and tracked task, as it already does for directory lookup. This is control
  delivery, not a database write queue or packet broker. The returned drained
  pairing state preserves latest-completion recognition if enrollment fails or
  is cancelled and the same workspace resumes. Opening a replacement workspace
  starts fresh recognition state.
- `ui::ws_suite::ReceiverUi` provides
  `open(&mut self, opener: Option<egui::Id>, view: &ReceiverView, commands:
  &mpsc::Sender<SessionCommand>)`, `is_open(&self) -> bool`,
  `poll(&mut self, view: &ReceiverView)` and
  `show(&mut self, context: &egui::Context, commands:
  &mpsc::Sender<SessionCommand>)`. Its form converts active fields to
  `ReceiverSettings` or existing `FieldError` values; Off builds from the
  committed connection, changing only `enabled`.

- [x] **1. Pin preference semantics with real-store tests.** In
  `tests/ws_suite.rs`, add `receiver_preferences_reopen` and
  `receiver_preferences_noop`. Assert default Off does not require setup,
  saving an enabled unicast configuration reloads identically after closing
  and reopening the store, and saving the same value returns `changed=false`.
  Use existing workspace creation and setting operations, with no raw-table
  assertions or duplicate checks of the dependency's key/value behavior.
- [x] **2. Run the targeted tests before implementing preferences.**
  `SQLX_OFFLINE=true cargo +1.99.1 test -p clacks-client-store --test integration --locked --offline ws_suite::`.
  Expect failure for the missing Clacks module/behavior; then implement the
  typed module with existing `local_state::{get_in,set_in}` and transactions.
  Register its tests in `tests/integration.rs`; rerun expecting PASS. No SQL
  file/migration/offline metadata change is needed merely to reuse these queries.
- [x] **3. Pin UI intent using egui_kittest.** Add
  `off_bypasses_unavailable_fields`, `mode_controls_and_cancel`, and
  `apply_submits_current_receiver_settings` through the real dialog. Assert
  mode-specific controls, 300 ms validation, Apply disabled for current errors,
  Off bypassing stale endpoint/interface errors, and Cancel sending no request.
  Capture the actual `SessionCommand::WsSuite`/oneshot boundary; do not build
  a fake service or repeat persistence tests through the form. Extend existing
  menu tests for ready/modal/closing admission and preserved unfinished entry.
- [x] **4. Run those scenarios to observe the absent UI behavior.**
  `SQLX_OFFLINE=true cargo +1.99.1 test -p clacks-desktop --locked --offline ui::tests::ws_suite`.
  Implement the shared-dialog form, command, outputs and response handling.
  Use OS interface name/address labels, not durable numeric indexes. Enumeration
  is `spawn_blocking` work on opening, explicit refresh and Apply; no watcher.
  A named IPv6 multicast interface resolves by exact name/address; a missing or
  ambiguous match is feedback, never automatic substitution. A unicast IPv6
  link-local bind must likewise resolve its required scope unambiguously from
  local interfaces rather than use a guessed index. Validate unicast addresses
  against actual local addresses (or an explicit wildcard); do not interpret
  adapter peer/destination addresses as broadcasts.
- [x] **5. Implement socket construction against fixed configuration cases.**
  Add workspace socket2 `=0.6.5` with `all`, network-interface `=2.0.5`, and
  inherited desktop dependencies; use existing winnow when Task 2 consumes it.
  Configure nonblocking/family/reuse/membership/loopback as spec §4.3 states,
  then `UdpSocket::from_std`. Do not wrap socket2 with a generic networking
  layer. IPv6 is IPv6-only. Multicast binds wildcard and joins the selected
  group; no Linux reuse-port load balancing. Use real configured socket cases
  to catch Clacks choosing the wrong bind/interface/group/sharing options.
- [x] **6. Add real session settings/lifecycle scenarios.** In
  `session/tests/ws_suite.rs`, add
  `saved_settings_survive_bind_failure_and_retry`: hold a real conflicting
  unicast socket, Apply through the session, assert persisted enabled settings
  plus Failed reception, release the conflict, Apply unchanged settings and
  assert Listening without another semantic save. Add
  `receiver_reconfiguration_and_disable`: apply a new endpoint without restart,
  disable it, and reopen the temporary workspace with the saved settings.
  Assert manual settings requests still work after a receiver bind failure.
  Run the cases red before adding their runtime wiring.
- [x] **7. Wire the actual workspace owner and outcomes.** Application creates
  the view; session starts `runtime::run` after workspace open, routes commands,
  and stops/joins it before store close or enrollment replacement. Validate,
  persist, then replace the socket in that order. A healthy no-op leaves it
  alone; retry of failed saved settings rebinds without a synthetic write.
  Persistence failure leaves the previous listener alone; activation failure
  reports saved settings plus Failed, without rollback. The dialog closes on
  successful Off/Listening Apply, stays open on activation failure, and retains
  submitted outcomes across dismissal. Outside-dialog failure feedback reopens
  the same settings UI. Normal configuration state uses the receive checkbox
  and existing unapplied-change treatment; add no Off/Listening status or
  endpoint summary. Keep a separate actionable reception failure message and
  clear it after successful activation or an applied disable. Preserve the
  existing single-modal rules and focus.
- [x] **8. Verify the milestone.** Run the store filter above and
  `SQLX_OFFLINE=true cargo +1.99.1 test -p clacks-desktop --locked --offline ws_suite`,
  then the existing menu filter `ui::tests::menu_commands`, formatting and
  Clippy for the two affected crates. Expect all relevant scenarios to pass.
  This milestone owns real configuration/sockets; completion reception is added
  to this same runtime in Task 2, with no alternative app or throwaway engine.
- [x] **9. Review this milestone's implementation and tests separately.**
  Check that every new helper has a production consumer and each test detects
  a specific Clacks mistake. Commit only if authorized, using the message-file
  convention; otherwise retain the reviewable diff for the next task.

## Task 2: Capture completion pairs through the real local save path

**Files:** Desktop protocol, contact, pairing and test modules; extend the Task 1
runtime and session notification wiring. No new server/store algorithm.

**Interfaces:**

- `protocol::decode(datagram: &[u8], sender: SocketAddr) ->
  Result<Option<DecodedCompletion>, DecodeError>` returns None for irrelevant
  message types. `DecodedCompletion` contains `SourceIdentity`,
  `CompletionIdentity`, `CompletionFormat`, named incoming contact facts and
  bounded conversion warnings. Facts preserve missing/invalid distinctions
  needed for live fallback; do not clone the complete persistence schema.
- `SourceIdentity { sender: IpAddr, instance: String }` excludes transport port.
  `CompletionIdentity` has the approved contacted callsign, UTC start, primary
  frequency-or-band and leaf mode discriminator. Use a named enum for the
  frequency/band alternative. `CompletionFormat` is Structured or Adif.
- `CaptureContext { operating: OperatingState, selected_view: SelectedView,
  definitions: DefinitionSet }` is copied from one existing settings snapshot.
  Recognition uses `RecognitionCatalog::adif_318()`. It does not retain an
  open read transaction, pending manual form or profile/equipment identities.
- `contact::build(id: QsoId, notice: &DecodedCompletion, context:
  &CaptureContext) -> Result<CaptureProjection, CaptureError>` returns named
  `qso` and `warnings` fields. `contact::parse_power_watts(text: &str) ->
  anyhow::Result<Option<Milliwatts>>` uses exact decimal arithmetic/truncation,
  not the manual form's fractional-digit restriction.
- `pairing::Pairing` is a receiver-owned collection of pending candidates and
  one latest completed identity per source. `CandidateKey { source:
  SourceIdentity, completion: CompletionIdentity }` names one candidate.
  `admit(&mut self, notice: DecodedCompletion, received: Instant) -> Admission`
  returns New/Joined with that key, or Suppressed. New allocates one QSO ID.
  `attach_context(&mut self, key: &CandidateKey, context: CaptureContext) ->
  anyhow::Result<()>`
  supplies the actual snapshot and prepares available representations using
  `contact::build`. `next_deadline(&self) -> Option<Instant>` selects the next
  wait; `take_ready(&mut self, now: Instant) -> Vec<ReadyCapture>` marks selected
  candidates as saving before returning them. `ReadyCapture` carries the key,
  QSO ID, first-receipt time, selected `Option<CompletionFormat>` (None for a
  rejected candidate with no usable format), and projection/rejection result.
  `settle(&mut self, key: &CandidateKey, saved: bool)` releases that candidate
  and, only on success, updates its source's latest completed identity. A
  selected/saving candidate stays recognized in the same collection; no second
  dedup cache, positional payload tuples or general event engine is introduced.
- Runtime consumes these interfaces inside its existing select loop alongside
  socket readiness, receiver requests, stop and tracked read/save futures.
  Snapshot acquisition and saves use ordinary task/future ownership; they never
  block packet consumption or become a custom client write queue.
  Pass the session's existing `settings_updates` and `local_changes` senders
  when the capture consumer uses them. Pairing state stays owned by this task
  while running and returns to the session on drain; reconfiguration retains
  it in place. No separate recognition store or pause coordinator is needed.

- [x] **1. Author fixed wire examples and failing decoder tests.** Add
  `completion_wire_fields` for schemas 2/3, type 5 appended-field boundaries,
  UTC/fixed-offset timestamps, exact Hz, and both mode representations.
  `adi_length_framing` contains a comment with literal `<EOR>` text, zero-length
  station values, Latin-1 optional text, a truncated field and multiple records.
  Assert exact native/transport values, not a round trip through Clacks's encoder.
  Keep bytes in test support; no upstream code execution or generated fixtures
  outside reproducible Rust tooling. Run
  `SQLX_OFFLINE=true cargo +1.99.1 test -p clacks-desktop --locked --offline ws_suite::tests`
  and observe the missing decoder behavior before implementing it.
- [x] **2. Implement only the delivered decoder.** Use winnow's checked
  big-endian/length-taking facilities for Qt byte arrays and type 5, and a
  length-framed single-record ADI parser for type 12. Apply spec §5.1 exactly:
  magic/schema checks, unknown-type/trailing-field rules, whole-second UTC,
  UTC/offset acceptance, local/named-zone rejection and the inspected ADI text
  encoding. Do not add a general import engine, heartbeat gate, decoder registry
  or persistent raw packets. Rerun the fixed examples expecting PASS.
- [x] **3. Pin native mapping with concrete assertions.** Add
  `live_context_and_selected_tag` with supplied own values, missing/empty own
  values, copied equipment/references and named/built-in selected views. Assert
  counterpart facts never borrow our defaults and source/default records are
  not mutated by the capture operation. Add
  `supplied_power_truncates_to_milliwatts` with
  `assert_eq!(power("5.0000"), 5000)`,
  `assert_eq!(power("5.0007"), 5000)`, and
  `assert_eq!(power("0.0019"), 1)`; zero-after-truncation and overflow follow
  optional-invalid handling rather than rounding up or default substitution.
  Here `power` is a test-local assertion adapter over `parse_power_watts`, not
  another production conversion API. Run red before implementing the mapping.
- [x] **4. Pin definition-driven activity lookup.** In
  `contest_lookup_respects_binding_conditions`, construct definitions with
  independently chosen GroupIds using existing checked definition APIs.
  Assert matching eligible Own/Always literals resolve to their actual group,
  duplicate bindings in one group count once, Contacted/BoundPresent do not
  qualify, and two eligible groups report ambiguity without an association.
  Missing/nonliteral matches are reported rather than using a seed ID or
  creating a group. Include received exchange fields that must not become
  own-station geography. Implement direct definition inspection only, respecting
  conditions and optional-identifier rules; do not reverse arbitrary expressions.
- [x] **5. Implement `contact::build`.** Use existing native constructors and
  the approved field map/optional-invalid policy. Take defaults and membership
  from `CaptureContext` once; apply supplied values before missing/empty fallback.
  Use exact decimal conversion, native band-frequency authority, correct mode
  hierarchy and separate recorded/provider facts. Invalid ADIF leaves valid
  structured fallback eligible. Report unsupported propagation/satellite/exchange
  fields according to the spec without new native slots or comment encoding.
  Run the mapping cases expecting PASS.
- [x] **6. Pin the pairing algorithm before implementing it.** Add
  `pairing_window_and_inflight_identity` using Tokio's simulated clock:
  either format at t=0 produces no save at 99 ms; its counterpart at 20 ms
  produces one ADIF selection immediately; either alone finalizes at 100 ms;
  a repeated same-type message at 80 ms does not move that deadline. Assert
  matching messages during a selected/saving candidate allocate no new QSO ID,
  latest-completed suppression remains while the next candidate is pending,
  and the next successful completion replaces it. Also pin distinct sources,
  exact key normalization, invalid-ADIF/usable-type-5 selection and absence of
  default/tag values from the key. An unusable ADIF representation must not
  suppress the valid structured fallback or start a second QSO. These test
  Clacks's selection, not Tokio.
- [x] **7. Implement and integrate pairing.** Record first receipt time before
  awaiting the context snapshot, allocate one QSO ID, retain one representation
  of each type, and cancel the deadline when both usable forms are available.
  Prefer ADIF as a whole, never merge type 5 optional fields into it. Read and
  close one existing settings snapshot for the candidate. Continue socket
  processing while reads/saves settle, and handle already-readable counterparts
  before expiration finalization as the spec requires. Dropping a duplicate
  must not reset a deadline, refresh a retention timer or change defaults.
- [x] **8. Wire one actual save per finalized candidate.** Call
  `clacks_client_store::save::contacts::save(&store, None, qso, None)`; no lookup
  is triggered by completion and no current manual-entry snapshot is guessed.
  Publish local changes only from a committed/acknowledged save, using the
  existing shared `local_changes` sender to invalidate reads and wake normal
  synchronization. Mark completion only at that boundary. Distinguish a
  committed save from any following refresh error. Terminal failure reports
  and releases the candidate; add no failed-packet cache or automatic retry loop.
- [x] **9. Exercise the assembled receiver through a real temporary session.**
  Add `udp_pair_saves_one_local_contact`: apply receiver configuration through
  the same request as the UI, send the authored pair, and query production
  contact reads to assert one QSO with the selected native facts/tag. Repeat
  the counterpart and same-type notification; assert still one contact. Send a
  different completion, then demonstrate the agreed latest-only replacement
  without asserting restart/deletion replay protection. Keep sync/directory
  unavailable; saving must succeed. A second scenario closes with a pending
  candidate, joins the save, reopens and reads it, establishing runtime drain
  wiring rather than generic SQLite durability.
- [x] **10. Preserve pairing across configuration changes.** Extend the Task 1
  runtime's Apply path to stop admitting new identities, finish admitted
  candidates through their existing deadlines/saves, then replace the socket.
  Retain latest-completed identities in the same receiver owner. Add
  `receiver_change_drains_and_retains_recent_identity` for a pending completion,
  an actual rebind and its repeated notification on the new endpoint. Workspace
  replacement/close must join the old receiver before closing the store, and
  reject late UI admission through existing lifecycle rules.
- [x] **11. Instrument the actual handling boundaries.** Use meaningful
  debug operation spans plus ordinary-level configuration/start/stop/commit
  events and handling-boundary failures. Define
  `clacks_ws_suite_captures_total` with bounded `format`/`outcome`, and
  `clacks_ws_suite_capture_duration_seconds` from first receipt to terminal
  candidate outcome. Count durable saves only after commit; suppressions are
  debug decisions, not extra captures. Keep source/QSO correlation in explicit
  permitted event fields, not metric labels. Exclude packet bodies, callsign
  fields, comments and credentials from default telemetry; no helper installs
  subscribers/exporters or duplicates span timing with synthetic events.
- [x] **12. Verify and review.** Run the desktop `ws_suite` filter and affected
  store integration filter, formatting, and affected-crate Clippy; expect PASS.
  Review all new tests against requirement/specific-Clacks-mistake criteria and
  inspect the composed runtime, not only the parser/pairer. If authorized,
  commit the integrated capture milestone with a reviewed wrapped message.

## Task 3: Complete workflow acceptance and owning documentation

**Files:** Existing authenticated synchronization host, telemetry composition
suite, new receiver scenarios, and owning desktop/integration/store documents.

**Interfaces:** Consume Tasks 1–2's real menu/session/runtime/store operations.
No alternative ingestion, synchronization, telemetry collector or test-only
production API is introduced for this task.

- [x] **1. Verify multicast integration on real sockets.** Add
  `multicast_coexists_with_peer_receiver` and
  `ipv6_multicast_capture_on_existing_interface` to the receiver suite. A second
  ordinary observer socket joins the same group/port; send a completion pair
  with local delivery enabled and assert both receive while Clacks saves one
  QSO. Cover IPv4/IPv6, explicit/default permitted scopes and Linux group
  isolation where applicable. The IPv6 packet case is an explicitly invoked
  native check, marked ignored with its prerequisite in ordinary runs. Use an
  existing non-loopback IPv6 interface from OS enumeration, interface-local
  groups, zero multicast hops and bounded waits. Missing interfaces and delivery
  failures fail the invoked check and leave that platform's evidence incomplete.
  Ordinary CI retains IPv6 socket-construction/options checks; it makes no IPv6
  delivery claim. Do not provision adapters/routes or a multicast simulator.
  These cases verify Clacks's configuration choices.
- [x] **2. Extend the existing authenticated workflow.** Use
  `enrollment/tests/synchronization.rs` and `synchronization/tests.rs`'s existing
  issuer/native-store/disposable-server setup. Configure a captured-contact
  source workspace, save via its actual UDP receiver, and observe the same QSO
  ID/content in a second enrolled workspace through normal synchronization.
  Add no manually fabricated accepted records, new synchronization algorithm,
  default credentials or persistent development accounts. This remains a native
  credential/platform acceptance case with the existing prerequisites.
- [x] **3. Verify observability through one production composition scenario.**
  Extend `telemetry/tests.rs`'s existing child-case registration and add
  `telemetry/tests/ws_suite.rs`. Exercise successful capture plus a real port
  conflict/failed activation, with configured ordinary-level filters. Assert
  truthful saved/failed outcomes, one committed capture for the pair, useful
  correlation despite filtered debug spans, finite label values, and absence
  of authored packet/comment secret-marker text. Reuse the existing collector;
  do not repeat the whole functional suite with telemetry on and off.
- [ ] **4. Run required automated checks in the execution worktree.** Before
  Task 1 begins, create/select isolation using the worktree-placement and
  git-worktree skills; preserve unrelated changes in the main checkout.
  Start with the focused commands in Tasks 1–2, then once the final change is
  complete run:

  ```sh
  cargo +1.99.1 fmt --all --check
  SQLX_OFFLINE=true cargo +1.99.1 test -p clacks-client-store --test integration --locked --offline
  SQLX_OFFLINE=true cargo +1.99.1 test -p clacks-desktop --locked --offline --no-run
  SQLX_OFFLINE=true cargo +1.99.1 clippy -p clacks-client-store -p clacks-desktop --all-targets --locked --offline -- -D warnings
  SQLX_OFFLINE=true cargo +1.99.1 check -p clacks-desktop --all-targets --target x86_64-pc-windows-msvc --locked --offline
  SQLX_OFFLINE=true RUSTDOCFLAGS='-D warnings' cargo +1.99.1 doc -p clacks-client-store -p clacks-desktop --no-deps --document-private-items --locked --offline
  git diff --check
  ```

  New dependencies may require normal authorized fetching before offline runs.
  Run the full compiled desktop suite on its supported host: on macOS use the
  README's signed/profile-authorized native test host, copying and executing
  the just-built test executable as documented; raw Cargo execution cannot
  establish protected-storage acceptance there. On Windows use the ordinary
  `SQLX_OFFLINE=true cargo +1.99.1 test -p clacks-desktop --locked --offline`
  path with its native prerequisites. Focused receiver/UI tests that do not
  access protected storage can run directly under their stated filters.
  Native enrollment tests require their documented disposable PostgreSQL roles
  and native credential configuration; failure to provide them is not PASS or
  permission to skip silently. Run the repository-required workspace tests,
  documentation tests and CI checks after the focused checks using their
  existing setup. Regenerate SQLx metadata only if actual checked queries change.
- [ ] **5. Establish rendered and native acceptance.** Follow the local
  inventory and desktop UI gates using an explicitly isolated production launch.
  Inspect Settings menu access, both receiver modes, interface choices/refresh,
  validation/Off behavior, Apply/Cancel/Escape/focus, failed-save versus
  failed-activation feedback, unchanged retry, normal sizing and narrow-window
  scrolling in light/dark appearance. Verify no redundant normal reception
  status or endpoint summary appears, while reception failures have a separate
  message that clears after successful activation or an applied disable.
  Confirm packet reception remains active
  independently of redraw, error feedback reopens the same dialog, and the
  unfinished manual contact is preserved. Record platform/states/screenshots.
  Native Windows must establish multicast sharing and same-computer delivery;
  macOS/Linux evidence or cross-compilation cannot substitute. If a required
  runtime/platform condition is unavailable, identify that acceptance gap and
  leave it unresolved rather than adding unrelated tests or claiming completion.
- [x] **6. Update the owning documents to the final implementation.** Add
  WS Suite to the desktop UI's Settings inventory; document both reception modes,
  one capturing client per source, workspace-local persistence, truthful
  checkbox/unapplied-change behavior, reception failure feedback, restart
  behavior and pairing rules in the
  desktop README/integration contract. Add the store preference API to its
  README. Replace contradictory wording rather than append alternatives;
  do not put this machine's provider/ports/secrets or review history in product
  contracts. Verify local documentation links and the final diff.
- [x] **7. Review the whole delivered workflow and report evidence.** Account
  for the accepted spec's sections through the coverage map below, inspect all
  added/changed tests separately, and resolve findings before claiming delivery.
  Preserve approved behavior rather than adding recovery guarantees to pass
  tests. Distinguish automated, native-verified and unavailable evidence. Keep
  #19 open until the actual capability and its required evidence are delivered;
  plan/spec approval alone does not authorize closing it or merging a PR.

## Spec coverage and self-review

| Approved spec | Implemented/verified by |
| --- | --- |
| §§1–3 outcome, authority, source findings and selected facilities | Global constraints; Tasks 1–2 direct consumers and fixed wire expectations |
| §4 UI, persistence, unicast/multicast, Apply and source/workspace lifetime | Task 1; Task 2 steps 9–10; Task 3 native/socket acceptance |
| §5 schemas, encoding, exact identity, 100 ms and latest-only recognition | Task 2 steps 1–2, 6–10 |
| §6 supplied/missing/empty station values, membership, power and source separation | Task 2 steps 3–5 and actual store/read scenario |
| §7 existing save/sync, failures, cancellation and drain | Task 2 steps 8–10; Task 3 authenticated dissemination |
| §8 telemetry, UI evidence, isolated tests and delivery limits | Task 2 instrumentation; Task 3 complete acceptance |
| §§9–10 bounded builder discretion and definition-driven contest lookup | Shared interfaces; Task 2 condition/mapping tests |

Self-review checks: task signatures use the types defined above; the runtime
created in Task 1 is the Task 2 production consumer; all five Review Focus
conditions have named scenarios; every spec section has a delivery/acceptance
owner. There are no additional product decisions or new persistent capture
mechanisms in this plan. All execution/acceptance checkboxes remain unchecked.

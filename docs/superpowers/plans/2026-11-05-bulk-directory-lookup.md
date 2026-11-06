# Bulk Directory Lookup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver Edit → Look up directory information for the ordinary history
selection, with Select all covering matching contacts across pages and a useful
partial-results report.

**Architecture:** Extend existing native-menu dispatch, contact selection,
workspace directory tasks and guarded snapshot application. Keep the provider
client, transient cache, snapshot arbitration and synchronization protocol.
One sequential bulk consumer publishes compact progress and one final report.

**Tech Stack:** Rust 1.99.1, locked egui/eframe/egui_kittest 0.37.0, muda 0.21.2,
Tokio 1.54.1, tokio-util 0.7.16, existing SQLx/SQLite and tracing/metrics.

**Spec:** [Approved bulk-directory specification](../specs/2026-11-05-bulk-directory-lookup-design.md).

## Global constraints

- “Refresh” means replacing a contact's stored directory snapshot.
- Reuse existing lookup and in-process cache behavior: 128 successes, 24 hours
  from completion, original provider/time/identity, ordinary in-flight sharing.
- One command: **Edit → Look up directory information**. No lookup toolbar
  button, scope submenu or separate all-matching lookup action.
- **Edit → Select all** adds matches across pages to ordinary selection,
  preserves hidden selected contacts, and respects text-editing focus.
- Recorded QSO facts remain untouched. Apply whole snapshots through current-key
  checks and latest-successful arbitration; failures/not-found preserve them.
- Cancellation retains acknowledged commits and never aborts a shared lookup
  or races an admitted database write. Provider waits never hold the serial
  settings/contact worker.
- No new dependencies, tables, migrations, protocol messages, persistent jobs,
  batch cache, automatic retries, provider fallback or result merging.
- Use temporary test workspaces and production operations; no test-specific SQL,
  failpoints, secret-store prompts or live provider accounts.
- Before implementation, establish isolation under the user-owned worktree
  placement policy. Preserve these uncommitted planning files when transferring
  work to that checkout. Planning itself requires no builds or app launch.
- Every eventual commit uses a prepared message file, a substantive body, and
  inspected 72-column body wrapping under the user's working agreements. This
  plan does not itself perform commits, pushes or implementation.

## Review focus

- A validated but not yet displayed filter must not replace the applied filter
  used by Select all. Pin this in Task 3's selection test.
- Native Select all in a text field or Worked Before must not select history.
  Pin text replacement and table focus in Task 3's menu test.
- A queued native command after a logbook switch must not use the old selection.
  Pin reconciliation before dispatch in Task 4's menu test.
- A response rejected for replaced credentials must not stop usable current
  material. Extend the real recovery scenario in Tasks 2 and 4.
- A metadata-only committed replacement is unchanged in the report, but still
  invalidates history and synchronizes. Pin both boundaries in Tasks 1 and 4.

## Files and ownership

Paths below are relative to the repository root.

| Files | Responsibility |
| --- | --- |
| `lib/clacks-client-store/src/directory/snapshots.rs`, `src/save/contacts.rs` | Transaction-local application classification; existing atomic save integration |
| `lib/clacks-client-store/tests/directory_snapshots.rs` | Extend actual save/reload/key/deletion scenarios in the existing integration suite |
| `lib/clacks-desktop/src/directory/lookup/failure.rs` (new), `lookup.rs`, `lookup/runtime.rs` | Safe typed lookup reasons and current admission state |
| `lib/clacks-desktop/src/directory/lookup/persistence.rs` | Preserve late-result notifications for every committed snapshot |
| `lib/clacks-desktop/src/directory/lookup/bulk.rs` and `bulk/report.rs` (new) | Admission, capture, sequential execution, cancellation, compact progress and terminal accounting |
| `lib/clacks-desktop/src/session/settings/contacts/selection.rs` (new), `contacts.rs`, `settings.rs` | Cancellable Select all enumeration and prompt bulk-run admission |
| `lib/clacks-desktop/src/menus.rs`, `menus/native.rs`, `ui/menu_commands.rs`, `ui/menus.rs`, `ui.rs`, `application.rs` | Native Edit commands, accelerator/focus routing, lifecycle admission |
| `lib/clacks-desktop/src/ui/contacts/selection.rs`, `contacts/bulk.rs` (new), `contacts.rs`, `contacts/responses.rs`, `contacts/membership.rs` | Selection preparation, action gating, progress and retained report |
| `lib/clacks-desktop/src/ui/contact_table.rs`, `ui/workbench.rs` | Actual history focus ownership; forwarding to the existing selection owner |
| `lib/clacks-desktop/src/ui/logging/directory.rs` and other actual `LookupState::Failed` consumers found by search | Preserve existing entry/editor/capture presentation after typed failure integration |
| Existing lookup, session, menu, contacts and telemetry test module lists; new child files named below | Component and real-workflow evidence at their owning boundary |
| `docs/history.md`, `docs/desktop-ui.md`, `references/callsign-directory.md`, both affected crate READMEs | Delivered user behavior and API obligations |

Keep new modules focused; do not restructure unrelated large modules. Update
actual call sites exhaustively when changing enums. Do not add facade re-exports
or test-only production accessors.

## Task 1: Report snapshot changes from their existing transaction

**Files:** Modify client-store `src/directory/snapshots.rs`, `src/save/contacts.rs`,
`tests/directory_snapshots.rs`; desktop `src/directory/lookup/persistence.rs`.

**Interfaces:** Keep `snapshots::apply(&ClientStore, QsoId, DirectorySnapshot)
-> anyhow::Result<SnapshotOutcome>` and its transaction-internal counterpart.
Replace `Applied` with `Applied { change: SnapshotChange }`, where
`SnapshotChange` is `New`, `Updated` or `MetadataOnly`. Keep `Unchanged` for an
equal/newer retained winner. Replace `Ineligible` with
`Ineligible { reason: SnapshotIneligible }`, whose reasons are
`ParentUnavailable` and `LookupKeyChanged`. All types remain in `snapshots`.

- [ ] Extend `directory_snapshots_survive_restart_and_key_changes` and add
  `directory_snapshot_outcomes_distinguish_material_changes` to the existing
  integration module. Pin these assertions through real operations:

  ```rust
  assert_eq!(new_outcome, SnapshotOutcome::Applied { change: SnapshotChange::New });
  assert_eq!(identical_newer_outcome, SnapshotOutcome::Applied { change: SnapshotChange::MetadataOnly });
  assert_eq!(older_different_outcome, SnapshotOutcome::Unchanged);
  assert_eq!(changed_outcome, SnapshotOutcome::Applied { change: SnapshotChange::Updated });
  assert_eq!(reloaded_qso, original_qso);
  assert_eq!(reloaded_snapshot, winning_snapshot);
  ```

  Retain existing absent-field replacement, changed-key and deleted-parent
  assertions; distinguish their new ineligible reasons rather than duplicating
  the entire arbitration suite. Provider changes also count as material changes.
- [ ] Run `cargo +1.99.1 test -p clacks-client-store --test integration
  --locked --offline directory_snapshot`. Expect the new contract assertions to
  fail before the enum/implementation changes, then implement them.
- [ ] Before calling `DirectoryRecord::record`, compare its snapshot for the
  candidate key with the candidate's provider and complete facts. Keep that
  comparison inside `apply_in`'s existing transaction. Return a change only
  after `save_in` acknowledges a save; keep the original time/ID arbitration.
- [ ] Update `save::contacts::save` and desktop late application to match every
  `Applied { .. }`, including metadata-only changes, for existing committed-save
  reporting and synchronization/history notifications. Preserve their existing
  entry/capture behavior. Do not add an extra before/after read or SQL query.
- [ ] Rerun the integration filter and
  `cargo +1.99.1 test -p clacks-desktop --lib --locked --offline saved_results::`.
  Expect all selected tests to pass. Review the new test assertions against the
  Clacks outcome-mapping mistake each detects. Commit this coherent change only
  within the authorized implementation workflow, using a message file.

## Task 2: Preserve actionable failure categories through shared lookup

**Files:** Create `src/directory/lookup/failure.rs`; modify desktop `lookup.rs`,
`lookup/runtime.rs`, the real `LookupState::Failed` consumers, and existing
`lookup/tests/recovery.rs`/`providers/tests.rs` where assertions belong.

**Interfaces:** Change `LookupState::Failed(String)` to
`Failed(failure::LookupFailure)`. Define a cloneable, secret-safe
`LookupFailure { kind: LookupFailureKind, message: String }` with Display
preserving existing safe user messages. Kinds: `Transport`, `Parse`,
`Subscription`, `Refusal`, `InvalidResponse`, `InvalidFacts`, `RateLimited`,
`Configuration`, `Internal`. No raw provider source or payload is retained here.

- [ ] Extend the existing real recovery cases to assert that 429 remains typed
  `RateLimited`, rejected credentials remain `PausedAuthentication`, and a late
  rejection of replaced material leaves the existing authentication recovery
  check usable. Use the
  current provider fixture server, not a fake lookup service. Preserve original
  request-count/no-automatic-retry assertions in those same scenarios.
- [ ] Run `cargo +1.99.1 test -p clacks-desktop --lib --locked --offline recovery::`;
  confirm the missing classification API fails, then implement it.
- [ ] Map existing `ProviderError`/`SafeProviderFailure` variants directly to
  typed reasons before display conversion. Use `InvalidFacts` at the existing
  native-facts construction failure. Use the safe typed category in the existing
  diagnostic failure branch and preserve safe Display text for ordinary callers.
  Do not parse strings or add bulk admission APIs before their Task 4 consumer.
- [ ] Keep request ordering unchanged: cached success, in-flight join, then
  current network-admission checks. Keep `Cleared`, authentication pause and
  normal failure UI behavior unchanged; ordinary consumers use safe Display.
  Check every `LookupState` match, including capture and component test hosts.
- [ ] Rerun `recovery::`, `sharing::` and `providers::tests::` desktop test filters
  with the same command prefix. Expect pass without new requests for a cache
  hit or a quiet-period gate. No new metric family belongs to this representation
  change; preserve current safe classifications. Commit using a checked message
  file under the authorized implementation workflow.

## Task 3: Add native Select all over the actual history selection

**Files:** Create desktop `src/session/settings/contacts/selection.rs`,
`src/ui/contacts/selection.rs`, and `src/ui/contacts/tests/selection.rs`; modify
`contacts.rs`, `settings.rs`, `ui/contacts.rs`, `ui/contacts/query.rs`,
`ui/contacts/membership.rs`, `ui/contact_table.rs`, `ui/workbench.rs`, `menus.rs`,
`menus/native.rs`, `ui/menu_commands.rs`, `ui/menus.rs`, `ui.rs`, `application.rs`,
`ui/tests/menu_commands.rs`, and existing test module lists.

**Interfaces:** Add `ContactAction::SelectAll { query: ContactPageQuery }` and
`ContactResponse::SelectedAll(BTreeSet<QsoId>)`. Implement
`selection::matching_ids(&ClientStore, ContactPageQuery)
-> anyhow::Result<BTreeSet<QsoId>>`. Add ContactsUi methods
`select_all(&mut self, &mpsc::Sender<SettingsRequest>)`,
`selection_pending(&self) -> bool`, `selected_ids(&self) -> &BTreeSet<QsoId>`,
`history_has_focus(&self, &egui::Context) -> bool` and
`poll_selection(&mut self)`. Workbench forwards these without exposing ContactsUi.

- [ ] Add a real-store `select_all_reads_every_applied_match` session/contact
  case: 205 matching contacts across three 100-row pages, two already-selected
  nonmatches, another tag/view and a pending deletion. Assert the worker returns
  exactly the 205 matching live IDs, and the UI installs their union with the
  previous two selected nonmatches: 207 selected, two outside the filter.
  Use existing Rust fixtures/production saves. Assert no lookup dispatch.
- [ ] Add `select_all_uses_applied_filter_and_rejects_obsolete_results` component
  assertions: edited/desired text differs from applied text, Show selected is
  active, only one page is loaded; the captured query still uses the applied
  expression, `show_selected == false`, offset zero. A view/filter/selection
  change before the actual reply cannot install it. Failure or receiver closure
  preserves the preceding complete selection. No partially enumerated set is
  exposed, and membership submission is disabled during preparation.
- [ ] Add `edit_select_all_respects_text_and_history_focus` to menu tests. With
  actual egui text focus, dispatch Select all and type a replacement: assert
  only the focused text changes and no contact request occurs. With an actual
  history row focused, assert one SelectAll request. Worked Before focus must
  not start history selection. Run the `select_all` desktop filter and confirm
  these contracts fail before implementation.
- [ ] Implement matching enumeration with one existing `ReadSnapshot`, existing
  `contact_page`, page size 100, default stable order and checked offset
  advancement. Clear query `selected_ids`, force `show_selected = false`, and
  release the snapshot before returning. Keep the action cancellable when its
  reply receiver closes by extending the existing contact-query cancellation
  branch. Extend its bounded query-kind label to `history_selection`, and report
  the real returned ID count through existing query diagnostics.
- [ ] Capture the applied expression from `HistoryQueryUi.state.applied` using
  the existing query parser/catalog, not `desired` or `desired_text`. Store the
  captured view/filter and original selected set beside the dedicated reply
  receiver in the selection module. Compare them before installation, discard
  on newer selection intent, and union returned IDs only on success. Show
  **Selecting contacts…** and disable selection consumers while pending.
- [ ] Add `MenuCommand::SelectAll` (`select_all`, **Select all**) in a native
  **Edit** group before Settings. Add the platform command/control-A accelerator
  through muda's ordinary MenuItem facilities. Use an application command,
  because muda's predefined SelectAll sends an AppKit responder selector on
  macOS and synthesized control-A on Windows rather than selecting Clacks IDs.
- [ ] Route focused text through egui's existing command-A handling: recognize
  the focused `TextEditState`, capture that widget ID, and inject one command-A
  key press/release via eframe's documented `raw_input_hook` on the next input.
  Discard the pending text command if focus changed. Do not manipulate text
  buffers, cursor offsets or operating-system input directly. Let ordinary
  keyboard command-A reach TextEdit; intercept it for history only when history
  owns focus, avoiding duplicate native/egui dispatch. Expose the current row's
  egui focus owner through `TableAction` and retain it in ContactsUi; do not
  equate any keyboard focus or Worked Before selection with history focus.
- [ ] Pass `&egui::Context` into `menu_enabled`, `update_menu_state` and
  `install_menus` and update their actual callers/tests, including application
  composition. Admit Select all for focused text before contact-modal checks,
  but after lifecycle checks. Use existing workbench/modal rules for history.
  No new focus manager or generic command routing layer is needed.
- [ ] Rerun `select_all`, `ui::tests::menu_commands::`, `ui::contacts::tests::`,
  and `ui::contact_table::tests::` desktop filters. Expect pass with actual
  requests and semantic input assertions. Update history selection and native
  Edit-menu documentation in their owning contracts. Commit the usable selection
  workflow with a checked message file.

## Task 4: Deliver the selection-based bulk consumer and retained report

**Files:** Create desktop `src/directory/lookup/bulk.rs`, `bulk/report.rs`,
`src/directory/lookup/tests/bulk.rs`, `src/ui/contacts/bulk.rs`,
`src/ui/contacts/tests/bulk.rs`, `src/telemetry/tests/bulk_directory.rs`.
Modify `lookup.rs`, contact action/response modules, selection gating, Workbench,
native menu definitions/dispatch, existing test module lists, and owning usage
documents. Extend `session/tests.rs`, `session/tests/settings.rs` and the existing
lookup test support for actual session-worker responsiveness.

**Interfaces:** Add `ContactAction::StartDirectoryLookup { ids: BTreeSet<QsoId> }`
and `ContactResponse::DirectoryLookupStarted(BulkLookupHandle)`. Implement
`LookupClient::start_bulk(&self, ids: BTreeSet<QsoId>,
changes: watch::Sender<()>) -> anyhow::Result<BulkLookupHandle>` in `bulk.rs`.
The handle owns `cancel: CancellationToken` and
`updates: watch::Receiver<BulkUpdate>`; it never owns provider request lifetime.

Add `LookupClient::admission_block(&self) -> Option<AdmissionBlock>` in
`failure.rs`, with `Closed`, `NotConfigured`, `Configuration { message: String }`,
`Authentication`, `RateLimited { remaining: Duration }` and `Internal` variants.
Derive these from current `closed`, `selected`, `paused` and monotonic
`retry_after` state under the existing lock, not a duplicate recovery state.
Extend LookupFailure with `origin: Option<LookupOrigin>`, where
`LookupOrigin { provider: DirectoryProvider, material_id: DirectoryMaterialId,
material_revision: i64 }` copies the existing captured nonsecret request
configuration identity. Add
`LookupClient::is_current_origin(&self, &LookupOrigin) -> bool` to compare it
with current selection. This context lets the bulk consumer distinguish a
current 429 without a delay from an obsolete-material response. It neither
retains credential bytes nor replaces the existing full-material equality
checks in authentication/provisioning. Add these APIs alongside their real
bulk consumer, avoiding unused scaffolding in Task 2.

Define the following concrete report values in `bulk/report.rs`, with documented
named fields and no serialization or new model schema:

- `BulkContact { id: QsoId, callsign: Option<RecordedCallsign>, start: Option<UtcSecond> }`;
  unavailable parents can lack captions and display **Contact no longer available**
  with its identity available in diagnostic details rather than invented facts.
- `BulkTarget { contact: BulkContact, key: LookupKey }`, retained only by the worker.
- `BulkCounts { new_updated, unchanged, not_found, failed, unprocessed: u64 }`.
- `BulkFailure`: typed lookup failure, rejected authentication, unavailable
  configuration, or safe local-storage failure. Separate `UnprocessedReason`
  variants for unavailable parent, changed key, cancellation and early stop.
- `BulkDetail { contact: BulkContact, outcome: BulkDetailOutcome }`, where
  `BulkDetailOutcome` carries named Failed/Unprocessed reasons only.
- `BulkReport { total: u64, counts: BulkCounts, details: Vec<BulkDetail>, stop: BulkStop }`;
  `BulkStop` distinguishes completed, cancelled, early stop and preparation failure.
- `BulkUpdate`: Preparing with selected total, Running with settled count and
  compact counts, or Finished with `Arc<BulkReport>`. The UI derives Cancelling
  from its token until Finished. No growing detail list in intermediate updates.

- [ ] Add `bulk_lookup_reuses_success_and_reports_real_partial_results` using
  existing lookup Service/provider fixtures and real saved contacts. Prime one
  cached success and share another pending request; include same-key selected
  contacts, a not-found, transport failure and authentication stop. Assert the
  exact five totals, original cached lookup time/ID, no extra cache-hit request,
  unchanged recorded contacts and the expected persisted directory winners.
  Assert every frozen ID has exactly one accounted outcome.
- [ ] Add `bulk_lookup_cancellation_keeps_commits_without_owning_shared_work`:
  cancel after an acknowledged application boundary while another target joins
  an actual delayed lookup also held by an ordinary consumer. Release the
  response, verify that consumer succeeds, and reload to prove no post-cancel
  bulk application. Use progress/change notifications and fixture gates,
  not sleeps, test failpoints or a fake provider engine.
- [ ] Cover early-stop/state races in that same real-workflow suite: current
  authentication/rate/configuration stops; replacement material stays usable
  despite late old-material rejection; parent deleted/key changed while awaiting
  a response; metadata-only writes notify history while counting unchanged.
  Cause a real local-storage refusal by closing the temporary store after an
  earlier commit and before releasing the next provider result; reopen afterward
  to verify earlier durable data. No dropped tables, triggers or test-only SQL.
- [ ] Run the new `bulk_lookup` desktop filter, confirm failing contracts, then
  implement admission/capture/execution. Reject empty/closed admission before
  spawning, register under the existing lookup state lock/closed guard, and
  spawn on its TaskTracker with current subscriber/context. Return the handle
  immediately. Capture all selected records in one read snapshot, in stable ID
  order; release it before any provider waits or writes. Missing parents become
  unprocessed details. Keep original selected total even if capture is cancelled
  or fails; report unknown captions honestly.
- [ ] For each captured target, call ordinary `request(key)`, await only this
  consumer's handle, and apply Found through Task 1's `snapshots::apply`.
  Map New/Updated to new_updated; MetadataOnly and retained winners to unchanged;
  NotFound to not_found; ineligible to its unprocessed reason. Notify existing
  change consumers on every acknowledged Applied result, including MetadataOnly.
  Continue individual failures and stop for shared blocking conditions using
  Task 2's current admission state. A current 429 with no retry delay still
  stops this run, without inventing a lasting runtime gate; obsolete-material
  rate-limit outcomes are recognized through their captured LookupOrigin and
  do not impose a new stop on usable replacement material. Do not
  preflight admission ahead of the ordinary cache/in-flight path.
- [ ] Check run cancellation/workspace stop during capture, before request,
  while awaiting the shared handle, and immediately before a write. Await
  every started `snapshots::apply` to settlement without a select/abort branch.
  Then count its acknowledged effect and stop before the next write. Build
  remaining unprocessed details once and publish one terminal report. A closed
  UI receiver does not abort admitted work; workspace shutdown uses existing
  lookup cancellation/drain before store closure.
- [ ] Extend contact execution to start and acknowledge this tracked work,
  without awaiting preparation/network completion on the settings worker.
  Do not mark StartDirectoryLookup as an immediately committed shared write:
  the worker notifies actual commits. Handle unavailable lookup/session admission
  as a visible failure without dropping selected identities or claiming saves.
- [ ] Add `MenuCommand::LookupDirectory` (`lookup_directory`, **Look up directory
  information**) below Select all with a separator, using existing menu label,
  event and enabled-state facilities. Gate empty/pending selection, an active
  run and normal modal/lifecycle state. Before native dispatch, reconcile the
  latest settings view's selected logbook with ContactsUi, so a queued command
  cannot revive a selection cleared by a view switch. Forward selection to the
  existing contact action path; add no history-toolbar opener.
- [ ] Implement `ContactsUi::start_directory_lookup`, `poll_bulk_lookup` and
  `bulk_feedback` in `ui/contacts/bulk.rs`. Keep its reply, cancel token and watch
  receiver separate from `operation`, which currently implies modal ownership.
  Poll in normal logic as well as rendering to avoid stale native enablement.
  Preserve run/report across history view changes and refreshes. Show Preparing,
  processed/total, Cancel/Cancelling, five totals, and expandable Failed and
  Unprocessed lists with the spec's corrective guidance. Dismiss only settled
  reports; no auto-dismiss, repeated success toast or new retry mechanism.
- [ ] Add `bulk_lookup_menu_uses_current_selection` component/menu assertions:
  empty and pending Select all disable lookup, one accepted command captures
  selected IDs outside loaded/filter rows, repeated queued commands do not start
  a second run, and switching logbook before dispatch clears old selection.
  Add `bulk_lookup_report_survives_view_change` with semantic Cancel/Dismiss
  interactions, a long safe error, exact displayed counts and a native component
  snapshot. Test the composed history layout rather than only the new controls.
- [ ] Extend the real session/settings scenario so a delayed provider lookup
  overlaps an ordinary ContactAction::Save and Page; both settle before the
  provider release. Request shutdown and verify tracked bulk work drains and
  the actual committed prefix survives reopening. Retain the existing lookup
  output receiver in the session test Fixture so it can obtain the session's
  actual LookupClient. Refactor the existing test-only Service configuration
  setup into `lookup::tests::support::configure_client(&LookupClient, url::Url)
  -> anyhow::Result<()>` and reuse it to select the concrete fixture provider
  for that session client. Change test-module visibility only as needed. This
  reuses real adapters/runtime and existing test setup; do not add production
  endpoint overrides, test-only production interfaces or a second service engine.
  A separate shutdown case is needed only if this cannot fit the existing
  session scenario.
- [ ] Add the workflow's info-level run span with a fresh local `ulid::Ulid`
  correlation value and explicit account/target-count fields. Define
  `clacks_directory_bulk_runs_total{outcome}` and
  `clacks_directory_bulk_duration_seconds{outcome}` at run settlement; bounded
  outcomes: completed, cancelled, stopped, preparation_failed. Define
  `clacks_directory_bulk_contacts_total{outcome}` with the five report categories,
  counted once after each classification (remaining unprocessed at settlement).
  Keep IDs, callsigns, expressions, safe messages and secrets out of labels.
  Keep existing provider metrics unchanged for cache/join consumers; emit safe
  stop/error diagnostics at their owning boundary without duplicate span timers.
- [ ] Extend the existing isolated telemetry composition case with one real
  mixed bulk run: compare report totals to exported contact counts and verify
  deliberate exclusion of credential/callsign/provider-body sentinels. Preserve
  useful account/run correlation at ordinary info filtering; do not assert event
  names, span trees, ordering or library formatting. Use existing collectors and
  process-owned subscriber; do not introduce another capture framework.
- [ ] Run desktop filters `bulk_lookup`, `select_all`, `ui::tests::menu_commands::`,
  `directory::lookup::tests::`, plus the affected existing telemetry composition
  case. Run Task 1's client-store integration filter again. Expect all selected
  checks to pass; review changed tests separately for specific Clacks mistakes.
- [ ] Update `docs/history.md`, `docs/desktop-ui.md`, directory research and crate
  usage documents in place: native Edit commands, selection across pages,
  cancellation/report behavior and the now-delivered bulk consumer. Remove
  conflicting claims that bulk lookup is still absent while preserving other
  deferred capabilities. Keep issue #22 open until delivery evidence exists.
- [ ] Run `cargo +1.99.1 fmt --all --check`, affected-crate clippy with
  `--all-targets --locked --offline -- -D warnings`, and
  `cargo +1.99.1 check -p clacks-desktop --all-targets
  --target x86_64-pc-windows-msvc --locked --offline`. Select additional affected
  documentation/API checks if public rustdoc changed. No server schema/query
  changes are planned, so a full server/database suite is not a default gate.
- [ ] Inspect the changed native Edit menu and history report in the production
  app using the user-owned development inventory and explicit isolated paths.
  Verify text Select all, contact Select all, disabled states, Cancel, long report
  rows and continued entry use in the affected composition. Select resizing or
  appearance checks only for observed/changed layout risks. Record actual platform
  evidence; a Windows cross-check does not establish native accelerator behavior.
  Do not provision a new development environment or touch the default workspace.
- [ ] Review the complete diff against the spec, including task ownership,
  cancellation/commit boundaries, cache order, diagnostics and changed tests.
  Commit the delivered workflow with a prepared, inspected message file only
  under the authorized implementation workflow. Report evidence and any relevant
  native validation limit without claiming issue completion from this plan.

## Plan self-review and execution boundary

All specification sections map to the four tasks: transaction accounting (1),
actionable failures/current admission (2), selection/native text semantics (3),
and integrated run/report/lifecycle/observability (4). Task interfaces above are
the implementation defaults; none changes provider/cache behavior or introduces
new persistent state. The new workflow has a production menu consumer in Task 4.

Before executing, reread the spec and current sources; the inspected baseline is
`424158b38d1f8e2119b01d58aed37f34dcaa5a35`. Resolve actual drift rather than assuming
the plan proves current code. This document is a plan, not verification evidence.

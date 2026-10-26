# Offline Manual Logging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:executing-plans for native execution, or
> superpowers:subagent-driven-development if the user chooses delegated execution.
> Implement task by task; checkboxes track verified completion, not intent.

**Goal:** Deliver production offline QSO entry, saved-contact editing and individual
contact deletion in the existing desktop.

**Architecture:** Keep one transient entry owned by the desktop, with shared
native-value forms and an independent saved-contact editor. Execute typed contact
requests through the existing session runtime and commit through the existing
SQLite store. Extend the real reader and table with bounded selected-view pages;
#10 and #51 integrate these components rather than replacing them.

**Tech Stack:** Rust 1.99.1; egui/eframe and egui_extras 0.37.0; wgpu; Tokio;
SQLx 0.9.0/SQLite; existing clacks-model/clacks-sync; tracing and metrics.

**Spec:** [Manual logging](../../manual-logging.md). Read it alongside
[desktop UI](../../desktop-ui.md), [operating settings](../../operating-settings.md),
[logbooks](../../logbooks.md), [client storage](../../client-store.md),
[native model](../../native-model.md) and [observability](../../observability.md).

## Global constraints

- Windows MVP; retain cross-platform architecture and the thin `bin/clacks-log`.
- Use production components. No mock repositories, placeholder results, extra
  runtime, custom write queue, save-generation counters or persisted drafts.
- No Python, xtask, new TQSL execution, production test hooks or SQL-trigger
  failure injection. Tests prove Clacks behavior, not framework behavior.
- Fixed SQL lives in query files with committed SQLx offline metadata. Do not
  add a migration unless an actual schema change is necessary; QSO persistence
  and local preference storage already exist.
- Use native validation and value preservation; no new saved-contact schema,
  automatic field merge, telemetry abstraction or generic form framework.
- Rust module files use named paths, not `mod.rs` outside tests. Keep modules
  focused, review growing files before they approach 600 lines, use named fields
  and standalone rustdoc, and avoid production indexing, `unwrap` and `expect`.
- Add dependencies at workspace level and inherit them. Use the existing `time`
  version with parsing/formatting features for UTC input, not hand-written
  calendar arithmetic. Add egui_extras matching eframe's exact version.
- Preserve unrelated `references/desktop-toolkit.md` changes. Existing uncommitted
  manual-logging notes/specification belong with this work; inspect their diff.
- Before each commit/amend, prepare a substantive message file, hard-wrap prose
  at 72 columns and inspect physical line lengths. Commit coherent capabilities;
  fold corrective work into its owning commit before publication.
- Do not push or create a PR until authorized. Keep GitHub delivery status open
  until implementation and verification are delivered.

## Review focus

1. An imported two-character locator or unknown mode/reference survives an
   unrelated contact edit: cover in Task 1 and durable editing in Task 5.
2. Active settings/view change while a save is pending: submitted facts stay
   fixed; success uses current defaults, failure preserves input (Tasks 1/4).
3. Equal times on different dates and equal sort keys: full timestamps and
   stable IDs determine ordering, including null optional columns (Task 2).
4. A save succeeded but its response/view refresh did not: reconcile by submitted
   ID and facts, never silently create another contact (Tasks 3/4).
5. A contact changes or is deleted while its editor/confirmation is open:
   preserve input and report stale state, never overwrite or resurrect (Task 5).

## File and responsibility map

New modules below are private or `pub(crate)` except real store query APIs.
Names/signatures below fix task handoffs; small local helper choices remain with
the implementer. Do not expose form internals as public library APIs.

| Files | Responsibility |
| --- | --- |
| `lib/clacks-desktop/src/logging.rs`, `logging/form.rs`, `logging/time.rs`, `logging/radio.rs`, `logging/defaults.rs` | Transient native-value input, timestamp locks, radio-pair edits and explicit operating corrections. |
| `lib/clacks-desktop/src/operating/{station,position}.rs` | Reuse station/reference/position conversion with explicit recorded versus own-entry rules. |
| `lib/clacks-client-store/src/read/contacts.rs`, `sql/read/contacts.sql` | Real bounded list queries and typed ordering; share SQL predicates with existing history reading. |
| `lib/clacks-desktop/src/session/settings/contacts.rs` | Typed contact requests and actual durable outcomes on the existing runtime. |
| `lib/clacks-desktop/src/ui/logging.rs`, `ui/logging/{entry,details,timestamps,responses}.rs` | Entry ownership, shared panes, keyboard submission and pending-save presentation. |
| `lib/clacks-desktop/src/ui/contacts.rs`, `ui/contacts/{table,editor,deletion,responses,preferences}.rs` | Real saved-contact table, independent editor, confirmation, request results and presentation preferences. |
| `lib/clacks-desktop/src/ui/workbench.rs` | Two upper panes/full-width lower list, local split sizes and reset. |
| Existing `ui.rs`, `session/settings.rs`, `ui/{fields,operating,preferences}.rs` and manifests | Composition and narrowly scoped shared-component reuse. |
| Tests named in each task | Requirements at their actual form, runtime, store and UI boundaries. |

## Task 1: Native contact forms, clocks and operating defaults

**Files:** Create the `logging` modules above and `logging/tests.rs`; modify
`lib/clacks-desktop/src/lib.rs`, `operating/{station,position}.rs`, workspace
`Cargo.toml` and `lib/clacks-desktop/Cargo.toml`.

**Interfaces (all desktop-internal):**

- `ContactForm::from_qso(qso: &Qso) -> Self` and
  `build(&self, id: QsoId, definitions: &DefinitionSet) -> Result<Qso, Vec<FieldError>>`.
  Named fields cover the complete native QSO, including unchanged tags and
  preserved mode identity. Reuse `StationForm`, `EquipmentForm`, position and
  watt parsing instead of copying their conversion logic.
- `ContactTimestamp` holds complete UTC text and live/locked state.
  `tick(&mut self, now: UtcSecond)`, `lock(&mut self)`,
  `unlock(&mut self, now: UtcSecond)` and
  `value(&self) -> Result<Option<UtcSecond>, FieldError>` serve real controls.
- `NewEntry::new(active: &OperatingState, view: SelectedView, now: UtcSecond) -> Self`,
  `refresh_defaults(&mut self, active: &OperatingState, view: SelectedView)`,
  `submit(&mut self, now: UtcSecond, definitions: &DefinitionSet) -> Result<Qso, Vec<FieldError>>`,
  and `reset_after_save(&mut self, active: &OperatingState, view: SelectedView, now: UtcSecond)`.
  Keep stable submission ID separately from the next unfinished contact.

- [ ] Write `complete_contact_form_builds_native_facts` and
  `independent_clocks_preserve_unknown_and_overnight_end` in `logging/tests.rs`.
  Cover building the complete native value; optional independent Name/operator call; band-only
  primary; receive absence/band-only; `.5` W; equal and overnight timestamps;
  live/locked Start/End and cleared locked End. Test that clearing/editing one
  radio pair applies its own frequency/band rules without affecting the other.
- [ ] Add `unrelated_edit_preserves_unrecognized_and_coarse_values`: loaded
  unfamiliar mode parent/submode identity, unknown references
  and two-character recorded locators surviving a comment-only edit. Test that
  explicit new own-location entry still requires four characters and that
  contacted-station entry accepts native coarse areas. These test form policy,
  not the native parser's entire catalog or geographic conversion accuracy.
- [ ] Add `active_defaults_preserve_explicit_corrections_and_clears` and
  `view_switch_replaces_only_proposed_membership`: active power 100 W → 50 W, explicit
  contact-specific corrections and clears, and view Main → another tag →
  Untagged. Only membership follows view changes. Position evidence is one
  correction unit; ordinary independent recorded geography fields remain
  independently editable. Explicit overrides remain overrides even when an
  active default later happens to equal them.
- [ ] Run `cargo test -p clacks-desktop logging::tests --locked --offline` to
  establish missing behavior before implementation.
- [ ] Implement the forms and named correction flags. Reuse the already chosen
  native representations; do not encode directory data in recorded facts.
  Preserve untouched original values where a UI reparse would lose information.
  Add an explicit position-entry policy to the existing form conversion so
  operating editors keep their current validation while recorded-contact
  editing preserves native values. Reuse reference normalization only on explicit
  reference edits. Keep optional text normalization in the existing shared logic.
- [ ] Parse/display UTC as `YYYY-MM-DD HH:MM:SS` with the `time` crate and convert
  through checked whole-second native constructors. Pass actual UTC to form
  operations from the UI; this is not a test clock service. Static saved-editor
  values never tick. Use one captured UTC instant for both live fields at Save.
- [ ] Run the new tests and existing operating tests; all must pass without
  relaxing the four-character own-location rule or losing imported coarse data.
- [ ] Commit the native form/default behavior with its tests and dependency edits.

## Task 2: Selected-view contact pages and real table ordering

**Files:** Create `lib/clacks-client-store/src/read/contacts.rs`,
`lib/clacks-client-store/sql/read/contacts.sql` and
`lib/clacks-client-store/tests/contact_pages.rs`; modify `read.rs`,
`read/history.rs`, the existing `sql/read/history.sql` as needed and `.sqlx/`.

**Interfaces:**

- Public `ContactPageQuery { view: SelectedView, order: ContactOrder,
  offset: u32, limit: u32 }`; `ContactOrder { column: ContactColumn,
  direction: SortDirection }`. Columns: contacted call, start, band, mode,
  frequency, sent report, received report, DX grid and pending/review status.
- `ReadSnapshot::contact_page(&mut self, query: &ContactPageQuery) -> anyhow::Result<ContactPage>`;
  `ContactPage { rows: Vec<HistoryRow>, has_more: bool }`. Default is start
  descending, with QSO ID as deterministic final tie-breaker.
- Retain existing `ReadSnapshot::history` filter/keyset behavior. Share native
  working-row decoding and SQL selection predicates; no parallel mock reader.

- [ ] Write `selected_views_show_each_working_contact_once` in
  `tests/contact_pages.rs`: All contacts includes
  tagged and untagged; Untagged excludes any tag; named view returns a multi-tag
  QSO once. A pending edited value is shown and a deleted QSO is absent.
- [ ] Add `contact_pages_sort_complete_values_and_break_ties_by_id`: cases
  across dates whose time-of-day order is reversed, equal
  starts with distinct IDs, band-only/null frequency and unfamiliar labels.
  Check each delivered sort column uses recorded/native values, not display text
  for numeric/time values. Optional absent values sort last in both directions;
  ID ascending breaks ties. Page boundaries must not repeat/omit tied rows in
  one snapshot. Validate positive bounded page size rather than unchecked casts.
- [ ] Run `cargo test -p clacks-client-store --test contact_pages --locked --offline`
  to establish the missing reader behavior.
- [ ] Implement fixed, bound SQL with an allowlisted column/direction enum and
  relational ordering. Use an extra row to determine `has_more`; never fetch all
  contacts to sort them in Rust. Reuse existing working-value projections; add
  narrowly needed relational joins/projection fields, not serialized QSO blobs.
  Preserve the existing history keyset contract through the shared query path.
- [ ] Use offset paging for this UI query, default page size 100. Release each
  snapshot after its bounded page read. Local mutations, changed view or sorting
  invalidate the page and return to offset zero; do not imply that independent
  reads during concurrent mutations form one immutable multi-page snapshot.
  The existing keyset history API remains available for snapshot consumers.
- [ ] Regenerate offline metadata against an actual disposable migrated SQLite
  database using the workflow's pinned SQLx command. Run the new tests plus
  `cargo test -p clacks-client-store --test reads --locked --offline`.
- [ ] Commit the production reader and query metadata together.

## Task 3: Contact commands on the existing runtime

**Files:** Create `session/settings/contacts.rs` and its `tests.rs`; modify
`session/settings.rs` and its existing tests. Extend actual store operations only
where a concrete missing transaction guarantee is demonstrated.

**Interfaces:**

- Add `SettingsAction::Contacts(ContactAction)` and corresponding typed responses.
- `ContactAction::{Page { query: ContactPageQuery }, Load { id: QsoId },
  Save { expected: Option<Box<ClientRecord>>, next: Qso },
  Delete { expected: Box<ClientRecord> }}`.
- `ContactResponse::{Page(ContactPage), Loaded(Option<ClientRecord>), Saved(SaveOutcome),
  Deleted(SaveOutcome)}`; retain existing `SettingsResponse::RefreshFailed`,
  `Failed` and `Closing` handling. `execute(store: &ClientStore,
  action: ContactAction) -> anyhow::Result<ContactResponse>` owns dispatch.

- [ ] Add `contact_commands_preserve_guarded_local_working_state` for real-store
  create, edit, unchanged save and deletion; stale expected bases preserve current records. Verify the produced pending
  metadata and stored native facts, not SQLx transaction implementation details.
- [ ] Add `repeated_create_acknowledges_only_the_same_submitted_contact` for
  resubmission of one stable create ID after a lost acknowledgment:
  the same existing working QSO is acknowledged without another record/edit;
  different current facts or a deleted current record produce a stale outcome.
  Equal callsign/time facts with a different new QSO ID remain permitted.
- [ ] Cover `RefreshFailed` response handling at the existing response-consumer
  boundary with an actual committed outcome. Do not make a database read fail
  through an injected trigger or add a production fault flag.
- [ ] Run `cargo test -p clacks-desktop session::settings --locked --offline`
  before implementing the new command variants.
- [ ] Execute Load through `save::load`, Page through Task 2, Save through
  `save::save` and Delete through `RecordState::Deleted` with the expected basis.
  Construct keys from the opened account and native ID. Inspect create retries
  by stable ID plus complete submitted facts, never a callsign/time heuristic.
- [ ] Preserve the existing admission, shutdown drain and committed-result versus
  settings-refresh handling. Contact replies carry their own page/record; do not
  put the full account's contacts into `SettingsSnapshot` or the settings watch.
- [ ] Add focused domain observability at this execution boundary: meaningful
  info-level committed create/edit/delete outcomes and bounded outcome metrics,
  stale outcomes with safe correlation, debug list reads, handling-boundary
  errors. Reuse the store's actual commit reporting; do not double-count it or
  dump form values. Measure workflow duration here if a metric needs it; let
  tracing own span lifecycle.
- [ ] Run the command tests and existing shutdown/session tests; commit the
  runtime integration with its own reporting and tests.

## Task 4: Production quick entry and adjacent details

**Files:** Create `ui/logging.rs`, its planned child modules and
`ui/logging/tests.rs`, plus `ui/workbench.rs`; modify `ui.rs`,
`ui/operating/home.rs`, `ui/operating/fields.rs`, `ui/fields.rs` and presentation
preferences only where reuse/composition needs it.

**Interfaces:**

- `LoggingUi` owns `NewEntry`, selected detail tab, validation/errors, exact
  submitted QSO and pending response receiver. Expose `refresh(&mut self,
  snapshot: &SettingsSnapshot)` and `show(&mut self, ui: &mut egui::Ui,
  requests: &mpsc::Sender<SettingsRequest>)` to composition.
- `ContactForm` is shared by entry details and the saved editor. Reuse or move
  actual station/operator/equipment widgets to a common `ui` owner where needed;
  no second copy with subtly different validation or field omissions.
- Workbench composition owns split sizing using eframe presentation storage,
  not SQLite account facts; reset restores the agreed two-upper/one-lower layout.

- [ ] Add `detail_tabs_share_the_quick_entry_contact` and
  `single_line_enter_saves_without_stealing_other_control_input` for one draft
  across Contact details and Our settings; quick Call/Name placement; one combined Start and End control;
  no duplicated timestamps or expandable detail controls. Drive actual input
  events for Enter versus multiline Enter/dropdown consumption, Tab, and locks.
- [ ] Add `pending_submission_preserves_its_facts_and_resets_from_current_defaults`
  and `failed_save_preserves_input_and_committed_refresh_failure_resets_once`:
  settings/view updates do not change the pending submitted contact; success resets using the latest defaults/view, retains radio/mode and clears
  exactly the agreed DX/report/comment values. Failure preserves the submitted
  input and does not repeat submission. A successful commit plus refresh failure
  resets once and offers refresh rather than saving a second contact.
- [ ] Run `cargo test -p clacks-desktop ui::logging --locked --offline` before
  implementing presentation and response transitions.
- [ ] Compose the actual fields in the spec's compact row order, Name adjacent
  to Callsign, using the shared fonts/theme, changed/invalid/focus treatment and
  `ValidationState`. Radio values use the radio typeface. Additional tabs expose
  all remaining native values directly. Do not add a Worked Before placeholder.
- [ ] Show complete `YYYY-MM-DD HH:MM:SS` UTC controls with independent locks.
  Schedule repaint at the next whole-second boundary while a field is live;
  no busy redraw loop. Preserve typed text while locked. Lock then clearing End
  keeps it unknown. Convert through Task 1 on validation and actual submission.
- [ ] Route Save button and single-line Enter to one submission method. Honor
  consumed widget events and unrelated dialogs; don't register an unconditional
  global Enter save. Revalidate current text, then retain the exact submitted
  QSO, make entry read-only and display Saving while its request is pending.
- [ ] Preserve normal active-summary semantics and responsiveness. After success,
  focus Callsign and show quiet local feedback. A failed response restores the
  same form and a visible actionable error. Keep a stable submitted ID until a
  definitely settled create outcome; do not allocate a second ID on retry.
- [ ] Render-check actual layout in both appearances, increased UI scale and a
  smaller usable window. Compare with `docs/images/desktop-workbench-reference.jpg`
  and `docs/images/desktop-workbench-light-reference.jpg`
  and the agreed field grouping. Check palette, controls, typography and compact
  operating strip, not just labels. Keep screenshot artifacts outside the repo
  unless a deliberate maintained reference is needed.
- [ ] Run the new UI tests plus existing operating/logbook/layout tests; commit
  the functional entry work and its own visual integration.

## Task 5: Real contact table, saved editing and deletion

**Files:** Create `ui/contacts.rs` and its planned child modules;
`ui/contacts/tests.rs`, `ui/contacts/editor_tests.rs` and
`ui/contacts/deletion_tests.rs`. Modify `ui.rs`, `ui/workbench.rs`, manifests,
lockfile, `ui/logbooks/responses.rs` and `ui/operating/responses.rs` only for real
view invalidation/default propagation. Extend existing store integration tests
where needed rather than duplicating a round-trip suite.

**Interfaces:**

- `ContactsUi` owns the current `ContactPageQuery`, returned page, selected QSO
  IDs, pending request and independent optional editor. `refresh(&mut self,
  view: SelectedView)` invalidates view-dependent data; `show(&mut self,
  ui: &mut egui::Ui, requests: &mpsc::Sender<SettingsRequest>)` renders the list.
- `ContactEditor { expected: ClientRecord, form: ContactForm, ... }` owns loaded
  native facts, validation and pending response. Deletion confirmation retains
  its expected record until response. Internal response methods use Task 3's
  typed results, not anonymous tuples of row/payload/version data.
- `ContactTablePreferences` stores column IDs/order/width/visibility and sort;
  serialize through workspace-local eframe storage, separate from appearance,
  pane sizing and the SQLite selected view. Never persist selected IDs, draft
  values or saved-editor input as preferences.

- [ ] Write real-store/UI tests: create through the runtime, read selected-view
  rows, open by stable ID, edit through `ContactForm` and reopen the workspace.
  Check changed facts and the preserved unknown/coarse facts from Task 1.
  Cancel/close/Escape and successful Save changes leave new entry and active
  operating settings unchanged. An unchanged save fabricates no pending edit.
- [ ] Add `stale_editor_cannot_overwrite_or_resurrect_a_contact`: make a second
  real edit or deletion before Save changes/Delete;
  current state survives and the editor keeps its input with a stale result.
  Reload replaces input only explicitly. A deleted current record is shown as
  unavailable, not recreated or offered an MVP restore action.
- [ ] Test sorting retains selected IDs, view changes clear them, and table
  responses for an old view/sort cannot replace the currently requested page.
  Compare replies with their actual request/query; no synthetic generation
  counter. A response arriving after an editor was cancelled must not re-open it.
- [ ] Add `confirmed_deletion_removes_contact_from_all_views_and_retains_review`:
  confirmation shows call, combined UTC start, band/mode, account-wide effect and initial Cancel focus. Use real multi-tag
  contacts: committed deletion hides the QSO in every view and remains present
  in store review results with evidence. Cancellation writes nothing.
- [ ] Run `cargo test -p clacks-desktop ui::contacts --locked --offline`
  and the focused store integration tests to establish missing behavior.
- [ ] Add egui_extras `=0.37.0` at workspace level and inherit it in desktop.
  Use `TableBuilder` for actual bounded rows, native IDs for interactions and
  selection, keyboard focus/Enter and double-click activation. Show Pending/
  Conflict status explicitly; do not allocate one durable UI object per account
  QSO. Display a combined UTC start column and do numeric/time sorting in SQL.
- [ ] Supply column configuration for visibility, order, widths and remembered
  sorting, plus reset. Apply native table resizing where available; implement
  only the missing selection/navigation behavior, not a replacement table
  framework. Unknown stored presentation IDs fall back without resetting account
  data. Keep row height/fonts consistent with the selected UI scale.
- [ ] Read pages of 100 through Task 3, with Previous/Next and truthful page
  feedback. Do not fabricate a total count from the loaded page. View changes,
  sorting and known mutations reset offset; keep selection by ID where the UI
  contract requires it. Request refresh after contact saves/deletions and after
  actual tag operations affecting membership/deletion; invalidate using real
  operation outcomes rather than polling the database every frame.
- [ ] Load the full current record before editing. Render a separate full editor
  with all native facts, static date/time controls and unchanged tag membership.
  Save changes uses its retained expected basis. Share error/highlight/validation
  widgets with entry; block duplicate submissions and distinguish stale, failed,
  saved and unchanged outcomes. Close after successful Save changes, leaving
  new-entry state alone. Reload unavailable/deleted records by showing their
  actual outcome and preserving the user's edits until explicitly discarded.
- [ ] Implement Delete QSO through Task 3 using the confirmed expected record.
  Do not discard a dirty editor on a failed/stale deletion; after committed
  deletion close that editor and refresh the list. Keep pending evidence and
  normal-list hiding in the store. Do not add #51 membership controls or #10
  filters/Worked Before as a shortcut to finishing this issue.
- [ ] Run focused tests and render-check the full workbench, editor and deletion
  confirmation in both appearances. Check controls do not overlap at larger
  fonts/scale, and radio text and relative typography match shared styling.
- [ ] Commit the real table/edit/delete capability, preference handling,
  manifests and lockfile as one coherent change, or split table and editor
  commits if each is independently reviewable and functional.

## Task 6: Integration verification, diagnostics and maintained guidance

**Files:** Update `lib/clacks-desktop/README.md`,
`lib/clacks-client-store/README.md`, `README.md` and
`docs/observability.md` for actual delivered API/screen/telemetry behavior.
Adjust `docs/manual-logging.md` only to correct a demonstrated mismatch without
silently changing accepted behavior. Extend the owning tests from Tasks 1–5;
use `lib/clacks-desktop/src/session/settings/contacts/observability_tests.rs` only
if the existing telemetry support cannot host the focused operation checks.

- [ ] Through actual create/edit/delete operations, verify truthful committed
  events/counters and safe correlation at info when debug spans are filtered.
  Verify stale/unchanged outcomes do not count as newly committed changes and
  canary names/calls/comments/reports never enter Clacks event fields/labels.
  Reuse existing subscriber/recorder capture support. Do not assert dependency
  JSON formatting or repeat the full functional suite with telemetry disabled.
- [ ] Review every changed production boundary for ordinary control flow,
  scoped `#[instrument(skip_all, ...)]`, normal module targets and relevant
  domain events. Do not instrument trivial getters or duplicate span lifecycle,
  operation names, component labels or ID formatting.
- [ ] Verify real shutdown admission rejects unsent work without clearing input;
  already admitted work drains under existing shutdown behavior. Add a focused
  production-session test only if existing shutdown coverage does not exercise
  contact dispatch. No sleeping manual harness or public test-only delay flag.
- [ ] Update maintained documentation to describe actual entry/edit/delete paths,
  local saving, retained pending deletions, current-view scope, independent
  unfinished entry and the remaining #10/#51/provider boundaries. Link the
  specification, keep GitHub as delivery authority, and do not add a review
  chronicle or copy of backlog status. Fold docs into their owning feature
  commits where possible; a standalone usage/verification commit is acceptable
  if it contains substantive integration guidance rather than cleanup narration.
- [ ] Regenerate and check SQL metadata using the production migrations. The
  following POSIX-shell commands match the existing workflow's owning-crate
  scope; on Windows use equivalent PowerShell environment assignment. Keep the
  disposable database outside the repo and remove it after closing connections.

```sh
# Use a newly allocated temporary directory and its real absolute path.
export DATABASE_URL=sqlite:///absolute/temporary/directory/clacks.sqlite
cargo sqlx database create
cargo sqlx migrate run --source lib/clacks-client-store/migrations
cargo sqlx prepare --workspace -- --package clacks-client-store --all-targets --locked --offline
cargo sqlx prepare --check --workspace -- --package clacks-client-store --all-targets --locked --offline
```

Use the existing pinned `sqlx-cli` 0.9.0 with SQLite features. Review generated
`.sqlx/` changes and remove only entries made obsolete by these query changes.

- [ ] Run the repository checks and require successful exit, zero failing tests,
  no formatting diff, no Clippy warnings and no rustdoc warnings:

```sh
cargo +1.99.1 fmt --all --check
cargo +1.99.1 test --workspace --all-targets --locked --offline
cargo +1.99.1 test --workspace --doc --locked --offline
cargo +1.99.1 clippy --workspace --all-targets --locked --offline -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo +1.99.1 doc --workspace --no-deps --document-private-items --locked --offline
cargo +1.99.1 check -p clacks-desktop --target x86_64-pc-windows-msvc --locked --offline
```

The target check requires the installed Windows target/dependencies and is not
native runtime acceptance. A missing cache/toolchain is an explicit environment
limitation, not a passing check. Online dependency acquisition may be needed
before locked/offline verification for the new egui_extras dependency.

- [ ] On native Windows, use the production executable and an isolated real
  workspace: enter two contacts on different dates, unknown End and an overnight
  contact; change defaults and view before Save; switch detail tabs; reopen,
  edit/cancel, and individually delete a multi-tag contact. Check combined UTC
  ordering, styling, keyboard submission, persistence and pending indicators.
  If no Windows session is accessible, provide these concise production checks
  for the user and report acceptance as pending; do not substitute an ignored
  interactive test or claim cross-compilation exercised graphics/input.
- [ ] Review the complete diff against all ten specification sections and
  repository Rust/test/observability conventions. Fix real findings in their
  owning commits and repeat checks affected by those fixes. Inspect each commit
  message's actual line lengths before committing/amending.
- [ ] Report local verification and native limitations. After authorization to
  push/create a PR, monitor the existing PR-triggered Linux and Windows workflow
  until both pass; do not broaden triggers to all branch pushes. Keep native
  visual/input acceptance separate from CI results. Leave completed plan removal
  for post-merge cleanup, preserving the specification and maintained guidance.

## Coverage and sequencing check

| Specification | Implementation and verification |
| --- | --- |
| 1–2: offline scope, production composition and shared layout | Tasks 3–5; rendered review and native acceptance in Task 6. |
| 3: transient input, active corrections and selected-view membership | Task 1 state behavior, Task 4 applied/submitted boundaries. |
| 4: full native values, preservation and validation | Task 1 conversion/policy, shared widgets in Tasks 4–5. |
| 5: combined UTC clocks/locks/unknown End | Task 1 values, Task 4 controls, Task 5 static saved editing. |
| 6: Enter, Save, pending state, failure and reset | Tasks 3–4. |
| 7: local working list, ordering, editor/stale behavior | Tasks 2/5; extends existing read/runtime contracts. |
| 8: account-wide individual deletion | Tasks 3/5, real-store review evidence. |
| 9: transactions, stable identity, runtime and recovery | Tasks 2–4; shutdown verification in Task 6. |
| 10: meaningful telemetry and actual acceptance | Integrated in owning tasks, final checks in Task 6. |

Execute sequentially: Task 1 and Task 2 establish the value/query interfaces;
Task 3 binds them to the production runtime; Tasks 4–5 deliver the actual
workflows; Task 6 completes integrated verification and usage guidance. Routine
implementation details belong within these boundaries, not a reopened product
design or an excuse to add speculative abstractions.

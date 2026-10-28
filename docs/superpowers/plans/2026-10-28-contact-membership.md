# Contact Logbook Membership Editing Implementation Plan

> **For agentic workers:** Use `superpowers:executing-plans` for native execution,
> or `superpowers:subagent-driven-development` if explicitly selected. Complete
> the checkboxes task by task; this document does not authorize execution.

**Goal:** Deliver #51's individual and selected-contact membership editing through
production forms, guarded local saves and a bounded Gmail-style picker.

**Architecture:** Extend the saved-contact form and history/session request path.
A shared picker expresses add/remove intent; the store applies it sequentially to
captured complete QSO bases using ordinary guarded saves. Progress and cancellation
travel through existing Tokio facilities, with a final record-specific report.

**Tech stack:** Rust 1.99.1; egui/eframe 0.37 with wgpu; Tokio; SQLx/SQLite;
existing tracing and metrics. No new dependency is expected.

**Spec:** [Contact logbook membership editing](../../contact-membership.md).
Read it alongside [desktop UI](../../desktop-ui.md),
[manual logging](../../manual-logging.md) and [storage](../../client-store.md).

## Global constraints

- Keep tag membership inside the complete ordinary QSO; preserve whole-record
  stale checks, pending comparison evidence and accepted reconciliation rules.
- Removing the last tag leaves an active untagged QSO. Do not alter other facts,
  directory snapshots, service evidence, external associations or entry defaults.
- Individual picker changes belong to the contact form's Save changes/Cancel;
  bulk picker changes belong to Apply. No extra individual Apply/Done step.
- Use the accepted shared palette, section treatment, typography and scaling.
  The picker must float without resizing panes or weakening editor modality.
- Use existing production components. No new schema, repository layer, job
  service, generic bulk framework, UI driver or synchronization protocol.
- Use named Rust files, library-owned behavior, workspace dependencies, rustdoc,
  named fields and ordinary error propagation. No production unwrap/expect.
- Fixed SQL remains in query files with offline metadata; first reuse the
  existing snapshot/record/catalog reads, so no SQL additions are anticipated.
- Tests demonstrate Clacks decisions using real schema and native component
  facilities. No injected triggers, failpoints, dependency tests, public APIs for
  tests, or duplicate end-to-end coverage through every form.
- Preserve the unrelated `references/desktop-toolkit.md` modification.
- CI is unavailable until the monthly reset; do not request runs or re-enable
  Windows CI. Report local verification and unavailable native acceptance honestly.
- Before commits/amends, inspect a substantive message file and verify physical
  prose wrapping at 72 columns. Commit coherent capabilities, not repair history.

## Review focus

1. Mixed choices must preserve each QSO's untouched tags, including tags hidden by
   search; pin this in Tasks 1 and 3 rather than replacing sets with an intersection.
2. Hidden/off-page selected IDs must be included once while unselected contacts
   remain unchanged; Task 4 pins the actual request scope.
3. A complete expected record must reject intervening fact edits or deletion;
   Task 1 verifies saved state, Task 4 verifies retained operator feedback.
4. Cancellation after a commit must preserve that result and leave the remaining
   contacts unprocessed; Task 1 pins the boundary and Task 2 its runtime wiring.
5. Picker dismissal must preserve individual form input but discard an unapplied
   bulk operation; Tasks 3–4 pin requests, values, focus and modal interaction.

## File and interface map

| Owner | Files | Responsibility |
| --- | --- | --- |
| Guarded membership saves | `lib/clacks-client-store/src/save.rs`, new `src/save/tags.rs` | Move/extend existing `change_tags`; preserve `save` as commit owner. |
| Store integration | new `lib/clacks-client-store/tests/membership.rs`, `tests/integration.rs` | One shared-suite scenario family, using existing fixtures. |
| Runtime membership requests | new `lib/clacks-desktop/src/session/settings/contacts/membership.rs`, `contacts.rs`, `settings.rs` | Snapshot preparation, progress/cancellation bridge and authoritative report. |
| Shared picker | new `lib/clacks-desktop/src/ui/tag_picker.rs`, `ui/tag_picker/tests.rs`, `ui.rs` | Bounded checklist, intent and summary presentation. |
| Shared tag labels | existing `ui/logbooks/labels.rs` and tests; `ui/logbooks.rs` | Reuse duplicate-label implementation with narrow sibling visibility. |
| Editor integration | `ui/contacts.rs`, `ui/contacts/editor.rs` | Pending membership inputs and normal saved-contact Save/Cancel. |
| History integration | new `ui/contacts/membership.rs`, existing `contacts.rs`, `contacts/responses.rs` | Contextual picker, frozen target scope, progress/results and refresh. |
| Runtime/UI regression | existing `session/settings/contacts/tests.rs`, `ui/contacts/tests.rs`; new child test files as needed | Focused boundary assertions, not another executable or fake service. |

Paths abbreviated with `src/` or `ui/` in the table remain under the named crate.
Keep each module focused and below roughly 600 lines. Extract the existing
`ContactsUi::show_editor` implementation into its `contacts/editor.rs` owner as
part of editor integration; avoid expanding the already large parent module.
Do not restructure unrelated UI or storage modules.

## Task 1: Preserve partial outcomes in guarded membership saves

**Files:** Store save files and shared integration files from the map.

**Interfaces:** Introduce public production types under `save::tags`, without
re-export aliases. Names/fields below define the interface for Task 2:

- `TagChangeEntry { id: QsoId, outcome: SaveOutcome }`.
- `TagChangeProgress { total: usize, completed: usize, changed: usize,
  unchanged: usize, stale: usize }`.
- `TagChangeStop::Cancelled` or `Failed { id: QsoId, error: anyhow::Error }`.
- `TagChangeReport { entries: Vec<TagChangeEntry>, stop: Option<TagChangeStop>,
  unprocessed: Vec<QsoId> }`.
- `async fn change_tags(store: &ClientStore, expected: &[ClientRecord],
  add: &BTreeSet<TagId>, remove: &BTreeSet<TagId>,
  progress: impl FnMut(&TagChangeProgress) -> std::ops::ControlFlow<()>)
  -> anyhow::Result<TagChangeReport>`.

The callback is a production boundary for progress and cancellation, not a test
hook. Call it before processing the first record and after each settled save,
checking its returned stop request before starting the next transaction. Never
cancel a save future in flight. All counts are derived from returned outcomes.
After the last target settles, completion wins over a late cancellation request.

- [x] Extend the shared store suite with `membership_preserves_per_contact_values`
  and `membership_retains_stale_and_cancelled_results`. Build A with Main/Activity,
  B with Main, and an unrelated contact. Add Portable/remove Main: A retains
  Activity, B does not acquire it; facts and non-membership record roles remain
  intact. Reload from the store, including one close/reopen in this scenario.
  Remove all tags from one contact and assert its live identity in All contacts
  and Untagged using existing read contracts. Do not add another fixture solely
  to duplicate generic persistence coverage.
- [x] In the second scenario, change B's comment after capturing its basis, then
  submit A/B/C. Assert B is Stale and its newer comment survives. In another run
  cancel through the production progress callback after the first completed save;
  assert that entry is retained and the precise remaining IDs are unprocessed.
  Include an ordinary deleted-record stale outcome without resurrecting it.
  Assert unchanged results preserve the complete record without artificial edits.
- [x] Run `cargo test -p clacks-client-store --test integration membership --locked`.
  Confirm the new assertions fail for the missing report/cancellation contract,
  rather than expecting a failure in SQLite or SQLx.
- [x] Move the existing helper into `save/tags.rs` and implement the interface.
  Validate all supplied bases, account/key uniqueness and disjoint intent before
  the first write. Preserve input order; desktop preparation will sort by QSO ID.
  Transform each captured QSO's tags and call existing `save::save` once per target.
  Continue after Saved/Unchanged/Stale. On operational error retain prior entries,
  identify the failing ID separately, and put only not-yet-attempted IDs in
  `unprocessed`. A failure to acknowledge a commit is not proof of rollback.
- [x] Keep preflight validation errors as the outer `Err` before mutations.
  After processing starts, operational errors belong in the report. Use existing
  save events/counters; do not duplicate mutation telemetry in the batch loop.
  Keep error sources through `anyhow`, not string-only store failures.
- [x] Rerun the focused command; inspect each test's Clacks requirement and actual
  mistake caught. Do not manufacture a failure just to exercise the Failed arm.
- [x] Commit the focused store capability with motivation and a wrapped body.

## Task 2: Integrate preparation, progress and cancellation with the runtime

**Files:** Runtime contacts files, `session/settings.rs`, and existing contact
runtime tests. Add a child `membership/tests.rs` only if it keeps ownership clearer.

**Interfaces:** Put these crate-private types in
`session::settings::contacts::membership`:

- `MembershipBasis { records: Vec<ClientRecord>, catalog: LogbookSnapshot }`.
- `MembershipSubmission { basis: Vec<ClientRecord>, add: BTreeSet<TagId>,
  remove: BTreeSet<TagId>, cancel: CancellationToken,
  progress: watch::Sender<TagChangeProgress> }`.
- `async fn prepare(store: &ClientStore, ids: &BTreeSet<QsoId>)
  -> anyhow::Result<MembershipBasis>`.
- `async fn apply(store: &ClientStore, submission: MembershipSubmission)
  -> anyhow::Result<TagChangeReport>`.
- Add `ContactAction::PrepareMembership { ids: BTreeSet<QsoId> }` and
  `ContactAction::ChangeMembership(MembershipSubmission)` with corresponding
  `ContactResponse::MembershipPrepared(MembershipBasis)` and
  `MembershipChanged(TagChangeReport)`.

- [x] Add a runtime scenario `membership_preparation_captures_the_selected_basis`:
  save real contacts, prepare a stable selected-ID set, assert each returned
  complete record and the catalog. Modify one record before applying and assert
  the runtime returns its Stale result while other captured targets can save.
  Extend this scenario with a missing/deleted target at preparation: produce an
  actionable preparation error and no mutation, not a reduced silent target set.
- [x] Run `cargo test -p clacks-desktop session::settings::contacts --locked` and
  establish the missing runtime behavior before implementing it.
- [x] Implement `prepare` with one existing `read::snapshot`: use `record` for
  selected keys and `settings().logbooks` for the catalog. Reject an empty target
  set or unavailable/non-live target with its ID in diagnostics. Read all selected
  IDs once in stable order; a page limit must not truncate selection. Close the
  snapshot before returning; no database transaction survives operator interaction.
- [x] Implement `apply` through Task 1. Its callback publishes progress using
  `watch::Sender::send_replace` and returns Break when the existing Tokio
  cancellation token is cancelled. Closing a watch receiver alone must not cancel
  writes or lose the final result. No new cancellation dependency in the store.
- [x] Treat preparation as a read in `settings::execute`: no follow-up settings
  refresh, and an abandoned preparation receiver may cancel obsolete read work.
  Treat changes as admitted writes: retain the final response and existing
  commit-versus-refresh separation even if the popup is dismissed. Preserve the
  current session admission/drain path; do not spawn a second scheduler.
- [x] Add a focused reporting assertion to the existing runtime scenario: use a
  pre-cancelled submission, verify zero changed entries and the complete unprocessed
  scope. Verify the watch's final counts agree with the authoritative report.
  Store Task 1 owns the after-first-commit case; do not recreate it through the UI.
- [x] Instrument this runtime operation with `#[instrument(skip_all, ...)]` and
  default module targets. Emit a focused outcome event with truthful changed,
  unchanged, stale and unprocessed counts, and cancellation/failure status. Use
  info for completed user work, deliberate error reporting for operational failure,
  and existing save counters for commits. Do not add an overall latency metric
  unless an operational use beyond the trace is identified.
- [x] Extend the runtime scenario's existing capture with an assertion on the
  focused membership outcome event: counts agree with the report and do not
  describe stale/unprocessed contacts as saved. Reuse telemetry test support;
  do not repeat the functional scenario solely to exercise an exporter.
- [x] Rerun the focused tests; commit the production request integration.

## Task 3: Build and use the shared picker in the saved-contact editor

**Files:** Shared picker, labels, `ui.rs`, contact editor and their component tests.

**Interfaces:** Keep picker types internal to the desktop UI:

- `MembershipMark::{All, Some, None}` and `TagChoice { id: TagId, label: String,
  initial: MembershipMark, current: MembershipMark }`.
- `MembershipIntent { add: BTreeSet<TagId>, remove: BTreeSet<TagId> }`.
- `MembershipPicker` owns choices, search text and normal egui list state;
  `new(catalog: &LogbookSnapshot, memberships: &[BTreeSet<TagId>]) -> Self`,
  `intent(&self) -> MembershipIntent` and `is_changed(&self) -> bool`.
- `show_contents(ui: &mut egui::Ui, picker: &mut MembershipPicker) -> bool` returns
  whether membership choices changed. Callers own popup lifetime and Save/Apply.
  Expose the existing `logbooks::labels::labels` only to sibling UI modules.

- [x] Add one component scenario `picker_preserves_mixed_intent_through_search`
  using native egui_kittest semantic queries. With A Main/Activity and B Main,
  assert untouched Activity yields no operation; checking then clearing it yields
  add then remove; returning to mixed yields none. Add Portable while Activity is
  hidden by search and assert only Portable is added. Exercise actual callbacks/
  resulting intent, not the dependency's checkbox mechanics in isolation.
- [x] Run `cargo test -p clacks-desktop ui::tag_picker --locked` and verify the
  missing Clacks intent/presentation boundary fails before implementing it.
- [x] Implement native `Checkbox::indeterminate` presentation and E2 cycling.
  Uniform rows toggle two states; initially mixed rows cycle Some → All → None
  → Some. Only changed rows generate intent. Match search by case-insensitive
  literal substring of the full displayed label and sort name then ID.
  Derive labels from the complete live catalog before filtering, reusing existing
  collision handling. Built-in views are never checkbox entries.
- [x] Use egui's native Popup, vertical ScrollArea, focus and checkbox semantics.
  Start with preferred width 360 logical points, clamped to available viewport;
  cap the list at 240 logical points, with search and actions outside scrolling.
  Use existing theme tokens. Summaries show at most three assigned labels plus
  remaining count, truncated to available width; empty reads "None". These are
  layout defaults subject to actual production visual inspection, not new design
  language. Keep unknown/deleted memberships identifiable and explicitly removable,
  but unavailable as new assignment choices; do not treat a catalog error as empty.
- [x] Move `ContactsUi::show_editor` into its editor child owner and insert the
  compact Assigned logbooks summary/Edit… before the other detail sections.
  Build single-contact picker state from the form's current pending tags when
  opened. Apply choice changes to that form immediately using the opening copied
  tag set plus picker intent. Dismissal preserves form input; reopening reflects
  it. Never mutate the saved expected record or form baseline for highlighting.
- [x] Add `editor_membership_uses_the_existing_save_boundary` to contact component
  tests: change tags and a fact, dismiss picker, assert no runtime save request;
  Save changes submits both together with the original complete expected basis.
  Cancel discards input. Verify Escape closes the picker before the modal and
  Enter while operating a checkbox does not accidentally submit the contact.
  Extend existing stale-input assertions rather than duplicate store transactions.
- [x] Show membership changes using existing unapplied styling without adding
  catalog-size-dependent sections. Disable picker editing while saving. Keep
  new-entry draft/defaults untouched and existing modal backdrop blocking intact.
- [x] Run `cargo test -p clacks-desktop ui::contacts --locked` plus the picker tests;
  retain duplicate-name label tests in their current owner, not a copied suite.
- [x] Commit the shared picker and its working individual-editor integration.

## Task 4: Deliver selected-contact assignment and retained result feedback

**Files:** `ui/contacts/membership.rs`, parent contacts, responses and contact tests;
minimal shared-window invalidation wiring only where existing `changed` requires it.

**Interfaces:** `BulkMembership` owns frozen selected IDs, an optional prepared
basis/picker, the preparation reply, submission cancellation/progress handles and
final report. Keep these named fields, not tuple states. Implement its presentation
and polling in the membership child module through `ContactsUi` methods; do not
export a second contact controller or public workflow framework.

- [x] Add `history_membership_request_uses_all_selected_ids`: build the existing
  contacts UI with one visible selected ID and selected IDs off-page/outside the
  filter; click Edit assigned logbooks… and assert PrepareMembership carries the
  precise deduplicated selection. With no selection, assert the action is absent.
  Use the existing lightweight component request channel, not a fake database.
- [x] Run the focused contact tests and verify this missing request boundary fails.
- [x] Render the exact contextual action without a numeric suffix. Capture IDs on
  opening and show Loading/error rather than an empty picker until preparation
  succeeds. Verify originating scope before accepting replies; discard obsolete
  preparation on view switch. Filtering/search must not change target IDs.
  Present the selected count and known outside-filter count in the picker; retain
  the normal history status. Do not require targets to be present in the page.
- [x] Instantiate Task 3's picker from the prepared per-contact working sets. Apply
  enabled only for changed intent; Cancel/Escape/outside dismissal discards the
  unsubmitted bulk state. Apply submits Task 2's captured bases and intent once.
  Preserve a submitted operation through view changes/popup dismissal, without
  retargeting it or allowing duplicate submission.
- [x] Display read-only submitted choices and progress; Cancel processing calls
  the token, not task abort or transaction cancellation. Keep the final report
  receiver until acknowledgment even if its popup closes. While awaiting progress,
  request ordinary bounded repaints (100 ms); do not add a custom frame driver.
- [x] Retain a dismissible record-specific report in the history feedback area:
  saved, unchanged, stale, failed/unacknowledged and unprocessed identities are
  distinct. Show useful existing contact context where available; IDs remain the
  stable identity. No persistent job/history table. A failed acknowledgment does
  not justify blind replay; require current-state review before another attempt.
- [x] Extend contact response assertions with a report containing saved/stale/
  cancelled-tail outcomes. Assert preserved report, truthful counts and changed
  invalidation only after actual saves. Mark history dirty and set the existing
  workbench `changed` flag for affected Worked Before refresh. Retain selection
  for identities leaving the filter; do not alter the unfinished QSO/defaults.
  Keep reports on SettingsResponse::RefreshFailed and offer refresh, not re-Apply.
- [x] Verify a catalog/read failure retains input and errors; changing views
  discards an unsubmitted picker, and a late old preparation cannot reopen it.
  Build these into existing component scenarios rather than one host per case.
- [x] Run the focused contact/runtime tests and inspect the integration in the
  actual application. Commit the working bulk capability and feedback together.

## Task 5: Verify the assembled application and reconcile usage documentation

**Files:** Update `docs/contact-membership.md` only for confirmed implementation
integration details; adjust crate README usage where it helps callers. Keep the
product spec separate from verification chronology. Record evidence in delivery/
PR notes, not a checked-in review report.

- [x] Run focused store, desktop picker/contact and runtime tests once more after
  integration. Audit changed tests against requirement + concrete Clacks mistake
  + cost. Remove redundant scaffolding instead of defending it with coverage.
- [x] Run `cargo fmt --all --check`, `cargo test --workspace --all-targets --locked`,
  `cargo test --workspace --doc --locked`,
  `cargo clippy --workspace --all-targets --locked -- -D warnings`, and
  `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --document-private-items --locked`.
  Successful exit and passing tests are required; record exact failures/limits.
  Run SQL metadata preparation only if fixed queries actually change, using the
  production migrations and pinned CLI command from the existing workflow.
- [ ] Run the production `cargo run -p clacks-log --locked -- --workspace <temporary-path>`.
  Through actual UI, prepare hundreds of logbooks using existing supported
  production APIs/tooling if needed (Rust only; no production seed flags). Inspect
  single and mixed membership, duplicate names, search-hidden choices, keyboard/
  focus/dismissal, Save versus Apply, cancellation and results. Include light/dark,
  scaling and narrow/wide windows; verify picker bounds, unchanged pane geometry,
  no modal click-through, no giant full-catalog form and no ambiguous count suffix.
- [ ] Capture assembled-application screenshots for the individual and bulk
  workflows, noting platform and exercised states. Use the accepted desktop
  reference/mockup as the visual comparison. Component tests do not substitute
  for this inspection. State Windows native acceptance as unavailable unless the
  user provides results; do not create human-operated cargo test harnesses.
- [x] Check docs/links and `git diff --check`; review the final diff for accidental
  schema/protocol/CI changes, unrelated edits, public test APIs and generic helpers.
  Fold branch-introduced corrections into their owning commits if requested.
- [ ] Prepare a final delivery summary with actual checks and remaining limits.
  Stop before pushing or creating a PR unless the user explicitly authorizes it.

## Execution handoff

Tasks are sequential: store outcomes → runtime → shared picker/individual editor
→ history integration → assembled verification. Native execution fits the tightly
shared interfaces; no separate agent orchestration is necessary to implement them.
The approved spec remains the authority if implementation exposes a discrepancy.
Do not reopen settled requirements or silently change them to satisfy tests.

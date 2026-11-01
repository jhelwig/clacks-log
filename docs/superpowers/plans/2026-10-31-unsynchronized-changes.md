# Unsynchronized Changes (#15) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for native execution, task by task. Steps use checkbox (`- [ ]`) syntax for tracking. Do not begin implementation until this plan has been reviewed and execution authorized.

**Goal:** Deliver account-wide review and explicit resolution of unsynchronized
changes in the production desktop without disrupting logging or unrelated sync.

**Architecture:** Extend the existing SQLite review reads, guarded resolution
operation and desktop session request path. A non-modal egui viewport owns typed,
in-memory resolution drafts and reuses existing field controls. Existing native
transitions retain ownership of conflict detection, preparation and acceptance.

**Tech Stack:** Rust 1.99.1, eframe/egui 0.37.0 with wgpu, Tokio, SQLx 0.9.0,
SQLite, clacks-model, clacks-sync, egui_kittest and existing telemetry facilities.

**Spec:** [#15 integration contract](../../conflict-review-proposal.md), together
with [D382–D387](../../decisions.md#d382) and the linked product/UI/sync contracts.
Engineering defaults are implementation choices, not additional agreed product
decisions.

## Global constraints

- Carry forward D382–D387 exactly: separate non-modal window, collapsible
  Conflicts/Pending sync, record-type filtering, no logbook selector, explicit
  scalar choices/custom values, local initial membership and direct checkboxes.
- Preserve drafts across navigation and outdated comparisons. Refresh before
  saving changed comparisons; closing discards unsaved drafts only.
- Keep whole-record detection, three-way attribution, conservative missing-evidence
  handling, reference validity, receive-first sync and conditional acceptance.
- Use Rust 1.99.1 from rust-toolchain.toml and existing workspace dependencies.
  No Python, xtask, new external service or protocol change.
- No new conflict engine, generic form/repository framework, persistent draft
  schema, generation counters, manifest, action-intent history or automatic merge.
- Production APIs need actual delivered callers. Connect new capabilities in the
  same task; later tasks extend them. No dead_code allowances or test-only APIs.
- Use named data fields, focused modules and rustdoc. No production indexing,
  expect/unwrap, wildcard re-exports or mod.rs outside tests. Split a source file
  approaching 600 lines by responsibility rather than accumulating another one.
- Keep fixed checked SQL in query files and regenerate committed offline metadata
  with project tooling. Use production migrations and disposable test databases.
- Tests establish specific Clacks decisions; do not test SQLx/SQLite/framework
  guarantees, manufacture triggers/failpoints, duplicate an entire GUI matrix,
  or introduce an alternate service/test application.
- Follow desktop-ui.md and observability.md. No payload/field-value logging,
  redundant telemetry fields or lifecycle abstractions. Rendered inspection is
  required; passing semantic tests is not visual acceptance.
- Before execution, read AGENTS.md and the user-owned worktree-placement skill.
  Create isolation through its guarded placement workflow. Preserve the existing
  references/desktop-toolkit.md edit. Do not run/configure the persistent server
  before reading the user-owned development inventory; tests use disposable data.
- Each substantive task is one coherent commit unless size warrants a real split.
  Fold corrections into the owning commit. Before every commit/amend, prepare a
  substantive message file, verify physical prose wrapping at 72 columns, inspect
  it, and use git commit --file. Do not commit review-process reports.
- Do not push, create a PR, enable Windows CI, or treat unavailable CI as passing
  without the applicable authorization. Current CI excludes Windows tests.

## Agreed behavior versus engineering defaults

The decisions above are requirements. The following are defaults adopted from
the proposal for planning, not new decision-register entries:

| Default | Implementation owner |
| --- | --- |
| Both sections initially expanded; page size 100; independent section cursors and counts | Task 1 |
| Four labeled comparison columns where space permits; stacked field groups when narrow | Task 2 |
| Hide equal rows with a Show unchanged fields control | Task 2 |
| Native position, radio pair and mode hierarchy as composite editor choices | Task 2 |
| Retained accepted versions available as additional comparison sources | Task 2 |
| Preserve unaffected choices on refresh; changed inputs require explicit reconsideration of retained results | Task 2 |
| Permit disconnected resolution when durable comparison evidence is usable | Task 2 |
| Refresh references / Retry deletion using current accepted reference facts | Task 4 |

Do not reinterpret these defaults as a new interdependent-field detection model.
If execution reveals a material contradiction, report it rather than selecting a
replacement behavior silently. Routine implementation choices stay with the
owning task and do not become another approval queue.

## Review focus

1. A selected historical native value must survive without editor-text conversion
   changing its coarse position, unknown labels or timestamp (Task 2 tests).
2. An unrelated update must not erase or invalidate a draft; a relevant update
   must prevent saving against unseen inputs, including late replies (Task 2).
3. A pending local reference repair must not count as accepted shared repair and
   falsely unblock target deletion (Task 4).
4. An old-actor hold or retained-only recovery state must remain visible even
   before a usable comparison exists (Tasks 1 and 4).
5. Closing/navigating during an admitted save must not cancel its commit or turn
   a later refresh failure into a reported failed save (Tasks 2 and 5).

## File and responsibility map

Paths below are relative to the repository root. New modules have real production
consumers in their task. Exact helper extraction may follow existing module
visibility; do not create a new crate for this capability.

| Files | Responsibility |
| --- | --- |
| lib/clacks-client-store/src/read/review.rs and read/review/references.rs | Review sections/pages, exact detail, counts and accepted referrers |
| lib/clacks-client-store/sql/read/review/*.sql; existing sql/read/pending.sql | Checked review queries; remove the replaced combined query |
| lib/clacks-client-store/src/synchronization.rs and synchronization/resolution.rs | Existing guarded save plus reference validation and blocked-deletion retry; move owning code rather than duplicate it |
| lib/clacks-desktop/src/review.rs and review/{comparison,draft,fields,validation}.rs | Typed comparisons, explicit choices, native construction and draft lifecycle |
| lib/clacks-desktop/src/ui/review.rs and ui/review/{list,comparison,editors,references}.rs | Native window and concrete rendered states |
| lib/clacks-desktop/src/session/settings/review.rs | Review request execution on the existing runtime |
| lib/clacks-desktop/src/ui.rs, ui/actions.rs, ui/workbench.rs | Owned review UI state, workbench action and viewport presentation |
| lib/clacks-desktop/src/session.rs, session/settings.rs, synchronization/exchange.rs | Existing request/reply, committed-result and workspace invalidation wiring |
| Existing logging/operating/tag-picker controls and forms | Narrow reuse/extraction only where the normal and review screens both consume it |
| Existing store integration executable; desktop review/session tests | Owning behavior assertions, not another linked executable per scenario |
| docs/product.md, docs/synchronization.md, docs/desktop-ui.md and crate READMEs | Current delivered contracts and usage; no review chronology |

## Task 1: Open the real account-wide review window

**Deliverable:** a production workbench action opens a populated non-modal review
window, with separately paged sections, actual identities and read-only details.
Do not display future resolution controls until their task implements them.

**Files:** create the store read/reference modules and desktop UI/session review
modules above; modify read/review.rs, sql/read/pending.sql, session/settings.rs,
ui.rs and the workbench action owner. Update .sqlx/ and the existing read tests.
Add review tests to the existing tests/integration.rs module declarations.

**Interfaces:**

- Keep ReviewQuery's kind/after/limit meanings; remove its unused tag filter.
  Add `section: Option<ReviewSection>`; None preserves combined reads needed by
  existing callers. `ReviewSection` has Conflicts and PendingSync variants.
- Extend `ReviewRow` with `authorization_hold: Option<ClientActorId>`; retain its
  named record/review fields. Use the existing stored hold, not a copied marker.
- Add `ReviewCounts { conflicts: u64, pending_sync: u64 }` and
  `ReviewPage { rows: Vec<ReviewRow>, counts: ReviewCounts,
  next: Option<RecordId> }`, with named fields and documented cursor semantics.
- `ReadSnapshot::review_page(&mut self, query: &ReviewQuery)
  -> anyhow::Result<ReviewPage>` and
  `ReadSnapshot::review_record(&mut self, key: RecordKey)
  -> anyhow::Result<Option<ReviewRow>>` use the same short snapshot as any
  requested SettingsSnapshot. Existing `review` callers may delegate to the page
  operation; do not maintain two query implementations.
- Exact detail reads return an existing record even if it has just left the
  unsynchronized list. Do not apply the page's pending predicate to a selected
  record read; that would hide the authoritative post-save/settled state.
- Add `SettingsAction::Review(ReviewAction)` and matching
  `SettingsResponse::Review(ReviewResponse)`; define ReadPage and ReadRecord as
  named-field request variants, with identity-qualified Page/Record responses.
  ReadRecord returns the row plus SettingsSnapshot for its source catalog from
  the same bounded snapshot. Reads are not shared writes.
- `ReviewUi::show_window(&mut self, context: &egui::Context,
  requests: &mpsc::Sender<SettingsRequest>)` uses the Worked Before viewport
  pattern and stable ID `clacks.unsynchronized_changes`.

- [x] Add a failing `review_sections_include_hidden_and_held_work` scenario to
  `tests/reads.rs`: create actual pending edits/deletion and existing review/hold
  states using production operations; assert distinct section membership, full
  filtered counts and no duplicate row when a record has both pending and review.
  A held row without usable ReviewInput is Conflicts/Waiting for comparison data.
  Retained-only recovery work stays visible under Conflicts as blocked evidence.
- [x] Extend that scenario with a bounded continuation, record-type filtering,
  untagged QSO, duplicate tag names and exact detail read. Assert pages/counts
  share the selected snapshot; the deleted QSO is identifiable from retained
  facts without appearing in normal history. Remove old tag-filter assertions
  that no longer express an agreed review requirement.
- [x] Run `SQLX_OFFLINE=true cargo test -p clacks-client-store --test integration
  review_ --locked --offline`; establish failure at the changed Clacks contract.
- [x] Implement checked section predicates and counts from actual review/hold/
  retained state. Do not classify concurrency in SQL. Reuse kind-qualified
  cursors; collect only a page plus bounded lookahead and obtain next from it.
  Remove the unused tag filter/join and replaced query metadata.
- [x] Implement the real session read requests, appbar action and viewport.
  Conflicts and Pending sync collapse independently; each keeps its own loaded
  page/cursor. Counts label records. Unsupported record kinds remain visible
  with safe identity/reason, not silently filtered out or made into JSON editors.
- [x] Add one egui_kittest test `review_sections_preserve_workbench_context`:
  open the production component with native snapshots, collapse either section,
  change type filter and request more rows; assert actual typed read requests,
  section expansion state and unchanged workbench draft/view. Do not test native
  viewport creation as a framework capability.
- [x] Regenerate/verify scoped SQLite SQL metadata using the commands in the
  verification section. Run the read scenario and desktop `review` tests. Update
  crate usage docs and commit this usable read-only window with a wrapped body
  explaining account-wide scope and why the tag filter was removed.

## Task 2: Resolve QSO fields with guarded saves and durable native values

**Deliverable:** the window resolves real QSO conflicts, including custom values,
source shortcuts, membership, retained sources, draft navigation and refresh.

**Files:** create the desktop review comparison/draft/field/validation modules
and UI comparison/editors modules; modify session/settings/review.rs,
session/settings.rs, session.rs, synchronization/exchange.rs,
client-store synchronization/resolution.rs and necessary shared field controls.
Add `review/tests.rs`, `ui/review/tests.rs` and store resolution scenarios under
the existing integration test executable. Update metadata if queries change.

**Interfaces:**

- `ComparisonBasis { record: ClientRecord, review: ReviewInput }` owns the exact
  inputs shown. `ComparisonSource` uses Local, Shared and
  `Retained { stamp: VersionStamp }`; find native alternatives by identity, never
  by displayed position or unchecked indexing.
- `ResolutionDraft::new(basis: ComparisonBasis) -> anyhow::Result<Self>` and
  `ResolutionDraft::build(&self, definitions: &DefinitionSet)
  -> Result<RecordState, Vec<FieldError>>` live in desktop review/draft.rs.
  Its typed editor variant owns field inputs and explicit choice state.
- `ResolutionDraft::select(&mut self, field: FieldId, source: ComparisonSource)
  -> anyhow::Result<()>`, `select_all(&mut self, source: ComparisonSource)
  -> anyhow::Result<()>`, and `refresh(&mut self, basis: ComparisonBasis)
  -> anyhow::Result<()>` operate on actual native fields. FieldId is a closed,
  crate-private typed field identity with named station-side/nested-field data,
  not string paths or a public extensibility registry. QSO fields are listed
  below; Tasks 3–4 extend only variants they immediately consume.
- Add `ReviewAction::Resolve { expected: ClientRecord, shown: ReviewInput,
  chosen: RecordState }`; its production executor calls the existing
  `synchronization::resolve` signature and returns the actual SaveOutcome.
  `is_shared_write` includes resolution; the existing session wake follows the
  committed operation, not a UI button click.
- Preserve `synchronization::resolve(store, expected, shown, chosen)
  -> anyhow::Result<SaveOutcome>`; add local-reference validation inside its
  existing immediate transaction before storing the chosen state.

**Field contract:** start/end separately; own/contacted callsign and geographic
scalar fields separately; each native Position as one composite choice; each
primary/receive pair as one composite choice; one preserved mode hierarchy;
operator callsign/name separately; power, reports and comment separately;
copied equipment description/antenna/notes separately; each station-side
participation set as a typed set; complete logbook membership as a TagId set.
Identity, causal metadata, directory fallback and service evidence are not
editable QSO fields. Composite controls enforce existing calculations, not a
new conflict-trigger model.

- [x] Write `resolution_preserves_selected_native_values` in review/tests.rs.
  Use a whole-record conflict whose shared side includes an imported two-character
  own locator and unfamiliar mode/band. Select source values, edit a different
  comment, then assert exact native position/basis, mode, band and timestamp
  preservation. Assert genuinely typed new own position still requires four
  characters. Do not round-trip all selected values through ContactForm text.
- [x] Write `resolution_choices_are_explicit_and_membership_starts_local` in
  that module: differing scalar fields cannot build while unchosen; an explicit
  clear and custom valid text count as choices; equal fields need none. Initial
  membership equals local, Use shared replaces it, adding the local-only tag
  produces Main/Field Day/Portable, and clearing all tags yields an active QSO.
- [x] Write `resolution_draft_refresh_preserves_inputs` with two drafts:
  navigation preserves both; unrelated record changes do not invalidate them;
  a changed basis disables Save and keeps the old displayed comparison. Refresh
  retains typed inputs, preserves unaffected choices and requires reconsidering
  changed inputs. A late reply for the first record cannot overwrite the second.
  Closing discards drafts without reverting a completed save.
- [x] Run `SQLX_OFFLINE=true cargo test -p clacks-desktop --lib review --locked
  --offline`; establish that the new behavior is absent before implementation.
- [x] Implement typed comparison using genuine original/native local/shared
  values. Do not manufacture attribution for MissingOriginal or unknown ordering.
  Copy source selections as native field values, assemble QsoFields and validate
  the final QSO. Use existing parsers/controls only for explicitly edited inputs.
  Extract only narrowly shared controls, with actual existing-screen callers.
- [x] Reuse the membership picker with exactly one resolved set. Include IDs
  from local/shared/retained sets in its available context so an unavailable or
  shared-only membership is not lost. Source replacement and checkbox changes
  update the resolved set immediately; no mixed state or extra Apply action.
- [x] Implement real resolved editors and source buttons. Show all available
  native alternatives without duplicating QSO history. Use fixed timestamp
  editors, existing debounce and field errors, edited highlights and disabled
  Save. Do not display a flashing Checking changes row.
- [x] Wire exact basis/record identity to Resolve and shared-write wake. An
  admitted save becomes read-only with Saving feedback; its result belongs to
  the originating record even after navigation/closure. Retain the result when a
  subsequent settings read fails; do not silently retry with a newer basis.
- [x] Extend the existing workspace-change notification path after actual sync
  incorporation and local saves. Bounded rereads compare bases; do not use the
  Connected status as invalidation. Do not add another bus, polling loop or
  persisted generation. Disconnected usable comparisons may be resolved; an
  Unavailable observation is not a shared empty/deleted value and cannot save.
- [x] Add `resolution_save_rechecks_basis_and_live_references` to the existing
  store integration executable: a real intervening update returns Stale and
  changes nothing; valid resolution clears only its review, becomes pending,
  and preserves native values/retained evidence. A newly deleted tag fails
  resolution validation instead of clearing the reference issue; an existing
  live but unpublished tag is allowed and retains prerequisite ordering.
- [x] Add one focused egui_kittest scenario that custom-edits a comment and
  changes membership, asserts the exact Resolve request and disabled Save for
  invalid input/outdated comparison, then accepts Saved/Stale responses. Use
  native snapshots and existing simulated-clock facilities; do not host a store
  merely to click controls or repeat all field tests in the GUI.
- [x] Run the focused desktop/store scenarios, formatting and affected clippy
  checks. Inspect the real assembled window at wide/narrow sizes with working
  QSO entry alongside it. Update delivered docs and commit with context about
  preserving native facts and requiring a refreshed comparison.

## Task 3: Resolve saved profiles, equipment and logbook tags

**Deliverable:** the same guarded workflow edits the existing account-shared
operator-owned records without changing active operating copies or QSO history.

**Files:** extend review/fields.rs, draft.rs, comparison.rs and ui/review/editors.rs;
modify operating/editor.rs, ui/operating/fields.rs, ui/logbooks/editor.rs and
ui/tag_picker.rs only as needed for shared controls. Keep modules focused; split
QSO/station/settings fields under review/fields/ if that prevents a large file.
Add cases to the existing review and store resolution tests.

**Interfaces:** extend the same FieldId and typed editor variants with profile
name/station/default equipment; equipment name/description/antenna/notes; tag
name/independent LoTW and QRZ policy fields. Reuse ProfileEditor, EquipmentEditor,
EquipmentForm and the normal policy controls without their ordinary save/apply
handlers. All submissions remain ReviewAction::Resolve from Task 2.

- [x] Write `saved_source_resolution_does_not_apply_operating_values`: combine
  profile fields and replace/remove its equipment default through actual
  resolution; assert saved reusable facts change while active copied values,
  loaded baselines and historical QSO snapshots retain the existing contracts.
  Resolve equipment values and a duplicate-named tag with independent service
  policies in the same scenario where useful, not a fixture per text field.
- [x] Run the affected review/store test filters to show absent behavior, then
  implement the typed choices, direct editors and existing domain construction.
  Preserve unknown reference associations and unchanged historical values;
  apply current definition normalization/validation only to explicit edits.
- [x] Ensure selecting saved sources in this window cannot call OperatingEditor
  Apply or load a saved profile. Deletion continues through the existing
  metadata::client::store cleanup; no duplicate UI cleanup or silent reload.
- [x] Render profile/equipment/tag differences and exact field errors in the
  same comparison layout. Surface missing equipment as an explicit reference
  repair rather than automatically dropping the default. Do not add management
  screens for service records, group definitions or export profiles.
- [x] Add a bounded component request assertion to the existing review test for
  a reusable-record save and Cancel. Verify no Apply/SaveProfile/SaveEquipment
  request escapes instead of Resolve. Reuse the existing test host.
- [x] Run affected tests, inspect one assembled reusable-record conflict and
  update the relevant crate docs. Commit this completed workflow with a wrapped
  body explaining why reusable record resolution does not apply operating values.
  Focused store command: `SQLX_OFFLINE=true cargo test -p clacks-client-store
  --test integration saved_source_resolution_ --locked --offline`; desktop
  command: `SQLX_OFFLINE=true cargo test -p clacks-desktop --lib review --locked
  --offline`. Both must pass with the actual new assertions executed.

## Task 4: Reference, deletion and reauthorization review

**Deliverable:** all existing review reasons have truthful presentation and
usable permitted actions; blocked deletions can retry safely after real repair.

**Files:** create read/review/references.rs and its checked SQL if not already
needed; extend synchronization/resolution.rs, session/settings/review.rs and
ui/review/references.rs. Add focused cases to existing preparation/enrollment/
recovery fixtures and the shared store integration executable, not new test-only
production entry points.

**Interfaces:**

- `ReferencePage { rows: Vec<ReviewRow>, total: u64, next: Option<RecordId> }`;
  `ReadSnapshot::review_references(&mut self, key: RecordKey,
  after: Option<RecordId>, limit: u32) -> anyhow::Result<ReferencePage>` reads
  actual surviving accepted QSO/profile references for Tag/Equipment targets.
  Do not reuse a working-value deletion preview as accepted-reference evidence.
- `synchronization::retry_deletion(store: &ClientStore, expected: &ClientRecord,
  shown: &ReviewInput) -> anyhow::Result<RetryDeletionOutcome>` has named-field
  variants Ready { record: ClientRecord }, StillReferenced { page: ReferencePage }
  and Stale { current: Option<ReviewRow> }. Its existing review reason must be
  Referenced; never clear a different hold/reason through this action.
- Add ReadReferences and RetryDeletion as named-field ReviewAction variants and
  corresponding identity-qualified ReviewResponse variants. RetryDeletion is a
  shared-write action; ReadReferences is not. Use the existing ordinary contact/
  profile editor requests for explicit referrer repair, or open that referrer's
  review when it has its own conflict.

- [x] Write `blocked_deletion_waits_for_accepted_reference_repair`: accepted
  fourth QSO references a locally deleted tag; local membership removal alone
  leaves StillReferenced and the review intact. Install the actual accepted
  repaired QSO, retry and assert the same pending deletion is eligible. A new
  concurrent accepted reference returns a refreshed block through ordinary
  rejection. The test detects Clacks confusing local repair with shared repair.
- [x] Include equipment/profile references using the same owning scenario's
  real operations. Assert full count and bounded referrer pages, not merely the
  rejection's sample. Exact target/review changes return Stale without clearing
  an old-actor hold, consuming counters twice or overwriting intervening values.
- [x] Run the focused store test before implementing the checked reference reads
  and retry. Guard the exact record/review in one immediate transaction, recheck
  accepted referrers, and only then reuse core resolve_review with Deleted to
  rebase that explicit deletion for normal preparation. Do not reserve a new
  proposal counter here; the normal preparation transaction does that later.
  Use the same store path as guarded resolution, without nested transactions.
- [x] Present both deletion/edit orientations as explicit record-state choices.
  Keep a pending deletion hidden from ordinary history until an explicit live
  result is saved. Cancelling a pending tag deletion does not undo separately
  saved QSO effects. Missing references require explicit removal/replacement;
  never resurrect a tag or silently delete an unexpected referring QSO.
- [x] Present Referenced with total count, affected records, Refresh references,
  Retry deletion and the permitted keep-live target choice. Repairs awaiting
  acceptance remain visible as such. References changing after retry are handled
  by the ordinary server acceptance/rejection path, not a background retry loop.
- [x] Add `reauthorized_work_requires_usable_explicit_review` to the existing
  enrollment/recovery scenario: held work before comparison is visible but cannot
  save; usable catch-up creates ExcludedActor review; explicit resolution releases
  only that matching hold. Unrelated records continue. Reuse the current hold
  and completed-receipt transitions; do not rename old vectors/prepared bytes.
- [x] Exercise MissingOriginal and OrderingUnknown using existing actual recovery
  fixtures. Assert truthful explanations, no invented three-way attribution, and
  saving only when usable compared shared state exists. Keep retained accepted
  alternatives accessible and preserved by the native transition. Noneditable
  record kinds show safe diagnostics instead of a fabricated general editor.
- [x] Add a focused UI test for opening the correct referring record and issuing
  the exact Refresh/Retry request. Run affected store/desktop tests, regenerate
  SQL metadata and commit with context about shared references and explicit
  operator repair. Update proposal-derived docs to describe delivered behavior.

## Task 5: Verify the complete production workflow and finish documentation

**Deliverable:** working window-to-store-to-sync behavior, assembled visual
verification and current portable documentation. This task adds no subsystem.

**Files:** extend `session/tests/settings.rs` (or a focused tests/review.rs child)
and the existing authenticated sync test host used by
`lib/clacks-desktop/src/synchronization/tests.rs`; update desktop/store READMEs,
product/UI/sync docs and the integration proposal's status. Do not create an
in-repository acceptance/review report.

- [x] In the existing real session fixture, resolve a QSO then process an actual
  subsequent settings-refresh failure; assert the committed result remains
  authoritative and reopening reads the saved pending value. Close/navigate
  after admission and assert the owning write result is not cancelled or applied
  to another draft. Do not invent failpoints or injected DB behavior to provoke
  the refresh failure; use an existing ordinary failure case or verify result
  handling at the typed component boundary instead of manufacturing one.
- [x] Extend the existing authenticated endpoint scenario to run the production
  synchronization loop awakened by a real session resolution. Assert acceptance
  settles that pending row and an unrelated edit progresses while another review
  remains unresolved. Reuse the disposable PostgreSQL/enrollment setup; do not
  replace WebSockets, authentication, or add a fake transport.
- [x] Run the focused tests and checks below, then the existing workspace checks
  with disposable PostgreSQL if available. Read the machine-local inventory
  before touching server configuration; record unavailable checks explicitly.
- [ ] Inspect the production executable with real local working fixtures created
  through project tooling: both sections populated/empty/collapsed, untagged and
  duplicate names, custom values, clears, combined memberships, deletion/referrer
  repair, stale refresh, busy/error feedback and reopening after save. Check
  light/dark appearance, widths, full field widths, section framing, typography
  and keyboard behavior. The workbench remains usable and its draft unchanged.
  macOS inspection covered dark/light presentation, navigation, combined values,
  pending details, save/reopen and preserved workbench input. The complete narrow
  and rare-recovery visual matrix and native Windows acceptance remain unchecked.
- [x] Check telemetry using the existing configured test subscriber at the owning
  boundary: safe record IDs/reasons and save outcomes, no callsigns/comments/
  resolved payload dumps. Do not add generic operation metrics or retest tracing.
- [x] Reconcile current docs and usage links. Keep D382–D387 as agreed decisions;
  identify defaults adopted during this implementation as implementation choices,
  not invented user approvals. Retain useful contract content at stable paths.
- [x] Audit changed tests for a concrete Clacks requirement and failure they catch;
  audit changed production APIs for real callers, Rust idiom and redundant
  framework machinery. Run the branch review required by native execution and
  fold any fixes into the appropriate substantive commits.
- [x] Before final delivery, inspect the actual diff and commit messages. Document
  native macOS evidence and unavailable Windows acceptance separately. Windows
  tests remain disabled; do not reactivate CI or promise results while the stated
  monthly CI limit prevents runs. Hold branch push/PR until requested.

## Verification commands and evidence

Run focused checks after the owning changes. These are commands to execute during
implementation, not checks claimed to have passed by writing this plan.

```sh
SQLX_OFFLINE=true cargo test -p clacks-client-store --test integration review_ --locked --offline
SQLX_OFFLINE=true cargo test -p clacks-client-store --test integration resolution_ --locked --offline
SQLX_OFFLINE=true cargo test -p clacks-client-store --test integration blocked_deletion_ --locked --offline
SQLX_OFFLINE=true cargo test -p clacks-client-store --test integration reauthorized_ --locked --offline
SQLX_OFFLINE=true cargo test -p clacks-desktop --lib review --locked --offline
cargo fmt --all --check
SQLX_OFFLINE=true cargo clippy -p clacks-client-store -p clacks-desktop --all-targets --locked --offline -- -D warnings
RUSTDOCFLAGS='-D warnings' SQLX_OFFLINE=true cargo doc -p clacks-client-store -p clacks-desktop --no-deps --document-private-items --locked --offline
```

For SQL metadata, follow lib/clacks-client-store/README.md with a fresh disposable
SQLite file and SQLX_OFFLINE=false. Use SQLx CLI 0.9.0 and the production client
migrations; do not touch a persistent workspace. From the workspace root:

```sh
cargo sqlx database create
cargo sqlx migrate run --source lib/clacks-client-store/migrations
cargo sqlx prepare --workspace -- --package clacks-client-store --all-targets --locked --offline
cargo sqlx prepare --check --workspace -- --package clacks-client-store --all-targets --locked --offline
```

DATABASE_URL must identify that disposable file. Remove obsolete generated query
metadata, not unrelated server query metadata. Do not hand-edit .sqlx files.
No migration is anticipated by this plan.

## Requirement coverage and final checks

Coverage mapping: proposal sections 1–3 map to Tasks 1–2; review cases in section
4 map to Tasks 2–4; field/editor reuse and validation in sections 5–6 map to Tasks
2–3; application wiring and storage in sections 7–8 map to Tasks 1, 2 and 4;
verification in section 9 maps to Task 5. All five Review focus conditions have
owning assertions. Native core detection remains unchanged rather than being
reimplemented for tests or presentation.

The same ReviewAction/ReviewResponse branch carries every desktop operation;
only Resolve and RetryDeletion are shared writes. Section counts and selected
detail use the page/read interfaces above; Task 4 adds accepted-referrer reads.
No task depends on an undefined generic repository, form registry or server API.
Review and typing remain outside database transactions; saving uses the existing
immediate transaction and conditional acceptance afterward.

For final checks, retain the existing workflow's workspace build, all-target and
documentation tests, clippy and rustdoc checks. Server-dependent tests use the
established disposable database credentials and production migrations; never
claim they ran when skipped. The Rust version comes from rust-toolchain.toml.
Native Windows acceptance, CI availability and user review of the rendered
workflow are explicit verification limits, not reasons to introduce scaffolding.

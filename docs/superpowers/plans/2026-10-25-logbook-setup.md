# Offline Logbook Setup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for native execution, task by task. Steps use checkboxes for tracking. Preserve the user's execution choice; implementation starts only after plan review and authorization. Native execution ends with one fresh whole-branch review.

**Goal:** Deliver #8's production tag-management dialog and SQLite-owned view selection, including atomic fallback when a selected tag is deleted.

**Architecture:** Extend the existing relational client store and its effective-working-record installation path. Add the logbook catalog to the current consistent settings snapshot and use the existing session request lane and watched snapshot for the desktop. Reuse native tag classification, guarded saves, shared fields and dialogs; add no contact browser or second runtime framework.

**Tech Stack:** Rust 1.99.1, egui/eframe 0.37.0, Tokio, SQLx 0.9.0, bundled SQLite, tracing and metrics; existing workspace dependencies only.

**Spec:** [Offline logbook setup and view selection](../../logbooks.md). Read it with this plan. The [desktop UI](../../desktop-ui.md), [client store](../../client-store.md) and [observability](../../observability.md) contracts also apply.

## Global constraints

- #8 owns tag setup, policies, selection and reviewed tag deletion. #9 owns contact entry/basic list; #10 owns extended history/filtering/Worked Before; #51 owns ordinary individual/selected-contact membership editing. Do not implement those later interfaces as scaffolding.
- Keep Rust 1.99.1 and `rust-toolchain.toml` as the toolchain source. Reuse workspace-inherited dependencies and the thin `bin/clacks-log` application.
- Preserve account-owned QSOs, full TagIds, whole-record reconciliation, and the three atomic tag-deletion effects. TagIds use the existing typed Universally Unique Lexicographically Sortable Identifier (ULID) values. Built-in views are not synchronized tags.
- Preserve SQLite write-ahead logging (WAL) with synchronous=FULL, SQLx-managed forward-only migrations, immediate writes and ordinary read snapshots. No custom writer, extra schema ledger, workspace marker or generation counter.
- Keep fixed SQL in separate files with `query_file!`/`query_file_as!` and committed `.sqlx` metadata. Use named row types/fields, not record-like tuples.
- Use focused modules, no `mod.rs` outside test directories, no production `expect`/`unwrap`, direct collection indexing or facade/glob re-exports. Document production items with standalone rustdoc.
- Reuse native tracing spans, structured domain events and operation-owned metrics. Avoid telemetry-only wrappers, duplicated lifecycle/timing code and payload logging.
- Use Rust-only fixtures and noninteractive automated tests. No Python, xtask, TQSL execution, shipped delay controls or manual cargo-test harnesses.
- Preserve the unrelated `references/desktop-toolkit.md` working change. Make an isolated execution worktree from the documentation commits, using the worktree skill; do not reset the main checkout to prepare execution.
- Commit independently reviewable task results. Prepare each substantive message in a file, inspect it, and hard-wrap prose at 72 columns before `git commit --file`. Follow this procedure for amendments too. Do not push or create a PR without authorization.

## Review focus

1. A tag becomes unavailable/deleted between catalog display and selection: never persist an invalid target or turn a failed read into untagged selection. Task 1.
2. Equal deletion counts conceal different contacts or changed records: stale review commits nothing; successful deletion commits contact effects and selection together. Task 3.
3. An incoming tombstone is staged or conflicts with a live local working value: fallback occurs only when working deletion is effective, including receipt and later resolution. Task 3.
4. A committed operation loses its next read or its dialog closes: preserve exact success/fallback, avoid mutation retries and keep shutdown responsive. Tasks 4 and 6.
5. Duplicate names share the same short suffix, or match a built-in label: display remains unambiguous while every command uses the full ID. Task 5.
6. Actual main-window rendering preserves the accepted application bar, operating strip, palette, control treatment and type hierarchy; default widget styling or label-presence checks do not establish visual conformity. Task 8.

## Production file map

Paths below are repository-relative; organize children by the listed responsibility.

| File(s) | Responsibility |
| --- | --- |
| `lib/clacks-client-store/src/logbooks.rs` | Public `LogbookSnapshot` and module declarations; no facade re-exports. |
| `.../src/logbooks/selection.rs` | SelectedView, persisted selection operations, startup repair and transaction-local deletion fallback. |
| `.../src/logbooks/read.rs` | Crate-private consistent selected-view/tag-role hydration. |
| `.../src/logbooks/tags.rs` | Named-tag save validation using the existing guarded native save. |
| `.../src/logbooks/deletion.rs` | Move the existing reviewed deletion operation here; add preview and exact committed selection result. |
| `.../sql/logbooks/catalog.sql`, `.../sql/local_state/*.sql` | Consistent catalog and shared key/value queries. |
| `.../migrations/0010_local_state.sql`, `.../src/local_state.rs` | Shared key/value preferences with native defaults and transactional writes. |
| `.../src/settings.rs`, `.../src/settings/read.rs` | Include the logbook catalog in the existing consistent snapshot. |
| `.../src/metadata/client.rs`, `.../src/save.rs` | Effective-deletion hook and committed tag-mutation metrics; remove relocated deletion items. |
| `lib/clacks-desktop/src/session/settings/logbooks.rs` | Typed logbook requests/results dispatched within the existing settings lane. |
| `.../src/session/settings.rs`, `.../src/session.rs`, `.../src/workspace.rs` | Existing work admission, authoritative results, startup restoration and snapshot publication. |
| `.../src/logbooks.rs` | Transient native TagEditor; reuse the existing FieldError type. |
| `.../src/ui/logbooks.rs`, `.../src/ui/logbooks/{labels,editor,deletion,responses}.rs` | Real selector/dialog, duplicate-name labels, forms, review and result handling. |
| `.../src/ui.rs`, `.../src/ui/startup.rs` | Integrate current controls and the restored-selection notice. |

Test modules/files are named in their tasks. Keep store/UI modules focused; split a coherent responsibility if a Rust file approaches 600 lines. Do not restructure unrelated code or rename the existing settings channel merely to make it generic.

## Shared SQL and verification procedure

Use the existing target directory when practical; set `CARGO_TARGET_DIR` to the shared repository target explicitly in an isolated worktree. Do not change global Cargo settings.

For each task adding/changing checked SQL, create a fresh scratch database and regenerate/check metadata before the task's final tests and commit. Run from the workspace root; the relative scratch directory works with Git Bash on Windows too:

```sh
mkdir -p target
query_check_dir=$(mktemp -d target/logbook-query-check.XXXXXX)
export DATABASE_URL="sqlite://$query_check_dir/query-check.sqlite"
export SQLX_OFFLINE=false
cargo sqlx database create
cargo sqlx migrate run --source lib/clacks-client-store/migrations
cargo sqlx prepare --workspace -- --package clacks-client-store --all-targets --locked --offline
cargo sqlx prepare --check --workspace -- --package clacks-client-store --all-targets --locked --offline
export SQLX_OFFLINE=true
unset DATABASE_URL
```

Use the existing pinned SQLx CLI. Include generated `.sqlx` additions/removals in the owning task. A missing metadata file, database connection failure or stale fixture is not a meaningful RED test: identify the missing API or failed Clacks assertion. Query preparation itself must succeed before interpreting a functional regression.

Before every task commit, run `cargo fmt --all --check`, `git diff --check`, the named tests and strict Clippy for the changed crate(s). Inspect the actual message file with an overlength-line check, then commit only the task's paths and metadata. Record failures/limitations truthfully; do not repeat successful broad checks without a new reason.

---

## Task 1: Persist selected views and expose a consistent catalog

**Files:** Create the store `logbooks.rs`, `logbooks/{selection,read}.rs`, shared `local_state.rs`, migration `0010_local_state.sql`, `sql/local_state/{get,set}.sql` and `sql/logbooks/catalog.sql`. Modify `src/{lib,settings}.rs` and `src/settings/read.rs`. Test in `lib/clacks-client-store/tests/logbooks.rs` with private `tests/support/logbooks.rs` fixtures and `tests/sql/invalid_selected_view.sql`. Verify native preference defaults and preservation rather than SQLx journal versions, commits or retries. Update the two existing desktop `SettingsSnapshot` literals in `src/operating/tests.rs` and `src/ui/tests/main_screen.rs` with explicit empty logbook snapshots.

**Interfaces:**

```rust
// clacks_client_store::logbooks::selection
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectedView { AllContacts, Untagged, Logbook(TagId) }
pub async fn select(store: &ClientStore, next: SelectedView)
    -> anyhow::Result<SelectedView>;
pub async fn restore(store: &ClientStore) -> anyhow::Result<Option<TagId>>;
pub(crate) async fn load_in(connection: &mut SqliteConnection, account: AccountId)
    -> anyhow::Result<SelectedView>;
pub(crate) async fn fallback_deleted_in(connection: &mut SqliteConnection, key: RecordKey)
    -> anyhow::Result<()>;

// clacks_client_store::logbooks
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogbookSnapshot {
    pub selected_view: SelectedView,
    pub tags: Vec<ClientRecord>,
}
// Add `pub logbooks: LogbookSnapshot` to settings::SettingsSnapshot.
// logbooks::read (crate-private)
pub(crate) async fn load_in(connection: &mut SqliteConnection, account: AccountId)
    -> anyhow::Result<LogbookSnapshot>;
```

`restore` returns the deleted TagId only when it actually commits a repair. `fallback_deleted_in` is an internal transactional side effect, not a post-commit event or public test hook. Do not add standalone public catalog/default-tag convenience APIs without a production caller; the desktop uses `settings::snapshot` and `ReadSnapshot::settings`.

- [ ] Write failing `fresh_workspace_defaults_to_all_contacts` and `selected_view_is_local_durable_and_snapshot_consistent` tests using real ClientStore create/open/close. Assert AllContacts initially, Untagged survives reopen, choosing a live tag survives rename/reopen by ID, another database stays independent, and an already-open ReadSnapshot retains its old selection/catalog while a fresh snapshot sees the committed change.

```rust
assert_eq!(settings::snapshot(&store).await?.logbooks.selected_view,
    SelectedView::AllContacts);
selection::select(&store, SelectedView::Untagged).await?;
store.close().await?;
let reopened = ClientStore::open(&path, id(1), StoreSettings::default()).await?;
assert_eq!(settings::snapshot(&reopened).await?.logbooks.selected_view,
    SelectedView::Untagged);
```

- [ ] Write `selection_rejects_deleted_or_unavailable_targets_without_replacing_current_view`: keep a valid selected tag, delete another target through the existing store before selection, and also try a missing ID. Both operations fail and the saved choice stays unchanged. Test `restore` preserves an unresolved/missing selected target with a test-only database fixture; only a known effective working deletion repairs it and returns Some(old_id).
- [ ] Write `invalid_selected_view_does_not_reset_the_workspace`: store an invalid serialized native value under the selection key through a test-only SQLite fixture. Snapshot/restore must report an error; inspect the row afterward and verify it was not replaced with AllContacts and existing native records/account identity remain. Do not add a production corruption hook or another startup marker.
- [ ] Run `cargo test -p clacks-client-store --test logbooks --locked --offline`; confirm the specific missing API/assertion failures. Do not accept SQL metadata errors as the expected failure.
- [ ] Create a shared STRICT `local_state` table with `key TEXT PRIMARY KEY NOT NULL` and `value TEXT NOT NULL`. Use ordinary Serde JSON for the native SelectedView under `logbooks.selected_view`; default an absent key to AllContacts and reject invalid stored values in Rust. Do not seed a required setting row or create selection-specific database constraints. Use shared get/set query files, with parameterized key lookup and upsert inside the caller's transaction; preserve unrelated keys. Selection holds a TagId, not a baseline/payload copy, so it introduces no new payload-retention root.
- [ ] Implement selection/restore with ordinary immediate transactions. Validate named targets against their actual live working Tag record. Return the committed value, emit one focused `info` event only for a changed explicit selection, and use the existing error chain on failure. `restore` checks effective working deletion, repairs inside its own transaction and returns its committed notice context. The internal fallback decodes and replaces only a matching selected tag and emits no premature success event.
- [ ] Hydrate selection and all current tag ClientRecords in the existing settings snapshot's single read transaction, including tombstones/unavailable roles. Do not hydrate QSOs or add an extra watched cache. Read queries stay crate-private; UI fixtures construct the explicit new field rather than adding a production Default solely for tests.
- [ ] Regenerate/check SQL metadata; run the Task 1 tests, existing `opening`/`settings` store tests, store unit tests (`cargo test -p clacks-client-store --lib --locked --offline`) and desktop test compilation (`cargo test -p clacks-desktop --no-run --locked --offline`). Verify enrollment emptiness is unchanged by client-local selection storage.
- [ ] Commit with subject **Persist workspace-local logbook view selection**, explaining why view state is workspace-local key/value data and why unavailable targets are not defaults. Use the shared checked-message procedure.

## Task 2: Save named tags through a typed production boundary

**Files:** Create `lib/clacks-client-store/src/logbooks/tags.rs`. Modify the logbook module declarations and `src/save.rs`. Extend `tests/logbooks.rs` and `tests/support/logbooks.rs`.

**Consumes:** Task 1's SelectedView and catalog through `settings::snapshot`; existing LogbookTag, `save::SaveOutcome`, `save::save` and full ClientRecord bases.

**Produces:**

```rust
// clacks_client_store::logbooks::tags
pub async fn save(store: &ClientStore, expected: Option<&ClientRecord>, next: &LogbookTag)
    -> anyhow::Result<SaveOutcome>;
```

- [ ] Write `tag_save_preserves_identity_selection_and_pending_basis`: create two Main tags with distinct fixed IDs and Neither/Neither policies; duplicate names succeed. Rename one and change only its QRZ policy to Forbid; its ID, current selected ID, unrelated tag, operating state and existing QSO facts/evidence remain intact. A repeated identical save returns Unchanged without allocating fresh pending evidence.

```rust
let SaveOutcome::Saved(record) = tags::save(&store, None, &tag).await? else {
    panic!("a fresh tag must be saved");
};
assert_eq!(record.key.id, RecordId::Tag(tag.id));
assert!(matches!(tags::save(&store, Some(&record), &tag).await?,
    SaveOutcome::Unchanged(_)));
```

- [ ] Write `tag_editor_save_rejects_blank_names_and_stale_or_mismatched_bases`: whitespace-only name fails without a write; valid nonblank text is preserved exactly. Intervening changes yield Stale; an expected row for a different tag/account fails rather than changing its identity. Do not add a uniqueness rule, arbitrary length cap or read-time normalization.
- [ ] Use the existing scoped telemetry test support to assert the actual Task 2 create/update increments are one each, while Unchanged/Stale/failed saves add zero. Keep the tag name as a recognizable exclusion sentinel. These assertions must fail before adding the tag-counter branch; do not defer all instrumentation coverage until Task 7.
- [ ] Run the focused tests and observe RED. Implement the typed validation/delegation to the existing guarded save; no alternate synchronization record builder or redundant error wrapper. Give the function caller-facing rustdoc and a meaningful `#[instrument(skip_all, ...)]` span using direct IDs.
- [ ] Extend the existing committed native-save metric classification with `clacks_client_tag_mutations_total{change=create|update|delete}` for tag saves. Use structured native identity/change fields on the existing successful-save event rather than another generic completion event. Count Saved only, after commit; do not label by name/ID or double-count Unchanged/Stale.
- [ ] Run Task 2 tests plus existing `settings` and `tags` tests. Commit as **Save account logbook tags without changing the active view**, with context about ordinary pending bases and duplicate names.

## Task 3: Integrate reviewed deletion and atomic fallback across store paths

**Files:** Create `lib/clacks-client-store/src/logbooks/deletion.rs`; move `sql/save/members.sql` to `sql/logbooks/members.sql`. Modify `src/{save,metadata/client,logbooks/selection}.rs`. Update `tests/tags.rs`; add `tests/logbook_deletion.rs`, `tests/logbook_transfer.rs`. Reuse the existing Rust transfer fixtures and add only private, scenario-specific support.

**Consumes:** Task 1 selection helpers; existing `plan_tag_deletion`, guarded save_in, client role storage and transfer/receipt/resolution APIs.

**Produces:** Move the existing public deletion operation/type from `save` into its owning `logbooks::deletion` module; update all repository callers instead of facade re-exporting it. Preserve its algorithm and strengthen its result:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagDeletionCounts {
    pub affected: usize,
    pub also_tagged_elsewhere: usize,
    pub unique: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagDeletionPreview {
    pub tag: ClientRecord,
    pub members: Vec<ClientRecord>,
    pub counts: TagDeletionCounts,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TagDeleteOutcome {
    Applied {
        plan: TagDeletionPlan,
        selected_view: SelectedView,
        fallback_from: Option<TagId>,
    },
    Stale { current: Option<Box<ClientRecord>> },
}
pub async fn preview(store: &ClientStore, id: TagId)
    -> anyhow::Result<TagDeletionPreview>;
pub async fn delete_tag(store: &ClientStore, expected: &TagDeletionPreview,
    choice: TagDeletionChoice) -> anyhow::Result<TagDeleteOutcome>;
```

Compute counts once on the session/store worker by delegating to the native DeleteUnique plan: affected.len(), also_tagged_elsewhere and mark_deleted.len() give the three fields. Drop the temporary plan after extracting counts. The UI renders those small counts rather than cloning/classifying the complete member set on every frame. Commit revalidation still uses the exact tag/member records and a fresh native plan for the chosen effect. Do not add a redundant public preview.plan wrapper, test-only accessor or alternative classifier.

- [ ] Adapt the existing three-choice tests to the owning API. Add preview assertions for 100 distinct active contacts, 30 shared: LeaveAll deletes 0, DeleteAll deletes 100, DeleteUnique deletes 70. Select tag id(20) before preview and assert the read did not change it. Use smaller real-store fixtures for the three deletion choices so verification does not multiply costly 100-record setup unnecessarily. Pending deletions are excluded from active membership counts.

```rust
let preview = deletion::preview(&store, id(20)).await?;
assert_eq!(preview.counts, TagDeletionCounts {
    affected: 100, also_tagged_elsewhere: 30, unique: 70,
});
assert_eq!(settings::snapshot(&store).await?.logbooks.selected_view,
    SelectedView::Logbook(id(20))); // Preview made no selection change.
```

- [ ] Add `unchanged_counts_do_not_validate_changed_preview_members` and `unrelated_fact_edit_invalidates_exact_deletion_basis`: substitute one member or change its recorded call while keeping totals identical. Assert Stale, unchanged tag/QSO/selection rows and no mutation metrics. New, removed and newly shared contacts must also invalidate the reviewed basis.
- [ ] Verify successful deletion persists the expected QSO states, tag tombstone and selected-view fallback together after reopen. Verify changed tag/member inputs reject a stale review without changing any of them. Use the production schema and normal public operations; do not inject triggers or manufacture late SQL failures.
- [ ] Run these tests to observe RED. Move/reuse the existing deletion implementation: one immediate transaction re-reads tag plus the full member map, compares exact records, delegates classification, commits all effects and returns the authoritative selection/fallback fields. A stale outcome commits nothing. Current selection is read inside the transaction and is not part of the preview guard: changing views alone must not invalidate contact impact or reset an unrelated selection.
- [ ] Call `selection::fallback_deleted_in` beside the existing source-selection cleanup in `metadata::client::store`, only when the effective local working state is Deleted, covering both changed and idempotent store branches. It must first match RecordId::Tag, then qualify by account/full TagId: deleting a QSO with the same ULID bits must not clear tag selection. Add that cross-kind case to the deletion tests. The helper uses the caller's connection/transaction and does not open a pool write or emit an uncommitted success event.
- [ ] Add real `transfer::install`, `synchronization::receive` and `resolve` cases: effective selected-tag deletion persists AllContacts; an incoming tombstone with a live pending tag alternative, staged input and unknown/absent reads do not force fallback. A pending QSO referencing the deleted tag retains its working membership and explicit-repair review. Installing an incoming tag deletion does not replay local LeaveAll/DeleteAll/DeleteUnique over unrelated local QSOs.
- [ ] Use actual prepared/accepted fixtures where an update can become effective through receipt; assert selection, record and window state after reopen. Verify replay is idempotent and conflicting live alternatives preserve selection. Audit all `client::store` call paths so baseline/projection or later resolution cannot bypass the shared hook.
- [ ] Preserve `tag.deleted` and the existing committed QSO counters, add the one tag-delete count after commit, and record local fallback cause/IDs with the actual operation result. No fallback metric or telemetry result wrapper. Run updated tags/deletion/transfer tests; regenerate metadata and commit **Keep tag deletion and selected-view fallback atomic**, explaining why installed working state, not raw tombstones, owns cleanup.

## Task 4: Carry logbook operations through the existing desktop session

**Files:** Create `lib/clacks-desktop/src/session/settings/logbooks.rs`. Modify `src/{workspace,session,ui}.rs`, `src/session/settings.rs`, `src/ui/startup.rs` and `src/ui/operating/responses.rs`. Update Ready constructors/matches in `src/{session/tests,session/settings/tests,telemetry/tests/session,ui/operating/tests/support,ui/tests/main_screen}.rs`. Add focused tests under `src/session/settings/logbooks/tests.rs`; test modules use ordinary named Rust paths.

**Consumes:** Tasks 1–3, `SettingsSnapshot.logbooks`, existing SettingsRequest/SettingsResponse, Completion and the single admitted-work JoinSet.

**Produces:** A nested domain action/result within the existing settings lane:

```rust
pub(crate) enum LogbookAction {
    SelectView { next: SelectedView },
    SaveTag { expected: Option<Box<ClientRecord>>, next: LogbookTag },
    PreviewDeletion { id: TagId },
    DeleteTag { expected: Arc<TagDeletionPreview>, choice: TagDeletionChoice },
}
#[derive(Debug)]
pub(crate) enum LogbookResponse {
    Selected(SelectedView),
    Saved(Box<SaveOutcome>),
    DeletionPreview(Arc<TagDeletionPreview>),
    Deleted(TagDeleteOutcome),
}
pub(crate) async fn execute(store: &ClientStore, action: LogbookAction)
    -> anyhow::Result<LogbookResponse>;
// Add SettingsAction::Logbooks(LogbookAction) and
// SettingsResponse::Logbooks(LogbookResponse).
// Add restored_view_from: Option<TagId> to OpenedWorkspace and SessionStatus::Ready.
```

The desktop's Arc shares the potentially large immutable preview between its dialog and admitted worker; it does not change the store API or clone all member records on a UI-frame/confirmation path. Use the existing std::sync::Arc, not a new ownership abstraction.

Box the owned tag-save basis and save result in these enums so selection and
preview messages do not carry the size of a complete client record. This is
ordinary Rust ownership indirection; the store signatures remain unchanged.

- [ ] Write `session_logbook_operations_publish_committed_catalogs` against a temporary real workspace: submit save, select, preview and delete through the production request lane. Assert exact native results, snapshot selection/tags, and persisted values after close/reopen. No second settings/logbook queue or watched catalog is introduced.
- [ ] Exercise save/select/preview/delete through the production session and verify the committed outcomes agree with published catalogs and reopened store state. Do not alter the schema to force follow-up read errors.

```rust
let SettingsResponse::RefreshFailed { outcome, .. } = response else {
    panic!("the committed operation must survive its failed refresh");
};
let SettingsResponse::Logbooks(LogbookResponse::Saved(outcome))
    = *outcome else { panic!("preserve the exact tag-save result"); };
let SaveOutcome::Saved(record) = *outcome else { panic!("preserve saved values"); };
assert_eq!(record.key.id, RecordId::Tag(tag.id));
assert_eq!(save::load(&store, record.key).await?, Some(record));
```

- [ ] Write `dismissed_logbook_requests_drain_during_shutdown`: drop the originating receiver after admission, request close, and reopen the actual database. Admitted writes commit/roll back normally; queued/not-started requests receive Closing. Test session state, not a new production delay flag or interactive ignored test.
- [ ] Observe RED, then add the nested domain dispatch to the existing execute function and reuse its follow-up snapshot/error path. Existing operating-editor response matching treats logbook results as another domain's results without consuming them. Keep ordinary control flow and the existing runtime ownership intact.
- [ ] During workspace open, call `selection::restore` before loading the initial settings snapshot; return its optional deleted-ID notice in OpenedWorkspace/Ready. Follow existing pool-close behavior if either read/repair fails. An unavailable selected target remains selected and is not a workspace reset. Update Ready matches/tests with the explicit field or `..` where appropriate.
- [ ] Keep snapshot publication ordered with replies using the existing completion path. Add no timer, custom generation guard or new cache invalidation layer. The existing main settings snapshot now contains the authoritative logbook catalog and selection.
- [ ] Run session/workspace tests and existing operating-settings regressions. Commit **Route logbook setup through the desktop session**, explaining admitted-work ownership and authoritative outcomes after read failure.

## Task 5: Deliver the selector and named-tag editor

**Files:** Create `lib/clacks-desktop/src/logbooks.rs`, `src/ui/logbooks.rs`, `src/ui/logbooks/{labels,editor,responses}.rs` and their focused private tests. Modify `src/lib.rs` for the crate-private form module and `src/ui.rs` for rendering/ownership. Use `src/ui/tests/logbooks.rs` for actual App-level interaction tests.

**Consumes:** Task 4 actions/results and the existing dialog, field, validation, theme and feedback components. Reuse `crate::operating::FieldError`; do not reorganize validation types solely to support this form.

**Produces:**

```rust
// crate::logbooks (crate-private)
pub(crate) struct TagEditor {
    pub id: TagId,
    pub name: String,
    pub automatic_upload: AutomaticUploadPolicies,
}
impl TagEditor {
    pub fn new(id: TagId) -> Self;
    pub fn from_tag(tag: &LogbookTag) -> Self;
    pub fn build(&self) -> Result<LogbookTag, Vec<FieldError>>;
}
// ui::logbooks::labels (private to the UI workflow)
pub(super) fn labels(tags: &[&LogbookTag]) -> BTreeMap<TagId, String>;
// ui::logbooks
#[derive(Default)]
pub(super) struct LogbooksUi { /* private view/form/request ownership */ }
impl LogbooksUi {
    pub fn observe(&mut self, snapshot: &LogbookSnapshot,
        restored_from: Option<TagId>);
    pub fn show(&mut self, ui: &mut egui::Ui, snapshot: &LogbookSnapshot,
        sender: &mpsc::Sender<SettingsRequest>);
}
```

`observe` is real catalog/form-invalidation and startup-notice handling, not a telemetry-only helper. It must not silently replace editor values/bases. Its observed snapshot is presentation state, not another persisted selected-view authority. Exact private frame structs remain implementation choices; keep requests/results owned after dialog dismissal as in the existing workflow.

- [ ] Write builder/label tests with actual native TagIds: new fields are Neither/Neither; blank names fail; other supplied text is preserved. Two Main tags get suffixes; a unique name has none. Use fixed IDs `00000000000000000000000001` and `00000000000000000100000001` for a final-eight-character collision: their labels extend to `Main · …000000001` and `Main · …100000001`. Also cover duplicate names whose eight-character suffixes are already distinct. Sort by actual name/full ID; renaming removes an obsolete duplicate suffix without changing selection identity.

```rust
assert_eq!(editor.automatic_upload.lotw, AutomaticUploadPolicy::Neither);
assert_eq!(editor.automatic_upload.qrz, AutomaticUploadPolicy::Neither);
let captions = labels(&[&first, &second, &unique]);
assert_ne!(captions.get(&first.id), captions.get(&second.id));
assert_eq!(captions.get(&unique.id).map(String::as_str),
    Some(unique.name.as_str()));
```

- [ ] Add actual App rendering/input tests: built-ins and named logbooks form distinct groups, including a real tag named All contacts. Select the named tag by its full ID and assert SelectedView::Logbook(id), then select the built-in and assert AllContacts. Do not add a dummy QSO draft or table to demonstrate the default.
- [ ] Add `manage_logbooks_save_does_not_select_and_cancel_does_not_save` with a real session/store: create/rename/set independent policies through native controls. Save changes the tag only, Cancel/close/Escape discard unapplied edits, and successful saves survive reopening the workspace. Saving one tag must not change station/profile/equipment state.
- [ ] Run the focused tests to observe RED. Build the compact selector with Manage logbooks beside it in the existing main screen's application area, separate from station/equipment controls. Use real built-in choices and available working tags; a saved unavailable selection is visibly unavailable rather than replaced. Show pending/failed selection distinctly, prevent duplicate submission, and update the committed caption from the actual response/snapshot.
- [ ] Implement duplicate labels from the complete available catalog before any menu filtering. Format duplicate names as `name · …suffix`; start at eight trailing characters and extend collisions up to the full ID (omit the ellipsis when the whole ID is shown). Use checked/iterator access rather than indexing/expect. Unique names do not acquire suffixes. Full IDs are visible/copyable in Manage logbooks using the native clipboard UI facility; they do not become stored name text.
- [ ] Implement list/New/Edit views inside the dedicated native management dialog with independent Save/Cancel boundaries. The editor shows name and separate LoTW/QRZ Enable/Forbid/Neither choices with concise explanation. Reuse debounced validation, edited/error highlights, disabled invalid Save and submission revalidation. Save never opens the tag. Do not add a permanent panel, automatic creation rules or a fake unavailable Delete control; deletion arrives as a real action in Task 6.
- [ ] Route requests/results through Task 4 and preserve a successful Selected/Saved result when catalog reload fails. Refresh errors stay visible; ordinary success is quiet. Preserve editor input on Stale/Failed and require explicit reload/review instead of autosaving over changed state. Duplicate names remain allowed.
- [ ] Run focused form/UI tests plus existing Appearance focus, operating summary/inline-power and main-screen regressions. Commit **Add the production logbook selector and management editor**, explaining the separation between saving a tag and choosing a view.

## Task 6: Complete deletion review, fallback presentation and dismissed results

**Files:** Create `lib/clacks-desktop/src/ui/logbooks/deletion.rs`, `src/ui/logbooks/{deletion_tests,refresh_tests}.rs`. Extend the workflow's responses and tests plus existing main-screen tests. Keep deletion-specific form/request state here rather than growing a single UI module over 600 lines.

**Consumes:** TagDeletionPreview.counts, Task 3's TagDeleteOutcome, existing session result ownership and Task 5's LogbooksUi.

**Produces:** The actual Delete action, explicit choice/confirmation, stale review reload, and committed-fallback presentation. No new public helper API is required.

- [ ] Write `deletion_review_uses_distinct_counts_and_requires_an_explicit_choice`: render a real preview and check Leave all/Delete all/Delete unique totals. Confirmation is disabled until a choice is explicit; Cancel leaves tag/contacts/view unchanged. For populated data, drive real production store/session operations; do not insert a test harness into the application.
- [ ] Write `stale_deletion_preview_requires_new_confirmation`: mutate a member between preview and submission. The first response commits nothing; the refreshed review retains useful input but requires an explicit new confirmation. A deleted/unavailable tag shows its actual condition; there is no automatic recreation or destructive retry.
- [ ] Write `deletion_result_and_fallback_survive_read_failure_or_dialog_dismissal`: the operation commits LeaveAll/DeleteAll/DeleteUnique with selected-tag fallback, but the catalog read fails or the dialog has been dismissed. Keep its exact counts/selected view/fallback notice; do not restore the deleted selection or repeat the write. Ordinary tag Save/Select results from Task 5 retain the same ownership guarantee.
- [ ] Write `effective_deleted_selection_is_presented_once_without_repairing_pending_qsos`: obtain before/after snapshots from a real synchronized store transition and feed the actual UI observation path. Show All contacts plus the explanation only for an observed effective deletion. Raw staging, read failures and a live local working alternative produce no fallback notice. A later explicit selection or dismissed notice is not resurrected on every repaint. Preserve pending QSO membership for #15/#51's actual repair workflow.
- [ ] Observe RED; implement deletion as a separate review, not an immediate tombstone save. Use worker-computed preview counts and exact preview records; do not reclassify/cloned-load member payloads during rendering. Keep contact history out of this dialog; show operation totals and implications, not a temporary contact browser. Respect busy admission and normal graceful shutdown.
- [ ] Apply exact Selected/Deleted results before optional refreshed views. Local Applied provides fallback_from even if the read fails. Initial Ready supplies the startup-repair context. Later catalog updates use the last committed UI selection and effective working tag state to explain the visible fallback, excluding an already-acknowledged explicit selector change. Record a focused `info` presentation event with account/tag/cause when showing that explanation; it reports the observed committed state, not a newly executed mutation. Future network integration publishes its committed catalog through this same observation path.
- [ ] Do not create a durable notification ledger, another saved selection, custom lifecycle manager or new generic Completion type. Incoming store APIs retain their existing native results; the shared installation hook supplies atomic persistence. The UI reads committed state and existing authoritative results.
- [ ] Run logbook deletion/refresh/UI/session tests and existing editor-dismissal/shutdown regressions. Commit **Review tag deletion and present committed view fallback**, explaining exact-basis confirmation and post-commit read failures.

## Task 7: Verify the integrated capability, document it and review the branch

**Files:** Update `lib/clacks-client-store/README.md`, `lib/clacks-desktop/README.md`, `docs/logbooks.md` only for interface/engineering-default reconciliation, and `docs/observability.md` for the actual new metric/event producers. Add focused scenario tests in the owning files above; no implementation-review report or duplicated backlog document.

**Consumes:** The production APIs/UI from Tasks 1–6 and the spec's acceptance table.

- [ ] Add a focused observability test using existing scoped test support: an actual tag create/update/no-op/stale/delete. Assert the tag counter counts only changed committed records with change=create/update/delete, existing QSO deletion counts remain truthful, and no name/payload is emitted. With diagnostic spans filtered, info-level selection/fallback/save/deletion events retain their necessary IDs/outcome. Do not test tracing formatting or repeat every functional case with telemetry disabled.
- [ ] Exercise real logbook management in Light/Dark at 100%/125%/150%/200% scale and 680×440/1024×768/1440×900 logical windows using existing native App test facilities. Assert required labels, errors, policy choices, IDs and actions remain reachable by normal scrolling; do not make pixel-perfect egui tests. Cover keyboard traversal, focus return to the actual Manage opener and copy-full-ID behavior. Reuse existing test contexts/fixtures instead of opening a database for every layout combination.
- [ ] Document shipped APIs and current UI behavior in the crate guides, including exact migration/selection ownership, defaults, duplicate labels, three deletion effects, fallback, and partial follow-up-read failure. Keep D302–D309 and the later #9/#10/#51 boundaries intact. Do not claim that QSO entry, filtering, capture or network synchronization is delivered by this issue.
- [ ] Regenerate/check SQL metadata against fresh production migrations. Run the full verification commands below and inspect every result; no success claims from stale logs.

```sh
cargo fmt --all --check
cargo run --bin clacks-log --locked --offline -- --help
cargo test --workspace --all-targets --locked --offline
cargo test --workspace --doc --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --document-private-items --locked --offline
git diff --check
```

- [ ] Run a native smoke check with the actual executable and a disposable workspace: create duplicate Main tags; copy/compare their full IDs; change policies; select one; close/reopen and verify the same ID; rename and recheck; switch both built-ins; cancel edits; delete selected/unselected empty tags and confirm view behavior. Check pointer/keyboard and readable controls in both themes/scales. Populated/incoming deletion is covered by real Rust store/session tests until those actual logging/network consumers arrive. Do not introduce a manual cargo-test harness to manufacture them.
- [ ] Record native Windows acceptance truthfully. If the user has the Windows machine but no session can be provided, supply concise production-executable checks and leave that human observation explicitly pending. Windows/Linux continuous integration (CI) remains required after an authorized push; keep the existing cached/scoped workflow and PR/main triggers.
- [ ] Commit the completed integration/docs/regression evidence with a substantive wrapped message, using the global procedure. Run one fresh whole-branch review under the native execution workflow against this plan, the spec, Rust idioms, public-API purpose and observability rules. Fix confirmed findings in their owning commits, verify affected checks, and preserve meaningful task commits rather than adding cleanup-only commits or squashing unrelated work together.
- [ ] Report the local branch, verification results and any native-platform limits. Ask for push/PR authorization only after the actual result is reviewable, unless the user already authorized publication. Once authorized, push/create the PR and monitor Windows/Linux jobs to success, fixing failures in their owning commits and re-verifying affected checks. Keep issue #8 open until implementation and completion evidence are delivered; a plan alone does not close it.

## Coverage and handoff check

| Spec requirement | Owning tasks |
| --- | --- |
| Scope, approaches and existing component reuse | Global constraints; 1–6 |
| Shared key/value SelectedView, migration/defaults and consistent catalog | 1 |
| Immediate committed selection, startup restoration and unavailable-state preservation | 1, 4, 5 |
| Duplicate names, collision-checked labels and full-ID access | 5, 7 |
| Name/policy Save/Cancel and ordinary guarded tag edits | 2, 5 |
| Atomic reviewed deletion, stale rejection and reopened state | 3, 6 |
| Effective deletion through local/received/installed/resolved state | 3, 4, 6 |
| Entry default follows current view; saved contacts unchanged by switching | Typed selection contract in 1/5; document #9 handoff, no fake draft |
| Independent workspaces, account isolation and enrollment preserved | 1, 2 |
| Admitted/dismissed work, read failure and shutdown | 4, 6 |
| Domain events, truthful committed metrics and data exclusion | 1–4, 6, 7 |
| Cross-theme/scale keyboard/native acceptance and final verification | 5–7 |

Before handing over this plan, self-review spec coverage, step clarity, type names/signatures, the six Review Focus cases and proportion. Do not add a separate review-history file. No implementation work is authorized by the existence of this plan.

## Task 8: Restore the agreed workbench chrome before final acceptance

**Consumes:** The accepted Defined sections workbench reference and current real
logbook/operating/profile/equipment workflows. This correction is required before
Task 7's final verification/review; it does not add entry/history placeholders.

**Files:** `src/ui.rs`, `src/ui/{actions,fields,logbooks,theme}.rs`,
`src/ui/operating/{home,summary}.rs`, relevant native App/workflow tests and the
owning desktop UI documentation in `lib/clacks-desktop` and `docs`.

- [ ] Establish layout regressions against the accepted reference: the logbook
  selector and management opener share the application bar; station/profile,
  equipment, operator, inline power and full editing share the operating strip
  immediately below. Default-state saved-management actions do not occupy a
  permanent content panel. Check placement and grouping, not merely label presence.
- [ ] Preserve current Save/Apply/Cancel, quick-change, stale-state, admitted-work
  and inline-power semantics. Keep reusable management accessible through compact
  application chrome and existing editor navigation. Do not replace real workflows
  or fill the future logging region with placeholders.
- [ ] Use the shared visual tokens and normal reflow/scrolling at small sizes.
  Verify wide/default layout proportions and both appearances/scales against the
  reference; compacting margins alone does not establish conformity.
- [ ] Explicitly implement the reference surface palette, recessed/outlined
  controls, relative type roles and primary-action emphasis in the shared native
  foundation. Verify actual App paint output in both appearances, including
  separating labels from value controls. Preserve native widget state handling.
- [ ] Run actual App input tests and existing operating/logbook regressions. Update
  stale descriptions of the stacked main screen and retain the visual reference
  at a stable repository path. Commit with context about restoring the agreed
  composition, then return to Task 7's final verification and whole-branch review.

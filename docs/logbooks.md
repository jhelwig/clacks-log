# Offline logbook setup and view selection

This specification defines issue [#8](https://github.com/jhelwig/clacks-log/issues/8),
**Support offline logbook tag setup and selection**. It consolidates the account-owned
tag model and decisions D302–D309 into the production desktop/store contract.
It describes required behavior and integration, not an implementation-status report
or a task-level implementation plan.

The [product contract](product.md#contacts-logbooks-and-tags) owns contact and tag
semantics; the [desktop UI contract](desktop-ui.md) owns appearance and common
interactions; the [client-store contract](client-store.md) owns persistence and
synchronization transactions. Those contracts continue to apply.

## 1. Outcome and delivery boundary

An operator can create, rename, configure and delete named logbooks offline,
choose the current contact view, and reopen the workspace with that choice
preserved. A logbook is a tag-defined view of account-owned contacts, not an
owner of separate QSO copies. Contacts can have multiple tags or none.

Use the real egui/eframe desktop, its application-owned Tokio runtime and the
SQLite store. Preserve Rust 1.99.1, the thin application wrapper, production
observability and cross-platform foundations. Windows remains the MVP target.
No server connection, enrollment, provider access or saved station profile is
required for this workflow.

| Capability | Owner |
| --- | --- |
| Named-tag lifecycle, policies, current view, deletion review and atomic effects | #8, this specification |
| New-contact entry, individual contact editing/deletion and the basic saved-contact list | #9 |
| Extend that list with full history, selection, filtering and Worked Before | #10 |
| Individual/selected-contact tag membership editing | #51, after #9/#10 |
| Import-added memberships | #24 |
| Actual synchronization networking and account-wide conflict review | Their Phase 3 capabilities |
| Automatic publication and service routing | Their Phase 5 capabilities |

Deliver the selector and management workflow without placeholder contact panes,
a temporary contact browser or a replacement application shell. #9/#10 consume
the same persisted view and extend the existing application. Include the store
integration for synchronized deletion now, using the existing transfer APIs;
a future network adapter must not implement another fallback algorithm.

Tag deletion retains its chosen QSO effects in #8. Moving ordinary membership
editing into #51 does not remove those effects or restrict deletion to empty
logbooks. Store tests can supply real saved contacts before manual entry is
implemented; those tests are not a demonstration runtime or substitute UI.

## 2. Selected approaches

### Account-owned tags

Reuse `LogbookTag`, `TagId`, QSO tag sets and ordinary whole-record reconciliation.
Independent membership replication, automatic set merging and separate QSO copies
are outside the agreed model. Duplicate names do not merge identities.

### Dedicated management dialog

Use a compact main-window selector with **Manage logbooks…** beside it. Immediate
view selection stays available without a permanent management pane. The dedicated
dialog separates named-tag edits and deletion from routine view changes. It uses
the existing Save/Cancel, validation, focus, dismissal and feedback facilities.

### Client-local SQLite selection

Persist view selection in SQLite, separately from synchronized records and
station/equipment state. Framework preferences were considered, but selection
also determines new-contact default membership and must fall back atomically
with effective tag deletion. SQLite supplies that transaction boundary. Appearance
and other presentation preferences retain their existing ownership.

Use normal SQLx transactions and the existing asynchronous session/store path.
Do not introduce a custom write queue, reactive cache framework, generation
counter, generic repository, second sync model or additional runtime service.

## 3. Values and ownership

### 3.1. Named tags

Reuse the native tag record:

```text
LogbookTag
  id: TagId
  name: text
  automatic_upload
    lotw: Enable | Forbid | Neither
    qrz: Enable | Forbid | Neither
```

The account owns this synchronized record. The name is display data, not identity,
an activity reference, a profile association or a service destination. Use the
complete native ID in commands, persistence and comparisons.

Each new tag starts with Neither independently for Logbook of The World (LoTW)
and QRZ. For one service across a QSO's tags, any Forbid takes precedence;
otherwise at least one Enable grants tag-based automatic-upload eligibility.
All-Neither and untagged contacts have no tag-based grant. These fields configure
later publication; saving a tag does not claim an upload, check credentials or
create an external association. Manual-upload rules remain unchanged.

### 3.2. Selected view

The logical selected-view value has exactly these forms:

```text
SelectedView
  AllContacts
  Untagged
  Logbook(TagId)
```

This is client-local workspace state. It has no synchronized record ID, revision,
causal clock or independent membership identity. It does not belong inside the
station snapshot or operating-profile state.

| View | Later history meaning | New-entry default tag set |
| --- | --- | --- |
| All contacts | Each active QSO once across the account | Empty |
| Untagged | Active QSOs with an empty tag set | Empty |
| Named logbook | Active QSOs carrying its full TagId | That one TagId |

Built-in views are not stored tags and have no tag ID, editable name or upload
policy. Pending deletions remain excluded from normal history. This specifies
what later history reads mean; implementing those contact queries/UI belongs
to #9/#10, not this issue's management screen.

### 3.3. Consistent catalog and preview reads

A purpose-specific catalog snapshot contains the persisted selected view and
complete current tag `ClientRecord` values from one ordinary read transaction.
Retain the roles needed for guarded edits and distinguishing deletion from
unavailable knowledge. Show live working tags as selectable; a selected tag that
is temporarily unavailable needs an explicit unavailable presentation, not a
silent change to All contacts. A failed refresh preserves the last usable view.

Do not hydrate every contact while reading the tag catalog. Only deletion review
needs the complete current member set. Its preview contains named fields for
one expected live tag record and every associated active working QSO record,
read in one consistent snapshot. Close that read transaction after producing the
preview; do not retain a database transaction while the user reviews a dialog.

The store already supplies guarded saves and `delete_tag`; add the catalog and
preview access needed by the real UI. Reuse native values and `ClientRecord`
rather than constructing an alternate copy of synchronization state.

## 4. Selection, restart and entry handoff

### 4.1. Choosing a view

Selecting a view submits a short store operation without a separate Apply
button. A named target must still be a live working tag when the write executes.
A catalog row is not permission to select an interveningly deleted target.

Persist the new selection before reporting it committed. While a request is
pending, distinguish its requested choice from the current committed view and
prevent duplicate submissions using the existing busy-state facilities. On a
failed write, retain the prior committed view and show an actionable error.
Do not claim a successful switch merely because a widget changed locally.

View selection does not edit saved QSOs, station/equipment values, publication
jobs or account synchronization scope. Later history selection follows D297:
switching logbooks clears selected contact IDs, including hidden selections.
No selected-contact implementation or additional contact window is added here.

### 4.2. Restart and unavailable data

- New workspaces and existing workspaces without selected-view state start in
  All contacts.
- Restore the last committed view per workspace. Named views use stable TagId,
  so rename changes the label without losing selection.
- If the remembered tag is known to be effectively deleted, persist All contacts
  and explain why it was restored instead.
- Missing rows, unavailable roles and loading failures are not themselves proof
  of deletion. Preserve the saved choice and present the loading/error state.
- Invalid selected-view storage is an error, not permission to recreate the
  workspace or silently overwrite it with a default.

The effective-deletion predicate uses the store's current local working state.
A received accepted tombstone that leaves a live local working alternative does
not automatically settle that record's conflict or make its working value
Deleted. Staging an incoming tombstone is not installing it. Apply the fallback
when the local working deletion becomes effective, including later resolution.

### 4.3. Deletion during use

When the selected tag becomes effectively deleted, commit its All contacts
fallback in the same transaction. This applies to local deletion and to changes
installed through existing synchronization/receipt/resolution store operations.
Explain the fallback in the relevant UI. Deleting another tag leaves the current
selection alone. A failed transaction changes neither records nor selection.

The fallback changes the view and the default for new contacts. It does not
repair or retag saved pending QSOs. Local deletion still applies the explicitly
chosen QSO effects in section 6; synchronized deleted-tag references retain
D250's explicit removal/replacement workflow and unrelated sync can continue.

### 4.4. Contract consumed by contact entry

The unfinished new-contact entry follows changes to the selected view's default
membership. Do not freeze the prior proposed membership when entry first begins.
For Main → another named logbook, use the new tag; for Main → either built-in
view, use an empty default tag set. Preserve the other entered contact values.
This remains transient form state until #9 validates and saves an actual QSO.

Already saved contacts and existing-contact edits are not retagged by view
changes. The same shared new-entry state remains authoritative across future
entry windows; #8 does not add independent drafts or implement entry windows.

## 5. Manage logbooks workflow

### 5.1. Selection and identity presentation

The compact selector includes both built-in views and live named logbooks.
Manage logbooks exposes the named records and their full IDs for viewing and
copying. Identify edited/deleted targets by the full ID, never by name or row
position. A tag rename preserves all contact memberships and the selected ID.

Append a short ID suffix only where tag display names are duplicates. Unique
names remain unsuffixed. Determine collisions from the current available tag
catalog, not merely from a filtered subset of visible menu rows. Recompute labels
when names or catalog availability change without changing the stored names or
selection. Shortened IDs are display labels, never command identities.

Engineering defaults for the implementation plan:

- Put built-in choices and named logbooks in separate selector groups. A named
  tag called All contacts or Untagged remains a named tag with its own identity;
  the grouping distinguishes it from the built-in choice without reserving names.
- For duplicate tag names, start with the final eight characters of the canonical
  ID. Extend colliding suffixes until each is distinct within that duplicate-name
  group, up to the full ID. Use the same disambiguation in selector and dialog.
- Compare the actual displayed names for this purpose; do not silently case-fold
  or rename stored values. Sort by display name with full ID as a deterministic
  secondary key. Exact label typography follows the shared UI contract.

These are presentation/algorithm defaults, not additional user-approved tag
metadata or identity rules. The full ID remains available independently of them.

### 5.2. Create, rename and configure

Use the existing editor pattern: pending form values, live debounced validation,
Save/Cancel, an edited-state highlight and disabled Save while errors remain.
Submission validates the latest form again. Close/Escape follows Cancel without
an extra discard prompt. Keep errors visible and preserve edits on failure.

Create uses a fresh native TagId and both policies set to Neither. Rename and
policy edits target the existing full tag ID and use its exact expected record
basis. No-op saves preserve pending evidence and produce no mutation count.
A stale save does not overwrite an intervening edit or resurrect a deleted tag;
retain the user's input, explain the stale state and use the established explicit
reload/review workflow. Do not silently turn a failed edit into a new tag.

Engineering defaults for the implementation plan:

- Require a non-whitespace name for explicit creation/rename, matching the current
  profile/equipment forms. Preserve other supplied name text; do not impose a
  new length cap or normalize stored names during reads. Existing values remain
  inspectable even if they would not pass new-entry validation.
- Saving a tag creates/updates that record without opening it or changing the
  selected view. Opening a view remains the selector's explicit operation.
- Give each record editor its own Save/Cancel boundary. A completed record save
  survives later dismissal of Manage logbooks. Moving between management and
  deletion review follows the existing nested-editor rules, not autosave.

### 5.3. Policy editing

Show separate LoTW and QRZ Enable/Forbid/Neither choices with enough explanation
to understand Neither and Forbid precedence. Save the name and policies as one
ordinary tag-record edit. No service connection is needed. Changing one tag
must not rewrite QSOs or infer service success; later publication evaluates its
existing policy and service-evidence rules.

## 6. Tag deletion review and commit

### 6.1. Preview

Read the tag and complete associated active working QSO set consistently. Use
`clacks_model::tag::plan_tag_deletion` for the three effects. Count distinct QSO
identities, never memberships, retained alternatives or multiple payload roles.
Show total associated contacts, those also tagged elsewhere, and each choice's
proposed deleted/preserved totals.

For 100 contacts with 30 also tagged elsewhere:

| Choice | Mark QSO deleted | Preserve QSO and remove this tag |
| --- | ---: | ---: |
| Leave all QSOs | 0 | 100 |
| Delete all QSOs | 100 | 0 |
| Delete only QSOs unique to this tag | 70 | 30 |

The tag is deleted in every choice. Deleting a QSO marks the account-owned QSO
deleted, excluding it from all normal views; it does not turn it into an active
untagged contact. Preserving a QSO while removing its last tag can make it
untagged. External copies, service evidence and deletion memory follow the
existing contracts, including suppression of downloaded-add offers.

Require an explicit choice and confirmation; no destructive choice is silently
selected. Empty tags use the same operation with zero contact effects.
Cancellation before submission changes nothing. Once admitted, the operation
follows the existing session ownership and transaction-completion rules; closing
a dialog does not cancel a committed write or manufacture partial cancellation.

### 6.2. Revalidation and atomicity

Reuse the existing store's `delete_tag` operation. Inside one immediate write
transaction, re-read the current tag and complete member set and compare them
with the exact preview basis. Do not accept equal counts as proof that the
reviewed records are unchanged. The current operation compares full expected
records; unrelated QSO edits can also invalidate the review.

If stale, commit no effects. Refresh the preview and explain the change, requiring
explicit confirmation against the new basis. Preserve applicable user input;
do not automatically retry a destructive operation against unseen records. If
the tag is now deleted/unavailable, report that actual state rather than making
a replacement tag or treating a read failure as successful deletion.

If valid, commit the selected QSO effects, tag working deletion and conditional
selected-view fallback together. Read the actual selected view inside this write
transaction: switching views after preview does not itself make the deletion
basis stale or authorize resetting an unrelated view. Do not split a large tag's
effects into partly visible batches. The accepted transaction/memory cost is
proportional to the complete affected set; no resumable bulk framework is added.

Local tag-management deletion must use this reviewed operation, not a shortcut
that saves a tombstone without the chosen QSO effects. Retain the lower-level
native synchronization contracts and their distinct incoming-deletion behavior.

## 7. SQLite and runtime integration

### 7.1. Shared workspace-local state

Store application preferences in a shared `local_state(key, value)` table in the
existing account workspace database. Each key owns a serialized native value;
selected view uses `logbooks.selected_view` with ordinary Serde JSON enum encoding.
An absent key means All contacts. Invalid stored values are decoding errors and
are preserved rather than silently replaced. Rust owns setting-specific validation;
there are no selected-view columns, database constraints or dedicated table.

This preference storage is distinct from the relational domain records used for
contact queries. It adds no synchronized field, schema-version ledger, workspace
marker or server actor identity. Validate named targets against current native
working records. Update only the selection key inside the same transaction as
effective deletion, leaving all other local-state keys untouched.

### 7.2. Transaction boundaries

| Operation | Transaction and result |
| --- | --- |
| Catalog read | One ordinary read snapshot: authoritative selected view and current tag roles. No eager QSO hydration. |
| Choose view | Immediate write; revalidate a named target, write only selected-view state, commit before successful acknowledgment. |
| Create/edit tag | Existing guarded native-record save; preserve exact expected basis, record identity and ordinary pending evidence. Selection is unchanged. |
| Read deletion preview | One ordinary read snapshot of tag and complete member roles, then release it before UI review. |
| Delete tag | Existing immediate transaction plus the conditional selected-view fallback; all QSO effects, tag deletion and selection commit or roll back together. |
| Effective deletion through receipt, transfer or resolution | Clear the matching selected tag to All contacts within the same store transaction that installs the effective local deletion. |
| Startup repair of a known-deleted selection | Immediate transaction verifies current state, persists All contacts and returns the fallback presentation. Failed reads do not repair. |

Integrate effective-deletion cleanup where client working records are installed,
as the store already does for selected profile/equipment cleanup. Cover both
changed and idempotently reprocessed deleted records. Use the caller's current
transaction; do not open a nested pool write or publish UI changes before commit.
Installing a raw accepted tombstone, a retained alternative or staged input alone
must not bypass the effective-working-state rule.

Preserve write-ahead logging (WAL), synchronous=FULL, normal checkpointing,
automatic forward-only
startup migrations, fixed query files, compile-time SQLx checking and committed
offline metadata. Keep the Rust version in `rust-toolchain.toml` as the source
of truth. No new database service or migration runtime is introduced.

### 7.3. Component boundaries and asynchronous ownership

The model keeps native tags, IDs, policies and pure deletion classification.
A focused client-store logbook module owns catalog/preview reads, selected-view
storage and tag-management operations on top of existing record transitions.
Store row mappers and SQL files follow the existing schema-family organization.

The desktop owns transient tag forms, display suffixes and native UI controls.
Session workers perform store operations off the UI thread and return their
actual outcomes through the existing request/result and snapshot mechanisms.
Use purpose-specific named values. Exact module/file names and whether the
existing request enum is split for readability belong in the implementation plan;
this does not authorize a second queue, generic command bus or cache framework.

Ordinary catalog refreshes do not discard open form edits or advance their
expected bases silently. Preserve the last usable snapshot on failure. A
successful save, selection or deletion remains successful even if the following
catalog refresh fails: retain its exact committed result, distinguish the refresh
failure and offer an explicit refresh. Admitted requests outlive originating
dialog dismissal under the established ownership rules. Shutdown stops new work
and lets in-flight transactions complete or roll back normally.

## 8. Observability

Follow the [production observability contract](observability.md). Use framework
instrumentation and existing domain events without telemetry-only wrappers,
manual lifecycle reconstruction or changes to ordinary control flow.

- Meaningful operation spans cover selection, tag save, deletion preview and
  application. Prefer `#[tracing::instrument(skip_all, ...)]`, native module targets
  and direct structured IDs/choice fields; exclude names, forms and QSO payloads.
- Successful tag creation/update/deletion and explicit selected-view changes have
  focused `info` domain events. An effective-deletion fallback also records its
  cause and affected IDs, with enough context when diagnostic parent spans are
  filtered. Catalog details/preview counts belong at diagnostic levels.
- Stale saves/previews are noncommitting outcomes with an explained bounded
  reason. Runtime error handling logs actionable failures once at its boundary;
  an expected user deletion is not automatically a warning.
- Preserve the committed QSO-mutation metrics already emitted by tag deletion.
  Count changed tag records after commit with the purpose-specific counter
  `clacks_client_tag_mutations_total{change=create|update|delete}`.
  Do not count no-ops, previews, stale outcomes or rollbacks. Avoid double-counting
  if the existing save boundary owns the event/metric.
- No metric is required for every selector change or rendering helper. Do not
  log keystrokes, arbitrary names, paths, form contents or high-cardinality metric
  labels. Libraries install no subscriber or exporter.

Test reporting through real Clacks operations: committed versus stale/failed
outcomes, truthful deletion counts, fallback correlation and deliberate payload
exclusion. Do not test tracing/metrics framework formatting or duplicate the
functional suite solely to exercise configured/unconfigured telemetry.

## 9. Worked workflows

### Duplicate names and restart

Create two tags named Main with different TagIds. Both remain selectable with
distinct suffixes and full IDs in Manage logbooks. Select one and restart: restore
that exact ID. Rename either tag: labels update, but saved selection and contact
memberships keep their original identity.

### Default follows the selected view

Select Main. Later, #9 begins a new-contact form. Switching to another named
logbook changes its proposed default tag; switching to All contacts or Untagged
makes that form untagged by default, preserving the remaining input. A saved QSO
or existing-contact edit does not change membership merely because the view
changes. #8 delivers the persisted selection and notification contract.

### Populated deletion and changed preview

Review a tag with 100 active contacts, 30 also tagged elsewhere. Delete unique
would delete 70 and preserve 30 without this membership. If a contact is added,
removed, edited or receives another tag before submission, the complete expected
basis can become stale even when the displayed totals happen to match. Commit
nothing, reload the preview and require explicit confirmation. A valid retry
commits all effects and any selected-view fallback together.

### Incoming deletion with pending work

A client has a saved pending QSO referencing tag T. Synchronization makes T's
working deletion effective. Persist selected-view fallback if T was selected,
while leaving that pending QSO for the existing explicit reference-repair flow.
Local Leave all/Delete all/Delete unique effects are not replayed merely because
an incoming deletion was installed. Unrelated synchronization continues.

### Post-commit refresh failure

Deletion commits, including fallback to All contacts, but the following catalog
read fails. Keep the successful deletion and exact fallback result visible; show
the refresh problem separately. Do not restore the old selected tag, repeat the
deletion automatically or claim it was rolled back because the read failed.

## 10. Verification and implementation-plan boundary

The implementation plan must establish the following through real production
APIs and noninteractive tests, with native Windows smoke checks using the actual
executable. No manual test harness or project-owned non-Rust fixture generator
is introduced.

| Case | Required evidence |
| --- | --- |
| Fresh/existing workspace without a selected-view row | Forward migration/default produces All contacts without changing existing records or enrollment. |
| Independent workspaces | View choice remains local to each database; no synchronized selection record is emitted. |
| Built-in selection | Persists/reopens without a tag row or upload policy. |
| Named selection and rename | Stable ID survives restart and rename; only label changes. |
| Duplicate names and suffix collisions | Only duplicated names gain suffixes; collisions extend; commands retain full IDs. |
| Tag named like a built-in | Clearly separate choice; selecting it supplies that tag, not the built-in behavior. |
| Invalid creation/rename | Field errors block Save without persistence; correction enables the actual save. |
| New policies | Both services default to Neither; existing Forbid precedence remains intact. |
| Save failure/stale record | Preserve pending input and prior committed selection; no lost intervening edit or silent resurrection. |
| Deletion preview | Distinct active-QSO counts and exact three-choice effects from consistent working values. |
| Changed membership/record with unchanged totals | Stale result commits nothing and requires refreshed review. |
| Mid-operation failure | Tag, all contact effects and conditional selection update roll back together. |
| Selected versus unrelated tag deletion | Only matching selection falls back; choice survives other tag changes. |
| Effective synchronized deletion | Selection and installed deletion commit together; pending QSO references remain for explicit repair. |
| Staged/unknown/non-effective deletion | No fallback from staging, unknown knowledge, failed reads or an unresolved live working alternative. |
| Idempotent deletion installation | Reprocessing does not create extra mutations or lose the persisted fallback. |
| Refresh failure after commit/dismissed editor | Exact operation result survives; no automatic mutation retry or stale-selection restoration. |
| UI integration | Real selector/dialog, validation, Save/Cancel, copied full ID, readable duplicate labels and deletion review work across the existing appearances/scales. |
| Entry/history handoff | Later consumers use the same authoritative selection; no placeholders are required to verify #8. |
| Telemetry | Focused IDs/outcomes/counts match real commits and exclude tag names and payload data. |

The plan chooses exact migration/table/column names, Rust module boundaries,
function signatures, query files, suffix helper organization and test placement.
It must preserve the contracts above and the explicitly identified engineering
defaults, without treating this document as authorization to start coding.

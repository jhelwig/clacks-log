# Contact logbook membership editing

This specification defines [issue #40](https://github.com/jhelwig/clacks-log/issues/40).
It consolidates D243, D249, D250 and D352 with the existing selection, saved-edit
and bulk-operation contracts. It is a specification for review, not an execution
plan or a statement that the interface is already implemented.

**Agreed behavior** is identified below. **Engineering defaults** make the
integration concrete without turning implementation choices into additional
product decisions. They remain reviewable in this document and the later plan.

The [native model](native-model.md#112-tags-and-contact-deletion) owns QSO tag
sets; [client storage](client-store.md#7-transactions-and-operation-boundaries)
owns guarded transactions; [manual logging](manual-logging.md) owns saved-contact
editing; [history](history.md) owns selection and current working rows. The
[desktop UI contract](desktop-ui.md#editing-assigned-logbooks-d352) owns the
accepted interaction. [Logbook setup](logbooks.md) continues to own tag lifecycle,
view selection and tag-deletion effects.

## 1. Outcome and scope — Agreed

An operator can add and remove existing logbook tags on a saved QSO or selected
contacts through the production desktop, entirely offline. Removing the last
tag leaves an active, untagged contact accessible through All contacts and
Untagged. It does not delete the QSO.

Extend the existing saved-contact editor, history selection, shared components,
application runtime and SQLite store. Preserve Rust 1.99.1, egui/eframe with wgpu,
SQLx and cross-platform foundations; Windows remains the MVP desktop target.
These are production extensions, not a separate contact browser or temporary host.

Membership changes preserve QSO identity, recorded facts, directory snapshots,
service evidence and external-record associations. In the individual editor,
other facts change only when explicitly edited in that same form. A bulk
membership operation changes no other QSO values.

The capability covers:

- A compact assigned-logbook field and picker in the saved-contact editor.
- A contextual history action for the selected QSO IDs, including hidden ones.
- One searchable checkbox interaction for both single and mixed membership.
- Guarded local saves, refresh, progress, cancellation and record-specific results.

Tag creation/rename/deletion and upload-policy configuration remain in #8;
new-entry defaults remain in #9; import-added membership remains in #24.
Automatic assignment/creation rules, tag hierarchies, organizational QSO copies,
unrestricted bulk fact editing and bulk QSO deletion are outside this capability.
This does not add synchronization transport, account-wide conflict-review UI,
provider calls or a persistent bulk-job/resumption facility.

## 2. Selected approach and values — Agreed

Use the familiar Gmail-style label checklist, adapted to the existing Clacks
forms and toolbar. The individual editor owns its pending values and Save changes;
the history picker owns a membership-only pending change and Apply. Sharing the
picker does not introduce a second confirmation inside the contact editor.

Reuse account-qualified QSO and Tag IDs. A QSO's complete unordered, deduplicated
tag set remains part of its ordinary record. No independent synchronized
membership record, extra causal identity or special automatic tag merge is added.
Existing whole-record conflict rules apply even when another edit changes only
non-membership facts or different tags.

The view selected in application chrome determines displayed contacts and new-entry
defaults. Editing assigned logbooks is a different operation on saved contacts;
it never switches that view or retags the unfinished entry. Built-in All contacts
and Untagged are not stored tags or assignment choices. Actual named tags retain
their identities even when their names coincide with a built-in view.

Duplicate names remain distinct. Use the existing collision-checked short-ID
suffixes based on the full catalog, not only the search results. Changing a name
or search does not change which Tag ID a checkbox addresses. Full IDs remain
available through Manage logbooks under its existing contract.

## 3. Individual contact workflow — Agreed

Open the saved-contact editor through existing row activation: double-click or
Enter on the focused row. This edits that contact independently of the table's
bulk selection. Opening from Worked Before uses the same editor.

Show a compact **Assigned logbooks** summary with **Edit…** opening the picker.
The summary occupies bounded space, with truncation/overflow indication where
needed. Do not list the account's full catalog in a permanent form section or
allow the summary to grow indefinitely with membership count.

The picker offers search and a bounded scrollable checklist. A single contact's
choices are checked or blank. Changes update the existing pending contact form
and use the shared unapplied-value styling and textual indication. They do not
write to storage as checkboxes are toggled.

Dismissing the picker preserves those pending form inputs. There is no additional
membership Apply, Done or Return with changes action. Escape dismisses an open
picker before acting on its containing editor; dismissal never saves anything.

**Save changes** validates and commits the complete edited QSO, including its tag
set, against the editor's existing expected record. **Cancel**, Escape with no
picker open, or closing the editor discards its unapplied edits. Existing validation,
read-only Saving state, stale-save handling and explicit Reload behavior remain.
Reload replaces the form from the current saved record; it is not an automatic
merge of old membership intent into a new QSO version.

Saving or cancelling this editor leaves the unfinished new QSO and active
operating settings unchanged. Removing all memberships is a valid saved edit.

## 4. Selected-contact workflow — Agreed

History shows **Edit assigned logbooks…** only while at least one QSO is selected,
as part of its contextual selection actions. Do not show an inactive membership
launcher with no selection. Do not append a number to the launcher: the selection
status already provides the selected count. The wording denotes assignment,
rather than changing the displayed logbook or filter.

Open a compact picker from this action. Identify its selected-contact scope,
including selected contacts outside the applied filter. It targets selected IDs,
not the visible page, all query matches or one QSO per membership. Filtering,
sorting and paging retain selection under D293; a view switch clears it under
D297. Show selected and Clear selection retain their shared behavior.

The picker initially represents the membership of the selected working QSOs:

| Checkbox | Membership across the targets | Meaning if left untouched |
| --- | --- | --- |
| Checked | All have the tag | Preserve their membership. |
| Dash | Some have the tag | Preserve each QSO's different membership. |
| Blank | None have the tag | Preserve their absence of membership. |

Choosing checked requests the tag on every target; choosing blank requests its
removal from every target. Other tags remain as recorded on each QSO. This is
not replacement of every target's tag set with the tags common to the selection.

**Apply** submits the chosen membership changes. Before submission, Cancel,
Escape or dismissal discards the pending operation without saving contacts.
Search, checkbox toggles and opening the picker are read-only with respect to
saved QSO data. Disable Apply when the pending choices would change no target.

Use existing progress, cancellation and result feedback after submission. Prevent
duplicate submission and make submitted choices read-only. Cancellation stops
further processing at a safe QSO transaction boundary and retains completed work.
Do not roll back completed saves as though Apply were one account-wide transaction.

After committed changes, refresh history, counts and affected Worked Before
results through the existing invalidation path. A contact may leave the displayed
view/filter after membership changes. Selection still refers to its identity;
hidden-selection counts and Show selected follow the history contract. View
changes do not retarget an operation already submitted.

## 5. Shared picker behavior — Agreed

Use the accepted defined-section styling, palette, typography, fonts, scaling,
focus cues and unapplied/error states. The picker floats without resizing its
containing form or workbench panes. An individual contact's editor remains modal;
opening its picker must not enable interaction with the workbench behind it.

Bound the picker and its scrollable list independently of catalog size. The
interaction must work with hundreds of logbooks accumulated over many years.
Search narrows tag choices, not QSO targets, and must not discard pending choices
for tags hidden by the search. Keep choices and tag identity stable as the user
searches and scrolls. No bespoke expression language is added to this search.

Provide normal keyboard navigation and checkbox activation through the framework's
native facilities. Preserve search input, scroll position and useful focus when
choices change. Distinguish a pending checkbox edit from mere selection/focus.
Do not introduce another visual state system or custom popup framework.

## 6. Local saves, results and recovery — Existing contracts

Re-read and compare each complete expected record inside its committing immediate
write transaction. Use existing native validation, pending records and comparison
evidence. No transaction stays open while the operator edits or decides.

An unchanged value produces no artificial edit. A stale basis produces the
existing explicit stale outcome/current state and does not overwrite saved work.
A missing or deleted QSO is not recreated by assigning tags. Preserve edited
input for the existing refresh/re-evaluation workflow; never silently rebase a
captured full QSO or resolve a synchronization conflict through membership UI.

Bulk processing retains record-specific outcomes, with saved, unchanged, stale,
failed and unprocessed work distinguishable. Do not count an unchanged or stale
result as a committed mutation. Retain completed outcomes if later work fails or
is cancelled, and identify the remaining unprocessed contacts. Reports use IDs
and useful contact context rather than row positions and remain available under
D294 instead of disappearing on a notification timer.

A save and its subsequent read refresh are distinct. If refreshing fails after a
commit, report the refresh problem and offer refresh without repeating the save.
Do not blindly replay a write whose outcome is uncertain; retain the submitted
scope and use the ordinary durable-state/expected-basis checks before retrying.
This does not require a persistent import-like session or new operation-history
schema. After restart, saved working records remain authoritative.

Catalog loading failure is not an empty catalog, an untagged contact or proof
that a tag was deleted. Preserve copied membership and show an actionable read
error. Existing unknown/deleted tag references remain identifiable until explicit
removal/replacement under D250; never silently drop them, resurrect a tag or offer
a known-deleted tag as a new assignment choice. Local saved state and shared
acceptance retain their existing distinct rules.

Changing membership can change tag-based automatic-upload eligibility under the
existing Enable/Forbid/Neither rules. It does not change those policies or submit
to a provider, and it does not invalidate service evidence merely because tags
changed. Later server processing uses its accepted QSO state as already agreed.

## 7. Engineering defaults for review

These are concrete starting choices, not additional product decisions:

- **E1 — Search/presentation:** use literal case-insensitive substring matching
  against the displayed tag label, including its disambiguating suffix. Sort by
  display name with Tag ID as a deterministic tie-breaker. Summarize a small
  bounded number of assigned names plus remaining-count/overflow indication;
  exact dimensions and summary capacity belong to the shared component layout.
- **E2 — Checkbox cycling:** initially mixed choices cycle dash → checked → blank
  → dash, allowing restoration of the original mixed intent before Apply.
  Uniform choices toggle checked/blank. Compare intent with the initial selection
  basis so returning to it creates no edit. This local input behavior is not a
  new synchronization merge rule.
- **E3 — Read/submission basis:** capture selected IDs when the picker opens and
  obtain their working membership/expected-save bases and the tag catalog through
  ordinary snapshot reads. Keep reads bounded to the requested scope and release
  transactions before interaction. Discard obsolete asynchronous reads. A pending
  view change dismisses the unsubmitted picker; submitted work retains its scope.
- **E4 — Write integration:** retain the existing sequential, guarded per-QSO
  transaction boundary. Continue past returned business outcomes such as Stale;
  stop on an operational failure or cancellation, preserving prior outcomes and
  identifying the unprocessed tail. Extend the existing tag-change operation to
  return/report those outcomes instead of losing them behind an error string.

Keep all UI/request state transient within the existing desktop session. Use
existing stable IDs and record comparisons rather than additional save-generation
counters. The implementation plan chooses exact Rust types, module boundaries,
query files, request/cancellation wiring and meaningful verification scenarios.
No new database schema, generic repository, custom write queue or background-job
service is presumed necessary.

## 8. Production integration and observability

Extend the saved-contact form and history/session requests; keep database work
on the existing application runtime, outside egui rendering callbacks. Share the
picker and duplicate-name labeling through their owning components. Reuse normal
SQLx transactions and separate fixed query files with committed offline metadata
where needed. Preserve runtime admission, graceful shutdown and drain behavior.

Follow the [observability contract](observability.md). Existing record-save
instrumentation owns committed QSO mutations. Reuse its actual commit counters;
unchanged/stale/skipped values do not increment them. Runtime handling boundaries
own the operator operation, duration, overall errors and read-refresh distinction.
Report useful selected/changed/unchanged/stale/unprocessed counts with truthful
local outcomes and existing account/request correlation.

Keep metric dimensions bounded; do not use contact/tag IDs or names as labels.
Avoid logging callsigns, tag names, comments, credentials or whole QSO snapshots.
Use native ID formatting in appropriate diagnostic fields. Do not add telemetry
wrappers, manual span lifecycle machinery, per-checkbox logs or a generic bulk
completion framework. Any additional metric needs a concrete operational question.

## 9. Acceptance and review boundary

Verify concrete Clacks behavior through the existing suites and real store schema:

- Reopen a saved QSO after membership and fact edits committed together; Cancel
  preserves its saved value and does not affect unfinished entry or defaults.
- Remove the last membership and reopen the active QSO through All contacts and
  Untagged without creating another identity or losing service evidence.
- Use a mixed selection: A has Main and Activity, B has Main. Add Portable while
  leaving Activity mixed; A keeps Activity, B does not acquire it. Explicitly
  checking/clearing Activity applies/removes it across the selected targets.
- Include a selected contact outside the filter and another on a different page;
  mutate the selected IDs once, preserving unrelated contacts and normal counts.
- Use real intervening edits/deletion to verify guarded outcomes, retained input
  and unchanged newer saved state. Exercise cancellation and partial results
  without injected triggers, failpoints or test-only production APIs.
- Distinguish committed saves from subsequent read failure and truthful telemetry;
  do not duplicate SQLite, SQLx, checkbox or telemetry-library tests.
- Verify the compact summary and bounded picker with hundreds of tags, duplicate
  names, search-hidden pending changes and mixed/empty/unchanged choices. Clear
  selection and verify the history membership action disappears.

Apply the [UI completion gates](desktop-ui.md#ui-change-completion-gates) to the
assembled production application. Inspect actual placement, modal/popup blocking,
keyboard/focus/dismissal, scrolling, appearance, scale and pane bounds. Component
interaction or geometry assertions alone do not establish visual conformance.
Keep unavailable native Windows acceptance explicit.

This document is the review input for an implementation plan. It does not select
an exact file layout or authorize product-code changes before that review.

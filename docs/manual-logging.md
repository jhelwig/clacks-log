# Offline manual contact entry, editing and deletion

This specification defines [issue #9](https://github.com/jhelwig/clacks-log/issues/9).
It consolidates the accepted manual-logging behavior, including D86–D87,
D112, D181–D182 and D310–D318. It describes the required production capability,
not implementation status or a task-level implementation plan. Timestamps use
Coordinated Universal Time (UTC).

The [native model](native-model.md) owns recorded values and validation;
[client storage](client-store.md) owns durable transactions and stale-save
protection; [synchronization](synchronization-model.md) owns pending records and
comparison evidence. The [desktop UI contract](desktop-ui.md) owns shared visual
and interaction rules. [Operating settings](operating-settings.md) and
[logbook setup](logbooks.md) own active defaults and selected-view behavior.

## 1. Outcome and scope

An operator can enter, save, reopen, edit and individually delete contacts in
the production Windows desktop without a server or provider connection. Saved
contacts survive restart and use the existing account-owned native records,
SQLite persistence and pending synchronization state.

Extend the actual desktop application, its application-owned Tokio runtime,
shared UI components and client store. Keep Rust 1.99.1, egui/eframe with wgpu,
and the thin application wrapper. The contact list, forms and storage operations
must be production components that later capabilities extend.

Included:

- One unfinished new-contact entry with quick fields and adjacent detail tabs.
- All native recorded contact facts, using active operating defaults where
  applicable and permitting contact-specific corrections.
- A basic saved-contact list in the selected view, sufficient to reopen a
  contact for full editing or individual deletion.
- Durable local save/edit/delete outcomes, validation and actionable failures.

Not delivered by this issue:

- Full history filtering and Worked Before (#10), contact membership editing
  (#51), bulk editing/deletion, import/export or live synchronization transport.
- Directory lookup, geographic autofill or provider submission/downloads.
- Separate live/historical entry modes, restart-persistent unfinished drafts,
  docking or additional entry windows. Future entry windows share the same
  unfinished contact under the desktop UI contract.

The agreed upper-right area accommodates Worked Before when #10 delivers it.
The [history specification](history.md) defines that capability and its
expression-based filters, extending this issue's production list and editor.
For #9, deliver the actual Contact details and Our settings tabs; do not add a
nonfunctional Worked Before tab or mock results. Later work extends this same
area and the saved-contact list.

## 2. Workbench and form boundaries

Preserve the agreed workbench: compact operating summary, quick entry at upper
left, one adjacent tabbed area at upper right, and contact history across the
width below. Apply the existing defined-section design language, palette,
relative typography, selectable radio-data font, scale, control states and
locally remembered pane sizing. Do not substitute stacked cards or default
framework styling for the shared components.

Quick entry uses this order:

1. Callsign and Name together.
2. Frequency, Band, Mode and DX grid.
3. Sent and received reports, followed by Start and End UTC date/time controls.
4. Full-width Comment, then Save QSO and local validation/save feedback.

Field labels do not append “optional.” Start and End each combine the date and
time in one control; do not duplicate them in the adjacent tabs.

**Contact details** contains additional contacted-station geographic facts,
participation/reference associations, the secondary contacted operator callsign,
and receive band/frequency override. **Our settings** contains the contact's
own station/location/references, operator, power and copied equipment values as
one coherent operating-settings group. Additional fields are directly available
within their tab, with scrolling where needed, rather than expandable details.

Both panes edit the same unfinished contact. Switching tabs preserves all input
and neither saves nor discards it. Save QSO covers both panes; there is no
intermediate “Return with changes,” Apply or Update-entry step.

The compact operating summary continues to edit active operating settings under
its own contract. Contact-specific changes in Our settings never alter active
defaults, profiles or equipment configurations. Saving a reusable profile remains
a separate operating-settings action, not a contact-save side effect.

## 3. Unfinished input and operating defaults

Values copied from active operating settings are the entry's comparison baseline,
not unapplied edits. Refresh that baseline as active defaults change, while
continuing to distinguish explicit contact-specific corrections.

Unfinished text and partially completed fields are transient desktop form state,
not saved or synchronized QSO records. Do not persist a QSO until its complete
submitted value passes native validation. Closing the application does not
promise recovery of unfinished input.

Initialize own station, operator, power and equipment from the client's active
operating settings. These defaults may be incomplete; both station callsigns
must nevertheless be supplied before saving. A saved profile is not required.

While entry is unfinished, newly applied active settings update fields that
still follow those defaults. Preserve explicit contact-specific corrections,
including deliberate clears. For example, changing active power from 100 W to
50 W updates an unfinished contact unless its power has been explicitly edited.
Use the native position and band/frequency consistency rules when changing
related inputs; this is local form behavior, not a new synchronization merge.

The selected view supplies new-contact tag membership:

- A named logbook supplies its tag.
- All contacts and Untagged supply no tags.
- A view change replaces the proposed default membership, preserving other
  input. Do not retain the previous named logbook as a hidden default.

An edit to a saved contact retains its tags regardless of current view changes.
This issue does not introduce membership editing.

## 4. Recorded values and validation

Use the native-model types and validation, including their preservation of
unfamiliar mode, band and reference-group values. Recognition is interpretation,
not a reason to rewrite an existing value or create an edit. Do not impose a
second, stricter saved-contact schema in the UI.

A saved contact requires both station callsigns, a valid UTC start timestamp,
mode information, and primary frequency or band. A recognized band is not
independently required when frequency is supplied. End, reports, name, location,
operator information, power, equipment, references and comment remain optional.

Expose the native values through the quick and detailed forms:

- Recorded callsigns trim surrounding whitespace and uppercase while preserving
  prefixes, suffixes and slash components. Do not normalize a recorded callsign
  into a future directory-lookup key.
- Name is the contacted operator's independently optional recorded name. It
  does not require a distinct operator callsign. Do not infer that callsign
  from the contacted station. Future directory names remain separate snapshots.
- Reports are optional text supporting phone, Morse and digital operation;
  do not impose a numeric-only signal-report format or contest-exchange schema.
- Primary and receive frequencies have whole-hertz resolution. Editing frequency
  derives band where possible; editing only band clears exact frequency. Do not
  let a derived band update clear the frequency that caused it.
- An absent receive override uses the primary pair for both directions. A
  band-only receive override stays band-only, without borrowing primary
  frequency. Clearing both override fields removes the override.
- Power has one-milliwatt resolution. Follow the existing watt-input component,
  including acceptance of `.5` as half a watt; do not invent extra precision.
- Positions use the native grid-area or explicit-point forms, preserving grid
  length and coordinate provenance. Follow the existing coordinated grid/point
  editing rules. Own operating-location entry requires at least four locator
  characters; an imported two-character area is not silently rewritten merely
  by reopening its saved contact.
- Geography remains manually editable here. Do not imply a provider lookup or
  geographic autofill ran. Use native recorded location fields and validation.
- Each station has its own unordered, deduplicated set of group associations,
  each with a group ID and optional identifier under its definition. Use ordinary
  account-shared definitions; do not hard-code POTA or contest-specific forms.
- Empty/whitespace-only optional text is absent; otherwise preserve its text.
  Unchanged unknown associations and values must survive unrelated edits.

Apply shared debounced live validation (currently 300 ms), changed-field
highlighting, field errors and a clear summary near Save. Disable saving while
errors remain and revalidate the actual submitted input. A native-valid contact
is locally savable even if an external service would need more information.

## 5. Start and end timestamps

Use UTC and whole-second resolution. Start and End
are independently live-updating until their lock control is selected. Locking
freezes that complete timestamp and permits manual date/time editing. Unlocking
resumes the current UTC date/time. This is one workflow whether the contact
happened now, five minutes ago or on an earlier day.

Start is required. A locked End can be cleared to unknown and stays blank until
edited or unlocked. When present, End must be at or after Start; equal timestamps
and contacts crossing midnight are valid. Each control includes its own date,
so overnight contacts do not depend on inferring a date from time-of-day order.

On Save, capture one coherent submitted contact, including the current live
values. Do not let clocks advance the submitted payload while it is saving.
Failure preserves the submitted timestamps and other input for correction or
retry; only successful new-contact saving invokes the reset below.

## 6. Save and next-entry behavior

Enter from a single-line contact-entry field invokes Save QSO. Tab moves focus.
Dropdowns and multiline text retain their ordinary control behavior. Unrelated
dialogs do not acquire a global contact-save shortcut. Keyboard and button
submission use the same validation and save operation.

While saving, make the submitted entry read-only and show Saving. Block duplicate
submission while keeping the rest of the workbench responsive. Changes to the
view or active settings during this period do not mutate the submitted contact.
On successful completion, initialize the next entry using the then-current view
and active settings. If saving fails, restore the preserved submitted form
rather than silently substituting new defaults.

Successful new-contact saving:

- Clears the contacted station's callsign, recorded location, operator/name and
  references, both reports and comment.
- Retains primary and receive band/frequency and mode.
- Uses current active own station/operator/power/equipment, without carrying
  contact-specific corrections into the next contact.
- Returns both timestamps to live updating and uses current-view membership.

Keep routine success quiet and local to entry. Return focus to Callsign for the
next contact. A failed save leaves an actionable error, preserves input and
allows an explicit retry. Do not claim synchronization or external upload
success from a local save.

## 7. Saved-contact list and editing

Read current local working contacts for the selected named logbook, All contacts
or Untagged. Show one row per QSO, using stable QSO IDs. Multi-tag membership does
not duplicate a contact. Exclude pending deletions and retain existing pending/
conflict indicators; do not substitute the last synchronized values for local
working edits.

The basic list uses the production reader and shared table components, with
bounded reads rather than loading the entire account. As an engineering default,
start with newest-first UTC start order and stable QSO-ID ordering for ties.
Honor the shared column visibility/order/width/sort and selection rules for the
columns delivered here. Default columns expose contacted callsign, combined UTC
start, band, mode, frequency, reports, DX grid and pending/conflict status.
Timestamp columns compare the complete date and time, never time of day alone.
A date-only calculated/custom column is not added by this issue.

Double-clicking a row or pressing Enter on the focused row opens that contact's
full editor directly. Selection identifies contacts independently of row order;
view switches clear selection. Sorting retains selected IDs. Full filtering,
Worked Before and later membership operations extend these same components.

The saved-contact editor is a separate dialog over the workbench. It contains
all recorded contact facts, grouped consistently with entry, and uses static
editable Start/End timestamps rather than live clocks. Save changes validates
and saves this QSO only. Cancel, Escape and dialog close discard unapplied edits;
they do not save a QSO or change the unfinished new-contact entry. A successful
edit likewise does not reset or modify new entry or active operating settings.

Retain the loaded record as the expected save basis. If the current stored
record changed, do not silently overwrite it. Preserve edited input, show the
stale outcome and make the current stored contact available for reload. Reload
explicitly replaces the form with current values. Do not reinterpret this as a
new automatic field merge or invent a second synchronization conflict policy.

## 8. Individual deletion

Provide an individual Delete QSO action for a saved contact. Confirm using its
contacted callsign, combined UTC start, band/mode and an explicit statement that
this deletes the contact across all logbooks. Offer Delete QSO and Cancel, with
initial focus on Cancel. Removing a tag is a different operation.

Submit deletion against the expected stored contact. A stale outcome or failure
must not report success or discard the user's current editing state. Only after
committed deletion does the contact disappear from normal lists. The deletion
uses the existing pending-deletion state and retained comparison evidence;
keep it accessible to the later pending/conflict-review capability. Do not add
hard deletion, restore/clone UI or bulk deletion as part of this issue.

## 9. Persistence, runtime and recovery

Reuse the client store's immediate write transactions, shared relational payload
storage, expected-record comparison and synchronization transition contracts.
Save payload and pending metadata atomically. Keep accepted state, pending work
and retained evidence intact under those contracts; a UI operation is not a
new transport protocol or causal model.

Use stable identity for the submitted create operation and preserve it across
retries where the commit outcome is uncertain. Distinguish a failed write from
a successful commit followed by a failed list refresh: the latter remains saved
and offers refresh, not a second contact creation. Do not infer duplicates from
equal callsign/time/band facts or forbid intentionally equal contacts.

Perform database work on the existing application runtime, outside egui rendering
callbacks. Do not hold a database transaction while a person edits a form. Reuse
existing shutdown admission and drain behavior; do not create a custom write
queue, runtime or generation-counter scheme for this capability.

Extend fixed SQL query files and SQLx offline metadata as needed, including the
real Untagged predicate and selected-view paging. No replacement schema, inline
fixed SQL, temporary in-memory repository or demonstration contact list.

## 10. Observability and acceptance

Follow the [production observability contract](observability.md). Instrument
meaningful save, edit, delete and list-read boundaries with diagnostic outcomes
and durations. Emit committed outcomes only after commit; keep read/refresh
failures distinct. Use existing tracing and metrics facilities, with bounded
labels and no callsigns, names, reports, comments or complete snapshots in logs.
Do not add telemetry-only lifecycle abstractions or tests of framework behavior.

Acceptance must demonstrate Clacks behavior through real production components:

- Save valid contacts offline, including band-only, unknown band/mode values,
  unknown End and overnight timestamps; reopen the workspace with facts intact.
- Verify independent timestamp locks, clears, Enter submission, validation,
  Saving state and successful versus failed-save preservation/reset behavior.
- Verify active-setting changes and explicit contact corrections, view changes,
  current-view membership and the submitted-save boundary.
- Verify both panes describe one contact and switching tabs preserves input;
  saved editing/cancellation leaves unfinished entry and active settings alone.
- Verify editing preserves unrelated native facts and stale saves cannot
  overwrite a newer working record. Unchanged saves do not fabricate edits.
- Verify real selected-view list results, Untagged behavior, stable paging,
  full-timestamp sorting, pending indicators and hiding of committed deletions.
- Verify individual deletion's account-wide effect and retained pending metadata,
  using the actual schema and transitions rather than injected SQL triggers.
- Verify committed-success versus refresh-failure handling and retry identity
  without production-only test hooks or duplicated dependency behavior.
- Check actual rendered layout, styles, keyboard operation and persistence on
  Windows using the production executable. Automated tests stay noninteractive.

The implementation plan selects focused test locations and commands. Do not
manufacture failures in SQLite/SQLx, retest its migration/transaction machinery,
or implement test-only public APIs to make a scenario injectable.

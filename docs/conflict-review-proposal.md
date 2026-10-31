# Unsynchronized changes: integration proposal for #15

Status: proposed integration design for review. D382–D387 are agreed behavior;
choices marked **Engineering default** below are recommendations, not additional
accepted decisions. This document does not authorize implementation.

## 1. Intent and boundaries

Deliver account-wide review in the existing desktop and SQLite workspace,
using the synchronization model's durable comparisons and resolution operations.
Do not build another conflict engine, synchronized conflict queue, generic form
framework, or desktop-to-server resolution endpoint.

The behavioral sources are [product scope](product.md#pending-and-conflict-review-d182),
[conflict semantics](synchronization.md#accepted-conflict-semantics),
[the desktop contract](desktop-ui.md#unsynchronized-changes-d383), and
[the decision register](decisions.md#d382). Whole-record detection, deletion as
an edit, conditional acceptance, recovery uncertainty and reference validity
remain unchanged. This capability does not add automatic field/tag merging,
restoration browsing, equipment restoration/cloning, or account-wide atomic saves.

## 2. Agreed interaction model

- Open **Unsynchronized changes** in a separate non-modal window. Preserve the
  workbench's selected view, arrangement, unfinished QSO and normal operation.
- Separate **Conflicts** and **Pending sync** into independently collapsible
  sections. Retain record-type filtering and omit a logbook selector.
- Show local/shared values and available three-way change attribution. Differing
  scalar fields start without a choice. Equal fields need no choice. **Use all
  local** / **Use all shared** populate choices without saving.
- Allow direct entry of new resolved values. Apply existing validation and
  unapplied-change styling. Invalid or incomplete choices cannot be saved.
- Membership is an editable set, initially the local working set. Checked
  logbooks always define the result. Field-level **Use local** / **Use shared**
  replace the set; individual changes can combine memberships or leave the QSO
  untagged. No indeterminate boxes or **Use selection** action applies.
- Preserve drafts when switching records. When compared inputs change, preserve
  the draft and displayed comparison, indicate that newer values exist, and
  require an explicit refreshed comparison before saving. Refresh keeps entered
  values for reconsideration, without silently approving them against new inputs.
- **Save resolution** saves one validated record locally. It does not claim
  shared acceptance. Closing the window discards unsaved resolution drafts like
  Cancel and does not undo completed saves.

Draft lifetime is the open window, not a new persisted form/session feature.
Saved local work and comparison evidence retain their existing durability.

## 3. Concrete presentation

### Window and navigation

Use the palette, section frames, typography and field states in
[desktop-ui.md](desktop-ui.md). The workbench action is **Unsynchronized changes**.
The window contains the record-type filter, grouped record list and selected
record detail. Counts label records, not edits or an overall synchronization
completion claim. Connection state and current errors use existing diagnostics;
there is no last-successful-sync timestamp.

Each list item identifies its record unambiguously: for a QSO, contacted call,
start date/time and useful band/mode context; for a reusable record, kind and
name, with ID context where needed. Deleted records use retained values for
identification without presenting those values as a live record. Use existing
short-ID duplicate-name treatment for logbooks, with full identity available.

**Engineering defaults:** both sections initially expanded; one selected record;
existing identity-qualified ordering and bounded pages of 100 per section.
Section counts cover all matching records, not just the loaded page. Each
section has its own continuation and a Load more action. Collapsing a section
preserves selection and draft; filtering can hide a draft without discarding it.
Keep only loaded rows and visited drafts in memory, not every account payload.

A saved resolution leaves Conflicts and appears under Pending sync until normal
synchronization settles it. Retain the just-saved detail long enough to show the
actual save acknowledgment; do not immediately jump to another conflict.
Ordinary pending items show their local change, including deletion, without
resolution controls or an implied user task.

### Comparison and editing

Use Field / Local / Shared / Resolved columns on sufficiently wide windows;
stack each field's labeled values at narrow widths. This is a presentation
layout, not a four-copy persistence model. The resolved control uses the same
unit, parsing and validation conventions as its normal editor. Local/shared
shortcuts copy the native value, not formatted text. A supplied empty optional
value is a valid choice and differs from having made no choice.

For three-way comparison, label differences as Changed locally, Changed in
shared log, or Both changed. Equal displayed results are not conflicting merely
because their formatting or recognition metadata differs. Hide equal rows by
default, with a Show unchanged fields control for context (**Engineering
default**); this does not permit editing identities or causal metadata.

Original values need not form a permanent fourth comparison input on screen.
When the genuine original is unavailable, explain that attribution is unavailable
and show the available compared values without fabricated change labels. When
ordering is unknown, explain that ordering evidence is insufficient; do not label
it proven concurrency. Keep metadata failures distinct from user-value conflicts.

Retained accepted alternatives in `ReviewInput.retained` must remain accessible.
**Engineering default:** show distinct additional native values as labeled
comparison sources, with their accepted timeline/revision available as context.
They are not extra QSO rows in history. Selection can copy a value from such a
source, or the user can type a result. **Use all shared** always means the shown
`review.shared`, not an arbitrarily selected retained alternative. Preserve all
native retained evidence through the existing core resolution operation.

### Outdated comparisons

Observe normal workspace change notifications and compare the saved basis with
the draft's retained `ClientRecord` and `ReviewInput`. Changes to unrelated
records do not invalidate a draft; relevant referenced records and definitions
also require rechecking validation. Do not substitute a newer comparison beneath
the user's controls. Display a stable notice and disable Save until Refresh
comparison has completed.

**Engineering default:** after refresh, retain resolved inputs and explicit
choices for fields whose compared native inputs are unchanged. Preserve a typed
result for a field whose inputs changed, but mark it for explicit reconsideration
and require choosing a side or confirming that retained result before Save.
This is confirmation against changed evidence, not an extra Use selection step
for an ordinary unchanged membership field. Initialize newly differing fields
without a choice; retain an existing membership draft rather than resetting it.

If refresh shows the conflict has already settled, explain that fact; do not
silently save an old draft as a new edit. Keep it available until the user
switches away or closes the window. No new distributed conflict is invented.

## 4. Review cases and permitted actions

| Existing case | Presentation and integration |
| --- | --- |
| `BothDifferent` | Show native field differences, attribution and resolved editors. Save through the existing guarded resolution operation. |
| `MissingOriginal` | Explain that original values are unavailable. Offer explicit value resolution without three-way labels. |
| `OrderingUnknown` | Explain the concrete ordering limitation. If actual compared shared values exist, allow the agreed conservative resolution. An unavailable observation is not an empty or deleted record; keep the draft but disable saving until usable comparison evidence arrives. |
| Local deletion versus changed shared live record | Show deletion as a record-state choice alongside the live values. Choosing the live record cancels the pending deletion; choosing deletion keeps it. No automatic QSO deletion follows from a tag action. |
| Shared deletion versus changed local live record | Show the actual deletion/edit comparison and explicit keep-edited-record or accept-deletion choice under D05/D138. This is resolving an existing conflict, not browsing accepted deletions to restore them under D55. |
| `MissingReference` to a tag | Identify the deleted/unavailable membership without losing its ID. Preserve the QSO draft; require explicit removal or replacement with an available tag. Do not silently untag, resurrect the tag or discard the QSO. |
| `MissingReference` to equipment | Preserve the profile draft. Select existing equipment or remove the optional default. Do not restore/clone the missing configuration; leave copied active operating values untouched. |
| `Referenced` for tag/equipment deletion | Show the target, total surviving-reference count and bounded affected-record list. Explain that these references block deletion, rather than treating them as competing edits to target fields. See below. |
| `ExcludedActor` | Explain that formerly pending work requires explicit reconciliation after reauthorization. Compare real shared values and allow an explicit chosen result. Release only the corresponding saved hold using the existing resolution transaction; do not relabel old prepared proposals. |

For a blocked deletion, provide access to the actual referring QSO/profile editor
or its own conflict review if it is conflicted. Remove/replace references through
those records' ordinary guarded saves. Do not infer whether an earlier tag action
meant Delete all, and do not automatically delete newly discovered referrers.
Allow cancelling a still-pending deletion by choosing the surviving shared live
record. Do not recreate an already accepted deleted target.

Cancelling that pending target deletion does not undo QSO deletions, membership
changes or profile repairs already saved as separate record changes. The system
does not retain an inverse of the earlier tag-deletion action.

**Engineering default:** a Refresh references action reads the current local
accepted/reference view and full count, not just the rejection's bounded sample.
After explicit reference repairs, Retry deletion rechecks references and releases
the same pending deletion for normal preparation when eligible. If repairs are
still awaiting shared acceptance, show that state and leave the deletion blocked;
never treat local repairs as proof of shared acceptance. The server's ordinary
atomic reverse-reference check remains authoritative. This needs a focused
store operation, described below, rather than calling generic resolution with
an unchanged deletion to clear its review unconditionally.

Held old-actor work before initial catch-up may lack a usable `ReviewInput`.
Show it as blocked work under Conflicts, with Waiting for comparison data and no
save action; do not classify it as ordinary ready-to-send pending work. Reuse the
existing enrollment hold and the review created when catch-up completes.

## 5. Field and editor reuse

Keep comparison selection separate from editor rendering and persistence.
**Engineering default:** explicit typed matching on the native record variants
and purpose-specific field identities, not a JSON tree diff, string field paths,
reflection registry, new serialized value representation or universal form engine.
Only add field-control extraction when the existing and review screens both use
it. Keep existing form save handlers out of the comparison window.

| Record/field | Reuse and comparison boundary |
| --- | --- |
| QSO identity | Immutable native ID; never an editable difference. |
| QSO start/end | Existing combined UTC date/time parsing, whole-second values; resolution uses fixed values, without new-entry Live clocks or save-reset behavior. Start/end remain separately selectable, with final end-at-or-after-start validation. |
| Each station | Reuse `StationForm` parsing and station controls for recorded callsign, position, recorded geographic fields and participation. Own/contacted sides remain distinct. |
| Position | One native position choice, including grid-area versus explicit-point basis and precision. Reuse `PositionForm`; grid and coordinates are coupled views, not independently selectable contradictory facts. |
| Primary / receive radio | Each native band/frequency pair is a composite value. Reuse `RadioForm`; custom frequency changes set band and band changes clear frequency. An absent receive override differs from a band-only override. |
| Mode | One preserved native hierarchy, edited through existing mode/parent controls. Recognition changes are not differences or rewrites. |
| Operator, power, reports, comment and copied equipment | Existing text, optional callsign, watts, report and equipment controls; compare the actual native optional fields, never directory fallback text or live equipment links. |
| Logbook membership | Native `BTreeSet<TagId>`; local initial set, per-field source shortcuts and a searchable single-QSO picker using normal boxes. Include memberships present only on the shared side and missing IDs. Preserve checks across search. |
| Station participation/reference sets | Compare the native unordered associations; offer both sides and direct editing through the existing data-driven reference controls. Do not add contest-specific branches or silently merge sets. |
| Saved profile | Name, station fields and optional default-equipment ID. Reuse `ProfileEditor` value preparation and selectors, without applying/reloading operating settings. |
| Equipment configuration | Name plus equipment description, antenna description and notes. Reuse `EquipmentEditor`/`EquipmentForm`, without changing copied QSO facts or current operating values. |
| Logbook tag | Name and independent LoTW/QRZ upload policies, using the existing tag editor's controls. Duplicate names remain legal; stable ID is never edited. |

Composite editor boundaries above are **engineering defaults** derived from
existing atomic native values and calculated-field rules, not a new
interdependent-field conflict detector. Whole-record comparison remains the
trigger even for disjoint edits.

`MembershipPicker` already supports a single QSO without mixed-selection marks,
search, duplicate names and unavailable IDs. Reuse it with one resolved set;
never pass the local and shared sets as if they were two selected contacts.
Add a focused way to replace/rebase that single set for source shortcuts, with
actual review use. Do not copy the bulk-assignment workflow or its Apply step.

Do not make group-definition, export-profile or service-evidence management
screens merely because `RecordValue` has those variants. Preserve the seeded-data
and service-specific arbitration rules. Any such record encountered in review
must remain visible with its real reason and safe source information; do not
silently omit it or offer arbitrary edits of causal/service facts. Concrete
editable resolution belongs to a producing workflow's domain contract. #15's
operator-editable records are the QSO, tag, saved profile and equipment types
already produced by the desktop. This is not permission to ship unreachable
editor scaffolding for later capabilities.

## 6. Validation and value preservation

Copying a local/shared/retained native value must preserve that value exactly.
Do not reconstruct the entire chosen record through formatted editor strings.
Use existing native field/input types to assemble and validate one complete
replacement; existing form controls prepare only the explicitly edited inputs.

This distinction matters today: `ContactForm::build` retains an original QSO for
historical values, but treats a changed own position as operating-location
entry. A previously valid imported two-character position selected from the
other side must not become invalid merely because it was selected in review.
Keep historical native values on source selection; enforce the existing minimum
four-character own-location rule for genuinely new operator-entered positions.
Likewise preserve unfamiliar modes/bands, reference identities and untouched
historical timestamps without reclassification or lossy text round trips.

Use existing debounced validation, field errors, edited styling and disabled
Save behavior. On Save, perform final complete native validation again. Empty
optional text normalizes to absence; an explicit clear is a selected value.
Validate required station callsigns, primary frequency-or-band, mode, timestamps,
power and reference syntax under their existing contracts. Do not validate
provider fallback information as if it were manually recorded QSO data.

Resolve known deleted references explicitly. Available locally created tags and
equipment may still be pending synchronization; that does not make them deleted.
Retain ordinary prerequisite ordering and server acceptance checks. A reference
or definition changing after UI validation must not enable a stale save.

**Engineering default:** permit resolution from already durable usable comparisons
while disconnected. The selected core/store APIs do not require a network call;
adding an online-only gate would duplicate no existing safety guarantee. This
uses D139's permitted offline behavior without promising a current server view.
Unavailable comparison evidence still prevents saving, connected or not.

## 7. Module responsibilities and application wiring

Proposed placement is an **engineering default**, using normal Rust modules with
`review.rs` and `review/…` files, not `mod.rs` outside tests:

| Owner | Responsibility |
| --- | --- |
| `clacks-sync::client` | Existing detection, reason, retained evidence and `resolve_review` transition. Keep UI selection and text formatting out. |
| `clacks-client-store::read::review` | Consistent pending/conflict pages, counts, selected record/review and required reference captions from SQLite. |
| `clacks-client-store::synchronization` | Guarded resolution save and focused blocked-deletion retry, using the existing transaction/core operations. |
| Desktop `review.rs` | Typed comparison attribution, selected sources, custom values and per-record in-memory drafts. It does not classify distributed causality. |
| Desktop `ui/review.rs` | Native review viewport, sections, list, comparison and reuse of shared field controls. No database/network calls in egui callbacks. |
| Desktop `session/settings/review.rs` | Typed read/refresh/resolve/retry requests on the existing runtime and request/reply path. |

Use `Context::show_viewport_immediate`, as the Worked Before window already does,
with a stable viewport identity. Keep account/record identity in every response;
late reads must not replace a different record's draft. Reuse existing mpsc,
oneshot and watch channels and repaint integration. Add no event bus, polling
thread, background form worker or generic repository interface.

A shared workspace invalidation wakes review reads after local saves and actual
sync-store changes. Extend the existing notification path rather than inferring
changes from connection status: a connection remaining Connected does not prove
that a record is unchanged. Notifications trigger a bounded reread; the returned
native basis determines whether a particular draft is outdated. The runtime
continues normal receive-first synchronization and unrelated publication.

## 8. SQLite reads and write transactions

### Reads

Reuse `ReadSnapshot`, `ReviewRow`, `ClientRecord`, `ReviewInput`, and existing
payload/original/observation/retained tables. Current `ReviewQuery` supplies a
combined identity-qualified page, but no section selection, counts or exact
selected-review read. Its optional tag filter has no production desktop caller
and conflicts with D383's interface. Remove that unused filter when replacing
its query contract; do not retain it solely for tests.

Extend checked query files with section filtering/counts and a selected-key read
that returns the record and review in the same short snapshot. Include the
existing authorization hold when determining blocked presentation. Conflicts
means actual persisted review or an explicit existing admission hold; it does
not mean merely differing local/shared values detected again by SQL or UI.
Retained-only rows remain visible; when no resolution input is available, show
the actual evidence/recovery wait instead of pretending they are publishable.

Do not load the whole account and partition it in memory. Independent section
pages and their counts come from one bounded snapshot per refresh. Release the
read transaction after copying values; never keep a database reader alive for
the lifetime of a user draft. Use stable kind-qualified cursors, reset affected
pages on invalidation, and preserve visited drafts by native key. Add no durable
summary table or new generation counter.

### Resolution save

Use the existing `synchronization::resolve(store, expected, shown, chosen)` as
the integration boundary. It already checks the complete saved record and shown
review in an immediate transaction, invokes core `resolve_review`, stores the
new pending record, clears that review and releases only a matching old-actor
hold. Preserve this atomicity and `SaveOutcome::Stale` behavior.

Before committing, validate the proposed live references against current local
records in that same transaction. Retain the existing ability to save ordinary
working edits with unresolved references outside this resolution operation; the
resolution must not claim a known missing-reference issue was repaired while
preserving that same invalid reference. Reuse domain checks and query helpers,
not a second conflict policy. Shared conditional acceptance remains necessary.

Deletion storage already runs selected-logbook fallback and loaded-source cleanup
inside `metadata::client::store`; call that existing path. Do not add duplicate
cleanup in the window or reset active operating values when a profile is saved.
The UI refresh shows changed source indicators without explicitly reloading it.

After commit, acknowledge the actual save to its originating editor, refresh
ordinary read views, and wake synchronization through the existing local-change
notification. A later read/exporter failure does not turn that commit into an
unsuccessful save. While a save is in flight, its record's editor is read-only
with Saving feedback; navigation or window closure does not cancel an admitted
write or erase its authoritative outcome.

On stale return, change no saved state; preserve the draft and load a fresh exact
comparison under D385. Do not rebuild the request with newer expected values and
silently retry the old choices. If review vanished, report the current state.

### Blocked-deletion retry

Add a narrowly scoped store action whose production caller is the blocked
reference panel. Guard the exact shown target/review and reread current surviving
accepted references in the same write transaction. If references remain, return
the refreshed count/list without clearing review or altering the target. If
none remain and no other review reason intervenes, prepare the same explicit
deletion against current evidence through existing native transitions, clearing
only the reference block. Do not allocate a counter twice or resend old frozen
bytes against new evidence. Concurrent new references can still cause the
server to reject; receive that normal outcome as a refreshed review.

Reference repair writes are ordinary per-record operations, not one transaction
spanning the user interaction or a promise that every listed repair succeeds.
The UI shows actual saved/pending results and rechecks the target afterward.
No new schema, snapshot manifest, retained action-intent record or synchronization
protocol is required by this proposal. Checked query changes require SQLx offline
metadata updates using the existing project tooling.

## 9. Verification boundaries and remaining design status

Meaningful regression scenarios should establish these Clacks decisions, reusing
fixtures and real production operations where needed:

- Native comparison attributes local-only/shared-only/both-changed fields after
  the existing whole-record trigger; missing originals never fabricate labels.
- Actual review controls submit the correct key, retained basis and chosen native
  values, including custom text, explicit clears and combined membership. Source
  shortcuts do not submit; checked membership starts local and directly saves.
- Switching records retains drafts; closing discards only unsaved drafts. A
  changed comparison preserves inputs and prevents stale submission; refresh
  requires reconsidering changed inputs without losing the draft.
- A real store resolution clears only its exact review/hold, preserves native
  values and retained support, and becomes normal pending work. An intervening
  committed update produces a stale outcome without overwriting current state.
- Deleted-reference repair and blocked-deletion retry use actual QSO/profile
  changes and surviving shared references; a local repair awaiting acceptance
  cannot falsely unblock deletion. Do not inject database triggers/failpoints.
- A production-session resolution wakes the existing synchronization cycle;
  shared acceptance settles it, while unrelated records continue and rejected
  state remains reviewable. Use existing authenticated test setup, not a second
  fake sync implementation.

Use egui_kittest for bounded control/request assertions, native core/store tests
for value and transaction claims, and assembled application inspection for the
window, section framing, widths, typography, stale and error states. Do not test
every field through the entire GUI or retest SQLite/framework guarantees. Follow
[observability](observability.md): existing resolution events plus focused safe
user-action outcomes, no field values, comments, payload dumps or telemetry
abstractions. CI availability and unavailable native Windows checks must be
reported rather than treated as passing.

No new product decision is asserted here. The engineering defaults requiring
review include composite comparison boundaries, reconfirmation after refresh,
offline use of available comparisons, and the bounded read/retry integration.
The final specification should incorporate the accepted proposal and carry
forward the earlier behavior without reopening it.

The [#15 implementation plan](superpowers/plans/2026-10-31-unsynchronized-changes.md)
translates this proposal into production increments. Its preparation does not
change the agreed/default distinction or authorize execution.

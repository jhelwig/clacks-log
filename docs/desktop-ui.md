# Desktop workbench and interaction design

This document owns the agreed desktop workbench behavior. The
[desktop foundation](desktop.md) owns runtime and lifecycle behavior;
[operating settings](operating-settings.md) owns station, equipment and
reusable-record semantics; the [product contract](product.md) owns contact entry,
logbook views and Worked Before. These are design requirements for the product,
not a claim that every described view is already implemented.

## Workbench composition

The main window is a compact workbench with four continuously available areas:

- QSO entry at the upper left.
- Worked Before for the current callsign at the upper right.
- The current logbook's contact history across the width below those two areas.
- A compact summary of the active station, equipment, operator and transmit
  power above the logging areas.

Use desktop space efficiently and keep useful data visible. The accepted compact
arrangement establishes relative placement and density, not fixed pixel sizes,
column widths, fonts, colors or a docking system. The visual direction below
governs separation. The shared-component contract below defines common behavior;
exact visual tokens remain implementation design.

Worked Before presents one row of data per QSO, rather than a multiline card for
each contact. Its callsign-prefix matching, current-logbook scope and handling of
local working versions follow the product contract. Contact history remains a
table of the current logbook's contacts.

### Resizing and reset

Make the divider between QSO entry and Worked Before, and the divider between
the upper pair and contact history, user-resizable (D300). Remember pane sizes
locally on the client. Provide a reset to the default arrangement, with QSO
entry and Worked Before above full-width history. Reset changes layout, not
operating settings, contact data, appearance, fonts or selection.

Use minimum pane sizes that keep controls usable at the selected UI scale.
Restore remembered sizes within the space available in the current window;
old dimensions must not leave a pane inaccessible after display/window changes.
Additional windows retain the independent-view behavior below.

## Additional windows and shared entry

Dedicated QSO-entry and Worked Before windows are additional views. Opening or
closing one leaves the main workbench's visible areas and arrangement intact;
it does not switch the main window into a different mode or relocate its tools.

All QSO-entry views on one client share the same in-progress contact. Changes to
that contact and its current callsign are reflected in the other entry and
Worked Before views. An extra entry window does not introduce an independent
draft. Existing nonpersistent-draft and successful-save behavior still applies.

## Compact operating summary

The summary shows active values, with selected saved sources distinguished from
unassociated operating values. Keep source-change and equipment-override
indicators consistent with the operating-settings contract.

Provide quick station-profile selection, equipment-configuration selection and
operator changes directly from this summary. Small editors use explicit
Apply/Cancel. Preview the equipment consequence of selecting a station profile:
load its associated default when present, or retain current equipment when no
default is specified. Preserve operator and transmit power on profile/equipment
loads. These actions change current operation, not the saved reusable records.

Transmit power is an inline editable value with a visible watts unit. It requires
no popup or separate Apply button. Enter, Tab or leaving the field commits a
valid value; Escape restores the previous committed value. Accept ordinary
decimal input such as `.5` as 0.5 watts, within native milliwatt resolution.
Blank means absent. Invalid text must not replace active power; preserve the
input and show a useful local error. Changes to power or operator do not detach
station/equipment selections. Computer-aided transceiver control remains
post-MVP; no radio-controlled power workflow is added here.

Place an explicitly named **Operating settings…** action beside these controls,
within the summary group. It opens a dialog for full editing. Avoid an ambiguous
“Edit all…” action separated from the values it affects.

## Full editing and persistence boundaries

The operating-settings dialog retains one pending context across its station,
equipment and operator/power views, with one Apply/Cancel pair. The compact
summary continues to represent applied operation. Manual adjustments detach
only the affected station/equipment selection as specified by D285; operator
and power remain separate. Saved-source changes and deletion retain their
existing comparison, stale-save and cleanup rules.

Reusable station-profile and equipment-configuration editors remain separate
from current-operation editing. Saving them does not implicitly apply, load or
select their values. Quick changes and full-dialog application affect future
operating defaults; they do not rewrite historical QSO snapshots.

## Visual language

Use **Defined sections** (D289): panel headers, restrained borders and recessed
editable controls make tool boundaries and inputs apparent while retaining the
compact workbench density. Carry this hierarchy across the workbench, dialogs
and additional windows. Avoid turning each data item into a separate card.

This selects surface hierarchy, not exact prototype colors, typography, spacing
or pixel dimensions. Preserve the selected egui/eframe architecture when
implementing the shared components below; prototype markup is not the production
component implementation.

The current main screen applies this hierarchy to its available production
tools: an application bar, the operating-context section, a separate saved-settings
section and a compact workspace-status footer. Workspace paths and account
identity are available through **Workspace & diagnostics**. Opening failures,
retry actions and degraded-diagnostic warnings remain visible when those details
are collapsed. Later contact capabilities extend this screen without placeholder
panes for tools that do not yet exist.

### Appearance preference

Follow the operating system's light/dark appearance by default (D298), including
changes while the application is running. Provide an application-level preference
for System, Light or Dark. An explicit Light/Dark choice overrides OS appearance;
System restores following it. Apply the selected appearance consistently across
the workbench, dialogs, menus and additional windows, preserving the same state
meanings and hierarchy in both appearances.

### Typography and UI scale

Provide one application-level UI scale preference (D299). Scale text, controls,
spacing and table-row heights together, in addition to normal OS display scaling.
Retain the compact visual relationships as size changes; reflow or scroll where
needed rather than clipping values or shrinking essential text to fit.

Use monospace for radio data by default, including callsigns, frequencies and
timestamps. The font is selectable rather than fixed to the prototype's family.
Font selection changes presentation, not recorded values, recognition or input
validation. Exact default families, available choices and scaling increments
belong to the component specification and implementation design.

## Validation and unapplied changes

Validate edited values live after a debounce, rather than waiting for the field
to lose focus or for Save/Apply. The current desktop uses 300 ms. Validation
reports whether input is valid; it does not itself save or apply it.
Dialog Save/Apply waits for current validation. Once available, the action
revalidates the latest input before submitting rather than trusting a cached
result. Inline-power commits validate immediately on Enter, Tab or focus departure.
Preserve those distinct commit boundaries.

Give unapplied changes an amber-tinted field background and amber edge marker,
alongside a text indication such as “Not applied.” Keep this distinguishable
from keyboard focus, selection and validation errors; color alone must not carry
the meaning. A value can be both changed and invalid: retain the edit highlight
while showing the validation border/message and independent focus outline.
An invalid edit never replaces the active value. D290 defines validation and
D291 selects this highlight treatment; semantic color tokens are centralized in
the desktop theme implementation.

Save/Apply is disabled while validation errors remain (D292). Keep a clear error
summary near the action and messages at the affected fields, identifying what
needs fixing without requiring an attempt to submit. Re-evaluate availability
as debounced validation runs; clearing errors enables the action without applying
the values. Submission still validates the latest input rather than relying on
the earlier live check. An independently valid reusable-record save is not
blocked by errors in unrelated operating fields.

## Contact-table selection

Preserve selected contact IDs across filtering and sorting (D293). A change in
row order or visibility does not replace or clear the selection. Show the total
selected count and an explicit count outside the current filter. Provide
**Show selected** and **Clear selection** actions so the full set can be inspected
or cleared. Sorting/filtering must not silently change which records a selected-
contact operation targets.

Bulk-action labels state the selected count. Distinguish an action on selected
contacts from selecting all matching contacts. Keep selection separate from
pending/conflict status; selection highlighting must not hide those indicators.
This does not add unrestricted bulk editing of recorded QSO facts or change the
existing export, tag-assignment or deletion contracts.

Switching logbooks clears the contact selection (D297), including hidden selected
contacts. Do not carry it into the newly opened logbook or restore it on return.
Cross-logbook selection belongs in a search/view that is not logbook-scoped.
Filtering and sorting within a view still preserve selection under D293.

Double-clicking a contact row or pressing Enter on the focused contact opens its
editor directly (D296). Do not interpose a read-only detail view. This activates
the focused contact, rather than opening an editor for every selected contact;
existing restrictions on bulk editing remain unchanged. Enter inside a text
field or another control retains that control's behavior.

### Columns and sorting

Both contact history and Worked Before support column visibility, ordering,
resizing and remembered sorting (D301). Store each table's configuration locally
and independently: changing Worked Before does not reconfigure contact history.
Provide a reset to each table's defaults. Column configuration changes the view,
not recorded QSO values or the selected-contact set.

Hidden columns remain available through column configuration; hiding a field
does not remove it from a contact. Widths and row height must respect the chosen
font and UI scale. Use horizontal scrolling where necessary for the remaining
columns rather than squeezing values into unreadable fields. Exact default
column lists and available fields belong to the delivered table capability.

## Operation feedback

Use quiet feedback in the relevant tool for routine success, without a modal or
toast for every saved QSO (D294). Distinguish saving locally from synchronization
and external-service success; report only the completed effect.

Keep failures visible beside the affected form or operation, with an actionable
explanation and diagnostic details available. Preserve edits and make the active
state clear when a save fails. Offer an appropriate explicit retry without
claiming that an uncorrected underlying problem has been fixed.

Long-running bulk operations show progress and cancellation where supported by
their existing contracts. Completed, failed and unprocessed counts remain
distinct. Show expandable record-specific reports for partial results or
cancellation; preserve completed work under the workflow's existing rules.
Errors and partial-result reports do not disappear on a notification timer.
Keep them available within the workflow until dismissed or the workflow is left;
this does not select a durable operation-history feature.

## Editor dismissal

Closing an editor dialog or pressing Escape has the same effect as its Cancel
action: discard that editor's unapplied changes without another discard prompt
(D295). Do not apply or save on dismissal. Preserve the existing nesting rules:
cancelling an inner editor retains its caller's pending form, and cancelling an
outer editor does not undo a separate reusable-record save already completed.
This rule concerns editor dismissal; application shutdown retains its own
lifecycle and unfinished-work rules.

## Shared-component contract

### Intent and scope

Provide a consistent desktop language that current screens adopt and later
capabilities extend. Reuse the same field states, actions, source presentation,
table behavior and feedback in the workbench, operating dialogs and additional
windows. This is a product-wide presentation contract, not authorization to
implement every downstream logging capability at once.

Retain the production desktop foundation, runtime, durable store and native
model. Existing operating/profile/equipment workflows are the first real
consumers. Contact entry, history, Worked Before and service workflows adopt the
same components when their capabilities are implemented. Do not ship sample
QSOs, inert controls or temporary workflow implementations from the mockups.

### Approaches and design boundaries

The selected workbench keeps entry and Worked Before above full-width history;
the compared left-entry arrangement was not selected. Additional windows remain
additional views of the same client state. Defined sections was selected over
Quiet surfaces because explicit headers, restrained borders and recessed inputs
make tool and editing boundaries apparent at compact density.

The written interaction rules govern behavior. Exact prototype markup, example
data, colors and browser mechanics are illustrative. The existing Rust desktop
and egui controls remain the implementation foundation.

### Component responsibilities

| Component family | Owns presentation of | State/behavior contract |
| --- | --- | --- |
| Visual foundation | Semantic colors, surface hierarchy, spacing, typography, units and action emphasis | System/Light/Dark appearance, selectable radio-data font, unified scale; preserve readable states in both appearances. |
| Pane and section | Header, content boundary, local actions and resizable dividers | Workbench placement, usable minimum sizes, locally remembered pane sizes and reset. |
| Labeled field | Label, editable value, unit, optional help and errors | Debounced live validation, amber unapplied state, independent focus/error state, no implicit saving. |
| Inline committed value | Compact value and unit, plus validation feedback | Power's established Enter/Tab/blur commit and Escape rollback; previous active value survives invalid input or failure. |
| Source selector | Named source with distinguishing context, optional selection and source status | Preserve profile/equipment ownership and loading rules; distinguish source changes, detachment, missing references and failed reads. |
| Action group | Save/Apply/Cancel and workflow-specific actions | Validation-dependent availability, specific labels, duplicate-submission prevention and committed-result feedback. |
| Contact table | Headers, configurable columns, focused/selected rows and local record status | Stable-ID selection, explicit hidden counts, selection boundary on logbook switch, direct editor activation and independent local table settings. |
| Editor/dialog | Title, content, validation summary and action boundary | Preserve pending edits across internal views; close/Escape follows Cancel; completed independent saves remain saved. |
| Operation feedback | Loading, progress, success, failure, cancellation and result details | Truthful operation stages, persistent problems, retained edits, distinct counts and record-specific reports. |

Fields and action groups support ready, edited, invalid, unavailable and busy
states as applicable. These are independent facts: a field may be both edited
and invalid; a selected row may also have pending or conflicted data. Do not
collapse these into one mutually exclusive status or rely on color alone.

Derived/read-only values remain legible and identifiable as such. Unavailable
actions explain the missing prerequisite. Loading, an empty collection, a filter
with no matches and a failed read are distinct presentations; a failed read must
not masquerade as an empty/deleted source. These carry forward the owning
workflow contracts rather than defining a new source-availability model.

### Ownership and integration

The following are engineering directions for the component specification, not
new account, synchronization or domain rules:

- Keep shared presentation modules within `clacks-desktop` initially, organized
  by responsibility. Use egui controls, layout, focus, window and input facilities
  directly; extract helpers for coherent repeated presentation. Avoid a generic
  form engine, custom reactive framework, component registry or second event bus.
- Screens own workflow decisions and pending forms. Components display supplied
  values/state and return ordinary interaction results. They do not query the
  database, launch provider work or decide synchronization conflicts while drawing.
- Native-model validators and transformations remain authoritative. A field may
  retain incomplete text, but live validation must not create a parallel set of
  domain rules. A commit checks the latest input and real persistence result.
- Keep UI preferences local and presentation-only. They must not modify or
  synchronize contact facts or reusable station/equipment records. Reuse the
  existing local configuration/store facilities; the implementation plan chooses
  their concrete ownership and migration needs.
- Keep widget identity stable across refresh, sorting and reordering. Background
  results must preserve in-progress input and must not unexpectedly move keyboard
  focus. Use the existing runtime/session integration and stale-save protection.
- Use visible labels and accessible names for controls, including icon actions.
  Keep keyboard focus apparent and tab order aligned with the visible workflow.
  Input-local keyboard behavior takes precedence over table/global shortcuts.

### Shared-component defaults

These defaults complete the shared interaction contract:

- Use a 300 ms live-validation debounce. Dialog actions remain disabled while
  validation is pending or invalid; inline-power commit validates immediately.
  Validation state remains associated with the current edited value.
- Return focus to the opener on dialog dismissal. Keep modal focus inside the
  active dialog; clicking its backdrop does not discard edits. Escape dismisses
  the innermost transient control first, rather than cancelling an outer editor
  while a menu is open.
- Confirmation actions name their effect and scope. For tag deletion, show the
  three existing choices and counts, with no preselected contact-handling choice.
  Put initial focus on Cancel and require explicit confirmation of the chosen
  outcome. Existing deletion semantics remain unchanged.

Concrete colors and spacing are centralized in the desktop theme; the desktop
guide describes current font selection, scale range and local preference storage.
Table column definitions and pane preferences arrive with their production
consumers. Use existing framework facilities and keep defaults centralized so
refinement does not require restyling every screen.

### Verification and delivery

Verify Clacks interactions through production forms and real local state:
validation disables the affected action, corrections re-enable it without saving,
Cancel discards only the pending edit, failed saves retain it, and valid commits
report their actual outcome. Cover selection identity across filtering/sorting,
clearing on logbook switch, independent table/pane preferences and reset, and
the shared-draft behavior when additional windows are delivered.

Check visual/focus behavior in both appearances and at multiple fonts, scales
and window sizes. Native Windows acceptance uses the production executable;
automated tests remain noninteractive. Test Clacks's state presentation and use
of the framework, not egui internals or a second fake application. Performance
checks for large contact tables belong to the real table capability.

Follow the [observability contract](observability.md). Preserve meaningful
operation diagnostics and data exclusion as views change. Do not log keystrokes,
hover/focus changes or entire forms, or add telemetry
frameworks to style helpers. The [desktop guide](../lib/clacks-desktop/README.md)
describes current production controls. Contact tables, panes and additional
logging windows adopt this contract with their owning capabilities; they are not
implemented by the operating-settings retrofit.

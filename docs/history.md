# Local contact history, Worked Before and queries

This specification consolidates the accepted behavior for
[issue #10](https://github.com/jhelwig/clacks-log/issues/10). It extends the
production saved-contact list and editor from [manual logging](manual-logging.md).

**Agreed behavior** comes from the product decisions and existing contracts.
**Engineering defaults** make the design concrete where the user has not
selected an implementation detail; they remain reviewable choices, not new
entries in the decision register. Their identifiers, E1–E9, are local to this
specification. The [desktop UI contract](desktop-ui.md), [native model](native-model.md),
[client-store contract](client-store.md) and [service-evidence contract](service-evidence.md)
continue to own their respective underlying behavior.

Coordinated Universal Time is abbreviated UTC. Logbook of The World is LoTW.
Application programming interface is API. SQL refers to the database query
language used internally; operators do not write SQL to filter contacts.

## 1. Delivery scope — Agreed

Deliver local history, prefix Worked Before and the expression-based history
query bar as production components integrated with the existing desktop,
account workspace and SQLite store. They work offline, without a synchronization
server, directory lookup, service credentials or live provider requests.

- A named logbook scopes contacts by its tag. All contacts includes every
  active account QSO; Untagged includes active QSOs with no tags.
- Read this client's saved working version, including offline saves and saved
  edits pending synchronization or conflict resolution. Show one row per QSO
  identity; accepted and proposed values are not separate contacts.
- Hide pending deletions from normal history and Worked Before. Their access
  through pending/conflict review remains the existing contract; #10 does not
  implement the Phase 3 resolution workflow.
- Preserve editor/deletion integration. Querying, sorting and navigation do not
  save the unfinished QSO, change operating defaults or mutate contacts.
- History selection supplies real contact identities for later export, bulk
  lookup and membership workflows. Issue #51 consumes it; membership editing
  is not a prerequisite for this capability.

This does not add awards, heard-station tracking, directory requests, arbitrary
SQL, query functions, regular expressions, wildcards, collection-member scopes
or a synthetic QSO-span field. Named timestamp predicates compare their own
fields; an absent end is not replaced by start. Tag-ID query disambiguation is
deferred; identically named tags are handled by D339.

## 2. Workbench and history table — Agreed

Preserve the accepted compact workbench: QSO entry at upper left, one adjacent
tabbed pane at upper right, full-width history below, and the compact operating
strip above. Worked Before is a tab alongside Contact details and Our settings;
there is no third upper pane or expandable entry-details section. Tab changes
preserve the same unfinished contact.

Use the existing shared defined-section styling, palette, typography, fonts,
scaling, padding and component states. History fills its allocated remaining
area while loading, empty or populated. User-resizable dividers retain their
local sizes; layout reset stays in View → Layout. Do not put filtering controls
or layout commands above the logging panes as additional stacked sections.

Place the always-visible query bar within the history section above its table.
Keep the section title, column controls and selection actions compact. Use the
shared unapplied/error styling and readable radio-data font. Do not introduce
a required filter dialog or an alternative form-based filter state.

History retains the current default columns from #9: Callsign, combined UTC
Start, Band, Mode, Frequency, Sent report, Received report, Contacted grid and
pending/conflict status. Both tables support independent locally remembered
column visibility, order, widths and sorting, with reset to defaults. Timestamp
sorting uses the complete date and time, never time of day alone. Horizontal
scrolling preserves readable columns instead of shrinking their text.

Column configuration uses the headers' right-click context menu, including
visibility, ordering and reset. Use full-cell table headings and a visible
sort indicator rather than individual button frames. Headers remain available
when results are empty or loading, including a recovery target when all columns
are hidden. These interactions apply to history and both Worked Before views.

Double-click or Enter on a focused contact row opens the existing contact
editor directly. Enter in the query bar or suggestion list follows that
control's behavior; it must not open a row or submit the unfinished QSO.
Successful saved-contact edits/deletions refresh affected history/Worked Before
without resetting the in-progress entry or changing active operating settings.

### Selection and counts — Agreed

**Edit → Select all** adds every saved active contact matching the applied
expression in the current view to the ordinary selection, across all pages.
It preserves already-selected contacts outside that expression. Unfinished
filter text and Show selected inspection do not change this scope. While the
selection is being prepared, show **Selecting contacts…** and disable actions
that consume it. Failure leaves the preceding complete selection intact; newer
view, filter or selection intent discards an obsolete result. The native command
selects text when a text field has focus and contacts when history owns focus.

**Edit → Look up directory information** consumes this same selection, including
hidden members, with no distinct all-matching action or history-toolbar button.
The run captures its selected identities before lookup; subsequent filter/view
changes do not retarget it. Progress, cancellation and retained failure reports
remain accessible through the history workflow. Successful results update only
separate directory snapshots under the existing applicability/arbitration rules.

Keep selected QSO identities across filtering, sorting and scrolling. Show the
selected total and count outside the **applied filter**, not merely outside the
visible rows. Show selected reveals the selected set; Clear selection clears
it. Contact actions operate on that selection; Select all includes all matching
contacts without requiring a separate action scope.
Changing the selected logbook/view clears selection, including hidden members;
returning does not restore it. Selection highlighting must not obscure pending
or conflict status. Deleting contacts removes those identities from active
selection; editing or moving rows does not transfer selection to another QSO.

**E6 — Engineering defaults:** retain newest-first UTC start sorting with QSO
ID as a deterministic tie-breaker. Show selected is a reversible history-table
inspection mode over selected active IDs within the current view; it temporarily
bypasses the expression, without rewriting its edited/applied state. Display
that mode explicitly and offer return to query results. Count outside the
filter against the applied expression, even in this inspection mode.

## 3. Worked Before — Agreed

Worked Before follows the effective callsign text in the shared unfinished
entry, independently of directory lookup and the history expression. Search
saved worked callsigns by literal prefix within the selected logbook/view,
across all own-station callsigns. W7HEX can match W7HEX/P; W7HEX/P does not match
plain W7HEX. Do not substitute directory lookup normalization, base-call
expansion or service/import matching rules.

Empty input clears Worked Before; it never requests all contacts. Manual
changes, successful-save reset, and future authoritative WS Suite callsign
updates all feed this same input boundary. #10 does not require implementing
live capture to provide that boundary. Normalize surrounding whitespace and
case consistently with recorded callsign entry, preserving slash components.
Older asynchronous responses cannot repopulate a cleared or changed call.

Keep older contacts visible. The Today indicator uses the current UTC calendar
day, not a rolling 24 hours. Refresh it when UTC midnight passes even without
a callsign edit. Use saved working rows and the same deletion/status rules as
history. Show available effective contacted grid and recorded activity/reference
information so different activations remain distinguishable. Unknown references
remain visible with their available identity; do not invent group properties.

### Table defaults — Agreed (D328)

Use one row per QSO, not multiline cards. Default columns, in order, are UTC
Start, Callsign, Band, Mode, Contacted grid and Contacted activity references.
Include a compact Today indicator and pending/conflict markers. Own-station
information and reports remain available through column configuration. Keep
Worked Before's column preferences independent of history's.

The accepted additional-window behavior remains: a dedicated Worked Before
view does not hide, relocate or change the main workbench. Its current call,
selected logbook and saved contact data are shared. It does not create another
QSO draft. Dedicated entry-window work is not a prerequisite for #10.

**E7 — Engineering defaults:** deliver the Worked Before tab and an Open in
window action for a dedicated Worked Before view. Both views use one logical
Worked Before model and table preferences, with window-local scroll/focus.
Use newest-first UTC start sorting with stable ID ties. Today is determined
from each QSO's saved UTC start date. Worked Before row activation uses the
existing saved-contact editor; history owns the bulk selected-contact set.

## 4. Query editing and state — Agreed

Automatically offer context-aware field names, comparison prefixes and values
with brief field descriptions. Keyboard selection and dismissal are supported;
suggestions assist but are not required for expression entry. Apply valid
expressions automatically after a debounce; Enter is not an Apply command.

Validate inline. Invalid or incomplete text retains the last valid applied
query and its results. Distinguish edited text from the query supplying the
results with shared unapplied styling and a clear applied-query indication.
Do not insert flashing Checking changes/progress text or repeatedly change
the toolbar height. A query-read failure is separate from invalid syntax:
report the failure while preserving the edited text and last complete results.

### Shared query state — Agreed (D351)

Preserve one client-local history-query state across views and application
restarts, including unfinished/invalid edited text and the last valid applied
query. For example, after applying `mode:SSB` and editing to `mode:SSB OR`,
restore both; show the incomplete expression and its diagnostic, and identify
`mode:SSB` as the query supplying results. Re-evaluate against current saved
data and the current selected view; do not persist stale result rows.

A view switch retains both expressions but still clears contact selection.
Subsequent valid text applies through the normal debounce. Query state is
local to the account workspace; it is not synchronized shared QSO data.

**E3 — Engineering defaults:** use 250 ms for validation/query debounce, distinct
from the directory lookup's separate 500 ms policy. Keep completion selection
in the query control: arrows select suggestions, Enter or Tab accepts one,
Escape dismisses; acceptance edits text and follows normal validation. Preserve
caret position and do not rewrite the whole expression into chips on each edit.
An explicit Clear query action empties the expression through this same path.

## 5. Expression language

### Syntax — Agreed

| Construct | Meaning |
| --- | --- |
| `W7HEX` | Worked-callsign prefix search, without base-call/portable expansion. |
| `field:value` | Named predicate with its field's default matching. |
| `field:=value` | Explicit equality; whole-value, case-insensitive for text. |
| `qso_start:>2026-10-27T18:00:00Z` | Named timestamp strictly after the bound. |
| `qso_start:>=2026-10-27T18:00:00Z` | Named timestamp at or after the bound. |
| `a b`, `a AND b` | Both must match, including repeated fields. |
| `a OR b` | Either may match. |
| `NOT a`, `-a` | Ordinary Boolean negation. |
| `(a OR b) c` | Group alternatives, then require `c`. |
| `has:field` | Field presence; ordinary negation finds missing values. |
| `comment:"first contact"` | Quoted multiword phrase. |

Precedence is NOT, then AND (including adjacency), then OR. There are no
same-field combination exceptions. For example:

```text
comment:activation comment:rain -name:alice
(comment:activation OR comment:rain) -name:alice
```

The first requires both comment substrings, while the second accepts either;
both exclude matching names. Dotted names identify useful nested concepts,
not every property of an internal data structure. The first colon separates
field and value; timestamp colons remain within the value. Operators are
optional prefixes within values, rather than SQL-style expressions.

Standalone, unquoted Boolean keywords are case-insensitive: `and`, `OR` and
`NoT` are operators. Field names are also case-insensitive. Quoted words and
field values remain literal, so `"or"` is a callsign term and `comment:and`
searches for that text. Longer words such as `orange` are not split into
keywords.

### E2 — Engineering defaults for parsing and validation

- Support `=`, `<`, `<=`, `>`, `>=` where permitted by the value type. Negation
  supplies inequality; do not add `!=`, query functions or wildcard syntax.
- Support quoted values and backslash escaping for literal `"`, `\`, `/`,
  parentheses and comparison punctuation. `comment:">5"` searches that literal
  substring; `comment:=">5"` requires that whole literal value. A minus before
  a field negates it; a minus within a numeric value belongs to that value.
- Diagnose unknown field names, unsupported operators and malformed values at
  the relevant text span. Do not silently drop a restriction or reinterpret
  an unknown named field as callsign text. Keep the last applied expression.
- Empty query means all active contacts in the selected view. A positive
  predicate against an absent scalar is false; its negation is true.
  `has:` can restrict a negated predicate to present values. Do not expose
  SQL's nullable three-valued behavior to the expression language.
- A collection predicate matches if at least one member qualifies. Repeated
  predicates remain independent: `tag:A tag:B` means membership in both named
  sets, not both conditions on one tag identity. Result rows never multiply.

## 6. Supported vocabulary and matching

The following core names and domain meanings are agreed. **E1 defaults** in
the tables complete ordinary matching/spelling details for specification
review. Optional additional operator terms are listed separately; no native
property is automatically a public query field.

| Core fields | Type and meaning | Matching |
| --- | --- | --- |
| Unqualified text | Recorded worked callsign | Prefix; slash components preserved. |
| `own.call`, `contact.call` | Recorded station callsigns | E1: prefix by default; `=` whole value. |
| `qso_start`, `qso_end` | Saved start and optional end | Named timestamp/date comparisons; no end fallback. |
| `name`, `comment` | Effective contact name / recorded comment | Case-insensitive substring; `=` case-insensitive whole value. |
| `tag` | Membership in tags with the supplied exact name | All identically named tags qualify; one QSO row. E1: case-insensitive name equality. |
| `frequency`, `receive.frequency` | Primary / effective receive frequency | E1: numeric equality or ordered comparison; MHz default. |
| `band`, `receive.band` | Primary / effective receive band | E1: case-insensitive whole-label matching, including unfamiliar labels. |
| `power` | Saved transmit power | E1: numeric equality or ordered comparison; watts default. |
| `mode` | Operator-facing operating mode | Mode family/specific-mode matching below; `=` has the same meaning. |
| `own.grid`, `contact.grid` | Effective station location within a queried grid | Geographic known-to-be-within matching below; `=` has the same meaning. |
| `own.activity`, `contact.activity` | Recorded activity participation | Group name from ordinary synchronized definitions. E1: case-insensitive exact name. |
| `own.reference`, `contact.reference` | Particular group-qualified recorded reference | One group/reference value. E1: whole-value case-insensitive group/identifier matching. |
| `lotw.uploaded`, `qrz.uploaded` | Current positive publication coverage | Boolean meanings and service contexts below. |
| `lotw.confirmed`, `qrz.confirmed` | Current applicable positive confirmation | Boolean meanings and service contexts below. |

### E1 — Additional operator terms and type defaults

Include the following as engineering-default vocabulary for review: `sent_report`
and `received_report` for exchanged reports; `equipment`, `antenna`, and
`equipment.notes` for copied own-equipment descriptions; and `own.operator.call`,
`contact.operator.call`, `own.operator.name`, `contact.operator.name` for known
recorded operator information. These express operator tasks, not raw object
paths. Reports and descriptions use the name/comment text defaults; operator
calls use the callsign defaults. `=` always gives case-insensitive whole-value
text matching. Operator callsigns are not inferred from station callsigns.

Use equality by default for numeric/Boolean values and ordering only for
numeric/time values. Booleans accept `true` and `false`; the equality prefix
has the same meaning. Do not offer text ordering or expose internal service
status enumeration names. Coordinate fields, raw provider fields and additional
geographic identifiers are not automatically included; adding vocabulary must
preserve the same typed language and the agreed native/fallback behavior.

Use locale-independent Unicode simple case folding for case-insensitive text
matching, without stripping accents, changing whitespace inside values or modifying
saved text. Quotes preserve contiguous phrases. Query compilation must honor
this behavior for names/comments, not rely on an ASCII-only comparison shortcut.

### Dates and units — Agreed

Contact timestamps have whole-second resolution. Date-only values denote full
UTC days: `qso_start:2026-10-27` matches starts anywhere that day;
`qso_start:>=2026-10-27` includes that day and later times;
`qso_start:>2026-10-27` begins after the day ends. Full timestamps give exact
comparisons against the named field.

Frequency defaults to megahertz (MHz); explicit compatible Hz/kHz/MHz suffixes
are supported. Power defaults to watts, with explicit W/mW supported. Query
values do not change native recording resolution.

**E2 defaults:** `<date` means before that day starts, `<=date` includes that
whole day and earlier times. Accept `YYYY-MM-DD` dates and full timestamps
with an explicit UTC `Z` or numeric offset, normalized to UTC; do not infer
the workstation time zone. Reject fractional contact seconds. Compare decimal
numeric thresholds exactly without rounding to stored resolution:
`frequency:>14.0000005MHz` matches 14,000,001 Hz but not 14,000,000 Hz.

### Mode matching — Agreed (D345/D346)

`mode:SSB` includes USB and LSB; `mode:USB` narrows to USB. Use the data-driven
mode relationships. `mode:=SSB` has the same meaning as `mode:SSB`; it does not
force a parent-only stored representation or require `mode.parent` navigation.
Unknown labels remain preserved; querying and recognition never rewrite them.

**E1 default:** match an unfamiliar term against its preserved mode label
case-insensitively, without guessing family membership. Known specialization
matching uses catalog recognition; no mode families are hard-coded.

### Grid matching — Agreed (D347)

A grid token means the station's location is known to be within that grid.
Match an explicit point inside the cell, or a grid area equal to or wholly
contained within it. A broader or merely overlapping area is insufficient:
`contact.grid:CN85` does not match a location known only as CN. A grid center
is not proof of an exact location. `contact.grid:=CN85` has the same geographic
meaning, not identical displayed-locator-string equality. Apply this to both
station sides and recorded variants, using the corresponding source binding.

### Activity and reference searches — Agreed (D348)

```text
own.activity:POTA
contact.activity:POTA
contact.reference:POTA/US-0001
own.reference:POTA/US-0001
```

Activity predicates search the indicated station's recorded participation;
reference predicates identify the group and reference together as one value.
Activity queries also support contests without individual identifiers. Resolve
group names through ordinary synchronized definitions; POTA is an example,
not a hard-coded token or privileged implementation. Catalog metadata or
provider geography does not create recorded activity participation.

**E2 defaults:** split a reference value at the first unescaped `/`, allowing
quoted/escaped group names and identifiers. Preserve delimiter-escape meaning
until the typed value is decoded. Match any recorded association whose group
has that exact queried name and whose identifier matches. Multiple definitions
with the same name qualify; missing definitions do not acquire guessed names.
Unknown associations stay retained. `has:own.activity` tests participation
presence, and `has:own.reference` tests for at least one recorded identifier,
including when its group definition is unavailable. Group entry/edit
normalization does not rewrite stored references during a query.

### Upload and confirmation searches — Agreed (D349)

| Predicates | Positive condition |
| --- | --- |
| `lotw.uploaded:true`, `qrz.uploaded:true` | Positive evidence covers current service-relevant values. |
| `lotw.confirmed:true`, `qrz.confirmed:true` | Positive confirmation applies to the current QSO. |

`false` negates the positive condition, including missing or no-longer-applicable
evidence; it does not assert provider rejection or explicit unconfirmed status.
Upload coverage and confirmation are independent. For example,
`lotw.confirmed:true lotw.uploaded:false` finds currently confirmed contacts
whose current service-relevant information lacks upload coverage. Preserve
historical evidence and the existing imported-positive/applicability rules.

#### Service context — Agreed (D350)

| Predicate | Context |
| --- | --- |
| `lotw.uploaded:true` | Successful submission or accepted imported evidence covers current LoTW-relevant values for the QSO's saved station identity. Certificate renewal/replacement does not invalidate coverage. |
| `lotw.confirmed:true` | Retained LoTW confirmation remains applicable. Changing download login does not invalidate it. |
| `qrz.uploaded:true` | Successful upload or accepted imported evidence covers current QRZ-relevant values in the QSO's applicable QRZ book. |
| `qrz.confirmed:true` | Confirmation associated with that QSO's QRZ record remains applicable under the evidence rules. |

QRZ uses the unique applicable book for the exact station callsign and QSO date,
with its API key selecting the book. Historical QSOs retain historical book
context; do not substitute today's/currently open book or aggregate unrelated
books. Preserve evidence and report inconsistent metadata/associations rather
than inventing a migration or treating configuration edits as QSO edits.

LoTW signed payloads authenticate uploads without an account-login parameter.
Certificates authorize station identities and QSO dates, not independent remote
logbooks. Personal callsigns can share an account; club accounts remain separate.
The download login controls report access and does not redirect signed uploads.
Retained personal-call confirmations do not disappear when downloading a club
account. See the [provider sources](../references/service-state-model.md#service-routing-and-account-identities).

Applicable imported positives count immediately even without destination
provenance. Missing credentials, paused/disabled uploads and failed downloads
do not invalidate positives. Query status does not mean upload-job eligibility.
An unchanged 2025 QRZ contact uploaded to a book covering 2025 stays uploaded
after a 2026 book is configured; an unchanged LoTW contact stays uploaded and
confirmed after certificate renewal. Relevant QSO corrections may make upload
coverage false while confirmation remains true.

### Field source bindings — Agreed (D344)

Ordinary name and location fields use effective history values: recorded,
eligible LoTW, eligible QRZ, then eligible directory information. Select a whole
location, including partial recorded locations. Do not patch fields from later
sources; location-bearing recorded references suppress fallback even if their
geography is unknown. Only information eligible for the named station side is
used. Querying changes none of these underlying policies.

| Fields | Source |
| --- | --- |
| Unqualified text, `own.call`, `contact.call` | Recorded station callsigns. |
| `name` | Effective contact name. |
| `own.grid`, `contact.grid` | Effective selected station location. |
| `*.operator.call`, `*.operator.name` | Saved operator information, without inferred callsigns. |
| `frequency`, `band` | Recorded primary pair. |
| `receive.frequency`, `receive.band` | Whole native receive override when present; otherwise primary. A band-only override does not borrow primary frequency. |
| `power`, reports, comments, equipment | Saved QSO facts, not current defaults or reusable-record values. |

Expose `recorded.name`, `recorded.own.grid`, `recorded.contact.grid`,
`recorded.receive.frequency` and `recorded.receive.band` to query saved facts
without fallback. Recorded receive fields inspect the optional override only.
Other already-recorded scalar fields need no duplicate prefix to change source.
Presence follows binding: `has:name` can find a fallback name, while
`has:recorded.name` requires a saved name; `has:receive.frequency` can use primary
frequency, while `has:recorded.receive.frequency` requires it in the override.
An absent field in the chosen whole location remains absent. Native position
identity and whole-location selection remain intact during grid matching.

## 7. Production integration and local persistence

Extend the existing desktop `ContactsUi`/session operations and client-store
history readers. Typed expressions supply the single history filtering contract.
Reuse real working-row/evidence selection and the existing
editor; do not build a throwaway query host, second persistence service or
an in-memory copy of the entire account as the execution engine.

**E4 — Engineering default:** place pure parsing, typed validation and the
operator field catalog in `clacks-model::query`, with store SQL compilation
and consistent reads in `clacks-client-store`, and editing/suggestions/result
presentation in `clacks-desktop`. Model code knows no SQL; the desktop does
not independently reinterpret predicates. Keep the catalog, help, suggestions
and execution in agreement. Use winnow's expression parser for Boolean
precedence, with combinators for grouping, adjacency, keyword boundaries,
quoted/escaped tokens, comparison prefixes and decimal syntax. Located input
retains byte ranges for diagnostics; typed-value interpretation remains separate
from syntax.

**E5 — Engineering defaults:** compile validated predicates to relational SQL
with bound values and catalog-controlled field expressions. Never interpolate
query text or user-supplied identifiers as SQL. Keep fixed SQLx queries in
files with compile-time metadata; runtime-variable expression structure uses
normal bound-query tooling. Use existential joins/subqueries or distinct QSO
identity before paging/counting so tags/references/evidence cannot duplicate
rows. Do not reconstruct a second service-evidence state machine in SQL;
preserve the existing domain coverage/applicability contract at that boundary.

Use bounded, consistently ordered result pages. Results, totals and hidden-
selection counts must describe the same applied view/query state. Do not cap
the logical search to the first fetched page or equate off-page with filtered
out. **E5 default:** return matching-page data and counts from one ordinary
read snapshot; use 100-row pages initially, with stable ID ties. The concrete
page/cursor representation is implementation work, not another operator policy.

Per issue #88, these pages are internal reads, never user-facing navigation.
History and both Worked Before presentations show all matching rows in one
continuous virtual table. The scrollbar spans the complete result count; jumping
to a distant position reads the needed rows without fetching intervening pages.
Keep only pages needed by active viewports and pending keyboard navigation.
Loading rows retain their logical positions. Preserve usable results after a
read failure and offer retry. Worked Before shares query/data and column choices
while its tab and dedicated window retain independent scrolling and focus.
Show selected uses the selected-contact count for its logical table extent,
independently of the expression's matching count.

**E8 — Engineering default:** persist edited/applied query text together as one
versioned native preference value under `history.query` in the workspace's
existing `local_state(key, value)` table. Use normal local write transactions;
do not create a dedicated per-preference table or synchronize this preference.
The absent-key default is two empty expressions. Do not persist SQL or use
serialized predicates as authoritative QSO data. Schema compatibility and
failed preference writes follow existing store error handling; never present
a failed persistence operation as durable success.

Reload preserved expressions through the current parser/catalog. While valid
edited text is waiting for debounce, the last applied query remains active.
Invalid text never launches a query. Parser errors or a failed read do not
silently broaden results to an empty query. If restored applied text cannot
be evaluated, show that failure rather than claiming unfiltered rows match.

## 8. Refresh, concurrency and observability

Refresh relevant reads after local saves, edits/deletions, view changes, newly
accepted working state, eligible provider-snapshot changes, and catalog/group
updates that affect query interpretation. This is an integration boundary for
future sync/provider adapters, not a requirement to implement them in #10.
History-query changes do not constrain Worked Before. Clearing/changing the
entry callsign prevents older Worked Before results from replacing the new
state. Query/view/sort changes likewise prevent stale history results from
replacing current ones. Preserve pending edits and selection by stable ID.

Use the application-owned asynchronous session/runtime. Database work must not
block rendering, and repainting must not restart unchanged reads. **E5 default:**
correlate requests with their complete view/query/sort inputs and normal request
identity, avoiding a second custom UI generation/cache-invalidation framework.
Choose source projections within one consistent local read; query evaluation
must not change a QSO or request remote data to fill missing information.

Follow [observability](observability.md). Instrument logical parse/validation,
query execution and read-handling boundaries with ordinary tracing facilities.
Keep domain diagnostics such as invalid field/operator or stale-result discard
focused and appropriately leveled. Do not log keystrokes, full query text,
contact names/comments, provider payloads or credentials, or use arbitrary
field values as metric labels. **E9 default:** store query execution latency,
result/failure counts and bounded outcome classes only where they answer an
operational question; do not create per-widget or per-token telemetry machinery.

## 9. Verification and completion

Verify specific Clacks requirements with independently stated expected results,
not parser/database/framework behavior in isolation. Use a small shared native
contact/evidence scenario set for the semantic and store assertions:

- Boolean grouping and repeated-field AND, phrase/exact text matching, named
  timestamp/date bounds, unit-equivalent comparisons and fractional thresholds.
- SSB versus USB family matching; known-within grids across point/area precision;
  effective versus recorded name/location/receive presence without mixed sources.
- Duplicate-name tags and multi-tag contacts producing one row; group-qualified
  references staying paired in one predicate; group definitions remain data.
- Service-specific current coverage/confirmation, imported positives, credential
  replacement and relevant corrections, without simulating a remote provider.
- Real relational internal paging/count/selection and query-state persistence
  over a restart; unfinished text and applied results remain distinguishable.
- The assembled query bar's completion/error flow, Enter handling, same-row editor
  request, view-switch selection clearing and stale-response suppression. Use
  egui_kittest's native facilities; do not create custom UI drivers or repeat
  model/store scenarios through each widget.
- Worked Before prefix changes/empty input, independent history expressions,
  working edits/deletions, older rows and a UTC-day transition using controlled
  time instead of real sleeps.
- Full-result scrolling beyond internal read boundaries, direct scrollbar jumps,
  keyboard activation across unloaded boundaries, selected-only extent,
  independent Worked Before viewports, stale-read suppression and read retry.

Inspect the production workbench under the [UI completion gates](desktop-ui.md#ui-change-completion-gates):
query bar and popup placement, unchanged entry/details arrangement, pane bounds
while empty/loading, single-row Worked Before, resizes, fonts/scales, light/dark
appearance and additional-window behavior. Functional tests do not establish
visual conformance. Keep native Windows acceptance and any unavailable checks
explicit. Existing continuous integration (CI) is temporarily Linux-only;
this issue does not authorize reintroducing Windows CI tests.

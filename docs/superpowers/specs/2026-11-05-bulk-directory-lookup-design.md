# Selection-based bulk directory lookup

Engineering specification for [issue #22](https://github.com/jhelwig/clacks-log/issues/22).
Status: approved in the specification discussion on 5 November 2026;
implementation has not started. Product and architecture decisions remain
authoritative. The engineering defaults below are accepted for this delivery,
not additional project-wide product requirements.

## Outcome and authority

An operator starts directory lookup for selected saved QSOs through the native
Edit menu. Select all selects every contact matching the applied history filter
in the current view, across all pages. Successful results
update separate contact-owned directory snapshots as they arrive. Recorded QSO
facts remain untouched. The operation reports partial outcomes and preserves
completed updates when cancelled.

| Requirement | Source |
| --- | --- |
| User-triggered selected/filtered lookup; account-selected provider | D99, D196; [product](../../product.md) |
| Separate directory facts and existing whole-source fallback | D198–D204, D238; [native model, section 12](../../native-model.md#12-source-separation-equality-and-component-interfaces) |
| Apply results as they arrive; report afterward without preview approval | D205 |
| Whole-snapshot replacement; preservation on failure/not found; cancellation retains completed updates | D206 |
| Current normalized-key eligibility; deterministic latest-successful arbitration | D207, D209 |
| Selection includes identities outside the applied filter and visible pages | [History selection](../../history.md#selection-and-counts--agreed) |
| Visible progress, cancellation and retained partial-result reports | [Operation feedback](../../desktop-ui.md#operation-feedback) |
| Reuse existing lookup and in-process cache behavior | User direction in this issue's specification discussion |
| One selection-based lookup action in Edit; Select all covers matching contacts across pages; no lookup toolbar button or separate matching action | User correction and direction in this issue's specification discussion |
| Outcome totals; contact details for failures/unprocessed contacts; reasons useful for distinguishing a later retry from corrective action | User direction in this issue's specification discussion |
| Meaningful operations, safe diagnostics, truthful counts and bounded measurements | [Observability](../../observability.md) |

“Refresh” means replacing a contact's stored directory snapshot. It creates no
requirement to bypass the in-process cache, obtain a new provider response for
each run, or change the existing lookup ordering rules.

This capability adds no general bulk editing, automatic reconnect enrichment,
provider fallback/merging, persistent reusable result cache, durable job history,
restart/resume protocol, or report export. Existing supported directory fields
and provider mapping remain unchanged.

## Existing facilities and approach

The inspected baseline is `424158b` on `main`.

- `clacks-desktop::directory::lookup::LookupClient::request` reuses a transient
  success, joins an in-flight operation, or admits a provider request. Ordinary
  entry and WS Suite capture use this same client.
- `lookup::cache` retains at most 128 successes for 24 hours from successful
  completion. Cache access and configuration changes do not extend that age.
  Results retain their original provider, lookup time and lookup identity.
  Failures and not-found results are not cached.
- `lookup::runtime` owns installed configuration, rejected-material pauses,
  provider-requested quiet periods, workspace shutdown and tracked tasks.
- `directory::providers` already distinguishes authentication, rate limits,
  transport, subscription, refusal, parsing and invalid-response failures.
  Some distinctions become safe strings at the lookup boundary today.
- `clacks-client-store::directory::snapshots::apply` checks the live parent and
  normalized key in its write transaction, then uses existing directory-record
  arbitration and ordinary pending synchronization work.
- `ReadSnapshot::record` and `ReadSnapshot::contact_page` supply consistent local
  working data. The history query compiler owns view/filter semantics.
- History owns the actual selected identities and the applied expression.
  Selected-contact membership editing already uses cancellation and progress
  channels, but its write semantics are specific to tag changes.

Extend these facilities with one desktop bulk workflow. A separate lookup engine
would duplicate admission, authentication, caching and provider behavior. Routing
the whole operation through the ordinary contact-action worker would hold up
other contact actions while awaiting providers: the session admits those actions
serially. Neither alternative serves the requested workflow. No new service,
write queue, database table, migration or synchronization message is needed.

Tokio 1.54.1 and tokio-util 0.7.16 are locked dependencies. Their source
documentation and the existing application integration supply the needed
facilities: `watch` retains the latest progress value, `CancellationToken`
signals cooperative cancellation, and `TaskTracker` waits for tracked work to
exit after closure. Tracker closure alone does not prevent spawning; retain the
existing admission lock and closed-state check. Database transaction boundaries,
contact eligibility and report classification remain Clacks responsibilities.

## Scope capture

Provide one command, **Edit → Look up directory information**, acting on the
history selection. Disable it when no contacts are selected, Select all is
still preparing selection, or a bulk run is active. Starting lookup needs no
preview or approval. There is no directory
lookup toolbar button, scope submenu or separate all-matching lookup action.
Use the existing native-menu integration, including the screen-level menu bar
on macOS; a menu drawn inside the history toolbar is not a substitute.

**Edit → Select all**, when contact history is the selection context, adds every
active contact matching the applied expression in the current logbook/view to
the ordinary selected-ID set, including matches on unloaded pages. Preserve
already-selected contacts outside that expression and include them in the
existing outside-filter count. This extends the same selection used by other
contact actions; it is not a special bulk-lookup scope. An empty applied
expression selects all active contacts in the current view, including built-in
All contacts and Untagged views under their existing rules. Ignore unfinished
filter text and the temporary Show selected inspection mode. Respect native
text-selection behavior when an editable text control has focus; Select all
must not select contacts instead of text in that context.

Resolve Select all from one consistent local read snapshot using existing
bounded contact pages, a stable order and `show_selected = false`. Publish the
complete selected-ID set only after enumeration succeeds. Show **Selecting
contacts…** while preparing and disable selection-consuming actions until it
settles, so lookup cannot silently use the preceding selection. Keep preparation
off the UI thread, retain the previous selection on failure/cancellation, and
reject a late selection result if the selected view, applied expression or
selection intent has changed. Selection changes remain local and make no
provider requests or writes.

When lookup starts, copy the actual selected QSO identities, including contacts
outside the applied expression and loaded pages. Do not intersect that set with
the expression or visible rows, or re-evaluate Select all as a live filter.
The worker resolves those IDs from one consistent local read snapshot before
requesting results or writing snapshots, using existing record reads. Retain only
target identity, captured normalized key and report identification (recorded
callsign and UTC start time). UTC means Coordinated Universal Time. Release the
read snapshot before provider waits or writes. Do not add a second expression
evaluator or copy history queries into another SQL implementation.

The frozen target list prevents new lookup results from changing membership in
a filter that refers to effective directory values. Subsequent filter edits,
scrolling, selection changes, view changes and incoming contacts do not alter
the admitted operation. A selected identity already unavailable during capture
is reported unprocessed. Contacts deleted or changed to another key afterward
are handled by the existing application guard. Tag changes alone do not remove
a frozen target. Pending local edits and conflicts use the same saved local
working contact as history; pending deletions are excluded.

Scope preparation is cancellable and displays **Preparing contacts…**. Before
preparation completes there have been no bulk snapshot writes. If preparation
fails or is cancelled, state that lookup never started. Selected IDs are already
known and can be reported unprocessed. Once preparation succeeds, every target
has exactly one final outcome.

## Execution and lifetime

Engineering default: allow one active bulk operation in this history workflow,
processing targets sequentially in the frozen order. Disable the lookup command
until it settles. This bounds provider demand without a parallel scheduler or
additional per-key batch cache. Repeated keys use ordinary lookup/cache behavior,
including ordinary eviction; there is no batch-wide promise of one network
request per distinct key. A later performance change needs actual evidence.

The ordinary contact-action path admits the run and returns its progress/report
handle promptly. Execute preparation and the run on the existing workspace
directory task tracker, subject to its closed-state admission guard. Provider
waits do not occupy the serial settings/contact worker. Preserve the process
subscriber and tracing context when spawning, following session conventions.

Each target uses its captured normalized key with `LookupClient::request`.
There is no entry debounce, cache bypass, timestamp refresh, provider pinning,
extra authentication reset, or automatic retry. New network admission uses the
then-current account configuration. Existing in-flight and cached results retain
their original provider/time/identity, including across configuration changes,
exactly as in ordinary lookup. Do not relabel an older result as a lookup made
by this run.

On success, apply through the existing snapshot transaction, retaining whole
replacement and current-key checks. Notify existing synchronization/history
change consumers after a committed snapshot write and wake the native UI.
Provider success and actual local application are distinct outcomes.

Continue after an individual not-found, transport, response, subscription or
refusal failure; report its safe reason. Batch scheduling default:
stop when lookup reports rejected authentication, a provider quiet period/rate
limit, absent/unusable directory configuration, or a storage failure. Classify
the affected target and explain why the remaining targets were unprocessed.
This avoids walking the remaining list through a shared unavailable condition.
Use the current runtime's admission state when deciding whether such a condition
still blocks the remainder. A late authentication rejection or rate-limit response
for replaced material must not stop the run solely because of that obsolete
material; report the affected target and continue if current admission is usable.
Do not wait out rate limits or automatically restart on credential replacement,
reconnection or provider recovery. Explicitly starting another run uses the
ordinary recovered lookup behavior. Stopping this run does not pause logging,
capture, synchronization or unrelated lookup consumers.

Changing history views preserves access to the active progress/report and Cancel;
it does not cancel the run or retarget it. Dismissing a completed report clears
only the report. Workspace shutdown requests cancellation and drains admitted
work through the existing tracker before storage closes. Reports are session
state, not synchronized records; restarting retains committed snapshots but does
not resume the run or restore its report.

## Cancellation and snapshot accounting

Check the run's cancellation token and workspace stop signal during preparation,
before requesting the next target, while awaiting a shared lookup result, and
immediately before starting a snapshot write. Cancelling this consumer must not
abort a shared provider operation needed by entry, capture or another consumer.
That provider operation may still finish and populate the ordinary transient
cache; the cancelled bulk consumer does not apply its result afterward.

Once a snapshot write begins, await its ordinary transaction outcome. Do not
race cancellation against a write future or abort the task to implement Cancel.
A commit acknowledged after the cancellation request still counts as completed;
then stop before the next write. Show **Cancelling…** until that boundary settles.
There is no rollback of earlier successful writes.

Extend the existing snapshot application result narrowly so the caller can
classify successful application from the winning snapshot read and written in
that same transaction. A separate before/after read outside the transaction
cannot establish the report result in the presence of synchronization.

| Report total | Meaning |
| --- | --- |
| New/updated | A winning snapshot was committed and either no current-key snapshot existed, or its provider or complete directory facts differ. |
| Unchanged | Successful lookup produced no material change: an equal/newer existing winner prevented replacement, or a committed replacement has the same provider and complete facts. |
| Not found | The provider explicitly returned no record; no snapshot replacement occurred. |
| Failed | Lookup failed or its local snapshot write failed; show the safe cause and next-step guidance. |
| Unprocessed | No applicable result was applied or settled as another outcome: cancellation, early stop, unavailable parent, or changed lookup key. Show the specific reason. |

Compare provider and complete `DirectoryFacts` using existing native equality.
Lookup time and identity alone do not make a material change. A newer identical
result still replaces the snapshot under ordinary arbitration and synchronizes
its metadata; the report calls it unchanged. An older losing candidate is also
unchanged because the retained winner did not change. It must not be reported
as a committed refresh. Preserve distinctions between commit, material change
and arbitration refusal in the internal result for truthful instrumentation.

All final totals sum to the frozen target count after successful preparation.
The totals describe this run's settled application outcomes, not a claim that
later concurrent synchronization cannot replace a winner.

## Progress, report and actionable failures

Show processed/total progress, current safe status and Cancel without blocking
the workbench. After settlement, show all five totals, whether the operation
completed, was cancelled, or stopped early, and an expandable contact-level
list for failures and unprocessed contacts. Identify rows with recorded callsign
and contact start time, retaining the QSO identity internally to distinguish
duplicate contacts. A changed/deleted target keeps its captured identification
and a clear explanation. Ordinary success needs no modal or per-contact toast.

Failure rows explain what was observed and what the operator can do:

| Observed failure | Guidance |
| --- | --- |
| Connection/transport failure | Check connectivity; retry later. May be temporary; do not claim a proven cause. |
| Rate limit/quiet period | Provider is limiting requests; retry later, respecting any known remaining delay. |
| Rejected credentials | Check/update the account's directory credentials; use the existing explicit authentication retry where appropriate. |
| Subscription restriction | Check the selected provider account's directory access/subscription. |
| Provider refusal | Provider refused the request; check account/service status. Whether a later retry will succeed is unknown. |
| Unreadable/invalid response or invalid mapped facts | Explain the response problem; retry later or investigate if it persists. Do not assert a transient or credential cause. |
| Missing/disabled configuration or protected-storage access failure | Configure the account directory or use the existing directory-storage recovery action, as applicable. |
| Local snapshot storage failure | Show a safe local diagnostic and corrective guidance when known; do not describe it as a provider failure. |

Keep a typed, secret-safe reason through the lookup boundary where strings
currently discard the provider category. Add only distinctions consumed by this
workflow and existing callers; do not parse display strings, invent error codes
from raw provider text, or introduce a generic failure/recovery framework.
Continue existing entry/capture messages and behavior. Known retry timing and
configuration states come from the existing runtime, not duplicated batch state.

Failure and unprocessed details remain available without a timer until explicitly
dismissed or replaced by a new run. Do not add automatic retries, a new batch
authentication workflow, or a special failed-contacts retry mechanism here.
Operators may correct the cause and start another lookup for their selection.

## Observability

Own the run span and measurements in the desktop bulk workflow. Record an
`info` span for the explicit run, correlated by a fresh run identity and account
identity; record prepared target count and terminal totals without
callsigns, expressions, directory facts, credentials or provider payloads.
Existing provider lookup and store spans retain their actual lookup/QSO
correlation. Propagate the run context through asynchronous work; shared lookup
operations retain their original owning context rather than being reassigned to
every joining consumer. Application diagnostics can record target and lookup
identities deliberately to explain a failed or ineligible application.

Normal completion/cancellation and explicit early-stop decisions remain visible
at `info`; degraded provider/configuration conditions use the existing appropriate
warnings. The handling boundary reports storage/run failures at `error`, with
safe categories and correlation, avoiding duplicate logging of propagated errors.
Supporting cache and arbitration decisions remain at their existing diagnostic
levels. No per-target success log beyond the existing storage reporting is needed.

Define separate bulk-run count/duration and contact-outcome metric families.
Run count/duration distinguish completed, cancelled, stopped and preparation
failure, measured from admission through terminal settlement. Contact outcomes
are counted exactly once after classification, with only the five bounded report
categories as dimensions. Do not use run/account/QSO identities, callsigns,
expressions, error messages or provider payloads as labels. Existing provider
metrics continue counting actual admitted lookups/network requests; cached or
joined bulk consumers do not inflate them. Count a metadata-only committed
snapshot as a store commit and an unchanged report outcome, not as a material
update. Record run duration at the workflow boundary; use tracing's own span
lifecycle rather than synthetic timing events. Publish compact latest progress
and retain one terminal report; do not clone the complete growing detail list
on every target or add a telemetry emission framework.

## Verification and delivery boundary

Use existing integration suites, concrete provider fixture servers, temporary
workspaces and production storage operations. Each added assertion must expose a
specific Clacks integration or domain mistake. Do not repeat dependency tests
or the complete ordinary-lookup suite through the new UI.

- Selection and triggering: native Edit commands enable/dispatch against actual
  history selection. Select all covers every matching page using the applied
  expression/current view, independently of unfinished text and Show selected;
  it preserves outside-filter selection and native text-selection behavior.
  Pending Select all cannot dispatch lookup against the previous selection.
  Late Select all results cannot replace a newer view/filter or selection intent.
  Lookup includes selected contacts outside the filter and loaded pages.
  Directory updates that change filter matches do not change the frozen targets.
- Shared lookup: prime the existing cache or join an actual pending fixture
  lookup, run bulk application, and verify the intended contacts receive that
  original snapshot without a bypass or invented timestamp. Cover ordinary
  provider failure categories through the report rather than string matching.
- Persistence/accounting: save and reload real contacts/snapshots; verify
  whole-snapshot replacement, unchanged recorded facts, metadata-only unchanged
  reporting, latest-winner refusal and current-key/deleted-parent handling.
  Extend existing snapshot integration cases rather than duplicating arbitration.
- Partial completion: a mixed-outcome run produces exact totals and safe
  contact-level reasons. Shared authentication/rate/configuration stops and an
  actual storage refusal preserve earlier commits and mark remaining contacts
  unprocessed. Do not manufacture database failures with triggers or dropped
  production tables.
- Lifecycle: cancellation while awaiting shared lookup prevents later bulk
  application without harming another consumer; cancellation at a write boundary
  retains the acknowledged commit. Shutdown drains writes. Other contact actions
  remain admissible during a delayed bulk provider response.
- UI: verify native menu admission/dispatch through existing menu integration
  checks. Use `egui_kittest` semantic controls, actual action/callback assertions
  and snapshots for selection feedback, progress, cancellation and report.
  Include long diagnostic rows and the surrounding history layout. Select
  production-app inspection according to the affected UI completion gates; any
  development launch must use the user-owned machine inventory and explicit
  isolated workspace/configuration/log paths.
- Reporting: verify truthful counts and deliberate data exclusion within these
  workflows through the existing process-owned telemetry composition. Do not
  assert tracing trees, event spellings or telemetry-library formatting. Review
  ordinary-level diagnostics for a mixed run and an early stop.

The implementation plan derives from this approved specification. Exact module
and type names, control arrangement, page size, report-list rendering and the
smallest adequate checks are builder choices within these contracts. Reuse the
current theme, native menus, controls, query/store operations and task facilities.
Update owning history, desktop UI/menu and usage documents when behavior is
delivered; keep issue #22 open until
the actual workflow and verification evidence exist.

# Production observability

D230 requires full production-grade server observability for the minimum viable product (MVP): tracing, structured logs and metrics for server operators. This applies to the one Clacks server application selected in D213/D223, including its web, synchronization and background-work paths.

## Operator and account-user needs

Operator telemetry and account-user diagnostics serve different audiences. Operator observability must include security events such as client enrollment/revocation, service-credential changes, certificate import and explicit supersession. Exclude passwords, tokens and private keys from telemetry. Exact event fields and access/retention arrangements remain specification work.

A retained account-scoped security-event history view in the web interface is post-MVP under D230. Existing account-isolated upload diagnostics and client synchronization status remain MVP under D66/D142. Deferring that history view does not reduce operator observability to basic diagnostic logs.

Operational telemetry is distinct from authoritative submission history, remote observations and synchronization/recovery evidence. Existing retention and recovery requirements for those application records still apply independently.

## Continuous engineering requirement

Tracing, structured logs and metrics are requirements for current and future
production code. They are developed and reviewed with each capability rather
than added only during release preparation. This includes shared libraries,
the desktop, web/server requests, synchronization, persistence, imports/exports,
background jobs and external-service adapters. Server-runtime setup in the
roadmap establishes collection and export; it does not defer instrumentation
of components delivered earlier.

Specifications and implementation plans identify the important operations,
fields, outcomes, measurements and failure paths for their scope. Reviews assess
whether an operator can diagnose failure, latency, load and stalled work using
that evidence. Passing functional tests alone does not establish observability.

## Instrumentation ownership

Libraries emit operation spans, structured events and measurements where the
behavior originates. Application composition configures subscribers, metric
recorders, filtering, sampling and export. Libraries must not install process-wide
telemetry, choose a collection backend or write operational logs directly to
standard output. Thin application wrappers retain the workspace's normal
composition responsibilities.

Instrument meaningful operations and useful diagnostic branches, including
expensive work and explicit conditional outcomes. Scalar getters and each
iteration of a hot loop do not need individual spans. Summarize batches with
counts and outcomes while retaining enough context to investigate a problematic
record deliberately.

Pure domain calculations remain independent of network, storage and telemetry
export. Optional observation of their execution must not change returned values,
synchronization semantics or domain serialization. They may measure planning or
calculation work; only the actual persistence or provider boundary can report a
committed effect or an explicitly successful external operation. Retried planning
must not inflate metrics named as durable QSO saves or completed uploads.

## Traces and structured events

Give operations stable span names and use module-path targets to identify their
component. Record useful domain outcomes and work counts; use framework facilities
for span duration. Do not duplicate span names or targets in `operation` or
`component` fields. Correlate requests,
synchronization sessions, jobs, attempts and downstream operations across async
and network boundaries. Define the propagation and trust rules when those
boundaries are implemented; IDs and timestamps alone do not establish causality.

Retain meaningful distinctions such as unchanged, stale, review required,
incomplete support, cancellation, retry, rejection and unexpected failure.
Expected conditional outcomes are not automatically error-level events. Record
an error at the boundary responsible for handling it, with its useful cause and
context, rather than emitting the same failure at every layer.

Use structured fields, not message-string parsing, as the operational interface.
Select fields explicitly rather than automatically recording arguments, whole
QSOs, account state, arbitrary user text or provider bodies through debug
formatting. Include such data only deliberately for a documented diagnostic
purpose. Passwords, authentication/access tokens and private keys are excluded,
including from headers, URLs, error chains and provider diagnostics. Correlation
fields may identify accounts or records in access-controlled operational traces;
that does not make them suitable metric dimensions or account-user-visible logs.

## Level-selection policy

Choose each span and event level from its concrete diagnostic question, operational
significance and expected volume. Being library code, a pure calculation or a
proposed change does not by itself determine its level. Specifications and reviews
must explain which important activity remains visible with `debug` and `trace`
filtered out, as well as which detail requires deliberate investigation.

| Level | Use |
| --- | --- |
| `info` | Significant normal operations, explicit user actions and recovery milestones worth retaining during ordinary operation. Names and fields distinguish attempts, proposals and committed effects. |
| `warn` | Detected anomalies or degraded conditions that warrant attention, even when recovery is supported. A witnessed server-history regression is a warning. Expected conflicts, retries and incomplete support are not automatically warnings. |
| `error` | An operation failure requiring error reporting at the boundary responsible for handling it. Do not log the same propagated error at every layer or treat normal validation/review outcomes as unexpected failures. |
| `debug` | Routine decisions, calculations and supporting detail useful for troubleshooting, including ordinary comparisons, projection/acceptance evaluations, staging and collection calculations. |
| `trace` | Fine-grained or frequently repeated investigation detail, such as reference-definition compilation and evidence-dependency checks. |

An operation span describes the attempted operation; its level does not certify
success. A significant event may have a higher level than its enclosing diagnostic
span. Ensure its necessary identifiers remain available at that level through an
enabled meaningful parent or explicit event fields. Do not depend on a filtered-out
span for essential context, mutate caller spans, or duplicate framework lifecycle
reporting to compensate for filtering. Metrics are independent of these levels.

The current `sync.recovery.fork.plan`, `sync.client.resolve` and
`sync.service.recovery.plan` spans use `info`. The `client.resolution_prepared`
and `service.recovery.planned` events use `info`; neither asserts a durable commit.
The self-contained `history.regression_detected` event uses `warn` even when its
`debug` history-check span is disabled. Routine review and missing-support detail
remains `debug`.

## Metrics and production behavior

Define units, aggregation and recording boundaries. Cover relevant work rates,
outcomes, latency distributions, batch sizes, queues/backlogs, retries and resource
pressure as those facilities are implemented. Planning measurements and committed
business effects have distinct names and ownership.

Metric labels have bounded value sets, such as operation, provider, outcome or
error class. Account/client/QSO/record/job IDs, callsigns, raw paths, timestamps,
error messages and arbitrary user-supplied values are not metric labels. Bound
histogram buckets and the number of series independently of account growth.

Telemetry must have controlled cost in hot paths. Runtime collection uses bounded
buffering and documented overflow/shutdown behavior; it must not perform
unbounded or blocking exporter work in operation paths. A missing subscriber,
recorder or unavailable telemetry backend must not change application correctness
or prevent normal operation. Trace sampling and log filtering must not silently
turn business counters into samples or suppress required security-event recording.

## Verification and remaining integration choices

Tests verify Clacks's reporting decisions through real instrumented operations.
A test collector is an observation tool, not the behavior under test. Check that:

- Proposed updates are reported as plans, not committed saves or successful uploads.
  Count durable effects at their actual persistence or provider boundary when those
  adapters exist. Repeated planning does not inflate a durable-effects counter.
- Stale proposals, required review, missing support and provider readiness are
  classified as their actual expected outcomes, not unexpected application failures.
- Counts describe the actual result: supplied versus deduplicated contacts,
  proposed versus staged records, and completed versus unprocessed work where
  those workflows exist. Do not invent import or cancellation behavior in a library
  that does not implement it.
- Nested and concurrent Clacks operations retain the correct request, account or
  job context without borrowing context from another operation.
- Clacks's selected fields exclude arbitrary payloads and sensitive values.
  Exercise credential-handling adapters with recognizable test secrets when those
  adapters exist; do not manufacture authentication facilities just for this test.
- Actual emitted metric dimensions remain bounded as account and record IDs vary.
- Configured module filters select the intended Clacks diagnostics while domain work
  still completes. Verify observed visibility rather than framework-generated target strings.

Do not test the telemetry libraries' event formatting, histogram implementations,
subscriber installation or general tolerance of an absent subscriber/recorder.
Do not duplicate the functional suite under every telemetry configuration.
Add a configuration-sensitive test only for a concrete Clacks failure risk, such
as necessary work being placed behind an enabled-span guard or fields being
recorded on a caller's span when the Clacks span is filtered out. Exporter failures
belong to tests of the actual runtime/export integration once implemented.

Desktop runtime reporting checks use the production telemetry initialization in
the existing isolated-process composition tests, including its normal logging
filter and provider lifetime. Use ordinary production export deadlines for
successful cases; abbreviated deadlines belong to cases that explicitly test
slow or unavailable collectors. Keep narrow library-producer captures separate
from verification of application composition.

Use ordinary existing functional tests to verify return values, domain data,
synchronization semantics and serialization after instrumentation changes. Keep
test collectors and fixtures in tests; no public production API exists solely
to support telemetry tests.

The current-library producer interfaces and instrumentation schema are defined
below. The [desktop specification](desktop.md) defines desktop composition and
engineering defaults. Concrete dependency/feature verification belongs to its
implementation plan. Server composition uses standard W3C trace-context
propagation and opt-in OTLP HTTP/protobuf export of all three signals over
Rustls; see [Linux deployment](../deploy/server/README.md). Collection/visualization,
access, sampling and retention remain deployment responsibilities. No vendor or
additional application service topology is selected by this contract. Desktop diagnostics and any telemetry export must respect the
existing offline-operation and credential-handling requirements; this requirement
does not introduce automatic upload of desktop diagnostics to a hosted service.

## Desktop runtime composition (D274/D275/D277/D278)

The [desktop specification](desktop.md#8-observability-composition) owns concrete
runtime composition: local rotating JSON logs at `info`, bounded nonblocking
writing, retained-file limits, optional console output and opt-in OTLP export
of traces, metrics and logs. Export uses HTTP/protobuf with Rustls. Local logs
continue with export enabled; offline use requires no collector. The specification
defines startup configuration, failure/overflow/shutdown behavior and acceptance
through real desktop operations. Use standard tracing/appender/metrics adapters
and exporters, preserving this document's instrumentation and level policies.
No custom telemetry or span-lifecycle framework is introduced.

Desktop composition caps OpenTelemetry SDK and HTTP transport diagnostics at
`warn`, even with a more verbose user filter: their debug diagnostics can contain
endpoint query strings or response bodies. Collector-provided `error_message`
events are suppressed at every output level as well. These backend targets are excluded
from the log-export bridge to avoid feedback. Safe SDK warnings remain in local
output. Operational sink failures degrade diagnostics rather than prevent local
storage; invalid supplied configuration is still a startup error.
Runtime file I/O failures set a persistent UI diagnostic and move the standard
writer's output to stderr; queue saturation remains a separate dropped-line count.
The narrow I/O adapter neither formats events nor owns rotation, queues or workers.

## Library producer contract

`clacks-model` and `clacks-sync` emit through `tracing` 0.1 and `metrics` 0.24.
They install no subscribers, recorders or exporters. Public signatures, concrete
errors, domain serialization and synchronization behavior remain unchanged.

### Framework-owned lifecycle and context

Use `#[tracing::instrument(skip_all, ...)]` directly on meaningful operations,
with explicit safe input fields. Select levels according to the
[level-selection policy](#level-selection-policy); do not assign a uniform level
to all library operations. Keep the default module-path
target for spans and events, allowing crate/module filtering. Static span names
and fields identify model interpretation, profile/tag planning, export/projection,
client preparation/receipt, admission, transfer, recovery,
compaction and service-evidence operations. Getters and inner loops do not receive
blanket spans.

Annotations provide function scope across ordinary returns and `?` propagation.
Use the framework's lifecycle support rather than moving function bodies into
closures, capturing every result, emitting synthetic completion events or adding
manual operation timers. Libraries do not generate their own close records or
latency histograms for those spans.

Application composition configures lifecycle presentation and export through
standard subscriber/exporter facilities. For example, `tracing-subscriber`'s
formatter can enable `FmtSpan::CLOSE`; with timestamps enabled, its close records
include busy time (time entered) and idle time (time alive but not entered).
A tracing exporter can retain span start/end timestamps. These are framework
facilities, not reasons to implement a parallel timing protocol in domain code.

Span timing does not automatically update the `metrics` facade. Explicit
request/job latency histograms, when needed, belong at the runtime workflow or
adapter owning that measurement and must have deliberate recording boundaries.
Do not claim a trace duration is an unsampled operational counter or histogram.

Span names identify operations and module-path targets identify components;
separate `operation` and `component` fields are redundant and omitted. Spans carry
available input IDs and useful domain context. Omit static `phase` fields that
only restate the operation name.
Native IDs use their `Display` implementations rather than payload debug dumps. Account context is inherited
where the native inputs lack an account ID; do not add parameters solely for tracing.
Subscribers/exporters include enclosing span context as configured. Domain events
need their own meaningful fields, not a copy of every parent field.

Events use ordinary contextual parentage. Filtering an operation span may leave
an event under the nearest enabled caller or without a parent. Event names and
selected domain fields retain their meaning without the span. Never recover an
annotation's span with `Span::current()` or mutate a caller's fields.

Prefer existing `Display` (`%value`) or `Debug` (`?value`) implementations for
selected trace fields. Payload-free choices and typed review/ordering reasons
use `Debug`; do not invent alternate string spellings for them. `RecordId` and
`ServiceRecordId` implement `Display` by delegating to their contained native ID.
Their displayed text is the canonical underlying identifier, without a kind prefix.
Serialization retains its existing typed representation.

Keep annotation field expressions to constants, direct inputs/field access and
`%`/`?` formatting. Do not embed matches, mappings, fallback selection, method
chains or wrapper construction in attributes. Use values already resolved by the
domain body at meaningful events, or inherit caller context. Do not introduce
telemetry-only helpers or new body computations just to conceal attribute logic.
There is no separate telemetry ID wrapper or entry-time record-kind classifier.

Keep deliberate category selectors when a value contains excluded payloads or
when defining bounded metric labels. For example, service-operation variants
contain QSO/evidence data, and a failed acquisition includes arbitrary diagnostic
text; these values are not logged wholesale through `Debug`. Trace formatting
and metric-label schemas serve different purposes.

Do not enable automatic `ret`/`err` formatting: native values and error messages
can contain arbitrary user/provider text. Errors retain their concrete causes
and propagate normally. The boundary handling an error owns its deliberate,
safe error-level diagnostic; inner libraries do not intercept every error to
classify or log it again.

### Domain events

Events describe decisions and produced plans, rather than generic function
completion. Logbook of The World is abbreviated LoTW.

| Event | Domain facts |
| --- | --- |
| `profile.equipment_selected` | Validated equipment selected while loading a profile; native equipment ID |
| `publication.coverage_decided` | Why current values are covered or pending: coverage decision, service/destination and supporting assertion or attempt/result IDs |
| `publication.support_missing` | Why coverage cannot be established: missing attempt, batch mismatch or unavailable current projection, with evidence IDs |
| `client.resolution_prepared` | Validated explicit resolution produces pending values; account/record identity and typed reason |
| `reference_export.planned` | Returned plan readiness; supplied/unique contact, output-entry, partition, decision and warning counts |
| `projection.prepared` | Evaluated LoTW/QRZ readiness and warning/issue counts, after native validation |
| `projection.conversion_unavailable` | Safe conversion/signing rejection reason where detected, excluding supplied values and error text |
| `projection.text_converted` | Which supported QRZ field needed transliteration, and whether output was omitted; no text values |
| `projection.reference_omitted` | Unsupported service reference mapping omitted, without the arbitrary mapping name/value |
| `tag.deletion.planned` | Affected, shared, proposed-deleted and membership-removed QSO preview counts |
| `acceptance.decided` | Client/server origin and actual returned decision |
| `actor.admission_denied` | Unknown/closed actor during acceptance or recovery; actor ID and known closure boundary |
| `acceptance.rejected` | Missing reference, service namespace/support or binding condition; safe relevant IDs |
| `transfer.batch.planned` | Ready/staged result and supplied, proposed, staged and proposed-install-key counts |
| `compaction.acknowledgement_limits_coverage` | Which authorized client lowered a timeline frontier, with previous and acknowledged revisions |
| `compaction.acknowledgement_excludes_timeline` | Which client lacks coverage for a candidate timeline |
| `compaction.collection_blocked` | Missing surviving baseline or retained actor context prevents collecting actor entries |
| `compaction.projection_blocked` | Which record lacks the baseline or lineage proof needed for projection |
| `compaction.baseline.planned` | Input/projected-record and removable-client counts for a valid baseline proposal |
| `service.operation.incomplete` | Required operation input is unavailable; safe reason and relevant observation, attempt or record IDs |
| `service.operation.review_required` | Explicit selection or admission remedy is required; safe reason, relevant IDs and candidate counts when matching requires selection |
| `observation.field_uncertain` | Conflicting undominated provider facts leave a field unknown; native field path and conflicting-observation count, without provider values |
| `service.operation.planned` | Operation kind and proposed-record/job-update counts for a ready coupled proposal |
| `service.recovery.planned` | Proposed-record and original unit-member counts for retained evidence recovery |
| `service.recovery.incomplete` | Dependency cycle, missing target read, deferred service validation, missing unit member or uncovered member; safe stage and record IDs |
| `association.binding_conflict` | Requested binding conflicts with the existing association/QSO/state |
| `association.target_incompatible` | Requested binding state cannot use the selected target |
| `service.recovery.covered` | Retained service operation already represented, requiring no proposal |
| `service.recovery.review_required` | Returning actor requires explicit authorization/reconciliation |
| `service.retention.calculated` | Required semantic-record and removable-staging counts |
| `client.retry_prepared` | Existing frozen version selected for retry |
| `client.review_required` | Client preparation/receipt/rejection requires review; typed reason |
| `history.regression_detected` | Advertised timeline/revision contradicts the reported remembered revision |
| `history.ordering_unknown` | Available lineage cannot interpret the reported advertised timeline/revision; typed reason |
| `recovery.review_required` | Recovered record needs a choice; typed review reason including ordering detail |
| `evidence.dependencies_missing` | Semantic support is incomplete; missing-dependency count and first missing native record ID, bounding detail volume |
| `download.closure.planned` | Ready closure proposal has acquisition outcome `complete` or `failed` |

`client.resolution_prepared` and `service.recovery.planned` use `info`;
`history.regression_detected` uses `warn`. Other events in this inventory use
`debug`. Diagnostics can explicitly identify relevant native
records/accounts but do not log raw provider keys, provider responses or returned
error strings. A returned plan requiring review remains a normal domain outcome.
Service-operation holds emit their reason at the decision branch, including
holds found while building the coupled changes. Reasons describe missing inputs,
required selection or rejected admission without formatting returned diagnostic
strings. Observation-field uncertainty is reported where differing undominated
values are detected; an established field does not emit that event. These expected
outcomes are distinct from ready proposals.
Routine function exits and propagated errors do not require a separate domain event.
No event here asserts a durable commit, a sent request or a new successful upload.

### Domain metrics

Names, quantities, label sets and recording boundaries belong to their owning
operations. A family measures one logical quantity; bounded labels alone do not
justify grouping unrelated activities. There is no universal execution/item metric,
completion framework or producer clock abstraction.

| Metric | Type/unit | Labels | Recording boundary |
| --- | --- | --- | --- |
| `clacks_service_projection_readiness_total` | Counter / readiness evaluations | `service`, `readiness` | Ready/not-ready projection decision after native validation |
| `clacks_sync_acceptance_plans_total` | Counter / returned decisions | `origin`, `decision` | Actual returned admission decision; validation errors are not decisions |
| `clacks_sync_transfer_batch_plans_total` | Counter / returned batch plans | `result` | Ready/staged batch plan; errors produce no batch plan |


`service` is `lotw` or `qrz`; `origin` is `client` or `server`. Admission decisions
are `proposed`, `covered`, `stale` or `rejected`. Projection readiness is `ready`
or `not_ready`; batch results are `ready` or `staged`. No metric label contains IDs, arbitrary paths, mode
labels or diagnostic text.

Repeated planning counts again. These are domain-result measurements, not request
success/error rates, unique durable saves, new uploads or rollback-event counters.
Validation errors do not generate result metrics. Acceptance can be evaluated
tentatively while constructing a coupled plan; its counts include those evaluations
even if a later step prevents the outer plan from being returned. The owning runtime workflow handles overall error
rates and latency; it must not derive unsampled totals from filtered traces.

Reference-export calculations emit counts in their plan event, not workload
metrics: QRZ projection also calls that planner for a single QSO. The future
user-export workflow owns its actual workload and completion metrics. Compaction
and coupled service planning use their diagnostic events rather than counters
that measure only successful construction.

Metric emission and domain work are independent of span/event filtering.
Libraries have no live queue/resource gauges. Application composition owns
histogram buckets, aggregation, collection/export and async/network propagation.
Library telemetry introduces no automatic desktop diagnostic upload.

## References

- [Tracing annotations](https://docs.rs/tracing/0.1.44/tracing/attr.instrument.html) provide operation scope and explicit input-field selection.
- [Standard formatter span events](https://docs.rs/tracing-subscriber/0.3.23/tracing_subscriber/fmt/format/struct.FmtSpan.html) provide lifecycle presentation without custom producer machinery.
- [Prometheus metric naming](https://prometheus.io/docs/practices/naming/) and [instrumentation guidance](https://prometheus.io/docs/practices/instrumentation/) inform metric quantities, ownership and bounded dimensions.

## Durable client storage

Client-store operations use default module targets and significant operation spans
for saves, profile loads and explicit resolution. Routine preparation, receipt,
window batches, reads and local cleanup use `debug` context. Committed `record.saved`,
`profile.loaded`, `tag.deleted`, `record.resolved`, `schema.upgraded` and `sync.window.completed` events
report actual transaction milestones at `info`, with account/native IDs and relevant
counts or typed reasons. Core planning events still describe proposals.

`clacks_client_qso_mutations_total{change=create|update|delete}` counts changed
local QSO values only after commit. Stale, unchanged and failed transactions do
not increment it. Tag deletion counts its actual committed QSO effects.
`clacks_client_tag_mutations_total{change=create|update|delete}` counts changed
local tag records at their owning save/deletion commit boundaries. Previews,
unchanged/stale saves and rolled-back contact/tag effects add nothing.
`record.saved` identifies the native record kind and create/update/delete change
for QSO, tag, profile and equipment saves. `tag.deleted` includes actual deleted
contact/membership counts and the optional selected-tag fallback cause.
`logbook.view.selected` reports committed local selection at `info`; the desktop's
`logbook.view_fallback_observed` reports an observed committed deletion fallback,
with account/tag context and `deleted_tag` cause, including startup repair.
The presentation event does not count another mutation or replay on every frame.
Selection/save/preview/restore spans use the owning module targets; routine
preview and startup restoration context is `debug`. No selector/rendering metric
or generic operation-completion wrapper is introduced.
`tag.deletion_stale` is a focused `debug` event carrying account/tag IDs and
`tag_changed` or `members_changed`, distinguishing the two unchanged-database
review outcomes without logging record contents or counting a deletion.
`clacks_client_sync_records_installed_total` counts changed incorporated accepted
observations after a committed batch; replaying an identical envelope does not
increment it. These are separate quantities from synchronization planning counts.
These counters have no account, record, path, payload or arbitrary-error labels.

The store disables SQL statement/bound-value logging. No recorded call, name,
comment, provider report, secret reference, certificate/private key or workspace
path is included in its telemetry. Runtime error boundaries decide deliberate
logging and operator diagnostics; the library propagates concrete causes.

## Manual-logging runtime

Contact requests use the `desktop.contacts` span on the existing runtime. The
store remains the owner of `record.saved` and
`clacks_client_qso_mutations_total`; the desktop does not emit a second mutation
counter or repeat a committed event merely to acknowledge the UI action.
`contact.create_retry_acknowledged` reports recovery of an identical stable-ID
create at `info`, with account and record IDs. `contact.save.stale` reports an
expected-basis mismatch without logging contact facts or inventing a new
conflict policy. These events retain correlation when debug spans are filtered.
Bounded list reads use `client_store.contact_query` at `debug`. Handling-boundary
failures and later refresh failures remain distinct from committed writes.

History parsing emits `query.rejected` at `debug`, with a source range and safe
validation reason, excluding the query itself. The desktop request boundary
emits `contact_query.completed` at `debug` with account correlation, bounded
`kind=history|worked_before`, matching count and returned count. Dismissed reads
emit `contact_query.cancelled`; they do not count as failures. Admitted writes
continue after their receiving dialog is dismissed.

`client_store.contact_query.duration_seconds{kind}` measures completed desktop
contact reads from snapshot acquisition through release, including failed reads.
`client_store.contact_query.failures_total{kind}` counts read errors at that same
boundary. Cancelled reads have no completed sample. These answer whether
history or prefix reads are slow/failing without per-query metric dimensions.
Result counts belong to the structured event, not labels. There is no latency
metric per widget or parser invocation and no second mutation counter.

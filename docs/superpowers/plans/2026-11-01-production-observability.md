> Integration correction: the bespoke HTTP implementation described below was
> superseded by the maintained Tower integration. The current decision, source
> comparison and remaining limits are recorded in
> [the integration evaluation](../evaluations/2026-11-02-observability-integrations.md).

# Production Observability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task after approval. Steps use checkbox (`- [ ]`) syntax for tracking. The user approved implementation in this chat; deployment remains unauthorized.

**Goal:** Make actual Clacks requests, synchronization, recovery, maintenance and desktop failures diagnosable with production telemetry, resolving all ten findings from the audit in the accompanying chat.

**Architecture:** Keep the existing `tracing`, `tracing-subscriber`, `tracing-opentelemetry`, OpenTelemetry SDK/exporters, `metrics` bridge, Axum and Tower stack. Use standard HTTP semantic conventions, context propagation, span links, subscriber filters and instrument configuration. Application code supplies domain decisions, safe error classifications and transaction boundaries; it does not implement another telemetry framework.

**Tech stack:** Rust 1.99.1; the workspace's locked dependency versions; OpenTelemetry Protocol (OTLP) HTTP/protobuf; the existing Grafana, Tempo, Loki and metrics backend.

**Spec:** [Production observability](../../observability.md), [desktop specification](../../desktop.md), [synchronization contract](../../synchronization-model.md), and the user's audit/remediation instructions in this chat. The choices below are proposed engineering decisions for review, not previously accepted requirements.

## Global constraints

- Planning only until approved. No implementation changes, new infrastructure, dependency installation, deployment or live failure injection in this task.
- No new tests asserting the shape, names, fields, nesting, ordering, formatting or serialized representation of emitted logs/traces. No golden telemetry snapshots or replacement custom capture framework.
- Useful production evidence is an acceptance gate. Passing tests, delivered signals and matching attribute inventories are insufficient.
- Preserve offline use, synchronization semantics, committed effects, enrollment and credential handling. No telemetry-driven changes to the wire protocol or database schema.
- Libraries do not install global subscribers, recorders or exporters. Executable composition owns them and their shutdown.
- No passwords, tokens, proof headers, cookies, private keys, native payloads, arbitrary provider bodies or arbitrary error chains in telemetry. No raw callback queries. No account/client/record/exchange/grant IDs in metric dimensions.
- Keep bounded SDK queues and existing shutdown ownership. Do not add queues, a context registry, a custom tracing lifecycle, polling workers or a generic operation-completion abstraction.
- Reuse the machine-local development environment according to its external inventory; never reproduce its credentials or provider registration in the repository. Fault exercises use separate disposable application data and infrastructure.
- Keep the existing unrelated `references/desktop-toolkit.md` edits untouched. At implementation pickup, recheck HEAD, worktree placement and changed files before editing.

## Approach and evidence boundary

The audit ended at `eb9c192`. Recheck the affected source at implementation pickup. The audit reviewed workflows and selected tests; it was not an exhaustive test-assertion review or live collector audit.

The preferred approach is to finish the existing standard integrations. Replacing the telemetry stack would add migration cost without resolving domain omissions such as discarded error causes or cleanup counts recorded after the wrong commit boundary. Snapshot-based conformance tests would preserve selected representations without establishing operator usefulness.

Use the maintained OpenTelemetry Tower integration for HTTP server spans,
propagation, status and metrics. Its request duration ends at response headers,
not streamed body delivery or WebSocket completion. Standard Tower callbacks
supply response/failure diagnostics; do not add a custom HTTP lifecycle or body
adapter. The SDK owns normal batching and provider shutdown.

Preserve HTTP paths, ordinary queries and user agents in traces. Keep sensitive
application values out of URLs at construction. Configure specific OAuth query
parameter redaction beside the owning protocol code and configure only those
OAuth-owned names. The released library's encoded-name
limitation is documented; no dependency patch or blanket SDK scrubber is used.
Keep route templates, never raw URLs or query values, as metric dimensions.

## Review focus

- A request that returns 503 must identify the failing application boundary and a useful safe cause, not only report an HTTP status.
- A connected synchronization client waiting indefinitely for a peer must be distinguishable from healthy idle without enabling debug.
- A cleanup failure after an earlier successful commit must preserve the earlier committed measurement.
- A specific dependency-debug filter and a rejecting collector must not expose credentials or collector response bodies.
- Sampling, offline operation and unavailable export must not change business outcomes or remove required security-event records.

These conditions are verified by ordinary functional checks where applicable and the real-stack acceptance exercises below, not output-shape tests.

## Audit finding coverage

| Audit finding | Owning work |
| --- | --- |
| 1. Dependency-debug disclosure | Task 2 |
| 2. HTTP semantics and dimensions | Task 3 |
| 3. Synchronization causality | Task 4 |
| 4. Discarded error causes | Task 5 |
| 5. Histogram resolution/metadata | Task 6 |
| 6. OAuth replay attribution | Task 7 |
| 7. Compaction blockers | Task 7 |
| 8. Silent recovery review/discovery | Task 7 |
| 9. Partial cleanup accounting | Task 8 |
| 10. Geography executable composition | Task 8 |

Tasks 1, 9 and 10 establish the baseline, operational evidence and maintained acceptance obligations for every finding.

## Task 1: Establish a reproducible baseline and the acceptance record

**Files to read:** `docs/observability.md`, `docs/desktop.md`, runtime crate READMEs, `deploy/server/README.md`, current deployment telemetry configuration and existing Grafana queries/dashboards. Evidence belongs in the change review, not product documentation.

- [ ] Record source revision, build identity, application configuration, production log level, sampler, collector routes, export intervals, retention and relevant existing dashboards. Inspect actual settings before assuming defaults. Record secret file references only, never values.
- [ ] Use the existing production binaries and representative synthetic account data in a disposable environment. Run the healthy HTTP, two-client synchronization, contact-query, recovery/retention and geography publication scenarios from Task 9 against the baseline.
- [ ] Record which operator questions cannot be answered, including representative trace links and time ranges. Keep the workload/data sizes so the after comparison exercises the same work.
- [ ] Inventory the existing telemetry tests in affected modules. Classify each assertion by the application requirement it protects; do not count assertion or test quantity as evidence of observability quality.

**Deliverable:** A baseline and an executable acceptance procedure with known inputs and expected business outcomes. No new test harness or permanent load generator is required.

## Task 2: Protect every logging sink independently of verbosity settings

**Modify:** `lib/clacks-server/src/telemetry.rs`; inspect `lib/clacks-desktop/src/telemetry.rs` for consistent policy. Keep the implementations application-owned.

- [ ] Use `tracing_subscriber::filter::filter_fn` as an independent maximum-level/privacy filter for dependency diagnostics, following the existing desktop composition. Apply it to all sinks, independently of `EnvFilter` directive specificity.
- [ ] Cap the existing sensitive dependency targets, including `opentelemetry*`, `reqwest*`, `hyper*`, `h2*`, `sqlx*`; retain the intended session-library suppression. Keep arbitrary collector `error_message` suppression and exporter log-bridge exclusion. Do not parse/redact arbitrary body text after it has entered an event.
- [ ] Preserve safe exporter failure categories/status and local sink warnings. Verify safe warnings remain visible; disabling every exporter diagnostic is not completion.
- [ ] Exercise the real binary with broad debug and with the specific `opentelemetry-otlp=debug` directive against a local collector returning a recognizable fake secret in a rejection response. Inspect stderr and the connected log backend for disclosure, while confirming an actionable safe failure remains visible.

**Acceptance:** The same safe diagnostic remains available under both filters, fake credentials/body text are absent from all sinks, and application work still succeeds while export is unavailable. This is a controlled acceptance exercise, not a serialized-log test.

## Task 3: Configure standard HTTP telemetry

**Modify:** `lib/clacks-server/src/http.rs`, OAuth protocol owners and
`lib/clacks-server/src/telemetry.rs`.

- [ ] Use maintained OpenTelemetry Tower middleware with its Axum matched-route
  extraction, global providers and standard HTTP histogram. Supply the trusted
  configured server address/port through its attribute extractor.
- [ ] Keep the request-to-response-headers boundary. Use standard Tower callbacks
  for response/failure logs; do not create a custom HTTP span/body lifecycle or
  claim full response delivery or complete local access logging.
- [ ] Compose named OAuth redaction from callback/enrollment owners. Preserve
  paths, benign query values and user agents. Do not install a provider-wide span scrubber or rewrite requests.
- [ ] Preserve public protocol metadata. Document the released helper's raw-name
  matching limitation instead of claiming percent-encoded spellings are covered.
- [ ] Keep metric dimensions bounded to route templates and standard protocol
  outcomes, excluding raw URLs, query values and user agents.
- [ ] Verify normal requests, authorization failures and readiness failures in
  the real telemetry stack. Verify useful values survive and designated OAuth
  values are redacted, without new log/trace output-shape tests.

**Acceptance:** Operators can identify method, route, response status and duration;
HTTP traces retain useful URL/user-agent information while named OAuth values are
redacted. Actual OAuth workflows still complete. Library limitations remain
explicit review/qualification work, and nothing is deployed.

## Task 4: Restore synchronization context and expose actual workflow progress

**Modify:** `lib/clacks-desktop/src/telemetry.rs`, `synchronization.rs`, `synchronization/exchange.rs`, `synchronization/pending.rs`; `lib/clacks-server/src/synchronization.rs`, `synchronization/connection.rs` and its dispatch children.

- [ ] Install the standard trace-context propagator in desktop composition when trace export is enabled. Create a bounded HTTP client span for the upgrade and inject its context with `opentelemetry_http::HeaderInjector`; use client-kind/status conventions, without recording authorization/proof headers.
- [ ] Capture standard OpenTelemetry context before entering the server's spawned upgrade callback. Carry immutable context into the socket task with explicit arguments and use `tracing::Instrument` on the newly created operation futures. Do not recover/mutate function spans via `Span::current()`.
- [ ] Use standard links for work that outlives the HTTP handshake. Do not keep the HTTP span open for an hours-long WebSocket or put every socket event into one unbounded trace. Create new-root bounded negotiation, inbound-window, publication and acknowledgement operation spans linked to the shared handshake context, rather than implicitly inheriting one long-lived connection parent. Use existing exchange IDs as diagnostic attributes where already available.
- [ ] Be precise about the boundary: header propagation correlates the upgrade across client/server; linked traces and existing exchange IDs correlate later work. It does not establish per-message remote parentage. Do not invent a trace-context wire envelope or claim one exists.
- [ ] Make significant connection, wait-for-required-peer-work, progress and recovery transitions observable at `info`; keep ordinary idle receives, per-frame work and per-record details at debug. Record actual phase, bounded work counts and outcome at the existing state transitions. A wait for pending work must be distinguishable from an idle connection.
- [ ] Record application-owned negotiation and transfer-window duration histograms with bounded outcome labels, independent of trace sampling. Define one sample per completed/failed/cancelled bounded operation; do not fabricate completed latency for a still-running wait. Retain the existing reconnect counter with a documented counting boundary.
- [ ] Observe synchronization task join errors at session ownership boundaries rather than discarding them; report safe task failure and correct the presented connection status. Do not add a fabricated panic path or test-only failpoint.

**Acceptance:** With two clients, an operator can locate an edit's publication and receipt, distinguish negotiation/persistence/network wait, navigate the handshake links, and identify a stalled required phase without debug. Ordinary idle creates neither false failure reports nor a growing trace.

## Task 5: Preserve actionable safe causes at error boundaries

**Modify:** server `auth/error.rs`, `synchronization/connection.rs`, `synchronization/connection/failure.rs`; desktop `session.rs`, `session/settings.rs`, `enrollment/worker.rs`, `synchronization.rs` and its transport boundary.

- [ ] Keep peer-safe response text separate from operator diagnostics. Preserve concrete errors until the owning boundary; use existing typed errors/downcasts to select safe categories rather than parsing error strings.
- [ ] For storage, retain distinctions such as pool timeout, transport, database response with SQLSTATE, decode, metadata and invariant failure. For desktop connection/enrollment, retain credential access, discovery, transport, upgrade status, authorization refusal and invalid protocol categories. Preserve distinctions only where the selected dependency actually supplies them; do not guess TLS versus DNS after an earlier conversion has erased that cause.
- [ ] Include the action/phase and available account/actor/record context on the event that handles the failure. Put `error.type` on the owning operation span and outcome on its measurement. Keep diagnostic fields bounded/safe; identifiers remain out of metrics.
- [ ] Reuse local typed match functions only where branches need a safe representation. Do not introduce a universal Clacks error taxonomy, automatic error-chain serializer or another error-logging layer in every library.
- [ ] Preserve cancellation, expected stale/review/refusal and committed-write-followed-by-refresh-failure as distinct outcomes. Avoid duplicate error records at every propagation step.

**Acceptance:** Independently exercise unavailable storage, malformed protocol input, missing protected credentials, refused upgrade and a failed post-write refresh in disposable environments. An operator can tell which action failed and which cause class applies without the transient UI message or secret-bearing debug logs. Existing functional checks still establish correct retry, cancellation and persistence behavior.

## Task 6: Configure usable metrics with explicit ownership and units

**Modify:** server `telemetry.rs`, desktop `telemetry/export.rs`, and the owning metric declaration sites, including `application.rs`, `session/settings.rs`, server/store lock and retention modules, client-store retention and geography lookup.

- [ ] Describe existing metric families with `metrics::describe_*`, explicit units and recording boundaries before first registration. Counts are counts; durations use seconds. Respect the metrics bridge's registration timing so late descriptions do not masquerade as applied configuration.
- [ ] Use `OpenTelemetryRecorder::set_histogram_bounds_mut` for facade instruments, and SDK instrument advisory boundaries/views where native SDK instruments are used. Do not create another histogram implementation or aggregation worker.
- [ ] Standard HTTP boundaries: `[0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1, 2.5, 5, 7.5, 10]` seconds.
- [ ] Lock-wait/hold and contact-query initial boundaries: `[0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10]` seconds. Add `30, 60, 300` seconds for maintenance/retention and network-operation duration families; keep desktop startup's already deliberate buckets. These are measurement resolution choices, not accepted performance thresholds.
- [ ] Give workload-size histograms count-appropriate boundaries, starting `[0, 1, 10, 100, 1000, 10000, 100000]`; compare baseline sizes before finalizing upper resolution. Inventory every emitted histogram and assign it an owning runtime configuration rather than leaving new families to accidental SDK defaults.
- [ ] Reconcile backend metric-name/unit translation with actual exported names before writing dashboard queries. Query histogram count/sum/buckets, not trace durations, for unsampled rates/distributions. Confirm bounded series as accounts and records vary.

**Acceptance:** The same representative workload shows millisecond/subsecond distributions, and a controlled slower environment shifts the relevant distribution without changing unrelated routes/operations. Compare volume, counts and bucket resolution, not merely the existence of a named metric. Set no latency service objective from these initial bucket choices.

## Task 7: Restore security, recovery and compaction decision context

**Modify:** server `oauth/events.rs`, `oauth/authorization.rs`, `oauth/revocation.rs`; sync `compaction/installation.rs`, `client/returned.rs`; client-store `synchronization.rs`, `discovery.rs`.

- [ ] Retain the safe grant-family identity already supplied for code replay and containment failure. Record the same family identity at successful grant activation so a replay can be traced back to its account/actor without logging a token or authorization code. Do not fabricate an identity when the library reports none.
- [ ] Keep security events at their deliberate levels and independent of trace sampling. Review revocation events against the information actually supplied by the library; use existing owning persistence context where account/actor attribution is available. No extra database lookup solely to decorate a log.
- [ ] At compaction branches, restore the contract's acknowledgement-limit/excluded-timeline and projection-blocked diagnostics, with actor/record/timeline identity and typed reason. At normal level, summarize the account's limiting client/timeline, blocked-reason counts and a representative blocked record where applicable; reserve enumeration of further record details for debug. Emit no per-record success events. Bound summary size within the existing calculation; add no cross-sweep cache, deduplication registry or polling process. Update the contract's level policy to match this deliberate production visibility.
- [ ] Add an operation span and a post-commit recovery-review decision event at the existing `receive_review` boundary, carrying account/record and the available typed review reason. Do not report a plan or a rolled-back attempt as a committed review requirement.
- [ ] Instrument source-side synchronization discovery/page assembly as bounded operations with examined/returned/support counts and useful outcome. Preserve existing limits and complete-unit semantics; no per-row spans or timers duplicating library function lifecycle.

**Acceptance:** In a disposable account, identify the grant affected by a real code replay; explain which client limits compaction; distinguish missing interpretation from no eligible work; and locate why a record entered recovery review. Repeat/no-op runs must not invent another committed mutation.

## Task 8: Correct cleanup accounting and complete CLI composition

**Modify:** server `oauth/cleanup.rs`; geography-builder `application.rs`, new application-owned `telemetry.rs`, `Cargo.toml`, README; root workspace manifest only for already selected dependencies that must be inherited.

- [ ] Move OAuth cleanup removal increments to the successful per-account commit and each successful independently committed cleanup statement. Keep the sweep's returned aggregate for command logic if needed, but remove the second whole-sweep increment. Use the existing counter; no progress database/table or recovery ledger.
- [ ] Give the geography executable explicit `--otlp` opt-in, standard OTLP environment configuration and service identity `clacks-geography-build`. Initialize the standard metrics recorder and providers before `execute`; flush them after success, failure or supported cancellation. Preserve stdout results and structured stderr when export is disabled/unavailable.
- [ ] Compose the already selected standard providers locally. Do not depend on the server crate merely for telemetry initialization and do not extract a universal telemetry framework as part of this work. Apply the same dependency/privacy policy and useful histogram settings.
- [ ] Keep build publication/unchanged/cancelled/failed measurements at their current real publication boundary. SDK metrics collection cannot be assumed to run before a short-lived process exits; use its ordinary force-flush/shutdown facilities.

**Acceptance:** A multi-account cleanup interrupted by an ordinary infrastructure failure preserves measurements for already committed removals; a retry counts only remaining work. Inspect business state and metrics together. Exercise real geography publication, identical repeat, ordinary input failure and cancellation with export enabled; the short-lived binary's measurements reach the backend before exit. Do not introduce database triggers, drop tables or production failpoints to manufacture these cases.

## Task 9: Accept operator usefulness in the actual telemetry stack

Use release-equivalent binaries, ordinary `info` filtering, real production migrations in disposable databases, the same collector pipeline and Grafana data sources as deployment, and synthetic application data. Desktop export remains opt-in. Run deliberate failures and code replay only against disposable identities/infrastructure. Deployment and production smoke checks occur only after review and authorization.

| Exercise | Operator must answer from retained telemetry |
| --- | --- |
| Successful sign-in/page load, expected refusal, unknown route, server unavailable response | Which route/method/status, which application boundary failed, and where time was spent? |
| Two independent desktop edits followed by receipt, unchanged retry and cancellation | Which account/actor/exchange did the work; what actually committed; where did the client and server wait? |
| Peer stops responding during required synchronization work; compare healthy idle | Is the application idle or blocked in a specific required phase, and since when? |
| Missing protected credential, refused upgrade and unavailable storage | Which safe cause class applies without reproducing the original UI message? |
| Fast and deliberately slower contact reads/lock contention | Can route/workflow distributions distinguish milliseconds, hundreds of milliseconds and seconds? |
| Authorization-code replay on a disposable grant | Which grant/account/actor is affected and did containment persist? |
| Lagging client, missing interpretation, then restored evidence and recovery review | Why is retention blocked, which record needs review, and what changed after recovery? |
| Cleanup partial infrastructure failure followed by retry | Do totals agree with actual committed work, including commits before failure? |
| Geography publish/repeat/failure/cancel | Does the production CLI deliver truthful measurements before exit? |
| Rejecting/unavailable collector, aggressive debug settings, trace sampling disabled | Are safe failures visible, fake secrets absent, security events retained and business behavior unchanged? |

- [ ] For each exercise record the deployed revision/configuration, reproducible action, expected business result, metric comparison, time range and Grafana trace/log/dashboard links. After ordinary operations, allow two configured collection/export intervals plus the longest configured exporter timeout before assessing missing samples; short-lived commands must finish SDK shutdown instead. Record what remains unanswerable instead of declaring a pass from delivered signals.
- [ ] Inspect parent/link navigation, local stderr/file diagnostics, and exported logs. Verify actual backend trace-to-log navigation; local JSON span fields are not proof of exported correlation. Use standard backend field mapping rather than another producer serializer if the gap is ingestion configuration.
- [ ] Compare against the Task 1 baseline with the same workload, repeat/no-op cases and a defined export-wait window. Record export losses, latency overhead, CPU/memory and series growth. Establish a measured baseline before selecting sampling rates or overhead budgets; document unresolved performance limits explicitly.
- [ ] Update existing Grafana dashboards/queries to answer the listed questions using route/phase/outcome and appropriate histogram queries. Keep account/grant/exchange identifiers in trace/log searches, never dashboard metric labels. Locate the dashboard configuration owner before editing it.
- [ ] Review sampler/queue/collector loss behavior. Security records must remain logs independent of trace sampling; a trace is not their audit store. Distinguish unsampled operations from missing export.
- [ ] After approved deployment, perform a small healthy-production smoke check and inspect its actual Grafana evidence. Repeat invasive failures only in disposable infrastructure. Verify telemetry after rolling back as well; no database migration is required by this plan.

**Acceptance rule:** Every row needs an answer supported by real application/backend evidence. A green functional suite or an attribute checklist cannot waive an unanswered row. If a scenario cannot be exercised safely/reliably with ordinary infrastructure, report the specific unverified boundary for review; do not add hidden failpoints or relabel it passed.

## Task 10: Replace representation enforcement with maintained acceptance obligations

**Modify:** `docs/observability.md`, relevant crate READMEs, `deploy/server/README.md`, and existing telemetry test files only where their assertions are affected.

- [ ] Inspect each existing affected telemetry test before retaining it. Remove golden/output-shape assertions and tests whose only claim is that the current event/span representation exists. Do not replace them with another event collector or assert an exact event count/tree.
- [ ] Preserve useful functional scenarios and assertions: saved data, unchanged retry, correct recovery/refusal, cancellation, committed progress and offline behavior. Reuse existing suites rather than duplicate operations just to observe telemetry.
- [ ] Add an ordinary functional regression test only for an actual business-behavior defect found during implementation, using existing public operations and production schema. No new log/trace shape tests are part of this plan. Metrics truthfulness and privacy are checked in the controlled acceptance exercises, not by asserting framework behavior.
- [ ] Update the maintained contract with current HTTP conventions, measurement boundaries, error ownership, context/link strategy, privacy rules and the operator-question acceptance gate. Reconcile obsolete event-inventory entries rather than preserving absent workflows as supposedly implemented behavior. Keep dated evidence and audit findings in the review, not the product contract.
- [ ] Run repository checks appropriate to the changed Rust code: `cargo +1.99.1 fmt --all --check`, affected existing tests, then workspace tests and clippy with the documented disposable database configuration. Run SQL metadata checks only if query changes actually require them. Report native-platform acceptance limits truthfully.

## Delivery order and completion

Baseline first; then logging protection; HTTP and instrument configuration; synchronization and error boundaries; security/recovery/compaction; cleanup and CLI composition; final real-stack acceptance. Update the owning documentation and reassess affected tests within each change, with Task 10 providing the final consistency review.

Deliver focused reviewable commits/PRs. Keep production deployment separate from code approval. No finding is closed solely because its proposed fields were added. Close it when the corresponding operator question can be answered with evidence under ordinary production settings, with business behavior preserved and remaining limits explicit.

## Primary references

- [OpenTelemetry HTTP span conventions](https://opentelemetry.io/docs/specs/semconv/http/http-spans/): names, kinds, status, attributes and sensitive URL handling.
- [OpenTelemetry HTTP metrics](https://opentelemetry.io/docs/specs/semconv/http/http-metrics/): standard duration instruments, units and recommended boundaries.
- [Tower HTTP tracing](https://docs.rs/tower-http/0.7.1/tower_http/trace/index.html): request/response/failure/body lifecycle callbacks and supplied spans.
- [Tracing/OpenTelemetry bridge](https://docs.rs/tracing-opentelemetry/0.34.0/tracing_opentelemetry/): standard special fields and semantic attribute integration.
- [OpenTelemetry span extensions](https://docs.rs/tracing-opentelemetry/0.34.0/tracing_opentelemetry/trait.OpenTelemetrySpanExt.html): explicit context, links, attributes and status.
- [OpenTelemetry Tower implementation](https://github.com/open-telemetry/opentelemetry-rust-contrib/blob/main/opentelemetry-instrumentation-tower/src/http/server.rs): inspected response-future measurement boundary and automatic path/query capture; not selected as an additional dependency.
- The locked `metrics-opentelemetry` and OpenTelemetry SDK implementations were inspected for instrument-registration behavior and default histogram boundaries. Their defaults do not infer seconds from metric names.

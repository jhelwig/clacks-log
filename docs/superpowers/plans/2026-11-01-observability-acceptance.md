# Observability implementation acceptance evidence

Review branch: `fix/production-observability`. Original baseline: `eb9c192`;
main was integrated through `bab0e72`.
No deployed application or observability configuration was changed.

## Real stack exercised

Synthetic local application instances exported through the existing
Alloy receiver into Tempo, Loki and Prometheus. The isolated database was
`clacks_observability_review`; persistent development accounts were not used.
Evidence was inspected in backend APIs and the rendered Grafana trace view.
This is runtime qualification, not an automated log/trace shape test.

- Baseline instance `baseline`: trace `5c9b127b7004223837ce1c31d56300da`
  exhibited the original internal `http.request` span without HTTP status.
- Changed instance `review-after`: exact 200, 401 and 404 responses appeared in
  standard HTTP duration metrics with bounded route templates. Parent context
  `11111111111111111111111111111111` survived propagation and Grafana displayed
  `GET /health/ready`, status 200 and the route. Unknown private paths were
  excluded. Durations end when response headers are produced.
- Closing only the functional scenario's runtime pools reproduced a health
  middleware bug (liveness 500). After moving health routes outside session
  middleware, that scenario passed: liveness 200, readiness 503 with the safe
  `storage_pool_closed` cause.
- `review-two-clients-final`: the existing native credential/session scenario
  passed using two clients and actual issuer/WebSocket/storage paths. Trace
  `5b7b964e6ff058d031561b49f9cc6c47` contains a bounded inbound operation with
  an immutable handshake link. Loki wait diagnostics carry that trace ID and
  span ID, plus synthetic account/actor/exchange context. This exercise caught
  and corrected explicit-parent log events lacking active SDK context.
- `review-retention`: the existing retention scenario passed with real export;
  baseline installation had useful subsecond histogram observations.
- `review-cli` and `review-cli-cancel`: the real geography executable exported
  published, unchanged, failed and cancelled outcomes before exit. Cancellation
  used ordinary publication-lock contention and preserved the current pointer.
- `review-unsampled`: with the standard sampler set to `always_off`, Loki still
  received the authorization-code replay warning, grant family and containment
  result. Security diagnostics do not depend on sampled traces.
- A rejecting collector and an invalid endpoint containing fake secret markers
  exercised exporter diagnostics and constructor failures. Specific library
  debug directives did not bypass privacy caps; raw fake endpoint/header/body
  markers did not appear after the correction.

The existing functional suites, workspace clippy, formatting and documentation
checks are recorded in the PR. No new log/trace output-shape tests were added;
selected existing presentation assertions were removed while behavioral and
privacy checks remain.

## Initial middleware replacement qualification (superseded URL policy)

This block records the initial replacement, before targeted query redaction.
The blanket URL/user-agent removal exercised here was subsequently removed. The
HTTP implementation uses `opentelemetry-instrumentation-tower` 0.19.0; the application
HTTP span lifecycle/body adapter was removed. Desktop, server and geography CLI
target caps/exclusions now use standard `tracing-subscriber::filter::Targets`.

- `review-upstream-http`: actual 200 readiness/sign-in, 404 unknown route and 401
  native upgrade refusal appeared in the standard HTTP histogram with exact
  codes and bounded matched routes. Protocol and configured address/port labels
  are present. No route label was invented for the unmatched request.
- Trace `33333333333333333333333333333333` preserves the supplied W3C parent,
  server kind, `GET /health/ready` name and response status 200. The synthetic
  private query/user-agent attributes were removed before export. Unmatched trace
  `aa037d394a30b6873054a9905b4d0cce` also excludes the private path canary.
- Loki's standard Tower response logs carry status, latency and SDK trace/span
  context; this is not a complete independent local HTTP access log.
- `review-upstream-issuer`: the existing issuer/health functional scenario passed.
  Readiness trace `5f0dded4d329a8513e5c468587f7af45` has status 503 and ERROR.
  Prometheus retains the 503 duration observations after process shutdown.
- `review-upstream-native`: the existing native two-client workflow passed in
  82 seconds with live export. Inbound trace
  `bf6aa278b84bff60218fb58835db11ab` has a handshake link; a Loki peer-wait event
  carries the same trace ID and its operation span ID.

The former SDK attribute-removal processor is not retained. Current behavior uses
only the released middleware's named-parameter query redaction and the SDK's
standard batch processor. The
[source-level integration evaluation](../evaluations/2026-11-02-observability-integrations.md)
records the policy, retained handshake/provider wiring and upstream limitations.

## Targeted query-redaction qualification (current URL policy)

The provider-wide `PrivateSpans` processor was deleted. The project uses the
unmodified published Tower 0.19.0 crate and its named-parameter configuration;
there is no percent-encoded-name dependency patch or replacement decoder. Only
the OAuth-owned parameter names are configured; unrelated signed-URL defaults
are not copied into the application.

The `review-targeted-query` instance exported to the real Alloy/Tempo/Loki/
Prometheus stack using the disposable review database:

- Trace `44444444444444444444444444444444` retains `/health/ready`, the exact
  `page=2&sort=asc&note=a%2Bb` query and `ClacksReview/URL-preservation` user agent.
- Unmatched trace `45454545454545454545454545454545` retains the ordinary
  `/review/ordinary-path?view=contacts&limit=25` URL. Its HTTP metric has no
  invented route or raw-path dimension.
- Callback trace `46464646464646464646464646464646` redacts `code` and `state`,
  preserving the callback path, user agent, `client_id`, `scope` and
  `code_challenge` values. The callback remains a normal 303 refusal when the
  synthetic provider is unavailable, rather than a telemetry-induced failure.
- Trace `47474747474747474747474747474747` redacts both repeated `code` values,
  `state`, `error` and `error_description`. Its malformed duplicate-field request
  is still rejected with 400 by the ordinary application extractor.
- Trace `48484848484848484848484848484848` redacts the free-form `client_label`
  and state while retaining public authorization metadata.
- Real histogram series contain method/status/route/protocol/configured-origin
  dimensions; no path, query or user-agent dimensions were added. The inspected
  Loki stream contained no synthetic secret-canary values.

Existing workspace tests passed (549 passed, 6 ignored), as did workspace
all-targets clippy with warnings denied, formatting and diff checks. Existing
issuer and native two-client scenarios also passed with production export under
`review-targeted-issuer` and `review-targeted-native` (2.5 and 62 seconds).
No log/trace output-shape tests were added. The middleware still matches raw
parameter names; encoded-name redaction is not claimed. Nothing was deployed.

## Remaining qualification limits

This does not establish production readiness or authorize deployment. Still
unqualified: partial OAuth cleanup failure after a preceding commit, sustained
stalled-peer/failure-phase exercises, production-load overhead and native Windows
execution. The cleanup counter boundary was inspected in code, but no synthetic
schema failpoint was introduced to manufacture that scenario. The native runtime
exercise does not constitute rendered desktop UI acceptance.

The Operations companion PR updates the two managed HTTP dashboard queries and
Loki's trace derived-field matcher. Neither has been applied. Rollout must
coordinate the renamed metric queries with the application release.

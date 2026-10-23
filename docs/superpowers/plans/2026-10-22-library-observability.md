# Native-library observability implementation plan

**Goal:** Add domain-specific production instrumentation to the native model and
synchronization libraries while preserving ordinary control flow and using the
observability framework for lifecycle, context and timing.

**Contract:** [Production observability](../../observability.md), alongside the
[native model](../../native-model.md), [synchronization model](../../synchronization-model.md)
and [service evidence](../../service-evidence.md). Public APIs, concrete errors,
serialized values and synchronization semantics remain unchanged.

## Constraints and dependencies

- Rust 1.99.1; libraries in `lib/`, without a telemetry framework or application.
- Workspace-inherited `tracing` 0.1 with `std`/`attributes` and `metrics` 0.24.
  Test-only `tracing-subscriber` 0.3 and `metrics-util` 0.20.
- No producer clock dependency, manual duration capture, synthetic close/completion
  records, result wrappers or generic item/operation metrics.
- No runtime installation, exporter, background work or desktop diagnostic upload.
- No automatic payload/error formatting, secrets, unchecked indexing, production
  panics, test-only public APIs or facade re-exports.
- Preserve default module-path targets, existing function bodies, early returns
  and `?` propagation. Add reporting beside the decisions and plans it describes.
- Report calculations/proposals; persistence/provider adapters own durable-effects,
  external-success and overall workflow measurements.

## Task 1: Add scoped test observation

Files: both `tests/support/telemetry.rs`, `tests/support/mod.rs`, and workspace/member manifests.

- [x] Capture structured span/event metadata, target, fields and parentage through
  a scoped tracing layer; capture metric names/labels/values through a local recorder.
- [x] Keep collectors and fixtures in tests; avoid process-global state.
- [x] Observe Clacks reporting choices rather than testing formatter/lifecycle
  machinery or requiring every span to produce a completion event.

## Task 2: Use framework lifecycle and context

Files: existing model and synchronization operation modules; native `RecordId` and `ServiceRecordId` definitions.

- [x] Add `#[tracing::instrument(skip_all, ...)]` with stable operation names,
  explicit safe input fields at meaningful boundaries. Select levels by operational
  significance: explicit resolution and recovery milestones at info, witnessed
  history regression at warn, routine diagnostic detail at debug, and trace for
  definition compilation and evidence-dependency investigation.
- [x] Identify operations by span name and components by default module target,
  without redundant `operation`/`component`/`phase` fields; preserve enclosing caller/account context.
- [x] Leave scalar getters, inner loops and trivial arithmetic uninstrumented.
- [x] Use existing `Debug` for payload-free choices and typed review/ordering
  reasons. Use existing `Display` for native IDs. Retain category selection only
  for meaningful metric dimensions or excluded payloads in domain reporting.
- [x] Implement `Display` on native ID types and delegate to contained IDs. Keep
  annotations to constants/direct inputs and field access with `%`/`?`. Reuse
  existing body locals for derived equipment, destination and resolution context;
  do not hide attribute computations in telemetry helpers.
- [x] Place test-support module declarations before imports and functions.
- [x] Keep normal bodies and error propagation. Use no immediately invoked result
  closures, explicit timers, catch-all error classifiers or completion framework.
- [x] Leave span lifecycle presentation and duration collection to standard
  subscribers/exporters supplied by application composition. Do not add an
  artificial runtime integration to the libraries.

## Task 3: Report domain decisions and quantities locally

Files: model export/projection/tag modules; sync admission, transfer, compaction,
service operation/recovery/retention and diagnostic decision branches.

- [x] Use the exact [domain-event inventory](../../observability.md#domain-events)
  and [metric inventory](../../observability.md#domain-metrics).
- [x] Report supplied/unique contact and output counts in the export-plan event.
  Keep export workload metrics at the future user-export workflow; service
  projection also calls the shared planner.
- [x] Measure returned admission decisions at their branches/final planning call,
  without interpreting a propagated error as an admission decision.
- [x] Report ready/staged batches, valid baseline proposals, ready coupled native/
  evidence proposals and retained-evidence recovery work beside their construction.
- [x] Report publication coverage and supporting evidence, safe conversion causes,
  limiting acknowledgements, collection/projection blockers, binding conflicts,
  recovery holds and missing support identities beside their decisions. Retain
  tag previews, retry/review reasons, download closure and retention quantities.
- [x] Report service-operation missing-input/review branches with safe reasons
  and relevant IDs/counts, and conflicting observation fields without provider values.
- [x] Keep only projection-readiness, acceptance-decision and transfer-batch
  counters, with coherent bounded dimensions. Do not infer durable effects or
  workflow error rates from repeated planner evaluations.
- [x] Preserve returned error causes and arbitrary diagnostic text for the caller;
  handling boundaries own deliberate error-level reporting.

## Task 4: Verify Clacks reporting behavior

Files: both `tests/observability.rs` and relevant existing integration tests.

- [x] Check supplied/unique/expanded export quantities, review versus failure,
  stale/replayed admission, staged batches and retained evidence versus new uploads.
- [x] Assert no returned-plan event is fabricated when export fails.
- [x] Check default module targets, nested/concurrent caller context and deliberate
  exclusion of arbitrary payload/provider text through real domain operations.
- [x] Filter an export span without changing the caller's fields or suppressing
  domain work; its domain summary can inherit a visible caller or be root.
- [x] Vary account/record IDs and check actual metric dimensions remain bounded.
- [x] With debug/trace filtered out, check info-level resolution/recovery spans
  and events retain their context, and witnessed history regression remains a
  self-contained warning. Verify exact levels without changing domain results.
- [x] Exercise missing reviewed evidence, required candidate selection and
  conflicting observations; verify meaningful hold reasons, no provider
  text disclosure and no uncertainty event after a dominating refresh.
- [x] Keep test helpers focused on named domain events/metrics. Do not assert a
  generic completion schema or duplicate framework lifecycle tests.

## Task 5: Document and validate

Files: `docs/observability.md`, library READMEs, `AGENTS.md`, overview and cross-cutting roadmap requirement.

- [x] Document framework-owned lifecycle/timing and the exact domain reporting
  boundaries without prescribing structural changes to support instrumentation.
- [x] Explain that span timing does not automatically populate `metrics` histograms;
  runtime-owned workflow/error/latency instrumentation needs deliberate integration.
- [x] Run native tests/doctests, formatting, Clippy, Windows all-target compilation,
  private-item rustdoc, pinned Logbook of The World (LoTW) data and documentation checks.
- [x] Attempt browser compilation and report the existing `getrandom` `wasm_js`
  configuration limitation without changing identity/randomness policy.

```sh
python3 scripts/extract-lotw-data.py --check
cargo +1.99.1 fmt --all --check
cargo +1.99.1 test --workspace --all-targets --locked --offline
cargo +1.99.1 test --workspace --doc --locked --offline
cargo +1.99.1 clippy --workspace --all-targets --locked --offline -- -D warnings
cargo +1.99.1 check --workspace --all-targets --target x86_64-pc-windows-msvc --locked --offline
RUSTDOCFLAGS='-D warnings' cargo +1.99.1 doc --workspace --no-deps --document-private-items --locked --offline
cargo +1.99.1 check -p clacks-model --target wasm32-unknown-unknown --locked --offline
git diff --check
```

The [tracing annotation](https://docs.rs/tracing/0.1.44/tracing/attr.instrument.html)
provides operation scope. Standard formatter lifecycle facilities include
[`FmtSpan`](https://docs.rs/tracing-subscriber/0.3.23/tracing_subscriber/fmt/format/struct.FmtSpan.html).
Metric families follow [Prometheus naming](https://prometheus.io/docs/practices/naming/)
and [instrumentation](https://prometheus.io/docs/practices/instrumentation/) guidance:
one logical quantity per family, with coherent bounded dimensions.

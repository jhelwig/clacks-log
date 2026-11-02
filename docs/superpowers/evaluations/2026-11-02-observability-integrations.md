# Observability integration decisions

Scope: replacement of the observability PR's custom server HTTP instrumentation
and target matching, plus evaluation of client handshake and provider lifecycle
wiring. This is not an evaluation of every custom component in Clacks.

## HTTP server: use the maintained Tower integration

Selected `opentelemetry-instrumentation-tower` 0.19.0 from OpenTelemetry Rust
Contrib. Its `axum` feature supplies matched-route extraction; its dependencies
accept our selected OpenTelemetry 0.33 and Axum 0.8 versions. The library owns
request spans, propagation, response status, HTTP metrics and response-boundary
measurement. Global providers are installed before building the router. Its
instrument catalog and histogram boundaries replace the application's duplicate
HTTP duration registration. Standard Tower response/failure callbacks provide
operational logs without the default URI-bearing tracing span.

The application's request attribute extractor supplies only the configured public
server address and port, not values from arbitrary forwarding headers. Routing
construction propagates the library builder's error rather than panicking.

Paths, ordinary query values and user agents are useful HTTP trace attributes and
are retained. Application URL construction must keep sensitive/personal data out
of ordinary paths and queries. An exception in a particular protocol does not
justify a provider-wide attribute deletion policy. There is no SDK span scrubber.

Use the released middleware's named-parameter query redaction. The callback type
owns `code`, `state`, `error` and `error_description`; enrollment approval owns the
free-form `client_label` extension. The router configures only those OAuth-owned
names. This replaces the middleware's default list; Clacks has no demonstrated
signed-URL requirement. Values are replaced with `REDACTED`; names, parameter order, other raw query text and the actual request
passed to handlers are preserved. Public OAuth metadata such as `client_id`,
`scope`, `response_type` and `code_challenge` remains observable. Path/query/user
agent values do not become metric labels; metrics use matched route templates.

The released helper matches raw parameter names case-sensitively. A percent-encoded
name such as `co%64e` is not covered by a `code` rule. This limitation is explicit;
the repository uses the unmodified published crate, with no dependency patch or
replacement parser. Encoded-name redaction is not claimed as verified behavior.

Source-level limitations to qualify and carry upstream: this release derives
`url.scheme` from the request URI (origin-form requests can leave it empty), and
uses unnormalized extension method names. It marks 5xx spans as errors but does
not supply an `error.type` attribute for those HTTP statuses. Do not claim
complete HTTP semantic-convention coverage, `_OTHER` normalization or
that the configured public scheme is supplied by the library. Do not work around
these limitations with another application-owned HTTP instrumentation layer.

Sources: [server implementation](https://docs.rs/crate/opentelemetry-instrumentation-tower/0.19.0/source/src/http/server.rs),
[attribute helpers](https://docs.rs/crate/opentelemetry-instrumentation-tower/0.19.0/source/src/common/attributes.rs).

## Target filtering: use the dependency already selected

`tracing-subscriber::filter::Targets` supplies target prefix matching and level
limits. The server, geography CLI and desktop use it for independent dependency
caps and exporter bridge exclusions. The environment filter remains a separate
constraint. A field-based predicate suppresses `error_message`, which may contain
arbitrary backend text; it does not implement target matching. No custom filter
type, target matcher or shared telemetry framework is necessary.

Source: [Targets](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/targets/struct.Targets.html).

## Native WebSocket handshake: retain a narrow adapter

The actual request goes through `tokio_tungstenite::connect_async`, not Reqwest.
It must preserve Huskarl's DPoP authorization headers and response processing,
including nonce challenges, and leave an immutable handshake context for bounded
synchronization operations. The selected Tungstenite client offers the handshake
request/response and typed errors, but no OpenTelemetry layer. `reqwest-tracing`
wraps `reqwest-middleware` requests; it cannot intercept this transport. The
inspected `tracing-opentelemetry-instrumentation-sdk` 0.42 HTTP module has server
and gRPC helpers, but no native WebSocket HTTP client implementation.

Retain the adapter around the real handshake: standard propagation injection,
client span attributes, exact response status, duration and bounded safe error
categories. No custom socket implementation or wire context envelope is needed.
Replacing the transport solely to consume Reqwest middleware would introduce a
separate migration, not remove an existing requirement. This is a scoped finding
for these integrations, not proof that no WebSocket instrumentation crate exists.

Sources: [connect_async](https://docs.rs/tokio-tungstenite/latest/tokio_tungstenite/fn.connect_async.html),
[Reqwest tracing middleware](https://docs.rs/reqwest-tracing/latest/reqwest_tracing/struct.TracingMiddleware.html),
[shared instrumentation HTTP module](https://docs.rs/tracing-opentelemetry-instrumentation-sdk/0.42.0/tracing_opentelemetry_instrumentation_sdk/http/index.html).

## Provider lifecycle: retain direct standard SDK composition

Inspected `init-tracing-opentelemetry` 0.43.0's `TracingConfig`, registration path
and `OtelGuard`. It can initialize traces, metrics and logs and customize the
subscriber, so configuration is not inherently a reason to reject it. However:

- Provider registration propagates an initialization error through the combined
  setup. The desktop must retain successful signals and local operation when one
  signal fails, and expose sanitized degradation diagnostics.
- Its guard's `Drop` calls force-flush/shutdown and discards all results. Clacks
  requires an explicit bounded shutdown result (or a safe warning for the CLI),
  while preserving the business command outcome.
- Desktop providers must finish before the nonblocking file writer is released.
  File I/O failures and bounded writer loss remain visible to the native UI.
- The chosen HTTP integration needs global providers, while domain metrics use
  the existing metrics-facade bridge.

Retain executable-owned composition of SDK providers, standard batch processors,
standard subscriber layers and the standard appender guard. The application owns
ordering and failure policy; it does not implement exporter retries, batching or
worker scheduling. Adopting the helper would still require taking over its
provider construction and lifecycle to meet these concrete requirements, so it
would add little over direct SDK composition. Do not introduce a common custom
provider-management framework to remove similar builder code across executables.

Sources: [configuration/registration](https://docs.rs/crate/init-tracing-opentelemetry/0.43.0/source/src/config.rs),
[guard lifecycle](https://docs.rs/crate/init-tracing-opentelemetry/0.43.0/source/src/otlp/mod.rs).

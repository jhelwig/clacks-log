# Synchronization transport comparison

Research checkpoint: 17 October 2026. D235 selects encrypted WebSockets for MVP synchronization. No protocol implementation or benchmark. Contributor-agreement work remains with legal review under D234.

## Accepted requirements

D59 requires the same reconciliation principles in both directions; D61 selects batch catch-up on connection/reconnection followed by streaming updates when available. D151–D161 select causal vectors, timeline ancestry, durable acknowledgments and full-account participation; D248 selects one scope per account. D156 requires durable installation before acknowledgment. D142 defines user-facing diagnostics; D230 requires full operator tracing, logs and metrics. D213/D223 select one Clacks server application with PostgreSQL and Axum/Leptos. Windows desktop is MVP with cross-platform foundations; the future browser interface must remain viable.

Transport selection does not change these requirements, replace the researched replication model, or imply different merge rules for client-to-server versus server-to-client traffic.

## Options

| Option | Fit | Costs and limits |
| --- | --- | --- |
| Encrypted WebSocket session | One bidirectional connection carries catch-up batches, changes in either direction, acceptance/comparison responses and acknowledgments. Native desktop and browser use are compatible with this communication model. | Clacks must correlate messages and instrument per-operation outcomes. Reconnection/replay and durable acknowledgments are application responsibilities. Proxies need appropriate upgrade and idle-timeout configuration. |
| Hypertext Transfer Protocol (HTTP) requests plus server-sent events (SSE) | Requests carry submissions/catch-up and the event stream carries live server updates. Fits Axum and browser workflows. | Two communication paths require coordinated catch-up/live-stream handling. Event-stream framing is text based; streaming proxy buffering/timeout behavior needs validation. This remains a credible alternative, not inherently unreliable. |
| gRPC, a remote procedure call framework | Typed service definitions and native streaming can carry the exchanges. | Adds Protocol Buffers/service-generation tooling alongside the selected ordinary Rust serialization direction. Browser gRPC-Web support is not equivalent to native bidirectional streaming. No requirement currently makes this additional stack preferable. |

Source findings: [WebSocket specification](https://www.rfc-editor.org/rfc/rfc6455.html) defines bidirectional communication and framing. Axum provides [WebSocket support](https://docs.rs/axum/latest/axum/extract/ws/index.html) and [SSE responses/keep-alives](https://docs.rs/axum/latest/axum/response/sse/index.html). The [SSE standard](https://html.spec.whatwg.org/multipage/server-sent-events.html) defines text event streams, browser reconnection and last-event-ID handling, with proxy/timeout authoring considerations. The [gRPC-Web project](https://github.com/grpc/grpc-web/blob/master/README.md#streaming-support), checked on this date, documents unary/server streaming but not client or bidirectional streaming. This is not a claim that using Rust requires TypeScript or a separately deployed proxy.

## Selected direction (D235)

Use an encrypted WebSocket session as the desktop synchronization transport, within the existing Axum application. Carry bounded catch-up batches and ongoing messages on the same connection. This is a fit recommendation for the bidirectional exchange, not a measured performance advantage over HTTP/SSE.

Both directions retain the same data/comparison semantics. A connection carries the account’s single synchronization scope; current tag/display selection does not control replication. Ordinary web/configuration endpoints remain part of the existing application; no additional service or shared-database topology is proposed.

On reconnect, negotiate using durably installed scope checkpoints and the selected fuller-state fallback. A successfully delivered frame is not proof of durable installation, and a missing acceptance response does not establish whether a write committed. Message identity/retry handling, authorization/revocation on live connections and gap-free catch-up/live transition still need specification using standard mechanisms. A transport does not itself resolve those issues. Do not introduce a new replication algorithm or permanent per-edit history as part of this choice.

HTTP/SSE is the alternative if ordinary request boundaries and browser event-stream integration are preferred over a single duplex session. D235 selects no automatic second-transport fallback, polling interval, message encoding, exact crate version or browser offline-sync feature.

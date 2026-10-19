# Clacks Log

Clacks Log is a planned amateur radio desktop logger with independent offline use on an arbitrary number of clients, reconciliation through a self-hostable server, and server-managed logbook service integration.

Product and architectural planning for the agreed minimum viable product (MVP) is complete (D242). The native-model and synchronization libraries provide pure domain values and conditional synchronization transitions; applications, persistence and integrations remain downstream capabilities. Rust is the intended implementation language wherever practical. PostgreSQL is selected for server persistence, embedded SQLite for client persistence (D213), and egui for the desktop interface (D215). No desktop/server application or database migration is delivered by this library.

## Planning documents

- [Planning completion and continuation handoff](docs/handoff.md)
- [Phased MVP roadmap](docs/mvp-roadmap.md)
- [GitHub issues](https://github.com/jhelwig/clacks-log/issues) and [milestones](https://github.com/jhelwig/clacks-log/milestones)
- [Native-model specification](docs/superpowers/specs/2026-10-18-native-model-design.md)
- [Native-model implementation plan and progress](docs/superpowers/plans/2026-10-18-native-model.md)
- [Product scope and operating workflows](docs/product.md)
- [Decision register](docs/decisions.md)
- [Architectural review](docs/architecture-review.md)
- [Client synchronization and conflicts](docs/synchronization.md)
- [External logbook services](docs/integrations.md)
- [Logbook of The World (LoTW) signing and certificate management](docs/lotw.md)
- [Remaining engineering and deferred work](docs/open-questions.md)
- [Research provenance and original documents](references/README.md)
- [TrustedQSL reference source map](references/trustedqsl/source-map.md)

The current planning documents reflect the conversation through 18 October 2026 (D01–D256). Original handoffs are historical evidence, not instructions or current requirements. Later explicit decisions take precedence over earlier research and assistant proposals.

## Native-model library

[`lib/clacks-model`](lib/clacks-model/README.md) contains recorded values, contacts,
profiles, equipment, tags, data-driven reference definitions, effective source
selection and in-memory reference-export planning. Rust 1.99.1 is pinned.
Run `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`
from this checkout. See the [native-model library overview](docs/native-model.md)
for scope, validation commands and downstream boundaries.

## Synchronization library

[`lib/clacks-sync`](lib/clacks-sync/README.md) supplies production metadata and
conditional transitions for offline edits, acceptance, complete transfer windows,
retirement and timeline recovery. The [integration contract](docs/synchronization-core.md)
defines real-store transaction obligations and recovery limits.

## Project direction

Each account owns its contacts; Clacks logbooks are tag-defined views. Contacts may carry multiple tags or remain untagged. Tags control organization and per-service automatic-upload eligibility, while all account data uses one synchronization scope. An external record can associate with at most one local contact within an account/service destination. The [product model](docs/product.md#contacts-logbooks-and-tags) describes membership, deletion, import and service behavior.

The desktop interface should support substantial operating context, efficient use of space, and specialist applications such as WS Suite (the WSJT-X family of weak-signal radio applications) and GridTracker. The logger is not intended to replace every application in the shack.

Synchronization will be self-hostable. The MVP server targets documented Linux container deployment; native Windows server hosting is not required (D189). An optional hosted subscription service is part of the product direction; its launch can be post-MVP (D241). The desktop targets eventual Windows, macOS, and Linux support. Windows desktop support is sufficient for the first release (D162); macOS/Linux releases are post-MVP, with cross-platform architecture and tooling required from the start. Minimum viable product (MVP) supports arbitrary client counts and multiple independent accounts on one server, with isolated account data, profiles, clients, and service credentials.

LoTW integration will use a source-guided Rust port of relevant tqsllib behavior, preserving interoperability while designing our own credential storage, tenant isolation, and execution model.

## License status

Clacks Log licenses its own application code under [Elastic License 2.0](LICENSE) (D221), without a free/community-hosting exception. Individual personal self-hosting remains allowed under the selected license; hosting for others is reserved whether paid or free. Included third-party reference code, dependencies and assets retain their applicable upstream licenses and notices.

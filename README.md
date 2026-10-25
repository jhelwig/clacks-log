# Clacks Log

Clacks Log is a planned amateur radio desktop logger with independent offline use on an arbitrary number of clients, reconciliation through a self-hostable server, and server-managed logbook service integration.

Product and architectural planning for the agreed minimum viable product (MVP) is complete (D242). The native-model and synchronization libraries provide pure domain values and conditional transitions. The client-storage library persists those values, pending work, evidence and complete-window receipts in SQLite. The desktop foundation creates or reopens a local account, presents startup diagnostics and coordinates activation and graceful shutdown. Logging workflows, server persistence and live provider adapters remain downstream capabilities.

## Documentation

- [Product scope and workflows](docs/product.md)
- [MVP roadmap](docs/mvp-roadmap.md)
- [GitHub issues](https://github.com/jhelwig/clacks-log/issues) and [milestones](https://github.com/jhelwig/clacks-log/milestones)
- [Decision register](docs/decisions.md)
- [Native-model contract](docs/native-model.md)
- [Durable offline client storage](docs/client-store.md)
- [Desktop application specification](docs/desktop.md)
- [Operating settings, station profiles and equipment](docs/operating-settings.md)
- [Geographic-pipeline specification](docs/geographic-pipeline.md)
- [Synchronization behavior](docs/synchronization.md) and [metadata model](docs/synchronization-model.md)
- [Accounts and enrollment](docs/accounts.md)
- [External services](docs/integrations.md), [service evidence and associations](docs/service-evidence.md), [LoTW workflow](docs/lotw.md) and [certificate/signing specification](docs/lotw-signing.md)
- [Backup and recovery](docs/backup-recovery.md)
- [Production observability](docs/observability.md)
- [Research provenance](references/README.md) and [TrustedQSL source map](references/trustedqsl/source-map.md)

Product and architecture requirements are settled. The desktop specification
consolidates D270–D279; implementation plans are prepared for the capability being
undertaken. GitHub owns delivery status; the roadmap describes phase outcomes
and dependencies. Historical source material is evidence, not operative guidance.

## Architecture

| Component | Selected direction |
| --- | --- |
| Desktop | Rust with egui/eframe, wgpu and egui_extras; Windows MVP, cross-platform foundations for later macOS/Linux clients |
| Storage | Embedded SQLite on clients; PostgreSQL on the server; ordinary transactions without a custom client write queue |
| Server | One Clacks application containing web, synchronization and background workers, deployed in Linux containers alongside PostgreSQL |
| Web | Leptos/Axum with server rendering and Rust/WebAssembly hydration; architecture accommodates later browsing, editing and reporting |
| Identity | Configurable OpenID Connect (OIDC), independent internal accounts, multiple login bindings and individually revocable clients |
| Secrets | Per-account encryption keys protected by a deployment wrapping key; directory credentials reach protected client storage, LoTW signing keys stay server-side |
| LoTW | Source-guided Rust port with pure-Rust certificate import, interpretation and signing |
| Geographic data | Census U.S. states/counties, HA8TKS CQ/International Telecommunication Union (ITU) zones, Natural Earth plus explicit DX Century Club (DXCC) mapping; Rust ingestion and `geo`, indexed FlatGeobuf layers, and Rust-only high-precision area coverage |

## Native-model library

[`lib/clacks-model`](lib/clacks-model/README.md) contains recorded values, contacts,
profiles, equipment, tags, data-driven reference definitions, effective source
selection and in-memory reference-export planning. See its crate guide for usage and adapter boundaries, and [development](#development) for workspace verification.

## Native desktop

Run `cargo run --bin clacks-log -- --help`, or start a separate local workspace
with `cargo run --bin clacks-log -- --workspace /absolute/path/to/workspace`.
The [desktop guide](lib/clacks-desktop/README.md) covers configuration, logs,
activation, shutdown and native Windows acceptance. This foundation does not yet
provide contact-entry or synchronization networking workflows.

## Synchronization library

[`lib/clacks-sync`](lib/clacks-sync/README.md) supplies production metadata and
conditional transitions for offline edits, acceptance, complete transfer windows,
retirement and timeline recovery. The [integration contract](lib/clacks-sync/README.md)
defines real-store transaction obligations and recovery limits.

## Geographic pipeline

[`lib/clacks-geography`](lib/clacks-geography/README.md) validates immutable data bundles and queries native points or entire locator areas. [`lib/clacks-geography-builder`](lib/clacks-geography-builder/README.md) and its thin CLI acquire pinned sources, compile reviewed geometry/mappings, run acceptance cases and atomically select releases. The representative [recipe](references/geography/README.md) exercises the production path; expanded coverage and application autofill belong to issue #23.

## Project direction

Each account owns its contacts; Clacks logbooks are tag-defined views. Contacts may carry multiple tags or remain untagged. Tags control organization and per-service automatic-upload eligibility, while all account data uses one synchronization scope. An external record can associate with at most one local contact within an account/service destination. The [product model](docs/product.md#contacts-logbooks-and-tags) describes membership, deletion, import and service behavior.

The desktop interface should support substantial operating context, efficient use of space, and specialist applications such as WS Suite (the WSJT-X family of weak-signal radio applications) and GridTracker. The logger is not intended to replace every application in the shack.

Synchronization will be self-hostable. The MVP server targets documented Linux container deployment; native Windows server hosting is not required (D189). An optional hosted subscription service is part of the product direction; its launch can be post-MVP (D241). The desktop targets eventual Windows, macOS, and Linux support. Windows desktop support is sufficient for the first release (D162); macOS/Linux releases are post-MVP, with cross-platform architecture and tooling required from the start. Minimum viable product (MVP) supports arbitrary client counts and multiple independent accounts on one server, with isolated account data, profiles, clients, and service credentials.

LoTW integration will use a source-guided Rust port of relevant tqsllib behavior, preserving interoperability while designing our own credential storage, tenant isolation, and execution model.

## License status

Clacks Log licenses its own application code under [Elastic License 2.0](LICENSE) (D221), without a free/community-hosting exception. Individual personal self-hosting remains allowed under the selected license; hosting for others is reserved whether paid or free. Included third-party reference code, dependencies and assets retain their applicable upstream licenses and notices.

Contributor-agreement selection, substantive adaptation and legal drafting are
reserved for a qualified lawyer (D234), before accepting external contributions
under that agreement. No agreement or future relicensing policy is selected.
The [agreement comparison](references/contributor-agreements.md) and
[questions for counsel](references/jetbrains-cla-review.md) retain the supporting
evidence. Dependency versions, features and distribution require their applicable
license and notice checks.

## Development

Use the pinned Rust 1.99.1 compiler and committed lockfile. Project-owned
maintenance tooling and non-trivial fixture generation are Rust too; Python is
not used, including for uncommitted fixture authoring.

The [maintenance tool](lib/clacks-reference-data/README.md) checks or regenerates both
derived LoTW datasets without executing reference sources:

```sh
cargo +1.99.1 run --bin clacks-lotw-data --locked --offline -- --help
cargo +1.99.1 run --bin clacks-lotw-data --locked --offline -- --check
cargo +1.99.1 run --bin clacks-lotw-data --locked --offline -- --write
```

Reconstruct and verify all synthetic LoTW fixtures using the locked Rust tools:

```sh
cargo +1.99.1 run --release --bin clacks-lotw-fixtures --locked --offline -- --check
```

Use `--help` for usage and `--write` to regenerate. The
[fixture recipes](lib/clacks-lotw/tests/fixtures/README.md) document public test
inputs and how the expectations are checked independently of production code.

Verify the workspace:

```sh
cargo +1.99.1 fmt --all --check
cargo +1.99.1 test --workspace --all-targets --locked --offline
cargo +1.99.1 test --workspace --doc --locked --offline
cargo +1.99.1 clippy --workspace --all-targets --locked --offline -- -D warnings
cargo +1.99.1 check --workspace --all-targets --target x86_64-pc-windows-msvc --locked --offline
RUSTDOCFLAGS='-D warnings' cargo +1.99.1 doc --workspace --no-deps --document-private-items --locked --offline
```

The server-side [LoTW library](lib/clacks-lotw/README.md) provides checked
certificate import and signed batch construction without native cryptography.
Its tests use independently constructed static inputs; supplied TQSL sources
remain reference-only.

Offline checks require cached dependencies and the selected target standard
library. Windows target checks do not exercise the runtime or installer.

Follow [collaboration guidance](AGENTS.md) when working on an issue. Keep
maintained requirements and integration contracts in their owning documents.
Completed execution plans are retained in Git history and pull requests; active
plans may remain under `docs/superpowers/plans/` while their work is in review.

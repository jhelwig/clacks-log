# Clacks Log

Clacks Log is owned and published by **Technosorcery, LLC**. See [NOTICE](NOTICE)
and [LICENSE](LICENSE) for project ownership and license terms.

Clacks Log is a pre-alpha amateur radio desktop logger with independent offline use on an arbitrary number of clients, reconciliation through a self-hostable server, and server-managed logbook service integration.

Product and architectural planning for the agreed minimum viable product (MVP) is complete (D242). The native-model and synchronization libraries provide pure domain values and conditional transitions. The client-storage library persists those values, pending work, evidence and complete-window receipts in SQLite. The desktop foundation creates or reopens a local account, presents startup diagnostics and coordinates activation and graceful shutdown. The desktop supports local manual contact entry, editing and individual deletion. The server provides PostgreSQL-backed accounts and OIDC login/identity management; enrolled desktops synchronize through authenticated WebSockets, while external-service adapters remain downstream capabilities.

## Documentation

- [Product scope and workflows](docs/product.md)
- [MVP roadmap](docs/mvp-roadmap.md)
- [GitHub issues](https://github.com/jhelwig/clacks-log/issues) and [milestones](https://github.com/jhelwig/clacks-log/milestones)
- [Decision register](docs/decisions.md)
- [Native-model contract](docs/native-model.md)
- [Synchronization retention and recovery](docs/synchronization-retention.md)
- [Durable offline client storage](docs/client-store.md)
- [Desktop application specification](docs/desktop.md)
- [Desktop workbench and interaction design](docs/desktop-ui.md)
- [Local contact history, Worked Before and queries](docs/history.md)
- [Contact logbook membership editing](docs/contact-membership.md)
- [Operating settings, station profiles and equipment](docs/operating-settings.md)
- [Offline logbook setup and view selection](docs/logbooks.md)
- [Offline manual contact entry, editing and deletion](docs/manual-logging.md)
- [Geographic-pipeline specification](docs/geographic-pipeline.md)
- [Synchronization behavior](docs/synchronization.md) and [metadata model](docs/synchronization-model.md)
- [Synchronization integration contract](docs/synchronization-integration-proposal.md)
- [Accounts and enrollment](docs/accounts.md)
- [Desktop enrollment schema and workflow proposal](docs/client-enrollment-proposal.md)
- [Credential storage and key rotation](docs/credential-protection-proposal.md)
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
| Desktop | Rust with egui/eframe, wgpu and egui_extras; early Windows installer followed by macOS packaging; Linux desktop deferred |
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
activation, shutdown and native Windows acceptance. Manual logging works offline;
enrolled workspaces reconnect automatically for account-wide synchronization.
The account web page configures client-direct QRZ/HamQTH lookup. Separate
contact-owned directory snapshots supply attributed fallback without changing
recorded contact values, including during sync outages. macOS credential access
requires the signed/profile-authorized bundle described in the desktop guide;
the Data Protection Keychain has no legacy migration or fallback.
The non-modal Unsynchronized changes window separates conflicts from ordinary
pending work and supports explicit per-field resolution without changing an
unfinished contact or active operating settings.

## Synchronization library

[`lib/clacks-sync`](lib/clacks-sync/README.md) supplies production metadata and
conditional transitions for offline edits, acceptance, complete transfer windows,
retirement and timeline recovery. The [integration contract](lib/clacks-sync/README.md)
defines real-store transaction obligations and recovery limits.

## Geographic pipeline

[`lib/clacks-geography`](lib/clacks-geography/README.md) validates immutable data bundles, queries native points or entire locator areas, and supplies shared geographic population rules. [`lib/clacks-geography-builder`](lib/clacks-geography-builder/README.md) and its thin CLI acquire pinned sources, compile reviewed geometry/mappings, run acceptance cases and atomically select releases. The [shipping recipe](references/geography/shipping/README.md) maintains U.S. coverage and documented worldwide limits. Desktop editors and automatic capture use the same offline rules; [native packaging](references/geography/BUILDING.md) restores one validated geographic build artifact or generates it on a cache miss, sharing the output between both platforms. Server-provided data updates remain future work.

## Project direction

Each account owns its contacts; Clacks logbooks are tag-defined views. Contacts may carry multiple tags or remain untagged. Tags control organization and per-service automatic-upload eligibility, while all account data uses one synchronization scope. An external record can associate with at most one local contact within an account/service destination. The [product model](docs/product.md#contacts-logbooks-and-tags) describes membership, deletion, import and service behavior.

The desktop interface should support substantial operating context, efficient use of space, and specialist applications such as WS Suite (the WSJT-X family of weak-signal radio applications) and GridTracker. The logger is not intended to replace every application in the shack.

Synchronization will be self-hostable. The MVP server targets documented Linux container deployment; native Windows server hosting is not required (D189). An optional hosted subscription service is part of the product direction; its launch can be post-MVP (D241). D392 advances release-mode desktop builds before complete MVP workflow readiness: deliver the early Windows installer first, then macOS packaging as a separate milestone. Windows signing is optional for early artifacts and must extend the same pipeline later; macOS retains D391 credential-related signing requirements. Final release-readiness validation covers both platforms; Linux desktop delivery remains deferred. Minimum viable product (MVP) supports arbitrary client counts and multiple independent accounts on one server, with isolated account data, profiles, clients, and service credentials.

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

For a running server and web interface, follow the complete
[server development setup](docs/server-development.md), including local PostgreSQL,
role/password setup, secret-file formats, OIDC configuration and the watch loop.

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

Model, synchronization, client-store and LoTW integration scenarios are modules
of each crate's `tests/integration.rs` target. Register new cases there so shared
fixtures compile once and the dependency graph is linked once per suite. For
example, run the contact-page cases with:

```sh
cargo +1.99.1 test -p clacks-client-store --test integration --locked --offline contact_pages::
```

Continuous integration (CI) builds workspace applications and tests together,
runs the resulting desktop
executable for its command-line smoke check, then runs the full test suite.
Compilation and execution have separate workflow steps. The workflows define the runner requirements for Linux workspace and web
verification. Job-scoped PostgreSQL containers provide disposable test databases.
Windows CI is temporarily disabled because of its runtime cost. Cargo
timing HTML is available in the `cargo-timings-ubuntu-latest` artifact for seven
days and reports compilation units and their dependency timelines.

The self-hosted runner is scoped to this private repository and trusted writers.
Same-repository pull-request conditions are defense in depth, not a security
boundary for a public repository: a fork can change its workflow. Before making
the repository public, establish an enforced trusted-workflow boundary for runner
access. An issues-and-feature-requests-only policy does not enforce this itself.

The ordinary suite checks IPv6 multicast socket construction and options.
IPv6 packet delivery, receiver coexistence, group isolation and capture are an
explicit native check using an existing non-loopback IPv6 interface. The case
is listed as ignored, with its prerequisite, in ordinary test runs. Run it with:

```sh
cargo +1.99.1 test -p clacks-desktop --lib --locked --offline ws_suite::tests::socket::ipv6_multicast_capture_on_existing_interface -- --ignored --exact --nocapture
```

The check reports the selected interface and keeps traffic on this computer
using interface-local scope and zero multicast hops. It fails if no suitable
interface exists or delivery fails; that platform's IPv6 verification remains
incomplete. No adapters, routes or test address overrides are provisioned.
A green ordinary CI run does not establish IPv6 packet-delivery acceptance.
Native Windows multicast verification remains required for the MVP.

The [account server](lib/clacks-server/README.md),
[web interface](lib/clacks-web/README.md) and
[Linux deployment](deploy/server/README.md) provide the production account,
identity-management, synchronization and release-operations foundation. Full workspace tests require the disposable
PostgreSQL database and role-specific environment variables documented in the
server guide; they never skip silently when that configuration is missing.

Verify the workspace:

```sh
cargo +1.99.1 install cargo-sort --version 2.1.4 --locked
cargo +1.99.1 sort --workspace --check --check-format
cargo +1.99.1 fmt --all --check
cargo +1.99.1 test --workspace --all-targets --locked --offline
cargo +1.99.1 test --workspace --doc --locked --offline
cargo +1.99.1 clippy --workspace --all-targets --locked --offline -- -D warnings
cargo +1.99.1 check --workspace --all-targets --target x86_64-pc-windows-msvc --locked --offline
RUSTDOCFLAGS='-D warnings' cargo +1.99.1 doc --workspace --no-deps --document-private-items --locked --offline
```

Use `members = ["bin/*", "lib/*"]` so new crates join the workspace automatically
without maintaining an individual member list. Keep all dependency tables
alphabetical, with no informal blank-line groups.
[cargo-sort](https://github.com/DevinR528/cargo-sort) handles
the root and member manifests, including target-specific dependencies; run
`cargo +1.99.1 sort --workspace` from the repository root to apply the formatting
configured in `tomlfmt.toml`. Comments belong beside the entry whose purpose or
constraint they explain. CI runs the same check before compiling applications.

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

## Desktop packaging

The [desktop package workflow](deploy/desktop/windows/README.md) provides
manual development builds as Actions artifacts and version-tag builds as draft
GitHub Releases. It targets current-user installation on Windows 11 x64;
early artifacts are unsigned. Windows builds do not run for pull requests or
ordinary branch pushes. Release builds reuse existing successful Linux/web
results for the exact tagged commit rather than rerunning them. Native Windows
acceptance and release publication remain distinct from package construction.

The same workflow builds a [macOS disk image](deploy/desktop/macos/README.md)
for macOS 26 Tahoe and newer on Apple Silicon. Mac development downloads and
tagged artifacts require Developer ID signing, the existing Keychain profile
authorization, notarization and stapling. Manual dispatch selects Windows,
macOS or both; tags require both packages before creating one unpublished draft.
The owner supplies signing inputs and tests the exact installed artifacts before
publication. Packaging automation alone does not establish native acceptance.

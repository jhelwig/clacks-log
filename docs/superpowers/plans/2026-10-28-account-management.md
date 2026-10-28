# Account Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Do not start implementation until the user authorizes execution.

**Goal:** Deliver #11's production PostgreSQL account store, browser sign-in and identity management, Leptos interface, and Linux-container deployment.

**Architecture:** One Axum server uses `openidconnect` for OpenID Connect (OIDC), `axum_session` for general sessions and cookies, and separate PostgreSQL login records for authoritative authentication. Leptos server rendering and browser hydration call the same account operations through standard Axum extraction. `clacks-admin` is one thin operator CLI over production library operations, with credentials resolved only for its selected subcommand.

**Tech stack:** Rust 1.99.1, SQLx 0.9.0, PostgreSQL, Axum 0.8, Leptos 0.8 with `leptos_axum`, `openidconnect` 4.0.1 with asynchronous reqwest/Rustls, `axum_session` 0.22.0 and `axum_session_sqlx` 0.12.0, existing Tokio/config/clap/observability libraries.

**Design:** [Account-management proposal](../../account-management-proposal.md), [accepted account decisions](../../accounts.md), D231–D233 and D353–D369. The request to prepare this plan carries forward that proposal as the planning basis; engineering defaults remain distinct from accepted product policy.

## Global constraints

- Use `rust-toolchain.toml` as the toolchain source. Preserve Rust 1.99.1.
- Use `lib/<crate>` and thin `bin/<application>` crates, workspace inheritance, named module files, a cohesive `clacks-admin` operator command and standalone rustdoc.
- No Python, xtask, TypeScript application layer, custom identity/session framework, pending-flow database, freshness cache, write queue or tenant connection settings.
- No production `expect`/`unwrap`, unchecked indexing, anonymous tuple bundles, telemetry-only wrappers or unnecessary public re-exports. Use existing error conventions: domain errors through `thiserror`, orchestration through `anyhow`.
- Fixed queries live in `.sql` files and use `query_file!`/`query_file_as!`. SQLx owns migration history, checksums and locking.
- Account changes use short transactions and account-row locking. Provider/network operations stay outside database transactions.
- New production components must remain in the final product. Later issues add enrollment, synchronization/shared-data persistence and account service-secret protection; do not simulate them here.
- Instrument meaningful operations as they arrive. Apply [observability.md](../../observability.md) without lifecycle wrappers, payload logging or high-cardinality metrics.
- Do not re-enable Windows CI or trigger hosted CI while the user's monthly allocation is unavailable. Local Linux-container/PostgreSQL verification is required. No automatic push or PR creation is authorized by this plan.
- Before execution, preserve existing planning edits and the unrelated `references/desktop-toolkit.md` changes. Use the global worktree-placement skill if isolation is created; never discard or silently omit the approved planning inputs.

## Review focus

1. Identity target supplied by another account must never authorize a binding change. Task 4 checks the real operation.
2. Concurrent unlink operations must preserve a remaining login identity. Task 4 checks committed outcomes with both production transactions.
3. A stale middleware reference after logout/unlink must not revive authentication. Task 3/4 check subsequent production requests.
4. Linking callbacks must not provision another account or switch the current authenticated identity. Task 4 checks both callbacks and final state.
5. A hydrated account page must not expose server configuration or secrets, and must remain usable through provider errors and form failures. Task 5 checks the assembled production UI and emitted data.

## File and dependency map

Create these focused owning modules; add subordinate files when their responsibility warrants it, not to hit a line-count target. No source file should grow into a >600-line aggregation.

| Path | Responsibility |
| --- | --- |
| `lib/clacks-server/src/lib.rs` | Module declarations and server-library documentation. |
| `src/config.rs`, `src/config/{providers,secrets}.rs` | Typed config resolution, deployment-secret inputs and validation. |
| `src/store.rs`, `src/store/{accounts,identities,logins,administrators}.rs` | Concrete SQLx PostgreSQL operations and transaction boundaries; named row types with their owners. |
| `src/auth.rs`, `src/auth/{principal,session,oidc,pending,callbacks}.rs` | Standard Axum authentication, library configuration, typed pending callback values and routes. |
| `src/maintenance.rs`, `src/maintenance/{cli,migrate,registration,roles,recovery}.rs` | Shared clap command interface and production release/operator operations. |
| `src/runtime.rs`, `src/http.rs`, `src/telemetry.rs` | Runtime composition, health/shutdown/request policies and observability configuration. |
| `lib/clacks-server/migrations/0001_accounts.sql`, `sql/<operation>.sql`, `.sqlx/` | Actual schema, external fixed queries and server-only offline metadata. |
| `lib/clacks-server/tests/accounts.rs`, `tests/support/{mod,provider}.rs` | Small number of production-operation/HTTP scenarios; test-only fixture support. |
| `lib/clacks-web/src/{lib,app,pages,actions}.rs`, `src/pages/{sign_in,identities}.rs` | Hydratable Leptos shell, account pages and server actions. |
| `lib/clacks-web/style/account.css` | Defined sections, typography, validation and interaction states for the actual account screens. |
| `bin/clacks-server/src/main.rs` | Server/web composition, no business logic. |
| `bin/clacks-admin/src/main.rs` | Thin operator entry point for `role grant`, `role revoke`, `identity recover`, `oidc register` and `database migrate`, with standard nested help. |
| `deploy/server/{Dockerfile,compose.yaml,config.example.toml,database-roles.sql}` | Immutable image, local/self-hosted deployment example, nonsecret config and explicit grants. |

All `src/` paths in the first six rows are relative to `lib/clacks-server`.
Create crate manifests/READMEs with their first real deliverable. Update root `Cargo.toml`, `Cargo.lock`, `.github/workflows/rust.yml`, README and owning docs as their integrations arrive.

`clacks-web` has `ssr` and `hydrate` features; its optional server dependency is enabled only for `ssr`. `clacks-server` never depends on `clacks-web`. The server binary composes both; the browser build contains the web library only. `clacks-admin` depends on `clacks-server`, not Leptos. Name the server executable package `clacks-server-app` (with `[[bin]] name = "clacks-server"`), so it does not collide with the library package `clacks-server`; its directory remains `bin/clacks-server`.

### Shared interface decisions

Keep these concrete types in their owning modules; do not introduce store traits or an authentication framework. Signatures describe the cross-task seam, not a mandate for a public API on every helper.

- `config::ServerConfig::load(cli: &ServerArgs) -> anyhow::Result<ServerConfig>`; contains public URL, listener, trusted proxies, provider configs, secret paths, session duration and telemetry config. Defaults use Serde/type defaults with the `config` crate.
- `store::AccountStore::new(pool: PgPool) -> AccountStore` owns a concrete pool. Methods return `Result<T, StoreError>`; `StoreError` exposes wrapped sources and named domain failures.
- `auth::oidc::VerifiedIdentity` is an internal named input record containing issuer, subject and optional display name. Production callers construct it from library-verified claims. Store module tests can supply its ordinary crate-visible fields to test persistence decisions without a public constructor, proof-token abstraction or test-only API. Define the stored principal/login records in Task 1; Task 3 adds the HTTP extractor and OIDC wiring.
- `auth::principal::Principal` contains `AccountId` and an opaque identity ID; its Axum extractor verifies current login validity using `ServerState`. The native login reference remains an internal secret, not a display ID.
- `auth::pending::PendingFlow` has named fields for provider, nonce, verifier, creation time and a purpose enum: normal sign-in, authorize link, complete link, or authorize unlink. Action-local fields include originating account/login, authorizing identity and target/provider where needed. No reusable recent-authentication assertion.
- `runtime::ServerState` holds the store, validated config and configured provider clients. `http::router(state: ServerState, pages: Router<ServerState>) -> Router` mounts production authentication and health routes. Web composition adds the Leptos routes without a circular dependency.
- Account operations: `provision_login(VerifiedIdentity) -> Result<EstablishedLogin, StoreError>`, `authenticate(&LoginReference, Duration) -> Result<Option<Principal>, StoreError>`, `logout(&LoginReference) -> Result<(), StoreError>`, and named `LinkIdentity`/`UnlinkIdentity` command structs consumed by `link_identity`/`unlink_identity`. `EstablishedLogin` contains the principal and secret reference. Every mutation validates its own transaction-time conditions.
- Maintenance operations accept typed account/identity inputs and return named outcomes: `AlreadyPresent`, `Changed`, or a specific refusal/error. They do not print or parse command-line arguments themselves.
- `maintenance::cli` owns the clap command enum and `clacks-admin` dispatch. Resolve only the chosen subcommand's inputs using the shared configuration facilities; never call full web-server startup/configuration to run maintenance. Keep separate runtime, migration and maintenance database connection-secret paths, with no fallback to a more privileged connection. `oidc register` reads only the selected provider's registration inputs and destination; it never opens PostgreSQL. The proposal's [credential matrix](../../account-management-proposal.md#release-and-operator-procedures) is the access contract.

## Task 1: Durable account operations and release-managed migrations

**Files:** Create server crate, `store` modules, migration, fixed queries, server README, `bin/clacks-admin`, its initial `database migrate` subcommand and `src/store/tests.rs`. Modify workspace manifests/lockfile.

**Consumes:** Native `AccountId`, SQLx 0.9.0 and the proposal's schema/transaction contract.
**Produces:** `AccountStore`, its row/error/outcome types, login lifecycle operations, and `clacks-admin database migrate`.

- [ ] Pin the selected dependency baseline in workspace metadata. Add PostgreSQL/Rustls features at the server member so the client retains its SQLite dependency choices. Keep optional OIDC/web dependencies out until their owning tasks. Use `time` types consistently with existing native timestamps; any Chrono conversion needed by the OIDC verifier stays at that boundary. Duration inputs below use `time::Duration`.
- [ ] Implement the actual five-table schema: accounts, unique exact issuer/subject identity bindings, authoritative logins with composite ownership foreign key/delete cascade, administrator assignments and the adapter-compatible generic session table. Use schema-qualified SQL, explicit indexes and adapter table configuration. Identity IDs use ordinary stable opaque IDs; login references use an established cryptographically secure generator. Do not add identifier-format CHECK constraints solely to mirror Rust parsing.
- [ ] Write the first store-module behavior scenario against real PostgreSQL: concurrent validated sign-ins for one identity resolve one account and usable login records; the losing attempt leaves no extra account. Assert through production account operations, not by testing unique constraints themselves. Supply ordinary internal identity input records in this store-level scenario; OIDC verification itself is outside its subject. Keep that input access crate-local, without a test-only constructor or sealed proof abstraction.
- [ ] Implement first-sign-in account/identity creation with rollback-and-fresh-lookup on the specific uniqueness collision. Admission to an existing binding coordinates through the account lock. Implement conditional unexpired-login refresh and deletion; refreshing must never insert or resurrect a row.
- [ ] Implement `maintenance::migrate::run(pool: &PgPool) -> anyhow::Result<()>` with embedded SQLx migrations, and expose it through the thin `clacks-admin database migrate` subcommand. Runtime code never calls migration execution. No custom schema marker, version registry or startup lock.
- [ ] Create the ordinary clap subcommand hierarchy in the server library and thin `bin/clacks-admin` wrapper. Implement only `database migrate` in this task; later tasks extend this same command. Require its migration connection input, without loading provider/cookie secrets or starting the web server. Verify `cargo run -p clacks-admin -- database migrate --help` and the actual migration invocation against the disposable database; do not test clap parsing as an independent subject.
- [ ] Add server query metadata under `lib/clacks-server/.sqlx/`, using SQLx's native package-local cache support. Preserve existing workspace-root client metadata. A workspace-root regeneration for one backend would delete the other's metadata; do not write a custom cache merger.
- [ ] Against a fresh disposable PostgreSQL database, run migrations and generate server metadata from `lib/clacks-server` with `SQLX_OFFLINE=false cargo sqlx prepare -- --all-targets --locked`. Check with `cargo sqlx prepare --check -- --all-targets --locked`. The client keeps its existing scoped workspace-root command.
- [ ] Build the server library and operator binary with `SQLX_OFFLINE=true cargo check -p clacks-server -p clacks-admin --all-targets --locked`. Run `SQLX_OFFLINE=true cargo test -p clacks-server --lib --locked` with the disposable test database configured; the provisioning scenario must pass now, without waiting for callback fixtures. Inspect schema grants and queries directly for ownership assumptions.
- [ ] Commit this independently reviewable storage/migration change with a substantive, physically wrapped message. Later tasks add queries and metadata alongside their operations.

## Task 2: Typed deployment configuration and durable provider registration

**Files:** Server config, OIDC provider configuration and registration modules; extend `bin/clacks-admin` with `oidc register`; example config and library README.

**Consumes:** Task 1's crate and migration boundary; D362–D365.
**Produces:** `ServerConfig`, validated provider/client setup, the reusable registration-file loader and `clacks-admin oidc register`.

- [ ] Add `openidconnect` 4.0.1 with its asynchronous reqwest/Rustls backend; disable native TLS and automatic HTTP redirects. Use library discovery, registration request/response and standard verification APIs. Resolve and lock compatible released dependencies; verify with `cargo tree -p clacks-server -e features` rather than assuming matching top-level version numbers prove feature compatibility.
- [ ] Implement conventional `--config <PATH>` and deployment environment/file sources through `config`/clap. Deserialize defaults on types: authoritative session duration 14 days, pending round-trip duration ten minutes. Bound pending entries to eight per browser session as an implementation default, discarding expired entries and then the oldest entry when needed. Document that the latter limit is tuning, not an account policy.
- [ ] Resolve secret files without logging their contents. Validate the public URL, fixed callbacks, configured provider keys/issuers and preconfigured-versus-dynamic mode. Use a durable cookie key shared by instances. No implicit provider discovery from arbitrary incoming URLs.
- [ ] Add `oidc register` to the existing `clacks-admin` hierarchy. Its setup path requires only selected-provider registration authorization, callback configuration and the destination file; unrelated database/cookie secret paths may be absent. Do not give registration a database credential merely because another subcommand uses PostgreSQL.
- [ ] Implement `registration::run(provider: &ProviderConfig, public_url: &Url) -> anyhow::Result<RegistrationOutcome>` with the standard registration types. If a matching saved registration exists, reuse it without contacting the registration endpoint. Refuse mismatched issuer/callback configuration with an actionable operator error.
- [ ] Publish newly returned registration data using a protected temporary file and atomic no-clobber publication in the destination directory. Keep an existing destination unchanged. Report uncertain external completion separately from a local publication failure; never silently register again. Deployment automation serializes setup; do not add an application distributed lock.
- [ ] Exercise Clacks's registration reuse/error decisions using the narrow test provider for this task. Create the narrow registration/discovery HTTP fixture in `tests/support/provider.rs` now; Task 3 extends the same fixture for callbacks. One scenario must prove a second setup uses the saved client ID without another registration request and a mismatched configuration preserves the file. Do not test Serde, filesystem rename semantics or protocol cryptography.
- [ ] Verify `clacks-admin oidc register --help`, configuration diagnostics without secrets, offline compilation and package-local SQL metadata. Commit provider setup/configuration with its documentation and focused assertions.

## Task 3: Production browser sign-in and authoritative authentication

**Files:** `auth` modules, `runtime.rs`, `http.rs`, telemetry composition, server executable, and test-only provider fixture.

**Consumes:** Tasks 1–2, middleware session APIs and configured OIDC clients.
**Produces:** Real sign-in/callback/logout routes, `Principal` extractor and `ServerState` for web/actions.

- [ ] Add `axum_session` 0.22.0 and PostgreSQL adapter 0.12.0 against the selected Axum 0.8 baseline. Connect the adapter to its already migrated table through supported public APIs without startup schema initialization: construct `SessionPgSessionStore::new(None, config)`, then attach `store.client = Some(pool.into())` before installing `SessionLayer`. Do not use the constructor with a live backend, which invokes schema initialization, or enable optional key-store machinery without a demonstrated need. Retain native cookies, typed data, `get_remove`, `renew` and `destroy`; do not wrap them in a parallel session abstraction.
- [ ] Configure middleware lifetime and cookie maximum age consistently with the 14-day authoritative sliding policy; configure long-term behavior explicitly rather than inheriting a shorter or conflicting default. Secure/HTTP-only/host-only cookie at `/`, SameSite=Lax, no IP/user-agent binding. HTTP development mode is explicit.
- [ ] Implement state-keyed typed pending entries. Compare secrets through the OIDC library's timing-resistant support. Validate provider selection, expiry and callback purpose; remove the applicable pending entry using native session APIs. Do not promise distributed atomic consumption or add a replay table. Missing/stale context fails closed with a restart path.
- [ ] Use standard authorization-code requests with random nonce and Proof Key for Code Exchange (PKCE). Exchange through the configured async client and validate tokens with the expected nonce/issuer/audience. Configure `set_auth_time_verifier_fn` when a flow requested `max_age=300`; enforce presence and the requested age without caching freshness. Reuse normal library verification errors without exposing token/provider bodies to users.
- [ ] Implement `/auth/start/{provider}`, `/auth/callback/{provider}`, `/auth/logout` and the `Principal` extractor. After validated normal sign-in, commit account/login state, retain only the secret login reference in middleware and renew the session ID. A database failure yields an availability error, not anonymous reprovisioning.
- [ ] Apply same-origin POST validation using standard Axum/Tower request facilities and parsed `Origin`/fallback `Referer` against the configured public origin. Reject missing/null/mismatched origins for browser mutations; exempt the correlated GET provider callback. Do not write custom CSRF cryptography. CSRF means cross-site request forgery.
- [ ] Extend Task 2's test-only provider fixture with token and public-key responses needed to drive Clacks routes; retain its discovery/registration behavior. Author inputs in Rust using library signing facilities or reproducible Rust fixtures. Do not implement an authorization provider framework, verify dependency cryptography, expose test hooks in production, or introduce a browser-driver test stack just for this fixture.
- [ ] Complete a compact production HTTP scenario: sign in, exercise an authenticated request, logout, replay the old cookie/reference on another application instance, and observe unauthenticated status. Include a wrong-purpose/expired callback and verify no account mutation. Provider fixture failures must leave a clear restart outcome.
- [ ] Use one explicitly configured disposable test database per integration test process, with production migrations applied once. Let scenarios create distinct accounts/identities. Use a few combined application scenarios, not a fresh migrated database per field/assertion or a generic fixture-reset framework. A missing test database is a clear test-run configuration error, not a silently passing test.
- [ ] Add safe request spans/events and bounded request latency/outcome metrics now. Exclude callback query strings, cookies and all pending/login/token secrets. Use standard instrumentation lifecycle and module targets; record durable provisioning/logout outcomes after commit.
- [ ] Verify the server can start with a restricted runtime role, serve live/ready endpoints and shut down using ordinary Axum/Tokio facilities. Run targeted tests with `CLACKS_TEST_DATABASE_URL` pointing at the disposable database and `SQLX_OFFLINE=true cargo test -p clacks-server --test integration --locked`. Commit sign-in/session integration.

## Task 4: Identity changes and out-of-band administration

**Files:** Identity/administrator store queries, callback purpose handling, role/recovery maintenance modules and `clacks-admin` subcommands; extend the existing integration scenarios.

**Consumes:** `Principal`, pending flows, verified identities, Task 1's transaction model.
**Produces:** Self-service link/unlink operations and `clacks-admin role grant`, `role revoke` and `identity recover`.

- [ ] Implement link initiation as existing-identity authentication with `max_age=300`, followed by a separate new-identity authorization round trip. Its continuation is restricted to this operation and originating login/account. Use supported standard account-selection prompting where appropriate; no provider-specific workaround.
- [ ] At final link callback, lock account and recheck originating login, authorizing binding and target ownership. Link unbound identity; return unchanged for already linked to this account; refuse ownership elsewhere. Never fall through into provisioning, move ownership or switch the session's authenticating identity.
- [ ] Implement unlink through an identity that remains linked. Under the account lock recheck all conditions, remove the target and invalidate precisely its login sessions atomically. If this invalidates the initiating session, return to sign-in. Keep unrelated sessions and desktop credentials unchanged.
- [ ] Add the actual cross-account and concurrency assertions to the existing integration binary: an A operation targeting B's identity does not mutate B; two competing unlinks leave a valid remaining identity; a removed identity's old login fails while the other identity's login succeeds. Coordinate concurrent production operations in test code, without triggers or production failpoints.
- [ ] Extend the HTTP scenario for both link callbacks. Assert stable account/session identity, no additional account after an attempted link owned elsewhere, and no binding changes after logout between callbacks. These assertions cover Clacks dispatch/admission, not the provider's token machinery.
- [ ] Implement role grant/revoke and recovery methods with named outcomes, exposed through `clacks-admin role grant`, `clacks-admin role revoke` and `clacks-admin identity recover`. Standard arguments identify account ID and, for recovery, exact issuer/subject; the operator performs documented verification before running the command. No automatic identity verification from email.
- [ ] Resolve role/recovery database access from the maintenance credential input, without migration privileges or provider/cookie secrets. Use the maintenance grants required by the selected operation; this does not prescribe a separate database login for every subcommand. Verify real operator commands in the restricted-role deployment exercise; do not add a second generic credential dispatcher or a privilege-escalating fallback.
- [ ] Verify role changes through the actual role-authorized path and recovery through normal sign-in afterward. Recovery preserves existing bindings/logins and refuses an already-owned replacement. No test-only administrator HTTP operation; use the production maintenance operation.
- [ ] Add distinct committed operator events for identity/role/recovery outcomes with safe fields. Keep runtime `server_administrators` privileges read-only; maintenance gets only needed write grants. Update query metadata and verify focused scenarios, help and clippy. Commit identity management/maintenance.

## Task 5: Leptos account interface and hydration

**Files:** Web crate, page/action/style modules, root build metadata and server composition. Update account/web READMEs.

**Consumes:** Server account operations and `Principal` through `leptos_axum::extract_with_state`; no duplicate account logic.
**Produces:** Real server-rendered and hydrated sign-in/identity pages.

- [ ] Add Leptos 0.8 and matching `leptos_axum` integration with `ssr`/`hydrate` feature separation. Use normal native/Tokio defaults for `leptos_axum`; do not accidentally select its alternate JavaScript-hosted-server mode. Lock compatible patch releases after actual compilation.
- [ ] Configure standard `cargo-leptos` workspace build metadata with the server binary package and web library package, `ssr`/`hydrate` features, stylesheet and output directory. Use released `cargo-leptos` 0.3.11 as the evaluated tooling baseline; lock its installation with `--version`/`--locked`. Use Rust tooling for generated assets, no Node application build or custom task runner.
- [ ] Implement sign-in provider selection, linked identity list, current identity label, Add identity, Remove and Sign out. Use server functions/forms with account context obtained from the authenticated request, not browser account IDs. Disable last-identity removal with an explanation; server operations remain authoritative.
- [ ] Use defined sections, coherent typography and modest density. Implement disabled/pending submit states, inline action errors and clear restart paths for cancelled/expired/provider-failed actions. Include keyboard focus, accessible labels and narrow-window behavior. No placeholder future dashboard or administrator role editor.
- [ ] Mount server functions under the same mutation/authentication policies as ordinary account routes. Rendered pages and direct server-function calls enforce the same account boundary. Never serialize config, provider tokens, login references or pending contexts into hydration data.
- [ ] Extend an existing account HTTP scenario only where necessary to prove the real rendered/action boundary, such as a direct attempted removal using another account's identity. Do not duplicate all store assertions across HTTP and UI, or test Leptos HTML/Serde internals.
- [ ] Build the production assets with `cargo leptos build --release`, then inspect actual pages in a browser with the production server: first sign-in, two identities, last-identity restriction, link/refusal/unlink, provider error, expired action and invalidated current login. Inspect browser/network responses for secret exclusion. Record visual evidence in the delivery response/PR, not a repository audit report.
- [ ] Verify hydration with `cargo check -p clacks-web --no-default-features --features hydrate --target wasm32-unknown-unknown --locked` and inspect its dependency tree: no server/store/SQLite/OpenSSL runtime dependencies. Preserve Rust 1.99.1; add only the required WebAssembly target, no toolchain upgrade. Commit web interface/assets.

## Task 6: Linux deployment, scoped verification and completion

**Files:** Dockerfile/Compose/config/grants, runtime startup/health/shutdown, workflow, READMEs, owning account/backup/observability docs.

**Consumes:** All production deliverables above.
**Produces:** Usable immutable release image, deployment/maintenance procedures and verified #11 outcome.

- [ ] Package `clacks-server`, `clacks-admin` and hydrated assets in one nonroot Linux image. Use `rust-toolchain.toml` to obtain the builder Rust version; pin the chosen image digest in implementation. Configure one application service plus PostgreSQL and an example ingress boundary, not separately deployed services that share its tables.
- [ ] Document and exercise explicit migration → registration setup/reuse → runtime startup → ordinary sign-in → role assignment. Use separate migration, runtime and maintenance grants from the proposal, without runtime DDL rights or runtime writes to administrator membership. Supply registration/cookie secrets through protected mounts.
- [ ] Document the actual `clacks-admin database migrate`, `oidc register`, `role grant`, `role revoke` and `identity recover` invocations and nested help. Exercise each operation with only its own required credential inputs present. Sharing the image and command must not make provider registration require PostgreSQL, or make database maintenance require provider/cookie secrets.
- [ ] Provide readiness from real database usability and loaded configuration, with process liveness separate. Provider outage should affect sign-in rather than invalidate all existing logins. Use standard graceful serving/shutdown and flush telemetry; propose 30 seconds as a configurable shutdown default, not a database or authentication guarantee.
- [ ] Document PostgreSQL/secret backup and restore, ambiguous registration outcomes, cookie-key replacement, recovery verification and the replacement-identity first-sign-in pitfall. Restoring older database authorization can restore older revoked state; do not claim otherwise. Keep uploads/recovery controls owned by later capabilities.
- [ ] Extend Linux verification with PostgreSQL service health, production migration application, server metadata checking and the small server scenario binary. Keep existing SQLite checks scoped to the client store. Install SQLx CLI 0.9.0 with `--no-default-features --features sqlite,postgres,rustls` and update the cached-tool key. Do not add MySQL/native-TLS defaults.
- [ ] Run the server metadata check from `lib/clacks-server` against PostgreSQL, without `--workspace`, and retain the existing root/client check against SQLite. Compile ordinary workspace code offline. `CLACKS_TEST_DATABASE_URL` is separate from compile-time `DATABASE_URL`; do not accidentally compile SQLite queries against PostgreSQL.
- [ ] Add a cached Leptos/hydration release-build check and include the new deployment paths in workflow filters. Keep main/PR trigger scope and disabled Windows matrix. Do not manually trigger remote CI until the user's allocation permits it; describe the pending hosted verification honestly.
- [ ] Run locally after locked dependency fetch: `cargo fmt --all --check`; `SQLX_OFFLINE=true cargo test --workspace --all-targets --locked`; `cargo test --workspace --doc --locked`; `cargo clippy --workspace --all-targets --locked -- -D warnings`; and `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --document-private-items --locked`. Supply the disposable PostgreSQL URL for tests. Check both metadata sets and the release hydration/container build separately.
- [ ] Exercise a real intended tsidp version/configuration for discovery, dynamic registration, PKCE and `max_age`/`auth_time`. Use a separate synthetic configured identity provider only for automated Clacks wiring scenarios, never as evidence of tsidp interoperability. If live tsidp is unavailable or fails the standard flow, report the exact gap and preserve it as incomplete acceptance evidence; do not add a bypass or replacement freshness model.
- [ ] Review production dependencies, public APIs, SQL, logging and assembled UI against the accepted design and user conventions. Verify planned tests have a concrete Clacks assertion. Remove accidental test scaffolding rather than preserving it to satisfy tests.
- [ ] Update current-product docs and crate usage guides. Keep this plan as an execution checklist until merge; no checked-in audit/hand-off narration. Commit deployment/integration work with a reviewed, 72-column-wrapped substantive body. Do not automatically stage the unrelated desktop-toolkit research.

## Execution and verification notes

Use a disposable local PostgreSQL Docker service for integration, not an existing personal database. Document its exact start/cleanup commands in the server README when implementation selects the image. Apply migrations once per disposable database and use real runtime-role connections in application scenarios. Remove only resources created for this task. No manual SQL mutation of migration history or fault-injection triggers.

The standard SQLx package-local metadata path is intentional: SQLx 0.9 preparation clears its destination's query JSON files, and macros search a crate-local `.sqlx` before the workspace-root directory. This allows two database backends without custom metadata bookkeeping. See [SQLx CLI source](https://github.com/launchbadge/sqlx/blob/v0.9.0/sqlx-cli/src/prepare.rs).

Keep product behavior settled; implement routine library wiring in the owning task. Stop for a demonstrated incompatibility or material design contradiction, with evidence, rather than inventing a workaround or asking for another approval of standard facilities.

Before each commit/amend, write the actual message to a file, inspect it and check physical body line lengths, preserving unbreakable identifiers. Use `git commit --file`, never the skill examples' one-line-only messages. Commit boundaries above describe coherent deliverables, not mandatory cleanup commits.

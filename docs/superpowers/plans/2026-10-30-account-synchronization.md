# Account Synchronization Implementation Plan (#14)

> **For agentic workers:** Use `superpowers:executing-plans` for native execution when authorized, or `superpowers:subagent-driven-development` if the user selects that method. Execute task-by-task; this plan does not authorize implementation, pushing or opening a PR.

**Goal:** Two enrolled desktop clients automatically exchange durable account state through the existing server while remaining independently writable offline.

**Architecture:** Extend the existing native synchronization core and SQLite adapter, and add a concrete PostgreSQL adapter inside the server. Journal discovery and fuller-state reconciliation feed the same incremental receipt/completion rules. The existing application runtimes own authenticated WebSockets and change notifications.

**Tech stack:** Rust 1.99.1, SQLx 0.9.0, PostgreSQL, bundled SQLite, Axum 0.8.9, Tokio, Serde, Huskarl and Rustls.

**Spec:** [Synchronization integration proposal](../../synchronization-integration-proposal.md), with the authoritative [behavior contract](../../synchronization.md), [metadata model](../../synchronization-model.md), [client store](../../client-store.md) and [service evidence](../../service-evidence.md).

## Global constraints

- Carry forward full-account scope, receive-first catch-up, record-level comparison, frozen retries, timeline ancestry and existing recovery/retention rules.
- Core rules are shared; SQLite/PostgreSQL SQL, row mapping and transaction execution are backend-specific. Purpose-specific operations own transactions; private helpers use their caller's connection.
- Preserve intrinsic Rust validity and atomic value encoding. Keep queryable facts relational; use named row types and external checked SQL with committed offline metadata.
- Use the existing account lock, runtime pools, application tasks and protected credential store. No generic repository, custom write queue, second synchronization algorithm or test-only production interfaces.
- No successful-sync timestamp or replacement aggregate currentness indicator. Connection state and current error remain the user-facing diagnostics.
- Use `#[instrument(skip_all, ...)]`, domain events and operation-owned metrics under [the observability contract](../../observability.md). Report committed effects separately from planned/staged work; exclude credentials, proofs and QSO contents.
- Libraries are in `lib/`, thin applications in `bin/`; inherit workspace dependencies/metadata. No `mod.rs` outside tests, production `expect`/`unwrap`, unchecked indexing or project-owned `dead_code` allowances.
- Production work and fixture tooling are Rust-only. Tests must identify a Clacks decision and the concrete mistake they detect, using production schemas and real operation boundaries. Do not test dependency machinery or inject database triggers/failpoints.
- Preserve the current Linux-only CI matrix and paused hosted-CI availability. Native Windows verification is explicit when available; do not re-enable Windows CI or change protocols to simplify tests.
- Read user-owned development guidance before running/configuring a server. Use disposable databases for verification, preserve the persistent environment, and use the worktree-placement skill before creating a worktree at execution time.
- Before each commit, prepare and inspect a substantive message with body prose physically wrapped at 72 columns. Fold branch-introduced repairs into their owning commits when requested.

## Engineering defaults carried into this plan

These are implementation choices from the proposal, not retroactively recorded product decisions:

1. Native Serde JSON messages, protocol version 1, at `/api/sync`.
2. Incrementally registered receipt requirements, sealed at discovery end, replacing the manifest-first restriction in the production path.
3. Existing `NeverCreated` original basis with an optional certified checkpoint, distinguishing known creation from lost history.
4. In-process Tokio change notices; durable state/journal supplies work and correctness.
5. Refresh/reconnect through a fresh authenticated upgrade when access authorization expires.

Batch/channel capacities, reconnection delays and transport limits are bounded implementation tuning. Choose them against actual serialized native values and avoid turning transport limits into native validity rules.

## Review focus

- A record changes or is deleted between discovery pages: the target obligation remains discoverable and is represented by the version or a proven successor (Tasks 2, 4, 9).
- Logging begins before first receipt: known new records become publishable after catch-up; genuinely lost originals retain conservative review (Tasks 1, 5, 9).
- A connection fails after some durable effects: retry neither advances incomplete coverage nor creates another accepted edit (Tasks 5–9).
- A concurrent membership survives tag deletion: only the invalid deletion is blocked; ordinary QSO classification and unrelated progress remain intact (Tasks 6, 9).
- Authority changes on an already-open connection: current database authority governs admitted work; a notification alone never grants or revokes authority (Tasks 7–9).

## File and interface map

Paths below describe production ownership. Split coherent child modules as needed to keep files focused; do not add an abstraction merely to make adapters identical.

| Owner/files | Responsibility and consumer |
| --- | --- |
| `lib/clacks-sync/src/client.rs`, `client/state.rs`, `client/reconciliation.rs`, `client/preparation.rs`, `compaction.rs`, `compaction/witness.rs` | Known-creation basis, existing comparison and witness rules consumed by the client store |
| `lib/clacks-sync/src/transfer.rs`, new `transfer/discovery.rs`, `transfer/receipt.rs`, existing service modules | Shared discovery eligibility, version-qualified requirements, receipt transitions and completion checks consumed by both adapters |
| `lib/clacks-sync/src/protocol.rs`, new `protocol/messages.rs` | Versioned shared wire values consumed by server and desktop |
| `lib/clacks-client-store/src/{save,transfer,read,retention}.rs`, `metadata/{client,lineage}.rs`, new `discovery.rs` | Actual SQLite preparation, accepted-state sourcing, transactional receipt and completed coverage |
| `lib/clacks-client-store/migrations/0005_sync_state.sql`, `sql/{original,window}/`, new `sql/discovery/`, workspace `.sqlx/` | SQLite representation and checked queries |
| New `lib/clacks-server/src/store/{payload,metadata,discovery,synchronization,rows}.rs` and focused child modules | PostgreSQL mapping, source reads and transaction-owning operations |
| New `lib/clacks-server/migrations/0004_synchronization.sql`, `sql/{payload,metadata,discovery,synchronization}/`, crate `.sqlx/`, `deploy/server/database-grants.sql` | Accepted relational state, journal, interpretation, receipts needed for recovery, runtime grants |
| `lib/clacks-server/src/store/accounts.rs`, `enrollment/membership.rs` | Shared initialization, actual account coverage and client acknowledgments |
| New `lib/clacks-server/src/synchronization.rs` and `synchronization/{connection,exchange}.rs`; existing `http.rs`, `runtime.rs`, `oauth/resource.rs` | Authorized WebSocket route, exchange dispatch, authority rechecks, owned shutdown |
| New `lib/clacks-desktop/src/synchronization.rs`, `synchronization/{connection,cycle}.rs`; existing `session.rs`, `session/settings.rs` and child workflows | Automatic synchronization and notices from settled local operations |
| Existing desktop `enrollment/flow.rs`, `enrollment/completion.rs`, session/view presentation | Reuse protected authorization, start after enrollment and refresh actual views |

New public APIs must have an actual application caller by branch completion. Keep server/session helpers crate-private. Do not add a new common-storage crate or make the server depend on the client store for production persistence.

## Execution and verification rhythm

Tasks 1–6 establish interfaces consumed by Tasks 7–8; the deliverable is the connected branch, not separate foundation releases. Execute them in order. For each authored behavior assertion, run the owning focused command before and after implementation, inspect the actual reason for failure, and finish with the task's broader affected checks. A missing dependency/configuration error is not evidence that the intended assertion failed. Commit each task's coherent production change and its meaningful verification; schema/SQL metadata updates belong with their owning task.

Use these scenario names when extending the existing suites so focused checks are discoverable:

| Task | Scenario and focused command |
| --- | --- |
| 1 | `new_creation_before_first_receipt_keeps_known_absence`: `SQLX_OFFLINE=true cargo test -p clacks-client-store --test integration new_creation_before_first_receipt --locked` |
| 2 | `incremental_receipt_requires_sealed_complete_obligations`: `SQLX_OFFLINE=true cargo test -p clacks-sync --test integration incremental_receipt --locked` |
| 3 | `account_defaults_are_accepted_once`: `SQLX_OFFLINE=true cargo test -p clacks-server --lib account_defaults_are_accepted_once --locked` |
| 4 | `moving_source_preserves_target_obligations`: run the `moving_source` filter in the server `--lib` suite and client-store `--test integration` suite |
| 5 | `interrupted_receipt_reuses_durable_state`: `SQLX_OFFLINE=true cargo test -p clacks-client-store --test integration interrupted_receipt --locked` |
| 6 | `tag_deletion_checks_current_reverse_references`: `SQLX_OFFLINE=true cargo test -p clacks-server --lib tag_deletion_checks --locked` |
| 7 | `authenticated_socket_uses_current_account_authority`: `SQLX_OFFLINE=true cargo test -p clacks-server --lib authenticated_socket --locked` |
| 8 | `session_receives_before_publishing_new_work`: `SQLX_OFFLINE=true cargo test -p clacks-desktop --lib session_receives --locked` |
| 9 | `two_clients_exchange_and_recover_without_duplicate_acceptance`: `SQLX_OFFLINE=true cargo test -p clacks-desktop --lib two_clients_exchange --locked` |

Extend an existing equivalent scenario instead of adding a duplicate fixture solely to use these names. Confirm each focused run actually executes its intended assertions; a zero-test result is not verification.

## Task 1: Preserve honest creation evidence before first receipt

**Files:** Core client/compaction files above; client `save.rs`, `metadata/client/original.rs`, `rows/client.rs`, `sql/original/`; existing `lib/clacks-sync/tests/client_state.rs`, `compaction.rs`, client-store `tests/local_saves.rs`, `tests/preparation.rs` and their existing module lists.

**Interfaces:** Change `OriginalBasis::NeverCreated { key, checkpoint }` to use `checkpoint: Option<InstalledCheckpoint>`. Retain the variant's original absence meaning and the distinct `Unavailable` variant. Existing `synchronization::prepare` remains the durable client preparation boundary.

- [x] Add focused assertions to existing save/preparation scenarios: a newly allocated identity saved before receipt reloads as known creation with no certified checkpoint; a genuinely unavailable original remains unavailable. Neither can prepare against an unavailable shared observation.
- [x] Run `SQLX_OFFLINE=true cargo test -p clacks-sync --test integration client_state --locked` and the affected client-store tests; confirm the new assertions expose the current conflation rather than a dependency behavior.
- [x] Update checked construction, matching, Serde restoration and original persistence for the optional checkpoint. At actual creation (`current == None` under the stale-save check), preserve known original absence; do not infer creation from a later missing server row.
- [x] Update compaction/witness logic so absent certification cannot produce a certified absence witness or release interpretation it never established. Existing certified originals retain their checkpoint evidence.
- [x] Adjust the existing pre-first-receipt test that currently expects permanent `MissingOriginal` review; retain its meaningful local-save assertions. Do not preserve that accidental outcome as a requirement.
- [x] Run the affected core/store scenarios and regenerate/check SQLite SQL metadata if query types change. Commit this coherent representation correction with its documentation.

## Task 2: Implement shared incremental discovery and receipt contracts

**Files:** Core transfer/protocol files in the map; `tests/transfer.rs`, `tests/service_transfer.rs`, `tests/durable_validation.rs`, `tests/integration.rs`, crate README.

**Interfaces:** Define these shared native contracts in their owning modules:

- `DiscoveryStart { source: InstalledCheckpoint, mode: WindowMode }`.
- `DiscoveryPage`: ordered page sequence, exact `RecordRequirement` values, accepted input, baseline/lineage interpretation and `CoupledRequirement` descriptors. Use named fields and existing native types.
- `DiscoveryEnd`: terminal page sequence and source target; seals the existing exchange, not a new coverage advertisement.
- `ReceiptState`: connection-owned source/mode, next sequence, exact registered requirements, interpretation and coupled-effect completion evidence. No persisted receipt ID or progress generation.
- `register_page` and `finish_receipt` in `transfer::receipt`: ordered obligation registration and completion against the matching source end and actual stored values. `transfer::readiness` shares dependency decisions; the client batch planner applies working-record reconciliation.
- `protocol::messages::{ClientMessage, ServerMessage}`: version introduction, discovery start/page/end and page acknowledgment, accepted-state recovery pages with included support, pending proposal/results, durable coverage acknowledgment and bounded actionable failures. Keep `AcceptanceCommit` internal; encode only committed result data.

- [x] Add scenarios with exact assertions: an unsealed receipt cannot complete; a missing requirement or staged coupled effect prevents completion; A92 plus B102 covering B96 completes target 100 without claiming 102; an incomparable B102 cannot substitute for B96.
- [x] Verify ordered pages and matching end; a new attempt at the same source target may register different qualifying successors. Repeated semantic values are safe across new exchanges; there is no within-receipt retransmission.
- [x] Run the focused core transfer tests to establish the failing cases.
- [x] Refactor the current `plan_batch`/completion logic into the shared receipt operations, retaining native `satisfies`, client receipt/comparison, service dependency and atomic-unit decisions. Requirements deduplicate by key plus exact stamp/deletion; do not force incomparable requirements into the current one-version-per-key manifest.
- [x] Define discovery eligibility from actual retained coverage/lineage. A missing journal interval selects fuller-state reconciliation; numeric gaps caused by account metadata updates are not missing record entries.
- [x] Define protocol rejection before state-changing requests for incompatible versions, account mismatch and invalid sequencing. Retain existing checked native deserialization. An end message alone cannot certify receiver completion.
- [x] Run core integration and documentation tests. Remove superseded manifest-only restrictions from the production API/README rather than retaining a parallel unused receipt engine. Commit.

## Task 3: Add PostgreSQL accepted-state persistence and initialization

**Files:** Server store modules/migration/SQL/grants above; `src/store.rs`, `src/store/accounts.rs`, `src/enrollment/membership.rs`; new `src/store/tests/synchronization.rs` registered in existing `store/tests.rs`.

**Interfaces:** Private `payload::{insert,load}(&mut PgConnection, ...)` and metadata helpers construct/load existing native values. Extend `AccountStore` operations for actual account/membership state. The account provisioning transaction initializes synchronization metadata and accepts `clacks_model::defaults::account_definitions` once.

- [x] Extend real account scenarios to assert that newly provisioned accounts have one accepted set of default definitions with interpretable stamps; repeated sign-in/enrollment does not reseed them. Assert public record values, ownership and head coverage, not SQLx migration internals.
- [x] Implement common relational payload families from the client schema, adapting PostgreSQL types and checked SQL. Store complete atomic maps/configurations with existing native Serde where agreed; keep queryable facts relational. Reuse native constructors rather than copying their validation rules.
- [x] Persist accepted heads/envelopes, vectors, baselines, lineage, installed coverage, journal positions/stamps and explicit retained-journal boundaries. Keep original acceptance positions distinct from installation positions. Retain exact service support versions independently of heads.
- [x] Extend the existing `sync_accounts`, `sync_baselines` and membership representation. Replace enrollment's head-only coverage reconstruction and hard-coded absent acknowledgments with actual persisted values. Reuse the account lock and authority; do not create duplicate account or membership state.
- [x] Update explicit runtime grants, including required sequence privileges where identity columns are used. Keep schema changes release-managed.
- [x] Run production migrations/grants against a disposable PostgreSQL database, generate server offline SQL metadata, then run the account scenarios under the runtime role. Commit mappings and their real initialization consumer together.

## Task 4: Supply complete discovery from real stores

**Files:** Both `discovery.rs` modules, backend query folders and named rows; core `transfer/discovery.rs`; new client `tests/discovery.rs` and server store synchronization scenarios.

**Interfaces:** Each concrete adapter supplies `start` and `page` operations with the shared start/page/end values from Task 2. Backend-private cursor state is immutable journal position or `(record kind, ID)`, never mutable contact fields. Exact accepted-version/support lookup accepts native key/stamp references. The client may advertise no usable journal and use the same fuller-state path; do not create a second client journal merely for symmetry.

- [x] Add a real-source scenario that captures target 100, reads A92, then uses ordinary accepted operations to create A101, B102, C's deletion at 103 and D104 behind the cursor. Assert complete obligations through 100, qualifying B/C successors, and no coverage credit for omitted A101/D104.
- [x] Implement journal enumeration over the requested coverage difference through the fixed target, using immutable installation positions. Account for all covered timelines; lexicographic paging order is enumeration order, not causal order. Read current qualifying heads/support without filtering them to `latest <= target`.
- [x] Implement fuller-state traversal of accepted identities, tombstones and required retained evidence. Never enumerate the working-history projection or use pending originals as accepted source values. Values and support in each bounded page come from a consistent short read transaction: a PostgreSQL read-only repeatable-read transaction or an ordinary SQLite read snapshot. End the transaction before awaiting network delivery.
- [x] Check retained-journal eligibility and target interpretation at page reads and source completion. If the interval becomes unavailable, abandon incremental completion and restart fuller-state discovery while retaining received facts. Keep identities/deletion evidence required for the full traversal enumerable; no physical tombstone collection is added here.
- [x] Read the final empty range and certify discovery end using the same validity checks. Post-target insertions behind a cursor do not invalidate the target; loss of pre-target identity/evidence does. Metadata-only changes and source history discontinuity cannot be silently omitted.
- [x] Add assertions to the same bounded scenarios for an unavailable journal selecting full traversal and concurrent page changes not hiding required records. Use ordinary accepted writes and supported retention state, not test triggers or production pause hooks.
- [x] Regenerate both backends' query metadata serially, run focused store scenarios and commit the producers with shared eligibility rules.

## Task 5: Persist incremental SQLite receipt and completion

**Files:** Client `transfer.rs`, focused `transfer/` children as needed, `metadata/transfer.rs`, original/lineage helpers, sync migration/query files, `retention.rs`; existing `tests/transfers/`, `tests/recovery.rs`, integration module list.

**Interfaces:** `transfer::begin` returns connection-owned `ReceiptState`; `install_page` commits actual data before advancing it; `complete` consumes the matching discovery end and verifies actual durable rows before saving coverage. Semantic staging/support persists independently across attempts. See [the simplification plan](2026-10-31-synchronization-simplification.md) for the concrete interfaces.

- [x] Add a real SQLite scenario: install an incomplete page, close/reopen normally, begin a new receipt, reuse accepted/staged input, supply missing support and complete. Assert the prior checkpoint remains unchanged until all obligations are represented; assert actual pending/original values survive.
- [x] Commit interpretation, semantic staging/installation and client updates in one immediate transaction. Advance connection-owned requirements and sequence after commit; reconnect starts fresh discovery.
- [x] Preserve shared observation, local working values, genuine original, frozen proposal, retained accepted alternatives and enrollment holds through existing transitions. Refresh binding indexes atomically using current ownership checks.
- [x] At completion, verify matching source exhaustion, actual stored requirements/support, coupled units, interpretation and prior checkpoint. Persist coverage and affected pending comparison evidence together before acknowledgment.
- [x] For exhaustive initial receipt, establish certified absence for pending identities absent from enumeration. Complete the known-creation checkpoint from Task 1 without rewriting genuinely missing originals. For incremental receipt, rely on the previous complete view plus fully represented interval, not unqualified row absence.
- [x] Preserve staged inputs, exact dependencies and coupled descriptors independently of receipts; preserve service-evidence retention obligations.
- [x] Update existing transfer tests and crate README to the production interface, remove unused manifest-only storage/API paths, regenerate SQLite metadata, run client transfer/preparation/retention scenarios and commit.

## Task 6: Commit PostgreSQL acceptance, recovery and acknowledgments

**Files:** Server `store/synchronization.rs` and focused children, metadata/binding queries, core `acceptance.rs`, client review/rejection types; server synchronization tests and core acceptance/recovery scenarios.

**Interfaces:** Crate-private server operations:

- `accept_pending(&AccountStore, &AuthorizedClient, &PreparedChange) -> anyhow::Result<AcceptanceDecision>` returns committed native accepted/covered/stale/rejected results.
- `receipt::receive` owns the account-locked returned-page transaction. Its private `recovery::install_returned` helper consumes the caller's connection, authenticated carrier and original accepted envelope; it executes existing recovery planning inside the same transaction/savepoint. Only the transaction owner returns committed represented/staged/client-review results.
- `acknowledge` consumes authenticated actor and completed issued coverage; persists that actor's acknowledgment under current metadata/authority.
- Extend existing rejection/review enums for a tag deletion blocked by surviving active references. Use named native referenced/referrer identities, with bounded explanatory data.

- [x] Add real accepted-write assertions for frozen retry after a lost response, stale target/reference rejection and distinct already-accepted recovery. Assert no second journal acceptance on covered retry, and original stamp preservation on recovered installation.
- [x] Under the account lock, load current grant/actor authority and all inputs, invoke existing acceptance/recovery planners, enforce every emitted guard, and commit envelopes/heads/journal/account/index effects atomically. Return success only after commit; network waits stay outside transactions.
- [x] Add the fourth-QSO scenario using two actual client states: publish the three eligible QSO deletions independently; accept the fourth membership; reject only the tag deletion for its surviving reference. Check the inverse ordering: accepted tag deletion causes a later membership proposal to fail the forward-reference check.
- [x] Implement the reverse-reference rule against current accepted QSO membership. Make eligible local prerequisite publication possible before treating residual referrers as explicit review. Preserve required service coupling independently of tag deletion.
- [x] Install recovery interpretation/fork/recovered-coverage plans through current conditional transactions. Do not admit old pending work merely because its original author once had access; accepted facts carried by a currently authorized actor retain their different recovery status. Do not add a server conflict queue.
- [x] Persist acknowledgments for coverage actually offered/completed by that exchange and interpretable under retained lineage; never derive coverage from a maximum record stamp or overwrite another client's acknowledgment. Peer durability is asserted by its protocol acknowledgment, not remotely inspected storage.
- [x] Run core and PostgreSQL acceptance/recovery scenarios, check runtime grants and server metadata, then commit. Full operational garbage collection and rollback acceptance remain #16's consumer of this real persistence.

## Task 7: Serve authenticated WebSocket exchanges in the existing application

**Files:** Server synchronization modules, `http.rs`, `runtime.rs`, `oauth/resource.rs`, native token authorization fields as needed; workspace/server Cargo manifests and lockfile; server store synchronization/HTTP test modules.

**Interfaces:** `synchronization::router() -> Router<ServerState>` mounts `GET /api/sync`; connection handling consumes the existing `AuthorizedClient` and Task 2 messages. `ServerState` owns change notices, upgraded-task cancellation and draining. Store operations remain concrete from Tasks 3–6.

- [x] Add an actual route scenario that authenticates a native request, negotiates v1, exchanges discovery pages, durably acknowledges receipt and submits a proposal. Assert a cross-account message and incompatible protocol cannot mutate state; rejected scope returns Invalid, malformed input is not echoed, and revocation reports lost authority. Use existing native authorization fixtures, not a test-only auth bypass.
- [x] Enable Axum's standard `ws` feature. Use its upgrade/frames with bounded message handling, ordered page acknowledgment and connection-local exchange IDs. Reject malformed sequencing/account scope before dispatching mutations.
- [x] Reuse DPoP upgrade validation against the public HTTPS URI. For active work, check current grant/actor authority at admission/source boundaries and inside mutating transactions. Carry access expiry from the existing authorization result so the connection can close for normal refresh/reconnect.
- [x] Register for account change notices before discovering/reading state. Treat notices as wakeups, coalesce them, and recheck durable state on lag. Publish from settled owned application operations; no broker, outbox or database notification subsystem.
- [x] Dispatch common discovery and accepted-state recovery in either direction, carrying support in normal pages without an unused exact-version request exchange. Server recovery uses the same message contracts and concrete store operations; new pending edits still use server acceptance.
- [x] Tie upgraded tasks into the existing server shutdown lifetime. Stop admission, cancel network waits and drain admitted transactions before closing pools. Preserve in-flight transaction ownership if the peer drops its reply receiver.
- [x] Return safe, classified Invalid/Unauthorized/Unavailable failures from the socket-owning boundary; preserve ordinary cancellation, access expiry and reconnect behavior. Use typed request/source conditions, never error-string matching or arbitrary error-chain disclosure. Preserve the category in desktop diagnostics.
- [x] Verify revocation on an already-open socket through the real authority path, and failure after committed acceptance through ordinary connection close/reconnect. Assert committed state rather than WebSocket/Tokio internals. Commit the running endpoint and dependencies.

## Task 8: Connect the real desktop session and local workflows

**Files:** Desktop synchronization modules; `session.rs`, `session/settings.rs` and contact/logbook/settings children; `enrollment/{flow,completion}.rs`; existing status/view code; Cargo manifests/lockfile and focused session tests.

**Interfaces:** `synchronization::run` consumes the real `ClientStore`, enrolled binding, existing protected authorization, change notices, cancellation and nonsecret status sender. It is a session-owned task, not a public generic transport engine. Extract reusable authorization from `enrollment::flow` around Huskarl's `HttpAuthorizer`, token source/cache and signing key; enrollment and synchronization consume the same implementation.

- [x] Add one session integration scenario that starts from a real enrolled store, receives accepted state, saves local work while catch-up is active, publishes only after inbound completion and observes the committed local views. Assert the pending values/originals, not widget labels or framework lifecycle.
- [x] Add `tokio-tungstenite = 0.29.0` at workspace level with default features disabled and `connect`, `handshake`, `rustls-tls-webpki-roots`; inherit it in the desktop. Reuse the locked WebSocket family and standard Rustls validation. Add `futures-util` only for the stream/sink methods actually used. Do not enable native TLS/OpenSSL.
- [x] Build the HTTPS proof target and WSS connection request from the same validated origin. Obtain proof/access headers through Huskarl and attach them to the upgrade; no credentials in URLs, protocol labels or telemetry. Refresh/reconnect through existing protected credentials without enrollment prompts for ordinary expiration.
- [x] Start after an enrolled workspace opens or enrollment activates that workspace. Run history negotiation and complete inbound receipt before any pending proposal or frozen retry. Accepted-state repair toward an older server remains distinct from publishing new local work.
- [x] Enumerate actual pending records, prepare through existing durable store operations, publish eligible reference prerequisites before dependents, and apply committed server results through normal receipt/rejection. Ordinary unresolved records stay local and do not produce a busy retry loop.
- [x] Publish local-change notices from settled account-owned save/edit/delete operations, independently of editor reply ownership. Start another cycle for changes accepted or saved during the exchange; use capped reconnection delay with cancellation after transport failure.
- [x] Refresh actual contacts/settings/history through existing session update paths after committed receipt. Preserve active copied operating values and their selection/baseline rules. Present connection state/current error in existing status treatment, without an aggregate completion timestamp.
- [x] Verify graceful shutdown drains admitted local operations before store closure, and revocation/reauthorization preserves pending/frozen/original evidence. Follow desktop UI completion gates for any status presentation changes; native Windows acceptance stays explicit. Commit the actual application consumer.

## Task 9: Verify the integrated capability and reconcile documentation

**Files:** Extend the existing desktop native enrollment test host/support and server/store integration modules with synchronization scenarios; focused core/store cases from prior tasks; crate READMEs, `docs/synchronization-integration-proposal.md`, `docs/synchronization-model.md`, `docs/synchronization.md`, `docs/accounts.md`, README.

- [x] Run a production-path scenario with two real SQLite workspaces, the PostgreSQL runtime role and real authenticated WebSockets. Reuse the existing authorization host; do not build a parallel fake synchronization server. Assert independent edits converge, frozen retry has one effect, and reconnect uses journal catch-up or fuller-state fallback as appropriate.
- [x] Exercise the concurrent discovery example from Task 4 through the wire path, including page interruption and logging during catch-up. Assert complete-prefix coverage separately from individually newer received values and pending local work.
- [x] Exercise required service support/coupled effects and one returned-accepted recovery case through real stores. Missing support prevents completion while installed unrelated data remains durable. Confirm the implementation does not credit unknown/incomparable versions or relabel recovered acceptance as a new edit.
- [x] Integrate meaningful spans/events/metrics at discovery, committed receipt, acceptance, recovery and connection boundaries as each owner is built. In existing telemetry fixtures, assert truthful committed counts and useful info-level outcomes with lower-level spans filtered; verify intentional credential/payload exclusion. Avoid a separate copy of the functional suite for telemetry.
- [x] Review each changed/retained test against its Clacks requirement, detectable application mistake and cost. Combine assertions in the owning scenario; no generated trigger failures, SQLx machinery tests, generic round trips or coverage targets. Every production API/helper must have a real workflow consumer.
- [x] Regenerate/check query metadata serially against disposable production schemas: client metadata at workspace root and server metadata in `lib/clacks-server/.sqlx/`. Use the documented SQLx 0.9.0 commands and production grants; never use persistent development accounts for tests.
- [x] Run the verification commands below. Record actual results and unavailable platform/CI evidence; do not claim Windows or hosted CI passed when not run.
- [x] Update owning contracts/READMEs to the delivered interface and real limitations. Keep the removed timestamp and settled receive-first/reference rules reconciled. Do not leave superseded manifest-only guarantees or claim #15's review UI/#16's complete retention/recovery verification is delivered.
- [x] Perform a whole-branch correctness/idiomatic Rust review, including native type restoration, source completeness, transaction ownership, authorization, test value and telemetry. Address findings before requesting integration. Keep branch repairs in their coherent owning commits, with physically wrapped messages.

## Verification commands and delivery boundary

Ordinary checks use committed query metadata:

```sh
cargo fmt --all --check
SQLX_OFFLINE=true cargo test -p clacks-sync --all-targets --locked
SQLX_OFFLINE=true cargo test -p clacks-client-store --all-targets --locked
SQLX_OFFLINE=true cargo test -p clacks-server --lib --locked
SQLX_OFFLINE=true cargo test -p clacks-desktop --lib --locked
SQLX_OFFLINE=true cargo test --workspace --all-targets --locked
SQLX_OFFLINE=true cargo test --workspace --doc --locked
SQLX_OFFLINE=true cargo clippy --workspace --all-targets --locked -- -D warnings
SQLX_OFFLINE=true RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --document-private-items --locked
```

Database-backed commands require `CLACKS_TEST_DATABASE_URL` and
`CLACKS_TEST_RUNTIME_DATABASE_URL` pointing to the same disposable PostgreSQL
database with the documented roles/grants. Missing database configuration is not
a passing test. Build the production server/browser assets with the existing
`cargo leptos build` workflow as well; do not change provider registration or
machine-local setup to create a new worktree environment.

Completion requires the running desktop/server synchronization consumer, meaningful
failure/retry evidence, real checked SQL metadata, documentation and review—not
only new planners, tables or a standalone test harness. Pushing, opening a PR and
running hosted CI remain subsequent user-authorized actions; preserve the current
CI availability and Windows deferral.

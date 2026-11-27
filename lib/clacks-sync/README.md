# Clacks synchronization core

`clacks-sync` implements record vectors,
baseline-aware comparison, durable client working/prepared rows, conditional
acceptance, verified discovery completion, retained basis witnesses and
exceptional timeline recovery. Native payloads come from `clacks-model`.
The [synchronization contract](../../docs/synchronization.md) defines product behavior;
the [metadata design](../../docs/synchronization-model.md)
explains the representation and examples.

## Entry points and saved state

| Owning module | Production operations and purpose |
| --- | --- |
| `clock`, `record`, `account` | Typed record domains, checked writer counters, native envelopes and complete installed checkpoints |
| `lineage`, `compare` | Position-dependent ancestry, explained missing evidence and whole-record change attribution |
| `client`, `client::preparation` | Prepare/retry one frozen version, incorporate accepted own versions, retain later edits, resolve client review |
| `acceptance` | Check client admission or server-origin changes; return envelope, journal, account and reference guards |
| `transfer` | Accumulate discovery obligations, plan bounded batches and verify every version/deletion before checkpoint advance |
| `transfer::discovery`, `transfer::receipt` | Select eligible journal discovery, register ordered page obligations and verify the matching discovery end before complete-prefix advancement |
| `protocol::messages` | Versioned native WebSocket messages, with symmetric accepted-state transfer and committed proposal outcomes |
| `membership`, `compaction` | Close admission and preserve original-value witnesses and their causal interpretation |
| `recovery` | Detect witnessed regression, establish exceptional boundaries, return retained accepted state and reconcile compatible equal results |

A record vector describes one record, not receipt of the account. Positions have
no scalar ordering across timelines. Ancestry established at T2:41 does not apply
to T2:40. Incomparable accepted envelopes remain in a client-local retained set, separate
from the genuine pending original. Equal native values can prepare a context-only
proposal; receipt removes retained alternatives only when the new version proves
them represented. Explicit resolution retains their context for the resulting
proposal. This set holds unresolved alternatives, not a per-edit history.

Native equality and causality are separate: metadata bookkeeping alone
does not change recorded facts or request an external upload.

`VersionStamp::new` checks the accepted server component before exposing causal
context. `AcceptedRecord::new` binds native state to its record key. Deserialize
accepted envelopes through these same boundaries; their native wire fields remain
unchanged. A stamp is independently checked and can be replaced as a whole during
compaction, without changing the envelope's key or saved values.

Construct or restore local state through `ClientRecordFields::try_into` and frozen
proposals through `PreparedChangeFields::try_into`. Ordinary callers borrow the
saved state; complete edits use `ClientRecord::into_fields` and checked replacement.
Pending input belongs to the checked client record and cannot be mutated through
its saved view. Sealed `BasisWitness` restoration reestablishes same-version or
before ordering from its supplied lineage and baselines. These checks do not prove
that current account authority or transaction guards still hold.

`Lineage::initial_timeline` and `boundaries` expose the complete snapshot for
durable restoration without inferring its root from current heads. Native errors remain
concrete causes through `anyhow::Error`; metadata diagnostics use `thiserror`.
Expected review, retry, stale-state and admission outcomes are result data.

## Real transaction obligations

Later SQLite/PostgreSQL adapters and WebSocket sessions integrate this same
library. These obligations cannot be established by a pure row planner:

1. Load compared records, account metadata, membership and references within the
   actual transaction that saves the result. Recheck every emitted head, admission
   generation, target version and dependency condition. Re-evaluation against a
   newer head can produce a fresh plan; the old plan is no longer applicable.
2. Save a prepared client version and its counter reservation together before
   sending it. Later local editing changes working values, not frozen values.
   Save incorporated shared values and pending/original updates atomically.
3. Capture a fixed complete target, then discover obligations in bounded reads.
   Use an eligible real journal or full accepted-state traversal, including
   deletion descriptors. A newer proven successor can satisfy an older obligation
   without increasing the advertised complete target. Each attempt registers its
   own ordered pages; `ReceiptState` is connection-owned and requires the matching
   discovery end before completion. Reconnect starts a fresh receipt.
4. Load affected client records and dependencies in each batch's write transaction.
   Save native installation, comparison evidence, exact supporting facts and staged
   coupled effects. `plan_readiness` shares dependency decisions between adapters;
   the client batch planner applies working/original reconciliation. Staging
   does not establish complete coverage. Do not put older support versions in
   client conflict alternatives.
5. Verify completion against committed installed rows and baseline/lineage metadata.
   Save checkpoint advancement after checking the actual previous checkpoint in its transaction.
   Reload the committed checkpoint before acknowledgment or recovered-coverage
   publication. A proposed update, advertised revision or high record counter is
   not durable completion evidence.
6. Baseline publication saves the new interpretation, projected envelopes and
   revised account head under the observed membership/head and touched-version
   guards. Only afterward remove redundant detail. Preserve references named by
   surviving contexts; a client preserves original values and witnesses before
   releasing older evidence and acknowledging the newer checkpoint. `PendingRecord.witness`
   persists the sealed relationship with the baseline definitions it needs;
   receipt and preparation consume that private evidence after shared detail is
   collected. An own-version witness only applies where current context covers
   its actual witnessed version.
7. Recovery admission authenticates the currently returning client. Already
   server-distributed accepted state is distinct from a new proposal by its
   original author. Recheck the returned installation conditions atomically.
   Recovery uses `AcceptanceInput` so real native reference observations and their
   guards are available for recovered installations and context joins. Missing
   references remain explicit, without turning identical tagged records into a
   field conflict. A retained installation preserves its acceptance stamp; `Reconciled` instead
   identifies a new conditional acceptance covering both approved contexts.

The adapters also own authentication, account isolation in queries, persistent
uniqueness, enrollment, journal retention, concrete schema/protocol negotiation
and atomic persistence of coupled workflow operations. The service modules below
provide evidence interpretation and dependency rules without treating provider
contradictions as competing recorded-QSO edits. Consuming workflows own service
execution and maintenance transactions.

`compaction::installation` computes the expanded common durable coverage of
authorized clients and plans equivalent current-context conversions. The server
consumes this result in its account-locked atomic installation; the core does
not perform database cleanup. An unacknowledged authorized client prevents
advancement beyond the existing baseline. No-work plans create no new revision.

## Verification and limits

Tests call production operations with native QSO, tag, profile and group values.
Serialization/reload checks preserve pre- and post-commit row representations;
they do not claim SQLite or PostgreSQL crash atomicity. Storage and transport
integration must test actual commits and interruptions on those systems.

Covered own versions survive lost responses. Partial windows cannot certify full
prefixes. Indefinitely offline authorized clients retain their acknowledgment
obligations; inactivity never retires them. Future collection must preserve native
values, deletions and actual acceptance positions. Missing interpretation yields explained
unknown ordering rather than invented concurrency.

Regression detection is deliberately limited. A restored writable server that
has advanced beyond every available witness can escape a revision comparison.
Timeline ancestry distinguishes histories after a fork is established; it is not
a universal rollback detector. Ordinary restarts preserve identities. Concurrent
writable copies of a desktop identity remain outside this library's guarantees.

## Validation

Use Rust 1.99.1 and the committed lockfile. Run the [workspace verification
commands](../../README.md#development) from the repository root. Windows target
checks establish compilation, not Windows runtime or database durability.

## Service processing and retention

`service::dependency::check` checks durable semantic support separately from
write conditions. `service::combination` validates immutable identity and selects
provider fields using their own timestamp scope and original acquisition context.
Response arrival time does not establish freshness.

`service::association` classifies candidates and binding conditions. Actual
external-service workers/importers must apply related QSO, disposition and
binding effects atomically using ordinary acceptance and source guards. Their
execution and job coordination are integration obligations of those consuming
workflows, not an unconsumed generic operation planner in this crate.

Transfer receipts are connection-owned. `register_page` checks ordered input;
`finish_receipt` checks the matching source end and actual represented versions
and interpretation before complete-prefix advancement. Semantic staging and
coupled descriptors persist independently, so reconnect can start fresh without
losing accepted input. Current heads and exact historical support have separate
roles. An unresolved recovered alternative cannot certify accepted server coverage.

`transfer::readiness` consumes accepted candidates, current heads, exact support
and coupled descriptors without manufacturing client records. Client batch
planning applies actual working-record reconciliation afterward. Server recovery
uses its real account transaction, conditional acceptance and coupled-group
savepoints; no client updates are generated and discarded by that path.

Operational failures describe incompatibility, invalid input, lost authority and
source/storage unavailability. Fresh negotiation handles an unavailable journal.
There is no on-demand exact-version protocol, last-page retransmission cache or
persistent receipt identity.

Basis witnesses and expanded coverage interpretation are integrated with client
receipt and comparison. Common-baseline compaction and physical collection remain
maintenance workflow obligations. The oldest authorized client's acknowledged
coverage bounds collection; pending originals, deletion memory and service evidence
retain their independent obligations. No unconsumed collector or compaction planner
is shipped as a substitute for that integration.

## Sync contract publication

`protocol::contracts::SyncContract` enumerates implemented contracts. Its `id()`
operation defines each public identifier once. Desktop offers and generated
`deploy/desktop/sync-contracts.json` both enumerate these variants; the server's
ordered acceptance policy determines live selection independently.

When changing implemented support, regenerate and commit the publication copy:

```sh
cargo run --bin clacks-desktop-package --locked --offline -- sync-contracts --output deploy/desktop/sync-contracts.json
cargo test -p clacks-desktop-packaging --locked --offline
```

The normal packaging test compares shared in-memory generation with the on-disk
file without modifying it. Live exchange dispatch exhaustively matches the
selected contract; the JSON file is publication output, not executable input.

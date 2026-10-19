# Clacks synchronization core

`clacks-sync` implements record vectors,
baseline-aware comparison, durable client working/prepared rows, conditional
acceptance, complete transfer windows, membership-bounded compaction and
exceptional timeline recovery. Native payloads come from `clacks-model`.
The [synchronization contract](../../docs/synchronization.md) defines product behavior;
the [metadata design](../../docs/synchronization-model.md)
explains the representation and examples.

## Entry points and saved state

| Owning module | Production operations and purpose |
| --- | --- |
| `clock`, `record`, `account` | Typed record domains, checked writer counters, native envelopes and complete installed checkpoints |
| `lineage`, `compare` | Position-dependent ancestry, explained missing evidence and whole-record change attribution |
| `client` | Prepare/retry one frozen version, incorporate accepted own versions, retain later edits, resolve client review |
| `acceptance` | Check client admission or server-origin changes; return envelope, journal, account and reference guards |
| `transfer` | Validate immutable manifests, plan bounded batches and verify every version/deletion before checkpoint advance |
| `membership`, `compaction` | Close admission, select common authorized coverage, preserve original-value witnesses and project eligible closed writers |
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

Composite saved rows expose fields for ordinary Rust serialization. Call the
transition operation before accepting deserialized rows. Native errors remain
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
3. Enumerate a manifest from one consistent source view. Full enumeration includes
   deletion descriptors; incremental enumeration uses an eligible real journal.
   Store the manifest immutably under its source checkpoint and mode. Reject an
   unequal replacement with that same identity by comparing the existing manifest;
   source identity alone cannot detect a changed list the planner was never given.
4. Load client rows within each batch's real write transaction, then save accepted
   values, comparison evidence, appended installed keys and progress generation
   together. The batch guard prevents stale progress writes. It does not turn an
   earlier read of client rows into a safe later write.
5. Verify completion against committed installed rows and baseline/lineage metadata.
   Save checkpoint advancement under the previous checkpoint and progress revision.
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
and atomic coupled workflow operations. Service evidence arrives in issue #3;
this library does not force provider facts through recorded-QSO conflict policy.

## Verification and limits

Tests call production operations with native QSO, tag, profile and group values.
Serialization/reload checks preserve pre- and post-commit row representations;
they do not claim SQLite or PostgreSQL crash atomicity. Storage and transport
integration must test actual commits and interruptions on those systems.

Covered own versions survive lost responses. Partial windows cannot certify full
prefixes. Indefinitely offline authorized clients retain their acknowledgment
obligations; inactivity never retires them. Projection preserves native values,
deletions and actual acceptance positions. Missing interpretation yields explained
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

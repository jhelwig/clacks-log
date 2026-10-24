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

Composite saved rows expose fields for ordinary Rust serialization.
`ClientRecord::validate` checks restored native values, record keys, frozen
conditions and retained witness structure. Causal comparison still requires
the actual retained interpretation and the relevant transition operation.
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
and atomic persistence of coupled workflow operations. The service modules below
provide their guarded transition plans without treating provider contradictions
as competing recorded-QSO edits.

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

## Service processing and retention

`service::dependency::check` checks durable semantic support separately from
write conditions. `service::combination` validates immutable identity and selects
provider fields using their own timestamp scope and original acquisition context.
Response arrival time does not establish freshness.

`service::association` classifies candidates across an account and constructs
unique-index guards. `service::operation::plan_operation` returns one
`CoupledCommit` for related native/evidence changes, using ordinary acceptance
and checked revisions. Recheck its original head, membership generation,
source-version conditions and every external-index condition in the same real
transaction. Commit all accepted rows, journal rows, unit descriptors and actual
worker job updates atomically. Client proposals retain their author/vector and
original transaction guards while resolving same-operation dependencies against
prospective accepted versions. Adding an unmatched downloaded contact requires a
selected live tag; this does not prohibit untagged contacts in other workflows.
An observed absent external-index entry requires
protection against a competing insert, not just a missing row lock.

`service::download` serializes starts through one deterministic control row per
destination and scopes progress to stream, filters and selected fields. The worker
must supply complete parsed-response accounting, retain every input or its
unchanged represented equivalent with processing state, and advance watermarks
only after all requested pages/windows succeed. A page cursor is not a permanent
change watermark. Close failures explicitly; late results keep their original
request identity and cannot take a later request's control/progress.

Transfer adapters persist the immutable manifest once and call
`WindowManifest::validate_replacement` if that identity already exists. Persist
`InstallBatch.staged` and `completed_units` with its progress generation, alongside
accepted/client updates and installed keys. Staging does not acknowledge a full
prefix. A complete unit may span transport messages; its effects install together.
A later accepted successor/deletion can represent an old effect without restoring
its old body. Preserve client pending originals and working values on receipt.

`service::recovery::plan_recovered_operation` authenticates the returning carrier
separately from original authors and retains original acceptance stamps. Keep
incomplete input staged for retry, restore required interpretation, and use
existing client review for maintained binding conflicts. Recovery does not
schedule or dispatch a service request.

Before collection, obtain `service::retention::requirements` from the complete
relevant row/unit view and enforce `validate_collection` alongside interpretation
checks. Distinct semantic facts have no time-based expiry. Only unneeded closed
acquisition coordination is removable. `compaction::plan_baseline` projects
embedded service support as well as envelope context when its meaning is
established; unresolved interpretations remain required. Include surviving unit,
window and other external contexts in `CompactionInput.surviving` until equivalent
summaries cover them. Preserve old baselines when a valid replacement is absent.
Acknowledgment permits redundant sync-history collection, not deletion of
submission/confirmation history or deletion memory.

All new service destinations require a complete `DestinationRegistryObservation`
at the compared account head. The real transaction must preserve a single stable
namespace per remote collection, retain it after credential removal, and register
new public descriptors atomically with acceptance. Generic acceptance and recovery
perform the same namespace checks as coupled operations. A destination ID cannot
be repurposed for a different remote collection. A QRZ destination may refine an
unknown book ID to its established ID under the same namespace and guards.
Established IDs cannot change or be cleared, and older evidence with an unknown
book ID remains interpretable. Refinements and their pending prerequisites take
priority over unrelated members, and each accepted refinement updates the
operation's prospective registry. Supply the complete registry in
`ServiceOperationInput.destinations` and `DownloadContext.destinations`; recovery
accepts its explicit guarded registry observation.

`RecordState::same_values` is the whole-record value-policy comparison. Structural
`Eq` also compares clock representation. Baseline-aware record comparison proves
that projected support contexts remain equivalent, so metadata projection does
not create a recorded edit or manual conflict. Covered historical bindings need
not pass conditions for reinstalling their old active state.

`SelectedObservations.uncertain` identifies fields needing provider refresh;
workers must not treat contradictory provider ordering as a manual QSO edit.
Selection retains conflicting values from maximal mutually reachable ordering
components. A later refresh that dominates them can establish a value again.

`SelectedObservations.current_supporting` identifies occurrences whose supplied
fields are all established and undominated, while `supporting` includes the complete
retained input used by selection. Ordering-only refreshes reuse a disposition
only when its reviewed occurrence represents the current facts; an older
rejection of different content cannot suppress new review work.

## Observability

The [production observability contract](../../docs/observability.md) applies to
this library. Meaningful operations use `#[tracing::instrument(skip_all, ...)]`
with explicit safe input fields and default module-path targets. Standard
subscribers/exporters supplied by application composition own lifecycle
presentation, context and span timing. Instrumentation preserves normal function
bodies, early returns and error propagation.

`record::RecordId` implements `Display` using the contained native ID's canonical text.
No telemetry-specific wrapper is needed. Annotation fields use constants or
simple input/field access; derived context is reported where domain code resolves it.

Local events and metrics report domain decisions and actual produced plans:
readiness, proposed/staged work, retry/review reasons and relevant quantities.
They do not report generic function completion or duplicate span timing. Metric
families have their own coherent bounded dimensions; IDs remain trace fields. The
model counts projection-readiness decisions; sync counts acceptance decisions
and ready/staged transfer plans. These include repeated or tentative calculations.
Shared export planning, compaction and coupled service planning report useful
quantities through events, without successful-call-only counters.
A propagated error produces no fabricated plan/proposal result. Error handling
and overall workflow success/error/latency measurements belong to their owning
runtime boundary; span timing does not automatically populate metric histograms.

Filtering cannot gate domain work or metrics, and domain events do not mutate
caller spans. The library installs no subscriber, recorder or exporter. Returned
values, concrete errors and synchronized records are independent of configuration.

Significant recovery and explicit conflict-resolution operations use `info`;
witnessed server-history regression uses `warn`. Routine planning detail remains
`debug` or `trace`. These levels describe the significance of the attempted
operation or reported decision, without asserting a durable commit. See the
[level-selection policy](../../docs/observability.md#level-selection-policy).

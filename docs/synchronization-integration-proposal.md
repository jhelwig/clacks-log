# Account synchronization integration proposal (#14)

Proposal for review for [issue #14](https://github.com/jhelwig/clacks-log/issues/14).
This document maps the existing production components into the running
client/server workflow. It does not authorize implementation or make the
engineering defaults below agreed product decisions.

The [implementation plan](superpowers/plans/2026-10-30-account-synchronization.md)
maps this proposal to production changes and verification tasks.

The [synchronization contract](synchronization.md), [metadata model](synchronization-model.md),
[client-store contract](client-store.md), [enrollment contract](client-enrollment-proposal.md)
and [service-evidence contract](service-evidence.md) remain authoritative.
SQL means Structured Query Language; SQLx is the selected Rust database library.
Serde is the existing Rust serialization library; JSON is its text-based wire
encoding here. Tokio supplies asynchronous tasks and channels; Axum supplies
the HTTP server and WebSocket upgrade. Huskarl supplies the existing native
authorization client.
Demonstrating Proof of Possession (DPoP) is the existing credential-binding
mechanism. A prefix is all relevant accepted history through a timeline position.

## 1. Settled scope

- One account scope, with all account-owned contacts, tags, shared definitions
  and modeled service evidence replicated to every enrolled client. Current
  display selection does not restrict synchronization.
- The existing Axum application and encrypted WebSockets; PostgreSQL on the
  server and SQLite on the desktop, both accessed through SQLx.
- Bounded journal catch-up, with automatic fuller-state reconciliation when
  the required journal history is unavailable. These are complementary paths.
- Existing record vectors, server acceptance components, baseline interpretation,
  exceptional timeline ancestry and authorized-client retention rules.
- Receive-first client catch-up: incorporate the incoming shared state before
  publishing any pending local changes or retrying frozen proposals. Logging
  and local saves continue. Work arising during an exchange is handled by
  subsequent synchronization cycles.
- Client-side whole-record comparison. Conflicts and invalid references stay
  local; other eligible records can progress. No server conflict queue or
  automatic field/tag merge.
- Validate references in both directions. A tag deletion is blocked if an active
  QSO would still reference it in resulting accepted state. This is reference
  validation, not a competing tag edit. Recheck at shared acceptance.
- Synchronize resulting record states. Do not infer or preserve a delete-all
  action from coincident pending deletions. Local atomic tag deletion does not
  require an indivisible synchronized action group.
- Preserve explicitly coupled service effects under their existing rules.
- Preserve pending/original/prepared values and interpretation before advancing
  durable coverage. Sending a frame or observing a high record revision does not
  establish complete receipt.
- Connection state and current error are diagnostics. No overall successful-sync
  timestamp or replacement aggregate currentness indicator.
- Server-only secrets remain server-only. This scope does not authorize copying
  credential material beyond the existing purpose-specific distribution rules.

Issue #15 supplies the account-wide resolution UI. Issue #16 completes integrated
retention, compaction and indefinite-return/rollback verification. Their later
work extends the actual persistence and exchange implemented here. Directory and
external-provider workflows retain their existing later capability boundaries.

## 2. Existing components to reuse

| Existing owner | Reused responsibility | Integration work |
| --- | --- | --- |
| `clacks-model` | Checked native values, reference groups, defaults, service facts and exact external keys | No parallel synchronized domain schema |
| `clacks-sync::client` | Preparation, frozen retry, receipt, comparison and review inputs | Invoke from the ordinary cycle using current durable observations |
| `clacks-sync::acceptance` | Conditional client/server acceptance and journal instructions | Execute against real PostgreSQL rows and current authorization |
| `clacks-sync::{account,lineage,membership,recovery,compaction}` | Coverage, actor lifecycle, recovery and interpretation plans | Persist the existing values and enforce their transaction guards |
| `clacks-sync::service` | Evidence dependencies, associations, coupled effects and retention meaning | Use the same accepted-state path and qualified ownership |
| Client `payload`, `rows`, `metadata` | Relational payloads and role-qualified causal storage | Adapt common families to PostgreSQL; retain SQLite client roles |
| Client `synchronization`, `transfer`, `retention`, `read` | Durable preparation/receipt, staging, interpretation, pending paging and checkpoints | Add accepted-state source reads and connect actual exchanges |
| Server enrollment/OAuth paths | Account identity, actor/grant authority, proof validation and account locks | Extend existing native-resource routes and transaction loading |
| Desktop `session`, enrollment and settings workers | Owned runtime, protected credentials, cancellation and committed-result presentation | Add the synchronization task and normal event wiring |

### Interfaces and ownership

Use two concrete storage implementations around one shared synchronization core.
The existing client storage functions accept `SqliteConnection` and use SQLite
query files. Their data model carries over; the functions themselves are not
callable against PostgreSQL. The interfaces are purpose-specific Rust
operations, not a common repository trait.

| Owner | Interface responsibility |
| --- | --- |
| `clacks-model` | Native values, validity, interpretation and existing serialization |
| `clacks-sync` | Causal comparison, preparation, acceptance, recovery, receipt/completion rules and shared protocol values |
| `clacks-client-store` | Execute client transitions atomically in SQLite; retain working/original/prepared state; supply accepted-state discovery |
| `clacks-server::store` | Execute server transitions atomically in PostgreSQL; persist accepted state, journal, membership and acknowledgments; supply accepted-state discovery |
| Desktop synchronization task | Drive receive-first synchronization, coordinate transport and local work, and refresh application views |
| Server WebSocket handling | Authenticate connections, dispatch protocol requests to concrete store operations, and deliver results/change notices |

The shared core decides what a transition means and which conditions must hold.
Each storage implementation loads those conditions and commits the corresponding
changes. Transport does neither.

### Module organization

The concrete server store and transport use the following modules:

```text
lib/clacks-server/
  src/
    synchronization.rs          # WebSocket exchange handling
    store.rs                    # Existing concrete PostgreSQL store
    store/
      synchronization.rs        # Transaction-owning write operations
      discovery.rs              # Accepted-state and journal reads
      payload.rs                # Native values ↔ relational payloads
      metadata.rs               # Stamps, coverage, lineage, membership
      rows.rs                   # Private named PostgreSQL query results
  sql/
    synchronization/
    discovery/
    payload/
    metadata/
```

These modules integrate with the existing account/enrollment operations; they do
not introduce another pool or membership authority. Payload and metadata helpers
remain available to other server workflows that consume the same stored values.

The client retains its existing organization: `synchronization` owns transactional
preparation, receipt and resolution; `transfer` owns durable receipt completion;
private `payload`, `metadata` and `rows` modules own persistence mapping. Add
accepted-state discovery reads without using the working-history projection.

### Transaction-owning operations

Concrete store operations own their transactions and return outcomes only after
required writes commit. For example, server acceptance follows this sequence:

1. The WebSocket handler supplies the authenticated actor and native
   `PreparedChange`.
2. The PostgreSQL operation starts a transaction and takes the existing account
   lock.
3. It loads current authority, account metadata, target state, references and
   external bindings from that transaction.
4. It calls `clacks-sync::acceptance::plan_acceptance` and enforces the resulting
   conditions, including the reverse-reference check required by section 4.
5. For acceptance, it persists the envelope, accepted-head pointer, journal,
   account update and required index effects together.
6. It commits before returning acceptance success.

An `AcceptanceCommit` is an internal plan consumed within that operation, not a
public handler contract for a later, separately coordinated persistence call.
Private payload/metadata helpers accept the caller's `&mut PgConnection`; they
neither start nor commit independent transactions. SQLite helpers retain the
equivalent caller-owned connection pattern.

Client operations own their different atomic boundaries: preparation saves frozen
values and counter reservations before transmission; batch receipt saves
installed/staged input and pending-state changes together; completion verifies
obligations and commits coverage before acknowledgment. Section 4 lists the
corresponding server and recovery boundaries.

### Discovery and shared conversions

Both adapters expose native accepted state through operations to read advertised
coverage/history availability, enumerate a bounded journal or fuller-state page,
and load exact accepted versions and required support. Outputs use shared
discovery/protocol types. SQL queries, database cursors and row decoding remain
private to each backend. Discovery-path selection and coverage-completion rules
remain shared synchronization policy; backend-specific reads implement those
rules rather than define different ones.

Keep backend-specific checked SQL and transaction execution. Both implementations
reuse native constructors and Serde representations; database-specific row
assembly stays local. If substantial identical, database-independent conversion
code emerges, share that specific code after identifying its consumers. Do not
extract every mapper into a new library or introduce a generic repository,
database abstraction framework or second synchronization engine merely to make
the adapters look identical.

The concrete starting points are the core's
[acceptance](../lib/clacks-sync/src/acceptance.rs) and
[transfer](../lib/clacks-sync/src/transfer.rs) planners, the client's
[save path](../lib/clacks-client-store/src/save.rs) and
[receipt adapter](../lib/clacks-client-store/src/transfer.rs), and the server's
[membership persistence](../lib/clacks-server/src/enrollment/membership.rs) and
[resource authorization](../lib/clacks-server/src/oauth/resource.rs).

## 3. Server persistence adaptation

### Shared families

Adapt the client's common relational families into the existing `clacks` schema:

- Account-qualified `record_identity` and immutable `payload` roots, including
  explicit deletion. Use the same QSO/station/location/reference/equipment child
  arrangement and native units.
- Tag, profile, equipment, group and output-profile payload families. Keep
  complete atomic configuration values serialized with ordinary native Serde
  where already selected; preserve queryable facts relationally.
- Service payload/support families and the accepted external-binding index.
  Preserve exact key equality, account/destination qualification and retention
  of immutable evidence.
- Record vectors, accepted stamps/envelopes, coverage, baseline and lineage
  families. Preserve actual acceptance positions during recovered installation.

A server `accepted_head` relation points each account-qualified record identity
to its current accepted envelope. It replaces the client's role of selecting a
latest observation; it does not require the client's pending, original, prepared,
review or operating-settings tables on the server. Required service support can
retain exact older envelopes independently of the head.

Keep the existing physical encoding rules: native IDs as canonical text,
Clacks-controlled counters and whole-hertz/milliwatt values within signed
64-bit storage, and complete atomic maps/configuration/encrypted values as
already agreed. Rust owns intrinsic enum/variant/domain validity. Database keys,
ownership, nullability and uniqueness retain relational integrity.

### Existing server metadata

Extend `sync_accounts`, `sync_baselines` and `desktop_clients`, rather than
creating another account/membership authority. The current enrollment reader's
`Coverage::from(head)` and `acknowledged: None` are insufficient for ongoing
synchronization. Load real installed coverage, full retained lineage and actual
per-client acknowledgments through the existing native types. Enrollment and
revocation must continue updating this same state.

Add an account/timeline-qualified installation journal with installation
position, record key and the installed native stamp. Its position is distinct
from the stamp's original acceptance position. It need not retain historical
payloads. Retain referenced stamp interpretation while journal entries need it.
Record the retained-journal boundary explicitly: an empty query result alone
cannot certify that an older requester has no missing changes.

Membership/history metadata changes share the account history. Journal discovery
must not assume that every numeric revision has a record-change row; account
metadata must also be conveyed. Counter allocation remains transactional and
checked through the existing account transition rules.

Current accepted identities, tombstones and required service evidence remain
enumerable. Reclaiming unreferenced payloads is distinct from collecting that
semantic evidence. #16 may extend collection only while preserving discovery
and recovery contracts.

### Initialization

Use `clacks-model::defaults::account_definitions` and ordinary server acceptance
to initialize a new account's shared definitions once. Extend the existing
account-provisioning transaction with initial synchronization metadata and seed
acceptance. An enrolled desktop receives those accepted definitions; it does not
independently seed another copy. Standalone local accounts retain their existing
local definition initialization.

## 4. Purpose-specific transactions

| Operation | Inputs checked inside the transaction | Durable result |
| --- | --- | --- |
| Accept pending proposal | Existing account lock; current actor/grant authority; target and required references; reverse references for deletion; external ownership; baseline/lineage | Native accepted envelope, head, account revision, journal and required index effects together |
| Install returned accepted state | Current returning actor authority and existing recovery-plan conditions | Original accepted stamp preserved, local installation journal position and updated history state |
| Incorporate client receipt | Actual current client roles and required interpretation/dependencies | Accepted state, preserved pending basis, review/staging and receipt progress together |
| Complete incoming coverage | Completed discovery, all represented requirements and coupled effects, prior checkpoint | New checkpoint and required pending comparison evidence; reload committed result before acknowledgment |
| Record client acknowledgment | Bound actor, retained interpretation and previously established transfer completion | That actor's durable acknowledged coverage, without changing other clients |
| Publish recovery coverage | Existing recovery-plan guards and durably incorporated accepted state | Position-dependent lineage boundary and account coverage together |

Keep transactions bounded and exclude network waits and user interaction. Reuse
the account lock already ordering enrollment/revocation. Different accounts do
not share that lock. No custom write queue is selected.

For reverse-reference validation, query current accepted QSO membership, not
historical payload references. Apply the same rule to accepted profile references
when deleting equipment. The client performs the corresponding check after
catch-up; the server rechecks it under the same acceptance transaction as the
deletion. Existing historical/original references do not block deletion merely
because their payloads remain retained.

Add a typed blocked-deletion result and matching local review reason to the
existing rejection/review path. Carry the referenced identity and enough
referrer information to explain the issue. Do not label it missing-original,
ordinary competing tag values, or a service-only error. #15 consumes the same
saved review state; no separate conflict subsystem is needed.

Order eligible proposals by actual reference prerequisites: create a referenced
tag/equipment definition before a record using it; publish eligible membership
removals/QSO deletions before retrying the affected tag deletion, and profile
reassignments before equipment deletion. Pending local
effects are not yet shared effects. A deletion waiting for its own eligible
prerequisites is not, by itself, a request for user resolution. A surviving
reference after those effects, such as the fourth QSO added by another client,
is the explicit-resolution case. The server still rechecks actual accepted
references; this ordering grants no atomicity to the group.

## 5. Native exchange contract

**Engineering default:** extend `clacks-sync::protocol` with ordinary Serde wire
values, using JSON text messages on `/api/sync`. Keep native envelopes, proposals,
checkpoints, baselines and lineage values; do not duplicate their domain schema.
Start with protocol version 1 and explicit incompatible-version rejection.
Compatibility is a protocol contract, not equality of application version strings.

The message families are:

| Family | Contents/purpose |
| --- | --- |
| State introduction | Protocol version, real installed coverage and retained history observations needed for negotiation |
| Discovery request | Receiver coverage and incremental/full-state request within the authorized account |
| Discovery start/page/end | Source coverage target, mode, bounded accepted descriptors/data, required interpretation and explicit enumeration completion |
| Accepted-state recovery | Previously accepted values and supporting versions returned toward the server, separately from new proposals |
| Pending proposal/result | Existing `PreparedChange` and accepted/covered/stale/rejected outcomes |
| Durable acknowledgment | Committed incorporated coverage for the identified exchange |
| Protocol failure | Actionable compatibility, malformed-input or operational failure without secret/payload dumps |

Use connection-local request correlation. A fresh connection negotiates from
durable state; it need not resurrect the old source session. Transport request
numbers are not causal identities, and do not replace frozen proposal identity.
Source completion is not a durable acknowledgment. A proposal result is not an
account-prefix acknowledgment.

The handshake binds one account and actor from verified credentials. Reject
account-qualified payloads outside that binding. Neither display selection nor
an account ID supplied in a message grants authority.

## 6. Normal synchronization cycle

1. Start when an enrolled workspace opens or enrollment completes for that active
   workspace. Reuse the protected credential reference and Huskarl refresh/proof
   facilities. An unenrolled workspace remains fully usable locally.
2. Authenticate, check compatibility and compare actual history/coverage.
3. Finish inbound catch-up and durable receipt before submitting any pending
   local proposal, including frozen retries. Accepted-state repair toward an
   older server remains distinct from publishing new local work.
4. Reconcile pending records against the newly incorporated shared observations.
   Prepare eligible records through the existing store, preserving frozen bytes
   and reserved counters before transmission. A known accepted own version may
   already settle a lost response.
5. Apply server accepted/covered/stale/rejected results through the existing
   receipt/rejection transitions. Reconsider stale state using normal comparison;
   do not blindly resend a changed proposal with its former identity.
6. Reassess current durable state and work arising during the exchange. Continue
   another cycle while eligible work remains. Otherwise wait for application
   events. Unresolved review alone must not produce a busy retry loop.

Catch-up targets describe finite committed coverage. Changes accepted after that
target are included in later delivery and remain subject to conditional
acceptance; synchronization does not require other writers to become quiescent.
Nothing here adds a manual sync requirement or a global completion timestamp.

### Creation before the first receipt

New records saved during initial catch-up use the next ordinary cycle. The
current save path assigns `OriginalBasis::Unavailable(MissingOriginal)` when no
checkpoint exists, even for a newly allocated record. `review_reason` then keeps
that original in review after receipt. Updating only `latest` does not fix this.

**Proposed engineering default:** extend the existing `OriginalBasis::NeverCreated`
to retain an optional certified checkpoint. A save creating a new local identity
with no existing client row records that known original absence; its checkpoint
is present if one exists, otherwise absent. This represents what the local save
actually knows. It does not assert certified shared absence or permit publication.
Keep `Unavailable` for genuinely unavailable originals. This is a proposed core
representation adjustment, not already implemented behavior or an additional
local-creation variant, timestamp or action journal.

After exhaustive first receipt, update current observations for pending identities
not encountered in the source enumeration to `Observation::Absent` at that
completed checkpoint, in the completion transaction. For a known new creation,
retain that checkpoint in its original absence evidence before preparation.
If the source instead contains the identity, use normal whole-record comparison;
different creations under the same identity require resolution. An initial save
can never bypass the incoming receipt simply because its original is known.

An incremental receipt proves absence only where the preceding complete view
already established it and the interval is fully represented. An interrupted
full scan does not prove absence. Neither path converts a genuinely unavailable
original into a known creation. Frozen proposals, actor-exclusion holds and
retained accepted alternatives keep their current rules. Compaction must not
claim a certified absence witness for a creation whose checkpoint is still absent.

## 7. Concrete discovery proposal

The following is an **engineering default for review**, not a replacement
replication architecture or a result already established by the current code.

Use a source's committed coverage target H and existing native causal comparison.
Generate bounded pages, apply them durably and explicitly certify completion.
Do not hold a database transaction over network delivery or require a durable
source session. Each page reads its envelopes, support and interpretation
consistently through ordinary bounded database transactions.

### Journal path

For an eligible receiver prefix C, enumerate installation activity after C
through H, using immutable journal positions as the paging order. Supply current
accepted state that represents the recorded requirements; a newer qualifying
successor is acceptable. Repeated activity for one record can be represented by
one qualifying state. Do not filter current heads by `latest revision <= H`:
a later edit could otherwise hide an earlier required change.

Check retained-journal eligibility in the page read and again before declaring
completion. If collection overtakes the needed interval, stop claiming that
incremental exchange complete and use fuller-state reconciliation. This does
not revoke the client, erase received work or require a source-side retention
lease. Interpretation and ordinary deletion/evidence retention remain separate.

### Fuller-state path

Enumerate accepted identities by immutable `(record kind, native ID)`, including
tombstones and retained evidence. Compare native versions, then transfer only
missing/inadequately represented values. Source reads use accepted state, never
the working-history projection. Include retained accepted alternatives where
needed to represent advertised coverage; never substitute unpublished working
values or genuine original comparison snapshots for accepted shared state.

The completeness argument for this traversal requires:

- Every identity needed for H remains enumerable until visited, or has explicit
  equivalent retained deletion/evidence representation.
- Record identity does not move when its values or tags change.
- The state supplied for an identity represents its obligation through H, with
  sufficient interpretation. Unknown/divergent context is not asserted to be a
  successor.
- End-of-enumeration is explicit and all required input is durably represented.

A new identity created after H behind the paging cursor does not invalidate H;
an identity needed for H disappearing does. A later observed record does not
advance account coverage by itself. Extra accepted state and its dependencies
may be incorporated without claiming the intervening account prefix complete.

Do not infer deletion from source absence. If the receiver holds accepted state
the source lacks, use normal bidirectional recovery. Preserve local pending work.

### Dependencies, service units and completion

The source can convey required support and coupled-unit descriptors as it
discovers them. Newer values may require additional support outside H; transfer
that support without claiming a larger complete prefix. Freeze a concrete
bounded set of selected inputs while sending that batch rather than continually
replacing an in-flight input with the latest value.

The receiver must distinguish durably staged input from installed/represented
state, retaining the existing service dependency and atomic-unit checks. Missing
support prevents that obligation from completing; unrelated records can install.
Recovered divergence stays in client review, and the server does not acknowledge
coverage containing an unrepresented accepted alternative.

Completion accounting must cover required versions, not just unique record IDs
when more than one incomparable accepted version is relevant. An explicit source
end message certifies exhaustion of its discovery procedure for H; the receiver
also verifies its durable processing and outstanding dependencies before saving
coverage. Ordered WebSocket delivery does not imply ordered database completion.

## 8. Receipt and completion responsibilities

`ReceiptState` belongs to the live receiving connection. It records the fixed
source target, discovery mode, next page sequence, exact version/deletion
requirements, required interpretation and coupled effects. It is not persisted,
and no old receipt ID is resumed after reconnecting.

| Transition | Responsibility |
| --- | --- |
| Begin | Validate the source target and preserve pending comparison evidence before incoming interpretation. Do not change completed coverage. |
| Receive page | Check the next sequence. Commit accepted values, exact support, interpretation and semantic staging in one transaction; advance connection-owned receipt state only after commit. Then acknowledge that page. |
| Complete | Validate the source end's target/page count and reload actual represented values, interpretation and coupled effects in the checkpoint transaction. Save coverage only when every obligation is represented. Acknowledge only after commit. |
| Disconnect/restart | Discard receipt bookkeeping. Keep committed accepted values, staging, support and pending work. Start fresh discovery from the last completed checkpoint. |

Requirements identify native keys and exact accepted stamps/deletion flags.
Identical requirements deduplicate. A proven successor can represent an older
requirement; an incomparable value cannot. Historical support remains distinct
from current heads, and unresolved recovered alternatives do not establish
accepted server coverage.

Both producers send ordered pages and await committed page acknowledgments before
advancing. There is no within-receipt page retransmission or last-page payload
cache. Safe repetition across new exchanges uses existing semantic comparison
and duplicate handling. Page acknowledgments provide bounded backpressure;
complete-coverage acknowledgments have the separate retention responsibility.

SQLite page installation and PostgreSQL recovery share accepted-value dependency
readiness. The client then applies working/original comparison rules; the server
performs actual recovery and reference checks under its account lock and atomic
coupled-group savepoints. The server does not construct synthetic client records
or an artificial complete window to obtain readiness decisions.

Semantic staging and coupled descriptors survive independently of a receipt.
Later exchanges can supply their missing dependencies. Exact service support and
pending originals retain their interpretation. No receipt generation, persistent
exchange ledger or in-flight pin manager is required.


## 9. Event, connection and application lifetime

**Engineering default:** ordinary in-process application publication/subscription
using Tokio facilities, with durable state/journal supplying actual work.
Publish account-change notices from settled application operations. Use existing
command/result ownership so an operation's committed outcome is not abandoned
merely because its originating window/request disappears. Coalesced notices mean
reassess state, not credit a revision as received. Subscriber lag requires a
durable-state recheck. No PostgreSQL notification mechanism, broker, outbox or
second persistent delivery log is selected.

Use the existing desktop session task ownership, cancellation and view-refresh
paths. Drain admitted storage work before closing its pool. Server application
shutdown also owns upgraded connection tasks, which ordinary HTTP request
draining alone does not describe. Use standard Tokio task/cancellation facilities.

Authorize the HTTP upgrade with the existing DPoP resource checks and derive the
proof target from the deployment's public HTTPS resource URL. The socket's `wss`
connection scheme does not create a separate proof-normalization algorithm.
Never put access credentials in query strings, protocol labels or telemetry.
Reuse Huskarl's signing, refresh and access caching rather than implementing
those protocols again.

Current actor/grant authority is checked for active sessions and at each
admission/source-read boundary. Revocation events close affected sessions;
notifications do not replace database authority. Already completed authorized
operations retain their normal transaction ordering against revocation.

**Engineering default:** renew a connection through a fresh authenticated
upgrade when its short-lived access authorization expires. This reuses the
normal refresh/reconnect/catch-up path instead of adding an in-band credential
replacement protocol. It does not re-enroll the client or discard pending work.
The tradeoff is periodic connection setup; this is a proposed session-lifetime
choice, not an additional product security requirement.

Use bounded standard channels and messages. Batch sizes and transport limits are
engineering tuning, not native field limits or permission to truncate values.
If a limit prevents required receipt, report the error and leave coverage
unchanged; do not bypass receive-first publication ordering. Choose limits
against real serialized native values during implementation planning rather
than treating an arbitrary transport default as a product restriction.

## 10. Recovery and retention boundary

Individually received accepted knowledge is offered independently of the client’s
last complete checkpoint, including a record received after that checkpoint and
retained alternatives whose current shared observation is absent. Use genuine
peer coverage to omit already covered input; never turn a record’s maximum stamp
into complete coverage. An ahead-of-head accepted position is a rollback witness
for the existing fork transition, without certifying its whole old prefix.
Unresolved alternatives retained for review do not satisfy recovered-prefix
completion; an accepted result or proven accepted successor must represent them.

Carry the existing `check_history`, fork planning, recovered installation and
recovered-coverage operations into real storage and transport. Already accepted
state offered by an authorized client retains its genuine original stamp even
when its author has been revoked. New pending changes still obey current actor
admission and the receive-first rule.

Do not expose old accepted state as a new client edit or invent coverage from
the largest returned stamp. Recovery publication uses actual durable represented
state and the native guards. Ordinary restarts preserve identities; arbitrary
rollback detection and recovery of facts lost everywhere remain outside the
guarantees. Operator-controlled publication pause/resume remains unchanged.

Persist acknowledgments and interpretation needed by the existing common-coverage
and baseline planners. #16 owns their complete operational collection/recovery
verification; #14 must not discard those roots or install an incompatible model.

## 11. Observability and meaningful verification

Extend existing tracing, structured events and metrics at the actual operations:
connection/authorization outcome, discovery path, committed receipt, proposal
outcome, blocked references, recovery and transport/storage failure. Account,
actor and request correlation are structured context, not unbounded metric
labels. Separate planned, staged and committed effects. Exclude credentials,
proofs, private material, QSO contents and arbitrary untrusted payload dumps.
Do not introduce a generic telemetry lifecycle/completion abstraction.

Verify the actual production path with real client stores, disposable PostgreSQL
using production migrations/grants, and the production WebSocket handlers:

- Two clients exchange independent changes; receive-first ordering holds while
  logging creates further work for a subsequent cycle.
- A lost acceptance response and later local edit reuse the native frozen/own
  version handling without duplicate acceptance or a false competing edit.
- Partial receipt does not advance coverage; restart/reconnect reuses committed
  state, including incomplete service support, and finishes safely.
- Stale references and the fourth-QSO/tag-deletion case preserve per-record
  classification and block only the invalid deletion. Exercise the actual
  acceptance check rather than a manufactured database trigger.
- Concurrent writes during enumeration, unavailable journal history, and source
  collection exercise the stated completeness argument through the real producer.
- Revocation denies active-session work and admission through current authority;
  account-qualified requests cannot operate on another account.
- Incompatible protocol rejection preserves enrollment, local values and
  comparison state. Source/receipt failure reporting counts committed work only.

Reuse existing core/store scenarios for their established behavior. Do not
repeat them through every UI widget or test SQLx/Tokio/WebSocket implementations.
Keep CI's existing Windows disablement and other checks unchanged. Native Windows
operation remains explicit acceptance evidence when available; it is not implied
by Linux tests or this proposal.

## 12. Defaults and unresolved choices

There is no additional product or architecture decision established by this
proposal. The following are implementation defaults, distinct from agreed product
behavior:

1. Incremental discovery/completion accounting in sections 7–8, adapting the
   earlier manifest-first interface while retaining its actual guarantees.
2. Native Serde JSON protocol version 1 on `/api/sync`, using existing domain
   values and connection-local request correlation.
3. In-process Tokio publication/subscription for application events.
4. Fresh authenticated connection on access expiry, using existing refresh and
   reconnect behavior.
5. Distinguish known local creation from missing history using the existing
   `NeverCreated` basis with an optional checkpoint, as specified in section 6.

Sections 7–8 define the implemented discovery/receipt boundary. Native
`DiscoveryStart`, `DiscoveryPage` and `DiscoveryEnd` messages register a fixed
target and growing obligations. Each page distinguishes current accepted heads
from exact historical supporting facts; only heads update the client's shared
comparison state. Supporting facts remain available for dependency checks and
recovery without replacing a current head. `ReceiptState` tracks ordered pages
and connection-owned obligations. SQLite commits domain effects in immediate
transactions and verifies final coverage against actual stored values; PostgreSQL
uses account-locked acceptance/recovery transactions and
short repeatable-read discovery reads. `Facts`/`FactsEnd` carry
individually accepted repair without claiming source coverage. `Deferred` keeps
incomplete returned input while allowing unrelated exchanges to continue.

The current implementation supplies the authenticated desktop/server consumer.
Conflict-review UI remains #15; #16 owns the complete retention/collection and
recovery verification matrix. Batch sizes, reconnect delays and transport limits
remain engineering tuning, not additional product validity rules.

Failure responses use Incompatible for protocol mismatch, Invalid for rejected
wire input, Unauthorized for lost authority on an open connection, and
Unavailable for operational/source failures. Explanations come from fixed
request diagnostics or bounded source conditions, never arbitrary error chains.
A source losing its target or journal interval ends that exchange; the ordinary
reconnect negotiates journal catch-up or fuller state from durable coverage.

The connection retains socket ownership for a best-effort failure response.
Shutdown, normal access expiry and broken transport do not trigger another error
write. Failure delivery is not an acknowledgment and cannot undo committed work.
The desktop preserves the supplied category for structured diagnostics and
displays the safe explanation using its existing error/reconnect behavior.
Dependency support travels in ordinary pages; there is no separate on-demand
exact-version request exchange.

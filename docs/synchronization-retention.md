# Synchronization retention and recovery

Automatic collection and real-store recovery extend the account synchronization
model. Decisions D152–D156, D248 and D388–D390 are requirements. Choices marked
**Engineering default** are implementation settings, not additional product
decisions.

## 1. Scope and existing behavior

Keep one synchronization scope per account, journal catch-up with fuller-state
fallback, receive-first client synchronization, client-side conflict resolution
and the existing WebSocket protocol. Authorized clients remain eligible
indefinitely. Revocation closes new automatic admission from the old actor; it
does not erase accepted contributions. Surviving accepted state can recover an
older server through another currently authorized client's server connection.
There is no peer-to-peer topology, storage quorum, per-edit history graph or
new identity on ordinary restart.

The production components provide:

- `clacks-sync`: coverage/lineage, baseline-aware comparison, durable-receipt
  rules, pending-basis witnesses, membership transitions and rollback recovery.
- `clacks-server`: account-locked acceptance/recovery, journal/full discovery,
  stored acknowledgments, retained/staged accepted versions and an existing
  runtime-owned maintenance task lifecycle.
- `clacks-client-store`: durable accepted/original/working/prepared/review roles,
  SQLite receipt transactions, witness sealing and full accepted-state return.
- `clacks-desktop`: authenticated synchronization and Unsynchronized changes
  review, including conservative comparisons and deleted-reference repair.

Common-boundary selection and metadata conversion run in one PostgreSQL
transaction. Separate transactions prune journal entries and reclaim unreferenced
storage. SQLite cleanup runs after a completed desktop synchronization cycle.
These operations use the existing native comparison and receipt rules.

## 2. Binding decisions and engineering defaults

| Binding behavior | Source |
| --- | --- |
| Common coverage of authorized clients bounds ordering-evidence collection; offline absence never retires a client | D155, D152 |
| Received state, checkpoint and pending comparison evidence are durable before acknowledgment | D156 |
| Missing incremental history selects fuller-state reconciliation | D144, D60 |
| Timeline ancestry records exceptional recovery boundaries; missing ordering has an explained reconciliation fallback | D148, D154 |
| All account data uses one scope and every enrolled client participates | D248, D161 |
| Routine server collection is automatic | D388 |
| Incremental journal retention uses a per-account entry-count limit | D389 |
| Baseline installation and required metadata conversion are one atomic transaction; physical cleanup is separate | D390 |
| Distinct semantic service evidence remains retained for MVP; deletion and replay memory have independent obligations | service-evidence.md §12, D140 |

**Engineering defaults:**

- Run collection inside the existing server process using Tokio and SQLx.
  Start one maintenance loop per application instance; account-row locking
  supplies existing cross-instance serialization.
- Start with a five-minute sweep interval, account discovery pages of 100, a
  journal target of 10,000 entries per account, and physical deletion batches of
  at most 1,000 owning rows. These are starting settings, not measured optimal
  values or a performance service-level promise.
- Make the journal target a deployment configuration value. Cadence, account
  page size and cleanup batch size are named internal defaults; do not
  introduce a configuration framework or public control for every constant.
- Process accounts serially within a worker. Page through the entire account
  set, checking shutdown between accounts/batches, rather than loading all
  accounts or doing only the first page on each sweep.
- Retain historical representations required by genuine exact-fact dependencies.
  Let covered effects and coupled members use established successors/deletions
  under the existing native rules; do not make an all-exact loader a retention
  policy or rewrite immutable service evidence for collection.
- Preserve nonredundant timeline ancestry. Remove duplicate/unreferenced stored
  snapshots, not timeline IDs or ancestry intervals merely because they are old.
- Do not add an operator collection command, durable maintenance-job table,
  persistent source lease, retention pin service or collection manifest.

## 3. Database changes

### PostgreSQL

Reuse `sync_accounts`, `sync_baselines`, `coverage`, `lineage`,
`timeline_boundary`, `checkpoint`, `desktop_clients`, `accepted_head`,
`accepted_record`, `version_stamp`, `record_vector`, the payload tables,
`installation_journal`, `staged_accepted`, `retained_accepted`,
`current_external_binding`, `coupled_unit` and `coupled_member`.

`installation_journal.installation_order BIGINT GENERATED ALWAYS AS
IDENTITY` has an index on `(account, installation_order)`. This is a database-local
retention ordering, not a synchronization revision or ordering proof. The current
`(account, timeline_id, revision, kind, record_id)` key remains the semantic
installation identity. Each account's writers already serialize on its account
row, so this sequence provides append order across timeline changes without
comparing unrelated timeline revision numbers. Gaps have no meaning. This column
is not transmitted, recovered from clients or used for causal comparison.
The server also uses it as its private journal page cursor. A metadata
installation can have several record entries at one revision and must not skip
entries at a page boundary. Continue
filtering the traversal by native per-timeline lower/target coverage; append order
is not a substitute for those checks.

Store the complete native journal obligation using the existing
`RecordRequirement` fields: record key, installed stamp and deletion state.
The journal retains key/stamp references and `deleted BOOLEAN NOT NULL`,
derived from the actual installed `AcceptedRecord` when constructing the native
journal change. This describes the historical obligation, not the current head.
Read it directly during discovery; do not reconstruct one deletion bit by loading
a historical accepted envelope or retain a QSO body for that lookup.

Continue using `sync_accounts.retained_journal_coverage_id` as the published
per-timeline journal floor. Do not add a second journal checkpoint or an age
column to implement the entry-count policy.

Add account-qualified reference indexes needed by the actual collection queries:
accepted-record references to payloads/stamps, stamp references to vectors and
baseline IDs, checkpoint references to baseline IDs, journal references to
stamps, and existing service/coupled/binding references. Inspect existing indexes
before adding any; primary/unique indexes that already cover a predicate are not
duplicated. Schema constraints express ownership, identity and references; do
not add enum mirrors, business-state CHECK constraints or stored liveness flags.

Extend the runtime role's grants only for the metadata/payload rows the automatic
collector actually updates/deletes. Do not grant schema mutation, account
administration, identity-management or key-management rights for collection.
Production schema changes remain release-managed migrations with checked SQL
files and regenerated SQLx offline metadata.

### SQLite

Reuse the existing role tables and `retention` module. No maintenance-state table,
local installation journal, schema-version marker or new counter is required.
Add only missing reference indexes demonstrated by the concrete cleanup queries.
Local cleanup uses the existing immediate write transaction and ordinary snapshot
reads. Client-local settings, active operating copies and enrollment material
are not synchronization garbage.

## 4. Common boundary and projection eligibility

Inside the account-locked installation transaction, read current account state,
retained interpretation and membership. Expand the server's installed checkpoint
and every authorized client's durable acknowledgment through their actual
baselines and position-dependent ancestry. Intersect certified prefixes by
matching timeline identity. Missing certification is not a zero or a high-water
claim.

**Engineering default:** an authorized client without its first completed
acknowledgment prevents advancement beyond the existing baseline. It can still
complete normal full catch-up. When there are no authorized clients, the server's
own complete installed coverage bounds the common prefix. Neither rule expires
or revokes a client.

Require the resulting coverage K to preserve the existing baseline's certified
coverage. If interpretation is missing/incomparable and the collector cannot
prove a safe advancement, leave that account unchanged and report the specific
constraint. Do not invent coverage, pause unrelated synchronization or select a
numeric minimum across unrelated timelines.

Only closed client actors whose actual closure positions are covered by K are
eligible for removal from residual vectors. A still-authorized actor, an actor
closed beyond K, and a component needed by an unproved comparison remain.
Preserve every acceptance position and its required server component.

For each current accepted context proposed for conversion, use the native
comparison/lineage rules to establish that it represents this record's K-prefix.
Projection removes only eligible closed-actor components and changes the baseline
interpretation. It must preserve the logical version and all native values.
An unprovable context stays unchanged with its existing interpretation retained.
Do not project an incomparable retained/staged alternative as if it were the
current accepted head.

Create a new baseline only when it enables a real eligible metadata conversion
or a proved reduction in required ordering detail. A larger acknowledged revision
alone is not work: repeated idle sweeps must not generate new baselines, revisions
or notifications in a maintenance/acknowledgment feedback loop. Journal pruning
and removal of unreachable relational copies can proceed without a new baseline.

Implement the common-coverage and context-projection calculations in
`clacks-sync::compaction` with their real server caller in this delivery. Pass
native keys/stamps, membership and interpretation; do not load full QSO payloads
merely to remove a vector component. Use named native result fields and existing
constructors, not serialized field diffs or a new database-neutral repository.

## 5. Atomic baseline installation

The PostgreSQL operation owns one transaction:

1. Acquire the existing account-row lock and read the authoritative account,
   membership, acknowledgments, interpretation and candidate contexts there.
2. Select K and eligible closed actors. If no proved conversion is needed,
   finish without allocating a baseline or advancing the account revision.
3. Construct a fresh immutable `Baseline` with K, the necessary ancestry and the
   membership generation used by this calculation.
4. Insert replacement vectors/stamps and accepted envelopes for the contexts
   actually converted, **reusing their existing payload IDs**. Preserve their
   native identity, payload and `accepted_at`. Repoint the corresponding current
   heads and current binding references to the equivalent representation where
   required. Keep original representations wherever an exact reference needs
   them. Do not invoke the ordinary accepted-value writer if it copies payloads.
5. Save the baseline and update the account's current baseline/head/installed
   metadata together. Allocate one ordinary account metadata revision through
   the native checked transition; do not claim additional recovered coverage.
   Append a journal entry for each converted current context at that installation
   position, so normal catch-up distributes equivalent projected representations
   to clients. These are metadata installations, not new native QSO edits or new
   acceptance positions. Private append-order paging traverses all entries even
   when their shared installation revision spans multiple pages.
6. Commit, then report committed conversions and use the existing account-change
   notification to wake connected clients.

The account lock prevents concurrent acceptance, enrollment, revocation or
acknowledgment from invalidating these inputs. Reads/planning must not escape the
transaction and later apply without the corresponding existing-state guards.
There is no network wait inside this transaction. No duration estimate or claim
that an account-wide metadata conversion is too expensive is established.

Old envelopes are not overwritten in place. Genuine exact-fact references can
continue naming them; journal obligations retain their own key/stamp/deletion
descriptor rather than requiring the old envelope body. Baseline-aware comparison
recognizes the new current representation as the same logical version. Avoid
rewriting shared QSO/service values, producing user conflicts or changing upload
eligibility solely because metadata was compacted.

## 6. Journal collection and its published floor

Use a separate account-locked transaction. Count/locate entries by the indexed
per-account installation order and select the oldest excess rows over the target.
A physical batch removes at most the configured/default cleanup batch size. The
entry limit is the maintenance target, not a reason to reject logging while a
backlog is being reclaimed; a busy account can temporarily exceed it.

For each timeline represented in the removed rows, advance the existing journal
floor through the highest **installation revision** removed, retaining earlier
floor coordinates. Delete the selected prefix and publish the new floor in the
same transaction. Do not derive this floor from the envelope's original accepted
position or the smallest surviving row, and do not treat gaps occupied only by
account metadata mutations as missing QSO records.

Normal serialized appends follow installation revision order within a timeline.
Nevertheless, correctness uses the maximum removed native revision, not an
assumption that every lower row was removed or that retained rows are contiguous.
Keep the floor monotonic; an empty journal does not prove that an old client has
no changes to fetch. This algorithm uses entry count across all of an account's
timelines, not a full separate allowance for every old timeline.

A batch can remove only part of the entries at one installation revision. The
floor still advances through that whole revision, conservatively requiring full
reconciliation for a receiver before it. A receiver durably acknowledged through
that revision already represents every obligation there; remaining entries at
the floor are harmless, not proof that the removed ones are available.

Existing discovery checks journal availability in its page transactions. If
collection overtakes a live incremental exchange, that exchange cannot certify
completion; use its existing unavailable-source/reconnect path and negotiate
again, selecting full reconciliation when needed. Do not retain a persistent
source session or postpone safe pruning until an absent receiver cooperates.

## 7. Liveness and physical reclamation

The collection queries must implement this root set and its reference closure.
A row having no direct foreign-key reference is not by itself proof that its
native meaning is unused.

| Root/consumer | Required retention |
| --- | --- |
| Current accepted heads, including deleted records | Keep their identity, native state and interpretable context. Never remove a current tombstone as an orphan. |
| Retained accepted alternatives and staged accepted input | Keep their complete values, exact contexts and support until existing reconciliation/staging rules release them. An old position alone does not release them. |
| Retained journal entries | Keep the stored key/stamp/deletion obligation and its ordering interpretation. The journal does not root a historical QSO body merely to recover deletion state. |
| Service `ExactFact` requirements | Keep the exact accepted representation and its interpretation. Do not silently replace it with newer QSO values. |
| Service `CoveredEffect` requirements and coupled members | Keep the descriptor/context and sufficient accepted effect, established covering successor or deletion under the existing readiness rules. An old body is not rooted when a suitable accepted successor/deletion already represents the effect, unless another consumer requires that body. |
| Current external bindings, semantic evidence heads and required historical service values | Keep all distinct semantic facts and their required references under service-evidence.md §12. Collection of duplicate encodings is not service-history expiry. |
| Current account baseline, lineage, installed coverage and journal floor | Keep their complete meanings and every nonredundant ancestry interval needed to interpret surviving positions. |
| Authorized clients' acknowledged checkpoints | Keep each exact checkpoint and its baseline/coverage interpretation, not just the highest numeric acknowledgment. |

Trace these roots through accepted envelope → payload/stamp, stamp → vector and
baseline meaning, checkpoint → coverage/baseline meaning, baseline →
coverage/lineage, lineage → boundary coverage, and through typed service and
coupled dependencies. Baseline IDs stored as native text still create logical
references even where the schema does not have a baseline foreign key.

Preserve each native requirement's kind during support loading. For `ExactFact`,
load the required exact accepted fact and its interpretation. For `CoveredEffect`
and coupled members, first use accepted material whose native comparison/readiness
result proves that it represents the required version: the same logical version,
an established successor, or a covering deletion. Include that actual accepted
material and the unchanged dependency descriptor in the transfer. Matching user
values alone do not prove coverage, and unknown ordering is not succession.

Discovery and collection use the same native support predicate, preserving
exact-fact versus covered-effect semantics. If no covering material
exists, retain/load the required historical effect where available; otherwise
keep the existing incomplete/staged outcome without inventing a replacement.
Do not reinstall an old effect over its accepted successor or alter genuine
exact-fact semantics. These changes need no projection aliases, immutable
evidence rewriting or new dependency engine. Required descriptors, vectors and
baseline meanings may still outlive an otherwise collectible historical body.

After baseline installation or journal pruning, delete unreachable copies in
bounded, account-locked transactions. Recompute liveness in each transaction;
do not persist an earlier mark set and later sweep newly referenced rows.
Ordering contexts above the common boundary remain protected even if journal
payloads were pruned. A historical payload may be removed when no consumer needs
its body, while its ordering stamp/vector remains retained if D155 requires it.

Delete owning rows and their exclusively owned dependent rows in schema order,
then reconsider now-unreferenced stamps/vectors/checkpoints/baselines/lineage/
coverage and typed payload components. Sharing a payload between old and
projected envelopes must not duplicate its body or prematurely delete it. Use
explicit checked queries for the actual schema families, not a runtime generic
foreign-key crawler, stored reference counts, or soft-delete staging tables.

Do not remove `record_identity` or accepted deletion memory merely because
payload history is collectible. Do not delete desktop enrollment/closed-client
rows or alter credential validity as a shortcut to collecting vector entries.
Existing authorization retention owns those records; no new timeout or lost-
device handshake is introduced. Normal PostgreSQL vacuuming remains database
maintenance rather than an application transaction.

## 8. Client interpretation and local cleanup

Keep the production receipt/preparation/review engine. Extend the existing
`retention` implementation, which already seals pending bases before receipt.
When a compacted interpretation is installed, receiving an equivalent projected
current envelope does not change native values or create a conflict. Pending
originals and prepared values retain their true contexts and are not rewritten
as if based on the newer baseline.

Before releasing a pending basis's old backing interpretation, retain its
self-contained witness and genuine original/working/prepared values in the same
SQLite transaction. A witness cannot turn a genuinely missing original or an
unknown/divergent relation into a proven predecessor. If sealing is insufficient,
keep the actual old context and its baseline/lineage roots instead.

Advance the completed checkpoint only after the existing receipt proof and
pending preservation requirements hold. Reload that durable checkpoint before
sending acknowledgment. Do not advertise the highest received revision or let
local cleanup advance account coverage.

**Engineering default:** perform local reclamation after a completed sync cycle,
when its connection-owned source/receipt objects have finished, using existing
session/store ownership and immediate write transactions. Perform one bounded
batch per completed cycle; any backlog waits for a later cycle. No second
desktop scheduler or offline-only save-history maintenance runs.

Local roots include `client_record` observations/pending roles, retained/staged
accepted values, `local_review` and `review_retained`, original/prepared/witness
rows and their conditions/references, the installed checkpoint/current lineage,
service support/coupled/binding state and active operating/saved-source state.
Support traversal starts from surviving accepted envelopes and from service
payloads owned by original, working, prepared and review roles. Keeping a draft
payload alone is insufficient when it still requires an exact historical body.
Keep the current actor's local counter high-water marks. Old counters are not
collectible while pending/prepared work or authorization-review holds need them;
never infer revocation merely from elapsed time. Credentials and enrollment
continuation state remain owned by their existing workflow.

Remove unreachable relational copies without changing those roles. Ordinary
read snapshots may finish against their earlier SQLite snapshot. No test-only
trigger, artificial generation counter or new journal protocol is needed.

## 9. Recovery and interruption behavior

| Interruption or return | Required result |
| --- | --- |
| Crash before atomic baseline commit | Old baseline/heads remain authoritative; a later sweep can try again. |
| Crash after installation but before cleanup | New interpretation is durable; redundant old rows remain safe to reclaim later. |
| Crash during a journal-cleanup batch | Removed rows and published floor either both commit or neither does. |
| Crash between physical cleanup batches | Previously committed removals remain valid; resume by querying current roots, without a persisted job cursor. |
| An authorized client returns below the journal floor | Normal full reconciliation, preserving original/working/prepared/review state; no export or re-enrollment. |
| Revoked actor's pending work returns | Existing admission cutoff/manual review applies; an accepted old contribution is still recoverable through a currently authorized carrier. |
| A server backup predates the latest baseline/closure | Use actual surviving client-held accepted state, witnesses and interpretation through the existing negotiation/recovery path. Do not infer lost authorization or missing facts. |
| Returned state is divergent or ordering is unavailable | Existing client review explains the actual uncertainty; unrelated state progresses. The unresolved return does not certify a complete recovered prefix. |
| Collection overtakes a source's retained interpretation | Abort that discovery's completion claim and negotiate again; do not fabricate an empty range, lease the source or turn unavailable context into concurrency. |

Retain genuine old baselines received during recovery under their immutable IDs;
do not attach the current baseline's meaning to an old ID. Recovery may bring
back an older representation of an already compacted logical version; native
comparison must recognize compatible projection without reintroducing a QSO edit.
Source enumeration/dependency closure and receipt proofs remain those of #14.

Ordinary restarts keep identities. Witnessed regression can fork the timeline;
reused counters already beyond every surviving witness remain outside universal
detection guarantees. Reapply known revocations after an older backup as required
by D143. Protected server-only credentials still require the documented backup
material; synchronizing QSO state does not recover a lost private key.

After a recovery fork, a surviving accepted record can contradict the restored
server's claimed absence inside an old prefix, including revisions reused by
reauthorization before the rollback witness arrives. Preserve the copy and
request explicit `ContradictoryCoverage` review for editable records. Other
records continue to recover; the unresolved record cannot certify recovered
coverage. An explicit resolution creates a new conditional acceptance through
the existing client workflow. Service evidence remains staged rather than
becoming a manual QSO edit. Same-history contradictory claims remain integrity
errors.

D141's external-upload pause and explicit resume remain unchanged. Actual
provider effects/history and their restored-startup controls belong to the
consuming publication/recovery workflows; this issue neither invents provider
uploads nor waits for every absent client to return.

## 10. Runtime and transaction ownership

The existing server runtime owns the collection loop alongside issuer cleanup.
Use Tokio interval/missed-tick facilities and the existing shutdown token/task
ownership, not a new job framework. Automatic collection uses the runtime pool
and account-qualified queries, not a schema-changing maintenance credential.

Each account operation acquires the same account row used by current writes.
A worker may skip a currently locked account using PostgreSQL `SKIP LOCKED` and
revisit it on a later sweep; it must not stop the sweep for other accounts.
A second application instance may observe no work after the first commits.
Neither needs a deployment-wide leader election or advisory-lock lease.

Stop admitting maintenance work at shutdown and let its admitted transaction
finish within the ordinary server drain policy. If the process/deadline ends it,
standard transaction rollback preserves atomicity. Do not claim a committed
conversion until commit is confirmed. Reconnect/restart reads durable state;
notifications are ordinary hints, not an additional durable publication log.

## 11. Module and dependency layout

| Owner | Change and actual consumer |
| --- | --- |
| `clacks-sync::compaction` | Native common-coverage and projection calculations consumed by the real account installation operation; preserve existing witness/comparison APIs. |
| `clacks-server::store` retention submodule | Concrete PostgreSQL account installation, journal pruning, liveness and cleanup queries. Runtime worker calls it; no generic cross-backend repository. |
| Existing server metadata adapters | Reuse immutable baseline/vector/stamp construction; add metadata-only accepted representation insertion that reuses a real payload ID. |
| Existing server discovery/recovery | Read stored journal obligations, enforce floor/interpretation availability, retain genuine exact facts and satisfy covered effects/coupled members through the existing native coverage rules. |
| Existing server runtime/configuration | Own scheduling, configuration, shutdown and domain telemetry. |
| `clacks-client-store::retention` | Preserve local roots/witnesses and reclaim unused relational copies after completed cycles; extend actual desktop session integration. |
| Existing desktop synchronization/review | Consume unchanged native outcomes and local state. No new conflict queue or recovery interface. |

Keep fixed SQL in separate query files with compile-time checking. Rust owns
normalization, vector/projection eligibility and semantic validation; SQL owns
account-qualified selection, relational references and atomic persistence.
Additional helper/API visibility is justified by these delivered callers, not
future consumers or tests.

## 12. Observability and optimization evidence

Follow [the observability contract](observability.md#synchronization-retention-and-baseline-installation).
Installation, journal pruning and physical cleanup are distinct workloads with
separate metric families. Metric names describe the measured operations:

| Quantity | Metric |
| --- | --- |
| Installation attempt duration including commit | `clacks_sync_baseline_install_duration_seconds{outcome}` histogram |
| Installation account-lock acquisition wait | `clacks_sync_baseline_lock_wait_seconds{outcome}` histogram |
| Application-observed installation lock-holding interval through commit/rollback | `clacks_sync_baseline_lock_hold_seconds{outcome}` histogram |
| Metadata workload actually examined by an attempt | `clacks_sync_baseline_contexts_examined` histogram; observed work, not a committed-effect count |
| Context representations actually converted | `clacks_sync_baseline_contexts_converted_total` counter, after confirmed commit |
| Journal-pruning transaction duration | `clacks_sync_journal_prune_duration_seconds{outcome}` histogram |
| Journal entries actually removed | `clacks_sync_journal_entries_removed_total` counter, after confirmed commit |
| Physical-reclamation duration | `clacks_sync_metadata_reclaim_duration_seconds{outcome}` histogram |
| Rows actually removed, separated by physical family | Family-owned removal counters for envelopes, vectors, stamps, payloads and interpretation rows; do not mix arbitrary row kinds into one execution metric |

Use bounded outcomes appropriate to each operation: installed/pruned/reclaimed,
unchanged, contended, blocked and failed as applicable. Do not add the entire
cross-product of unrelated operation labels to every metric. A skipped account
has no fictitious lock-holding duration. Journal pruning and physical cleanup
distinguish committed removals, no work and lock contention independently of
whether another bounded batch may be needed. Ordinary synchronization/enrollment
write boundaries report their own account-lock waits where the same lock is
acquired, rather than putting unrelated callers under the installation metric.

Use standard tracing spans with explicit safe fields and normal control flow.
Framework span lifecycle supplies trace timing; explicit runtime-boundary
histograms measure these operational quantities. Do not duplicate tracing with
custom completion/lifecycle machinery. Account IDs and selected timelines/coverage
belong in access-controlled structured context, never metric labels. Workload
counts include examined contexts, eligible actors, converted contexts, rows
removed and retained-root/blocked reasons. Do not log native values, arbitrary
user text or credentials. Record only confirmed durable effects as committed.

Successful installation is a significant `info` milestone. Routine no-work or
retention-boundary detail is `debug`; witnessed rollback is `warn`; unexpected
maintenance failure is reported once at its handling boundary with source causes.
Preserve existing account/source/record correlation when debug spans are filtered.

Initial measurements must relate installation duration and lock ownership to
actual metadata workload and the wait experienced by normal account writes.
Do not call the atomic design slow based on QSO count alone or set a performance
alert threshold without observed behavior and an operational requirement.

## 13. Meaningful verification

Extend existing real-store/session scenarios rather than build another simulator.
Use project Rust tooling and production migrations with disposable SQLite and
PostgreSQL stores. Do not modify persistent development accounts or enable
currently disabled Windows CI. No Python fixtures, database-trigger fault
injection, library-behavior tests or coverage-number gate is introduced.
Tests use existing preparation, discovery, receipt, review and maintenance
operations rather than test SQL or direct query-file execution. The isolated
archive restore uses ordinary PostgreSQL tools. Physical sharing and reference
selection remain implementation obligations; behavioral assertions do not claim
to measure duplicate rows or disk usage.

- Exercise three actual durable acknowledgments at different positions; verify
  common-boundary selection and that revocation removes only the closed actor's
  participant obligation. Preserve a never-acknowledged authorized client without
  making up its installed coverage.
- Compact eligible closed-actor contexts; verify unchanged native payloads,
  acceptance positions, external evidence and comparison outcomes. A repeated
  sweep produces no new baseline/revision or conversion count.
- Page an installation containing more converted contexts than one discovery
  page at the same metadata revision. All entries must reach the real client;
  acknowledgment cannot pass a skipped entry. Verify the equivalent incoming
  contexts replace current representations without changing native values or
  moving a pending original to a new basis.
- Prune beyond a small configured entry limit across a real recovery timeline
  change; verify incremental versus full discovery at the exact retained
  installation boundary, then return an older authorized
  client through actual full reconciliation with pending edits preserved.
- Retain a journal entry while reclaiming its unneeded historical QSO body;
  discovery must still emit its correct stored key/stamp/deletion obligation.
  Verify both live and deleted obligations without introducing a QSO restoration
  workflow. Current tombstones and independently required contexts remain roots.
- Retain an effect descriptor with a genuine accepted successor/deletion, reclaim
  the otherwise unneeded old effect body, and transfer/install sufficient support
  through the real discovery/readiness paths. Keep a genuine `ExactFact` case
  rooted, and preserve incomplete/staged behavior when neither exact nor covering
  material is available. Preserve distinct semantic service evidence throughout.
- Install a baseline while another account remains usable. Exercise the same
  account's existing locking/guard semantics through real writes, asserting a
  valid serialized result rather than timing the PostgreSQL lock implementation.
- After an actual installed baseline but before cleanup, close/reopen and
  continue maintenance. Between committed cleanup batches, close/reopen and
  confirm repeat/no-op behavior. Do not add a production failpoint to force a
  chosen internal line or separately retest transaction rollback guarantees.
- Restore an older real server database state from a disposable snapshot while
  retaining newer actual client stores. Cover backups before the baseline and
  before a later actor enrollment/revocation, recovery with counter reuse, and
  uncertainty with incomplete surviving interpretation. Preserve the #12 and
  #15 reauthorization/review behavior and unrelated progress.
- Verify committed counts, actual outcome classification and safe telemetry in
  the owning operation tests. Do not duplicate the functional matrix solely to
  inspect metrics or test histogram/subscriber implementations.

A deterministic snapshot/restore fixture uses ordinary disposable database tools
and Rust command drivers already appropriate to the existing test environment.
It is not a new shipped backup command, alternate authorization service or
artificial synchronization runtime. Native Windows and unavailable live/native
acceptance remain explicit verification limits, not passing claims.

## 14. Decisions versus implementation discretion

D388–D390 select when collection runs, the journal-budget kind and the initial
atomic installation direction. They do not select numeric defaults, a new
protocol, a background framework, a new restore mode or guaranteed lock latency.
Operational defaults can change with measured workload evidence while preserving
the binding behavior. Changing the protocol, authorization lifetime or conflict
policy requires a separate product decision.

# Synchronization metadata and transitions

**Status: proposed engineering design for [issue #2](https://github.com/jhelwig/clacks-log/issues/2), for review.**
This document specifies production synchronization components. It is not an
implementation plan, an implemented protocol, or a proof supplied by a research
paper. Later persistence and transport work integrates and extends these
components. No separate reference synchronization engine is proposed.

## 1. Requirements carried forward

The [decision register](../../decisions.md), [synchronization contract](../../synchronization.md)
and [recovery requirements](../../backup-recovery.md) remain authoritative.

- All synchronization traffic is between a client and the server. Clients never
  synchronize directly with one another. A client can return server-distributed
  accepted state to an older restored server through that same connection.
- One synchronization scope and timeline per account covers account-owned QSOs,
  tags and synchronized shared data. Every enrolled client receives that scope;
  the selected tag does not determine synchronization participation (D248).
- Client working edits are durable and may coalesce. The complete record,
  including its tag set, is the comparison unit. Equal results and one-sided
  changes do not prompt; differing changes on both sides require client-side
  field resolution. Deletion is a comparison state (D59/D136–D138/D243).
- Retain original values, local working values and their actual comparison
  basis. Unrelated synchronization must not advance that basis. Unresolved work
  remains local while unrelated records synchronize (D136/D156).
- Shared acceptance atomically revalidates the compared record and relevant
  dependencies. The server neither resolves business differences nor maintains
  unresolved business alternatives (D141/D146).
- Use dynamic version vectors and exceptional timeline ancestry. Ordinary
  restarts create neither new actors nor new timelines. No per-edit history
  graph, storage quorum or automatic field/tag merging is selected (D151/D154).
- Authorization lasts indefinitely until explicit revocation. Revocation closes
  new automatic admission under the old actor. Reauthorization preserves local
  data and requires manual handling of excluded work (D145/D152).
- Retain ordering evidence through the oldest authorized durable acknowledgment;
  compact earlier evidence into an equivalent baseline. Preserve surviving
  references and local pending comparison evidence (D155/D156).
- Recover surviving accepted state in either direction. Missing transfer history
  permits fuller-state reconciliation. Missing originals and unavailable order
  have different, explained recovery fallbacks (D144/D147/D148/D153).
- Restore publication follows the operator's upload pause/catch-up/explicit-resume
  workflow. Synchronization remains available, and unavailable clients do not
  hold publication hostage (D141).

Record values and equality come from the [native model](../../native-model.md).
Catalog recognition changes do not become record edits. Provider evidence is
separate from recorded QSO facts; issue #3 supplies its concrete evidence
operations. This proposal does not force directory snapshots or immutable
service facts through the mutable-record conflict policy.

## 2. Research mechanisms and proposed composition

A **version vector** maps writer identities to counters. Here its comparison
domain is one record: a larger component covers that writer's earlier versions
of this record. It does not claim receipt of other records. A **causal context**
is the known predecessor information for a version. **Coverage** below means a
complete accepted-history prefix represented by retained state or its successors.
A **baseline** is a durable common-history summary used to replace older detail.

| Source mechanism | Use in this proposal | Boundary of the published result |
| --- | --- | --- |
| Ordinary per-object version vectors, contrasted with shared predecessor vectors in [Malkhi and Terry, 2005, §§1–4](https://malkhi.com/files/WinFS-version-vectors-DISC2005.pdf) | Explicit vectors for record causality; no implicit borrowing of a changing account vector for pending work | Factoring with exceptions is an alternative representation, not a requirement |
| Explicit made-with knowledge and transactional delivery in [WinFS, 2006, §§5.1.4–5.2.4](https://www.microsoft.com/en-us/research/wp-content/uploads/2006/06/tr-2006-78.pdf) | Preserve a version's basis; atomically install records, comparison evidence and progress; recognize already-known versions | Clacks uses its accepted-state/client-pending policy rather than WinFS's shared conflict storage |
| Closed-writer knowledge in [Petersen et al., 1997, §4.3](https://www.csd.uwo.ca/~hlutfiyy/Perv/Papers/petersen.pdf) | Omitted writer information needs an interpretable meaning | Bayou's recursive identities and cooperative retirement are not copied; D152 supplies Clacks's admission cutoff |
| Registered-server clock identities in [Almeida et al., 2014](https://asc.di.fct.unl.pt/~nmp/pubs/dais2014.pdf) | Distinguish who requests an update from the server's role in version creation | Does not establish rollback safety or select Clacks's clock ownership |
| Peer-witnessed counter regression and its limits, examined in the [recovery-fit analysis](../../../references/replication-research/recovery-fit.md) | Detect contradictory history and establish an exceptional timeline before further accepted writes use that history | Not a complete detector of arbitrary writable restoration |

The fault model is authenticated, protocol-conforming replicas with crashes,
interruption and consistent older server backups. Metadata checks detect specific
contradictions; they are not a proof against arbitrary fabricated or corrupted
replica state. Account authorization is still enforced at every admission.

The engineering composition is: explicit record vectors, a server acceptance
component, complete account-prefix checkpoints, and sparse timeline coverage
boundaries. The baseline/projection rules below are a Clacks adaptation to D154–
D156, not a theorem inherited unchanged from any source.

### Why explicit record vectors

Choose explicit vectors over shared knowledge plus per-record exceptions for the
initial implementation. Their direct comparisons are easier to inspect, and
Clacks's expected writer population is modest. The cost is repeated metadata
across records. Client components persist only while required by retained
history, active admission or pending evidence; they are not an irrevocable
lifetime list on every QSO. Timeline summaries factor accepted history when
collecting retired components.

This choice does not prevent a later representation optimization preserving the
same comparison and coverage contracts. It avoids implementing the more complex
WinFS compression rules before measurements justify them.

## 3. Concrete logical metadata

These are logical records and invariants, not prescribed Rust signatures or SQL
tables. SQL means Structured Query Language. IDs use the project's existing
identity conventions; their timestamp/order is not causal evidence. Counters are
checked unsigned 64-bit integers. Exhaustion is an explicit error; never wrap or
reuse a version number.

### 3.1 Identities and positions

| Item | Fields and meaning |
| --- | --- |
| `AccountId` | Existing account boundary; every lookup and comparison is account-scoped |
| `RecordKey` | Native record kind and its stable ID; deletion does not free that identity for reuse |
| `ClientActorId` | Current enrolled-client admission identity; also the client's vector key. Reauthorization allocates a fresh admission identity |
| `TimelineId` | Identity of one accepted server history within an account |
| `Position` | `(TimelineId, revision)`; revisions increase through durable accepted mutations and synchronization metadata changes |
| `ClockActor` | Either `Client(ClientActorId)` or `Server(TimelineId)`; the server has an explicit vector component |
| `RecordVector` | Map from `ClockActor` to counter, interpreted only for the containing `RecordKey` and compatible baseline |

A client counter is scoped to `(account, record, client actor)`. Preparing an
outgoing version advances that counter durably. Choose one greater than both
the durable local high-water mark and the actor's counters in the current basis
and observed accepted context. It need not advance for every local edit. A server component uses its account acceptance revision; this can
skip numbers on an individual record. For that record it covers earlier server
versions in the same timeline, not other records' transferred payloads.

The server retains its current timeline on an ordinary process restart. On an
established recovery fork it uses `Server(new timeline)` for subsequent versions.
Old server components remain interpretable through ancestry and baselines.
There is no fresh server identity on every startup.

### 3.2 Accepted record envelope

| Field | Meaning |
| --- | --- |
| `key` | Account-qualified native record identity |
| `state` | Complete live native value, or explicit deletion |
| `accepted_at` | Position where this logical version was accepted |
| `baseline` | Baseline under which the residual vector is interpreted |
| `vector` | Explicit record context, including the version's client contribution and server acceptance component |

An envelope transferred during recovery retains its original `accepted_at` and
logical version. Installing an old accepted envelope into another timeline is
not a new operator edit. A later reconciliation choosing a new result gets a
new accepted position and context covering the versions it reconciles.

A bounded transfer journal separately associates installation/change activity
with the current timeline's positions. Thus a journal entry at `T2:7` may install
an envelope originally accepted at `T1:180`. Transfer position and original
version position are not interchangeable.

Equal recorded values do not imply equal causal metadata. Combining known
versions may update metadata without changing native values. Such bookkeeping
must not create a recorded-fact conflict or an external upload by itself.

### 3.3 Client state

| State | Contents |
| --- | --- |
| `accepted` | Latest durably incorporated accepted envelopes |
| `pending` | Record key, original accepted values or known absence, original version/context, and current local working state |
| `prepared` | Optional frozen outgoing value, expected accepted version/dependencies and its allocated client vector component |
| `writer_counters` | Durable high-water marks preventing reuse of prepared client versions |
| `installed_coverage` | Complete account prefixes and baseline durably incorporated by this client |
| `receive_progress` | Source checkpoint/range and exactly which required envelopes/metadata were durably installed for an incomplete transfer window |

`prepared` is part of pending synchronization state, not a separate RPC receipt
log. RPC means remote procedure call. Its identity comes from the record vector.
Changing the editor after transmission changes `pending.working`, not the bytes
or context of that already-transmitted version. Keep at most one unsettled
prepared version per record. Local editing and other records' synchronization
continue. Settle it by observing incorporation, a terminal admission failure, or
a changed accepted precondition that makes a delayed copy unable to apply.
Until then, retry that same version; later local edits remain coalesced working
state. Retain its values until settlement, so later edits have the correct
comparison basis. No permanent list of previous submissions is required.

If an accepted current vector covers a prepared own version whose acceptance
response was lost, retain that prepared value as the basis for subsequent local
work, with the current accepted envelope as its causal witness. Its exact server
acceptance position may remain unknown; do not invent one. This is enough to
compare the subsequent local edit with current shared values. It is not an
assertion that the current shared value still equals the prepared value.

Unavailable original evidence, a known never-created record and an accepted
deletion are distinct. Ordinary pending edits retain their originals. The
unavailable case is a recovery condition, not a normal incomplete saved record.

### 3.4 Account coverage and lineage

`Coverage` maps timeline IDs to completely represented accepted prefixes.
For example `{T1:200, T2:40}` is not shorthand for revision 240. Different
coordinates are different histories. A component is claimed only when its
records, deletion information and applicable shared evidence are present or
represented by retained successors. A number advertised by a peer is a transfer
target, not permission to install that coverage immediately.

`TimelineBoundary` contains:

- `at`: the position in the receiving timeline where the relationship holds;
- `includes`: the older timeline prefixes established by that point;
- `kind`: fork origin or subsequent recovered coverage.

For example, `(at=T2:0, includes={T1:160})` records the actual restored origin.
A later `(at=T2:41, includes={T1:200})` records completed recovery. Positions
`T2:1` through `T2:40` do not inherit the later coverage retroactively.

A record vector is record-specific; account `Coverage` is account-wide. Use
separate types and operations for them. Joining a record's vector must never
advance account coverage.

### 3.5 Membership and baseline

`ClientMembership` contains the current admission identity, authorization state,
creation/closure positions and last durable acknowledgment. Pending work is not
stored in that server membership record. Authorization remains an independent
check even when a causal comparison establishes that data is newer.

A local `BasisWitness` identifies the record, original accepted position or own
prepared version, an observed accepted version and checkpoint, the relationship
established at that observation, and the lineage summary needed to interpret it.
It accompanies the retained original values; it does not replace them. A witness
of succession through one checkpoint says nothing about a future branch that
does not cover that checkpoint.

`Baseline` contains an ID, certified common `Coverage`, the lineage summaries
needed by surviving positions and the membership version used to select that
coverage. It replaces old detailed ordering information, not current payloads,
deletions, service facts or client pending work. Enrollment initializes against
an available baseline; a new client's acknowledgment cannot certify an
uninstalled snapshot.

### 3.6 Initialization and state discovery

Create an account's initial timeline T1 at revision zero with an empty-history
baseline. Seeded shared definitions are ordinary server-authored accepted
records after that point, not data separately installed on each enrolled client.
Enrollment starts from an empty accepted replica and an available baseline plus
consistent full-state window. Installing that window establishes complete
account coverage; an advertised head alone does not.

For an eligible incremental checkpoint, the bounded journal identifies changed
record keys and the consistent source view supplies their current versions or
successors. The journal need not retain every intermediate payload. When it is
unavailable, compare exact, ordered manifests of current accepted record keys,
versions and deletion descriptors, then transfer the missing/different state.
A hash tree is not required. A manifest lacking deletion/absence semantics is
not a complete state comparison. The same operations serve either transfer
direction; only new-change admission is specific to the acceptance server.

## 4. Version preparation, comparison and acceptance

### 4.1 Coalescing local work

Saving edits changes local durable working values. It neither claims server
acceptance nor requires a historical event per save. After reconciliation makes
an outgoing value ready, prepare it from its actual record context and reserve
the next client counter in the same durable local transaction. Never relabel an
old edit with an account checkpoint received for unrelated records.

A candidate based on `{Client A:4, Server T1:200}` prepared by client B may have
`{Client A:4, Client B:1, Server T1:200}`. These counters concern this record.
The server's subsequent acceptance adds its new revision, for example
`Server T1:201`. The client already knows `Client B:1` even if the response is
lost.

### 4.2 Comparison sequence

1. Validate account, record identity, native state and metadata consistency.
2. Establish compatible baseline/lineage interpretation. A collected component
   is not an unexplained zero. If required interpretation is unavailable, return
   an explained ordering-unknown result.
3. For accepted copies, use valid timeline coverage and record vectors to
   establish equality, succession or divergence. Record vectors can prove a
   particular candidate is incorporated even when account-wide coverage has not
   advanced. A missing global ancestry edge is not proof against that positive
   record-specific evidence. Establish record divergence only after considering
   the complete compatible record context. Do not infer account coverage from
   a record-specific proof.
4. Compare ordinary pending original/local/current values under D137/D138.
   Causal succession does not grant a pending edit permission to overwrite a
   changed current record. Equal results and one-sided changes skip resolution.
5. Resolve genuine differing two-sided changes on the client. When a previously
   transmitted own version is now known to be incorporated, preserve later work
   relative to those transmitted values instead of treating the own version as
   another operator's competing edit.
6. Submit only a validated result with current acceptance preconditions. Unknown
   order or missing originals follows the corresponding accepted recovery flow.

Componentwise maximum combines compatible vector context; it does not merge
native fields. Incomparable complete vectors can establish causal divergence.
An incomplete interpretation produces `Unknown`, not an invented concurrency
claim. Timeline and vector evidence that contradict each other is invalid
metadata and must be diagnosed, not resolved by choosing whichever says newer.

The production comparison returns the following distinct results:

| Result | Evidence and next transition |
| --- | --- |
| `SameVersion` | Compatible context identifies the same version and state; no repeat mutation |
| `KnownOrSuperseded` | Current record context covers the transmitted client version or accepted version; preserve any later local work |
| `Precedes` / `Follows` | Interpretable vector/timeline evidence establishes succession for this record |
| `Divergent` | Complete, compatible evidence establishes that neither version covers the other; apply the existing value policy on the client |
| `OrderingUnknown` | Required interpretation is unavailable; preserve both inputs and explain uncertainty for differing values |
| `InvalidMetadata` | Identity/payload, vector or lineage evidence contradicts its invariants; preserve inputs and enter metadata recovery |

Value equality still bypasses business resolution when ordering is unknown.
It does not authorize fabricated ancestry or account-prefix coverage. Preserve
available context for metadata reconciliation; unrelated progress can continue.

### 4.3 Conditional acceptance

The client supplies the expected accepted envelope identity/context and relevant
reference preconditions. These use stable metadata, not a content hash or local
wall-clock time. A conditional create requires certified absence, not merely a
missing row in an incomplete client snapshot.

In one actual storage transaction the server must:

1. recheck admission and the expected record/reference state;
2. allocate the next account revision;
3. join the approved record contexts and add `Server(current timeline):revision`;
4. save the validated value/deletion, its envelope and transfer-journal change.

A mismatch returns current accepted state for client reconciliation. The server
does not choose fields. A stale prepared version already covered by the retained
record context is not accepted again. Return current accepted state/context;
there is no requirement to reconstruct the original response.

Use per-record acceptance by default. Coupled effects, such as a tag-deletion
operation and its selected contact effects, require explicit dependency and
transaction grouping in the consuming workflow. Nothing here promises one
account-wide commit or a custom write queue. PostgreSQL/SQLite integration later
implements these transaction obligations using ordinary database facilities.

For two different accepted versions with equal native values, the client can
propose recording their combined context without a field-resolution flow. The
server conditionally records that context under a new accepted position; this
is bookkeeping, not a new user edit. Its per-record context may cover a recovered
old version before full coverage of that old account prefix is established.

## 5. Interrupted transfer and durable acknowledgment

Choose explicit bounded transfer windows with completion tracking, rather than
an account prefix that jumps to the highest record counter received. Each window
identifies its consistent source checkpoint, required record versions/deletions,
lineage/baseline metadata and completion boundary. Exact enumeration may be
paged; completing a page is not by itself completion of the whole window.

For an incremental window, enumerate the affected records and their required
current versions or successors. A consistent full-state window also certifies
complete key enumeration and deletion/absence meaning. Its required set cannot
silently change while pages are being installed. If its source snapshot/journal
is no longer available, negotiate a new window and reuse already-known versions.

Install each bounded batch and its progress durably. On a client, install the
accepted envelope even if local work conflicts, preserving the original and
working values separately. This lets an unresolved business conflict coexist
with acknowledgment of the received shared version.

Before declaring a window complete, verify that every required version is
present or covered by a retained successor and all required metadata is durable.
Then atomically install the resulting coverage/checkpoint and preserve pending
comparison evidence. Only afterward acknowledge it. No new global acknowledgment
round is added. A crash before the commit leaves the previous checkpoint; a
lost acknowledgment allows safe repeat transfer.

The partial-window version set is completion bookkeeping, not a second causal
history. Unlike WinFS's fine-grained account exceptions, it does not advertise
partial receipt as a complete prefix. The tradeoff is extra repeated enumeration
when a source window must be replaced. Durable installed records/progress still
allow useful work across interruptions; a whole-account transaction is not
required.

The same mechanism transfers accepted state client-to-server during recovery.
Pending local proposals remain a distinct admission path. A divergent recovered
record stays in client review until resolved; unrelated recovered records and
new-timeline traffic continue. Do not claim the recovered old prefix complete
while one of its required accepted versions remains unrepresented.

## 6. Retirement and compaction

### 6.1 Revocation closes admission

Commit revocation with its account position and exclude the old actor from new
automatic proposals. A delayed pending proposal is not admitted because its
counter is high. Previously accepted versions remain shared; their author's
current admission status does not change their accepted status.

Retire active writer/admission bookkeeping as its obligations end. Keep clock
components still required by retained ordering evidence; they are historical
context, not authority to submit. No revoked device cooperation is needed.
Historical components can remain until the existing D155 boundary makes their
projection safe. This adds no new acknowledgment barrier beyond that selected
retention rule.

### 6.2 Selecting the common boundary

Expand acknowledgments through known timeline boundaries and baseline summaries.
Take their common covered prefixes, also bounded by the server's own installed
coverage. For normalized vectors this is a componentwise minimum in each
*same-timeline* coordinate. Missing certification is not coverage. Never take a
numeric minimum of revisions from unrelated timelines.

An indefinitely offline authorized client can hold that boundary back. Revocation
removes it from the participant set. Account coverage must satisfy the
[complete-window rules](#5-interrupted-transfer-and-durable-acknowledgment).
Membership and baseline publication must be checked atomically so enrollment,
revocation or concurrent synchronization cannot invalidate the chosen boundary.

### 6.3 Projecting old clock components

After the common boundary covers a closed actor's admitted history, replace its
older vector components with the common baseline interpretation. Keep each
record's actual accepted position; do not make it appear authored at the baseline.
Keep any components or lineage intervals still needed to interpret surviving
versions. If projection cannot preserve a required comparison, retain that
metadata instead of deleting it.

All ordinary participants must be able to interpret the old baseline until they
install the replacement. A client atomically preserves its pending originals,
working values, relevant prepared versions and their relationship to the
incorporated accepted state before advancing acknowledgment. For an original
below the new boundary it may retain:

- the complete original native value/deletion and its actual accepted position;
- the accepted version/checkpoint which established that original as known or
  superseded, and the relevant lineage summary;
- any residual vector information not explained by that relationship.

This is sufficient evidence for those particular comparisons, not a relabeling
of the pending edit as based on the new checkpoint. A later state covering that
checkpoint preserves the established succession relationship. Relationships
outside the witness's retained context are not established by that witness.

Apply projection as the following explicit transition:

1. Select common coverage K using current membership and installed coverage.
2. Select only closed client actors whose closure position is covered by K.
   Any actor closed after K, or still needed by a surviving comparison, remains.
3. For each current accepted envelope, establish that its accepted history
   represents this record's K-prefix. Keep its actual accepted position and all
   vector components outside the selected closed set. Replace the selected
   components with a reference to baseline K; do not interpret their absence as
   numeric zero. An envelope that cannot establish that relation is not projected.
4. Normalize received inputs before comparing clocks. A compatible current
   envelope can use that baseline interpretation. An older accepted input covered
   by K is known or superseded, not a fresh candidate. An input beyond K needs its
   residual vector/lineage; missing context returns `Unknown`.
5. Do not apply step 3 to a pending original as if it were the current accepted
   head. Preserve its actual values and position. Before releasing its old
   context, save a witness identifying the observed accepted record version,
   observed coverage and the established equality/succession relationship.
   Preserve residual context for a relationship that the witness cannot explain.
6. Only after all affected durable references are interpretable under K publish
   the replacement baseline and allow physical removal. A client whose local
   conversion is incomplete keeps its earlier acknowledgment and evidence.

The server need not store a witness for every private pending edit. The client's
existing durable-acknowledgment obligation preserves those witnesses locally.
If a future accepted head covers the witness's checkpoint, the known predecessor
relationship remains valid. Otherwise, the comparison requires additional
retained lineage/context rather than extrapolating from the cached relationship.

Keeping an old baseline ID solely as an opaque pointer is insufficient. A live
reference must retain or acquire its interpretation before the backing detail is
removed. This applies to old prepared versions as well as original values.

The baseline retains a sparse, piecewise description of timeline ancestry:
coverage added at position P applies from P onward, not before. Earlier detailed
transfer entries can be removed. Retain nonredundant lineage intervals referenced
by current envelopes, acknowledged checkpoints or pending evidence that must
remain server-interpretable. Summarize equivalent intervals; do not replace an
old interval with a later coverage maximum. Timeline IDs still named by surviving
records need not be forcibly collected. Their growth follows exceptional
histories, not client turnover or every edit.

Baseline installation precedes physical cleanup. A crash before installation
keeps the old baseline authoritative; afterward, redundant old detail can be
removed incrementally. Revalidate membership, coverage and the expected versions
of touched envelopes at installation. Concurrent changes must be retained or re-evaluated, not
overwritten by an obsolete projection plan. No extra all-client round is required.

Deleting ordering detail does not delete current record envelopes, QSO deletion
memory, external associations or service-processing evidence. Their separate
obligations remain. Issue #3 extends concrete evidence types; compaction may not
assume that ordering stability alone makes those facts disposable.

Rollback and restoration examples are in
[section 8](#8-rollback-and-restoration).

## 7. Worked transitions

The examples use short actor/timeline labels for readability. All operations are
within one account. `S1:n` denotes a server component for timeline T1; `B:n`
denotes client B's component on the record being discussed.

### 7.1 Response lost, then further local editing

| Step | Durable client state | Accepted server state |
| --- | --- | --- |
| Initial | Original/value A, record vector `{S1:200}` | A, `{S1:200}`, position T1:200 |
| Prepare | Frozen B with `{S1:200,B:1}`; working B | Still A |
| Accept; response lost | Frozen B remains; no acceptance is assumed | B, `{S1:201,B:1}`, position T1:201 |
| Operator edits | Working C; frozen B and its context remain | Still B |
| Reconnect | Received B's vector covers client version B:1 | No second acceptance of B |
| Continue | B becomes the established basis for the remaining B→C edit | C may be proposed against current B |

No separate request ID or historical RPC response is needed. If another client
has meanwhile produced D at T1:202, its inherited context still includes B:1.
The originating client compares original B, local C and shared D. If local work
is still exactly B, it simply takes D; it must not prompt for its own completed
A→B change. If C and D differ after both changed B, ordinary resolution applies.
If the server never accepted B, its current context does not cover B:1; the
client retains the original A basis and proceeds through ordinary comparison.

### 7.2 Interruption halfway through catch-up

A client has acknowledged T1:40. A source window through T1:60 requires latest
versions of QSO Q, tag H and profile P, with their predecessor coverage.

1. The client commits Q's accepted version at T1:60, preserves any pending edit
   to Q, and records that Q's required descriptor is installed.
2. The process crashes before H and P arrive. The account checkpoint is still
   T1:40. Q's high component does not certify the other records.
3. On restart, Q remains durable and is recognized on repeat delivery. The
   remaining required versions are transferred or replaced by valid successors.
4. Once the complete window and metadata are installed, the client commits
   checkpoint T1:60 with its pending comparison evidence and acknowledges it.

If its pending Q edit began at T1:20, that original value/context stays at 20.
An unresolved Q conflict does not prevent H/P installation or checkpoint 60.
If the acknowledgment is lost after the commit, safe repeat transfer suffices.

### 7.3 Revoked writer with an authorized offline client

Client A authored an accepted QSO version at T1:60. Client B acknowledged T1:40
and went offline. Client A has an additional unsynchronized edit when revoked
at T1:80.

1. Admission under A stops at the revocation transaction. Its unsynchronized
   tail is not automatically eligible, and A need not return or acknowledge.
2. The accepted T1:60 version remains shared. Server ordering and retained
   vectors distinguish it from the excluded tail.
3. B's old acknowledgment still holds the normal retention boundary at 40.
   Retiring A does not expire B or remove evidence B still needs.
4. B returns and supplies its old original/local values. If it also edited this
   QSO, compare them with the accepted T1:60 state under the ordinary rules.
5. Once the common boundary passes 80, A's historical clock components can be
   folded into the baseline where surviving references permit. There is no
   remaining active clock dimension merely because A once authored a QSO.

If A returns, reauthorization gives it a fresh admission identity and preserves
its excluded work for explicit reconciliation.

### 7.4 Compaction while a conflict remains pending

Server and clients have valid T1 history. The server is at 950; authorized
acknowledgments are 900, 700 and 300. The common boundary is 300.

Client B has a pending QSO edit with original A at T1:200, working B and latest
accepted shared C at T1:600. It can acknowledge 700 because it durably retains
those values and their actual relationships. Its acknowledgment does not change
its original from 200 to 700.

1. Build a baseline through T1:300, verifying current membership and surviving
   references. Client B's preserved original is still an accepted predecessor
   of C at 600; the values A/B/C are sufficient for its field comparison.
2. Durably install the baseline, then remove eligible older journal/detail and
   closed-actor components. Preserve current envelopes and deletion/service facts.
3. A later shared D at T1:960 succeeds C. B compares A/B/D without needing the
   discarded intermediate edit payloads or a retired actor component.
4. If the client at 300 remains offline, the boundary stays at 300. If it catches
   up or is revoked, recompute from the remaining authorized acknowledgments.
5. A crash during cleanup leaves either the old baseline, or the new baseline
   plus redundant old detail. Neither case discards pending values.

## 8. Rollback and restoration

This section applies the retained metadata to restored server state. The
[retirement and compaction rules](#6-retirement-and-compaction) define which
metadata remains available.

### 8.1 Older server, complete recovery, then a still-newer client

A server backup is at T1:160. Client A retains complete accepted state through
T1:200. The restored server advertises T1:160 to A.

1. A's durable knowledge establishes a regression. Preserve both copies and
   install T2 with an origin boundary `(T2:0 includes T1:160)`. Subsequent server
   components use S2. This is an exceptional recovery, not an ordinary restart.
2. A transfers accepted envelopes, deletion information and applicable evidence
   through the normal complete-window mechanism. Recovered envelopes retain
   their original positions and vectors. Installation has T2 journal positions.
3. If every required old version is present or represented by a retained
   successor, commit `(T2:41 includes T1:200)`. Seeing the number 200 alone never
   authorizes that boundary.
4. A client last at T1:199 can establish covered history through that boundary
   and continue normal reconciliation, including its pending local work.
5. A client with T1:201 has evidence beyond the established prefix. Compare its
   surviving state; T2's larger numeric revision is not proof of coverage.

If T2 had accepted a genuinely competing QSO change after the restore, the
recovering client detects that relationship using record context, compares
values and resolves differing two-sided changes. Until the older version is
represented by an accepted result, the server must not declare the whole old
window covered. Unrelated T2 synchronization continues. The unresolved proposal
stays client-local; the server does not gain a business-conflict queue.

An equal-value result can combine per-record context without a user prompt.
That combines this QSO's histories; it still does not certify all other records
through the old prefix.

### 8.2 Recovering a version authored by a revoked client

Client A submits a version at T1:60; the server accepts it and sends it to
still-authorized client C. A is then revoked. If the server later restores a
backup predating T1:60, C can return the server-distributed accepted envelope
during normal recovery. The path is A → server → C, followed later by
C → restored server. C never obtains the version directly from A.

A's revocation does not turn this retained accepted version into a new proposal
from A. C cannot relay A's excluded unsynchronized tail, which the server never
accepted or distributed. The returning envelope follows the ordinary accepted-
state recovery checks and retains its original acceptance context.

### 8.3 Backup predating a collected baseline

The current common baseline is through T1:700; the server backup is T1:160.
A surviving client has current records and local pending evidence but some old
ordering detail has been collected.

Transfer the retained baseline and current accepted/deleted state through a
complete recovery window. For positions whose relationships survive in that
baseline or client evidence, reconcile automatically. If differing records have
no interpretable ordering, return an explicit ordering-unknown review. If
concurrency is established but the common original payload is unavailable, use
the distinct two-way resolution flow. Do not call missing evidence concurrency,
treat a missing row as a proven deletion, or discard local pending work.

The old backup may also predate revocation. No mechanism here promises to recover
a revocation or service outcome absent from every surviving source. Uploads
remain under the operator-controlled restore pause/resume workflow.

### 8.4 Detection limits and metadata validation

Implement these concrete contradiction checks:

- the same timeline advertises a revision below a durably observed position;
- the same record/version identity has different native states;
- lineage claims create a cycle, change an established boundary, or retroactively
  extend coverage to earlier positions;
- metadata claims coverage while the required record/deletion/successor is
  missing from a completed transfer;
- record vector and accepted-position relationships contradict each other.

A contradiction invalidates the affected comparison domain; preserve data,
establish a fresh timeline for subsequent accepted writes and reconcile under
D147/D148/D153. Do not fabricate missing coverage or stop accepting unrelated
safe work merely because a client has a business conflict. Actual transaction
integration must ensure that once a recovery boundary is installed no accepted
mutation races ahead using the invalidated server component.

These checks are not a universal rollback detector. If a writable restored server
reuses counters and advances past available witnesses before a contradiction is
observed, a revision-only handshake can miss the rollback. Vector/payload
contradictions may expose additional cases, but this proposal makes no guarantee
that every such restoration is detected. The [existing recovery analysis](../../../references/replication-research/recovery-fit.md)
records that limit. Timeline ancestry prevents old/new history from being
conflated after the fork is established; it does not detect a fork by itself.

A consistent database restore, normal restart and copied desktop installation
are distinct situations. No per-start actor renewal, mandatory machine detector,
new storage quorum or built-in backup command is introduced here. These server
recovery rules do not claim to solve concurrent writable copies of one desktop
identity; that remains part of the deferred explicit client restore design under
D240. Returning clients incorporate their observed own counter high-water marks
before preparing new causal versions.

## 9. Production boundary and validation obligations

Issue #2 delivers the production metadata types and transition logic for:

- record comparison and context combination under the existing conflict policy;
- preparation, incorporated-version recognition and conditional-acceptance plans;
- complete-window installation/progress and durable acknowledgment preconditions;
- timeline coverage comparison, recovery-boundary validation and fuller-state plans;
- membership-sensitive retirement, baseline planning and safe metadata projection.

Use actual native QSO/tag/profile/equipment values in verification. Tests drive
these production operations; there is no separate simulated synchronization
engine. Transaction requirements are explicit outputs/preconditions consumed by
real stores, not a substitute database. Issues #6/#14/#16 add SQLite/PostgreSQL
transactions, WebSocket sessions and integrated recovery checks without replacing
this logic. Issue #3 extends evidence payloads and their dedicated operations.

The implementation specification must turn the following obligations into
meaningful tests and transaction contracts:

1. A record vector never certifies another record or an account prefix.
2. A completed window has every required version or a retained successor.
3. Checkpoint advancement preserves pending values and their actual basis.
4. Covered own versions are not replayed or mistaken for another client's edit.
5. Conditional acceptance cannot race past record/reference changes or revocation.
6. Projection preserves every comparison required by ordinary authorized return.
7. Missing older context produces an explained unknown result, not a false order.
8. Retirement never admits an excluded tail or deletes accepted contributions.
9. Compaction changes no native facts, deletion meaning or publication outcome.
10. Boundaries establish coverage at their actual installation positions only.

The worked cases show the intended transitions. In particular, baseline
projection is the application-specific part requiring careful implementation and
validation; published vector algorithms do not prove it merely because the
metadata contains a map of counters. Review this proposal before preparing the
implementation plan or writing product code.

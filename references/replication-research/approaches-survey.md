# Distributed replication approaches, data models, and data structures

Research redo: 13 October 2026, under D150. The requested unit of analysis is a published approach or abstract model/structure, not a pluggable filesystem, database, or synchronization framework. Software examples may provide implementation or measurement evidence. No implementation or new synchronization protocol is selected.

The [recovery fit audit](recovery-fit.md) distinguishes optimistic/manual repair under D153 from the stronger continuously safe storage problem. No quorum requirement follows for Clacks; overall-revision regression detection is a simple proposed witness mechanism with an explicit counter-catch-up limit. The parent-version-graph recovery suggestion has been removed.

The [recommended combination](recommended-model.md) prioritizes conservative record reconciliation with dynamic version-vector metadata, immutable service evidence, bidirectional anti-entropy, and conditional acceptance. It is a proposal rather than an accepted decision. The comparisons below support that recommendation.

## Research reports

- [Causality and knowledge representations](approaches-causality.md).
- [Reconciliation and consistency models](approaches-reconciliation.md).
- [Convergence, dissemination, and retention](approaches-convergence-transfer.md).

## Reading the comparison

An **actor** is an identity whose updates are counted separately; it is not necessarily the same thing as a login or authorized device. A **replica** is a stored copy of the data. **Causal order** means one change was made with knowledge of another; **concurrent** changes were made without either knowing of the other, even if their wall-clock times differ. A **causal context** summarizes that knowledge.

For example, two clients start from the same contact. One changes its comment and the other changes its signal report while offline. Their edits are concurrent; Clacks's whole-record policy requires resolution. If both instead reach the same complete record, no resolution is needed. A clock or history helps establish relationships, but does not itself dictate that policy.

**State-based** transfer sends resulting values; **operation-based** transfer sends commands or effects. A **delta** is a fragment of state or a described difference; its precise merge rules depend on the model. **Anti-entropy** compares replicas and repairs missing information; **dissemination** is its delivery to other copies. A **Merkle tree** is a hierarchy of hashes used to find differing data ranges.

A **join** combines represented information according to a specified rule. For an immutable set of observations, it can be ordinary set union. A **tombstone** is retained evidence that an item was deleted, so an old copy is not mistaken for a new addition. **Compaction** replaces detailed metadata with a smaller summary; **garbage collection** discards information that is no longer needed under the model's assumptions.

“Orthogonal” below means approaches perform different jobs and may be combined. A **register** is a replicated slot for a value; a **map** associates keys with values. **Visibility** specifies which changes a read can observe, whereas **arbitration** supplies an order for competing operations. **Commitment/admission** concerns acceptance into shared state, not simply receipt of a message.

## Compare orthogonal dimensions

A single list cannot fairly rank a version vector against a state-based conflict-free replicated data type (CRDT) or a Merkle tree. They answer different questions:

1. **Logical replicated state:** current value plus common state; causally versioned values; operation/dependency logs; lattice state; or an abstract event/transaction model.
2. **Reconciliation semantics:** conservative state comparison, dependency validation, semantic scheduling/transformation, or a join/commuting-effects rule.
3. **Consistency contract:** which events/transactions are visible and ordered, what a client may observe, and what accepted versus local pending state means.
4. **Causality/knowledge representation:** vectors, dots and contexts, factored shared knowledge, interval trees, bounded stamps, or explicit causal graphs.
5. **Dissemination:** full state, operations, deltas, summary-driven anti-entropy, and gap-safe catch-up; indexes/sketches optimize discovery rather than choose values.
6. **Retention/lifecycle:** stable prefixes, causal stability, actor retirement, snapshots/compaction, and persistent deletion/processed-effect knowledge, each under stated assumptions.

[Saito and Shapiro's optimistic replication survey](https://dsf.berkeley.edu/cs286/papers/optimistic-csur2005.pdf) supplies the broad state/operation, ordering, conflict, and propagation taxonomy. The new analysis adds the formal structures and modern convergence/consistency work needed to compare those dimensions directly.

## Properties applied to the conceptual comparison

These come from the current decision register; they do not select a representation:

- Offline independently writable enrolled clients in a server star, with arbitrary client count and coalesced working edits.
- Whole-record original/local/shared comparison: one-sided or equal results skip resolution; differing both-sided results use client-side field choices, including disjoint field edits. Unresolved proposals remain client-local.
- Atomic acceptance against the relevant state compared; a changed condition returns to client comparison.
- Normal symmetric reconciliation of surviving synchronized record, deletion, and service evidence into an older restored server. Facts lost everywhere cannot be recovered.
- Indefinitely old, unrevoked clients reconcile normally; unavailable transfer history permits fuller-state reconciliation. Under D152, revocation closes automatic acceptance from the actor. Recovered unsynchronized work requires manual reconciliation, with retained data preserved under D145.
- Genuine recovery divergence may use two-way comparison if originals are absent; unavailable ordering may require explicitly explained conservative resolution. Ordinary pending originals remain durable.
- Deletion conflicts and reference dependencies between account-owned radio contacts (QSOs) and tags, including explicit tag-deletion effects and pending references to deleted tags; external-record association uniqueness and observation deduplication remain separate invariants.
- Server-owned external jobs; operator-controlled all-upload pause/resume during restore. Authorization is rolled back if the only authorization state is rolled back.

## Consolidated comparison of approach families

The three analyses have been reviewed. The following rows compare approaches; they are not product names, and several occupy different dimensions rather than being mutually exclusive choices.

| Approach / abstract model | Represented data and reconciliation | Relevance to the desired properties | Important assumption or limitation |
| --- | --- | --- | --- |
| Common-state conservative reconciliation | Current identity map/tree/relations plus last common state; compare changes on both sides, propagate unambiguous results, retain conflicting residual state | Directly describes the selected whole-record original/local/shared behavior and coalescing | Common state must be meaningful; a rollback can look like a new one-sided edit, and archive loss can confuse creation/deletion |
| Causally versioned state replication | Current values/frontier plus event/context knowledge; partial-order comparison identifies superseded or concurrent states | Studies how missing shared state and divergence are recognized without keeping every historical payload | Unique event generation and truthful coverage matter; causal concurrency is not itself the chosen user-flow rule, and pruned evidence may be incomplete |
| Optimistic operation replication / tentative certification | Operations, parameters, dependencies, execution/commitment state, materialized records; validate, order, replay, accept/reject | Expresses whole-record and aggregate dependencies and the distinction between local work and shared acceptance | Coalescing must preserve semantics; some source models discard/replay tentative state, and internal replay cannot undo external effects |
| Semantic graphs and operation algebras | Action/constraint graphs or inferred command sequences with commutation/equivalence laws | Explicit models for cross-record dependencies and conservative residual conflicts | Search/graph costs and domain proofs matter; mathematically compatible field commands may still violate the chosen whole-record policy |
| State-based, operation-based, delta-state, and pure operation-based convergence models | Join-semilattice state, commuting effects, joinable fragments, or partially ordered logs with redundancy/stability rules | Strong candidates for converging immutable evidence or specified datatypes; not limited to simple flags | Their merge/query semantics must match the replicated object; delivery/garbage collection (GC) proofs have explicit assumptions and do not supply business admission automatically |
| Transformation and shadow/diff reconciliation | Context-dependent operations/history or live/shadow/backup-shadow states and diffs | Alternatives for incremental state change and coalescing; useful contrasts with conservative whole-record comparison | Transform/patch laws and history context are domain-specific; some repair fallbacks permit lost changes and are unsuitable unchanged |
| Visibility/arbitration and consistency models | Abstract operations, observation relations, order relations, and return-value specifications | Separates locally visible pending work, accepted shared state, convergence safety, and eventual delivery | A specification is not a persistent data structure or a transport/recovery algorithm |
| Invariant confluence / monotonicity analysis | Invariants, operations, merge functions, and logical growth of facts | Tests whether automatic combination preserves the actual business constraints; distinguishes evidence growth from current-status projection | Referential integrity or byte convergence alone is weaker than preserving conflicting intent for user resolution |

Published definitions and source links are in the focused reports. Application fit in this table is inference; no row proves Clacks's complete contract or selects an implementation.

## Data-structure comparison inside those models

A **dot** identifies one update, usually by actor and counter. **Shared knowledge plus exceptions** stores common event knowledge once and records where individual records differ, rather than repeating full vectors everywhere. A **stable prefix** is an earlier range whose stability has been established under the source model; silence alone does not prove it.

The causality report supplies a separate comparison, rather than calling structures competing complete replication solutions:

- **Scalar/hybrid logical clock (HLC) timestamps:** compact order labels; they do not prove the converse of happens-before.
- **Vector/version summaries:** componentwise knowledge with different event/current-state scopes; record repetition and represented lifetime identities affect cost.
- **Dots and causal contexts:** distinguish an event from its past; causal dot-store joins also carry removal semantics.
- **Shared knowledge plus exceptions:** factor repeated record metadata while preserving incomplete-delivery holes; storage savings depend on workload/disruption.
- **Structured retirement, interval ownership, and bounded stamps:** actual reclamation/reuse mechanisms under different membership, ownership, or fixed-population assumptions.
- **Causal graphs/hash histories:** retained edges establish relationships; pruning can conservatively lose ordering, and content equality needs explicit treatment.
- **Encoded vectors:** alternative encodings preserve information cost; one integer is not necessarily bounded storage.

Transfer mechanisms such as Merkle indexes, set sketches, delta buffers, and journals are analyzed separately. Faster difference discovery cannot establish which value should be accepted. Transfer-log collection, deletion knowledge, processed-effect evidence, and actor identity retirement have different safety conditions.

## Results relevant to Clacks

1. The closest **semantic family** is conservative state-based optimistic reconciliation over whole records. That follows from the already chosen value-comparison rules, not from selecting a filesystem product. Causal/knowledge representation remains a separate comparison within the literature.
2. Automatic lattice/operation convergence is worth examining per object, particularly immutable service evidence. A multi-value record model can display conflicts, but directly putting unresolved QSO alternatives into shared state conflicts with their client-local location. “CRDT” alone neither fits nor rules out an application.
3. Coalescing working changes does not require permanent per-edit history. Operation models need semantic-preservation reasoning; state models still need original comparison values, deletion knowledge, and honest synchronization coverage.
4. Several proofs assume recovery of immediately pre-crash durable state. Fuller-state repair of missing volatile transfer history is not evidence that the same algorithm handles arbitrary older-backup restoration with reused counters. The delta-state report identifies that boundary in the primary algorithm rather than extrapolating from a library feature.
5. Garbage collection can move evidence, summarize it, collect redundant operations, or reclaim identities under a stated lifecycle. A small active replica count, a removed tombstone table, and an O(1) encoded label do not establish bounded total lifetime information. The accepted uncertainty fallback relaxes exact old comparison needs but does not justify false coverage or replaying old effects.

No per-client/server clock namespace, sparse baseline, persistent revision graph, or software implementation is selected. The next research-to-design step is to compare the relevant abstract representations under the intended event/current-state and failure scopes, using their published state/transitions and counterexamples. Implementation/framework selection can follow that analysis; it should not substitute for it.

## Best-fit approaches and researched mechanisms

This is a property-fit recommendation, not a selected implementation or new protocol.

| Role | Closest approach / abstract data model | Why it fits |
| --- | --- | --- |
| Mutable QSOs including tag sets, tag definitions, profiles and equipment | Conservative state-based optimistic reconciliation over identity-keyed whole records; separate original, working, and synchronized values | Expresses selected whole-record comparison, coalesces local edits, preserves unresolved proposals locally, and permits symmetric state exchange |
| Ordering/coverage attached to synchronized state | Causally versioned records; recommend dynamic version vectors as the baseline, with dotted/context or factored-knowledge refinements if warranted | Distinguishes events from observed past and preserves partial-transfer holes without requiring every old payload. The structures inform comparison rather than automatically select a business winner |
| Historical service facts and decisions | Immutable identity-keyed evidence sets, with specified joins/causal removal context where needed and a separate current-state projection | Duplicate-safe union can recover surviving observations/outcomes, while current confirmation and publication eligibility remain derived business decisions |

For book containment and equipment references, an identity map needs explicit dependency/lifecycle semantics. Tree or action-dependency models are relevant research for that boundary; neither whole-book atomic commits nor a dedicated action-graph engine is selected.

Operation-log replication, semantic scheduling, and transformation models remain useful comparisons. They are less direct for the selected current-value/coalescing workflow when their primary semantics require automatic transformation, deterministic tentative replay, or propagation of every unresolved candidate. State-based, operation-based, and delta-state CRDT families should be assessed per replicated object; their strongest immediate fit is immutable evidence, not automatically shared competing QSO values. Delta exchange, journals, and anti-entropy indexes are orthogonal dissemination/efficiency choices.

### Published mechanisms for the required properties

The topics below are covered research areas, not an unanswered synchronization agenda. Source assumptions and unselected implementation choices are recorded separately.

| Required property | Published approach/mechanism | Application to the agreed properties and qualification |
| --- | --- | --- |
| Recover a lagging or older restored copy | Causal-history reconciliation distinguishes ancestor from divergent versions, then exchanges missing state. Causal graph and version/context representations supply ordering evidence; full state substitutes for unavailable transfer deltas. | A retained ancestor can be recognized as older without treating its payload as a newly authored edit. Missing history uses D147/D148. Rewound writable counters cannot be assumed unique: a counter-based representation needs its published identity/incarnation contract. Plain common-value archives alone are insufficient. |
| Compact history/metadata while old copies remain eligible | Causal prefixes/context compression, structured creation/retirement, ownership joins, and history-graph pruning provide different reclamation contracts. | These are actual alternative structures, not an unresolved need to invent GC. Hash-history pruning can trade exact ordering for conservative conflict reporting, compatible with D148. Cooperative retirement and exclusive-ownership mechanisms require their stated lifecycle assumptions; elapsed time alone does not retire an enrolled writer. |
| Delete versus edit, including descendants | Conservative hierarchical state reconciliation preserves parent-delete/descendant-change conflicts; semantic dependency predicates/graphs express cross-item constraints. Deletion context distinguishes absent from removed. | Map logbook containment to the actual dependency relation and keep whole-QSO comparison atoms. User resolution, not a delete-wins cascade, determines divergent outcomes. External-association suppression has a separate retention purpose. |
| Recover evidence without repeating internal effects | Stable message/observation identities, immutable fact joins, and completion/processing records atomically stored with the corresponding mutation. | Reconcile a processed observation with its accepted edit and an outcome with its sent-value context. This covers duplicate-safe internal processing. A service request whose outcome was lost everywhere remains uncertain; no internal atomic transaction proves an unrecorded external success. |
| Avoid comparison-to-write races | Conditional modification / optimistic transaction validation atomically checks the compared versions or predicates and applies the update. | D146 is this established pattern, with business interpretation on the client. Relevant multi-record dependencies belong in the validated set; a failed condition returns to comparison. |
| Catch up without a gap before live streaming | Snapshot plus a retained ordered change suffix, or watermark-window snapshot chunks reconciled with overlapping changes. | Transfer existing state and all concurrent changes; suppress stale scanned rows and duplicate delivery. This provides the boundary discipline D61 needs under the source's ordered-log/non-stale-read assumptions. It does not choose a business conflict policy. |

Primary mechanisms: [hash-history reconciliation](https://oceanstore.cs.berkeley.edu/publications/papers/pdf/hh_icdcs03_kang.pdf), [structured retirement](https://www.csd.uwo.ca/~hlutfiyy/Perv/Papers/petersen.pdf), [conservative hierarchical reconciliation](https://www.cis.upenn.edu/~bcpierce/papers/unisonspec.pdf), [durable completion records and conditional multi-object validation](https://web.stanford.edu/~ouster/cgi-bin/papers/rifl.pdf), [conditional modification semantics](https://www.rfc-editor.org/rfc/rfc9110.html#section-13.1.1), and [watermark-window snapshot/change transfer](https://arxiv.org/pdf/2010.12597). The focused reports give the broader alternatives and assumptions.

### What remains a selection choice, rather than missing survey coverage

Choose the causal representation and its cost/lifecycle tradeoff; choose the domain mapping of containment and transactional dependencies; choose which established snapshot/change-boundary mechanism the storage/transport supports. No model or implementation is selected yet. These decisions should use the mechanisms above, not reopen their general subject areas as research gaps.

The genuine evidence boundary is compatibility with the source model's failure assumptions. A result proved for intact immediately pre-crash durable state is not automatically a proof for a writer restarted from an older snapshot. The survey identifies that distinction; a proposed concrete representation must follow an established compatible identity/history model or retain an explicit uncertainty outcome. This is not a claim that older-copy reconciliation is an unsolved distributed-systems problem.

The concrete proposal prioritizes dynamic version vectors for the expected actor population: 20 active simultaneously would be exceptionally high; 20 lifetime actors is reasonable, 100+ lifetime actors extremely rare, and lifetime total almost certainly below 1,000. These are estimates, not imposed caps; costs depend on represented lifetime identities, not merely active clients. Hash histories avoid actor dimensions but add retained-update history and possible uncertainty after pruning; no compensating Clacks workload benefit has been demonstrated. See the [recommended combination](recommended-model.md) for the workload tradeoffs and source assumptions; no software implementation is selected.

## Evidence and completeness limits

This is a bounded primary-literature survey, not an exhaustive bibliography or a new formal correctness proof. Its new coverage includes current-state versus arbitrary-event comparison domains, hash/epoch histories, action-constraint graphs, differential/shadow state, tentative certification, and recent consistency formalisms, in addition to CRDT families and causal metadata.

Costs are source/model-specific: the reports distinguish mathematical bit growth, number of represented identities, retained events, partial-delivery holes, and published workload measurements. No Clacks benchmark or source conformance experiment was run. Preprints and case-study implementations are identified as such.

# Causal and knowledge representations: research survey

13 October 2026. This report compares published **data structures and abstract models**, not software products or complete synchronization systems, following D150. It selects no encoding, clock actors, or namespace. Clacks-specific observations are marked **Inference**; source results are marked **Published**. Mathematical comparisons describe knowledge, not automatic business conflict resolution.

## Terms and notation

A **replica** is a stored copy. An **actor** is an identity that generates separately counted updates; actor identities and authorized clients need not have identical lifecycles. An **event** is a modeled update or communication step, not necessarily a saved record revision or every keystroke. **Causal order** records which events could have informed another. Two events are **concurrent** if neither knows of the other; their physical timestamps need not be equal.

A **causal context** summarizes known events. A **prefix** contains every event from an actor up to a counter; a **hole** is a missing event inside an otherwise received range. For example, receiving events 1, 2, and 4 must not be summarized as “all events through 4”: event 3 is still missing. **Dominance** means one summary covers another's events. **Retirement** ends an actor's ability to generate new events under its old identity; merely being offline does not end writing eligibility. In Clacks, D152 makes explicit revocation the cutoff for automatic acceptance from the actor; its unknown unsynchronized work is excluded unless manually reconciled later. Published retirement procedures may establish closure differently.

Cost notation `O(n)` describes how storage or work grows with `n`, ignoring constant factors; `O(1)` means a fixed number of components or operations under the stated model, not necessarily a fixed byte count. A **join** combines known information; a **frontier** is the set of versions not superseded by another represented version.

The later comparison uses original, local working, and shared accepted record values; these are value snapshots, not causal timestamps. A **tombstone** retains deletion knowledge after a value is removed. **Coalescing** keeps the final result of several local edits instead of every intermediate change.

Let `R` mean identities represented by a structure, `M` records, `H` retained historical events, and `G` exceptional dots/holes. These are different dimensions. `R` is not necessarily the number of authorized desktops or currently connected devices. Unless a retirement mechanism removes them, represented identities can include lifetime churn. Counter entries are not constant-size in the mathematical sense when their integers grow.

## Scalars, vectors, and the object of comparison

### Scalar Lamport and hybrid logical clocks

**Published:** [Lamport, *Time, Clocks, and the Ordering of Events in a Distributed System* (1978)](https://lamport.azurewebsites.net/pubs/time-clocks.pdf) assigns increasing scalar timestamps consistent with happens-before: `a → b` implies `L(a) < L(b)`, but the converse fails. A scalar order loses the distinction between causal succession and concurrent events.

[Kulkarni et al., *Logical Physical Clocks and Consistent Snapshots in Globally Distributed Databases* (2014 technical report)](https://cse.buffalo.edu/tech-reports/2014-04.pdf) represents a hybrid logical clock (HLC) as `(l,c)`: maximum physical time learned plus a logical counter for ties. Lexicographic order preserves the same **one-way** implication. State and updates use O(1) integer components/operations; proximity and finite-counter bounds depend on stated physical-clock uncertainty and event-rate assumptions. Resetting the tie counter as physical time advances bounds it under those assumptions. This is not actor-history garbage collection (GC).

**Inference:** neither scalar nor HLC ordering alone can prove which differing accepted record is newer versus concurrent. Clock-skew tolerance does not establish rollback-safe record identity or restore lost data. Sortable entity identifiers likewise do not supply causal evidence.

### Vector clocks versus version vectors

**How to read one:** a version vector such as `{A: 3, B: 2}` says this copy covers the first three updates from actor A and the first two from actor B. It covers `{A: 2, B: 2}`, but is incomparable with `{A: 2, B: 3}`: each has information the other lacks. That inference requires meaningful actor identities and truthful prefix coverage.

**Published:** a vector clock maps processes to event counts. With its event/send/receive update rules, componentwise strict order corresponds to happens-before; incomparable timestamps denote concurrency. [Singhal and Kshemkalyani, *An Efficient Implementation of Vector Clocks* (1992)](https://www.cs.uic.edu/~ajayk/ext/IPL92.pdf) studies incremental timestamp transmission under first-in, first-out (FIFO) delivery. A version vector instead summarizes update knowledge for a replicated data state: local update increments its component; synchronization joins componentwise. Structural similarity does not make their comparison domains identical; [Almeida, Almeida, and Baquero, *Bounded Version Vectors* (2004), §§1–2](https://gsd.di.uminho.pt/publications/gsd-2004-01/file/bvvmain.pdf), formalizes the distinction.

**Representation consequence:** a direct map has `R` counters; comparing/joining scans represented components. Keeping a separate full vector on each record duplicates roughly `M×R` entries. Contiguous actor prefixes compact arbitrarily many event names into counters, but counter bit lengths grow. Missing gaps require extra representation. Neither map shape nor componentwise maximum supplies identity retirement; dropping an entry destroys information unless its meaning survives elsewhere.

**Inference:** event-level ordering is more expressive than every application needs. The source's distinction rules out using arbitrary-event clock lower bounds as a blanket proof against bounded current-state representations. It does not prove arbitrary historical saved contexts remain comparable after compaction.

## Dots, contexts, and dot stores

### Dotted version vectors

**Published:** [Preguiça et al., *Dotted Version Vectors: Efficient Causality Tracking for Distributed Key-Value Stores* (2012)](https://gsd.di.uminho.pt/members/vff/dotted-version-vectors-2012.pdf) separates a new event `(actor,counter)`—the dot—from its causal past, rather than making the same vector simultaneously name the version and summarize predecessors. Testing whether a particular dot is covered requires inspecting its actor's context entry. Its constant-time causal test is not a claim that comparing every pair of arbitrary vectors or merging all values costs O(1).

[Almeida et al., *Scalable and Accurate Causality Tracking for Eventually Consistent Stores* (2014)](https://asc.di.fct.unl.pt/~nmp/pubs/dais2014.pdf) formalizes scalable clock operations for concurrent versions. In the stated read/context/write model, registering server identities bound metadata rather than the much larger client population; full comparison/merge costs depend on servers and versions. These bounds do not include an arbitrary number of retired server identities without additional handling.

**Inference:** separating an event from observed context is useful for record evidence, but does not require every desktop to be an actor. Neither paper's size bound alone settles independently writable accepted-state recovery after destructive rollback, actor GC, or Clacks's acceptance policy.

### Causal dot-store lattices and delta state

A **dot** names one event, usually with an actor and counter. A **dot store** keeps currently represented events or their values; its accompanying context remembers which events have been observed, including removed ones. This lets a merge distinguish “never received” from “received and later removed.” A **lattice** here is an information order with a defined least combined state, called its join. A **delta** is a fragment sent instead of the entire state.

**Published:** causal state is `(D,C)`. `D` is a dot set, dot-to-value map, or key-to-dot-store map; `C` records observed dots. The store/context pair has a specialized join, not independent componentwise union. A dot covered by the other context but absent from its store represents removal; [Almeida's 2024 author survey, §5](https://arxiv.org/pdf/2310.18220), gives the formal join. [Almeida, Shoker, and Baquero, *Delta State Replicated Data Types* (2018; author manuscript)](https://arxiv.org/pdf/1603.01529) generalizes these stores and compresses contexts into contiguous vector prefixes plus exceptional dots when delivery leaves holes.

**Concrete GC:** [Bieniusa et al., *An Optimized Conflict-free Replicated Set* (2012)](https://arxiv.org/pdf/1210.3368) removes deleted element tags immediately while a vector remembers they were seen. Repeated adds coalesce per element/actor. Its bound is O(`elements×R + R`), with O(`elements + R`) under the stated common-case writer assumption. Prefix-only contexts assume causal delivery; exceptions support holes. Deleted payload/tag retirement does **not** retire the corresponding vector component.

A **delta interval** groups changes between two positions in a sender’s sequence. The 2018 delta-interval algorithm uses durable state/sequence counts, transient deltas, and peer acknowledgments; it can send full state when the interval is unavailable. Its causal delta-merging precondition requires the destination to subsume the interval's starting state. Collecting delta buffers is distinct from collecting causal context.

**Inference:** these are explicit structures for remembering removal without one tombstone per operation. Add-wins/remove-wins or multi-value-register semantics are not Clacks's original/local/shared-value comparison policy. A dot-store structure can be studied without selecting its automatic query semantics. Missing actor coverage must not turn an old deleted dot into an apparently fresh addition.

## Shared knowledge and per-record exceptions

### Predecessor vectors with exceptions (PVE)

This approach shares knowledge across records instead of repeating a complete vector on every record. Per-record exceptions preserve differences caused by interrupted or partial transfer. For example, knowing a peer received most records through a point must not imply that it received the one record still missing.

**Published:** [Malkhi and Terry, *Concise Version Vectors in WinFS* (2005)](https://malkhi.com/files/WinFS-version-vectors-DISC2005.pdf) stores object identity/version/predecessor knowledge and replica-wide knowledge. The invariant requires a known version to be present or superseded by a retained descendant. Shared knowledge can stand in for object predecessors only when its restriction to that object's versions is correct. PVE encodes counters plus positive/negative exceptions; partially received objects may need explicit predecessor vectors until shared knowledge covers them.

The paper reports roughly O(`R`) difference-summary metadata, ignoring coding logarithms, and usually constant per-object metadata when synchronization completes. Disruption adds holes and explicit record metadata; its study identifies a workload crossover where ordinary vectors are cheaper. Removing explicit predecessors is justified by shared knowledge, not elapsed age. PVE itself does not give lifetime collection of actor identities.

**Inference:** this factors metadata across records while preserving interrupted-transfer distinctions. An older backup that loses payload must not retain knowledge falsely claiming a covering descendant. Retaining original and local working values for pending work is a separate obligation from predecessor compression.

### Node-wide dot knowledge

Store much of the event knowledge once for a whole replica and use it to interpret individual records. A **watermark** marks a known progress boundary; an index associates event identities with affected record keys. This can remove repeated per-record history while leaving its necessary evidence at node level.

**Published:** [Gonçalves et al., *DottedDB: Anti-Entropy without Merkle Trees, Deletes without Tombstones* (2017)](https://repositorio.inesctec.pt/bitstreams/5528b116-d25d-48f9-b484-3d4f4fc380d5/download) defines node clocks (prefixes plus noncontiguous counters), per-object clocks, dot-to-key indexes, and peer watermark matrices. Object causal context is stripped when node knowledge covers it and filled from node knowledge on use. Node knowledge replaces per-deleted-key tombstones; index entries are removed after peer coverage. Current value dots remain significant. Small per-object metadata is not a bound on all node-level identity history. Its recovery model preserves stable storage or replaces a failed node with a new identity and empty store.

**Inference:** “without tombstones” relocates evidence. Actor coverage, deletion knowledge, repair indexes, and payloads remain separate costs. An arbitrary older-server restore is not proved by that stable-storage assumption.

## Dynamic and bounded identity structures

### Structured identity and logically complete vectors

Make identity creation and retirement part of the known history. This gives a missing vector entry a defined interpretation instead of simply forgetting a writer. **Prefix-closed** knowledge includes the earlier steps needed to interpret a later creation or retirement. In the paper, the writer stops accepting updates, then ensures another replica receives its final update range before its entry can be removed.

**Published:** [Petersen et al., *Flexible Update Propagation for Weakly Consistent Replication* (1997), §4.3](https://www.csd.uwo.ca/~hlutfiyy/Perv/Papers/petersen.pdf) represents a new actor identity using its creator's identity and creation timestamp. Prefix-closed creation/retirement knowledge lets a removed vector component be interpreted recursively as either not-yet-created or fully retired; absence is not blindly zero. Retirement requires the actor to stop writing and transfer its final prefix. Nested identity length can grow with creation chains. This actually removes actor components while retaining interpretable identity structure, rather than moving every component into every record.

**Application mapping:** the paper closes a writer through cooperative final transfer. D152 instead allows Clacks to close automatic acceptance by revocation without receiving unknown unsynchronized edits; any later recovery of that work is manual. Do not carry the paper’s obligation to transfer all locally generated writes into this different admission policy. Interpretable removal still needs the representation’s creation/retirement knowledge and coverage invariants. Previously accepted shared data remains recoverable; an unrevoked offline writer remains eligible. Destructive backup rollback does not automatically preserve retirement knowledge or unique counter generation.

### Applying vector retirement to old clients and restored servers

Analysis updated 15 October 2026 under D151–D152. **Logically complete** means that the meaning of an omitted component can be derived from retained information. It does not mean every historical value is stored. The account's accepted-update history is the scope of Clacks retirement; D152 excludes unknown unsynchronized work from a revoked actor.

**How the researched structure collects an entry.** In the Bayou source model, an actor identity includes its creator and creation position. Suppose actor A was created by S at position 10. The following illustrative vectors have different meanings under the model's creation/retirement and ordered-coverage invariants:

| Stored summary | Meaning of A's entry |
| --- | --- |
| S covers only through position 8; A absent | A's creation is not yet known |
| S covers through position 20; A explicitly at 7 | A is known, and its component explicitly covers through 7 |
| S covers through position 50; A absent | A's creation is known; absence means retirement has been processed |

Creation adds an entry; only processing retirement removes it. The creator relationship therefore lets absence stand for a completed history instead of zero. If the creator's entry was also removed, the source interprets it recursively. This collects explicit vector components rather than merely moving one permanent counter per retired actor into a different table. Structured IDs can still grow with creation chains; this is not a constant-space theorem for all metadata.

**Why other clients may stay offline.** Collection is local to a replica whose knowledge satisfies the conditions. An absent client keeps its older vector, originals, and pending values. When it returns, differing summaries reveal the mismatch and normal synchronization catches it up. It does not have to acknowledge every other actor's retirement before collection proceeds. Its own entry is not removed merely for being offline. Missing incremental transfer history uses D144's fuller-state fallback, preserving ordinary pending work.

**How the revocation cutoff maps to this.** The paper obtains a closed history cooperatively. D152 lets the server close the admissible history by refusing further automatic updates from that actor. The equivalent creation/closure information must participate in synchronized knowledge, covering already accepted work before a receiver treats the actor as complete. A revoked device's unknown local tail is not admitted through this mechanism; later recovery is manual. Revocation supplies closure, but does not justify simply deleting a map entry and treating all unknown values as already present.

The source explicitly discusses adopting its lifecycle technique with equivalent creation/retirement information and appropriate causal propagation, without requiring operation exchange (§5.1). This supports a model-level candidate for state reconciliation, not an unchanged proof of Clacks's admission policy or arbitrary per-record vector deletion. The source's summaries describe replica-wide knowledge. Record-specific predecessors and partial transfers must still be represented truthfully.

**How an older server backup is repaired (application mapping).** The backup has older values and knowledge together. A surviving authorized client can supply newer accepted state and knowledge, including retirement information. If the client has compacted an actor entry, the retained representation must still explain that omission; it must not be transmitted as an unexplained zero. If individual changes are gone, fuller-state reconciliation supplies current surviving records, deletion knowledge, and service evidence. Transfer must not advance knowledge to claim coverage of data that was not received or retained in a superseding state.

A separate requirement is avoiding reuse of writer event IDs after restoring counters. For example, an old server must not generate a different update named S:21 when an earlier S:21 survives elsewhere. A recovery mechanism must reestablish a safe counter continuation or use a distinct writer incarnation (a new causal identity for subsequent events while retaining the data and account). Choosing between those established recovery patterns is not the same as choosing retirement encoding. A larger counter learned from one available peer alone is not proof that no unavailable peer holds higher counters. D147/D148 cover missing comparison evidence; they do not permit ID collisions. D143 still governs a revocation lost with the only newer authorization state.

**Which related models supply which part.** Dotted version vectors separate an event ID from its past; they do not themselves collect lifetime actors. Predecessor vectors with exceptions share knowledge across records and preserve interrupted-transfer holes; they reduce repeated metadata, not membership history by themselves. Bounded version vectors offer a fixed-population/current-state comparison scope, not an unchanged solution for arbitrary saved contexts or backup rollback. Logically complete vectors are the researched mechanism here that actually eliminates retired components while preserving an interpretation for old peers. Combining it with record-level knowledge factoring needs the source invariants, not blind composition of their merge functions.

### Primary acceptance order and the server component

**Correction and candidate simplification, 15 October 2026:** an example using only client components is not a valid demonstration that collecting a retired client component loses essential ordering for Clacks. Every accepted update passes through the server. If acceptance advances a server component and record versions retain that position, accepted updates in one continuous server history remain ordered after a retired client component is removed.

The researched precedent is Bayou's primary-commit ordering (§§3.1–3.2 of the Petersen paper linked above): a monotonically increasing commit sequence number can summarize the committed prefix, while tentative updates require separate treatment. This is a more directly relevant compression opportunity for Clacks's accepted shared state than assuming fully independent accepted writers. It does not require adopting Bayou's tentative-write propagation or replay policy.

**Illustrative, unselected representation:** start from `{S:10, A:1, B:1}`. B's accepted edit is `{S:11, A:1, B:2}`. A's pending offline edit, based on S:10, is `{S:10, A:2, B:1}`. Removing B leaves `{S:11, A:1}` versus `{S:10, A:2}`. They remain incomparable: one contains the newer accepted server state, the other contains pending local work. The incoming edit does not acquire a later server stamp until it passes the existing comparison/conditional-acceptance rules. Equal-result handling remains value-based under D137.

A server component must be attached to the corresponding accepted version or truthful coverage summary; an account's current counter must not be copied onto an unchanged old record to make it appear newer. A high synchronization cursor alone does not mean every record has received a new accepted version.

**Rollback boundary:** S:11 from an original history and S:11 newly generated after restoring S:8 cannot be treated as the same event. Safe counter continuation or distinct server incarnations must distinguish them. Different post-restore histories may lack an established causal order and use the accepted explained-reconciliation fallback. This is distinct from the ordinary case of removing a retired client component within one server acceptance history.

**Consequence for selection:** assess committed-prefix/server-component compression before adding logically complete-vector retirement machinery. The latter remains a researched alternative where independent causal dimensions still need collection. Neither an omitted-server counterexample nor this server-order observation alone establishes that arbitrary historical components, deletion knowledge, or service evidence may be dropped at any time. Exact representation and compaction are still proposals, not newly accepted decisions.

### Recovery under the current exclusions

The [source-level recovery fit analysis](recovery-fit.md) checks stable-ID incarnation vectors, coordinated recovery, self-stabilizing clocks, bounded vectors, and disconnected knowledge models against Clacks's exact requirements. D153 allows detected rollback to enter manual reconciliation. Optimistic state reconciliation with explicit repair is the relevant no-quorum family; the audit distinguishes that contract from the stronger atomic-register papers and documents the scalar-regression detector’s limits. Per-restart actor creation and a version-history graph are not selected solutions. The removed parent-version-graph proposal did not answer the vector-recovery question.

### Interval tree clocks (ITC)

This approach divides the right to generate events into tree-shaped ownership ranges. A fork splits ownership; a join recombines it. Unlike a growing list of named writers, the tree can collapse compatible branches. That benefit depends on exclusive ownership: two restored copies must not both write using the same range.

**Published:** [Almeida, Baquero, and Fonte, *Interval Tree Clocks: A Logical Clock for Dynamic Systems* (2008)](https://gsd.di.uminho.pt/members/cbm/ps/itc2008.pdf) stores `(identity-tree,event-tree)`. Identity trees describe disjoint writable intervals; event trees describe a piecewise counter function. Comparison orders event functions pointwise. Fork splits ownership, join combines it, anonymous peek transmits knowledge without writing authority. Tree normalization merges equivalent subtrees and common event levels; ownership joins permit reuse without accumulating a list of globally named retired actors.

The paper gives algorithms and measured average sizes under fork/event/join workloads, not a universal constant-byte or O(active-actors)-only bound for arbitrary executions. Counters and tree fragmentation remain relevant.

**Inference:** an indefinitely offline writer still owns its interval. Authorization revocation is not a mathematical join; reauthorization or backup cloning cannot safely let two instances use the same writable identity. These are constraints on interpretation, not a proposed identity lifecycle. ITC's generalized event structure can compare more than current payloads but does not retain those payloads.

### Frontier version stamps and bounded stamps

These approaches limit the comparison scope to currently represented states so old naming information can be reused under their rules. They must be distinguished from clocks that promise to compare any saved historical event. A fixed bound for a fixed number of replicas is also different from a bound independent of client count.

**Published:** [Almeida, Baquero, and Fonte, *Version Stamps—Decentralized Version Vectors* (2002 technical report; author-uploaded manuscript)](https://www.researchgate.net/publication/2904177_Version_Stamps_---_Decentralized_Version_Vectors) models coexisting frontier elements, using binary names for identity/update components. Fork extends names; join collapses compatible names. Its proved comparison scope is the current frontier, not arbitrary old versions. ITC's later experiments report practical growth difficulties with this earlier representation.

**Published:** *Bounded Version Vectors* replaces each ordinary counter slice with finite symbols and locally cached symbol orders. Symbols not currently represented can be reused; unpropagated updates coalesce. A slice stamp has `N` rows of at most `N` symbols, from a finite domain sized for `N²` symbols; full vectors concatenate slices. Boundedness is relative to fixed `N` replicas and local-update/pairwise-sync operations, not independent of population size. This is controlled symbol reclamation, not dropping actor entries.

**Inference:** indefinite disconnection is not the same as preserving arbitrary historical stamps or permitting destructive rollback. Saved originals and old accepted copies must fall inside the proved comparison domain before a bound can be applied. No fixed client cap follows from discussing this model.

## Explicit causal graphs and alternative encodings

### Causal histories and hash-history graphs

Keep a graph of versions and derivation links: an edge records that one version was made from another. A content **hash** is a compact fingerprint of a record’s bytes. An **epoch** distinguishes repeated appearances of the same content, so returning from value A to B and back to A need not look like no update occurred.

**Published:** [Kang, Wilensky, and Kubiatowicz, *The Hash History Approach for Reconciling Mutual Inconsistency* (2003)](https://oceanstore.cs.berkeley.edu/publications/papers/pdf/hh_icdcs03_kang.pdf) represents version nodes as **content-hash/epoch pairs** with parent-child edges. Ancestor paths establish dominance; identity storage depends on retained updates/edges rather than actor count. Same content can recur, so plain hashes are insufficient: epochs distinguish repeats. Independent equal hash/epoch results are deliberately equated, even if not causally related; that is semantic equivalence beyond exact event causality.

The paper ages graph history using roughly synchronized timestamps and discusses archival portions. Missing pruned ancestry can conservatively yield a false conflict; trace results are not a universal safe retention duration. Graph links and historical payloads need not have identical retention.

**Inference:** this is an established graph alternative, not endorsement of the set-aside accepted-parent-link proposal. A graph comparison can become unknown after trimming, matching D148's permitted explanation. Hash collision assumptions, repeated-content epochs, and retained-path scope matter. Equality can support D137's no-flow case while service evidence remains independently missing.

### Prime-encoded vector clocks

Encode the counters of a vector in one integer, giving each actor a distinct prime base. Divisibility replaces componentwise comparison. This changes the representation, not the amount of information: the integer itself grows as counters or actor dimensions grow.

**Published:** [Kshemkalyani, Shen, and Voleti, *Prime Clock: Encoded Vector Clock to Characterize Causality in Distributed Systems* (2020)](https://www.cs.uic.edu/~ajayk/ext/JPDC2020-EVC.pdf) encodes `v` as `∏pᵢ^vᵢ`. Divisibility implements vector dominance; least common multiple (LCM) joins and greatest common divisor (GCD) intersects knowledge. This preserves event-clock expressiveness under the model's distinct primes. One integer is not constant space: its bit length is determined by `Σvᵢ log₂pᵢ`, and arithmetic cost depends on integer width. The paper explicitly distinguishes bounded-word from arbitrary-precision cost models and analyzes growth. Basic encoding supplies no lifetime actor retirement.

**Inference:** encoding dimensions into one number does not eliminate their information or restore lost comparison evidence. This prevents misleading constant-size claims without ruling out domain-specific compaction elsewhere.

## Compaction through causal stability; newer synthesis

**Published:** [Baquero, Almeida, and Shoker, *Pure Operation-Based Replicated Data Types* (2017 manuscript)](https://arxiv.org/pdf/1710.04469) represents a partially ordered log as timestamp-to-operation entries. Data-type redundancy can remove entries; causal stability—no further concurrent delivered operations—permits stripping timestamps and sometimes more operations. Receipt by all peers is a different condition. Stability depends on delivery/membership assumptions, not age alone.

[Almeida, *Approaches to Conflict-free Replicated Data Types* (2024 author survey)](https://arxiv.org/pdf/2310.18220), §§5–8, clarifies that dot context/store components are coupled, causal contexts may have holes, and replica-keyed maps remain a scalability obstacle. It describes some identity-renaming research as preliminary. It does not establish a universally bounded causal store under arbitrary client churn and rollback.

**Inference:** an indefinitely disconnected eligible writer can keep concurrent work outstanding. D148 permits unknown ordering, but not a fabricated stability claim. Clock metadata must not be silently treated as valid after its causal storage assumptions fail.

## Matching Clacks properties without choosing a structure

- **Original/local/shared comparison and field resolution:** causal evidence distinguishes shared-state relationships; values retained for pending work support whole-record comparison. Knowledge concurrency alone does not demand a prompt when whole results agree, or authorize automatic disjoint-field merging.
- **Coalescing and fuller-state fallback:** vectors, dot contexts, shared knowledge, and graphs summarize more than retained payload history. Delta-buffer or transfer-log GC does not prove actor/deletion GC.
- **Symmetric recovery:** any representation must accompany surviving acknowledged values, deletion/service evidence, and truthful coverage. Same writable identifier/counter reused after restore can alias different facts. A monotone join cannot recreate payload lost everywhere.
- **Unknown ordering / missing original:** pruned graph paths or discarded metadata may justify D148 uncertainty; missing common values separately justify D147 two-way resolution. Incomparability means proven concurrency only within a structure's intact evidence/assumptions. An invalid or pruned comparison must not masquerade as that proof. Ordinary pending originals still survive. No particular unknown-marker encoding is proposed.
- **Indefinite enrollment / reauthorization:** authorization identities and causal generating identities are distinct logical roles. No source requires every authorized desktop to contribute its own clock component. No structure alone decides revocation or recovery publication; manual upload pause/resume remains application policy.

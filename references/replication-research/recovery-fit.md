# Vector-based recovery: published mechanisms and requirement fit

15 October 2026. Source-level follow-up to D151–D154. **D154 selects timeline-level ancestry; exact protocol, representation, detection coverage, and collection rules remain open.** The vector-based design direction remains selected; the earlier suggestion to add parent-version history has been removed from the recommendation because it reintroduces the history model under another name.

## Question and result

Clacks requires eventual consistency and user-resolved conflicts, not uninterrupted atomic-register semantics. D153 explicitly allows detected rollback to require manual client reconciliation. The relevant model family is therefore **optimistic state reconciliation with detection of invalid synchronization metadata and explicit repair**. A storage quorum is not a prerequisite of that family.

The earlier audit's quorum candidates solve a stronger recovery problem. Their conditions must not be presented as requirements imposed by Clacks's weaker contract. Published optimistic replication supports preserving conflicts for manual resolution; primary implementation documentation also supplies peer-witnessed rollback detection and quarantine. Neither establishes a universal arbitrary-rollback detector based on a scalar alone.

## Optimistic reconciliation and explicit repair

[Saito and Shapiro, *Optimistic Replication*, 2005](https://dsf.berkeley.edu/cs286/papers/optimistic-csur2005.pdf), particularly §§1.2–1.3 and 4.2, describes asynchronous propagation, conflict detection, and manual resolution that excludes competing operations until the user submits a result. This is the relevant consistency/repair family rather than transparent recovery of an atomic register.

[Satyanarayanan, *The Evolution of Coda*, 2002](https://www.cs.cmu.edu/~15-749/READINGS/required/other/coda-evolution-satya-may02.pdf), §§5.4–5.5, preserves conflicting copies, marks affected objects for repair, and provides user-driven repair, including client-local versus shared-state differences. It is evidence for the repair model, not a proposal to represent contacts as files, block an entire logbook, or adopt Coda's storage architecture. The paper does not prove a complete detector for arbitrary writable backup rollback.

[Microsoft's primary rollback-detection documentation](https://learn.microsoft.com/en-us/troubleshoot/windows-server/active-directory/detect-and-recover-from-usn-rollback) supplies a concrete peer-witnessed detection/isolation/recovery precedent. It uses update sequence numbers (USNs) and a database invocation identity; supported restoration updates the comparison domain. Its [virtualization documentation](https://learn.microsoft.com/en-us/windows-server/identity/ad-ds/get-started/virtual-dc/virtualized-domain-controllers-hyper-v#undetected-usn-rollback) explicitly documents rollback that counter comparisons can miss. These are mechanism examples, not software recommendations.

### Overall server revision as a simple detector

**Proposal, not a selected protocol:** the server advertises the accepted-state revision for the relevant account. Each client durably remembers a server revision it previously observed or synchronized. If a client knows the server reached 200 but it now advertises 160 under the same comparison identity, there is evidence the assumed monotonic history is invalid. One available witness suffices to flag that contradiction. Do not compare against a number inferred from unsynchronized local edits; those edits have not been accepted by the server.

Clacks can then preserve local and restored data, stop trusting the affected incremental comparison metadata, and enter explicit reconciliation under D147/D148/D153. There is no need for an online majority to flag the regression or to have the user choose record values. D141 still governs the operator's external-upload pause/resume workflow. Exact repair scope and restoration of a valid metadata baseline are unselected.

**Detection limit:** if the server restores 160, accepts enough changes to reach 210, and only then meets a client that last saw 200, this scalar check sees no decrease. It cannot prove that the new 210 incorporates the original history through 200. The metadata may also be inconsistent after partial restoration. Thus scalar regression detection is a useful simple mechanism, not a universal proof against all writable restore histories. A declared-restore procedure and metadata repair are distinct from blindly detecting every possible snapshot restoration; no particular additional detector is selected.

### Timeline ID plus revision: selected direction

The user proposes `(timeline ID, revision)`, changing the timeline when a revision rollback is detected. **D154 selects this direction; the exact encoding remains open.** A timeline ID labels one comparison history; it is not a new enrolled client or a new actor coordinate on every ordinary server restart. Numeric revisions are compared only within a known common timeline.

Example: a server advertises `(T1,160)` to a client with complete synchronized state through `(T1,200)`. Detect the regression, create a fresh timeline T2, and reconcile the restored state with the client's surviving accepted state and pending work. If complete coverage and usable metadata survive, recovery can incorporate the old history through 200; merely remembering the number 200 does not supply its missing records, deletion information, or service evidence. Edits accepted after restoration can also be competing work to preserve during reconciliation.

A later client remembering `(T1,201)` must not treat `(T2,205)` as a continuation merely because 205 is larger. The timeline mismatch exposes the history boundary even after the numeric revision has passed 201. This uses a generation marker per detected recovery, not a per-version ancestry graph or an actor per process start.

A client last at `(T1,199)` may need only ordinary record-level comparison if recovery has established coverage of that old history through 200. The tuple alone does not prove that coverage: its old incremental cursor crosses a timeline boundary. A fuller-state comparison can still avoid user prompts for equal or otherwise unambiguous records. A compact record of the recovered old prefix could support automatic recognition, but no particular bridge/retention mechanism is selected.

This proposal protects against numeric reuse **after** rollback detection. It does not detect a restore that advances beyond every available witness before anyone observes a regression. Ordinary restart keeps the timeline. Exact account/server scope, atomic installation and transfer of the marker, and restoration of a valid comparison baseline remain to be specified. The earlier cited Active Directory invocation-ID change on restore is a primary precedent for the generation-and-sequence distinction, not a proof of this Clacks-specific detection or reconciliation procedure.

### Timeline-level ancestry instead of per-version history

The user further proposes recording ancestry at timeline boundaries, for example that `(T2,205)` incorporates `(T1,200)`. **Selected direction under D154; exact representation is open.** It is materially cheaper than retaining a predecessor node for every accepted record edit: records can carry fixed-size timeline/revision references, while shared ancestry metadata grows with recovery/fork/merge boundaries rather than contact edits.

If the relationship establishes that T2 at revision 205 incorporates the old T1 history through 200, an old `(T1,199)` position is covered. A `(T1,201)` position is not established as covered, so its retained data must be compared/reconciled rather than skipped because T2's numeric counter is higher. Ordinary pending local work still follows the existing whole-record rules. This establishes logical coverage; actual incremental change records may still have been collected, requiring fuller-state transfer.

**Fork origin and recovered coverage must be accurate.** If the physical restore starts from `(T1,160)`, that is the initial shared prefix. Merely meeting a client at 200 does not change it. After reconciliation really incorporates the old history through 200, a boundary can record that coverage at the particular T2 revision where incorporation completes. It must not retroactively make earlier T2 revisions claim that knowledge. A simple fork after complete recovery can have one parent cutoff; a later recovery that incorporates another history may require a merge/coverage relationship. This is a graph of exceptional timeline boundaries, not an assumed per-record event graph.

Primary mechanism precedent: [PostgreSQL's timeline history documentation](https://www.postgresql.org/docs/18/continuous-archiving.html#BACKUP-TIMELINES) describes small history files recording each timeline's parent and branch point. This is evidence for a shared lineage structure, not selection of PostgreSQL, its transaction log, or its recovery protocol. Clacks's client-supplied state recovery and user resolutions need the coverage meaning stated above.

The event count should be small when actual rollbacks/recoveries are rare; it is not a mathematical constant bound. Missing/collected lineage can use the accepted explicit uncertainty fallback without expiring offline clients. D154 selects timeline/revision lineage; no exact tuple encoding or collection policy is selected.

## Terms and required scope

A **replica ID** names a participant. An **incarnation** is one execution instance after a restart; an incarnation counter can change inside the same participant's vector component without adding a new actor entry. That still represents recovery boundaries; it is not the same as eliminating recovery metadata. A **quorum** is a sufficiently large subset of storage replicas whose overlap provides the protocol's guarantee. A **recovery gate** delays ordinary operations until the protocol establishes the required state. Such a gate can run on every startup without detecting whether rollback occurred.

Clacks has one logical acceptance server, arbitrary offline clients, durable pending edits, coalesced state comparison, and recovery of surviving accepted records/deletion/service evidence. No always-available majority of desktop clients or independent non-rollback durable store is assumed. Whole-record human resolution and conservative unknown-ordering handling remain required. D152 excludes further automatic writes from revoked actors; unrevoked offline clients remain eligible indefinitely. Facts lost everywhere cannot be recovered.

## Stronger background: stable identities with coordinated incarnation recovery

### Michael, Ports, Sharma, and Szekeres, 2017

[*Recovering Shared Objects Without Stable Storage*](https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.DISC.2017.36), published in the International Symposium on Distributed Computing; [extended report with Appendix A](https://homes.cs.washington.edu/~drkp/papers/recovery-tr17.pdf).

**Mechanism:** processes retain stable identities; crash vectors track their incarnations. Recovery reconstructs state before operational participation. Appendix A replaces a monotonic recovery clock either with sets of unique incarnation IDs or with a nonce-protected quorum read followed by increment. A **nonce** is fresh exchange-specific information used to reject stale responses. ID sets grow with recoveries; the compact alternative adds communication.

**Scope:** §3 assumes a fixed participant set, restart awareness, and unique incarnations. §4's communication primitives depend on intersecting quorums; its termination condition requires a suitable set of operational processes for long enough. The compact variant avoids a new vector coordinate for each restart, but does not avoid restart recovery or incarnation advancement.

**Fit:** no version-history graph or rollback detector is required if recovery runs on startup. It is not an unchanged fit for a single independent server and arbitrarily absent desktop clients. Promoting desktop clients into a quorum group changes availability and membership assumptions. Its atomic shared-object contract also does not implement Clacks's human conflict workflow by itself.

### Keshavarzi, Chockler, and Gotsman, 2025

[*TEE Is Not a Healer: Rollback-Resistant Reliable Storage*](https://drops.dagstuhl.de/storage/00lipics/lipics-vol356-disc2025/html/LIPIcs.DISC.2025.39/LIPIcs.DISC.2025.39.html). The title's TEE means **trusted execution environment**; this is literature evidence, not a proposal to require that hardware for Clacks.

**Mechanism:** the dynamic recovery algorithm uses stable storage-replica identities, incarnation numbers, and crash vectors. Its restart procedure marks a replica stale until recovery completes. Section 4.3 provides distributed incarnation allocation without relying on non-volatile counters that survive rollback. It uses two related vectors and coordinated reads/writes to avoid reusing an incarnation number.

**Scope:** §2 separates a fixed set of storage replicas from arbitrary clients. Section 4 requires appropriate available subsets and bounds on permitted failures/recovery behavior. The protocol can handle rollback without identifying a special rollback event; it does know when it starts. A fresh incarnation does not create a new participant entry, but recovery communication remains necessary.

**Fit:** conditional candidate for stronger continuously safe storage semantics, not a required or preferred recovery architecture for Clacks. It would require a replicated backing service with its stated recovery/availability conditions. Offline desktop clients need not participate if they remain clients of that service. Adding such a service is a substantive architecture choice, not a feature already implied by allowing a database service. The source does not establish the same guarantees for a standalone server with opportunistic desktop synchronization.

## 2. Self-stabilizing vector clocks

Salem and Schiller, [*Practically-self-stabilizing vector clocks without scheduling fairness*](https://link.springer.com/article/10.1007/s00236-026-00544-z), Acta Informatica, published 20 August 2026; [earlier manuscript](https://arxiv.org/abs/1712.08205).

**Mechanism:** bounded labels and paired vector state recover causality tracking after arbitrary transient corruption, without identifying a rollback event. Section 6.7 explicitly recognizes some unavailable comparisons when required shared reference information (a pivot) is absent.

**Scope:** a given set of processors and bounded channel capacity. The recovery guarantee permits a bounded number of safety violations in very long executions; §8's correct causal comparisons are established for legal executions. It does not prove that every potentially incorrect recovery-time comparison returns unknown.

**Fit:** relevant published work on the desired failure category, but not a safe direct substitute for Clacks's no-silent-loss reconciliation. Sending only detected unknown cases to a user does not cover undetected wrong comparisons. No published fail-closed wrapper establishing that stronger guarantee was identified in this audit.

## 3. Bounded and fault-tolerant vector variants

[Almeida, Almeida, and Baquero, *Bounded Version Vectors*, 2004](https://gsd.di.uminho.pt/publications/gsd-2004-01/file/bvvmain.pdf): finite reusable symbols preserve comparison of current states under initialization, local-update, and pairwise-synchronization transitions. Replicas may remain absent; absence alone is not a valid objection. Replacing an existing state with an arbitrarily older backup is outside the proved transition set. Saved historical contexts need separate scope analysis. This supplies bounded normal-operation summaries, not the requested restoration algorithm.

[Damani and Garg, *Fault-Tolerant Vector Clocks*, 1996](https://www.cse.iitb.ac.in/~damani/papers/ut/icdcs96.pdf): process-version fields, checkpoints, logs, failure tokens, and version history distinguish useful from invalid computational states after failures. Version advancement and recovery bookkeeping must survive according to the model. This does not eliminate recovery handling or prove recovery after the durable bookkeeping itself is rolled back. Its fault tolerance should not be read as the requested transparent backup-restore guarantee.

## 4. Disconnected state reconciliation and knowledge compression

[*Peer-to-Peer Replication in WinFS*, 2006](https://www.microsoft.com/en-us/research/wp-content/uploads/2006/06/tr-2006-78.pdf), §5, and [*Concise Version Vectors in WinFS*, 2005](https://malkhi.com/files/WinFS-version-vectors-DISC2005.pdf) give shared knowledge, per-record predecessor information, and exceptions for incomplete transfer. Their incrementing counters and knowledge invariants still require meaningful update identity and retained values or successors. They do not supply transparent restoration of an older writable member image under the same counter stream.

[Bayou update propagation, 1997](https://www.csd.uwo.ca/~hlutfiyy/Perv/Papers/petersen.pdf) provides primary-commit ordering, interpretable actor retirement, and fuller-state transfer when logs were truncated. These are useful mechanisms. They do not by themselves repair a writable server's rewound counter or prove its missing accepted data already covered. A full transfer fallback is not a proof of correctness for arbitrary old-backup counter reuse.

## What can be concluded for Clacks

- The quorum papers solve a stronger problem. Their lower bounds and recovery gates do not establish that Clacks requires a quorum.
- Optimistic state-based synchronization with explicit conflict repair is the relevant no-quorum semantic family under D153. Peer-witnessed invalid-metadata detection followed by isolation/repair has a primary documented precedent.
- Advertising overall accepted-state revision and comparing it with retained client knowledge detects genuine regressions using one witness. It does not detect a restored counter that has already caught up, so it remains a proposal with a known coverage limit.
- Manual reconciliation is an acceptable recovery outcome. That does not make an undetected incorrect causal comparison safe. Distinguish the detection contract from the repair policy.
- The checked papers and examples do not together prove Clacks's complete detector and repair procedure. Do not claim that an unchanged atomic-register algorithm is needed, or that such algorithms' availability assumptions are a necessity for the weaker model.
- Per-record version-history graphs, actor creation on every startup, replicated storage quorums, and independent durable metadata are not selected. D154 instead selects shared timeline-level ancestry; D151's vector direction and D152's revocation cutoff remain in force.

## Actor collection with indefinitely old clients

D154 lets accepted contributions use timeline positions even after their authors retire. An old reference to `(T1,199)` remains interpretable if the lineage establishes its relationship to current accepted state; no per-record retired-author counter is needed solely to order that contribution. This is a Clacks model implication, not a theorem that arbitrary vector projection or deletion is safe.

D152 closes future automatic submissions from a revoked actor. Collection need not wait for that actor or for every unrelated offline client. Unrevoked clients remain eligible; their old cursors can fall outside the retained incremental journal and use fuller-state reconciliation. Their durable pending originals/values survive. Required deletion and service evidence have separate retention obligations. Keep small shared lineage records, summarize equivalent relationships, or use D148's explicit uncertainty fallback if old ordering is lost; no elapsed-time enrollment expiry follows.

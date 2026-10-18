# Replication approaches, data models, and reconciliation structures

13 October 2026. Planning research; no protocol, clock, representation, or software selection. Current AGENTS.md, synchronization.md, and D136–D150 were read. **Findings** describe published models; **application implications** assess Clacks requirements without constructing a replacement protocol. Systems below are evidence for generic approaches, not a product shortlist.

## Reading guide

A **replica** is a copy of the data. **Reconciliation** is the process of comparing copies and deciding which differences can propagate automatically and which require resolution. **Optimistic** replication permits local work before shared acceptance. **Coalescing** means retaining the final effect of several local edits rather than sending each intermediate step.

A **precondition** is something that must still be true before an operation is accepted—for example, “this contact still has the values I compared.” An **invariant** is a rule that must remain true, such as each accepted tag membership referring to an existing tag, or an external record having at most one associated local contact within an account/service destination. **Convergence** means copies eventually agree after receiving the relevant changes; it does not by itself mean they agree on the values the user intended.

Each section introduces a different way to represent or interpret changes. The examples explain the approach; the application paragraphs assess its fit without adopting it. Decision references such as D137 point to the [decision register](../../docs/decisions.md).

## Scope and distinctions

Replication has several independent dimensions: represented state, propagated information, reconciliation transition, acceptance/commitment, visibility, and retention. A tree is a data model; a log is a representation of changes; a consistency axiom is an observable contract. None alone specifies all the others. State/operation transfer also need not coincide with state/operation-based conflict-free replicated data types (CRDTs): optimistic replication can retain explicit unresolved conflicts. Saito–Shapiro's survey makes these dimensions explicit [1].

Clacks requires independently writable offline clients through a server star; whole-record original/local/shared comparison; one-sided/equal results without resolution; differing two-sided results resolved by the client, even for disjoint fields; unresolved proposals local; conditional shared acceptance. Recovery returns surviving synchronized records and service evidence to an older server, without reconstructing universally lost facts. Indefinite return, fuller-state fallback, retained reauthorized-client data, and deleted-tag reference validity matter. D147/D148 allow explained uncertainty/two-way recovery; ordinary pending originals remain required.

## 1. Common-state, state-based optimistic reconciliation

**Finding: representation/transition.** Represent current replicas A and B plus common state O. A state can be a keyed map of opaque values, a tree, or a collection of relations. Compare changes relative to O, reconcile, propagate selected results, then advance O only where synchronization succeeded. Formal tree reconciliation specifies preservation of unresolved changes and parent/descendant conflicts; the Unison paper is evidence for this model [2]. A relational identity map is an application-domain interpretation, not a radio contact (QSO)-as-file proposal.

**Application implications.** For an opaque record the ordinary rule is: A=B means agreement; A=O selects B; B=O selects A; otherwise require user resolution. Deleted can be a comparison value. Choosing the whole record as the atom gives D137/D138 directly; decomposing fields into separately reconciled atoms would silently merge disjoint changes and violate D137. Field comparison inside the resolution flow is separate from the conflict trigger. Tree dependency semantics illustrate book deletion against child edits without making every child edit a book-wide conflict.

**Assumptions/cost/retention.** Shared O must be meaningful. Coalescing preserves current-value semantics, including changes later reverted to O. Comparing full maps/trees costs at least examining the relevant state; fingerprints/changed-path indexes can reduce transfer, but do not retain original field payloads. Pairwise archives scale with synchronization relationships; a star avoids all-pairs archives [3]. The formal model provides no timing-expiry requirement.

**Recovery limit.** With O=A=new and restored B=old, the ordinary rule selects old as a one-sided change. This is a rollback misclassification, not a conflict-predicate defect. Archive loss likewise removes creation/deletion attribution. Full scanning repairs transfer coverage, not missing knowledge. D148 conservative comparison can handle unavailable ordering, but is an explicit application policy beyond the ordinary common-state transition. No archive encoding is selected.

## 2. Operation-based optimistic replication with predicates and commitment

Send commands rather than just final record values. For example, a command can say “replace this contact only if it still matches the values I read.” Replicas retain enough command and dependency information to decide whether execution is still valid.

**Finding: representation/transition.** Retain operations, parameters/identity, observed dependencies or application preconditions, an execution order, and tentative versus stable status. Exchange missing operations; apply or replay them; detect violated dependencies; reconcile through rejection, modification, or application procedures; stabilize agreed outcomes. Bayou's write/dependency-check/merge-procedure model demonstrates multi-item predicates and rollback/reexecution of tentative operations [4]. These are broader semantics than a queue of field patches.

**Application implications.** A record-changing operation can depend on the whole compared record; QSO acceptance must validate tag references and external-record association uniqueness under D250/D251, while tag changes remain ordinary QSO edits under D243. Operations concerning distinct records may be independent. Mathematical independence of field writes does not override Clacks's whole-record policy. Standard Bayou-style replicated tentative proposals differ from Clacks's client-local pending work; this is a policy/storage boundary, not a reason to dismiss operation models.

**Assumptions/cost/retention.** Common execution needs agreed ordering/resolution and deterministic replay. Operations may save bandwidth for large state, but logs, undo data, and reexecution add costs [1]. Coalescing must preserve dependencies and intended effects; it is not forbidden, but arbitrary deletion of intervening operations is unsound. Stable-log pruning needs checkpoint/omission evidence so received operations are not accepted twice [4]. An old backup may erase that evidence; restore repair therefore cannot assume the last server acknowledgment is still present. External publication is not safely undone by reversing an internal operation.

## 3. Semantic scheduling over action/dependency graphs

Represent changes and their relationships explicitly, then find an acceptable execution order. For example, creating a contact requires its logbook to exist; deleting that book can be incompatible with a concurrent contact edit. A schedule is the selected set of actions and their execution order.

**Finding: representation/transition.** Actions have targets, preconditions, effects, and semantic constraints. A graph records ordering, enabling, incompatibility, or noncommutation relationships; a valid schedule is an admissible cut plus ordering, not necessarily the original arrival order. Schedules can be explored against scratch state and selected according to application policy. IceCube studies static/dynamic constraints; Telex formalizes an action-constraint graph and committed prefix [5,6].

**Application implications.** This represents relational/aggregate dependencies explicitly: child creation requires its book; deletion and conflicting child edit need not be jointly admitted. It can also express whole-record incompatibility despite algebraically independent fields. Automatic schedule selection is not equivalent to required client field resolution. Replicating all candidate actions would violate Clacks's proposal-locality requirement; constraint concepts remain useful for defining its semantics.

**Assumptions/cost/retention.** Convergence requires agreement on admitted actions and ordering of noncommuting actions. Graph closure and dependable constraints matter. IceCube's preliminary experiments found combinatorial search and strong sensitivity to constraint choice, including logs that needed cleaning [5]. Coalescing is therefore semantic log simplification, not mere timestamp compression. Telex permits collecting fully decided chunks after a later snapshot [6]; this is not proof that an indefinitely offline peer or restored backup can recover every discarded dependency. Constraints alone cannot reconstruct missing originals/evidence.

## 4. Algebraic reconciliation of inferred operations

Infer commands by comparing old and new states, then use laws about those commands to decide which combinations are safe. Two commands **commute** when applying them in either order gives the same result. The model can discover commands at synchronization time without recording every user gesture.

**Finding: representation/transition.** Define an algebra of create/edit/remove commands and laws of commutation/equivalence over structured state. Infer minimal sequences from old/new replicas, combine compatible commands, and retain conflicting residual commands for a separate resolution policy. Ramsey–Csirmaz provide sound/complete reasoning for their filesystem model and distinguish policies that leave conflicts at their sources from policies that disseminate conflict information [7].

**Application implications.** This is a complete formal reconciliation approach, rather than a demand to record every gesture. Minimal inferred commands support coalescing. Opaque record replacement preserves disjoint-field conflict detection; choosing field-level commands and commuting them does not. Parent/child algebra exposes aggregate dependencies, but “safe command composition” can be less conservative than Clacks policy. A policy selecting human field resolution is distinct from the algebra's compatibility calculation.

**Assumptions/cost/retention.** Laws are valid only for the specified operations and invariants. Generalizing from tree commands to relational records requires a domain proof, not unchanged reuse of filesystem laws. The paper's n-replica analysis starts from common state; rolling star comparisons, absent originals, and restored-peer recovery are not consequences of its algebra. Retention concerns the starting state and residual changes, even when no permanent operation history is stored. Its examples/formal results are evidence; no general empirical cost claim follows.

## 5. Operational transformation of context-dependent operations

Adjust an incoming operation to account for changes made since it was authored. In a text editor, inserting a character before another user’s target moves that target; transformation updates the position. This requires rules specific to the operations being transformed.

**Finding: representation/transition.** Represent document state, operations generated against particular contexts, causal context metadata, and an operation history. Transform incoming operations against concurrent effects so their positions/targets remain meaningful; execute transformed effects under a control algorithm. Convergence, causality preservation, and syntactic intention preservation are separate correctness properties [8]. Transformation functions and control conditions must be proved together.

**Application implications.** This directly handles concurrent sequence insertions/deletions; it does not infer which QSO values the operator wants. Automatically transforming and accepting two record edits can bypass D137. Record or aggregate delete needs explicitly defined transformations, not text-delete semantics. Client resolution can be a separate operation, but requiring it is application policy.

**Assumptions/cost/retention.** Coalescing operation sequences must preserve transformation context/effects. The 1998 history-buffer garbage collection (GC) scheme depends on knowledge of every participant's progress and reliable ordered channels [8]; indefinite disconnected members can impede collection under those assumptions. A checkpoint can replace old execution history only with adequate recovery/context rules. The prototype's experience supports feasibility in its editing setting, not arbitrary-member, backup-rollback correctness or QSO-scale performance.

## 6. Differential/shadow-state synchronization

Keep a shared comparison copy, called a **shadow**, alongside the live editable data. Each side computes differences against its shadow and sends patches. The shadow tracks agreement; it is not a second unresolved user proposal.

**Finding: representation/transition.** Maintain live state, paired shadows, pending diffs, and—in the reliable variant—a backup shadow and acknowledgments. Diff live state against its shadow; patch the peer and shadow; repeat symmetrically. Fraser gives per-client loops sharing server state, duplicate/loss handling, and fuller-text repair [9]. It captures changes from state rather than user interface (UI) operations.

**Application implications.** This is a distinct state/delta approach worth examining for streaming and coalescing. Its fuzzy patching can combine some differences while backing out others, so it does not implement whole-record human resolution unchanged. Shadow copies and proposal locality must be distinguished: a synchronized shadow is comparison context, not necessarily an unresolved business version.

**Assumptions/cost/retention.** Correctness depends on the domain's diff/patch behavior. The paper identifies diff/patch as expensive and limits its described protocol to one in-flight synchronization exchange. Its corruption/reinitialization fallback can lose changes [9]; therefore “self-healing” is weaker than Clacks's durable-work requirement. Deletion/aggregate dependencies, indefinite shadow retention, and old-backup repair need domain/recovery analysis, not an assumption that full-state resend fixes them.

## 7. Tentative transactions with primary certification

Clients keep proposed changes locally; a designated primary checks whether each proposed transaction can enter accepted state. For example, it can check contact and logbook dependencies together. “Tentative” means not yet accepted, not necessarily safe to discard.

**Finding: representation/transition.** Separate tentative local transaction effects from accepted primary state. Retain transaction procedures/inputs and acceptance predicates; submit/reexecute at the primary, accept or reject, then refresh the tentative replica. Gray et al.'s two-tier model is evidence for this family [10]. It allows disconnected proposals while coordinating accepted transactions.

**Application implications.** Transaction/read dependencies can span a QSO, its tag references and its external-record association. Contacts are account-owned; tags do not contain independent contact copies. Conditional shared acceptance is compatible with offline local progress: the guarantees apply to different operations. Transaction reexecution cannot silently replace a user comparison with freshly read values and overwrite intervening work. Equal-result and whole-record conflict interpretation must remain client policy; transaction serializability alone does not imply those rules.

**Assumptions/cost/retention.** Acceptance predicates determine admissibility; deterministic/replayable procedures matter. Coalescing working edits into one submitted transaction can fit, while irreversible service calls cannot be blindly reexecuted. The source's replication-instability scaling claims come from an analytic workload model, not a universal Clacks benchmark [10]. Primary authority is not proof its restored storage is newest. Surviving accepted-state/service evidence must be recoverable independently of unfinished proposals.

The particular two-tier algorithm discards tentative object versions before refreshing and defines transaction durability at base commitment [10]. Clacks's durable unresolved working values must not inherit that discard rule; separating pending proposals from accepted state is a relevant generic model distinction, not endorsement of its exact lifecycle.

## 8. Observable consistency formalisms

Specify what operations are allowed to observe, rather than choosing a storage structure. “Read your writes,” for example, means a client’s later reads include its earlier writes. **Safety** rules forbid incorrect outcomes; **liveness** rules require progress when their conditions hold.

**Finding.** Abstract executions contain operations, session/program order, visibility, arbitration, and type-specific return rules. Visibility says whose effects an operation observes; arbitration orders competing effects. These are specification structures, not requirements to persist every event. Viotti–Vukolić survey these distinctions [11]. Session guarantees constrain read-your-writes, monotonic reads/writes, and writes-follow-reads, potentially reducing reachable-server availability [12].

Recent work provides complementary formal lenses: the Shared Object Pool model separates ordering-lineage convergence constraints from inter-operation relationships and demonstrates small-history consistency checking [13]; Almeida's framework separates convergence safety from eventual-delivery liveness and permits specifications beyond sequential executions [14]. These models do not prescribe conflict UI, transfer logs, deletion storage, or recovery retention.

**Application implications.** Specify guarantees separately for local pending views and shared accepted operations. A stable accepted history need not incorporate unresolved local proposals. Client “read my local edit” does not mean the server accepted it. Causal visibility does not choose field values; an arbitration order need not be last-writer-wins. A restored server may regress previously observed state unless recovery handles surviving knowledge. Reauthorization is an authorization transition, not intrinsically a new data replica. Consistency checking tests recorded behavior; it cannot reconstruct facts missing everywhere.

## 9. Invariants and coordination boundaries

Ask whether independently valid changes remain valid when combined. Two clients can each make a valid reservation yet exceed a shared limit when merged. In contrast, combining immutable observations by union preserves the observations without needing a winner.

**Finding.** Invariant confluence evaluates an invariant, operations, and merge together: do independently valid reachable states merge to valid state? Consistency as logical monotonicity (CALM) characterizes when computation can proceed without coordination: adding input facts must not invalidate earlier conclusions [15,16]. Neither is a storage algorithm. Relational tuples, dependency graphs, sets of facts, and abstract merge functions are their modeling structures.

**Application implications.** An immutable service-evidence collection can grow while current confirmation decreases; projection and historical evidence are different semantics. Referential integrity preserved by cascading deletion is weaker than preserving concurrent child work for user resolution. Analyze that actual invariant. Gathering surviving facts and deciding no further relevant facts exist are also different computations. Unbounded offline return defeats an unsupported claim that silence proves absence. Conditional acceptance may coordinate shared admission while offline work remains available. These results constrain designs without selecting one.

## Evidence limits

No cited model automatically proves Clacks's complete retention/restore contract. Published assumptions must be evaluated against surviving synchronized records, service outcomes/processing decisions, missing originals, and unavailable ordering. The analysis identifies those assumptions; it does not construct a new protocol or select an encoding.

## Primary bibliography

1. Saito, Shapiro, [*Optimistic Replication*](https://dsf.berkeley.edu/cs286/papers/optimistic-csur2005.pdf), 2005.
2. Pierce, Vouillon, [*What's in Unison?*](https://www.cis.upenn.edu/~bcpierce/papers/unisonspec.pdf), 2004, formal specification/reference program.
3. [Unison author-maintained manual](https://raw.githubusercontent.com/bcpierce00/unison/master/doc/unison-manual.tex), checked 2026-10-13, pairwise archives/star evidence only.
4. Terry et al., [*Managing Update Conflicts in Bayou*](https://www.scs.stanford.edu/nyu/01fa/sched/bayou.pdf), 1995.
5. Kermarrec et al., [*The IceCube Approach to the Reconciliation of Divergent Replicas*](https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/podc2001.pdf), 2001.
6. Benmouffok et al., [*Telex: A Semantic Platform for Cooperative Application Development*](https://perso.lip6.fr/Marc.Shapiro/papers/2009/Telex-CFSE-2009.pdf), 2009.
7. Ramsey, Csirmaz, [*An Algebraic Approach to File Synchronization*](https://www.cs.tufts.edu/~nr/pubs/sync.pdf), 2001.
8. Sun et al., [*Achieving Convergence, Causality Preservation, and Intention Preservation in Real-Time Cooperative Editing Systems*](https://www.cs.cityu.edu.hk/~jia/research/reduce98.pdf), 1998.
9. Fraser, [*Differential Synchronization*](https://neil.fraser.name/writing/sync/), 2009, author text.
10. Gray et al., [*The Dangers of Replication and a Solution*](https://dsf.berkeley.edu/cs286/papers/dangers-sigmod1996.pdf), 1996.
11. Viotti, Vukolić, [*Consistency in Non-Transactional Distributed Storage Systems*](https://arxiv.org/abs/1512.00168), 2016 publication.
12. Terry et al., [*Session Guarantees for Weakly Consistent Replicated Data*](https://classes.cs.uchicago.edu/archive/2026/spring/23380-1/papers/terry_sessionguarantees.pdf), 1994.
13. Hu, Arpaci-Dusseau, Arpaci-Dusseau, [*A Unified, Practical, and Understandable Model of Non-transactional Consistency Levels in Distributed Replication*](https://arxiv.org/abs/2409.01576), 2024, revised 2025.
14. Almeida, [*A Framework for Consistency Models in Distributed Systems*](https://arxiv.org/abs/2411.16355), 2024 preprint.
15. Bailis et al., [*Coordination Avoidance in Database Systems*](https://www.vldb.org/pvldb/vol8/p185-bailis.pdf), 2014.
16. Hellerstein, Alvaro, [*Keeping CALM: When Distributed Consistency Is Easy*](https://arxiv.org/abs/1901.01930), 2019 preprint / 2020 publication.

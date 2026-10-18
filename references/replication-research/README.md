# Distributed replication research

The current research surveys published replication approaches, abstract data models, and data structures against the [agreed synchronization properties](../../docs/synchronization.md). conflict-free replicated data types (CRDTs) and vector clocks are examples, not an exhaustive scope. Software systems are supporting case-study evidence; no implementation or clock encoding is selected.

Start with the recommendation for the proposed Clacks combination, then use the survey to compare alternatives. The focused reports explain the individual models and their assumptions; reading the source papers is optional for understanding the notes.

Replication keeps copies of data on different machines. **Reconciliation** decides how their differences should be handled; **causal metadata** records which changes a copy knows about; **anti-entropy** discovers and repairs missing state. **Retention and garbage collection** determine which history or metadata can be discarded safely. These are separate jobs, not interchangeable algorithms.

- [Vector-based recovery fit](recovery-fit.md): published recovery mechanisms, exact assumptions, and the current lack of a verified complete fit.
- [Recommended combination](recommended-model.md): concrete model-level proposal, compatibility, and tradeoffs.
- [Survey and property comparison](approaches-survey.md): approach families, representations, fit, and researched mechanisms.
- [Causal and knowledge structures](approaches-causality.md): vectors, dots/contexts, shared knowledge, bounded and dynamic identities, and causal graphs.
- [Reconciliation and consistency models](approaches-reconciliation.md): common-state comparison, operation/dependency models, algebras, transformation, and consistency specifications.
- [Convergence, transfer, and retention](approaches-convergence-transfer.md): lattice/operation/delta models, datatypes, anti-entropy, and stability/garbage collection (GC) assumptions.

The focused reports cite primary research and distinguish published results from Clacks-specific inference. The survey is bounded literature analysis, not a correctness proof or an implementation plan.

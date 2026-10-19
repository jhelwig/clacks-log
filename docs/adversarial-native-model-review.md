# Native-model adversarial review dispositions

Panel date: 18 October 2026. Reviewed revision: `afb001d8f8f46c18fb8d497d4dcd4c67d0f3d3ca`, in [PR #39](https://github.com/jhelwig/clacks-log/pull/39).

Three independent reviewers examined native invariants, catalogs/reference export, and documentation/architecture conformance. The checkout stayed frozen during review. The user subsequently clarified the acronym rule and authorized fixes for all three P2 findings. This document records their disposition; it does not claim a second independent review of the fixes or completion of issue #1 or a roadmap phase.

## CAT-1 — P2 — Resolved: recognition labels could collide under lookup comparison

The catalog loader checked uniqueness case-sensitively, while recognition uses ASCII case-insensitive lookup. A supplied catalog with `X` at 100–200 hertz and `x` at 300–400 hertz validated: deriving the band for 350 hertz returned `x`, but interpreting it selected the first row, `X`. Likewise case-colliding submodes could select different parent/deprecation metadata when rows were reordered. Two reviewers independently reproduced this single defect; it is counted once. No collision in the bundled baseline was demonstrated.

**Resolution:** [catalog validation](../lib/clacks-model/src/recognition.rs) now rejects labels colliding under the same ASCII case-insensitive comparator used by recognition, separately within bands, modes and submodes. Accepted labels retain their original spelling; recorded identities and data-driven recognition remain unchanged.

**Regression evidence:** `catalog_rejects_labels_that_collide_during_recognition` in [recognition tests](../lib/clacks-model/tests/recognition.rs) first failed because a conflicting band pair was accepted. After the fix it passes for bands, modes and submodes in both row orders, including conflicting ranges, parent and deprecation metadata. Existing full-baseline and supplied-catalog extension tests also pass.

## DOC-1 — P2 — Resolved: product overview described implemented choices as unapproved

The product introduction still said implementation was unauthorized and native-model engineering proposals awaited approval; its catalog paragraph repeated that status.

**Resolution:** [product scope](product.md) now distinguishes the authorized/executed issue #1 plan and its implemented engineering choices from later application work. PR #39 remains unmerged. Engineering choices are not retroactively presented as individually approved product decisions.

## DOC-2 — P2 — Resolved: product summary reopened server process placement

The product workflow summary left web-interface process placement open despite D213/D223, the account documentation and the historical AR4 resolution.

**Resolution:** [product scope](product.md) now explicitly places web, synchronization and background workers within one Clacks server application alongside PostgreSQL. Exact callback/protocol integration and deployment tooling remain engineering work, without reopening the selected process topology.

## DOC-3 — Withdrawn: definitions of QSO, UI and MVP

The reviewer applied the former blanket acronym rule too broadly. The user clarified that the rule covers specialist technical jargon, not broadly familiar terms or vocabulary familiar to novice-to-intermediate amateur radio practitioners. QSO, UI and MVP do not need definitions. [Collaboration guidance](../AGENTS.md) was corrected in `8c0d33d`; no glossary additions are required for this finding.

## Questions, not accepted changes

- The optional-aggregate helper does not normalize every optional equipment/reference text value. Other construction helpers may prepare those values; no invalid saved-record bypass was shown. Its intended preparation boundary remains a possible engineering clarification, not a confirmed defect or an authorized expansion of this fix.
- Caller-supplied profile/catalog maps can contain keys inconsistent with embedded IDs. The registry invariant may belong to the caller; the panel did not establish a correctness defect or require validated registry types.

## Verification and limits

After the fixes, all 51 integration tests and the compile-fail documentation test pass offline with Rust 1.99.1. Formatting, Clippy with warnings denied, Windows cross-compilation, local documentation links/anchors/fences and whitespace checks pass. The regression was observed failing before the code change and passing afterward. Documentation corrections were compared against the authorized plan, D213/D223 and account guidance.

During review, the native-invariants reviewer additionally ran 600,000 sampled geometry/serialization checks; these are supplementary evidence, not exhaustive proof. The review did not certify upstream catalog extraction, deployed persistence/synchronization, full format adapters, provider integration or publication workflows. Those remain downstream capabilities. No critical or high-severity defect was demonstrated by the bounded panel review; this is not a claim of complete correctness.

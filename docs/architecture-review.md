# Architectural review

Initial review **17 October 2026** covered D01–D242; the follow-up review on the same date covers the integrated documentation through **D251**. This is a review of documented architecture and requirements, not an implementation/code audit. No application implementation, live service operation, dependency build, formal replication verification or deployment test was performed. Findings distinguish contradictions in current guidance from already selected engineering validation obligations; recommendations are not newly accepted product decisions.

## Relation to current implementation review

This file records the October 17 document-level reviews through D251. It does
not certify the later issue #1 implementation or D256. The
[native-model execution evidence](native-model-execution.md) records the code
review at `9ad572b`, regression fixes at `584d99c`, and later data-driven
recognition correction at `86baeb6`. The adversarial panel reviewed `afb001d`;
the user-authorized fixes for all three P2 findings were committed in `a7ed604`:
catalog label uniqueness now uses recognition’s ASCII case-insensitive comparison,
and the product overview correctly records native-model implementation status
and the selected single-server-application topology. The catalog regression
failed before the fix and passes afterward; all 51 integration tests and the
documentation test pass. These fixes were verified, not independently re-reviewed.
The acronym finding was withdrawn following the user’s clarification in `8c0d33d`.
AR1–AR4 retain their historical resolved status.

## Findings

### AR1 — Resolved in design: multiple local contacts can compete for one external publication target

Original review evidence (before the tag-model refinements): [Clacks logbooks and publication scope](integrations.md#clacks-logbooks-and-publication-scope) permits multiple local copies, independent upload enablement and multiple Clacks logbooks routed to the same QRZ logbook. [Destination routing](../references/service-routing.md#accepted-routing-direction-d53) selects a service destination by saved station callsign/date. The [publication transaction boundaries](../references/server-persistence.md#transaction-boundaries) retain actual dispatched values and attempt outcomes, but do not specify ownership or arbitration when different local contact identities target the same external record. D111 handles incoming confirmation association, which does not resolve competing outgoing payloads.

Concrete case:

1. A contact is copied by export/import from an activity logbook into the main logbook. Both local records remain and have distinct IDs, as permitted.
2. Both route to the same external destination, with the same service-side identifying fields. One copy is corrected to a different own-station grid while the other retains the earlier grid.
3. Uploading one and then the other can change the same external contact twice. Both local attempt histories can truthfully contain success, while the remote record has only the last applied values. Ordinary local conflict resolution never runs, because these are different local IDs.

This is a provider-supported possibility, not a claim that the case has been reproduced live: [Logbook of The World (LoTW) correction rules](https://lotw.arrl.org/lotw-help/submitting-qsos/) say exact matching identifying fields permit replacement of station-location details. [QRZ's interface](https://www.qrz.com/docs/logbook/QRZLogbookAPI.html) can replace an existing duplicate and returns its remote record ID. Those provider identities are not Clacks local IDs. The QRZ duplicate-key details still require provider-specific verification; the LoTW case already establishes the general architectural issue.

Consequence: per-local-record coordination is not enough to explain cross-copy replacement, status projection or retries. Historical successful submission must remain distinguishable from a claim about the current external contents. This review does not assume the product must keep every local copy equal to the external service, or that past success becomes false after another copy publishes.

Recommendation: specify external publication identity and coordination in addition to local contact identity. Equal payloads can potentially share coverage; conflicting payloads require a deliberate ownership/arbitration policy, or an explicit decision that later submissions may replace earlier copies with appropriate diagnostics. Do not silently select a winner by worker timing. Maintain separate local records and their provenance; this does not call for merging logbooks or a new service. Exact indexing/locking is engineering work. **The intended behavior when copies disagree is the focused product/architecture issue**, because existing independent upload switches and incoming match choices do not answer it. No proposed arbitration rule is accepted by this review.

#### Resolution through D251

D243–D250 replace organizational copies with account-owned QSOs carrying multiple tags, one synchronization scope per account, direct membership editing and account-wide import matching. The activity-to-main workflow adds a tag to one QSO, rather than creating competing publication records. Tag deletion distinguishes preserving contacts from marking them deleted; deleted-tag references require explicit removal/replacement while pending work remains local.

D251 also addresses genuinely distinct local QSO identities: within an account/service destination, at most one local QSO may associate with an external record. Ambiguous matching selects at most one QSO, superseding D111/D227’s multiple association. An outgoing QSO targeting an already-associated external record requires resolution rather than a second association or silent replacement. Local duplicates remain possible without sharing the external association; a QSO can associate independently with other services/destinations.

**AR1 is resolved at the policy/architecture level.** Provider-specific identity detection, transactional uniqueness and competing/in-flight attempt validation remain engineering obligations. This resolution does not claim an implementation has been built or tested.

### AR2 — Resolved: superseded source-of-truth wording corrected

Pre-correction evidence: [synchronization integration interaction](synchronization.md#integration-interaction) said D33/D36 automatically change recorded contact values from downloaded station details, advance the contact revision and cause user conflicts with local edits. The opening synchronization discussion also called D36's revision behavior current. D237 explicitly supersedes this replacement model, and D238 selects separate provider information with logical fallback. [Current integration guidance](integrations.md#reconciliation-context) follows those later decisions.

Concrete case: a user records a location from an on-air exchange while a confirmation download supplies a different location. Following the stale synchronization paragraph would overwrite the recorded facts or manufacture a recorded-contact conflict. Following D237/D238 preserves the facts and updates separate provider information. These are incompatible behaviors, not two encodings of the same protocol.

Recommendation: reconcile the synchronization document to D237/D238, explicitly separating recorded-contact reconciliation from synchronized provider evidence and effective output. The decision register already resolves the policy; no new approval is needed. Update related acceptance scenarios and remove current-tense instructions that rely on the former enrichment model. Do not change unrelated contact conflict semantics or directory-snapshot arbitration.

Lesser documentation drift reinforces the need for that pass: [accounts deployment](accounts.md#mvp-server-deployment-d189) still calls database selection open and describes an unselected process topology despite D213. [LoTW's introduction](lotw.md) retains horizontally scalable execution wording that is not an accepted additional deployment requirement. These are documentation corrections, not reasons to redesign persistence or add services.

### AR3 — Resolved: persistence comparison reconciled to account scope

Pre-correction evidence: [persistence comparison, architecture introduction](../references/server-persistence.md#architecture-comparison) says logbook lifecycle and the affected contact have different application synchronization scopes (`references/server-persistence.md:48`). The same document’s summary and requirements table correctly place synchronized account data in one account scope, as do D248 and [the synchronization contract](synchronization.md#synchronization-scope-d248).

A reader following the architecture introduction could unnecessarily design cross-scope publication/checkpoints for a tag-deletion operation, despite the accepted account-wide scope. The need for atomic reference validation is valid; its stated scope rationale is obsolete.

Recommendation: describe tag definitions, QSO tag sets and service evidence as sharing an account scope, while preserving transaction-level reference/compared-state checks. No new scope decision or protocol redesign is needed. **Resolved on 17 October 2026:** the persistence comparison now places tag lifecycle, QSO tag sets and service evidence in one account synchronization scope, retaining atomic reference, compared-state and association checks without requiring an account-wide transaction. No scope decision or protocol redesign was introduced.

### AR4 — Resolved: enrollment guidance follows the selected topology

Pre-correction evidence: [browser-based client authorization](accounts.md#browser-based-client-authorization-d65) says the Clacks web UI may be served by a separate authentication/management component and that no process topology is selected (`docs/accounts.md:53`). The [MVP web-UI section](accounts.md#accepted-mvp-web-ui-scope-d66) also leaves web-component deployment open (`docs/accounts.md:63`). The same document’s [deployment section](accounts.md#mvp-server-deployment-d189) explicitly selects one Clacks application containing web, synchronization and workers alongside PostgreSQL (`docs/accounts.md:97`), following D213/D223.

An implementer could interpret the earlier wording as permission to introduce a separately deployed Clacks management service or a second application boundary. An external OpenID Connect identity provider is compatible with the selected topology; it does not make Clacks management a separately selected service.

Recommendation: state that Clacks enrollment/management endpoints are part of the selected application and leave only callback/protocol integration, layout and deployment tooling open. **Resolved on 17 October 2026:** enrollment and web-UI guidance now places Clacks management endpoints within the selected application alongside synchronization and workers. The external identity provider remains separate; layout, protocol integration and deployment tooling remain engineering work. No new topology decision was introduced.

## Follow-up assessment through D251

The integrated account-owned QSO/tag model introduces no additional architectural blocker found in this review. The account is the ownership and synchronization boundary; the QSO including its full tag set is the ordinary comparison unit. Tag definitions are separately identified records. Direct membership editing, active untagged contacts, QSO deletion state, explicit tag-deletion choices and pending deleted-tag references have distinct specified outcomes. Ordinary conflicts between tag and fact edits are accepted MVP behavior, not findings.

Service publication uses accepted state, evaluates all current tags with Forbid precedence and retains actual sent-value evidence. One external record can associate with at most one QSO per account/service destination. That invariant prevents fan-out to organizational copies; distinct local duplicates require resolution before competing publication. Provider identity, atomic uniqueness enforcement and in-flight races remain explicit implementation validation obligations, not an unspecified ownership policy.

Synchronization, recovery, account/login identity, credential boundaries and local export rules were checked for consistency with those record boundaries. The independent read-only review of deletion, matching, publication and persistence found the same two stale passages and no additional defect. AR1 and AR2 remain resolved in design/documentation. AR3 and AR4 were corrected after the review; all four findings are resolved in design/documentation. No accepted decision needs reopening on the evidence reviewed.

## Architectural assessment

The selected components otherwise form a coherent architecture for the stated scope:

- The native desktop owns durable offline working state and user resolution; the server owns accepted shared state and publication. Neither a server conflict queue nor client publication workers are necessary.
- PostgreSQL in one server application supplies a transactional domain spanning catalog, contact, evidence and job invariants. D248 now places synchronized account data in one scope per account, without requiring an account-wide transaction. SQLite supplies the corresponding local durable-installation boundary without a custom writer service.
- Independent internal accounts, explicit external login bindings and revocable enrolled clients provide distinct identity boundaries. Per-account encryption keys protect stored credentials under a deployment wrapping key; directory secrets intentionally reach clients, while signing keys do not.
- Immutable service evidence, recorded facts, provider snapshots and effective fallback values have distinct purposes. Keeping them separate fits recovery, provenance and the rejection of automatic overwrites.
- Dynamic version vectors, timeline ancestry and authorized-client retention fit the accepted conservative recovery model. Their selection does not prove a concrete protocol correct; it also does not establish a need to replace them with a graph, quorum or different storage product.
- The Windows-first desktop, cross-platform foundations, pure-Rust signing, Linux-container server, Leptos/Axum web direction and selected geographic sources are consistent with their scope decisions. No contradictory framework or database choice was found.

This assessment is a document-level fit judgment, not a performance, security or interoperability certification.

## Existing engineering validation obligations, not additional architecture findings

These risks were already recorded and should be verified when the relevant work is authorized. They are not new product questions merely because their proofs or mechanisms are unwritten.

| Boundary | What must be demonstrated | Existing basis |
| --- | --- | --- |
| Restored histories | Reused counters are not mistaken for identical accepted histories; retained evidence supports recovery or explicitly explained uncertainty. Include a restored server advancing before an older client reconnects and rollback below a compacted baseline. | D144/D148/D154–D156; [recovery analysis](../references/replication-research/recovery-fit.md). No mandatory new actor on every restart or storage quorum follows. |
| Transactions and transport | Conditional acceptance, tag/QSO deletion, unique external associations, durable acknowledgment and retry preserve accepted state and pending evidence through interruption. Provider calls remain outside database transactions. | D146/D156/D235/D243–D251; [persistence boundaries](../references/server-persistence.md#transaction-boundaries). |
| Provider information provenance | A returned copy of our own uploaded values is not relabeled as the other station's report merely because confirmation is true. Only information with established source/applicability participates in D238 fallback. | D32/D237/D238; [service model](../references/service-state-model.md#primary-evidence). [QRZ explains](https://www.qrz.com/docs/logbook30/confirmations-how) that the parties' reports are independent and the other party's report is not disclosed. |
| Credential boundaries | Requests, streams, workers, logs and secret distribution enforce account context; removal stops subsequent permitted secret use. Disconnected clients cannot be assumed to forget credentials already received. | D62/D63/D97/D194/D228/D230/D236; [accounts](accounts.md) and [credential protection](../references/server-credential-protection.md). |
| Pure-Rust signing | Compatible released dependencies, required archive/certificate semantics, independent signature checks, signing-path assessment and actual LoTW acceptance. | D224; [backend assessment](../references/lotw-cryptography.md). [RustSec's advisory](https://rustsec.org/advisories/RUSTSEC-2023-0071.html), rechecked today, still lists no patched versions; this is not proof of a specific Clacks signing exploit or grounds to reopen OpenSSL. |
| Geographic lookup | Review six International Telecommunication Union (ITU) zone topology corrections, date-line handling, the explicit DX Century Club (DXCC) mapping and exceptions, dataset versions and grid-area coverage. | D184/D239; [geographic comparison and audit](../references/location-lookup.md). A selected source is not a compiled or verified shipping dataset. |

D240's explicit client backup/restore implementation and D241's hosted launch remain outside MVP requirements. This review does not reopen them as release blockers, promise universal recovery of copied client state or demand hosted operational machinery.

## Disposition

AR1 is resolved by the integrated account-owned contact/tag model and unique external-record associations (D243–D251). The current contracts are documented in [product ownership and workflows](product.md#contacts-logbooks-and-tags), [synchronization](synchronization.md) and [external publication](integrations.md#clacks-logbooks-and-publication-scope). Provider-record identity and atomic enforcement remain implementation verification, not an unresolved policy.

AR2 was corrected at the user's request on 17 October 2026: synchronization guidance follows separate provider information and fallback under D237/D238, deployment wording follows D213, and the unsupported horizontal-scaling requirement was removed. Its evidence above records the reviewed pre-correction wording.

D242 remains the historical planning-completion checkpoint. Subsequent decisions are integrated into the current architecture. The follow-up review identified AR3/AR4 as documentation contradictions; both are now corrected under the existing decisions. No new database, framework, separately deployed service or broad restart of planning was required. No implementation work was started.

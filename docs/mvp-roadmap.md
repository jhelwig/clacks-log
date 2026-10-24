# Clacks Log MVP roadmap

This roadmap organizes delivery of the agreed minimum viable product (MVP) into
phases and observable milestones. It describes what becomes usable, the major
dependencies and what establishes completion. Detailed implementation plans are
separate work, prepared for the milestone being undertaken.

Scope: accepted decisions D01–D256.

Delivery is tracked through [GitHub issues](https://github.com/jhelwig/clacks-log/issues) and [milestones](https://github.com/jhelwig/clacks-log/milestones).
Each issue starts with brainstorming to produce a reviewed specification and
implementation plan before coding. Creating the backlog does not mark any phase
complete or authorize implementation directly from issue descriptions.

The [product model](product.md) and [decision register](decisions.md) define the
scope. [Deferred scope](product.md#deferred-and-uncommitted-scope-boundaries) is recorded with the product contract. A radio contact is a QSO. Logbook of The World
is abbreviated LoTW; Parks on the Air is POTA; OpenID Connect is OIDC; Amateur
Data Interchange Format is ADIF, with its text representation called ADI.

Delivery issues produce production components, data tooling and artifacts that
later work integrates and extends rather than replaces. Tests and fixtures verify
those deliverables; a separate executable model, disposable prototype, artificial
runtime or evidence-only demonstration does not satisfy implementation scope.
Later integration adds persistence, transport, workflows or coverage to the same
components without treating the earlier implementation as throwaway work.

## Delivery sequence

| Phase | Milestone outcome | Main dependency |
| --- | --- | --- |
| 1. Foundations and early risks | Production native-model, synchronization, service-evidence, certificate/signing and geographic-data foundations are implemented and verified. | Accepted product and architectural requirements. |
| 2. Durable offline logging | A Windows operator can record, edit, delete, organize and reopen contacts without the server. | Phase 1 native model. |
| 3. Accounts and synchronized operation | Independently writable clients enroll, reconcile and recover access through an isolated account; directory setup is available. | Phase 2 durable state and Phase 1 synchronization/service-evidence foundations. |
| 4. Complete logging workflows | Capture, lookup, geographic assistance, ADI interchange and migration of unrelated populated logs work together. | Phase 3 account synchronization, directory configuration and protected credential provisioning. |
| 5. External services and recovery | Automatic/manual publication, confirmations and operational controls work safely with accepted account state. | Phase 3 evidence synchronization and association safeguards, Phase 4 interchange and Phase 1 certificate/signing components. |
| 6. Release readiness | Installed applications, deployment and recovery procedures demonstrate the complete MVP. | Integrated outcomes from Phases 1–5. |

The sequence expresses cumulative completion, not a requirement to finish every
activity in one phase before investigating another. In particular, certificate
compatibility and geographic-data preparation begin early, while offline logging
and synchronization are developed. Integration must respect their dependencies;
parallel preparation does not mean independent applications or deployed services.

Production observability is a continuous requirement for every phase, including
the libraries delivered before application startup and telemetry export exist.
Each capability includes relevant tracing, structured events, metrics and their
verification under the [observability contract](observability.md). Later phases
connect and validate those components in real deployments.

## Phase 1 — Foundations and early risks

The [native-model contract](native-model.md) and [synchronization model](synchronization-model.md)
define the corresponding foundations. Their crate guides describe implementation
and adapter boundaries; GitHub records capability status.

Milestones:

- Establish a native model covering contact identity, timestamps, modes,
  station snapshots, reusable equipment, profiles, tags and unified activity
  context with own-station/contacted-station reference sets. Recognition and
  source separation preserve recorded meaning across clients and upgrades.
- Implement the production synchronization and recovery logic for whole-record
  comparison, deletion, offline edits, timeline recovery, authorized-client
  retention and actor retirement. Explain conservative reconciliation when
  ordering evidence is unavailable. Later storage and transport work integrates
  and extends this implementation; it does not replace a separate simulation.
- Implement production shared service-evidence and external-record association
  components for imports, synchronization and later publication. Preserve provenance and recovery
  meaning; an external record may associate with at most one local QSO within an
  account and service destination. These foundations precede imports and live
  jobs; live provider integration remains in Phase 5.
- Implement source-guided pure-Rust LoTW certificate handling and signing
  components, with independently checked compatibility. Later credential
  management and publication integrate these components.
- Implement the production geographic-data preparation pipeline and artifact
  format using the selected sources, with verified provenance and redistribution
  rights. Validate representative derived artifacts through that pipeline; later
  geographic assistance extends its coverage and consumes its artifacts. The
  [geographic-pipeline specification](geographic-pipeline.md) consolidates the
  agreed behavior and proposed engineering contracts for this capability.

Completion means production foundations implement the accepted behavior with
meaningful failure and compatibility verification. Phase 1 need not deliver all
later user workflows or complete geographic coverage, but its implemented
components and data tooling must carry forward into those capabilities. A small
placeholder catalog, a collection of type declarations or tests of only happy
paths does not establish this milestone.

## Phase 2 — Durable offline logging

Milestones:

- Establish the production Windows desktop application under the
  [desktop specification](desktop.md), using the selected egui direction and
  durable client store before delivering its operating-context,
  tag, contact-entry and history workflows. Later capabilities extend the same
  application; installed release packaging remains in Phase 6.
- Deliver Windows manual contact entry with the agreed timestamp behavior,
  optional operating profiles, equipment selection and historical snapshots.
- Support editing saved contact facts and deleting individual contacts offline,
  independently of tag changes. Saved edits appear in local working history;
  pending deletions leave normal views while remaining available for review.
- Support offline tag creation, naming, renaming, membership changes and deletion
  choices. Contacts can belong to multiple tags or remain untagged; removing
  membership and deleting a contact have distinct effects.
- Provide local history and Worked Before using the saved working state, with
  the agreed filters. Data survives normal close, restart and interrupted work.

Completion means an operator can use the desktop for local logging without a
server or provider connection and reopen the resulting log. Interaction review
and durable-storage evidence establish usability and persistence together.

## Phase 3 — Accounts and synchronized operation

Milestones:

- Deliver isolated accounts, OIDC sign-in and identity linking, browser-based
  desktop enrollment, individual-client revocation and administrator-assisted
  account recovery.
- Support reauthorization of an existing installation without discarding its
  synchronized data or pending work. Previously unsynchronized edits excluded by
  revocation require explicit manual reconciliation. Joining unrelated populated
  logs is a separate Phase 4 interchange workflow; no automatic merge is implied.
- Synchronize all account contacts, tags, service evidence and shared configuration
  to every enrolled client. Provide catch-up and streaming, client-side conflict review,
  missing-reference resolution and preservation of pending work while unrelated
  synchronization proceeds. Preserve recovery-relevant outcomes, observations and
  processing decisions, and prevent competing claims to one external record.
- Establish web account/client management, including safeguarded login-identity
  linking and unlinking, and account credential protection. Deliver account-level
  directory-provider selection and web credential configuration with protected
  provisioning to enrolled clients before Phase 4 lookup.
- Establish production server tracing, logs and metrics. The web architecture
  accommodates its agreed future scope beyond administration; publication and
  confirmation-service setup and diagnostics finish with Phase 5.

Completion means multiple clients can edit independently, reconnect and reconcile
without losing work or exposing one account's state to another. Recovery from an
older server state, indefinite authorized offline return and safe retention must
be demonstrated for core account state and available service evidence. Phase 5
extends that evidence to actual provider effects and history. Reauthorization
preserves existing local state; it does not use the empty-log migration path.

## Phase 4 — Complete logging workflows

Milestones:

- Integrate WS Suite capture and live other-station callsign updates, source
  ownership, completion deduplication and the agreed Generate Messages action.
  WS Suite refers to the WSJT-X family of weak-signal radio applications.
- Deliver direct client lookup through the account-selected QRZ or HamQTH
  provider, separate synchronized directory snapshots, late results and
  user-triggered selected/filtered bulk lookup. Logging continues when lookup or
  the synchronization server is unavailable.
- Deliver bundled offline geographic assistance with required United States
  coverage. Pursue desired worldwide coverage with at least DX Century Club
  (DXCC) entity, CQ operating zones and International Telecommunication Union
  (ITU) zones, without making worldwide coverage an additional mandatory release
  gate. Retain point-versus-area meaning, ambiguity handling and manual correction.
- Deliver prepared ADI import with account-wide matching, explicit choices,
  partial results and diagnostics; provide consistent local-state export with
  ordinary and optional POTA-compatible output and destination protection.
- Support joining an unrelated independently populated log through export,
  enrollment with an empty local log, and re-import. This milestone follows ADI
  import/export availability and retains the existing matching and association
  safeguards; it does not add automatic populated-log merging.

Completion means these workflows operate against the same contact and source
model. Recognized positive imported upload and confirmation statuses are accepted
immediately with available dates and provenance, without verification calls;
missing upload status remains pending independently per service. Imports preserve
association uniqueness and synchronized recovery evidence before live publication
arrives. Imports, capture and lookup do not silently overwrite recorded exchange
facts or create organizational copies of contacts. Required United States
geographic coverage is demonstrated; worldwide results and limits are documented
against the desired scope.

## Phase 5 — External services and recovery

Milestones:

- Deliver LoTW certificate import, configuration, explicit supersession and
  credential removal, together with routing for multiple LoTW certificates and
  QRZ logbooks. Preserve public metadata and submission history.
- Deliver server-side publication using current accepted effective contact values,
  tag policies, trusted imported statuses, diagnostic retries and affected-only
  pauses. Unresolved client proposals do not become publishable state.
- Provide explicit manual uploads, live service status and routine operational
  controls in the desktop, with full publication/confirmation setup and detailed
  upload diagnostics in the web interface. Retry and pause/resume actions retain
  the agreed routing, eligibility and failure-remedy rules.
- Deliver confirmation downloads, explicit association of ambiguous records,
  duplicate/rejection/deletion handling and provenance-aware confirmation facts.
  Local deletions leave external copies untouched.
- Demonstrate restored-server reconciliation and collection with service evidence
  and publication history included. Verify interoperability with both services
  through controlled records, not solely simulated responses.

Completion means submissions and observations remain truthful through retries,
concurrent edits and recovery. Operator-controlled upload pause and explicit
resume are part of restore behavior; recovery cannot recreate information lost
from every available copy.

## Phase 6 — Release readiness

Milestones:

- Validate the complete operating workflow across real client/server storage,
  account isolation, disconnection, restart, conflicts and recovery failures.
  Include existing-client reauthorization, the separate ADI migration workflow,
  manual publication and native service controls.
- Produce and verify the installed Windows desktop and Linux-container server,
  including geographic data, dependency notices, protected credential storage,
  user-facing diagnostics and production observability.
- Exercise documented deployment, backup, restore, upgrade and account-recovery
  procedures using the actual artifacts. Include deployment wrapping-key recovery
  and the external-upload pause/reconcile/resume procedure.

Completion means the artifacts and operating procedures support the agreed MVP,
with recorded interoperability and recovery evidence. A successful development
build alone is insufficient. Built-in backup/restore commands and launching a
hosted subscription are outside this milestone's required scope.

## Scope and planning boundaries

The selected architecture remains Rust 1.99.1, egui on the desktop with SQLite,
and one Axum/Leptos server application with PostgreSQL. Libraries belong under
`lib/<crate-name>` and thin application wrappers under `bin/<application-name>`.
Windows is the MVP desktop target; tooling and architecture remain suitable for
later macOS/Linux clients. The server targets Linux containers.

The roadmap does not prescribe a crate inventory, file ownership map, database
schema, Rust type, function signature, protocol encoding, test name or coding
sequence. Those belong in the detailed plan for the milestone being undertaken,
with traceability to the agreed requirements and explicit treatment of remaining
engineering choices. Neither a new product-decision queue nor coding directly
from these milestone descriptions is the next step.

Deferred and uncommitted capabilities retain their status in the
[product contract](product.md#deferred-and-uncommitted-scope-boundaries).
No calendar estimate is asserted. Milestone completion is based on observed
capability and validation evidence.

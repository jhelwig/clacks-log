# Clacks Log MVP roadmap

This roadmap organizes delivery of the agreed minimum viable product (MVP) into
phases and observable milestones. It describes what becomes usable, the major
dependencies and what establishes completion. Detailed implementation plans are
separate work, prepared for the milestone being undertaken.

**Status:** No phase is complete. The initial implementation was discarded;
implementation remains stopped. This roadmap does not resume it.

The [product model](product.md) and [decision register](decisions.md) define the
scope. The [remaining-work index](open-questions.md) distinguishes engineering
work from deferred capabilities. A radio contact is a QSO. Logbook of The World
is abbreviated LoTW; Parks on the Air is POTA; OpenID Connect is OIDC; Amateur
Data Interchange Format is ADIF, with its text representation called ADI.

## Delivery sequence

| Phase | Milestone outcome | Main dependency |
| --- | --- | --- |
| 1. Foundations and early risks | Native model and synchronization behavior are established; certificate and geographic delivery risks have concrete evidence. | Accepted product and architectural requirements. |
| 2. Durable offline logging | A Windows operator can record, organize and reopen contacts without the server. | Phase 1 native model. |
| 3. Accounts and synchronized operation | Independently writable clients reconcile through an isolated, managed account. | Phase 2 durable state and Phase 1 synchronization model. |
| 4. Complete logging workflows | Capture, lookup, geographic assistance and ADI interchange work together during normal and disconnected operation. | Phase 2 logging and Phase 3 identity/credential capabilities. |
| 5. External services and recovery | LoTW and QRZ publication, confirmations and recovery work safely with accepted account state. | Phase 3 synchronization, Phase 4 interchange and early compatibility evidence. |
| 6. Release readiness | Installed applications, deployment and recovery procedures demonstrate the complete MVP. | Integrated outcomes from Phases 1–5. |

The sequence expresses cumulative completion, not a requirement to finish every
activity in one phase before investigating another. In particular, certificate
compatibility and geographic-data preparation begin early, while offline logging
and synchronization are developed. Integration must respect their dependencies;
parallel preparation does not mean independent applications or deployed services.

## Phase 1 — Foundations and early risks

Milestones:

- Establish a native model covering contact identity, timestamps, modes,
  station snapshots, reusable equipment, profiles, tags and unified activity
  context with correct contact/own-station/counterpart scope. Recognition and
  source separation preserve recorded meaning across clients and upgrades.
- Demonstrate the agreed synchronization semantics with an executable model:
  whole-record comparison, deletion, offline edits, timeline recovery, authorized
  client retention and actor retirement. Explain conservative reconciliation when
  ordering evidence is unavailable.
- Establish early compatibility evidence for source-guided pure-Rust LoTW certificate handling
  and signing, and produce representative geographic data with verified provenance
  and redistribution rights. Surface incompatibilities before publication or
  location workflows depend on them.

Completion means the models cover the accepted behavior and meaningful failure
cases, with evidence for the principal external compatibility risks. A small
placeholder catalog, a collection of type declarations or tests of only happy
paths does not establish this milestone.

## Phase 2 — Durable offline logging

Milestones:

- Deliver Windows manual contact entry with the agreed timestamp behavior,
  optional operating profiles, equipment selection and historical snapshots.
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
- Synchronize all account contacts, tags and shared configuration to every
  enrolled client. Provide catch-up and streaming, client-side conflict review,
  missing-reference resolution and preservation of pending work while unrelated
  synchronization proceeds.
- Establish web account/client management, account credential protection,
  protected client provisioning of directory credentials, and production server
  tracing, logs and metrics. The web architecture accommodates its agreed future
  scope beyond administration; external-service setup and diagnostics finish
  with Phase 5.

Completion means multiple clients can edit independently, reconnect and reconcile
without losing work or exposing one account's state to another. Recovery from an
older server state, indefinite authorized offline return and safe retention must
be demonstrated for the core account state; Phase 5 extends that evidence to
external-service effects and history.

## Phase 4 — Complete logging workflows

Milestones:

- Integrate WS Suite capture and live other-station callsign updates, source
  ownership, completion deduplication and the agreed Generate Messages action.
  WS Suite refers to the WSJT-X family of weak-signal radio applications.
- Deliver direct client lookup through the account-selected QRZ or HamQTH
  provider, separate synchronized directory snapshots, late results and
  user-triggered selected/filtered bulk lookup. Logging continues when lookup or
  the synchronization server is unavailable.
- Deliver bundled offline geographic assistance for required United States
  coverage and the agreed minimum worldwide coverage, retaining point-versus-area
  meaning, ambiguity handling and manual correction.
- Deliver prepared ADI import with account-wide matching, explicit choices,
  partial results and diagnostics; provide consistent local-state export with
  ordinary and optional POTA-compatible output and destination protection.

Completion means these workflows operate against the same contact and source
model. Imported statuses and associations are retained for Phase 5 service use;
imports, capture and lookup do not silently overwrite recorded exchange facts or
create organizational copies of contacts.

## Phase 5 — External services and recovery

Milestones:

- Deliver LoTW certificate import, configuration, explicit supersession and
  credential removal, together with routing for multiple LoTW certificates and
  QRZ logbooks. Preserve public metadata and submission history.
- Deliver server-side publication using current accepted effective contact values,
  tag policies, trusted imported statuses, diagnostic retries and affected-only
  pauses. Unresolved client proposals do not become publishable state.
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
[scope index](open-questions.md#deferred-and-uncommitted-scope-boundaries).
No calendar estimate is asserted. Milestone completion is based on observed
capability and evidence; no discarded implementation is counted toward progress.

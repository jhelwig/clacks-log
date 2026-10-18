# Clacks Log Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the agreed minimum viable product (MVP): a durable offline Windows amateur-radio logger with account-owned tagged contacts, self-hostable synchronization and server-owned Logbook of The World (LoTW)/QRZ integration.

**Architecture:** Rust domain libraries serve an egui desktop with embedded SQLite and one Axum/Leptos server application with PostgreSQL. The server contains web management, synchronization and background work; tags are views over account-owned contacts and all synchronized data has one scope per account. The existing vector/timeline model, record-level conflict policy, separate provider evidence and pure-Rust signing direction remain fixed.

**Tech Stack:** Rust, ordinary Serde serialization, egui/eframe, SQLite, PostgreSQL, Axum, Leptos, encrypted WebSockets and established pure-Rust cryptographic libraries. Exact compatible versions/features and routine supporting libraries are selected and verified by the tasks that own them.

**Spec:** [Product](../../product.md), [decisions through D251](../../decisions.md), [synchronization](../../synchronization.md), [accounts](../../accounts.md), [integrations](../../integrations.md), [LoTW](../../lotw.md), [observability](../../observability.md), [backup/recovery](../../backup-recovery.md), [architectural review](../../architecture-review.md), and [remaining engineering work](../../open-questions.md).

Prepared 17 October 2026 against commit `91078a5`, which contains the agreed documentation and resolved review findings AR1–AR4. There is no application implementation yet. **Only preparation of this plan is authorized.** Checkboxes are prospective; no application build, test, dependency selection, deployment or service interoperability described below has been executed.

A radio contact is a **QSO**; Logbook of The World is **LoTW**; Amateur Data Interchange Format is **ADIF**, whose text representation is **ADI** and whose XML representation is **ADX**. Parks on the Air is **POTA**. OpenID Connect is **OIDC**. Other specialist terms are defined in the subsystem plans that use them. ID needs no expansion.

## Global Constraints

- Workspace members use `lib/<crate-name>` for libraries and `bin/<application-name>` for thin wrappers. Keep behavior, startup/composition and tests in libraries.
- The account owns a QSO; its complete tag set belongs to the logical QSO record. No independently synchronized membership records or automatic tag-set merging (D243).
- Tag deletion offers leave-all/delete-all/delete-unique QSO choices. A deleted QSO is distinct from an active untagged QSO; explicit restoration is post-MVP (D244/D55).
- Add/remove tags for individual QSOs and selected groups in MVP. Pending deleted-tag references require explicit removal/replacement; unrelated synchronization continues (D249/D250).
- One synchronization scope per account. Synchronize all account contacts, tags, required service evidence and shared configuration to every enrolled client (D248/D161).
- Whole-record equal or one-sided changes avoid resolution; differing two-sided changes require client-side field choices, including disjoint/tag edits. Unresolved work stays local (D136–D138/D243).
- Authorized clients may remain offline indefinitely. Retain ordering evidence through the oldest authorized durable acknowledgment; ordinary restarts do not create new actor/timeline identities (D60/D151–D156).
- A rollback is repaired from surviving state with fuller-state/conservative fallback when needed. Operator-controlled upload pause/resume governs restore; no promises about facts lost from every copy (D141–D148).
- At most one local QSO associates with an external record within an account/service destination. Ambiguity selects at most one; competing publication requires resolution (D251).
- Per-tag/per-service automatic-upload states are Enable, Forbid or Neither, default Neither. Any Forbid wins; otherwise at least one Enable is required. Untagged/all-Neither has no grant (D245/D246).
- Only current accepted server effective values are publishable. Retain actual dispatched values/results; retry unless explicitly successful and pause only affected non-transient failures until remedy/resume (D46–D49).
- Recorded facts and provider information remain separate. Fallback is recorded QSO → eligible LoTW → eligible QRZ → eligible directory; location is selected as a whole (D237/D238).
- Directory lookup runs directly on clients using the account-selected QRZ or HamQTH provider, with account-configured credentials provisioned securely. No provider merging (D96/D97/D196/D197).
- One Clacks server application contains web, sync and workers alongside PostgreSQL. No independently deployed services directly sharing its database (D213/D223).
- Desktop storage is embedded SQLite with ordinary transactions; no custom write queue or external client database service (D213).
- Windows desktop and Linux-container server are MVP; architecture/tooling stay suitable for later macOS/Linux desktop releases. No tray mode or native Windows server requirement (D82/D162/D189).
- LoTW is a source-guided Rust port using established pure-Rust cryptography. Runtime OpenSSL/direct tqsllib foreign function interface (FFI) fallback is excluded (D13/D224).
- Web uses Leptos/Axum and must accommodate later browsing/editing/reporting/logging; MVP remains the agreed management scope. No TypeScript application layer (D222/D223).
- Use independent per-account encryption keys wrapped by deployment key material outside PostgreSQL/image; client directory credentials use protected local storage (D236).
- Application code uses Elastic License 2.0; preserve upstream/data notices. Contributor-agreement work remains lawyer-led. Hosted-subscription launch is not an MVP gate (D221/D234/D241).
- Provide full server tracing, structured logs and metrics, with no credentials/private keys in telemetry (D230).
- Provide documented consistent native backup/recovery; ADIF is not a native backup. Built-in backup/restore and the explicit client restore implementation remain post-MVP (D57/D185/D240).
- Before implementing desktop/web layouts, present concrete workflow mockups for review. This is a UI checkpoint, not a reopening of product/architecture choices.

## Review Focus

1. **Rollback plus reused counters:** S1/S4 and R1 must distinguish older/divergent histories, preserve pending originals and explain uncertainty rather than silently overwrite.
2. **Import/export interruption:** I1–I3 must stop at reliable parse boundaries, revalidate stale choices, retain completed imports and protect destination files.
3. **Delayed/duplicate capture and lookup:** D3/G3 must correlate type-5/type-12 completion and late lookup results to the original contact, including authoritative empty calls.
4. **Cross-account and credential escape:** A1–A4/R1 must deny guessed IDs, revoked streams and secret distribution outside its permitted account/client boundary.
5. **Concurrent publication identity collisions:** E1/E2/R1 must enforce one external association before provider writes, without losing truthful in-flight outcomes or blocking unrelated work.

Each case has named tests in its owning tasks below. These are requirements for review and testing, not claims that five abstract tests prove the complete system.

---

## Deliverables and plan navigation

The project is too broad for a single undifferentiated task list. These coordinated subsystem plans cover the full MVP; none is a proposal to split the deployed server into services.

| Plan | Tasks | Independently reviewable deliverable |
| --- | --- | --- |
| [01 Domain and persistence](2026-10-17-clacks-log-01-foundation.md) | F1–F3 | Native records, migrations, durable offline contact/tag/profile operations |
| [02 Synchronization and recovery](2026-10-17-clacks-log-02-synchronization.md) | S1–S4 | Executable model, conditional acceptance, durable WebSocket sync and recovery |
| [03 Accounts and web](2026-10-17-clacks-log-03-accounts-web.md) | A1–A4 | Isolated identity/enrollment, secrets, management and observability |
| [04 Desktop and capture](2026-10-17-clacks-log-04-desktop-capture.md) | D1–D4 | Windows manual logging, tagged views, review and WS Suite capture |
| [05 Interchange](2026-10-17-clacks-log-05-interchange.md) | I1–I3 | Prepared partial ADI imports and safe ordinary/POTA exports |
| [06 Directory and geography](2026-10-17-clacks-log-06-directory-geography.md) | G1–G3 | Client lookup/snapshots and a validated bundled geographic artifact |
| [07 External services](2026-10-17-clacks-log-07-external-services.md) | E1/E2/L1/L2/E3 | Evidence/jobs, pure-Rust LoTW and both provider integrations |
| [08 Release](2026-10-17-clacks-log-08-release.md) | R1–R3 | End-to-end validation, installed artifacts and verified operating procedures |

The 29 tasks are review units, not effort estimates. A task may contain several implementation commits if that keeps changes reviewable. Maintain the listed interfaces between tasks; contract changes require updating affected tests and downstream plans in the same change, not an unrecorded divergence.

## Planned workspace and file ownership

Workspace layout follows the user’s required convention: libraries live in `lib/<crate-name>` and applications in `bin/<application-name>`. Application crates are thin entry-point wrappers around libraries. Create members only when their first task needs them; do not add a catch-all `crates/` directory.

The application packages have distinct names from their libraries, so Cargo workspace package identities remain unambiguous. The library/package decomposition below is an engineering layout within the agreed architecture, not additional deployed services.

### Libraries

| Unit | Owner | Responsibility |
| --- | --- | --- |
| `lib/clacks-domain` | F1; later tasks add focused modules | Native values, identities, validation, source-aware effective values and logical record equality |
| `lib/clacks-replication` | S1/S4 | Pure causal/reconciliation model, protocol contracts, ancestry and retention rules |
| `lib/clacks-client` | F2/F3, S2/S3, D3, I2/I3, G3 | SQLite, durable working state, commands, client sync, capture/import/lookup coordination |
| `lib/clacks-server` | F2, A1–A4, S2–S4, E1–E3 | Server composition, startup/runtime, configuration, PostgreSQL, web/auth/sync routes and background work |
| `lib/clacks-web` | A4 | Leptos server-rendered/browser-hydrated components and internal application routes |
| `lib/clacks-desktop` | D1–D4 | Desktop composition, startup, egui workflows and platform adapters |
| `lib/clacks-adif` | I1/I3 | ADI decoding, mapping, diagnostics, serialization and POTA output transform |
| `lib/clacks-directory` | G1/G3 | Provider adapters, normalized request keys and snapshot arbitration |
| `lib/clacks-geography` | G2 | Read-only versioned bundle and point/grid-area queries |
| `lib/clacks-lotw` | L1/L2 | Pure-Rust archive/certificate rules, source-guided normalization and signing |
| `lib/clacks-geography-build` | G2 | Data-build/validation behavior and command-line handling, testable without spawning the executable |
| `lib/clacks-system-tests` | R1 | Disposable real-server/storage/client scenarios with simulated external services |

### Applications

| Unit | Owner | Responsibility |
| --- | --- | --- |
| `bin/clacks-log-server` | A4 | Thin `clacks-log-server` executable calling the server library entry point |
| `bin/clacks-log` | D1 | Thin `clacks-log` desktop executable calling the desktop library entry point |
| `bin/clacks-log-geography` | G2 | Thin developer-tool executable calling the geography-build library entry point |

### Supporting files

| Unit | Owner | Responsibility |
| --- | --- | --- |
| `data/geography` | G2 | Source manifest, mapping, artifact metadata and redistribution notices; no Cargo package |
| `docs/engineering`, `docs/design`, `docs/verification`, `docs/operations` | Producing task | Concrete contracts, reviewed mockups, observed evidence and tested runbooks |

Use Cargo workspace members under `lib/*` and `bin/*`. Domain behavior, application composition, configuration/argument parsing, runtime setup, telemetry setup and substantive error handling belong in libraries. Each binary contains its manifest and a minimal `src/main.rs`; its only responsibility is to delegate to a library entry point, returning the resulting exit status. Do not give binaries parallel implementations, integration-test suites or access to the database/provider internals. WebAssembly browser hydration remains a library target in `lib/clacks-web`.

Planned entry points are `clacks_server::main_entry() -> std::process::ExitCode`, `clacks_desktop::main_entry() -> std::process::ExitCode` and `clacks_geography_build::main_entry() -> std::process::ExitCode`. Keep testable composition/configuration functions behind these library entry points; tests must not have to invoke a process-exiting main function. Library integration tests stay beside the owning library, and the system-test harness is the development-only `lib/clacks-system-tests` package. Shared fixture data remains under `tests/fixtures`.

Build executable packages by name: `cargo build -p clacks-log`, `cargo build -p clacks-log-server` and `cargo build -p clacks-log-geography`. Test the owning libraries by their existing `clacks-*` package names. This distinction also applies to release packaging, container entry points and continuous-integration build checks.

These are library/code boundaries, not deployment boundaries. Keep dependency direction toward the domain: pure domain/replication code does not import desktop, Axum, SQL clients or secret-store implementations. Do not introduce a dependency-injection framework, generic repository hierarchy, event-sourced application or background writer service merely to fit the table. Share ordinary functions/data types where that suffices.

Native contract F1 defines identifiers, records, raw preserved values and validation. S1 defines accepted-state/causal/checkpoint types. F2 persists those contracts; A1 creates authenticated account/client contexts. E1 defines service identity, payload and evidence types. Downstream plans consume those symbols rather than selecting their own incompatible representations.

## Execution sequence and dependency gates

A milestone is achieved by observed deliverables, not by completing documents or scaffolding alone.

### Gate 0 — Testable contracts and early delivery risks

- [ ] F1: compile native values and workspace checks; write the domain field/transform contract.
- [ ] S1: executable replication model and protocol contract pass rollback, equality and timeline counterexamples.
- [ ] Start L1 certificate compatibility and G2 source/bundle validation as soon as F1 is available. Their independent fixtures can expose required-format/data problems before the rest of the application depends on them.

Do not let a polished desktop hide a failed replication or signing model. Conversely, do not hold independent archive/data validation until a complete server exists. This is dependency scheduling, not authorization to spawn agents or start execution now.

### Gate 1 — Durable offline engine and first logging slice

- [ ] F2/F3: real SQLite/PostgreSQL transaction and restart tests; local tags, contacts, profiles/equipment and operating-context snapshots.
- [ ] I1: preparation/mapping diagnostics and shared candidate classification.
- [ ] G1: source eligibility and snapshot rules, ready for provider identity/evidence contracts.
- [ ] D1: reviewed entry workflow supports saving a contact offline and reopening durable state.

This slice demonstrates offline saving, not the complete MVP desktop. History, provider lookup, imports and conflict UI finish at their later gates. No real provider writes are needed.

### Gate 2 — Isolated multi-client synchronization

- [ ] A1/A2: identity bindings and browser enrollment; independent client revocation.
- [ ] E1: service identity/evidence contracts consume A1’s authenticated context before S2 enforces external-association constraints. This is contract/transaction work, not provider dispatch.
- [ ] S2/S3: conditional acceptance, account-wide pending state, durable catch-up and streaming.
- [ ] D2: tagged history, Worked Before and field-level/missing-reference review.
- [ ] A3: protected account keys, credential storage and directory provisioning before any real secret-dependent work.

Use two clients as a scenario, not a system cap. Also test multiple independent accounts and an offline third client that pins collection. Keep unresolved proposals local while unrelated records progress.

### Gate 3 — Complete local operating workflows

- [ ] Integrate the Gate 2 E1 contracts into import/status workflows, including trusted imported positives and external association uniqueness.
- [ ] I2/I3: full prepared partial imports and consistent ordinary/POTA exports.
- [ ] G1/G2/G3: snapshot/fallback rules, bundled offline geography, both direct directory providers and user-triggered bulk lookup.
- [ ] D3: WS Suite status/capture, paired completion deduplication and Generate Messages path.
- [ ] A4: account/credential management and production telemetry; finish provider panels with E3.

Capture stays useful during sync/provider outages. Neither lookup nor remote publication delays the durable contact save. Directory/provider fields remain separate from recorded exchange data.

### Gate 4 — Publication and recovery complete

- [ ] E2: durable current-value attempts, retries, pauses, identity collision prevention and diagnostics.
- [ ] L1/L2: supported archive/certificate compatibility and independently checked signing/serialization.
- [ ] E3: both submission/query adapters, durable download windows/dispositions and confirmation applicability.
- [ ] S4: compaction, retirement and older-server recovery including actual service evidence.
- [ ] D4/A4: desktop/web workflows integrated with the completed services.

No cycle is implied: G1’s source rules precede E1; E1’s identity/evidence contract precedes S2/I2/S4; E2 jobs can run against fake adapters before L2/E3. A4 uses E1 DTOs first and receives real provider wiring in E3. D3 consumes I1, not the whole import UI. G3 consumes D1 entry identity and later accepts D3 events, so D3/G3 do not depend on each other to start.

### Gate 5 — Verified release candidate

- [ ] R1: real-store/server system tests, deterministic failure schedules, account isolation and complete accepted workflows.
- [ ] R2: installed Windows application, Linux server image, geographic/notices audit and controlled live QRZ/LoTW interoperability evidence.
- [ ] R3: execute deployment, native backup, restore, upgrade and administrator-assisted account-recovery procedures from the actual artifacts.

A simulated success does not satisfy the live provider gate. Live calls use controlled owner-authorized records and credentials; nothing in plan preparation sends contacts to a provider. Release or deployment to a user’s production environment remains a separate action from implementing these tasks.

## Verification strategy

Use pure unit/property tests for native conversions, matching, source selection and the replication model. Use real SQLite/PostgreSQL integration tests for commit, uniqueness, migration, locking and crash/reopen behavior. Use provider/identity simulators for reproducible error, duplicate, cursor and timeout scenarios, followed by controlled real-service interoperability checks. Use Windows installed-artifact/manual interaction evidence for desktop input, accessibility, protected storage and packaging claims.

The focused commands in each plan become runnable when their targets exist. F1 selects the compatible toolchain and library versions; task manifests add their targets to the workspace. Database-dependent tests require the documented disposable PostgreSQL setup and must not silently skip when it is missing. Browser hydration and native/server builds need their distinct target/feature checks; do not treat a default workspace build as coverage of all feature combinations.

At task completion, run its focused tests and necessary affected integration checks; at integration/release gates, run formatting, linting, full workspace tests and actual target builds. Add tests for meaningful behavior, failure boundaries and invariants. Do not add tests solely to mirror a reversible documentation/configuration edit. Commit bounded verified changes; do not repeatedly rerun unrelated full suites without a new reason.

Maintain test evidence in `docs/verification/`: command/environment, target/version, fixture/source provenance, observed result and limitations. Never put real private keys, tokens, personal log exports or customer data in test fixtures. Test-only generated private keys must be clearly identified as public disposable fixtures, not usable credentials.

## Product coverage map

| Agreed capability | Owning tasks / evidence |
| --- | --- |
| Native QSO fields, optional reports, mode preservation, band/frequency and location transforms | F1, I1, D1; domain/mapping fixtures |
| Account-owned contacts, overlapping tags, untagged access and three deletion choices | F3, S2, D2, R1; identity/deletion/race scenarios |
| Local defaults/operator, optional profiles, reusable equipment, explicit reload and historical snapshots | F1/F3, D1/D2; profile/operating fixtures |
| Symmetric offline sync, record-level conflict trigger, field-level choices and pending review | S1–S3, D2, R1 |
| Timeline recovery, authorized-client retention, durable acknowledgment, revocation and reauthorization | S1–S4, A2, R1/R3 |
| Multiple isolated accounts and OIDC identities, self-service link/unlink, browser callback and administrator recovery | A1/A2/A4, R1/R3 |
| Per-account envelope encryption, protected directory provisioning, server-only signing secrets | A3, L1, R1/R3 |
| Manual timestamps, save/reset behavior, Windows graphical logging and current-tag history | D1/D2/D4, R2 |
| WS Suite authoritative calls, source ownership, completion logging and GridTracker Generate Messages | D3/G3, D4/R2 |
| QRZ/HamQTH direct lookup, 500 ms debounce, late results and selected/filtered bulk lookup | G1/G3, D3/D4 |
| Offline U.S. state/county and worldwide DXCC/CQ/ITU data, point/grid ambiguity and manual corrections | F1, G2, D1, R2 |
| Read-only import preparation, built-in mappings, partial application, account-wide matches and Update/Duplicate/Skip | I1/I2, E1, D4 |
| Consistent local ordinary/POTA exports, transliteration and destination-file protection | I3, D4, R1 |
| Tag upload policy, multiple QRZ/certificate routing, trusted imported status and one QSO per external record | E1/E2/E3, L1, I2 |
| Pure-Rust LoTW import, explicit supersession, removal, signing and provider acceptance | L1/L2, E3, A3/A4, R2 |
| Current-value jobs, diagnostics, retries, scoped pauses, duplicate-safe downloads and provenance fallback | E1–E3, G1, S4, R1 |
| MVP web administration with a broader future web architecture | A4, E3, R2 |
| Full server observability and account-isolated user diagnostics | A4, E2/E3, S3, R1/R3 |
| Linux containers, Windows packaging, licensing/notices and documented backup/recovery/upgrades | F1, A3, G2, L1/L2, R2/R3 |

This table maps behavior, not every historical decision as a separate task. The decision register retains supersession; implement current contracts rather than resurrecting D102 child ownership, D106 move deferral, D158–D160 per-book scopes or D111 multiple external associations.

## Deferred or excluded work

Do not add these to the MVP critical path: transceiver control; paper-QSL workflow; dedicated contest exchanges; unrestricted bulk editing of contact facts; selective sync; ADX; macOS/Linux desktop releases; a separate geographic updater; built-in backup/restore; certificate requests/renewals or private-key export; outbound WS control; device-authorization fallback; automatic directory-provider fallback; account-user security-event history; hosted billing/subscription launch; general-purpose rule-driven tag creation/assignment; deleted-contact restoration or equipment restore/clone actions.

Source merging, restart-persistent unfinished manual entries and dedicated import-session resumption are not future requirements. Persistent reusable directory-result caching, award/heard-station features, eQSL integration and archive features remain uncommitted. Lawyer-led contributor terms are not an engineering drafting task. Direct tag assignment is already MVP; the former inter-book move deferral must not be used to exclude it.

## Risk and completion discipline

- **Replication correctness:** S1 establishes distinctions; S2/S3/S4 and R1 must demonstrate them through actual storage/protocol failure schedules. A conceptual paper comparison is not implementation verification.
- **LoTW compatibility/security:** L1/L2 must establish supported archive algorithms, certificate rules and signing-path assessment before R2 live acceptance. Pure-Rust preference is fixed; a hard compatibility failure gets a concrete counterexample, not an OpenSSL substitution.
- **Provider identity:** E1/E2 must prevent conflicting writes before a returned opaque record ID exists. Unknown identity semantics block that affected dispatch and require evidence, not optimistic fan-out.
- **Geographic delivery:** G2 produces the actual mapping/artifact and validates redistribution/topology; citing the comparison is insufficient.
- **Operational recovery:** R3 tests protected key recovery and upload-paused startup against restored state, including service evidence and account isolation.

No calendar estimate is asserted. Re-estimate from F1/S1/L1/G2 results and observed implementation throughput; do not reuse the withdrawn LoTW estimate. Routine choices such as crate versions, transaction batches, target build features and schema/index design are owned by implementation tasks. Escalate only a demonstrated behavioral tradeoff or contradiction with accepted requirements, with a concrete counterexample.

## Plan status and execution handoff

- [x] Read integrated product, architecture, handoff, research and review findings through D251.
- [x] Map MVP behavior to owning tasks and exclude superseded/deferred scope.
- [x] Define planned file ownership, inter-task contracts, dependency order and release gates.
- [x] Self-review type ownership, dependency cycles, failure-case coverage and plan links.
- [ ] User reviews this implementation plan and selects an execution approach.
- [ ] Implementation is explicitly authorized and begins with F1/S1 contracts, plus the early L1/G2 risk work when dependencies permit.

Recommended execution is native, in reviewed milestone slices: implement tasks in dependency order, use the executable model for the highest-risk contracts, and obtain an independent review at integration gates. The tasks have tightly shared record/evidence/protocol interfaces, so milestone integration matters more than broad simultaneous file editing. Subagent-driven execution is an available alternative when explicitly selected; the choice does not change scope or validation gates.

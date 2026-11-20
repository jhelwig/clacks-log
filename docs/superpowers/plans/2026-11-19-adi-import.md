# Prepared ADI import implementation plan

**Goal** — Deliver the desktop workflow for [#24](https://github.com/jhelwig/clacks-log/issues/24), from file selection and read-only preparation through reviewed local application and ordinary synchronization.

**Architecture** — The desktop adapts qso-io records to existing native contact values, prepares assignments from one account working snapshot, and holds a shared pause guard for that import. The local store saves each reviewed row and its source evidence in one transaction. The resulting records use the existing per-record synchronization path.

**Tech stack** — Rust 1.99.1, qso-io 0.2.0, egui and the shared desktop menu, Tokio, and SQLx with SQLite.

**Spec** — [Prepared ADI import and account-wide matching](../specs/2026-11-19-adi-import-design.md).

**Execution** — Review the plan before executing it. Use task-by-task execution with the agreed agent workflow, project test policy, and commit-message rules. Work on the ADI import branch and keep each logical change reviewable. Read the project [desktop development contract](../../desktop-ui.md) and machine-local development instructions before launching a development build.

## Global constraints

- Use the normal desktop import entry point in the delivered workflow. Connect preparation, review and application through that entry point.
- Use the current desktop workspace, account catalog, and chosen named tag. Use disposable databases and isolated temporary desktop workspaces for development and test runs.
- Reuse the existing native representations, comparisons, persistence schemas and synchronization protocol. Keep import-specific work in the desktop adapter and the guarded local save. Generate checked SQLite query metadata for new fixed queries.
- Apply file interpretation and user choices before any save. Stop on a structural error, preserve the readable prefix, and keep the reviewed input immutable during application.
- Preserve supplied source coverage for applicability, separate upload from confirmation, and keep claims distinct from dispatches and external associations.
- Report `QTH` and `MY_CITY` as unsupported. Preserve service-claim provenance and exact whole-second UTC timestamps. Disclose calendar-only dates and nonzero fractions in preparation while retaining the recognized claim. Extensions to the native model require explicit scope and approval.

## Review focus

- The reference export contains non-ASCII text with character-counted lengths. Valid names must survive decoding and unsupported locality fields must remain visible in diagnostics, while a damaged descriptor must leave the remainder unapplied.
- Source contradictions can change candidate identity. Choosing either value or omission must update that row's eligible assignments before Apply.
- A row and a saved QSO each have degree at most one in the final assignment, even when candidate sets overlap or a source row would have been an unchanged reuse.
- A reviewed match can keep a different recorded time or station identity. The selected target cannot fill omissions in the imported baseline or broaden a claim to the saved values.
- A stale local basis, crash or cancellation must preserve the local row transaction boundary. Multiple held guards must keep the sync exchange dormant until the final release.

## 1. Prepare ADI input and resolve source-field overlaps

**Create** `lib/clacks-desktop/src/import.rs`, `import/adi.rs`, `import/mapping.rs`, and `import/evidence.rs`. **Modify** the desktop and workspace manifests and lockfile to use the published qso-io release; wire the owning module in the desktop crate.

**Produces** `PreparedSource`, containing framed `PreparedRow` values, source spans, unsupported-field counts, warnings, and an optional terminal structural diagnostic. `PreparedRow` carries native mapped values, presence/clear intent, source timestamp precision, unresolved typed alternatives, and source evidence; it may be incomplete before review. The pure adapter reads supplied bytes and definitions and checks for cancellation between framed sections. The session job owns file I/O and cancellation.

- [ ] Add small, independently authored ADI inputs testing Clacks behavior. Use one file with an omitted field, an empty field, and a well-framed invalid optional field. Use another with two complete rows followed by a malformed field. Assert the resulting edit intents and eligible prefix. Include the exact LOG4OM header signature with a multibyte name and an unsupported locality field to pin interpretation.
- [ ] Frame with the permissive qso-io parser. Select ASCII/byte framing for ordinary ADI. Use UTF-8/Unicode-scalar lengths for the verified LOG4OM 2.42.1.0 source signature, identified from a framed header before parsing records. The reference file decodes all 374 contacts in that mode. Do not retry alternate framing modes to recover a damaged suffix. Surface decode errors with source location and retain independently usable fields. Extend the known-source interpretation only with evidence from additional exporters/versions.
- [ ] Map the spec tables using native checked parsers, band/reference catalogs, field-specific normalizations, and existing decimal helpers where they preserve semantics. Establish explicit import bindings for the supported station-side activity fields. Reuse seeded POTA, SOTA, and IOTA group identities and resolve contest codes through eligible literal account group bindings as in WS Suite capture. A group without an unambiguous supported mapping is reported. An output expression alone supplies no general inverse import mapping.
- [ ] Implement typed `Use(value)` and `Omit` choices for unresolved alternatives. Show the source name, span and value for each alternative. The chosen value becomes the sole input for that fact. Omit supplies no edit or claim for that fact. For an otherwise established claim with a disputed date, omit drops only the date. Use an exact UTC timestamp when agreeing fields establish the same event at whole-second precision. Disclose calendar-only dates and timestamps with nonzero fractions if the source supplies them. Offer separate source alternatives when first confirmation and receipt dates have no established precedence. Keep upload, confirmation, and download meanings distinct.
- [ ] Verify one test with conflicting recognized QRZ upload statuses and one with incompatible location qualifiers for a single park; test both selections and omission. Verify the adapter retains QRZ claim provenance, stores a supported exact UTC timestamp, and reports unrepresentable dates while retaining the claim. Run `cargo test -p clacks-desktop --locked import::`.

## 2. Match saved contacts and reuse field review

**Create** `lib/clacks-client-store/src/read/import.rs`, `lib/clacks-desktop/src/import/matching.rs`, and `import/workflow.rs`. **Modify** existing `review/fields/qso.rs`, `review/fields/station.rs`, and `ui/review/` comparison presentation only where the import needs the shared controls.

**Produces** `ReadSnapshot::import_identities` for matching fields from saved working contacts. Load eligible full records, settings, and definitions in the same SQLite snapshot. The workflow builds a `PreparedImport` from that snapshot and the mapped source. `PreparedImport` owns the candidate graph, guarded saved bases, and per-row review decisions. Convert an eligible resolved row to a guarded contact plus evidence through task 3. No database transaction spans review.

- [ ] Query live working identities by complete worked callsign and inclusive UTC time window, including alternatives from unresolved source disputes. Merge overlapping windows for each callsign and apply the remaining identity predicates per row. Include pending local working values and untagged contacts. Load each eligible `ClientRecord` once for review and guarded save.
- [ ] Compare complete compatible identities at the lowest supplied precision using full UTC dates. Retain the reason for explicit selection when the match uses incomplete identity or a nonzero difference within 30 minutes. Assert exact boundaries with minute-only versus second-precision times in both argument orders, including midnight, and cover broad mode, unfamiliar submode parent missing, and differing explicit parents.
- [ ] Keep candidate edges separate from assignments. Enforce one target per row and one row per target when selecting and again before applying. For A → X/Y and B → X, assert A → Y and B → X can coexist. A target contested by two rows requires explicit review even for an otherwise unchanged reuse. Verify that a choice among conflicting identity inputs refreshes the affected candidates, and that source rows and newly created QSOs do not enter the candidate set.
- [ ] Build the incoming field comparison from native values and presence intent. Use the shared comparison UI and native field operations. Adapt the source labels and choices without fabricating accepted remote revisions or a persisted sync conflict. Keep applying an update limited to selected changes. Require a valid historical callsign assignment before creating a separate QSO missing our identity.
- [ ] Verify the candidate snapshot with the real store and exercise the review state via `cargo test -p clacks-client-store --locked` and `cargo test -p clacks-desktop --locked import::`.

## 3. Save a complete imported contact

**Create** a guarded import operation under `lib/clacks-client-store/src/save/`. Reuse `save.rs::save_in` in the local save and `clacks-model/src/service/evidence.rs::imported_baseline` when the source supplies a complete contact.

**Produces** `apply_import_row(store: &ClientStore, row: ResolvedImportRow) -> anyhow::Result<ImportRowOutcome>`. The row carries the exact saved basis (or creation), the final validated native QSO including its tag, and reviewed source evidence. The outcome distinguishes an unchanged row, membership-only change, QSO change, added evidence, or stale save. A row can contribute to multiple factual report counts.

- [ ] Use the checked `save_in` version guard inside one immediate transaction. Save the QSO, membership, and new assertions together. Return stale before storing evidence. Treat an already represented assertion as unchanged using its normalized claim, source coverage and provenance. Keep stable evidence identities through prepared retries. Ordinary re-import recognizes equivalent committed effects without storing an import match history.
- [ ] Capture evidence from reviewed source values, separate from the resulting QSO. Construct sparse baselines using the existing optional fields. Never populate a missing or disputed source fact from the target or defaults. Evaluate the source claim against the resulting QSO through existing publication coverage and confirmation applicability. A merge that keeps a different service-relevant value must not inherit coverage from the imported value. Missing required evidence remains unknown. Preserve the historical claim, provenance, and representable timestamp. Reuse `ConfirmationBasis` optional identity where it already expresses the distinction.
- [ ] Send the usual local-change notification after a successful commit. Verify that the tagged QSO and imported assertions enter the existing per-record publication path.
- [ ] Exercise the real local save with a tagged QSO and assertion and re-import of the same committed source. Use the desktop cancellation scenario to verify that each saved contact has its own pending assertion and no assertion remains for an unprocessed contact. Include a source claim whose target preserved a different start or whose source lacked callsign evidence. Run `cargo test -p clacks-client-store --locked`.

## 4. Pause sync using a shared workspace guard

**Modify** `lib/clacks-desktop/src/session.rs`, `synchronization.rs`, and `synchronization/exchange.rs`. Test through existing desktop synchronization/session fixtures.

**Produces** a session-owned Tokio `RwLock<()>` shared by the import and sync worker. A pause operation acquires an owned read guard and retains it for the import lifetime. The sync exchange acquires the exclusive write guard before its active work.

- [ ] Pass the same coordination object through the active account session. Protect negotiation paths that can install review state and each active catch-up/publication cycle. Release before awaiting an idle socket/local change notification. The worker stays alive and normal notifications still drive its next exchange.
- [ ] Make waiting for a guard cancellation-aware on both sides. Drop the import guard on every normal terminal path and on session teardown. An active sync exchange finishes before import reads the candidates. Never hold a database transaction while waiting on the gate.
- [ ] Verify that shutdown releases a worker waiting for an import even while the hold remains active. Check active and idle exchange boundaries through the normal desktop sync flow.

## 5. Connect the desktop import workflow and verify it end to end

**Create** `lib/clacks-desktop/src/ui/import.rs` and the import session job. **Modify** `menus.rs`, `ui/menu_commands.rs`, and the desktop session/UI module registration. Reuse the shared native menus and Linux fallback. The codebase has no current file picker; use a native picker through [rfd's asynchronous file dialog](https://docs.rs/rfd/latest/rfd/struct.AsyncFileDialog.html), keeping the picker and its platform dependency in desktop. Verify the selected release and Linux packaging dependencies when locking it.

- [ ] Wire file and named-tag selection, preparation, diagnostics, row/field review, application, and final report to one session-owned import. Keep file I/O and preparation off the UI path. Capture file bytes before review. Acquire and hold the shared guard before loading candidates and throughout application.
- [ ] Reuse the mapped field choices and the guarded store call for each reviewed row. If cancellation arrives mid-save, let that row finish its transaction and stop before the next. Send existing local-change notifications after completed work. Display truthful committed counts, reported warnings and rejected or unprocessed rows.
- [ ] Validate a small end-to-end fixture through the desktop import controller. Include a duplicate target, a resolved source-field disagreement, a row with unsupported locality and usable contact fields, a matched contact whose only new effect is the selected tag, and a malformed suffix. Exercise cancel before Apply, cancel during Apply, and re-import after partial application. Test the reference LOG4OM export privately in a disposable workspace and record aggregate quantities only. The personal source file does not belong in repository fixtures.
- [ ] Refresh the owning contracts for the agreed behavior, including the sync pause lifecycle, lower-precision comparison, one-to-one assignment, unsupported locality reporting, and representable imported timestamps. Keep #25 export, #30/#31 provider scheduling, and follow-up fields as their owning work. Verify any provider publication integration already available against compatible accepted import evidence.
- [ ] Run the focused commands above, `cargo fmt --all --check`, the workspace Rust checks in `.github/workflows/rust.yml`, and supported-platform UI validation through isolated workspaces. Check native file selection/cancel, persistent report behavior, and sync resumption using the normal UI. Record any platform or provider verification limits.

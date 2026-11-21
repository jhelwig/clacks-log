# Desktop ADI export implementation plan

**Goal** — Deliver [#25](https://github.com/jhelwig/clacks-log/issues/25) through
**File → Export ADI…**, with ordinary interchange and optional Parks on the
Air (POTA) output, safe publication, and retained results.

**Architecture** — Capture the current saved scope in the client store. Reuse
its source selection and the model's reference planner, then map and validate
Amateur Data Interchange Format (ADIF) fields in a desktop export adapter.
The desktop owns choices, native dialogs and the job lifecycle. The file
publisher takes the reviewed data and never reopens the contact store.

**Tech stack** — Rust 1.99.1, SQLx and SQLite, Tokio, egui, rfd 0.18.0,
qso-io 0.3.0 and `tempfile`.

**Spec** — [Desktop ADI export MVP](../specs/2026-11-20-adi-export-design.md).

**Execution** — Review this plan before writing production code. Follow the
project's test and commit rules. Read the machine-local development inventory
before launching a desktop build. Retain separately reviewable changes when
there is an independent behavior to review. Deliver all tasks together through
the normal desktop entry point.

## Global constraints

- ADI is the text representation of ADIF. Use qso-io for serialization and
  preserve the existing import interpretation when upgrading the shared pin.
- Compose one read-only working-state snapshot. Keep recorded facts and source
  provenance, the native model and synchronization semantics intact. Use
  disposable databases and isolated desktop workspaces for verification.
- Reuse installed account definitions and output profiles. A POTA required-field
  skip is an explicit user choice. Mapping, configuration and I/O failures are
  reported as failures.
- POTA files go directly into the native folder picker's result. The picker
  offers folder creation. Protect existing files and report the actual results
  of the publication attempt.
- Keep paths, callsigns and contact payloads out of telemetry. Apply the
  [observability contract](../../observability.md) at preparation and publication.

## Review focus

- Selected contacts outside the current filter and matches on later pages must
  retain their distinct scopes. A new save or membership change after capture
  must leave the reviewed export intact (task 1).
- A recorded partial location must suppress lower-priority location fields.
  Stale directory keys and stale confirmations cannot supply exported facts
  (tasks 1 and 2).
- Multiple contest choices and optional text loss must never drop an ordinary
  QSO. A missing POTA transmit band must exclude only that QSO, with a visible
  reason before Continue (tasks 2 and 4).
- A POTA partition failure, filename collision, or cancelled staging pass must
  preserve earlier exports. A late publication failure must account for every
  file it published before the failure (task 3).
- A slow preparation or picker response must not write a previous scope or
  workspace. Results must remain accessible in the same window (task 4).

## 1. Capture the desktop scope and export facts

**Files** — Add `lib/clacks-client-store/src/read/export.rs` and register it in
`read.rs`. Extend `read/projection.rs` and its evidence modules where the
existing projection lacks an export input. Extend `tests/history_queries.rs`
or add an export scenario through `tests/integration.rs`.

**Interface** — `ReadSnapshot::export_input(&mut self, scope: &ExportScope)
-> anyhow::Result<ExportInput>`. `ExportScope` holds `Selected(BTreeSet<QsoId>)`,
`Matching { view: SelectedView, filter: Query }`, or `All`. `ExportInput` owns
unique working QSOs, eligible effective name and whole locations, effective
contacted references, supported applicable confirmation evidence, and the
account's `DefinitionSet` and `ProfileSet`. Consume it after closing the reader.

- [ ] Exercise the real store using the existing save/sync fixtures. Save a QSO
  visible under two tags, a selected QSO outside the applied expression, a
  pending edit with a distinct shared value, an unsynced save, and a pending
  deletion. Assert the exported identities and edited values for each scope.
  Make one scope cross a read batch boundary. Change a contact and its membership
  after capture and assert that prepared input retains the original facts.
- [ ] Reuse the history query compiler and working-contact loading as used by
  `read/query.rs::contact_page`. Match by explicit identities independently of
  filter for Selected, and use the established view/filter predicates for
  Matching. All uses the account's full active set. Use a stable identity
  order and bounded batches on the same transaction. Share selection logic if
  batch reads would otherwise repeat derived query preparation or counts.
- [ ] Reuse `read/projection.rs` and its scoped service evidence evaluation.
  Return the effective references from the chosen whole-location source plus
  recorded non-location participation using `effective.rs`. Carry only
  applicable confirmation claims with representable retained dates if available.
  The existing service booleans alone cannot supply dates. Load saved groups
  and output profiles from the same read. Share the existing eligibility
  evaluation, including normalized lookup keys and current confirmation
  applicability. Do not fetch providers or substitute the seed catalog for
  working definitions.
- [ ] In the export read scenario, persist a partial recorded location and a
  more complete eligible fallback. Assert no field mixing. Include a stale-key
  directory result and a confirmation superseded by a meaningful recorded
  change. Assert that neither supplies claims or location. Use existing checked
  query metadata procedures only if new fixed SQL is required.
- [ ] Run `cargo nextest run -p clacks-client-store --locked` and verify the
  export and existing contact-history scenarios pass.

## 2. Upgrade qso-io and prepare both ADI formats

**Files** — Update `Cargo.toml` and `Cargo.lock`. Add
`lib/clacks-desktop/src/export.rs`, `export/mapping.rs`, `export/prepare.rs`
and corresponding tests. Register `export` in the desktop crate root. Share
field formatting from
`lib/clacks-model/src/service/adif.rs` where the contract matches. If the
reference planner needs a typed readiness distinction, make the change in
`lib/clacks-model/src/export/{evaluate,select}.rs` with its existing tests.

**Interfaces** — `prepare(input: &ExportInput, format: ExportFormat,
resolutions: &BTreeMap<QsoId, ContextResolution>) -> Result<PreparedExport,
PrepareError>`. `PreparedExport` owns planned ADI records and logical partitions,
conflict decisions, POTA rejections by identity and missing field, and conversion
warnings. It supplies the requested, eligible and skipped QSO identities
independently of expanded entry counts. The job gives the writer only a
resolved and accepted plan.

- [ ] Pin `qso-io = "=0.3.0"` and update its locked package with
  `cargo update -p qso-io --precise 0.3.0`. Inspect the lock diff for collateral
  changes and run `cargo test -p clacks-desktop --locked import::` to verify
  existing prepared import behavior.
- [ ] Define the adapter's supported field mapping alongside the code. Use
  `CALL`, `STATION_CALLSIGN`, and recorded `OPERATOR` where present. Format
  start/end UTC dates and times, transmit and receive frequency/band, mode and
  submode, reports and power using native units. Map supported historical names,
  grid and available coordinates, subdivisions and other representable location
  fields from the selected station facts. Map applicable confirmations using
  their actual service fields and retained dates. Preserve the distinction
  between confirmation and publication. Reuse time, frequency, mode, decimal
  power and coordinate formatting from `service/adif.rs` where applicable.
  Keep optional field omissions visible and keep service-specific upload
  requirements out of ordinary export.
- [ ] Use the existing transliteration capability and qso-io's ADI field
  repertoire. Normalize multiline text to ADI CRLF and handle unsupported
  controls or empty conversion with an omission warning. Never transliterate a
  callsign or identifier. Keep a supported identity exact and surface invalid
  required values. Supply one record field per catalog tag, detecting conflicting
  adapter and reference bindings before writing. The qso-io writer validates
  structure and characters. The adapter owns typed ADIF values and receiver
  rules, including date bounds and validation of members of reference lists.
- [ ] Use `plan_reference_export` and its existing context resolutions for
  ordinary reference lists and per-QSO contest ambiguity. Carry the structured
  decisions to the dialog. Omit the ambiguous field only after the explicit
  choice, leaving the entry eligible.
- [ ] For POTA, select the installed POTA output profile and target group by
  stable identity. Validate required own park and applicable activation
  location and the required fields in the [POTA technical reference](https://docs.pota.app/docs/activator_reference/ADIF_for_POTA_reference.html).
  Use the native transmit band and applicable mode/submode. A frequency-only
  QSO without a supported band is a readiness rejection. Use a recorded
  qualifier and applicable own primary subdivision for location and `MY_STATE`.
  Resolve equivalent evidence in its own code system before comparing or
  assigning partitions. Keep distinct activation locations separate. Preserve a
  planned unknown subdivision only where allowed. Reuse the planner for
  partitioning and expansion. Assess all contacts in one preparation pass.
  Distinguish typed contact readiness from errors in account definitions or
  conflicting mappings. An optional contacted park may be absent.
- [ ] Add focused mapping cases using authored native contacts and fixed
  expected ADI fields. Assert an ordinary one-row multi-park output with lists,
  both contest decisions, exact portable callsign and UTC facts, named conversion
  warnings, and no optional text mutation. Include applicable confirmation/date
  and an omitted inapplicable confirmation. For POTA exercise one QSO with two
  own parks and three contacted parks, a QSO with no contacted park, and a QSO
  missing a transmit band. Assert the partition fields, unique rejection reason
  and expanded entry counts. Verify no contest expansion or identity changes.
  Run `cargo test -p clacks-desktop --locked export::` and, if model changes,
  `cargo nextest run -p clacks-model --locked`.

## 3. Stage and publish the reviewed outputs

**Files** — Add `lib/clacks-desktop/src/export/publish.rs` with focused tests.
Move `tempfile.workspace = true` to the desktop's production dependencies.

**Interfaces** — `file_names(plan: &PreparedExport, timestamp: UtcSecond)
-> Result<Vec<PlannedFileName>, PublishError>` and
`publish(plan: &PreparedExport, destination: &ExportDestination,
cancel: &CancellationToken) -> Result<ExportReport, PublishError>`. The
publication input carries the reviewed names and explicit ordinary replacement
intent where confirmed. Outcomes carry the paths and counts of files published before success or
failure.

- [ ] Generate safe single-component POTA names from park and location with a
  shared `YYYYMMDDTHHMMSSZ` timestamp. Use the timestamp from the preview for
  every file and when choosing another folder during the same attempt. Check
  that distinct partitions yield distinct names on case-insensitive filesystems
  and resolve a duplicate name deterministically before preview. Exclude path
  separators, platform-reserved characters and unsafe filename components.
- [ ] Stage `NamedTempFile` outputs in the chosen destination directory and
  pass the open handles to the qso-io writer. Finish and flush each file, then
  retain its `TempPath` and release the open handle. Check cancellation before
  final publication. Keep file operations off the UI thread. Use
  `persist_noclobber` for a new ordinary path and every POTA path. Use `persist`
  for an ordinary replacement confirmed for the exact destination.
  Drop staging files on cancellation or a write failure. Leave pre-existing
  destinations untouched on any failed attempt.
- [ ] Check all POTA destination names before staging and enforce no-overwrite
  again at publication. Track and report successfully published paths. Leave
  published paths alone after a partial failure, since checking identity and
  deleting the path are separate operations. Finish publication once begun.
- [ ] Verify through real temporary directories. Check that malformed prepared
  fields and cancellation keep existing content byte-for-byte. Use a real
  destination collision between staging and publication to exercise the
  publication boundary, preserving the colliding file and accounting for any
  earlier files from that attempt. Verify the reviewed name and completed file
  contents on success. Use the natural stage/finish split to arrange the late
  collision. Run `cargo test -p clacks-desktop --locked export::publish`.

## 4. Connect the desktop window and verify the workflow

**Files** — Add `lib/clacks-desktop/src/ui/export.rs` and the session job under
`lib/clacks-desktop/src/session/`. Update `menus.rs`, `ui/menu_commands.rs`,
`ui.rs`, and `session.rs`. Extend `ui/tests/menu_commands.rs` and exercise the
export window with the existing egui harness. Update the native File menu and
export presentation in `docs/desktop-ui.md` when the feature is delivered.

- [ ] Register `MenuCommand::ExportAdi` beside the import command and reuse
  native menu availability and dispatch. Capture the applied history query,
  active view and completed selection when opening. Preserve selected identities
  outside the filter and show their count. During an in-progress Select all,
  keep Selected unavailable while permitting the other explicit scopes. Share
  the desktop's prepared operation, repaint and cancellation conventions.
- [ ] Render scope, format and preparation states from the job. Refresh the
  request on scope or format changes and discard obsolete completions, including
  completions for a retired workspace. Render the POTA warning and **Show
  affected QSOs** beneath the POTA option. Enable Continue for a mixed eligible
  set after the warning is available, and keep it disabled for zero eligible
  QSOs. Continue freezes the reviewed skip set. Route ordinary ambiguity through
  the existing per-contact `ContextResolution` choices and retain omissions in
  the report.
- [ ] Use the existing parented asynchronous rfd dialog integration in
  `ui/import.rs` as the picker pattern. Use `save_file` for ordinary and
  `pick_folder` for POTA, preserving the native New Folder action. Show the
  frozen POTA file review and counts before opening the picker. Return to the
  preceding step on picker cancellation. Use the exact path returned by Save
  and confirm replacement in Clacks if it is occupied.
  While staging, show progress and Cancel. Release any picker exclusivity on
  its return and keep running export work from blocking logging or sync. Replace
  the export window body with the persistent success or failure result. Retain
  destination and contact, skip, entry, file and warning details until Done.
- [ ] Instrument the preparation and publication job with operation spans and a
  shared per-attempt trace correlation. At the desktop handling boundary,
  record elapsed duration by phase and bounded outcome. Emit structured totals
  for planned, skipped and published work where those facts become known. Use
  safe error categories and keep detailed paths, free text and contact identity
  in the local result. Verify truthful counts and data exclusion through one
  real export using the desktop's existing telemetry test support.
- [ ] Verify the normal command supplies the applied filter and hidden selection
  to setup. With semantic egui queries, exercise changing to POTA, its inline
  warning before Continue, accepted skips, all-ineligible state, consent on an
  ordinary ambiguity, and completion/failure in the same window. Exercise stale
  preparation by switching scope before its result is applied. Assert the
  actual request and outcome routed through the window rather than only testing
  widget presence.
- [ ] Validate the native Save and folder pickers on macOS and Windows in isolated
  development workspaces. Verify parent ownership, folder creation within the
  picker, exact-path replacement confirmation for ordinary Save, and correct focus after
  cancel and Done. Verify the Linux build/adapter path within its supported scope.
  Record any platform behavior not exercised. Run `cargo fmt --all --check`,
  `cargo nextest run -p clacks-desktop -p clacks-model -p clacks-client-store --locked`,
  and the relevant workspace checks in [the Rust workflow](../../../.github/workflows/rust.yml).

# Local History, Worked Before and Queries Implementation Plan

> **For agentic workers:** Use `superpowers:executing-plans` for the preserved
> native execution preference. Implement and verify each task before proceeding.

**Goal:** Deliver offline expression-based contact history and prefix Worked
Before using the production desktop, relational client store and native rules.

**Architecture:** Parse and validate a typed query in `clacks-model::query`;
compile it to bound relational SQL (Structured Query Language) in
`clacks-client-store`. Reuse native
evidence and effective-information selection for derived facts. Extend the
existing desktop request flow, tables and entry/details composition, sharing
Worked Before data across its tab and optional window.

**Tech stack:** Rust 1.99.1, SQLite/SQLx 0.9.0, winnow 1.0.3, regex,
num-bigint/num-integer, Tokio, egui/eframe and egui_extras 0.37,
egui_kittest, tracing and metrics.

Time comparisons use Coordinated Universal Time (UTC). Logbook of The World
is abbreviated LoTW.

**Spec:** [Local contact history, Worked Before and queries](../../history.md).
Read it alongside [desktop UI](../../desktop-ui.md),
[client storage](../../client-store.md) and
[service evidence](../../service-evidence.md).

## Global constraints

- Carry forward D328–D351 and the specification's reviewable E1–E9 defaults.
  Do not restore discarded form-filter decisions, span-overlap filtering or
  a `qso_time` field. Querying never edits contacts or operating settings.
- Keep Rust 1.99.1, `lib/<crate-name>` and thin `bin/<application-name>` crates;
  no `mod.rs` outside tests. Inherit dependencies from the workspace manifest.
- Use Rust-only tooling/fixtures. TrustedQSL sources remain reference-only.
- Preserve whole-location fallback, native receive overrides, recorded identity
  and service-specific applicability. No live provider/network prerequisite.
- Fixed queries remain `.sql` files with SQLx offline metadata. Dynamic structure
  uses `QueryBuilder<Sqlite>` and bound values; user text never supplies SQL.
- Use existing local-state key/value storage, ordinary transactions and request
  channels. No custom write queue, generic query backend or cache framework.
- Test a concrete Clacks requirement and plausible Clacks mistake. No dependency
  tests, injected triggers, failpoints, UI drivers or test-only production APIs.
- Follow the shared theme/components and assembled-UI completion gates. Linux
  continuous integration (CI) remains unchanged; do not enable Windows CI.
- Commit coherent deliverables with substantive bodies, physically wrapped at
  72 columns. Inspect message files before `git commit --file`; no push or PR
  creation without authorization. Preserve unrelated research changes.

## Review focus

1. Literal punctuation/Unicode must not become regex, SQL or delimiter syntax
   accidentally; Task 1 and Task 3 pin parsing and actual stored matches.
2. Missing values under NOT, whole-source fallback and band-only receive
   overrides must not broaden/narrow matches incorrectly; Tasks 2–3 pin IDs.
3. Multi-tag/reference rows and off-page selections must not inflate counts
   or confuse hidden with off-page; Task 3 uses independently expected totals.
4. An incomplete query, immediate close or late response must not lose the
   editor state or replace current results; Tasks 4–5 exercise that boundary.
5. Clearing the call, UTC midnight and a second Worked Before view must preserve
   one shared input/model without changing the QSO draft; Tasks 6–7 cover these.

## Concrete engineering choices

These refine E1–E9; they are implementation choices, not new product decisions.

### Parser, text and numeric support

Use winnow combinators with located input/token spans for the specified grammar;
do not adopt SQL syntax or implement a general expression framework. Version
1.0.3 is already in `Cargo.lock`; use its `expression` parser with prefix/infix
binding powers for NOT, AND (including adjacency) and OR, and combinators for
parentheses, case-insensitive whole-keyword boundaries, quotes and escapes.
Its `Parser::with_span` and `LocatingSlice` support diagnostics. Keep original
byte spans and escape/quote information through group/reference splitting.
Convert egui's character cursor to byte boundaries explicitly at the UI boundary.

Enable SQLx's `regexp` feature and `SqliteConnectOptions::with_regexp()` on the
normal store connection options. SQLx 0.9.0 implements that function with Rust
regex and caches compiled expressions through SQLite auxiliary data. Build
only escaped literal patterns: unanchored substring, `\A`-anchored prefix or
`\A…\z` whole value, all with Unicode case-insensitive matching. This implements
locale-independent **simple** Unicode case folding, without normalization or
accent stripping. It does not make user regular expressions a supported feature.
Use the same literal matcher for catalog-name resolution and suggestions.

Add workspace dependencies on the already locked `num-bigint` 0.4.8 and
`num-integer` 0.1.47. Parse a signed decimal coefficient and scale with winnow;
use `BigInt` and integer floor/ceiling division to lower numeric comparisons to
native integer bounds. Do not use floating point, round thresholds or add an
unrelated decimal-expression language. A nonintegral equality has no matches;
out-of-range bounds reduce to true/false for **present** values, retaining the
language's missing-value semantics. Accept decimal literals and the specified
units, not exponent notation. Whole-second timestamp parsing reuses `time`.

Support was checked against the locked dependency sources: `winnow/src/parser.rs`,
`combinator/expression.rs` and `stream/locating.rs`,
`sqlx-sqlite/src/regexp.rs` and `options/mod.rs`, and
`num-integer/src/lib.rs`. Regex's [Unicode documentation](https://github.com/rust-lang/regex/blob/master/UNICODE.md)
explains the simple-folding boundary. Source inspection establishes available
interfaces, not runtime performance or completed Clacks integration.

### Relational execution and native-derived facts

The compiler owns a closed field-to-column mapping. Scalar predicates, membership
and recorded participation use SQL predicates/`EXISTS`, including literal text
matching. Mode families are expanded from the current recognition catalog;
never match a preserved parent label against a contradictory specialization
without applying the catalog's interpretation. All positive predicates return
true/false, using explicit presence or `COALESCE(..., false)` before Boolean
composition, so `NOT` does not inherit SQL NULL behavior.

Effective names/locations and service status require existing native selection,
not a second SQL implementation of evidence ordering/applicability. For an
operation needing those facts, create a connection-local temporary relation
inside a nested SQLx transaction/savepoint under the established read snapshot.
Read scoped working inputs and relevant retained evidence in batches of 256
QSO identities; compute the requested derived columns with native functions,
and insert only QSO IDs and those query/display values into the relation.
Evaluate the complete expression, counts, ordering and page in SQL. Roll back
the nested transaction before returning; standard SQLx transaction drop handles
cancellation/error. Do not introduce custom cleanup guards or persistent caches.
Use one private `contact_query_facts` temporary table: `qso_id` primary key,
nullable effective name, own/contact position columns matching the native
grid/point representation already used by `recorded_location`, and the four
service-positive flags. Fields not requested by the operation remain unused.
Keep provenance/diagnostics in named native projection results, not SQL strings
that the UI would have to reinterpret.

Populate only the derived field families required by the query/order. For an
ordinary start-sorted query needing no derived predicate, select the page first
and project display facts only for that page. Do not load an entire account's
QSO objects or copy its recorded payloads into a temporary query engine. A
derived predicate can require examining the scoped contacts; memory stays
batch-bounded, and measurements must distinguish this path from the normal one.
Do not push a filter past OR/NOT unless its restriction is proven safe.

Use existing payload/evidence readers; add batch-selection queries for relevant
identities/support instead of reading unrelated account service history. Bind
ID sets as JavaScript Object Notation (JSON) text consumed by `json_each`,
avoiding interpolated ID lists
or one SQL bind per selected contact. Ordinary SQLx query-file metadata covers
fixed main-schema reads; temporary-schema statements are necessarily runtime SQL.
Add indexes only where actual query plans justify them, through a forward
migration; do not replace or rewrite existing migrations.

The directory-snapshot producer/storage is downstream work. Do not invent empty
provider records, a test-only snapshot insertion API or a new sync payload here.
Use retained eligible LoTW/QRZ facts now, and preserve the native selector's
directory input boundary for its eventual production producer. Absent evidence
stays absent. Record this delivery boundary truthfully in the crate guide.

## Task 1: Typed expression language and operator catalog

**Files:** Create `lib/clacks-model/src/query.rs` and
`query/{syntax,field,value,diagnostic,completion}.rs`; register the module in
`src/lib.rs`. Modify workspace/member manifests and lockfile. Create
`lib/clacks-model/tests/query.rs`, registered in `tests/integration.rs`.

**Interfaces:** `query::parse(text: &str, catalog: &RecognitionCatalog) ->
Result<Query, QueryDiagnostic>` returns a validated Boolean tree, with `All`,
`Predicate`, `Not`, `And`, `Or`. Predicate fields are closed `Field` variants;
values distinguish literal text, timestamp ranges, integer comparisons, modes,
grids, group-qualified references and Booleans. Use named types/fields, not
tuples. `field::definitions() -> &'static [FieldDefinition]` owns names,
descriptions, types, allowed comparisons and source bindings.
`completion::site(text: &str, caret_byte: usize) -> CompletionSite` supplies
context/replacement range for complete **or incomplete** text. Diagnostics
carry a byte range and actionable category/message; derive `thiserror::Error`.

- [ ] Add `precedence_and_literal_tokens` in `query.rs`: adjacency equals
  explicit AND for
  `comment:activation comment:rain -name:alice`; parentheses change OR grouping;
  NOT binds before AND before OR; `comment:">5"` is literal substring while
  `comment:=">5"` is literal equality. Unknown field/operator and unfinished OR
  return diagnostics spanning the offending text, never `All`.
- [ ] Check lowercase/mixed-case Boolean keywords against the expected tree;
  quoted keywords, named-field values and longer words remain literal. Reject
  incomplete or misplaced operators in every casing.
- [ ] Add `typed_fields_dates_units_and_diagnostics` for every field in spec §6,
  including E1 operator
  terms and recorded variants. Pin escaped slash handling in a group/reference,
  timestamp colons, a quoted reserved word and Unicode text preceding an error.
  Assert `>14.0000005MHz` lowers to `>=14000001Hz`, `14000kHz` equals `14MHz`,
  `>2026-10-27` starts on October 28 UTC, and absent-end comparisons have no
  synthetic start fallback. Include an offset timestamp and invalid fraction.
  Presence follows each field's binding; derived service Booleans are always
  defined, so their `has:` test is true rather than a provider-status shortcut.
- [ ] Run `cargo test -p clacks-model --test integration query --locked`;
  confirm failures concern the missing Clacks language, then implement the
  catalog, parser, typed conversions and diagnostic/completion positions.
  Preserve original text separately; no serialized query tree as stored truth.
- [ ] Run that command again and `cargo fmt --all -- --check`. Verify new public
  items have caller-focused rustdoc and no panic/indexing shortcuts. Commit the
  language/catalog deliverable with motivation and the exact matching boundary.

## Task 2: Native read projections for effective and service-aware values

**Files:** Create `lib/clacks-client-store/src/read/projection.rs` and focused
`projection/{evidence,station}.rs`, fixed selection files under
`sql/read/projection/`, and named row types under `src/rows/`. Modify
`src/read.rs`, relevant existing payload readers only as required for reuse,
and `.sqlx/`. Create `tests/history_projection.rs`, registered in integration.

**Interfaces:** Keep implementation crate-private:
`projection::load(snapshot: &mut ReadSnapshot, ids: &[QsoId],
needs: ProjectionNeeds) -> anyhow::Result<Vec<ContactProjection>>`.
`ProjectionNeeds` identifies requested name/location/service facts;
`ContactProjection` has named QSO identity, effective contacted name, whole
own/contact locations with source, per-service uploaded/confirmed Booleans and
actionable evidence diagnostics. Recorded receive resolution uses the whole
native override. Expose only the display portion later through `ContactRow`.

- [ ] Add `retained_evidence_projects_current_service_status` with one shared
  real-store scenario using normal saved evidence: positive
  imported upload and confirmation, retained successful attempt/result,
  associated selected observation, and a relevant QSO correction. Assert
  current coverage and confirmation independently. Keep submission/signature
  creation out of these tests; reuse native service fixtures and APIs.
- [ ] Pin 2025 QRZ book coverage after a 2026 book is configured, LoTW certificate
  renewal without loss of status, and applicable imported positives without
  destination provenance. A conflicting book association produces a diagnostic,
  not unrelated-book aggregation or invented migration. Paused/missing
  credentials are not read-side evidence invalidation.
- [ ] Add `effective_facts_preserve_source_and_station_side`: recorded partial
  location blocks later-source field filling,
  location-bearing participation suppresses directory/provider geography,
  eligible LoTW outranks QRZ, and inapplicable/wrong-side observations do not
  become fallback. Include name fallback independently of location selection.
- [ ] Run `cargo test -p clacks-client-store --test integration history_projection
  --locked` red, then implement scoped selection with current working roles,
  `select_observations`, `publication_coverage`, `confirmation_applicability`,
  `eligible_station_facts`, `select_name` and `select_location`. Reuse retained
  mapping/signing descriptors for comparisons; no private keys or network calls.
  Interpret `Covered`/`Confirmed` as positive; retain reasons when support is
  insufficient. Do not select observations by response arrival or latest wall
  clock, or resurrect superseded/deleted associations.
- [ ] Run the focused tests green, regenerate/check fixed-query metadata using
  Task 8's commands, and commit the read-projection integration. Confirm no
  stored QSO/evidence changes result from these reads.

## Task 3: Relational query pages, counts and selection inspection

**Files:** Create `lib/clacks-client-store/src/read/query.rs` and
`query/{compiler,predicate,derived}.rs`; modify `src/read/contacts.rs`,
`src/read/history.rs`, `src/store.rs`, manifests, row types and affected SQL
under `sql/read/`. Create `tests/history_queries.rs`; extend existing
`tests/contact_pages.rs`, not a second per-file test executable.

**Interfaces:** Extend `ContactPageQuery` with `filter: Query`,
`selected_ids: BTreeSet<QsoId>` and `show_selected: bool`, replacing its optional
subset field. Preserve view/order/offset/limit. Return `ContactPage` containing
`Vec<ContactRow>`, `matching_total`, `selected_total`, `selected_outside_filter`
and `has_more`. `ContactRow` contains the existing `HistoryRow` plus projected
display facts and references; no second editable QSO. Counts are `u64` after
checked database conversion. Extend `ContactColumn` with end, own-station
information and contacted references needed by either table. Keep existing
variants/serialized preferences compatible. Specifically add `End`, `OwnCall`,
`OwnGrid`, `OwnReferences` and `ContactReferences`; retain the existing report
columns. `ReadSnapshot::contact_page`
retains its async signature with the extended request/result types.

- [ ] Add `expressions_select_unique_working_contacts` with one relational
  scenario with multiple memberships, duplicate tag names,
  group-only participation, paired group/reference identifiers and an unknown
  group. Assert exact QSO ID sets for repeated AND, explicit OR, negation and
  `has:`. Include `comment:"100% _ [x]"` and non-ASCII case matching so neither
  SQL wildcards nor regex punctuation acquires special meaning.
- [ ] Add `field_bindings_preserve_missing_and_position_semantics` using the
  same fixture with effective/recorded name and grid differences,
  explicit points, CN/CN85/CN85ab areas, and a band-only receive override.
  `contact.grid:CN85ab` excludes CN85/CN; `=CN85ab` has the same geographic
  predicate. Pin effective-versus-recorded presence and `NOT` of missing fields.
  Exercise all native position precisions without rounding a grid to its center.
- [ ] Add `typed_domain_predicates_match_saved_facts`: SSB includes USB/LSB,
  USB excludes LSB, unfamiliar mode labels match
  without guessed families, and a catalog interpretation change alters matches
  without changing the contact. Assert unit/fraction/date predicates against
  saved values, and the four service Boolean predicates from Task 2's evidence.
- [ ] Extend `contact_pages_sort_complete_values_and_break_ties_by_id` and add
  `filtered_counts_and_rows_share_a_snapshot` with equal timestamps, an off-page
  selected
  match and a selected nonmatch. Assert stable unique pages, complete matching
  total and outside-filter count (not off-page count). Show selected bypasses
  the expression only for rows; counts remain against the expression. Deleted
  rows never appear or count. Use a committed edit between snapshot reads to
  prove Clacks keeps page/projection/count reads on the same snapshot.
- [ ] Run `cargo test -p clacks-client-store --test integration history_ --locked`
  and the `contact_pages` filter red. Implement bound compilation and the
  operation-local derived relation described above. Use current catalog/group
  records from the same snapshot. Compute precise grid containment with native
  point/area identity and rational bounds; reuse native geometry rather than
  an approximate SQL grid-center conversion. Remove the old desktop-only fixed
  contact filter SQL when replaced; retain `HistoryQuery` only for actual
  existing consumers, with its distinct purpose documented.
- [ ] Run both focused filters green; inspect query plans for the normal
  empty/start-sorted and prefix paths. Record execution time and visited-row
  counts on a reproducible Rust-authored larger scenario outside the default
  test suite; do not add a timing gate or benchmark framework. Confirm derived
  reads stay batch-bounded. Commit the real query reader/compiler deliverable.

## Task 4: Query-state persistence and asynchronous desktop operations

**Files:** Create `lib/clacks-client-store/src/read/query_state.rs`; modify
`src/read.rs` and use `src/local_state.rs`. Extend desktop
`src/session/settings/contacts.rs` and `src/session/settings.rs`. Create
`lib/clacks-client-store/tests/history_query_state.rs` and extend desktop
contact-session tests in their existing test target.

**Interfaces:** `QueryState { edited: String, applied: String }` is native local
preference data, not a parsed query tree. `ReadSnapshot::history_query_state() ->
anyhow::Result<QueryState>` and
`query_state::save(store: &ClientStore, next: &QueryState) -> anyhow::Result<()>`
use one versioned value at `history.query`. Add `ContactAction::LoadQueryState`
and `SaveQueryState { state: QueryState }` with typed acknowledgments, alongside
the existing `Page` request. No schema migration is required.

- [ ] Add `query_state_restores_unfinished_text_and_current_results`: save
  `edited = "mode:SSB OR"`, `applied = "mode:SSB"` through the actual
  store API, reopen and assert both strings survive while current saved data
  is queried anew. Extend the same scenario to distinguish absent-key defaults
  from an unsupported stored version; the latter is an actionable load error,
  not silent success with an empty query. Do not test JSON/SQLx round trips.
- [ ] Run `cargo test -p clacks-client-store --test integration history_query_state
  --locked` red; implement local preference serialization and reads/writes.
  Execute page reads off the UI thread through the existing session and always
  close the bounded snapshot. Preference writes do not refresh shared settings
  or create pending synchronization work.
- [ ] Add a session assertion that a syntactically valid query read returns the
  corresponding page/counts and the existing saved-QSO operation still returns
  its authoritative result. Run `cargo test -p clacks-desktop --lib contacts
  --locked` and the store filter green; commit this production session boundary.

## Task 5: Always-visible query editing and history integration

**Files:** Create `lib/clacks-desktop/src/ui/contacts/query.rs` and
`query/{suggestions,tests}.rs`; modify `ui/contacts.rs`,
`contacts/{responses,table,preferences}.rs`, `ui/workbench.rs` and `ui.rs`.
Extend existing table tests rather than creating a duplicate rendering suite.

**Interfaces:** Private `HistoryQueryUi` owns edited/applied text, parse
diagnostic, completion state and the 250 ms deadline. It produces validated
`Query` inputs and `QueryState` writes for Tasks 3–4. `ContactsUi` remains the
owner of selected IDs, page requests and the existing editor/deletion workflow.
Retain the exact submitted request inputs with each oneshot receiver and accept
only a response for the current view/query/order/inspection state. Use existing
request identity/channel ownership; no independent cache-generation subsystem.

- [ ] Add `invalid_query_retains_applied_results_and_completion_focus` using
  egui_kittest's native input/clock to apply `mode:SSB`, then leave
  `mode:SSB OR` unfinished. Assert one page request for the valid expression,
  no broadened request for invalid text, inline diagnostics/unapplied styling
  and the displayed applied query. Acceptance of a field/value suggestion uses
  only its replacement range, including a Unicode caret case. Escape dismisses;
  Enter/Tab accepts a suggestion without saving/opening a QSO. No sleeps.
- [ ] Add `query_replies_follow_current_inputs`: rapid edits coalesce at 250 ms
  and an older page response cannot
  overwrite a newer query/view/order. A failed read keeps the last complete
  page and edited text with failure feedback. Test this at the real channel
  boundary, not through a generic request simulator or custom event driver.
- [ ] Implement the bar **inside** the existing history section. Suggestions
  come from the same field catalog and current band/mode, tag/group and Boolean
  data; include brief field descriptions and only type-valid operators/units.
  Do not offer fetched contact names/comments as a new lookup capability.
  Keep status geometry stable; show no flashing validation-progress line.
  `Workbench` retains pane allocation; the existing history section owns its
  sole frame/padding. The query row/table expand to that content width; the
  native completion popup anchors to the edit control without resizing panes.
- [ ] Integrate selected totals/outside-filter counts, reversible Show selected,
  Clear selection and existing row activation. Preserve selected IDs through
  sorting/paging/query edits; clear on view change and remove actual deletions.
  Restore edited/applied state before issuing the initial filtered request.
  Preserve history defaults and local column preferences.
- [ ] Coalesce preference writes with one write in flight and the latest pending
  state. Before normal shutdown admission closes, flush the final edited/applied
  pair and await its acknowledgment asynchronously; do not wait out the debounce
  or block rendering. Report failure and retain ordinary retry/force-quit
  behavior. Add `shutdown_flushes_unfinished_query` against the session
  path, not subprocess termination or SQLite durability.
- [ ] Run `cargo test -p clacks-desktop --lib --locked`; review affected tests
  against the requirement/mistake rule. Inspect the running production query bar
  in empty/populated/invalid states, at narrow/wide sizes and Light/Dark. Confirm
  history fills its bounds and the upper layout is unchanged. Commit the query
  UI integration with actual verification limits recorded in the commit/PR.

## Task 6: Prefix Worked Before data and shared model

**Files:** Create `lib/clacks-client-store/src/read/worked_before.rs`; extend
`src/read.rs`, `read/contacts.rs` and desktop `session/settings/contacts.rs`.
Create `lib/clacks-client-store/tests/worked_before.rs`, register it in
`tests/integration.rs`, and create `lib/clacks-desktop/src/ui/worked_before.rs` with focused
`worked_before/{model,tests}.rs`. Modify `ui/logging.rs` to expose the existing
entry callsign text read-only; do not expose mutable draft internals.

**Interfaces:** `WorkedBeforeQuery { view: SelectedView, call_prefix: String,
order: ContactOrder, offset: u32, limit: u32 }`;
`ReadSnapshot::worked_before(&WorkedBeforeQuery) -> anyhow::Result<ContactPage>`
uses the same working-row/projection/table reader, with no history expression
or bulk selected set. Add `ContactAction::WorkedBefore { query }` and its typed
response. Private `WorkedBeforeModel` owns one normalized prefix/view/request
and current page. `LoggingUi::contact_call(&self) -> &str` supplies its input.
Today is calculated for display from the supplied current UTC day, not saved
as a database flag or an implicit history filter.

- [ ] Add `worked_before_follows_prefix_scope_and_working_state`: W7HEX matches
  W7HEX and W7HEX/P across own
  station callsigns, W7HEX/P excludes W7HEX, another selected tag scopes the
  result, history `mode:USB` does not restrict it, and pending working edits
  replace rather than duplicate the row. Deletion removes it. Keep old dates.
- [ ] Implement `worked_before` by building the internal typed call-prefix
  predicate directly, **not** concatenating callsign text into query syntax.
  Normalize trim/case like recorded callsign entry and preserve slash components.
  Empty input returns no rows and the desktop sends no database request.
- [ ] Add `worked_before_rejects_old_calls_and_refreshes_today`: changing or
  clearing the call discards
  previous replies; unchanged repaints cause no new request. Advance controlled
  UTC time from 23:59:59 to 00:00:00: Today changes, older rows remain, and no
  QSO field changes. Schedule repaint at the next UTC-day boundary with the
  framework's facilities; do not create a timer service or wall-clock sleeps.
- [ ] Run `cargo test -p clacks-client-store --test integration worked_before
  --locked` and `cargo test -p clacks-desktop --lib worked_before --locked`
  green. Commit the working prefix reader/model with its production consumers.

## Task 7: Worked Before tab, reusable table and dedicated window

**Files:** Modify `ui/logging.rs`, `ui/workbench.rs`, `ui.rs` and
`ui/contacts/{table,preferences,editor}.rs`; create focused
`ui/worked_before/{table,window}.rs`. Extend existing workbench/component tests.
Extract shared contact-table presentation into `ui/contact_table.rs` and small
submodules only where the two real consumers need it; do not create an unused
component library or relocate unrelated code.

**Interfaces:** `Workbench` owns one `WorkedBeforeModel`, visible from the new
`DetailTab::WorkedBefore` and the optional viewport. A shared table renderer
accepts `ContactRow` slices, table preferences and ordinary selection/activation
results; it neither queries nor creates a draft. Dedicated viewport rendering
uses `Context::show_viewport_immediate`, so existing application-owned state can
be borrowed directly. Window-local scroll/focus IDs differ; column settings and
sort order are shared between the two Worked Before presentations. Closing the
child viewport changes only its visibility. Route row activation to the same
saved-contact editor owner, not a second version of contact-editing logic.

- [ ] Add `worked_before_shares_entry_and_activates_existing_editor` with native
  component assertions: switching to Worked Before leaves
  the quick entry values unchanged; default columns are UTC Start, Callsign,
  Band, Mode, Contacted grid, Contacted activity references with compact Today
  and pending/conflict markers. Own information/reports remain configurable.
  One row activation requests the existing editor for its exact QSO ID.
- [ ] Implement independent history/Worked Before table preference keys and
  defaults, column visibility/order/widths/sorting/reset, horizontal scrolling
  and full UTC timestamp comparison. Display effective contacted grid and
  recorded references, including unknown group identity without guessed labels.
  Keep rows single-line; do not render references as multiline contact cards.
  The logging region owns its section frame/padding; tab content adds no second
  outer frame. Each table owns its scroll area and fills the available content
  width. The child viewport supplies its own single section frame, using the
  same theme and row renderer; it does not reuse the main pane's scroll ID.
- [ ] Add Open in window and reuse the exact shared model/data. Wire refresh
  after actual saves/deletions, selected-view changes and committed settings/
  group changes. Keep one explicit workbench invalidation boundary that future
  sync/provider producers can invoke; do not implement those producers now.
  Recognizer changes recompile affected queries without editing saved records.
- [ ] Run desktop library tests and inspect the assembled production app: both
  tab and window open, edit/clear callsign, change view, activate an editor,
  resize/scroll/sort and close/reopen the child. Confirm the workbench keeps
  its panes and unfinished QSO throughout. Exercise the actual additional
  viewport; a headless widget test is not evidence of native window behavior.
  Commit the integrated views/component reuse.

## Task 8: Operational diagnostics, verification and delivery documentation

**Files:** Update the affected crate READMEs, `docs/history.md`,
`docs/desktop-ui.md`, `docs/client-store.md` and `docs/observability.md` only
where their owning contract/usage needs it. Extend existing observability tests
in the model/store/desktop targets; keep no checked-in review/timing report.

- [ ] Add ordinary `#[instrument(skip_all, ...)]` spans around logical validation,
  store query and desktop request handling. Use default targets and structured
  outcome events; no redundant operation/component fields or lifecycle wrappers.
  Parse diagnostics are debug; repeated searches are debug; actionable read/
  persistence failures follow existing handling-boundary warning/error policy.
  Do not create an info event per keystroke or log query text through error Display.
- [ ] Add `client_store.contact_query.duration_seconds` for store-read latency
  and `client_store.contact_query.failures_total` only if not already represented
  by an equivalent owning metric. Use bounded `kind=history|worked_before` and
  failure category labels; record result count on the completion event, not as
  a label. Fold a successful/invalid/failed operation and private sentinel into
  existing telemetry tests to pin truthful reporting and query-data exclusion;
  do not duplicate the functional suite with telemetry enabled/disabled.
- [ ] Regenerate SQLx metadata with a disposable development database using
  production migrations and the pinned installed SQLx CLI, then verify:

  ```sh
  DATABASE_URL=sqlite:///tmp/clacks-history-query-check.sqlite cargo sqlx database create
  DATABASE_URL=sqlite:///tmp/clacks-history-query-check.sqlite cargo sqlx migrate run --source lib/clacks-client-store/migrations
  SQLX_OFFLINE=false DATABASE_URL=sqlite:///tmp/clacks-history-query-check.sqlite cargo sqlx prepare --workspace -- --package clacks-client-store --all-targets --locked
  SQLX_OFFLINE=false DATABASE_URL=sqlite:///tmp/clacks-history-query-check.sqlite cargo sqlx prepare --check --workspace -- --package clacks-client-store --all-targets --locked --offline
  ```

  Use an actually unused temporary path at execution; do not overwrite a workspace.
  Review changed `.sqlx` entries and remove metadata only for removed queries.
- [ ] Run `cargo fmt --all -- --check`,
  `cargo build --workspace --all-targets --locked`,
  `cargo test --workspace --all-targets --locked`,
  `cargo test --workspace --doc --locked`,
  `cargo clippy --workspace --all-targets --locked -- -D warnings` and
  `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --document-private-items --locked`.
  Run `cargo run --bin clacks-log -- --help` to retain headless startup coverage.
  Report actual results; do not infer Linux/Windows verification from macOS.
- [ ] Complete `docs/desktop-ui.md`'s visual gates on the production executable,
  including query completion/error placement, empty/loading/populated pane
  bounds, selected-row indicators, single-line Worked Before, fonts/scaling,
  Light/Dark and extra-window behavior. Record screenshots/platforms/limits in
  the delivery response or PR. Native Windows acceptance uses the production
  executable when a human has access; unavailable acceptance remains explicit.
- [ ] Self-review spec §§1–9 against these tasks and audit new/retained tests
  for actual Clacks assertions and avoidable fixture/setup cost. Document usage,
  supported fields/defaults, derived-query cost and the downstream directory
  producer boundary, without presenting future networking as implemented.
  Commit final integration/documentation in the relevant coherent commits;
  fold branch-introduced repairs into those commits. Stop for review before
  any authorized push/PR work, leaving the implementation plan until merge.

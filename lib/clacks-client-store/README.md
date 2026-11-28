# Local account store

`clacks-client-store` owns one SQLite database per local account workspace.
Native values, saved roles, operating defaults and received account state use real SQLx transactions. The
[storage contract](../../docs/client-store.md) defines the payload and
synchronization workflows implemented by this library.

`store::ClientStore::create` exclusively creates a local-only workspace;
`open` requires an existing workspace and the expected account ID.
`open_discovered` uses the stored account when only the local path is known,
sharing the same read-only preflight. `account_id()` supplies record qualification
after discovery; `identity::status` returns non-secret enrollment presentation.
`create_with_definitions` installs supplied groups/output profiles and the new
account row in one transaction after schema creation. It leaves no initialized
account on failure, preserves the exclusively created file, and never reseeds an
existing workspace. Raw `create` remains the empty provisioning path. Startup
inspects migration history and account in one read-only snapshot
through a read-only connection before enabling WAL or running migrations.
Unsupported newer schemas and wrong accounts are rejected without changing
user data. SQLx migration history is the sole schema-version record. Concurrent
migration attempts may return ordinary database/migration errors; startup can be
retried against fresh history. Once opened, preflight and initialization connections
are explicitly closed before returning success or an operation error, so retries can immediately
replace a failed database file. Reads and writes use SQLite transactions.

The pool enables foreign keys, WAL and `synchronous=FULL` on every connection,
with SQLite's standard automatic checkpointing. Close the store before making
a closed-file copy, or use SQLite's consistent online backup facilities.
Copying a live main database file alone is not a consistent backup. There are
no built-in backup/restore commands or automatic clone detection.

`identity::bind` receives an already verified enrollment. It stores only the
protected credential reference and checks the exact prior binding in an
immediate transaction. Initial enrollment requires an empty local workspace;
reauthorization of the same account preserves prior data and author identities.
Authentication and protected credential access belong to runtime composition.

`enrollment::first::install` adopts the verified account in the existing empty
standalone database and returns a newly account-qualified handle. It replaces
only unchanged unpublished bundled definitions, preserving local operating
facts and preferences. Independently populated shared data rejects enrollment
without changing the local account or records (D56). The owning session drains
old operations and reopens its resources before admitting account work again;
stale account-qualified handles cannot write after adoption.

`enrollment::install` distinguishes same-actor credential replacement from a
fresh actor. A fresh actor atomically places existing pending records on a local
admission hold. Original values, frozen proposals and old counters remain intact;
unrelated new records are not held. Preparation reports the hold before loading
missing comparison evidence or allocating a counter. Existing actor-exclusion
reviews remain usable; incoming shared observations create that normal review
when comparison becomes available. Only exact-basis explicit resolution clears
the corresponding hold, including a choice of the shared version.

`enrollment::attempt`, `save_attempt` and `clear_attempt` persist completion
boundaries in the shared local-state key/value table. These values contain only
the native credential reference, origin, destination and expected binding.
Clearing an old attempt cannot remove a newer one. No signing key or usable
access/refresh credential is stored in SQLite.

`directory::installation::reference` allocates a fixed workspace-local native
entry name under `directory.entry` in the existing `local_state` table. Updates
and reauthorization retain that name; simultaneously open workspaces remain
independent. The complete configuration and optional login live together in the
native entry. No directory configuration or installation transition is committed
to SQLite; synchronized contact-associated snapshots remain separate.

## WS Suite receiver preference

`ws_suite::{load,save}` owns the typed `ws_suite.receiver` value in the existing
workspace-local `local_state` table. Missing values default to disabled loopback
unicast without writing. Save validates active fields and distinguishes semantic
changes from no-ops. A disabled apply preserves the committed connection and
changes only its enabled flag. Explicit multicast interfaces retain OS name and
local address; numeric interface indexes are resolved at activation, not stored.

The desktop owns socket activation and retries independently of the committed
preference. This setting is not synchronized and stores no capture receipts,
packets or contact-processing history. Incoming QSOs use ordinary
`save::contacts::save` and existing contact read/synchronization facilities.

## Checked SQL

Crate-internal types in `src/rows/` group the relational schema by record family.
Queries and mappers import named decoder rows directly. Writes bind from native
variants or an immutable encoding constructed from them. Shared physical
representations, such as position fields and destination rows, have one definition;
native domain values remain the library's public interface.

Client metadata separates shared observations, original comparison evidence and
frozen proposals in `src/metadata/client/`. Group mappings separate definitions,
export plans and expression chains in `src/payload/definitions/`. Transfer
acceptance tests are grouped by scenario under `tests/transfers/`.

Fixed statements live in `sql/` and use `query_file!` or `query_file_as!`.
The workspace `.sqlx/` directory is generated metadata, not hand-authored schema.
Normal compilation uses `SQLX_OFFLINE=true`, without a database connection.
To regenerate it, install the pinned standard tool:

```sh
cargo install sqlx-cli --version 0.9.1 --locked --no-default-features --features sqlite
```

Choose a fresh temporary SQLite file, set both `DATABASE_URL` and
`CLACKS_SQLITE_DATABASE_URL` to its SQLite URL, and
run from the workspace root with `SQLX_OFFLINE=false`:

```sh
cargo sqlx database create
cargo sqlx migrate run --source lib/clacks-client-store/migrations
cargo sqlx prepare --workspace -- --package clacks-client-store --all-targets --locked --offline
cargo sqlx prepare --check --workspace -- --package clacks-client-store --all-targets --locked --offline
```

`--workspace` keeps metadata in the root `.sqlx/` directory; the Cargo package
selection limits compilation to the client store, including its test queries.
The crate's standard `sqlx.toml` selects `CLACKS_SQLITE_DATABASE_URL` for its
macros. Leave that variable unset when preparing PostgreSQL metadata; dependent
client queries then use their committed offline metadata rather than the
server's PostgreSQL connection.
The Linux verification workflow uses the same scoped check and caches
Cargo dependencies, compiled dependency artifacts and the installed SQLx CLI.
Cache hits do not skip metadata verification or any workspace checks.

Commit query metadata with changes to queries or migrations. Applied migrations
are forward-only; do not edit their checksums in deployed workspaces. Each
migration commits its schema change and SQLx history row together. A failed
migration leaves earlier committed migrations intact.

## Dependencies

SQLx 0.9.0 (MIT or Apache-2.0) and Tokio 1.54.1 (MIT) run under the repository
Rust toolchain. The `sqlite-bundled` feature builds SQLite 3.51.3 C sources using
a platform C compiler; it does not require an external database service.
`libsqlite3-sys` uses the MIT license; SQLite itself is public domain. Binary
packaging must retain applicable third-party copyright and license notices.
Only the SQLite backend and Tokio runtime are enabled; no SQLx TLS backend is
needed for a local file. The SQLx CLI is developer tooling, not a runtime
application or project task dispatcher.

`save` and `operating` commit guarded offline edits and owned defaults. `synchronization`
commits native preparation, receipt, rejection and explicit resolution with their
comparison roles. No individual record response advances completed account coverage.
The client retains its actual checkpoint and causal interpretation, without an
unused mirror of server account/membership metadata.

## Transactional workflows

- `logbooks::selection::SelectedView` represents All contacts, Untagged, or a
  stable tag ID. Migration 0010 creates shared workspace-local `local_state` key/value
  storage. The `logbooks.selected_view` key holds the serialized native enum;
  an absent key defaults to All contacts, and invalid values return decoding errors.
  It is not synchronized or stored with operating settings. Rust validates the
  value; `selection::select` commits a validated live
  target immediately; `selection::restore` repairs only known effective deletion
  before startup reads. Missing/unknown targets are preserved as unavailable.
- `settings::snapshot` includes `logbooks::LogbookSnapshot`, reading the catalog
  and selected view in the same ordinary snapshot transaction as operating state.
  Later QSO entry consumes this view: a named tag supplies the new-entry default,
  while either built-in supplies none. Switching views does not edit saved QSOs.
- `logbooks::tags::save` validates a nonblank name and exact editor identity,
  then uses the ordinary pending-record save guard. Duplicate names are valid;
  spelling is preserved. Saving does not select the tag.
- `logbooks::deletion::{preview, delete_tag}` uses exact tag/member records and
  precomputed distinct counts. LeaveAll removes membership, DeleteAll marks all
  members deleted, and DeleteUnique marks only unique members deleted. Any changed
  member invalidates confirmation, even if counts match. The transaction also
  falls back to All contacts when this tag is selected; its result carries the
  authoritative selected view and optional fallback cause.
- Effective tag deletion at the shared client-role storage boundary also clears
  the matching selected view inside receipt, transfer and resolution transactions.
  A staged tombstone or a live pending working alternative does not trigger it.
  Incoming deletion does not replay local contact-deletion choices or remove
  deleted-tag references from pending QSOs; those require explicit repair.
- `save` preserves the original basis and any already-frozen proposal while
  changing working values. Exact expected rows guard editor saves and tag changes.
  Tag deletion rechecks the complete preview before committing all effects together.
- `save::tags::change_tags` applies explicit inclusion/removal to captured complete
  QSO bases, one guarded transaction per contact. Its progress callback can stop
  before the next save. The final report retains saved/unchanged/stale outcomes,
  an operationally failing identity and the exact unprocessed remainder; completed
  saves are never replayed or rolled back as a batch.
- `operating` owns current values and independent optional profile/equipment
  source copies. Apply saves their values and selections together; later shared
  source edits do not reload them. Manual adjustments clear the affected selection.
- `synchronization::prepare` persists reservations and frozen values before returning
  them. `receive`, `reject` and `resolve` save actual core-produced role/review updates.
- `transfer::{begin, install_page, complete}` owns page transactions and final
  checkpoint installation. The caller holds connection-local `ReceiptState`;
  requirements are not persisted as transfer attempts. Commit data before updating
  that state or acknowledging a page. Complete only against a matching discovery
  end and actual stored obligations. Reconnect starts a fresh receipt while semantic
  staging, exact support, coupled descriptors and pending work remain durable.
  `discovery::Source` returns accepted heads/support, never pending working values.
  Its `repair` traversal offers individual accepted knowledge beyond the peer’s
  complete coverage, including retained-only records, without certifying a prefix.
  Dependency loading follows incoming, staged and local accepted versions, including
  support already stored outside the window. Without an embedded destination
  description, it supplies the stored destination to the native dependency checks.
- `retention` installs received interpretation while sealing covered pending bases.
  After a completed desktop sync cycle, `reclaim_completed_cycle` removes one
  bounded batch of unreferenced relational copies. It preserves originals,
  witnesses, current/pending/prepared/review roles, current actor counters and
  active operating values; it neither advances coverage nor expires enrollment.
  Equivalent baseline representations share equal native payloads.
  Insufficient coverage keeps original evidence. Physical collection is a later
  consuming maintenance workflow; this store does not run an unintegrated collector.
- `read::snapshot` establishes one read transaction for history, review, operating
  values, record roles and completed-checkpoint diagnostics. Keep it only for a bounded
  operation; it can retain WAL pages until closed or dropped. History filters use
  bound values and one working payload per QSO; review includes pending deletions.

Desktop entry/import/export and WebSocket integration extend these production
operations. They supply verified enrollment, authenticated source metadata and
actual accepted envelopes. They do not replace this store with a separate repository
or run server acceptance locally. Directory snapshots use the native
`RecordId::Directory(QsoId)` carrier and relational payload/snapshot/reference
rows; they never modify the recorded QSO or form a reusable callsign cache.
`directory::snapshots::apply` checks the live parent's current normalized call
inside its write transaction. `save::contacts::save` can attach an available
success atomically, including an unchanged creation retry; a stale edit attaches
nothing. Transfer staging waits for live/deleted parent evidence, and already
accepted history remains inert under a deleted parent.

Snapshot application returns transaction-local `SnapshotOutcome` information:
an applied `SnapshotChange` distinguishes new, updated and metadata-only winners,
while unchanged winners and parent/key ineligibility remain explicit. A
metadata-only winner still commits normal synchronized work; it is not a
material facts change. Callers notify local consumers for every applied winner.

The pool's `before_acquire` hook discards a connection whose SQLx transaction
tracking remains active after queued rollback work drains. This is a compatibility
workaround for SQLx 0.9.0 after SQLite automatically rolls back a storage-full
transaction; it is not supplied by SQLx's standard connection ping.

## Verification

```sh
SQLX_OFFLINE=true cargo test -p clacks-client-store --all-targets --locked --offline
SQLX_OFFLINE=true cargo clippy -p clacks-client-store --all-targets --locked --offline -- -D warnings
```

Tests assert Clacks decisions through saved values and their consuming workflows:
stale editor rejection, frozen proposal retries, transfer staging and receipt,
source detachment, deletion choices, query results and committed telemetry.
Reopening verifies that Clacks retained the state needed to continue those
workflows. It does not independently verify SQLite crash durability. Connection
option reporting, generic constraint enforcement and dependency rollback behavior
are not Clacks test subjects. The repository workflow runs the suite on Linux
without child-process interruption harnesses or external services. Windows CI is
temporarily disabled because of its runtime cost; Windows platform verification
remains a release obligation.

## Operating settings and reusable records

The [operating-settings contract](../../docs/operating-settings.md) uses
`operating::OperatingState`: optional current facts plus independent
`selected_profile` and `selected_equipment` copies. `operating::save` takes the
expected active state and returns Saved, Unchanged or Stale. Construct active
values and their exact source copies together with `OperatingState::new`.
`adjust` replaces copied values and detaches only affected sources;
`forget_source` removes a selection without changing those values. Saved views
are read-only to callers. Saving does not require sources to be fresh. Known effective tombstones clear selections in the
same transaction that installs their working deletion; absence is not deletion.

`settings::snapshot` and `ReadSnapshot::settings` read current settings, complete
profile/equipment roles and group definitions consistently. UI selectors use live
working values while preserving tombstones and unavailable roles in the snapshot.
`settings::save_profile` returns a missing-equipment issue separately from the
save outcome. `settings::save_equipment` changes only the reusable record.

`preview_equipment_deletion` captures the target, every referring profile and
available replacement records. `delete_equipment` requires an explicit choice
for every referrer and revalidates the complete set, including new referrers,
under one immediate transaction. Repairs, tombstone and active-source cleanup
commit together. Generic shared saves cannot bypass this review for referenced
equipment. Selected baseline payloads are retention roots; changing or deleting
a source never rewrites historical QSO snapshots.

Stale Apply and deletion reviews emit focused diagnostic reasons for changed
active settings, a changed target, changed referrers or a changed replacement.
These are noncommitting outcomes, not mutation successes. IDs remain structured
trace fields; equipment descriptions and other recorded values are excluded.

The forward startup migration separates older combined loaded metadata without
reloading saved sources: it retains a station selection only when the old copied
station still matches the current station semantically, starts with no inferred
equipment selection, and preserves current facts. Fixed queries and committed
SQLx metadata remain the schema contract. Regenerate/check metadata against a
fresh temporary migrated database using the scoped commands above, with
`SQLX_OFFLINE=false` and that temporary `DATABASE_URL`. Never use a real account
workspace for query preparation.

### Desktop contact pages

`read::ReadSnapshot::contact_page` returns bounded current-working rows for a
named tag, All contacts or Untagged. It orders native values in SQLite, with
absent optional values last and stable contact-ID ties. Full UTC timestamps
remain one comparison value. A point's displayed grid participates in grid
sorting alongside recorded grid areas. `clacks-model::query` owns expression
parsing, typed values and field help; the store compiles those closed structures
to SQL with bound literals. `history` supplies the shared working-row type.

The desktop uses internal pages of 100 to supply continuous virtual tables with
no user-facing pagination. It reads the pages needed by the current viewport or
keyboard target and invalidates cached rows after known mutations or view/sort
changes. A page's `has_more` is not an account-wide count; separate page
reads are not represented as one immutable account snapshot. Save/edit/delete
continues to use the existing expected-record transaction and pending metadata.

Rows, matching totals, selected totals and selections outside the expression
share the same snapshot. Show selected changes the row restriction without
changing the expression used for those counts. Current service applicability
and whole-source location/name fallback use the existing native rules. An
operation-local temporary relation holds only required native-derived facts;
it is rolled back with its nested transaction, not retained as another cache.
Evidence-bearing candidates are processed in batches of at most 256. Queries
over derived service facts cost more than predicates over recorded values.

`read::worked_before::WorkedBeforeQuery` supplies an independent literal prefix,
view and order. Empty input returns no contacts, and the desktop admits no
empty-prefix read. `read::query_state` stores edited/applied text together in
the shared local key/value relation under `history.query`. An absent key has
empty defaults; unsupported versions are errors. These preferences do not
create pending synchronized changes. Effective station reads include the eligible
directory winner for the current key, independently from recorded-field queries.
Whole-location precedence and account-defined reference mappings remain shared
model behavior.

A completed transfer window creates ordinary actor-exclusion reviews for held
pending work, including records absent from a full account view. Certified
absence uses that completed checkpoint. Originals and prepared proposals stay
intact, and only explicit exact-basis resolution releases the authorization hold.

### Account-wide review reads

`ReadSnapshot::review_page` selects Conflicts or Pending sync and returns full
filtered counts plus a kind-qualified continuation. Persisted admission holds
and retained recovery work are blocked review items, even before comparison
values are available. `review_record` reads an exact record and its persisted
review consistently, including after the pending operation settles. These reads
use short ordinary snapshots; callers keep copied inputs while deciding.

`ReadSnapshot::review_references` returns accepted QSO/profile referrers and a
full count independently of pending local repairs. Its continuation is qualified
by the native referrer kind and ID. `synchronization::retry_deletion` checks the
exact blocked-deletion comparison and accepted references in one immediate
transaction. A successful retry rebases the existing pending deletion; normal
preparation allocates its proposal counter later. Other resolution paths also
reject a tag/equipment deletion while accepted referrers survive.

An ordinary local edit made while review is pending refreshes that comparison's
local values atomically with the working record. It preserves the conflict and
its original/shared evidence. A previously opened review still becomes stale;
the operator must explicitly refresh it before resolving the new working facts.

# Local account store

`clacks-client-store` owns one SQLite database per local account workspace.
Native values, saved roles, operating defaults and transfer windows use real SQLx transactions. The
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
retried against fresh history. Reads and writes use SQLite transactions.

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

## Checked SQL

Crate-internal types in `src/rows/` group the relational schema by record family.
Queries and mappers import those named rows/field groups directly. Shared physical
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
cargo install sqlx-cli --version 0.9.0 --locked --no-default-features --features sqlite
```

Choose a fresh temporary SQLite file, set `DATABASE_URL` to its SQLite URL, and
run from the workspace root with `SQLX_OFFLINE=false`:

```sh
cargo sqlx database create
cargo sqlx migrate run --source lib/clacks-client-store/migrations
cargo sqlx prepare --workspace -- --all-targets
cargo sqlx prepare --check --workspace -- --all-targets
```

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
comparison roles. `observe_account` stores an authenticated coordinator's supplied
account/membership view under an exact prior-value check; authentication and recovery
policy remain with that coordinator. Observed server head and membership are separate
from the client's completed installed checkpoint. No record response advances receipt.

## Transactional workflows

- `save` preserves the original basis and any already-frozen proposal while
  changing working values. Exact expected rows guard editor saves and tag changes.
  Tag deletion rechecks the complete preview before committing all effects together.
- `operating` owns current values and full loaded-profile copies. Loading is
  explicit; later synchronized profile/equipment edits do not change these copies.
- `synchronization::prepare` persists reservations and frozen values before returning
  them. `receive`, `reject` and `resolve` save actual core-produced role/review updates.
- `transfer` remembers immutable manifests, stages incomplete coupled units,
  installs complete batches and their unique external ownership together, and
  reloads the completed checkpoint after commit. A first receipt requires a full window.
  Dependency loading follows incoming, staged and local accepted versions, including
  support already stored outside the window. Without an embedded destination
  description, it supplies the stored destination to the native dependency checks.
- `retention` installs received interpretation while sealing covered pending bases.
  Insufficient coverage keeps original evidence; collection removes unreachable local
  structures while retaining immutable service facts, required support and deletion memory.
- `read::snapshot` establishes one read transaction for history, review, operating
  values, record roles and receipt/account diagnostics. Keep it only for a bounded
  operation; it can retain WAL pages until closed or dropped. History filters use
  bound values and one working payload per QSO; review includes pending deletions.

Desktop entry/import/export and WebSocket integration extend these production
operations. They supply verified enrollment, authenticated source metadata and
actual accepted envelopes. They do not replace this store with a separate repository
or run server acceptance locally. Directory snapshot persistence will be added
with its native carrier rather than a generic serialized placeholder.

SQLx's standard acquisition check discards a connection whose transaction tracking
remains active after queued rollback work drains. This covers SQLite's automatic
rollback on storage-full errors while preserving the original concrete error.

## Verification

```sh
SQLX_OFFLINE=true cargo test -p clacks-client-store --all-targets --locked --offline
SQLX_OFFLINE=true cargo clippy -p clacks-client-store --all-targets --locked --offline -- -D warnings
```

Interruption tests spawn the Rust test binary itself, terminate it at real save,
preparation and transfer boundaries, then reopen the file. No test harness binary,
Python tooling, external storage service or compiled TQSL implementation is required.
The repository workflow runs these tests natively on Windows and Linux.

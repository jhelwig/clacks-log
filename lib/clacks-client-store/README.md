# Local account store

`clacks-client-store` owns one SQLite database per local account workspace.
Opening and enrollment metadata use real SQLx transactions. The
[storage contract](../../docs/client-store.md) defines the payload and
synchronization workflows being integrated with this boundary.

`store::ClientStore::create` exclusively creates a local-only workspace;
`open` requires an existing workspace and the expected account ID. Startup
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

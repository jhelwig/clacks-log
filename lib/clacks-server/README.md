# Clacks server library

Production account persistence and operator operations for Clacks Log. PostgreSQL
is the server store; its schema is independent of the SQLite desktop store.
The account contract is in [accounts.md](../../docs/accounts.md).

## Release migrations

Use a separate schema-changing credential; ordinary server processes do not run
migrations. Keep the connection URL in a protected file:

```sh
cargo run -p clacks-admin -- database migrate --database-url-file /run/secrets/migration-url
```

SQLx owns migration history and locking. The runtime role receives explicit
application data privileges, not schema ownership. Administrator membership is
read-only to ordinary requests.

## SQL query metadata

Fixed PostgreSQL queries use external SQL files and compile-time SQLx checking.
Generate metadata against a disposable database with the production migrations
applied, from this crate's directory:

```sh
SQLX_OFFLINE=false cargo sqlx prepare -- --all-targets --locked
SQLX_OFFLINE=false cargo sqlx prepare --check -- --all-targets --locked
```

Set `DATABASE_URL` to the query-preparation PostgreSQL database. Commit this
crate's `.sqlx/` files with query/schema changes. Client SQLite metadata remains
at the workspace root; do not regenerate that directory against PostgreSQL.
Ordinary builds use `SQLX_OFFLINE=true`.

## Account scenarios

Set `CLACKS_TEST_DATABASE_URL` to a disposable PostgreSQL database. Tests apply
the production migrations and use distinct account identities. Never point this
variable at a personal or production database.

```sh
SQLX_OFFLINE=true cargo test -p clacks-server --lib --locked
```

The scenario checks Clacks account admission, concurrent identity resolution and
login invalidation. It does not test SQLx migration validation, PostgreSQL
constraint semantics or cryptographic dependencies independently.

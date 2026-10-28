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

## Provider deployment setup

The [configuration example](../../deploy/server/config.example.toml) shows a
provider selected by a local key. The exact verified issuer/subject pair remains
the external identity; local labels and email do not establish ownership.

```sh
cargo run -p clacks-admin -- oidc register --config /etc/clacks/server.toml --provider primary
```

This subcommand does not read runtime database/cookie credentials or open a
PostgreSQL connection. Repeated setup reuses a matching saved registration.
Keep the registration file protected and distribute the same read-only file to
all application instances. An issuer/callback mismatch or expired client secret
requires operator action; do not automatically register again.

If provider registration completed but publication failed, the command preserves
the returned credentials in a protected temporary file and reports its path.
Inspect that result and provider state before retrying. Back up registration and
cookie secrets separately from PostgreSQL through deployment secret management.

## Browser runtime

```sh
cargo run -p clacks-server-app -- --config /etc/clacks/server.toml
```

Apply the deployment role/grant SQL as the database administrator/schema owner;
use a distinct runtime URL. The server attaches the session adapter to the
already migrated table without schema initialization. Public HTTPS terminates
at deployment ingress; the application listener is internal.

`CLACKS_TEST_RUNTIME_DATABASE_URL` supplies the restricted runtime role for HTTP
scenarios, separately from the migration-capable test database URL. Run all
server scenarios using `cargo test -p clacks-server --all-targets --locked`.
The narrow provider fixture signs independent ephemeral identity tokens in Rust;
no TQSL sources, external fixture generator or real account credentials are used.

## Roles and exceptional recovery

Role operations require a maintenance connection with administrator-membership
write grants, not migration privileges or provider/cookie credentials:

```sh
cargo run -p clacks-admin -- role grant --account ACCOUNT_ID --database-url-file /run/secrets/maintenance-url
cargo run -p clacks-admin -- role revoke --account ACCOUNT_ID --database-url-file /run/secrets/maintenance-url
```

For loss of every accessible login identity, verify the owner and replacement
issuer/subject out of band. Ensure the provider is configured for normal sign-in.
Do this before the replacement identity signs in and provisions another account:

```sh
cargo run -p clacks-admin -- identity recover --account ACCOUNT_ID --issuer https://identity.example --subject VERIFIED_SUBJECT --database-url-file /run/secrets/maintenance-url
```

Recovery adds the binding without removing existing identities, browser logins
or desktop clients. An identity owned elsewhere is refused, never transferred.
Suspected compromise needs separate explicit revocation, not implicit changes
hidden inside recovery. There is no automatic email linking or account merge.

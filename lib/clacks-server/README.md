# Clacks server library

Production account persistence, OpenID Connect (OIDC) and operator operations for
Clacks Log. PostgreSQL
is the server store; its schema is independent of the SQLite desktop store.
The account contract is in [accounts.md](../../docs/accounts.md).

For a running local instance, follow the
[complete development workflow](../../docs/server-development.md). The sections
below describe individual operations and test setup.

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
Install the pinned CLI with both workspace backends and Rustls:

```sh
cargo install sqlx-cli --version 0.9.0 --locked --no-default-features --features sqlite,postgres,rustls
```

Generate metadata against a disposable database with the production migrations
applied, from this crate's directory:

```sh
SQLX_OFFLINE=false cargo sqlx prepare -- --all-targets --locked
SQLX_OFFLINE=false cargo sqlx prepare --check -- --all-targets --locked
```

Set `DATABASE_URL` to the same disposable PostgreSQL database used for tests,
with the migration role. Commit this crate's `.sqlx/` files with query/schema
changes. Client SQLite metadata remains
at the workspace root; do not regenerate that directory against PostgreSQL.
Ordinary builds use `SQLX_OFFLINE=true`.
Run PostgreSQL and SQLite preparation serially when sharing a Cargo target
directory: SQLx uses a shared `sqlx-tmp` scratch directory during preparation.

## Account scenarios

Set `CLACKS_TEST_DATABASE_URL` to a disposable PostgreSQL database. Tests apply
the production migrations and use distinct account identities. Never point this
variable at a personal or production database.

```sh
SQLX_OFFLINE=true cargo test -p clacks-server --lib --locked
```

The scenarios check Clacks account admission, concurrent identity resolution,
login invalidation, desktop membership activation/recovery and shared replay
rejection. Set `CLACKS_TEST_RUNTIME_DATABASE_URL` to the restricted runtime role
on that same disposable database for HTTP and native-client scenarios.
They do not test SQLx migration validation, PostgreSQL
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
New registrations provide `client_name = "Clacks Log"` through standard OIDC
client metadata. Reuse does not rename an existing provider client; change an
existing display name through the provider's administration interface.
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
store scenarios using `cargo test -p clacks-server --lib --locked`.
Browser/provider scenarios run in `clacks-web` through production Leptos actions
and the provider callback; see the [web guide](../clacks-web/README.md).
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

## Disposable local PostgreSQL

One disposable PostgreSQL database serves both metadata preparation and account
scenarios. Its trust authentication is exclusively for synthetic local tests:

```sh
docker run --rm --name clacks-tests --tmpfs /var/lib/postgresql -e POSTGRES_HOST_AUTH_METHOD=trust -p 127.0.0.1:5432:5432 postgres@sha256:b994732fcf33f73776c65d3a5bf1f80c00120ba5007e8ab90307b1a743c1fc16
```

In another terminal, create roles and the database, then apply release
migrations once:

```sh
docker exec -i clacks-tests psql -U postgres -v ON_ERROR_STOP=1 < deploy/server/database-roles.sql
docker exec clacks-tests createdb -U postgres -O clacks_migrator clacks
```

Write the `postgres://clacks_migrator@127.0.0.1:5432/clacks`
URL to a temporary file and run `clacks-admin database migrate` with that file.
Apply `deploy/server/database-grants.sql` as the schema owner to that database.
Set these test inputs (adjust the port if already occupied):

```sh
export DATABASE_URL=postgres://clacks_migrator@127.0.0.1:5432/clacks
export CLACKS_TEST_DATABASE_URL=postgres://clacks_migrator@127.0.0.1:5432/clacks
export CLACKS_TEST_RUNTIME_DATABASE_URL=postgres://clacks_runtime@127.0.0.1:5432/clacks
export LEPTOS_OUTPUT_NAME=clacks-web
```

The two environment variables are deliberately different: store/maintenance
scenarios exercise their production operations, and HTTP/web scenarios use the
restricted runtime role. Unit scenarios may invoke the normal SQLx migrator on
the same database, but no scenario creates a new database per assertion or
injects schema behavior. Stop only this service with `docker stop clacks-tests`
and remove temporary synthetic credential files when done.

For the image, ingress, health, shutdown and backup procedures, see
[Linux deployment](../../deploy/server/README.md). The server currently provides
account management, not the later sync/upload endpoints.

Run the client and server metadata checks sequentially. Their committed caches
are separate, but SQLx uses one `target/sqlx-prepare-check` scratch directory;
concurrent preparations against that directory interfere with each other.

## Desktop authorization

The embedded oauth-as issuer serves authorization-code and persistent refresh
flows, with opaque access tokens and Demonstrating Proof of Possession (DPoP)
binding on both credentials. Browser approval chooses the authenticated account;
`/api/client` returns the currently authorized account/actor directly from
PostgreSQL. This does not implement the WebSocket synchronization transport.

A deployment wrapping key encrypts each account's independently generated
encryption key before that account key is stored in PostgreSQL. The account key
currently encrypts token references retained briefly so the server can revoke
issued credentials when an authorization code is replayed. The wrapping key
stays in a deployment secret file, outside the database.

Generate a deployment wrapping key in a protected, existing directory:

```sh
cargo run -p clacks-admin -- keys generate --output /run/secrets/account-wrapping-key
```

Configure `[wrapping_keys]` and `[wrapping_keys.files]` as in the deployment
example. Files contain exactly 32 raw bytes, with no encoding. The generation
command refuses replacement; protect directory access on all platforms. Unix
key files are created with owner-only access. All application instances need
the same retained keys. Missing keys are errors, never a reason to regenerate.
Independent account keys protect short-lived consumed-code reference receipts;
ordinary credential lookup values are one-way hashes. This key hierarchy is
separate from browser cookies and OIDC provider credentials.

The runtime reclaims expired issuer/proof records in bounded passes every minute.
Active client memberships and persistent refresh grants have no idle expiry.
Shutdown stops the cleanup task before closing the shared database pool.
Committed authorization/revocation events identify account and actor. Token
issuance counters distinguish library grant types; expected refusals are debug
and code replay is a warning, with containment failures reported as errors.
Credentials, proofs, key material and callback query strings are excluded.
Restore PostgreSQL and retained wrapping keys together; see
[backup and recovery](../../docs/backup-recovery.md#desktop-authorization-recovery).

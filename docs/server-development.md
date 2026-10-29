# Server development

Run PostgreSQL in Docker and the server/web application through cargo-leptos on
macOS or Linux. A real OpenID Connect (OIDC) provider supplies interactive login.
The server currently supports account management; synchronization and external
logbook-service networking are later capabilities.

Commands run from a repository root. Choose a private directory for configuration
and secrets, and set `CLACKS_DEV_DIR` to its absolute path. Keep secret files out of
Git. The example uses PostgreSQL port 55432, application port 3000, container
`clacks-dev` and volume `clacks-dev-postgres`; change those example values if they
conflict with other local services. The application does not choose this directory
or prescribe how developers organize their environments.
Replace `/absolute/path/to/your/development-environment` in the commands below
with the directory you choose.

## Prerequisites

Install Docker with its daemon running, Rustup, a shell, `curl`, `od`, and `head`.
The repository's `rust-toolchain.toml` selects Rust 1.99.1. PostgreSQL commands
below run inside its container; no host PostgreSQL installation is needed.

```sh
rustup show active-toolchain
rustup target add wasm32-unknown-unknown
cargo install cargo-leptos --version 0.3.11 --locked
cargo fetch --locked
```

Arrange access to an OIDC provider before testing sign-in. Its issuer is the
exact URL advertised in discovery, which can include a path. For the local HTTP
configuration below, the provider must accept this exact callback:

```text
http://127.0.0.1:3000/auth/callback/primary
```

Do not interchange `localhost` and `127.0.0.1`; callback registration and browser
origin checks use the configured URL. If the provider requires HTTPS, expose the
local application through deployment-managed HTTPS ingress and use that public
origin/callback instead. The application currently requires hosting at the
origin root. See [account configuration](account-management-proposal.md).

## One-time environment setup

Run this section only when creating a new development environment. Replacing
password/key files does not update existing PostgreSQL passwords and replacing
the cookie key signs browsers out.

### 1. Create protected secret files

```sh
export CLACKS_DEV_DIR="/absolute/path/to/your/development-environment"
umask 077
mkdir -p "$CLACKS_DEV_DIR/secrets"

for credential in postgres migrator runtime maintenance; do
  od -An -N32 -tx1 /dev/urandom | tr -d ' \n' \
    > "$CLACKS_DEV_DIR/secrets/$credential-password"
done
head -c 64 /dev/urandom > "$CLACKS_DEV_DIR/secrets/browser-cookie-key"

printf 'postgres://clacks_migrator:%s@127.0.0.1:55432/clacks\n' \
  "$(cat "$CLACKS_DEV_DIR/secrets/migrator-password")" \
  > "$CLACKS_DEV_DIR/secrets/migration-database-url"
printf 'postgres://clacks_runtime:%s@127.0.0.1:55432/clacks\n' \
  "$(cat "$CLACKS_DEV_DIR/secrets/runtime-password")" \
  > "$CLACKS_DEV_DIR/secrets/runtime-database-url"
printf 'postgres://clacks_maintenance:%s@127.0.0.1:55432/clacks\n' \
  "$(cat "$CLACKS_DEV_DIR/secrets/maintenance-password")" \
  > "$CLACKS_DEV_DIR/secrets/maintenance-database-url"
```

Each database URL file contains one plain-text connection URL, with no TOML,
quotes or variable references. Its format is:

```text
postgres://ROLE:PASSWORD@HOST:PORT/DATABASE
```

For example, with the password `example-runtime-password`, the runtime file would
contain `postgres://clacks_runtime:example-runtime-password@127.0.0.1:55432/clacks`.
The commands generate hexadecimal passwords, so URL escaping is unnecessary. If
you choose other passwords, percent-encode reserved URL characters. A trailing
newline in a URL file is accepted. The cookie-key file instead contains 64 raw
random bytes, not hexadecimal/Base64 text. Keep these files between restarts.

### 2. Start persistent development PostgreSQL

```sh
docker volume create clacks-dev-postgres
docker run --detach --name clacks-dev \
  --mount type=volume,src=clacks-dev-postgres,dst=/var/lib/postgresql/data \
  --mount "type=bind,src=$CLACKS_DEV_DIR/secrets/postgres-password,dst=/run/secrets/postgres-password,readonly" \
  --env POSTGRES_PASSWORD_FILE=/run/secrets/postgres-password \
  --env POSTGRES_DB=clacks \
  --publish 127.0.0.1:55432:5432 \
  postgres@sha256:b994732fcf33f73776c65d3a5bf1f80c00120ba5007e8ab90307b1a743c1fc16
```

Wait until this succeeds before continuing; repeat it while PostgreSQL starts:

```sh
docker exec clacks-dev pg_isready -U postgres -d clacks
```

The named volume retains development accounts across container/server restarts.
This container is distinct from the deployment Compose example: the Rust server
runs on the host, so its URL uses `127.0.0.1:55432`, not `postgres:5432`.

### 3. Create roles, migrate and grant runtime access

Create the three application roles and set their passwords as PostgreSQL's
administrator. The generated hexadecimal values can be inserted into these SQL
string literals without additional escaping. Passwords are passed on standard
input rather than command-line arguments.

```sh
{
  cat deploy/server/database-roles.sql
  printf "ALTER ROLE clacks_migrator PASSWORD '%s';\n" \
    "$(cat "$CLACKS_DEV_DIR/secrets/migrator-password")"
  printf "ALTER ROLE clacks_runtime PASSWORD '%s';\n" \
    "$(cat "$CLACKS_DEV_DIR/secrets/runtime-password")"
  printf "ALTER ROLE clacks_maintenance PASSWORD '%s';\n" \
    "$(cat "$CLACKS_DEV_DIR/secrets/maintenance-password")"
  printf 'ALTER DATABASE clacks OWNER TO clacks_migrator;\n'
} | docker exec -i clacks-dev psql -U postgres -d clacks -v ON_ERROR_STOP=1

SQLX_OFFLINE=true cargo run -p clacks-admin --locked -- \
  database migrate \
  --database-url-file "$CLACKS_DEV_DIR/secrets/migration-database-url"

docker exec -i clacks-dev psql -U clacks_migrator -d clacks \
  -v ON_ERROR_STOP=1 < deploy/server/database-grants.sql
```

The migration command connects over the published TCP port with the migrator
password. The ordinary server uses the runtime role, which cannot create schema
objects or assign administrator roles. The maintenance role is for explicit role
assignment/recovery. PostgreSQL setup grants are in
[database-grants.sql](../deploy/server/database-grants.sql).

### 4. Write application configuration

Create the local configuration with absolute paths expanded by the shell:

```sh
cat > "$CLACKS_DEV_DIR/server.toml" <<EOF_CONFIG
public_url = "http://127.0.0.1:3000"
listen = "127.0.0.1:3000"
allow_http = true
database_url_file = "$CLACKS_DEV_DIR/secrets/runtime-database-url"
cookie_key_file = "$CLACKS_DEV_DIR/secrets/browser-cookie-key"
otlp = false

[providers.primary]
label = "OIDC Provider Name"
issuer = "https://identity.example"
mode = "dynamic"
registration_file = "$CLACKS_DEV_DIR/secrets/oidc-primary.json"
EOF_CONFIG
```

Before registration/startup, edit `issuer` to your actual provider's discovery
issuer and `label` to its display name. `https://identity.example` is a placeholder,
not a running provider. TOML paths do not expand `$HOME` or `~`; the shell expands
the absolute paths in the example above. `allow_http` permits local HTTP and
non-Secure cookies; use HTTPS with this setting false for a deployed instance.
The fourteen-day sliding session duration and other tuning values can be omitted
because typed defaults supply them. Export is off, so structured logs go to stderr
without requiring a telemetry collector.

### 5. Configure the provider client

Choose one of these alternatives. Clacks uses the standard authorization-code
flow with Proof Key for Code Exchange (PKCE). Identity linking/removal additionally
requests `max_age=300` and requires the provider's corresponding `auth_time` claim.

**Dynamic client registration:** keep `mode = "dynamic"`. The provider must expose
a registration endpoint and permit this deployment to register. If it requires
an initial access token, save that provider-issued token as plain text in
`$CLACKS_DEV_DIR/secrets/oidc-initial-access-token` and add its absolute path as
`initial_access_token_file` inside `[providers.primary]`. Otherwise omit the field.
Do not substitute an ID token, authorization code or user password.

```sh
SQLX_OFFLINE=true cargo run -p clacks-admin --locked -- \
  oidc register --config "$CLACKS_DEV_DIR/server.toml" --provider primary
```

`Created` means the returned registration was saved. A repeat command reports
`Reused` for the same issuer/callback without another registration request.
New requests set the provider-facing client name to `Clacks Log`; the local
provider `label` instead names the sign-in choice displayed by Clacks. Reuse
does not update an existing registration's name. Use the provider's normal
client-editing interface for a name correction, preserving its credentials.
`oidc-primary.json` is generated by Clacks and contains provider credentials;
do not create it by copying an arbitrary client secret into a JSON file. An
issuer/callback mismatch, expired secret or uncertain registration outcome
requires inspecting the existing registration/provider before retrying. This
command does not require database or cookie credentials.

**Preconfigured provider client:** register a web application through the provider's
normal administration workflow, allowing the exact callback given above and code
flow with PKCE. Save its issued client secret as a plain-text file if applicable.
Replace the provider block with the following, filling in actual values/paths:

```toml
[providers.primary]
label = "OIDC Provider Name"
issuer = "https://identity.example"
mode = "preconfigured"
client_id = "PROVIDER-ISSUED-CLIENT-ID"
client_secret_file = "/absolute/path/to/client-secret"
# Set only when required by this provider's registration:
# token_endpoint_auth_method = "client_secret_post"
```

Remove `registration_file` and `initial_access_token_file` for this mode. The
secret file contains only the secret, not `client_secret=...` or JSON. With a
secret, authentication defaults to `client_secret_basic`; explicitly select
`client_secret_post` if required. For a public client, omit the secret file and
authentication defaults to `none`. Unsupported methods such as `private_key_jwt`
are refused; there is no silent fallback or automatic re-registration.

## Run and iterate

In each terminal, set `CLACKS_DEV_DIR` to the directory configured above and run
from the repository root:

```sh
export CLACKS_DEV_DIR="/absolute/path/to/your/development-environment"
docker start clacks-dev
SQLX_OFFLINE=true cargo leptos watch -- \
  --config "$CLACKS_DEV_DIR/server.toml"
```

Cargo-leptos builds the server, hydrated browser WebAssembly and stylesheet,
starts the binary and rebuilds/restarts it when source files change. Its output
is under the checkout's `target/` by default. Use stable-toolchain watch mode;
the optional `--hot-reload` flag requires nightly and is not needed. The normal server starts
without migration or registration side effects.

Restart watch after changing the external configuration; it is outside the source
watch tree. Server `CLACKS__...` environment overrides take precedence over the
configuration file, so remove unintended overrides when selecting an environment.

Open `http://127.0.0.1:3000/sign-in`. Select the configured provider and sign in.
An unbound verified identity provisions an independent Clacks account. The account
page shows its internal account ID and linked identities. It currently supports
linking/unlinking and sign-out. Tests using a synthetic provider do not establish
interoperability with a specific tsidp deployment/version.

In another terminal, check application readiness and a generated asset:

```sh
curl --fail --silent --show-error http://127.0.0.1:3000/health/ready
curl --fail --silent --show-error http://127.0.0.1:3000/pkg/clacks-web.css > /dev/null
```

An empty successful readiness response is expected; inspect the HTTP status with
`curl --include` if needed. A plain `cargo run -p clacks-server-app` does not build
browser assets, so use watch mode for normal development. For a build without a
watch process, run `SQLX_OFFLINE=true cargo leptos build`, then:

```sh
SQLX_OFFLINE=true LEPTOS_OUTPUT_NAME=clacks-web LEPTOS_SITE_ROOT=target/site \
  cargo run -p clacks-server-app --locked -- \
  --config "$CLACKS_DEV_DIR/server.toml"
```

To grant administrator membership, copy the account ID from the account page:

```sh
SQLX_OFFLINE=true cargo run -p clacks-admin --locked -- \
  role grant --account ACCOUNT_ID \
  --database-url-file "$CLACKS_DEV_DIR/secrets/maintenance-database-url"
```

Use `role revoke` with the same arguments to remove that role. Administrator
membership is not required for normal sign-in. Identity recovery and its
out-of-band ownership verification are described in the
[server guide](../lib/clacks-server/README.md).

After schema changes, stop the development server, run the explicit migration
command again from the branch whose schema you intend to use, apply updated
schema-owner grants, and restart. Regenerate SQLx metadata only for query/schema
changes, using the [server metadata procedure](../lib/clacks-server/README.md#sql-query-metadata).
`SQLX_OFFLINE=true` uses committed metadata during ordinary builds; it does not
prevent the running application from connecting to PostgreSQL.

Ctrl-C stops watch and the server. `docker stop clacks-dev` stops PostgreSQL while
retaining its volume. Resume later with `docker start clacks-dev`; do not repeat
one-time password/key creation or provider registration for an ordinary restart.

## Tests and isolation

Use a disposable test database, not development accounts, for the account/store
and provider tests. Follow the [test database setup](../lib/clacks-server/README.md#disposable-local-postgresql)
and [web test command](../lib/clacks-web/README.md). Metadata checking can share
that disposable database; it does not need another database of its own. The
persistent development instance described here is a different purpose/lifetime.

## Multiple instances and schema compatibility

`--config` accepts an absolute configuration path, so configuration can be used
from different checkouts without being copied into them. Each developer chooses
whether to share or isolate local environments; neither arrangement is required
by Clacks. Concurrent instances need compatible listener/asset settings and
non-conflicting ports. Saved dynamic registrations match an exact issuer and
callback; a different public callback URL requires explicit provider setup rather
than silently reusing a mismatched registration.

Source isolation does not isolate a database. Migrations are forward-only, and
switching branches does not undo them or make an older binary compatible with a
newer schema. Check compatibility before using an existing database and coordinate
migrations for any database shared by multiple processes. Use a separate database
when a branch needs an incompatible schema; do not edit migration history or run
down migrations to make it fit. Keep test data independent of development accounts.

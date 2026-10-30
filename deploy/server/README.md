# Linux server deployment

For a host-run development server with PostgreSQL in Docker, use the
[development workflow](../../docs/server-development.md). This guide covers the
packaged Linux deployment instead.

The release image contains one Axum/Leptos application, `clacks-admin` and its
hydrated browser assets. PostgreSQL is the only database service. HTTPS belongs
to deployment ingress; forward the application at the origin root, preserving
browser Origin/Referer headers. Origin-root hosting is a current routing/assets
limitation, not an agreed requirement forbidding future path-prefix support. Do not expose its HTTP listener directly to the
Internet. Browser callbacks use the configured public origin, never incoming
Host or forwarded headers.

## Release and initial setup

Build from the repository root:

```sh
docker build -f deploy/server/Dockerfile -t clacks-log:local .
```

The builder and runtime base images are pinned by digest. Rustup reads
`rust-toolchain.toml`; the Dockerfile does not duplicate the compiler version.
Cargo-leptos 0.3.11 builds server rendering and Rust WebAssembly separately.
The runtime uses numeric nonroot user/group IDs 65532 and serves read-only assets.

The Compose file illustrates self-hosted deployment. A production platform may
supply managed PostgreSQL, secrets and ingress instead. Copy the configuration
example to `config.toml`, set its public HTTPS origin/provider, and provision the
referenced secret files outside Git. The cookie key is at least 64 random raw
bytes, stable across releases/instances. Database URL files contain the proper
role's URL, including its deployment-managed password. Make mounted files
readable by user ID 65532 without making them world-readable on shared hosts.

Bring up PostgreSQL alone. As its administrator, run `database-roles.sql`, assign
each role its deployment-managed password, and make `clacks_migrator` the owner
of the Clacks database. For the Compose database, the ownership command is:

```sql
ALTER DATABASE clacks OWNER TO clacks_migrator;
```

Migrate explicitly as a release step, before starting new application instances.
Run the image with only a mounted migration URL file:

```sh
docker run --rm --network clacks_default --mount type=bind,src=/absolute/secrets/migration-url,dst=/run/secrets/database-url,readonly clacks-log:local clacks-admin database migrate --database-url-file /run/secrets/database-url
```

Then apply `database-grants.sql` as the schema owner. Runtime gets no schema
creation rights and only reads administrator membership; the maintenance role
can grant/revoke roles and add recovery bindings. Migration privileges are never
used by normal startup. Rollout automation serializes migrations and performs
compatible application/database rollouts; do not run migrations on every replica.

For dynamic registration, run `clacks-admin oidc register --config ... --provider
primary` as an explicit setup operation. Mount the configuration, optional initial
access token and a writable protected directory for `registration_file`; no
database URL or browser-cookie key is required. Publish the saved registration
to application instances as a read-only secret. Repeat setup reuses a matching
file without another registration request. Preconfigured client credentials need
no dynamic registration command. Remove the optional initial-access-token path
from runtime configuration after setup; it is not a runtime credential.

Start the application only after its registration/configuration is ready. The
Compose example mounts runtime credentials only. For preconfigured mode, replace
the registration-file volume with the configured client-secret file if needed.
Maintenance commands can use the same image with only their own URL file mounted.
For preconfigured clients, `token_endpoint_auth_method` supports
`client_secret_basic`, `client_secret_post` and `none`. The default is Basic
when a secret is configured, or `none` without one. Dynamic clients honor the
saved response's method; omitted metadata uses the protocol's Basic default.
Unsupported methods or inconsistent secret requirements refuse runtime startup,
preserving the registration for operator action rather than registering again.
See [the server guide](../../lib/clacks-server/README.md) for all command groups
and recovery safeguards. `clacks-admin --help` and nested `--help` need no secrets.

## Runtime operations

`/health/live` reports process liveness. `/health/ready` executes an account-schema
read through the actual runtime role, so an absent schema or unusable database
is not ready. Provider outages fail new authentication, while existing logins
continue to be checked against PostgreSQL. Configure ingress/platform HTTP health
probes; the runtime does not install a second probe executable.

SIGTERM/Ctrl-C stops admission and drains requests for `shutdown_seconds` (30 by
default). Telemetry then has a five-second bound for each enabled signal. Allow
at least 50 seconds before force-killing the container. Database transactions
remain ordinary PostgreSQL transactions; a forced stop is not a successful
request or an implicit migration/registration retry.
Optional export failures produce a local warning and preserve the server's
original exit result, including a successful graceful shutdown.

Structured JSON operator logs go to stderr. `otlp = true` enables OpenTelemetry
Protocol (OTLP) HTTP/protobuf traces, metrics and logs over Rustls. Configure
endpoints, headers and timeouts with standard `OTEL_EXPORTER_OTLP_*` variables;
keep authorization headers in deployment secrets. Library batching/queues and the
metrics bridge are standard libraries. The HTTP request span extracts standard
Trace Context headers without copying arbitrary baggage into logs. Request metrics
use bounded status classes, never account IDs or callback URLs. Account/identity
changes are logged after commit. User-facing security-event history is deferred.

## Backup and recovery

Back up PostgreSQL consistently with established PostgreSQL tooling and keep
provider registrations, cookie keys and deployment configuration in protected
secret/configuration backups. Recovery must restore the appropriate set together.
Never register another provider client merely because a local file is missing.
If registration has an uncertain remote outcome, inspect provider state first.
If local publication failed after success, the command reports a retained
protected temporary credential file to recover deliberately.

Replacing the cookie key signs browsers out; it does not delete identities or
accounts. Restoring an old PostgreSQL backup can restore old login/role state
and lose later revocations. There is no claim of rollback-proof authorization.
After verifying the intended owner out of band, use `identity recover` before
that replacement identity signs in and creates a different account. Recovery
adds a binding, preserving existing identities and logins; separate compromise
response must explicitly revoke affected access. Never transfer an already-owned
identity or infer ownership from email. See [backup/recovery](../../docs/backup-recovery.md)
for the separate future synchronization/publication restore procedures.

Desktop enrollment also requires the versioned `[wrapping_keys]` configuration.
Use `clacks-admin keys generate --output PATH` in a protected directory to create
an exact 32-byte raw key. Mount it read-only at the configured path, alongside
but separate from browser-cookie/provider secrets. Share retained versions across
all instances and back them up with the database. New active versions do not
rewrap existing account keys; retain any key still referenced by an envelope.
The [server guide](../../lib/clacks-server/README.md#desktop-authorization) covers
issuer operations and the [recovery contract](../../docs/backup-recovery.md)
explains rollback and native desktop credential recovery.

## Encryption-key rotation

Use the [operator procedure](../../lib/clacks-server/README.md#credential-storage-and-key-maintenance)
for account-key rotation and deployment-key rewrapping. Install updated grants
after release migrations. Deploy the account-locked writer and drain older
instances before any rotation; distribute new readable wrapping keys before
activating them and drain old-key writers before completing rewrap. Supported
application rollback targets must retain the writer boundary and live formats.
Maintenance requires a direct or session-pooled connection, not transaction
pooling. Keep every key required by retained backups even after current source
references reach zero.

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
The image includes the release's database role and grant SQL under
`/usr/share/clacks/database/`; migrations remain embedded in `clacks-admin`.

For Dokku source deployment, select both repository-relative metadata paths:

```sh
dokku builder-dockerfile:set clacks-log dockerfile-path deploy/server/Dockerfile
dokku app-json:set clacks-log appjson-path deploy/server/app.json
```

The repository root remains the build context. The native startup check requests
`/health/ready` on port 3000; it does not run migrations or require a probe
executable inside the application image. Keep deployment checks enabled and run
migrations with separate release credentials before candidate activation.

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
use exact response codes and matched route templates, never account IDs or
callback URLs. Account/identity changes are logged after commit. User-facing security-event history is deferred.

## Operator telemetry

### Export configuration, identity and filtering

Enable export in the existing server configuration with `otlp = true`. For an
already provisioned collector reachable from the server, a portable environment
example is:

```sh
export OTEL_EXPORTER_OTLP_ENDPOINT='https://collector.example.com:4318'
export OTEL_EXPORTER_OTLP_TIMEOUT=10000
export OTEL_RESOURCE_ATTRIBUTES='deployment.environment.name=production'
export RUST_LOG=info
```

Adjust the example values for your deployment. The shared endpoint is a base URL: the HTTP exporters append
`/v1/traces`, `/v1/metrics` and `/v1/logs`. Signal-specific
`OTEL_EXPORTER_OTLP_TRACES_ENDPOINT`, `OTEL_EXPORTER_OTLP_METRICS_ENDPOINT` and
`OTEL_EXPORTER_OTLP_LOGS_ENDPOINT` override it and must include the complete
signal path. Timeout values are milliseconds; signal-specific timeout/header
settings override the shared settings. Clacks selects HTTP/protobuf in code;
`OTEL_EXPORTER_OTLP_PROTOCOL` does not switch this executable to another transport.

If the collector requires authentication, provision
`OTEL_EXPORTER_OTLP_HEADERS='Authorization=Bearer%20REPLACE_WITH_COLLECTOR_TOKEN'`
through protected deployment secrets; never copy a real token into documentation
or shell history. Header values are URL-decoded. Leave the setting absent for a
trusted private receiver that does not require it. This does not expose a new
public collector listener or configure backend storage.

The executable sets `service.name=clacks-server` and `service.version` to its
package version after loading resource environment attributes; these values take
precedence over environment attempts to rename them. Add deployment attributes
through `OTEL_RESOURCE_ATTRIBUTES`. An optional `service.instance.id` can identify
an operator-selected server instance; it is absent unless configured. A package
version alone does not identify a source revision: use the deployment's running
image/revision record when investigating a particular release.

The default `RUST_LOG` filter is `info`. Significant account/client events remain
visible; routine `oauth.grant.refused`, `oauth.proof.refused` and
`oauth.client.refused` events are `debug`. For a deliberate issuer investigation,
`RUST_LOG=info,clacks_server::oauth::events=debug` selects those diagnostics.
Apply filter changes through your deployment configuration. Dependency
caps keep Reqwest, Hyper, HTTP/2, SQLx and OpenTelemetry diagnostics at warning or
higher even with more verbose directives; session internals are disabled.
Exporter diagnostics are excluded from the exported log bridge to avoid loops.
Trace sampling (`OTEL_TRACES_SAMPLER` and, where applicable,
`OTEL_TRACES_SAMPLER_ARG`) does not suppress required security logs or metrics.
A missing sampled trace does not invalidate a retained security event.

Configure collection, storage, access and retention using the selected backend's
supported deployment facilities. Local standard-error logs remain available when
export fails. Exporter/collector buffering may lose telemetry; operator telemetry
is distinct from durable synchronization evidence and account-user history.
Shutdown bounds and the local incomplete-export warning are described in
[runtime operations](#runtime-operations).

### Request failures and latency

Request spans follow OpenTelemetry HTTP semantic conventions. Duration covers
handling through response headers, excluding streaming body delivery and upgraded
socket lifetime. Exact response status and matched routes are available; a
headers-produced log does not prove body delivery. Paths, benign query values
and user agents remain in traces. The designated OAuth query values are redacted
by the released middleware. Percent-encoded parameter names are an explicit
upstream limitation; no blanket span-attribute scrubber is installed.

With a Prometheus backend, the standard HTTP duration instrument commonly maps
to `http_server_request_duration_seconds_count` and
`http_server_request_duration_seconds_bucket`, with `http_route`,
`http_request_method` and `http_response_status_code` labels. The following
Prometheus query language (PromQL) examples assume that translation and
`job="clacks-server"`. Verify metric/label translation in your collector and add
selectors for the deployment or instances you intend to inspect. Resource
attributes are not all automatically indexed as metric labels.

Request rate by route and exact response code:

```promql
sum by (http_route, http_response_status_code) (
  rate(http_server_request_duration_seconds_count{job="clacks-server"}[5m])
)
```

Server-failure rate by route:

```promql
sum by (http_route) (
  rate(http_server_request_duration_seconds_count{job="clacks-server",http_response_status_code=~"5.."}[5m])
)
```

Replace `5..` with `4..` to inspect client refusals separately. Expected 4xx
responses do not automatically set error span status; 5xx responses do. Unknown
routes omit `http.route`; never substitute raw paths as metric dimensions.

Route-level p95 handling duration:

```promql
histogram_quantile(0.95,
  sum by (le, http_route) (
    rate(http_server_request_duration_seconds_bucket{job="clacks-server"}[5m])
  )
)
```

The default metric export interval is 60 seconds (`OTEL_METRIC_EXPORT_INTERVAL`
is in milliseconds). Use a rate window spanning several exports. A short window
or a never-observed status may return no series. Empty data is not a zero failure
rate. Percentiles can be absent or uninformative at low volume. Counters reset on
process restart; `rate`/`increase` handle observed resets but cannot recover
activity before the first sample or promise exact per-scenario counts under
concurrent traffic. Use application request metrics for totals rather than
sampled traces or metrics derived from sampled traces.

### Authentication outcomes and correlation

With the same example Prometheus translation, inspect recent counter changes:

```promql
increase(clacks_client_authorizations_total{job="clacks-server"}[15m])
```

```promql
increase(clacks_client_revocations_total{job="clacks-server"}[15m])
```

```promql
sum by (grant_type) (
  rate(clacks_oauth_tokens_issued_total{job="clacks-server"}[5m])
)
```

Authorizations count successful activation; revocations count changed committed
client membership; issued tokens are grouped by grant type. None counts unique
users or successful provider sign-ins. A missing authorization/token series can
mean there was no observation in that process/window, not that enrollment or
sign-in failed. Add deployment selectors appropriate to your backend when several
instances or environments share a service name.

For an individual action, query the existing structured log attributes in your
logging backend. Select `service.name=clacks-server` and the intended deployment
resource identity, then filter the appropriate outcome:

| Outcome | Log field | Interpretation |
| --- | --- | --- |
| Provider sign-in | Body equals `Browser login established`; account/identity attributes | A browser login was committed for that account |
| Client authorization | `event=client.authorization.activated`; account/actor attributes | Client authorization was activated |
| Client revocation | `event=client.revoked`; account/actor attributes | Changed client membership was committed |
| Token issuance | `event=oauth.token.issued`; grant type, account and grant-family attributes | The issuer issued a token; not a count of provider sign-ins |
| Token revocation | `event=oauth.token.revoked`; token type; `cascade_failed=true` on cascade failure | Issuer token operation; inspect cascade outcome, not a substitute for the membership event |

A callback redirect alone does not establish successful login. The login success
record currently uses a fixed body rather than an `event` attribute; match the
body exactly instead of inferring success from arbitrary message text. Account,
identity, actor and grant-family IDs are operational correlation context, never
metric dimensions. A zero-result query requires checking the time window,
collection, retention and filtering before interpreting the workflow outcome.

Use the log record's standard trace/span context to retrieve the trace in your
backend. Match the span ID to the relevant operation and inspect its enclosing
HTTP span for matched route and response status. The `account.login.admit` span
belongs to `account.login.callback`; client revocation uses
`account.client.revoke`. Backend field names, indexed labels and log-to-trace
links depend on collector/storage configuration, rather than the Clacks wire
protocol or a fixed log-query schema. Configure and document those mappings for
your installation. Log and trace retention can differ; a log may outlive its
linked trace, and sampling may leave a security log without a stored trace.

Do not include provider codes, cookies, access tokens, proof headers or private
keys in investigation records. Operator telemetry does not replace authoritative
application evidence or provide an account-user security-history store.

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

## Automatic production deployment

`.github/workflows/server-deploy.yml` deploys the existing Dokku app `clacks-log`
after the `Rust` workflow succeeds for a push to this repository's `main` branch.
Main pushes always run both verification jobs, including documentation-only
updates; pull-request verification retains its existing path filters. Desktop
tag packaging is independent of server deployment.

Deployment uses the existing repository-scoped runner `clacks-ci-linux` with
labels `[self-hosted, Linux, X64, clacks-linux]`, sharing its single job slot
with verification. Apply Operations' runner guest prerequisites before enabling
deployment: GitHub CLI (`gh`) and passwordless sudo for the official Tailscale
action. The workflow checks those prerequisites before using deployment secrets.
The action starts an in-memory ephemeral tailnet connection only for the job
and logs out/stops its daemon afterward. The pinned action joins with subnet
routes accepted; the next step disables that preference with
`tailscale set --accept-routes=false` before deployment. The narrow tailnet grant
limits the identity's access throughout the connection. Passing the same option
through the action's `args` input is invalid because the CLI rejects duplicates.
Its cloud caching is disabled; the runner retains its normal local Cargo
artifacts. No permanent tailnet identity or deployment credentials are installed
by guest provisioning. Temporary SSH
key files are removed in an `always()` step.

Successful main push builds trigger production deployment automatically, without
an activation variable. Complete the infrastructure and credential setup before
merging workflow changes into main. Configure the GitHub environment
`clacks-production`:

| Kind | Name | Value |
| --- | --- | --- |
| Secret | `CLACKS_DOKKU_HOST` | Deployment SSH hostname; keep deployment-specific addresses out of committed files and public logs |
| Secret | `CLACKS_DOKKU_PORT` | Deployment SSH port |
| Secret | `CLACKS_DOKKU_SSH_KEY` | Dedicated private Ed25519 deployment key whose public half permits only the required deployment commands |
| Secret | `CLACKS_TS_OAUTH_CLIENT_ID` | Separate Tailscale deployment OAuth client ID |
| Secret | `CLACKS_TS_OAUTH_SECRET` | That client's secret, limited to creating auth keys for `tag:clacks-deploy` |
| Variable | `CLACKS_DOKKU_KNOWN_HOSTS` | Verified container host public key, prefixed by `clacks-dokku-production`, for example `clacks-dokku-production ssh-ed25519 PUBLIC_HOST_KEY_BASE64` |

Limit the production environment's deployment branch policy to `main`. For
unattended deployment, do not require a human environment reviewer; branch review
and protection remain the source approval boundary.
Deployment uses only a read-only GitHub token. Database, Google and wrapping-key
credentials remain on the server and must not be added to GitHub.

Restrict the deployment identity to the configured SSH endpoint through the
network policy. The dedicated SSH key permits only the Clacks Git receiver and
`clacks-deploy-revision`, which returns the running web container revision without
exposing its environment. Deny other applications and management commands.
Verify the endpoint host key out of band before configuring
`CLACKS_DOKKU_KNOWN_HOSTS`. Populate both endpoint secrets before merging a workflow
that consumes them.

The workflow accepts only successful push results for `main` from this repository,
checks out the exact tested SHA, and checks that it is still the current main tip
before setup and immediately before pushing. Failed/canceled verification and
pull-request results never deploy. A newer commit can arrive after the final
check; the in-flight release finishes and the newer successful main result deploys
subsequently. Superseded queued results are skipped. GitHub's standard concurrency
group uses `queue: max` to retain pending successful results instead of letting a
later completion replace them. It allows up to 100 pending jobs, prevents
overlapping releases and does not cancel an active deployment; it does not
promise deployment of every intermediate commit.

Deployment pushes the exact source SHA to Dokku's `main` without force. Dokku
builds the image and retains its migration/grant/readiness gates and deployment
lock. Then the workflow compares the running container's source revision with
the tested SHA and checks public HTTPS readiness and the packaged browser script.
Native build output, the GitHub job result and its summary report outcomes.

The first successful main push build after merging this workflow triggers
deployment and verifies the production credentials and connectivity.
A failed deployment can be retried by rerunning its GitHub deployment workflow
while that source remains main's current tip. If Git reports everything up to
date but running-revision verification fails, inspect native build/state rather
than forcing a push. A failed database release may have committed schema changes;
use the documented compatible recovery procedure, never automatic database rollback.

To stop new deployments, disable **Deploy Clacks server** in GitHub Actions or run
`gh workflow disable server-deploy.yml --repo jhelwig/clacks-log`. Inspect any
already running release separately; disabling the workflow does not undo a
deployed revision. Re-enable it with GitHub Actions or
`gh workflow enable server-deploy.yml --repo jhelwig/clacks-log`.
Production reachability, policy-test compilation and full first deployment are
live rollout checks, not established by local workflow linting or SSH tests.

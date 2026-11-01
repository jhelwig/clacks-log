# Account management: schema, workflows and deployment

This contract defines the schema, workflows and deployment boundaries for
[issue #11](https://github.com/jhelwig/clacks-log/issues/11).
[accounts.md](accounts.md) owns accepted behavior. D231–D233 and D353–D369 settle
account identity, authentication, recovery, sessions, deployment and transaction
policy. Concrete representations and tuning defaults below are engineering
choices rather than additional product requirements. The
[server guide](../lib/clacks-server/README.md) owns executable usage, and
[Linux deployment](../deploy/server/README.md) owns release operations.

OpenID Connect is abbreviated OIDC. A nonce is a random request-binding value
that the provider returns in its identity token for verification.

## 1. Scope and composition

Deliver a real Axum/Leptos application and PostgreSQL account store that later
synchronization, credential and service capabilities extend. No disposable
identity server, simulated persistence, independently deployed management service
or placeholder implementations of later capabilities.

Production boundaries:

- `lib/clacks-server`: configuration, PostgreSQL account operations, OIDC
  integration, request authentication, maintenance operations and application
  composition, organized into modules rather than a generic identity framework.
- `lib/clacks-web`: Leptos pages, forms and browser hydration; server-only
  integration delegates account operations to `clacks-server`. The server library
  does not depend on the web library. Browser builds exclude PostgreSQL and secrets.
- `bin/clacks-server`: thin executable composing the server library and web shell.
- `bin/clacks-admin`: one thin operator command delegating to the server
  library's migration, provider registration, role and recovery operations.

Use existing native `AccountId` values. Existing `clacks-account-data` generates
reference data; it is not the new account-management executable. Do not repurpose
it as an unrelated task dispatcher. Use standard clap subcommands and nested help
for `clacks-admin`; no command registry or generic task-execution framework.

## 2. Relational schema

Use a dedicated application schema, with SQLx's migration history and the
session adapter's own table structure maintained through release migrations.
There is no parallel schema-version table. PostgreSQL representation:
native account IDs as canonical 26-character ULID text, opaque login references
as random 256-bit tokens, and instants as `timestamptz`. ULID means Universally
Unique Lexicographically Sortable Identifier; its ordering is not authorization.

| Table | Columns and constraints | Purpose |
| --- | --- | --- |
| `accounts` | `account_id` primary key, `created_at` | Stable Clacks account, independent of provider and mutable display claims. |
| `login_identities` | `identity_id` primary key, `account_id` foreign key, `issuer`, `subject`, optional `display_name`, `created_at`; unique `(issuer, subject)`; unique `(account_id, identity_id)` | A verified external identity belongs to exactly one account. Display claims are labels, never linking authority. |
| `login_sessions` | `login_reference` primary key, `account_id`, `identity_id`, `created_at`, `expires_at`; composite foreign key `(account_id, identity_id)` to the identity binding, with delete cascade | Authoritative browser authentication, bound to its authenticating identity. Deleting a binding invalidates precisely those login sessions. |
| `server_administrators` | `account_id` primary/foreign key, `granted_at` | Live out-of-band role assignment. Ordinary users need no row here. |
| `clacks.browser_sessions` | Adapter-defined ID, serialized session data and expiration columns | Managed through `axum_session_sqlx`, not a second application session implementation. |

Index identities by account and login sessions by identity/account and expiration.
The login reference is a bearer secret: generate with an established random-number
facility, keep it server-side in middleware data, and exclude it from telemetry.
Use an ordinary identity ID; do not overload issuer labels or provider config keys
as account IDs. Compare issuer and subject exactly, with deterministic
case-sensitive database comparison; do not lowercase either. An issuer change is
not a provider rename and never silently rewrites existing bindings.

Account-owned tables added by later capabilities carry account ownership and
composite reference constraints where appropriate. Their schemas, synchronization
state and seeded shared reference definitions remain their own capability work;
#11 does not create fake QSO tables or a second shared-data representation.
When server shared-data persistence arrives, integrate the existing native
`account_definitions()` initializer into account provisioning and initialize
previously created #11 accounts before exposing shared state. New account
definitions then commit with provisioning. Do not replace the production account
store or invent a seed-status framework in #11 merely to anticipate that work.

### Account isolation

Use the live authenticated principal as the account source. Account operations
scope their fixed SQL queries to that account through explicit predicates/joins;
never authorize a caller-supplied account ID by its mere presence. Foreign keys
prevent cross-account reference mistakes. Administrator-role membership grants
only explicitly implemented administrator operations, not automatic access to
other accounts' log data.

Do not use PostgreSQL row-level security or connection-local tenant settings in
this contract. Authentication and account-scoped query construction enforce the
accepted boundary without tenant credentials or pooled-connection tenant state.
Maintenance commands use deployment-controlled access; do not expose equivalent
unauthenticated HTTP endpoints. Later jobs and synchronization derive their
account context from their authorized request or durable account-owned work.

### Transaction contracts

**First sign-in:** after complete OIDC validation, look up `(issuer, subject)`.
If absent, insert account and identity in one transaction. A concurrent unique
conflict rolls back the candidate account and retries ordinary identity lookup
in a fresh transaction; never leave an empty extra account or reassign the
winning binding. Establish the login session against the resolved account and
still-existing binding, with account-row coordination for identity admission.

**Existing-account identity changes:** lock the account row first, then recheck
current login validity, required authorizing identity and target binding. Commit
the binding change and login invalidation together. All binding mutations,
including recovery, follow that order. Uniqueness across accounts still depends
on the database constraint, not on locking one account.

**Authenticated request:** use an Axum extractor to conditionally extend an
existing, unexpired login row by the configured sliding duration and return its
account and identity. Missing/expired/deleted means unauthenticated. A database
failure is an availability error, not a new anonymous sign-in. Ordinary request
handling cannot insert or reactivate login rows. Consult administrator membership
when an operation needs that role; do not cache it in browser session data.

**Logout:** delete the authoritative login row and ask the middleware to destroy
the session. An earlier request may finish, but its middleware save cannot
restore authentication. At identity-changing commit boundaries, recheck login
validity under the account lock rather than trusting an earlier extractor result.

Provider requests and browser interaction never occur inside database
transactions. SQLx owns transactions, migration bookkeeping and database errors;
Clacks interprets only relevant domain outcomes. No custom lock manager or write
queue. Bounded contention/error handling details belong in the implementation
plan, not a general automatic retry around external effects.

## 3. Standard browser and callback integration

Use `openidconnect` authorization-code requests with Proof Key for Code Exchange
(PKCE), random `state` and nonce. The library handles protocol construction,
asynchronous code exchange and token verification. `axum_session` supplies
cookies, typed session data and renewal. Axum supplies routing and extraction;
Leptos supplies pages, actions and the Axum extractor bridge. None of these
facilities creates the Clacks identity-binding rules automatically.

Pending context belongs in ordinary middleware session data: expected provider,
nonce, verifier, purpose and the originating account/login for identity changes.
Store separate typed entries keyed by request state rather than one overwritten
"current login" slot. Default expiry is ten minutes per provider round trip;
expired or missing context requires restarting that action. Bound retained
entries to a small internal limit and remove expired entries when handling
authentication operations. The precise limit is an internal implementation
default, not an account/session eligibility policy or deployment setting.

Retrieve/remove the matching entry using native session APIs, compare secrets
using the library's supported timing-resistant facility, and validate the
callback before performing account operations. Retain expected issuer selection
from server-side context; never discover arbitrary callback-supplied providers.
Missing, mismatched or failed callbacks do not provision accounts or change
bindings. Do not log query strings containing authorization codes or state.

`get_remove` is process-local, not a distributed consume transaction. Duplicate
callbacks must still exchange their code successfully and pass all validation
before reaching Clacks mutations; the provider rejects repeated redemption.
A stale middleware save is not authority to bypass these checks. This contract
adds no pending-flow database, global consume guarantee or workflow framework.
Concurrent flows may lose cached middleware changes; fail closed and offer a
restart rather than promising loss-free multi-instance session editing. This
availability limitation is distinct from authorizing the wrong account.

For recent authentication, request `max_age=300` and let the provider handle
reauthentication. Keep the library's normal identity-token validation and
request-nonce verification. Do not require or independently age-check
`auth_time`, install an additional freshness-verification hook, or cache a
reusable freshness assertion. Action-local continuation identifies which
verified existing identity authorized a link; it is not a general freshness
cache or an alternative to the provider round trip.

### Routes and workflows

Route names below describe browser behavior, not a separate public web API.
Leptos server functions implement form actions using ordinary Axum extraction.
They are the browser mutation interface; there are no parallel hand-written
start, logout, link or unlink endpoints. The provider callback redirects to the
account page on success or sign-in with a safe error on failure, independently
of the request's Accept header.

| Entry point | Behavior |
| --- | --- |
| `GET /sign-in` | Show configured providers and a clear provider-failure/retry state. |
| Sign-in form action | Start normal sign-in using a configured provider. |
| `GET /auth/callback/{provider}` | Validate the pending flow and library response; dispatch only its recorded purpose. |
| `GET /account/identities` | Show linked identities and the current authenticating identity; offer link/unlink actions. |
| Link form action | Start recent authentication through an existing identity, then authenticate the additional identity. |
| Unlink form action | Identify target and authorizing remaining identity, then perform recent authentication and atomic removal. |
| Sign-out form action | End this Clacks login; do not imply logout from the identity provider or other devices. |

**Normal sign-in:** validate → resolve or provision identity/account → insert
login record → store its reference in middleware data → call middleware `renew`
→ redirect to the account page. Do not retain provider access/refresh tokens
merely to keep the Clacks session alive. Account data does not become browser
session data. A cookie-save failure can leave an unused login row, which expires;
it cannot establish an unverified login or require a distributed DB/cookie commit.

**Link:** retain the originating valid Clacks login and selected additional
provider. Authenticate an already-linked identity with `max_age=300`; then start
a distinct authorization request for the new identity. Carry forward only the
continuation of this particular link action. At the second callback recheck the
originating login/account and continued membership of the authorizing identity
under the account lock. Add an unbound identity; treat an identity already on
this account as unchanged; refuse one owned elsewhere. Do not provision another
account or replace the current session's authenticating identity during linking.
Where the provider supports it, use standard account-selection prompting when
choosing an additional identity from the same provider; no provider-specific
account-switching implementation.

**Unlink:** disable removal when only one identity remains, explaining why.
For an eligible target, authenticate through an identity that will remain linked
with `max_age=300`. The callback locks/rechecks the account and removes only the
selected binding. If the initiating login used the removed identity, it becomes
invalid; return to sign-in rather than silently creating a replacement login.
Other identities' sessions and desktop credentials survive.

Cancellation, provider refusal, an expired flow, logout during an identity action,
or a changed binding leaves existing account data/bindings untouched. Present a
specific action-level error and restart path. External provider failures and
local persistence failures remain distinguishable in operator diagnostics.

### Request protection and presentation

Mutations use POST, with same-origin request validation against the configured
public origin: check `Origin`, or the parsed origin of `Referer` if absent, and
reject missing, null or mismatched origins. Use standard header/URL parsing and
Axum middleware facilities; no homemade cryptographic CSRF token system. CSRF
means cross-site request forgery. Apply this to normal sign-in initiation,
logout and identity forms/server-function endpoints, including direct requests
that bypass the visible page. The provider's GET callback instead uses standard
OIDC correlation, nonce and PKCE validation. SameSite=Lax cookies allow that
cross-site top-level callback; do not apply mutation-origin rejection to it.

Use secure, HTTP-only, host-only cookies at `/` with the library's supported
cookie key, configured identically across instances. Do not bind cookies to a
client IP or user-agent string. Configure middleware persistence and cookie
lifetimes for the same sliding duration as authoritative logins, with browser
cookie retention sufficient for that interval. Do not leave the middleware's
shorter default lifetime in place or mistake its long-term lifetime option for
an absolute authentication deadline. The authoritative check remains decisive.
Development-only HTTP configuration must be
explicit and must not weaken production settings.

Use ordinary Leptos forms, field validation, pending/disabled submit state and
inline outcomes. The account screen shows account ID, provider/identity labels,
current identity, Add identity, Remove and Sign out. No account merge, password
form, web role editor or security-history dashboard. Keep the browser design
consistent with the defined-section desktop language without attempting to port
egui widgets. Detailed visual mockups remain presentation work, not authority
to invent additional settings or account policies.

## 4. Deployment and maintenance

Use one Linux-container application image with server-rendered pages and built
hydration assets, plus PostgreSQL and deployment-managed HTTPS. Keep the image
immutable and runnable without root. Static assets and web routes share the
configured public origin. The restricted internal listener trusts forwarded
information only from configured proxies. Reject inconsistent public callback
configuration at startup instead of deriving it from incoming host headers.

Use the existing `config` crate and typed configuration with type-level defaults,
plus conventional command-line parsing. Suggested inputs: public URL, internal
listen address, PostgreSQL connection-secret path, trusted proxy ranges, provider
keys/labels/issuers, preconfigured or dynamic registration mode, registration
file paths, cookie-key secret path, session duration and telemetry destinations.
Do not create a second hand-built environment/override framework. Configuration
contains provider keys for local lookup; verified issuer/subject remains the
external identity. No network-mutated provider list or web provider editor.

### Provider credentials and dynamic registration

Preconfigured credentials come from deployment secrets. For dynamic registration,
`clacks-admin oidc register` is a deployment-setup operation using standard discovery and
`openidconnect::registration`, then serializes the returned registration to a
protected deployment file using ordinary library/Serde facilities. Include the
expected issuer, client registration metadata and returned client/management
credentials and expiration metadata; never print secret contents. This is a
file artifact, not a new vault or versioned credentials framework.

The proposed file is writable only by deployment setup and mounted read-only
into runtime instances (owner-only permissions for a self-hosted file; the
platform's secret mechanism for managed deployment). Atomically publish it using
standard filesystem facilities, preserving an existing file on failure. Reuse
it on subsequent setup/start; mismatched issuer/callback settings require an
explicit operator change, not automatic re-registration. Deployment automation
runs setup once, rather than every replica racing to register. An ambiguous
provider outcome before credentials are saved requires operator investigation;
Clacks cannot promise exactly-once registration across provider and filesystem.

Do not store these deployment-wide credentials under an arbitrary user account.
D236's per-account encryption keys remain the requirement for account service
secrets in #13; they are not needed to invent encryption for deployment-mounted
OIDC secrets. Back up the registration and cookie-key secrets through deployment
secret-management procedures separately from PostgreSQL. Cookie-key replacement
may invalidate cookies; no seamless key-rotation promise is added here.

### Release and operator procedures

Use one discoverable operator entry point with operation-specific credentials:

| Subcommand | Credentials and access |
| --- | --- |
| `clacks-admin database migrate` | PostgreSQL migration credential with schema-changing privileges; no OIDC or cookie secrets required. |
| `clacks-admin role grant` / `role revoke` | PostgreSQL maintenance credential with administrator-membership read/write access; no schema-changing privileges, provider credentials or cookie secrets required. |
| `clacks-admin identity recover` | PostgreSQL maintenance credential with account/identity/login access needed for locked recovery; no schema-changing privileges, provider credentials or cookie secrets required. |
| `clacks-admin oidc register` | Selected provider's registration authorization and protected registration-file destination; no PostgreSQL or cookie secrets required. |

Resolve only the selected operation's configuration and secrets. Keep runtime,
migration and maintenance database connection inputs distinct; do not introduce
one shared elevated connection default or fall back to stronger credentials.
Use PostgreSQL grants to enforce access. Sharing the binary does not grant each
operation every credential or require starting the web application.
These requirements do not prescribe a separate database login for each
subcommand; a deployment may use its maintenance credential for both role and
recovery operations with the required grants.

1. Supply PostgreSQL and protected deployment configuration/secrets.
2. Run `clacks-admin database migrate` with the migration role. SQLx owns
   its lock/history/checksum checks; application startup does not run schema
   changes (data definition language, or DDL).
3. Run `clacks-admin oidc register` where selected, and distribute the resulting
   protected registration file. Reuse existing registrations on upgrades.
4. Start the application with a DML-only runtime role and the released assets.
   DML means data manipulation language; the role can read/write application
   records but cannot create or alter the schema. Connect middleware to the
   existing adapter table through its public APIs without startup initialization.
5. Sign in normally, then use `clacks-admin role grant` to grant the existing
   account ID the administrator role. There is no first-user-is-admin rule.

Use a deployment-controlled maintenance credential with only the required data
privileges for role/recovery operations; it need not have migration DDL rights.
Grant the runtime role read-only access to `server_administrators`, not blanket
write access to every application table. Maintenance role-assignment commands
have the necessary insert/delete privileges; the migration role owns schema
changes. Runtime writes are limited to the identity/login and middleware tables
its implemented operations actually use. Later capability migrations extend
those grants deliberately. Keep schema usage and object grants explicit.
Role commands grant/revoke membership;
recovery adds a verified issuer/subject under D358/D359 and refuses ownership
elsewhere. Document how the operator verifies account ownership and obtains the
exact replacement identity before ordinary first sign-in. Preserve prior
identities/sessions unless a separate revocation is explicitly requested.

Expose separate process-liveness and readiness checks. Readiness requires usable
PostgreSQL and loaded valid configuration/registration; a temporary provider
outage affects provider sign-in, not every existing authenticated request.
Do not add duplicate schema-version checks. Stop admitting requests on shutdown,
allow bounded in-flight completion, close pools and flush telemetry using the
standard runtime/server facilities. Exact timeouts are deployment defaults.

Backup/recovery documentation covers PostgreSQL plus deployment secrets;
restoring an older database restores older bindings, roles and session validity.
No rollback-surviving logout/revocation guarantee is claimed. Release changes
follow D361's compatible expansion and later cleanup; do not use automatic down
migrations or database restoration for ordinary application rollback.

## Engineering defaults and current limitations

These choices implement the agreed behavior; they are not additional user-approved
product requirements:

- The agreed browser-session policy is fourteen-day sliding expiration. A pending
  provider round trip instead expires after ten minutes by default, configurable
  with `pending_seconds`.
- At most eight pending requests are retained per browser session. This internal
  bound removes expired entries and then the oldest when necessary. It is not a
  deployment tuning surface.
- First-sign-in identity contention is retried at most three times. This is a
  local database admission bound, not an external-service retry policy.
- The runtime connection pool permits ten connections. Request draining defaults
  to thirty seconds (`shutdown_seconds`); each enabled telemetry signal has a
  five-second shutdown bound. These are resource and deployment defaults.
- The current routes, assets and redirects require hosting at the origin root.
  Consequently `public_url` rejects a path prefix. This is an implementation
  limitation, not a requirement of origin validation or an agreed prohibition on
  subpath deployments. Supporting a prefix requires consistent integration across
  those routes, assets and redirects.
- SQL metadata checking and account tests share one disposable PostgreSQL database
  with production migrations. Separate migration/runtime credentials enforce the
  agreed privilege boundary; separate databases are not required.

## 5. Observability and verification

Follow [observability.md](observability.md). Retain structured operator outcomes
for committed provisioning, identity changes, role changes, recovery and logout;
report provider/configuration/database failures at the boundary handling them.
Use standard HTTP and database instrumentation with safe route templates,
latency/error metrics and bounded outcome labels. Account/identity IDs can be
access-controlled event context, never unbounded metric labels. Exclude cookies,
login references, codes, state, nonce, verifiers, tokens and provider payloads;
request tracing must not record sensitive query strings or headers.

Verification targets Clacks decisions through its production routes/operations:

- Two sign-ins for one external identity resolve one account; linking an identity
  owned elsewhere does not transfer it or leave another account behind.
- A real account operation ignores/rejects another account's target identity;
  concurrent removals preserve a remaining identity using the production lock.
- Logout/unlink prevents subsequent authentication despite a stale generic
  session reference; an unrelated identity login remains usable.
- Link/unlink callback purposes cannot fall through into normal provisioning;
  failures/cancellation do not commit a binding change.
- Role/recovery maintenance preserves the documented account boundary and emits
  safe committed-outcome diagnostics.

Combine these where one scenario proves several assertions. Do not test SQLx
migration checks, PostgreSQL foreign-key semantics, library token cryptography,
cookie serialization or Rust randomness as independent Clacks requirements. Use
protocol fixtures only for Clacks callback wiring; live provider interoperability
is separate acceptance evidence. No injected database triggers, test-only
production APIs, mock account stores or coverage targets.

Live-provider acceptance distinguishes discovery, registration/reuse and normal
PKCE sign-in from identity-change flows that request `max_age=300` and return
through the ordinary validated callback.
Successful normal sign-in does not establish the identity-change requirement or
compatibility with other provider versions/configurations. Record the exercised
paths in the implementation plan/PR rather than inferring them from source/docs
inspection or adding a workaround for a provider that fails the agreed flow. Dependency feature resolution,
Rust/WebAssembly builds and actual container execution require release
verification; source inspection alone does not establish either.

## Sources for integration responsibilities

- [`openidconnect` standard asynchronous flow](https://docs.rs/openidconnect/4.0.1/openidconnect/#asynchronous-api)
  and [identity-token verifier](https://docs.rs/openidconnect/4.0.1/openidconnect/struct.IdTokenVerifier.html).
- [`axum_session` session facilities](https://docs.rs/axum_session/latest/axum_session/struct.Session.html).
  The engineering baseline is 0.22.0 source; cached latest documentation may show
  0.21.0. Do not infer cross-instance atomic consumption from `get_remove`.
- [Leptos Axum extraction](https://docs.rs/leptos_axum/latest/leptos_axum/fn.extract_with_state.html)
  and [server-function security responsibilities](https://book.leptos.dev/server/25_server_functions.html).
- [OWASP origin verification](https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html#using-standard-headers-to-verify-origin)
  and [OAuth authorization-code response](https://www.rfc-editor.org/rfc/rfc6749#section-4.1.2).

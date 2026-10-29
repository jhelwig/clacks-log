# Accounts and client enrollment

D62-D63 establish individual-client revocation and independent account isolation in minimum viable product (MVP). D64’s earlier statement was permissive scope language; D191 now selects automatic account provisioning on first successful OpenID Connect (OIDC) sign-in. D65 selects browser-based client authorization and a deployment-provided web user interface (UI). D190 selects configurable OIDC for web sign-in. D191 selects first-sign-in provisioning, while D231 clarifies that multiple external login identities can link to one independent Clacks account; D192 accepts manual out-of-band administrator assignment. D193 selects browser-callback desktop authorization for MVP, with device authorization as a post-MVP fallback. D357/D358 select maintenance commands for role assignment and recovery; the #11 proposal below supplies concrete schema and integration details. Desktop enrollment remains separate capability work.

The [account-management proposal](account-management-proposal.md) supplies the
concrete #11 schema, workflows and deployment design for review. It carries
forward the accepted decisions here; its physical schema, routes and engineering
defaults are proposals rather than additional accepted product decisions.

## Web-interface architectural scope (D222)

The web interface is intended to grow beyond the MVP account, setup and diagnostic workflows to logbook browsing/editing, reporting and potentially logging. D223 selects Leptos/Axum with server-rendered initial pages and Rust/WebAssembly browser interactivity through hydration, evaluated against that broader direction. This does not expand MVP scope, require browser offline operation or desktop feature parity. Exact component versions, styling and layouts remain open.

## Server persistence access (D360)

Use SQLx for server PostgreSQL access as well as client SQLite access. Keep fixed
server queries in separate `.sql` files using `query_file!` or `query_file_as!`,
with compile-time checking and committed offline query metadata. Reuse tooling
and conventions while keeping the PostgreSQL schema, migrations and queries
independent of the client's SQLite schema. D361 defines release-managed server
migration execution below; exact dependency integration remains specification
work.

## Accepted boundaries

One server can host multiple independent accounts in MVP. Each account's log data, profiles, enrolled clients, and external-service credentials are isolated. Apply that boundary to storage access, synchronization negotiation/streams, job execution, credential access, diagnostics, and client revocation. Guessing another account's object identifier must not authorize access. The #11 proposal uses explicit authenticated-account predicates/joins and account-scoped reference constraints, without PostgreSQL row-level security or connection-local tenant settings; its concrete storage/access design is under review.

An owner can revoke an individual client's server access without deleting shared radio contacts (QSOs), erasing the offline client, or affecting other clients. An enrolled client does not lose eligibility simply because it has been offline indefinitely (D60). Enrollment credential mechanisms and manual reauthorization workflows remain integration work; the synchronization model defines the admission cutoff.

D145 preserves a reauthorized installation’s synchronized local data and pending work without requiring Amateur Data Interchange Format (ADIF) migration or empty-log enrollment. D152 makes revocation the cutoff for automatic acceptance from the old actor; previously unsynchronized edits on a recovered installation require explicit manual reconciliation. Reauthorization alone does not automatically submit that work. Revocation need not wait for a final synchronization or cooperation from a lost device. Already accepted shared data remains shared and recoverable through other authorized clients. The [synchronization model](synchronization-model.md) defines retirement and compaction; enrollment and manual-recovery integration remain capability work. D56 still governs joining unrelated independently populated data.

D143 clarifies rollback scope: restoring server authorization state from an older backup can restore the earlier client authorization state, losing a later revocation recorded only there. No independently preserved authorization store or rollback-surviving revocation guarantee is selected. The earlier recommendation to require that guarantee is withdrawn; no new review/reauthorization workflow follows from this clarification.

This does not select cross-account shared logs, collaboration roles, account-wide remote erasure, billing, or hosted-service packaging.

## Account creation scope (D64)

D64 originally said administrator-created accounts were sufficient and self-service registration could be post-MVP. Those were permissive statements, not requirements. D190/D191 select normal OIDC sign-in provisioning: an unbound verified identity creates an isolated account, while a bound identity authenticates to its linked account. D231 separately permits linking additional external identities to an existing account; first-sign-in provisioning does not establish a one-to-one account/identity model. No administrator pre-creation or additional Clacks approval queue is required. Server-administrator role assignment remains separate from ordinary account provisioning.

Account provisioning remains separate from D62's per-client revocation and D63's account isolation.

## Internal accounts and external login identities (D231)

The Clacks account ID identifies the system account. Logbooks, profiles, service credentials, roles and enrolled clients refer to that account independently of its login providers. External OIDC issuer/subject identities are login bindings to that account, not the account itself.

A Clacks account can have multiple external login identities, from the same or different providers. Each external identity can be bound to at most one Clacks account. Authentication through any linked identity resolves to that same account; do not create a separate account merely because a different linked provider was used.

Adding or removing a provider binding changes the available authentication methods rather than migrating the internal account. D232 includes self-service linking/unlinking in the MVP web interface with the safeguards below. Automatic email-based linking and account merging are not selected. D233 makes documented administrator-assisted recovery sufficient for MVP when all linked identities are inaccessible.

## Browser sessions (D353)

Use PostgreSQL-backed browser sessions with an opaque random session identifier
in a secure, HTTP-only cookie. Server-side session state resolves to the stable
Clacks account and records the linked identity used to authenticate, so unlinking
can invalidate the appropriate sessions. Session expiration remains ordinary
session management; do not maintain custom provider-authentication freshness
state in the browser session.

D367 separates authoritative authentication validity from general middleware
session data. Successful OIDC sign-in creates a server-side login-session record
tied to the Clacks account and the linked identity used to authenticate. The
middleware session holds a reference to it, not sufficient authentication proof
by itself. Every authenticated request resolves that reference against current
PostgreSQL state and checks the login session's validity; do not rely on a cached
positive authentication decision across requests or application instances.

Logout invalidates the authoritative login session. Unlinking an identity
invalidates the login sessions established through that identity, preserving
other identities' sessions and enrolled desktop clients. Ordinary middleware
saves cannot create or reactivate authoritative login sessions. Stale middleware
data may survive or be recreated without restoring authenticated access. D366's
14-day sliding expiration applies to authoritative login validity; retention or
refresh of middleware data/cookies cannot independently extend authentication.
D368 selects `axum_session` with `axum_session_sqlx` for PostgreSQL persistence,
retaining the middleware's cookie handling, session identifiers, renewal and
general session storage. Use ordinary Axum request authentication to resolve the
login-session reference against authoritative PostgreSQL state under D367.
Middleware caching is not an authentication-validity cache. The [account-management contract](account-management-proposal.md) defines
the concrete schema and transaction integration; the
[server guide](../lib/clacks-server/README.md) documents the runtime.

Initialize middleware storage through release-managed migrations under D361;
ordinary application startup uses the runtime database role without executing
schema initialization. The evaluated dependency baseline is `axum_session`
0.22.0 and `axum_session_sqlx` 0.12.0, compatible with SQLx 0.9. Keep this
integration within the libraries' public APIs rather than replacing cookie or
general session machinery. See the
[session-store API](https://docs.rs/axum_session/0.22.0/axum_session/struct.SessionStore.html)
and [PostgreSQL adapter](https://docs.rs/axum_session_sqlx/0.12.0/axum_session_sqlx/).

Browser sessions are separate from independently revocable
desktop-client credentials. This choice does not select PostgreSQL
row-level security or connection-local account settings.

D354 makes unlinking a login identity invalidate browser sessions authenticated
through that identity. Sessions authenticated through identities still linked to
the account remain usable. Enrolled desktop clients retain their independently
revocable account-bound credentials. The unlinking action still requires recent
authentication through an identity that will remain linked under D232.

D366 selects a deployment-configurable 14-day sliding browser-session expiration
window in place of D356's earlier 30-day absolute expiration policy. The sliding
duration is shorter than that earlier absolute duration; this does not require
both deadlines simultaneously or establish a separate mandatory absolute cap.
Session activity can extend the sliding deadline. Do not show idle-expiration
warning dialogs under this default. Explicit logout immediately invalidates the
server-side session; D354 identity-unlink invalidation and D355 recent
authentication for identity changes remain in force. Browser expiration does not
affect desktop enrollment. This sliding policy is not a claim of compliance with
NIST's overall reauthentication-timeout guidance.

## Self-service login-identity management (D232)

An authenticated user can add another login identity from a deployment-configured provider. Require recent authentication to the existing account and authenticate the additional identity. Link it to that account rather than running D191 new-account provisioning. If the identity is already bound to a different Clacks account, refuse the link; do not transfer ownership or merge accounts automatically.

Unlinking requires recent authentication through an identity that will remain linked. Prevent removing the last login identity through ordinary account settings. Account data and enrolled clients remain attached to the stable internal account under D231. D233 makes documented administrator-assisted recovery sufficient for MVP when no linked identity is accessible; automated self-service recovery is post-MVP.

D355 uses the standard OIDC flow for recent authentication. Whenever linking or
unlinking requires recent authentication to the existing account, redirect the
browser to its provider with `max_age=300`. The provider determines whether its
existing authentication satisfies that age, performs any necessary
reauthentication using its own authentication methods, and returns through the
normal callback. If already satisfied, the round trip can complete without
additional user interaction.

Use the OIDC library for standard response validation, including the returned
`auth_time` claim for the requested maximum age, then check that the returned
issuer/subject belongs to the intended account. Linking uses an already-linked
identity for this step, separately from authenticating the new identity;
unlinking uses an identity that will remain linked. Do not use local freshness
timers, cached authentication proofs to bypass this round trip, or
provider-specific substitutes for the standard flow. Authentication methods and
reauthentication decisions belong to the provider, not Clacks. Five minutes is
the selected Clacks policy, not a duration prescribed by OIDC. See
[OIDC authentication requests](https://openid.net/specs/openid-connect-core-1_0.html#AuthRequest)
and [response validation](https://openid.net/specs/openid-connect-core-1_0.html#IDTokenValidation).

Verify provider interoperability with this standard flow during integration,
including tsidp. Unsupported protocol behavior is a
provider compatibility issue, not a reason to implement replacement
authentication semantics in Clacks.

### Identity-management transactions (D369)

Serialize changes to an existing account's identity bindings by locking that
account's row within a short PostgreSQL transaction. All identity-binding
mutation paths, including administrator-assisted recovery, follow this rule.
After acquiring the lock, recheck the applicable account, identity and
authentication requirements against current state. In particular, concurrent
unlink operations must not collectively remove the last identity or remove the
identity required to authorize another operation.

Commit the binding change and affected authoritative login-session invalidation
atomically. Retain database uniqueness enforcement for the issuer/subject pair
across accounts; an account-row lock alone does not enforce that constraint.
Complete provider authentication and other network requests outside the
transaction. This coordinates identity management, not ordinary account-wide
requests or future QSO writes.

Concrete callback integration, schema and interface layout remain specification
work. [OWASP authentication guidance](https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html#require-re-authentication-for-sensitive-features)
supports reauthentication for sensitive account changes; D232 selects the Clacks
workflow rather than a particular authentication library.

### Standard callback integration facilities

The engineering baseline is the standard authorization-code flow composed from
the selected libraries, not a separately designed callback protocol. The
[`openidconnect` asynchronous example](https://docs.rs/openidconnect/4.0.1/openidconnect/#asynchronous-api)
generates the authorization URL, random `state` and nonce, and a Proof Key for
Code Exchange (PKCE) challenge/verifier. It supplies asynchronous code exchange
and ID-token verification. The application retains the expected values, checks
the returned state, supplies the verifier during exchange, and passes the
expected nonce into token verification. Configure the library's supported
timing-resistant secret comparisons rather than inventing a comparison scheme.

`axum_session` supplies the request extractor, typed `set`/`get`/`get_remove`
session values, persistence, cookies and session-ID renewal. Its ordinary
session data can hold pending callback context. Clacks supplies the routes and
binds that context to the expected provider and intended sign-in or identity
management action. Only after successful protocol validation may it perform the
account operation and establish authoritative login validity under D367. Use
the middleware's `renew` facility for session-ID renewal after sign-in.

D355's `max_age=300` request uses the library's authentication-time verification
hook to require and check the returned `auth_time`. In `openidconnect` 4.0.1,
the default hook accepts that claim without checking its age; sending `max_age`
alone does not configure response enforcement. Supplying this policy through
the existing hook is ordinary integration, not local authentication-freshness
bookkeeping. See the
[verifier API](https://docs.rs/openidconnect/4.0.1/openidconnect/struct.IdTokenVerifier.html#method.set_auth_time_verifier_fn).

`axum_session` 0.22.0 removes values from its process-local session data through
`get_remove` and persists changes during response handling. This is not a
cross-instance atomic consume operation. Do not infer such a guarantee or add
dedicated workflow tables merely to obtain it. The provider already enforces
single-use authorization codes under
[OAuth section 4.1.2](https://www.rfc-editor.org/rfc/rfc6749#section-4.1.2).
That protects against repeated exchange of the same code; it does not make
middleware state removal globally atomic. Any additional coordination must be
justified by a concrete Clacks requirement beyond the standard flow. Dedicated
pending-operation records and a general callback-workflow abstraction are not
selected requirements.

## Recovery without an accessible login identity (D233)

A documented administrator-assisted procedure is sufficient for MVP. Establish ownership of the existing Clacks account out of band and explicitly link a newly verified external identity to that account. Preserve its internal ID, data, service credentials and enrolled clients. Record the recovery in operator security logs under D230. Respect D231’s uniqueness rule for external identity bindings.

D358 selects `clacks-admin identity recover` for recovery. Identify the existing
account by its Clacks account ID and add the newly verified identity using its
exact OIDC issuer and subject. Refuse an identity already bound to another
account. The operator verifies account ownership and the replacement identity
through a documented out-of-band procedure; the command performs the authorized
binding change without email-based ownership inference or account merging.
Preserve account data, service credentials and enrolled clients, and record the
recovery in operator logs. No web recovery-request system is required for MVP.

D359 defines this recovery as loss of access to every linked login identity,
without assuming compromise. The operator adds the newly verified identity to
the existing account; leave existing login bindings and browser sessions intact,
along with enrolled clients. A working Clacks browser session may still exist
even though the owner can no longer authenticate through its provider. Suspected
compromise requires separate, explicit revocation actions rather than implicit
revocation during recovery.

Verify the replacement identity out of band before ordinary Clacks sign-in:
normal sign-in with an unbound identity provisions a separate account under D191.
If that has already happened, the recovery command refuses the bound identity;
it does not transfer the identity or merge the accounts. The documented procedure
must explain this distinction.

Automated self-service account recovery is post-MVP. This exceptional procedure
does not replace D232 self-service linking/unlinking for users who can still
authenticate. Exact ownership-verification procedures and command interface
details remain specification work.

## Browser-based client authorization (D65)

The desired experience follows Tailscale/GitHub command-line interface (CLI): the client initiates authorization, the user authenticates in a browser through the deployment's web UI and authorizes the client, and the client obtains its own revocable authorization for ongoing synchronization. Account credentials are not entered into the desktop client. D193 selects browser-callback authorization for MVP: the desktop opens the browser and receives completion through a callback. Device authorization is a post-MVP fallback option.

The deployment provides the Clacks web UI and enrollment/management endpoints within the single Clacks application containing synchronization and background workers, alongside PostgreSQL (D213/D223). The external OIDC identity provider supplies authentication; it is distinct from the Clacks management endpoints. D190 selects configurable OIDC web sign-in without requiring a particular provider or local password registration. D191 selects automatic local provisioning after successful configured-provider authentication.

D190 selects OIDC for web sign-in. D193 selects the browser-callback flow for MVP, and D370 selects a temporary loopback listener on an operating-system-assigned port. D371 selects short-lived access tokens with a persistent per-client refresh credential. D374 selects opaque access tokens. D378 binds both credential types through Demonstrating Proof of Possession (DPoP) and retains the refresh credential across ordinary refreshes. D376 selects `keyring-core` with native macOS and Windows credential backends. D379 selects Huskarl for the desktop and oauth-as for the embedded authorization server, superseding D375. Refresh-token representation, binding storage mechanics, and interrupted-enrollment handling remain engineering work. Use standard supported mechanisms once selected, not a bespoke OAuth-like token exchange inferred from the user experience (UX) analogy.

Reference: [GitHub CLI login](https://cli.github.com/manual/gh_auth_login) documents browser authentication; [OAuth device authorization, Request for Comments (RFC) 8628](https://www.rfc-editor.org/rfc/rfc8628.html) specifies a device/user-code browser authorization flow. D193 defers device authorization to a post-MVP fallback; it is not the selected MVP flow.

## Accepted MVP web-UI scope (D66)

Provide account sign-in, D232 self-service login-identity linking/unlinking, client authorization, enrolled-client listing/revocation, external-service configuration, and upload diagnostics in the MVP web UI. D191 provisions accounts at normal first sign-in; administrator account creation is not a required web workflow. Administrator-role assignment is out of band under D357. Account-specific management and diagnostics follow D63 isolation. Issue #11 delivers the account/identity web workflows; later capabilities add enrollment, service setup and diagnostics.

Publication/confirmation-service configuration uses the server-owned configuration and credentials; diagnostics describe the server's actual jobs, attempts, outcomes, and pauses. The web UI, synchronization endpoints and background workers are internal parts of the same Clacks application and use its account-scoped configuration and publication state. Routine logbook operation remains native desktop scope under D06. D67 places full external-service setup in the web UI, opened from the desktop; station defaults/profiles remain native. Exact layout, native diagnostic detail and deployment tooling remain engineering work within that selected topology.

## Per-client synchronization diagnostics (D142)

MVP diagnostics show each enrolled client's connection state, last successful synchronization, and any current synchronization error. These are sufficient; the earlier separate catch-up-status suggestion does not establish a progress percentage, missing-record count, or measure of how current a client is. Exact presentation and timestamp/error semantics remain open. During restore, these diagnostics help the user judge which sources are available, not prove that unavailable data has been recovered.

## Desktop/web service responsibilities (D67)

The desktop opens the appropriate web UI for server-owned publication/confirmation-service setup: service credentials, QRZ logbook mappings, Logbook of The World (LoTW) certificate management, and service schedule configuration. Station defaults and saved location profiles remain in the native logging workflow. Live service status and routine operational actions remain available during native operation.

D228 includes LoTW private-key credential removal in MVP through service setup, preserving public certificate metadata and submission history; reusable certificate/private-key export is post-MVP. Removal prevents further signing with that credential and pauses only uploads left without an eligible route. It is distinct from issuer revocation and from documented native backup/recovery. Exact in-flight/removal mechanics remain open.

D15 still allows certificate import initiated by any connected client; opening its account-authorized web setup is consistent with that entry point. Detailed upload diagnostics are in the web UI under D66; exact native diagnostic detail, controls, and layout remain open. This does not select a web station-profile editor.

## Client-direct directory lookup

D96 requires QRZ/HamQTH directory requests to execute directly on clients with securely available directory credentials, even when sync is unavailable. This extends the credential model beyond server-owned publication/confirmation jobs. D97 selects central web configuration once per account with secure provisioning to enrolled clients for protected local storage. Credentials last received remain usable during sync outages, with updates received on reconnect. Exact local/server protection mechanisms, removal/update, and revocation consequences remain open. Do not assume D96 distributes LoTW private keys or publication credentials.

## Account-owned contacts and logbook tags

An account supports multiple Clacks logbooks, represented by tags on account-owned radio contacts (QSOs). These are distinct from external QRZ logbooks. Each QSO has one identity and a complete tag set; multiple tags do not create independent contact copies. Tag identities are stable even when names coincide or change. Universally Unique Lexicographically Sortable Identifier (ULID)-like identifiers remain the preferred direction (D120).

Current-tag selection is client-local. It supplies the default tag for new contacts and scopes history/Worked Before without changing operating defaults or historical facts. MVP supports adding/removing existing tags for individual QSOs and selected groups, including leaving contacts active and untagged (D249). Offline creation, naming, rename and deletion are supported; equal tag names do not establish shared identity. Automatic assignment/creation rules remain outside MVP.

Tag deletion offers leave-all/delete-all/delete-unique QSO choices, showing the impact on contacts with other tags (D244). Leaving a contact removes only the deleted tag; deleting a contact marks the QSO itself deleted across normal views. A pending QSO with a deleted-tag reference remains local until explicit membership removal/replacement, while unrelated synchronization continues (D250). See [contact and tag lifecycle](product.md#membership-and-tag-lifecycle).

Each tag’s automatic-upload policy is Enable, Forbid or Neither independently per service, with new tags defaulting to Neither (D245/D246). Any Forbid wins; otherwise at least one Enable grants eligibility. All-Neither and untagged contacts have no grant. Manual uploads retain existing rules, and downloads match across the account regardless of tag policy. At most one local QSO may associate with a given external record within the account/service destination (D251).

All QSOs with their tag sets, tag definitions/lifecycle, service evidence and synchronized shared configuration use one synchronization scope per account (D248). Every enrolled authorized client participates in its ancestry, durable acknowledgment and retention boundary. Current view selection does not restrict replication; selective synchronization remains post-MVP. Server-only credentials and client-local operating defaults retain their existing boundaries.

D179 permits requiring an incompatible client to upgrade before synchronization resumes. Preserve enrollment, local data, pending edits and comparison evidence. This does not require Amateur Data Interchange Format (ADIF) migration or re-enrollment. Exact compatibility negotiation and migration mechanics remain engineering work.

## MVP server deployment (D189)

Provide a documented Linux container deployment for the MVP server. Native Windows server hosting is not required for MVP; this is distinct from D162 Windows desktop support. D213 selects one Clacks server application containing the web interface, synchronization and background workers, alongside PostgreSQL. Independently deployed application services sharing tables are not the topology. Exact runtime/orchestration, image architectures and packaging remain engineering work.

## Public HTTPS boundary (D362)

Use deployment-managed HTTPS termination and certificates at a reverse proxy or
load balancer. Axum listens on a restricted internal endpoint; protect the
upstream connection appropriately for the deployment network. No built-in public
HTTPS/certificate-management implementation is required for MVP. Do not prescribe
a particular proxy, load balancer or orchestrator.

Configure the externally visible public base URL explicitly and derive OIDC
callback URLs from that configuration rather than arbitrary incoming host
headers. Accept forwarded client information only through an explicitly trusted
proxy configuration. Serve the web interface and application endpoints under
the same public origin. This boundary accommodates production SaaS ingress and
small self-hosted reverse proxies using the same application architecture.

## Release-managed server migrations (D361)

Design the core server deployment contract for a production software as a
service (SaaS), with simpler self-hosted operation using the same architecture.
Run schema migrations as an explicit deployment step, not as a side effect of
ordinary application startup. The release's immutable artifact provides the
`clacks-admin database migrate` command, which deployment automation can run as
a one-off job. This is
not another independently deployed application service.

Use a separate authorized database role for schema changes. Normal server
processes do not need schema-changing privileges. Use SQLx migration execution,
history, validation and locking rather than parallel migration/version machinery.
These facilities do not establish compatibility between application versions or
make a migration nonblocking.

Use expand–migrate–contract for changes that need staged deployment: apply
schema additions compatible with the running release, deploy compatible
application changes, perform substantial data conversions in resumable bounded
batches, then remove obsolete schema after old versions are retired and the
supported application-rollback window closes. Compatibility covers data written
by both versions as well as query/schema shape, including background work and
existing connections. Exact supported version windows remain release-design work.

Application rollback normally redeploys the previous compatible application
against current database contents. Do not automatically reverse schema
migrations or restore an old database as an application rollback; restoring data
has the separate recovery/loss implications already documented. Migration plans
must address lock waits, expensive operations, transaction boundaries and
appropriate online operations, including their failure/retry behavior. Record
migration execution and outcomes through production operator observability.

Establish the entry point, privilege separation and deployment contract in #11.
Do not build a generic backfill framework, select an orchestrator or require
multiple application replicas merely to establish this contract. Self-hosted
deployment may stop Clacks, run the same migration command, and restart. The
architecture must still support release-managed compatibility and ordering.
See [AWS schema/deployment guidance](https://docs.aws.amazon.com/whitepapers/latest/blue-green-deployments/best-practices-for-managing-data-synchronization-and-schema-changes.html)
and [GitLab staged migrations](https://docs.gitlab.com/development/database/avoiding_downtime_in_migrations/).

## OIDC web sign-in and provider interoperability (D190)

Use a deployment-configured OpenID Connect provider for MVP web sign-in. Clacks integrates through standard discovery/authentication endpoints without requiring a particular provider. The earlier built-in password proposal was not accepted and is withdrawn; no local password implementation is required for MVP.

Checked 16 October 2026: [Tailscale's tsidp documentation](https://tailscale.com/docs/features/tsidp) describes standard OIDC discovery/endpoints and provider-issued identity tokens. Exact tsidp version/configuration and live interoperability still need verification during implementation; documentation inspection is not a completed integration test.

Keep external authentication identity distinct from the Clacks account ID and each enrolled client's identity. [OpenID Connect section 5.7](https://openid.net/specs/openid-connect-core-1_0.html#ClaimStability) specifies the issuer/subject pair (iss, sub) as the stable external identity; display name/email are not stable account identifiers. Bind verified external identities explicitly to the appropriate Clacks account rather than deriving internal identity from mutable labels. D231 permits multiple verified external identities to link to one stable Clacks account, including identities from multiple providers. Each issuer/subject pair is linked to at most one Clacks account. D191 provisions an account during normal sign-in for an unbound identity and otherwise resolves its existing binding; linking an additional identity to an existing account is a separate workflow, not ordinary new-account provisioning. There is no administrator pre-creation or second Clacks approval step. D357/D358 select maintenance commands for role assignment and exceptional identity recovery; the #11 proposal supplies their deployment and transaction boundaries. D232 selects self-service identity-linking/unlinking safeguards. Signing in alone does not grant server-administrator privileges.

The OIDC browser login is separate from D65 desktop authorization and D62 independently revocable client access. D193 separately selects browser-callback desktop authorization for MVP. The long-lived client credential remains to be designed; provider client secrets are not distributed to desktop applications.

## OIDC protocol library (D365)

Use `openidconnect` with asynchronous `reqwest` and Rustls for server OIDC
integration, including standard discovery, authorization-code authentication,
maximum-authentication-age requests, response validation and dynamic client
registration. Connect the library's protocol operations to Clacks account
provisioning/linking and Axum routes rather than implementing the protocol in
Clacks. Configure the HTTP client according to the library's security guidance,
including disabling automatic redirects. D368 separately selects `axum_session`
with its PostgreSQL adapter. Concrete composition and dependency feature
resolution belong to the #11 proposal and implementation plan; no
provider-specific authentication workarounds are selected.

## OIDC Dynamic Client Registration (D363)

Support OpenID Connect Dynamic Client Registration (DCR) for configured identity
providers in MVP, including tsidp. Clacks acts as the
registering OIDC client; this does not introduce an OIDC provider or a public
client-registration endpoint in Clacks. Register the deployment's callback URLs
under D362. This registration identifies the Clacks deployment to its provider;
it is distinct from account provisioning, linked user identities and desktop
enrollment.

Use the standard registration protocol and provider discovery metadata, including
the `registration_endpoint`, and the provider's registration authorization
requirements. Protect any returned client credentials and registration-management
tokens as deployment secrets. D364 selects explicit preconfigured-client and
dynamic-registration modes per configured provider. Dynamic registration runs
through `clacks-admin oidc register`, an automatable deployment-setup operation
using the configured public
callback URL, separately from SQL schema migrations. Persist and protect the
returned registration so application restarts, replicas and releases reuse it.
Registration credential failures or expiration must produce clear diagnostics,
not silently create another registration. The #11 proposal uses a protected
deployment registration file, separate from account-owned service credentials. See
[OIDC discovery](https://openid.net/specs/openid-connect-discovery-1_0.html#ProviderMetadata)
and [OAuth dynamic registration](https://www.rfc-editor.org/rfc/rfc7591.html).

[tsidp documents DCR support](https://tailscale.com/docs/features/tsidp#set-up-an-application-capability-grant)
and requires appropriate Tailscale application-capability grants for its
registration endpoints; this is deployment configuration, not Clacks-specific
authentication behavior.

## Operator command

Use one `clacks-admin` command for `role grant`, `role revoke`, `identity recover`,
`oidc register` and `database migrate`, with standard nested `--help`. This is a
thin interface to production server-library operations, not a general task
runner. Resolve credentials only for the chosen subcommand: migration database
access for schema changes, maintenance database access for role/recovery work,
and provider registration authorization for OIDC setup. Runtime database access
remains separate. Registration requires no PostgreSQL connection; database
maintenance requires no provider or cookie secrets. See the
[credential requirements](account-management-proposal.md#release-and-operator-procedures).

## Server-administrator role assignment (D192)

Manual, out-of-band assignment of the server-administrator role is sufficient for
MVP. D357 selects server maintenance commands to grant or revoke that role for
an existing Clacks account ID. Run these commands with deployment-controlled
database access and record role changes in operator logs. For initial setup, the
first administrator signs in through normal provisioning before the operator
grants the role to that account. Store role assignments in the database rather
than a deployment-configured allowlist of administrator IDs. Use
`clacks-admin role grant` and `clacks-admin role revoke`; the shared operator
command does not change the required maintenance privileges.

No in-application role-management interface or automatic OIDC claim-to-role
mapping is required. Keep role assignment independent of D191 first-sign-in
provisioning: ordinary users do not await role assignment or administrator
approval.

## Browser-callback authorization (D193)

For MVP, the desktop opens the browser for sign-in and client approval, then receives completion through a temporary listener bound to a loopback interface on an operating-system-assigned port (D370). The initiating desktop process receives the callback without installing an application-specific URL handler. The listener exists only for the authorization attempt. Device authorization is a post-MVP fallback option. D371 selects the access-token/refresh-credential model, D378 requires DPoP binding for access and refresh credentials, and D379 selects Huskarl with oauth-as. See [OAuth for native applications, RFC 8252](https://www.rfc-editor.org/rfc/rfc8252#section-7.3).

Both flows require the Clacks client to reach the server. Device authorization can use a browser on the same computer or another permitted device and polls for completion; browser-callback authorization returns completion through the callback. This distinction is not about sync-server reachability. Keep browser sign-in and per-client enrollment/credentials distinct; D193 alone did not settle the ongoing credential model; D371 now selects short-lived access tokens and persistent refresh authorization.

## Desktop access and refresh credentials (D371)

Issue short-lived access tokens with a persistent per-client refresh credential.
An authorized installation obtains a replacement access token after expiration
without repeating browser sign-in, including after an indefinite offline period.
Do not impose refresh-authorization expiration solely because the client has
been inactive. This preserves D60; it does not adopt the browser-session lifetime
for desktop enrollment.

Keep desktop authorization bound to its Clacks account and enrolled client,
separate from provider credentials and browser login sessions. Individual-client
revocation prevents further refresh and authorized access without waiting for an
access token to expire. It preserves other clients and local work under D62,
D145 and D152.

D378 selects persistent DPoP-bound refresh credentials and binds access tokens
as well. DPoP is the standard application-layer proof mechanism defined in
[RFC 9449](https://www.rfc-editor.org/rfc/rfc9449.html). The client proves
possession of its private key when exchanging the authorization code, refreshing
access and presenting a bound access token. A token alone does not authorize
use; verify the proof and its binding to the credential, request and permitted
freshness window, with replay protection shared across application instances.

Ordinary refresh issues a fresh short-lived access token and retains the same
key-bound refresh credential. After an interrupted refresh or lost response,
retry with that credential and a fresh proof. There is no replacement refresh
credential to recover and no rotation-retry grace deadline. D378 supersedes
D372, D373 and D377's rotation-specific behavior. Do not retain recoverable
replacement credentials or cached successful token responses for retry recovery.

Preserve local data and pending comparison evidence during credential recovery;
credential recovery does not by itself establish explicit actor revocation.
Revoked-actor reauthorization and admission remain governed by D145/D152. DPoP
does not protect a client whose private key and credentials are both compromised.

D374 selects opaque access tokens backed by server-side authorization records.
Resolve the presented credential and current enrolled-client authorization through
PostgreSQL. Do not rely on token expiration alone for revocation. Enforcement on
existing synchronization streams and at change admission remains required under
either token format. No access-token signing key is required by this choice.

Use the standard authorization-code flow with required Proof Key for Code
Exchange (PKCE), the loopback-IP/variable-port redirect policy and full DPoP
binding. D379 selects Huskarl for the desktop client and oauth-as for the
embedded authorization server, superseding D375's Oxide Auth selection. Use an embedded Rust integration in the
Clacks server application; a separate authorization-server service is excluded.
Clacks owns account/enrolled-client authorization, SQLx persistence, transaction
boundaries and current authorization enforcement. Prefer the libraries' standard
protocol and HTTP facilities. Keep external-provider sign-in on the existing
`openidconnect` integration and HTTPS termination at the deployment boundary.

D376 selects `keyring-core` with `apple-native-keyring-store` using its `keychain`
feature on macOS and `windows-native-keyring-store` on Windows. Both native
integrations are required during MVP: macOS is the primary development platform.
SQLite retains non-secret credential references, not plaintext enrollment secrets.
Linux desktop release support remains post-MVP.

Refresh-token representation, access-token lifetime, proof-validation parameters,
binding/key lifecycle, interrupted-enrollment handling and concrete integration
remain engineering work. Do not introduce a custom acknowledgment exchange.
No exact token/proof lifetime or additional recovery protocol is selected.
The [client-enrollment proposal](client-enrollment-proposal.md) supplies the
concrete schema, transactions and workflows for review; its engineering defaults
are not additional accepted product decisions.

### Selected libraries and integration costs (D379)

Use Huskarl's standard discovery, PKCE, loopback callback, token acquisition,
refresh, access-token cache and DPoP proof generation. Persist the signing key and
refresh credential through D376's native credential stores. Keep application
credential persistence separate from the library's in-memory access-token cache.

oauth-as 1.0.0 requires a loopback redirect-port correction for D370: permit the
port to vary for an otherwise matching registered loopback IP redirect, retain
the actual requested URI in the authorization grant, and require that exact URI
at code redemption. Preserve host, path and query matching; this is not a general
redirect wildcard. Track [upstream issue #26](https://github.com/MattJackson/oauth-as/issues/26).
Until a compatible upstream release is available, use a pinned, reviewed patch;
selection accepts that bounded maintenance cost, not an unrestricted fork.

Clacks supplies the oauth-as storage integration using its relational PostgreSQL
schema, checked SQLx queries and transaction boundaries. The unpublished upstream
PostgreSQL reference adapter is not selected: its serialized payload schema and
recoverable token storage do not establish Clacks's production storage contract.
D380 selects hashed ordinary lookups and encrypted short-lived code receipts
against the library's existing storage interface.

Direct resource authorization resolves the credential and current account/client
authorization in PostgreSQL. Huskarl's complete opaque-token validator uses HTTP
introspection and does not expose a local lookup callback. Its public lower-level
proof validator supplies cryptographic and structural checks; Clacks must enforce
the proof's key, HTTP method, target URI and access-token hash bindings, scope,
expiry and shared replay exclusion. Use standard URI normalization and library
cryptography. Do not add server-to-itself HTTP introspection to avoid this boundary.
Admission transactions and already-open synchronization streams must still enforce
current revocation; successful handshake authorization is not sufficient.

The selected native crypto backend has no algorithm-specific feature switches.
Using ES256 therefore also brings unused algorithm implementations, parallel
RustCrypto dependency generations and release-candidate RSA/PKCS dependencies.
These are dependency maintenance and build costs, not evidence of a vulnerability
or permission to accept additional algorithms. The evaluated macOS desktop/server
normal/build dependency graph grew from 474 to 523 package/version entries, with
28 new package names; those counts depend on the selected versions and features.
No cold-build or shipped-binary-size benefit/cost has been established. reqwest
0.13 and Rustls's AWS-LC backend were already present in Clacks; they are not new
dependency families introduced by this choice. No OpenSSL backend is selected.

The evaluated versions are Huskarl 0.11.9, huskarl-core 0.10.10,
huskarl-crypto-native 0.11.3, huskarl-reqwest 0.9.3,
huskarl-resource-server 0.11.12 and oauth-as 1.0.0. Resolve concrete pins and
feature choices in the implementation plan, preserving the required behavior.
macOS interoperability establishes feasibility, not Windows credential-store
verification, production transaction correctness or a security audit.

### Credential storage (D380)

Hash ordinary code and access/refresh credential lookup values. Reconstruct the
credential field of an ordinary library lookup result from the current request's
presented value; do not persist another recoverable copy in the token tables.
oauth-as's consumed/replayed code record also contains the issued token strings
so replay can find and revoke the grant. Encrypt those references using D236's
per-account keys protected by a deployment wrapping key. Retain relational
metadata and native lifecycle state rather than serializing entire library
records into the database. Short-lived code-replay evidence is distinct from
the rejected cache of successful refresh responses.

Preserve the existing library policy: a repeated code exchange is rejected and
attempts to revoke the grant issued from that code. A lost initial token response
therefore requires restarting browser authorization. Ordinary persistent refresh
can be retried with the same refresh credential and a fresh proof. Do not change
either policy or add another dependency patch to enforce fully hash-only storage.
OAuth grant revocation does not itself close a synchronization actor: D152's
explicit individual-client revocation remains a separate operation.

The retained receipt can be recovered by an authorized server or someone holding
both its ciphertext and decryption keys. Expiry cleanup removes current rows,
not historical backup copies. This is an accepted bounded exposure, alongside
normal TLS, secret-exclusion, infrastructure and incident-remediation practices;
DPoP still requires the client's private key to use the recovered credentials.
Do not claim protection against complete endpoint or authorized-server compromise.

## Local log and credential protection (D194)

The local log itself does not require separate application-level encryption or an unlock/passphrase workflow for MVP. Directory credentials are sensitive and require protected local storage, separate from ordinary contact data. D96/D97 still require direct offline-capable client lookup and secure credential provisioning; D376 selects `keyring-core` with native macOS and Windows backends for desktop credential storage; service-specific provisioning and lifecycle integration remain capability work. Protect synchronization credentials and server-held service secrets according to their own requirements rather than treating them as ordinary log data.

## Directory authentication failures (D195)

When a directory provider rejects credentials, pause lookups for that provider and show a clear diagnostic. Updated credentials or explicit retry resume lookup. Logging, WS Suite capture, and synchronization continue normally. Contacts lacking enrichment can be populated through D99 user-triggered bulk lookup; do not introduce automatic bulk catch-up after authentication recovery. Exact error classification, transient-failure handling, and retry implementation remain open.

## Account-level directory provider selection (D196)

The selected directory provider is account-level configuration shared by enrolled clients, not a client-local preference. D89 provides QRZ/HamQTH choice; requests remain client-direct under D96. Clients continue using their last received selection when synchronization is unavailable and receive changed selection on reconnect. D197 uses only the selected provider in MVP, defers automatic fallback to post-MVP, and excludes merging results from multiple directory providers. Exact configuration interface and handling of requests in flight during a selection change remain open.

D198 keeps directory location separate from contact location; direct-client lookup and account-level provider selection do not authorize automatic population of QSO location fields. D203 retains/synchronizes separate directory-location snapshots with each QSO; D98 continues to exclude a reusable persistent lookup cache for future contacts.

D199 adds logical location fallback without copying: use recorded QSO location when present, otherwise available directory location. Directory location remains a separate source under D198; D200 includes exports and external uploads; D201 selects whole-location precedence without filling gaps from directory data. D203 retains/synchronizes the separate snapshot with each QSO, including provider and lookup time.

D204 extends D203 snapshots to all supported directory information, including names, retaining provider and lookup time. Recorded QSO information takes precedence in logical fallback; directory requests and snapshot updates do not populate recorded QSO fields.

## Operator observability and user-facing security history (D230)

Full server tracing, structured logs and metrics are MVP requirements, including account-security events without secrets. These serve server operators and are separate from account-user diagnostics. A retained account-scoped security-event history view in the web interface is post-MVP. D66 upload diagnostics and D142 synchronization status remain MVP. See [server observability](observability.md); exact instrumentation, export and backend integration remain engineering work.

## Account integration constraints

Enforce account context across requests, existing synchronization streams, queries,
jobs, credentials and diagnostics. Login binding uniqueness, recent-authentication
proof, callback/session binding, per-client credential renewal and interrupted
enrollment must preserve the accepted identity boundaries.

Credential integration covers authenticated account/credential binding, rotation,
key-version retention, protected client storage, and server credential removal
during in-flight work. Preserve public metadata and submission history; removal
cannot make disconnected clients forget secrets already received. Directory
requests distinguish transient failure from rejected credentials and obsolete
in-flight provider changes. Detailed mechanisms belong to each capability's
specification; the scope and selected frameworks remain settled.

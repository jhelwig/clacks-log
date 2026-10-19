# Accounts and client enrollment

D62-D63 establish individual-client revocation and independent account isolation in minimum viable product (MVP). D64’s earlier statement was permissive scope language; D191 now selects automatic account provisioning on first successful OpenID Connect (OIDC) sign-in. D65 selects browser-based client authorization and a deployment-provided web user interface (UI). D190 selects configurable OIDC for web sign-in. D191 selects first-sign-in provisioning, while D231 clarifies that multiple external login identities can link to one independent Clacks account; D192 accepts manual out-of-band administrator assignment. D193 selects browser-callback desktop authorization for MVP, with device authorization as a post-MVP fallback. Exact protocol integration, assignment mechanism, and binding storage remain to be designed.

## Web-interface architectural scope (D222)

The web interface is intended to grow beyond the MVP account, setup and diagnostic workflows to logbook browsing/editing, reporting and potentially logging. D223 selects Leptos/Axum with server-rendered initial pages and Rust/WebAssembly browser interactivity through hydration, evaluated against that broader direction. This does not expand MVP scope, require browser offline operation or desktop feature parity. Exact component versions, styling and layouts remain open.

## Accepted boundaries

One server can host multiple independent accounts in MVP. Each account's log data, profiles, enrolled clients, and external-service credentials are isolated. Apply that boundary to storage access, synchronization negotiation/streams, job execution, credential access, diagnostics, and client revocation. Guessing another account's object identifier must not authorize access. Exact enforcement mechanisms and storage layout remain unselected.

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

## Self-service login-identity management (D232)

An authenticated user can add another login identity from a deployment-configured provider. Require recent authentication to the existing account and authenticate the additional identity. Link it to that account rather than running D191 new-account provisioning. If the identity is already bound to a different Clacks account, refuse the link; do not transfer ownership or merge accounts automatically.

Unlinking requires recent authentication through an identity that will remain linked. Prevent removing the last login identity through ordinary account settings. Account data and enrolled clients remain attached to the stable internal account under D231. D233 makes documented administrator-assisted recovery sufficient for MVP when no linked identity is accessible; automated self-service recovery is post-MVP.

Exact authentication-freshness thresholds, callback/session binding, atomic uniqueness enforcement and interface layout remain specification work. [OWASP authentication guidance](https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html#require-re-authentication-for-sensitive-features) supports reauthentication for sensitive account changes; D232 selects the Clacks workflow rather than a particular authentication library.

## Recovery without an accessible login identity (D233)

A documented administrator-assisted procedure is sufficient for MVP. Establish ownership of the existing Clacks account out of band and explicitly link a newly verified external identity to that account. Preserve its internal ID, data, service credentials and enrolled clients. Record the recovery in operator security logs under D230. Respect D231’s uniqueness rule for external identity bindings.

Automated self-service account recovery is post-MVP. This exceptional procedure does not replace D232 self-service linking/unlinking for users who can still authenticate. Exact ownership verification, trusted administration access and administration tooling remain specification work; no particular command or database-editing procedure is selected.

## Browser-based client authorization (D65)

The desired experience follows Tailscale/GitHub command-line interface (CLI): the client initiates authorization, the user authenticates in a browser through the deployment's web UI and authorizes the client, and the client obtains its own revocable authorization for ongoing synchronization. Account credentials are not entered into the desktop client. D193 selects browser-callback authorization for MVP: the desktop opens the browser and receives completion through a callback. Device authorization is a post-MVP fallback option.

The deployment provides the Clacks web UI and enrollment/management endpoints within the single Clacks application containing synchronization and background workers, alongside PostgreSQL (D213/D223). The external OIDC identity provider supplies authentication; it is distinct from the Clacks management endpoints. D190 selects configurable OIDC web sign-in without requiring a particular provider or local password registration. D191 selects automatic local provisioning after successful configured-provider authentication.

D190 selects OIDC for web sign-in. D193 selects the browser-callback flow for MVP. Exact standard protocol/library integration, callback mechanism, per-client credential format/storage/renewal, binding storage mechanics, and interrupted-enrollment handling remain open. Use standard supported mechanisms once selected, not a bespoke OAuth-like token exchange inferred from the user experience (UX) analogy.

Reference: [GitHub CLI login](https://cli.github.com/manual/gh_auth_login) documents browser authentication; [OAuth device authorization, Request for Comments (RFC) 8628](https://www.rfc-editor.org/rfc/rfc8628.html) specifies a device/user-code browser authorization flow. D193 defers device authorization to a post-MVP fallback; it is not the selected MVP flow.

## Accepted MVP web-UI scope (D66)

Provide account sign-in, D232 self-service login-identity linking/unlinking, client authorization, enrolled-client listing/revocation, administrator account creation, external-service configuration, and upload diagnostics in the MVP web UI. Administrator account creation is restricted to server administrators; account-specific management and diagnostics follow D63 isolation.

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

## OIDC web sign-in and provider interoperability (D190)

Use a deployment-configured OpenID Connect provider for MVP web sign-in. Clacks integrates through standard discovery/authentication endpoints without requiring a particular provider. The earlier built-in password proposal was not accepted and is withdrawn; no local password implementation is required for MVP.

Checked 16 October 2026: [Tailscale's tsidp documentation](https://tailscale.com/docs/features/tsidp) describes standard OIDC discovery/endpoints and provider-issued identity tokens. Exact tsidp version/configuration and live interoperability still need verification during implementation; documentation inspection is not a completed integration test.

Keep external authentication identity distinct from the Clacks account ID and each enrolled client's identity. [OpenID Connect section 5.7](https://openid.net/specs/openid-connect-core-1_0.html#ClaimStability) specifies the issuer/subject pair (iss, sub) as the stable external identity; display name/email are not stable account identifiers. Bind verified external identities explicitly to the appropriate Clacks account rather than deriving internal identity from mutable labels. D231 permits multiple verified external identities to link to one stable Clacks account, including identities from multiple providers. Each issuer/subject pair is linked to at most one Clacks account. D191 provisions an account during normal sign-in for an unbound identity and otherwise resolves its existing binding; linking an additional identity to an existing account is a separate workflow, not ordinary new-account provisioning. There is no administrator pre-creation or second Clacks approval step. D192 accepts manual out-of-band administrator role assignment, including initial administrator setup; exact administrator-assignment and D233 exceptional account-recovery mechanisms remain specification work. D232 selects self-service identity-linking/unlinking safeguards. Signing in alone does not grant server-administrator privileges.

The OIDC browser login is separate from D65 desktop authorization and D62 independently revocable client access. D193 separately selects browser-callback desktop authorization for MVP. The long-lived client credential remains to be designed; provider client secrets are not distributed to desktop applications.

## Server-administrator role assignment (D192)

Manual, out-of-band assignment of the server-administrator role is sufficient for MVP. No in-application role-management interface or automatic OIDC claim-to-role mapping is required. The exact deployment configuration/command method remains open; this does not mandate a specific tool or hand-editing the database. Keep role assignment independent of D191 first-sign-in provisioning: ordinary users do not await role assignment or administrator approval.

## Browser-callback authorization (D193)

For MVP, the desktop opens the browser for sign-in and client approval, then receives completion through a callback. Device authorization is a post-MVP fallback option. Exact callback transport and standard protocol/library integration remain open; a temporary loopback listener is an option, not a separately accepted requirement. See [OAuth for native applications, RFC 8252](https://www.rfc-editor.org/rfc/rfc8252).

Both flows require the Clacks client to reach the server. Device authorization can use a browser on the same computer or another permitted device and polls for completion; browser-callback authorization returns completion through the callback. This distinction is not about sync-server reachability. Keep browser sign-in and per-client enrollment/credentials distinct; D193 does not settle credential format, renewal, or expiry policy.

## Local log and credential protection (D194)

The local log itself does not require separate application-level encryption or an unlock/passphrase workflow for MVP. Directory credentials are sensitive and require protected local storage, separate from ordinary contact data. D96/D97 still require direct offline-capable client lookup and secure credential provisioning; exact operating-system credential-store integration remains open. Protect synchronization credentials and server-held service secrets according to their own requirements rather than treating them as ordinary log data.

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

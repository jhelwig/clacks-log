# Desktop enrollment: schema, transactions and workflows

The client-enrollment contract for [issue #12](https://github.com/jhelwig/clacks-log/issues/12).
The completed implementation plan and verification are retained in
[PR #57](https://github.com/jhelwig/clacks-log/pull/57) and Git history.
[accounts.md](accounts.md) owns accepted account behavior;
[synchronization-model.md](synchronization-model.md) owns admission and recovery.

OpenID Connect is OIDC; Proof Key for Code Exchange is PKCE; Demonstrating Proof
of Possession is DPoP. SQL means Structured Query Language; SQLx is the selected
Rust database library. Compare-and-swap is CAS. A grant family
is oauth-as's non-secret identifier linking credentials issued from one approval.
It is distinct from a Clacks client actor, account and application registration.

## 1. Accepted requirements and engineering defaults

Carry forward D56, D60, D62–D63, D65–D66, D143, D145/D152, D155–D156,
D193, D236 and D370–D380:

- Temporary loopback callback; browser approval through the existing Clacks web
  account; required PKCE and DPoP; opaque short-lived access tokens and persistent
  key-bound refresh credentials. No rotation, retry-grace protocol or idle expiry.
- Huskarl client and embedded oauth-as issuer, with upstream loopback-port
  support in oauth-as 1.0.1. Keep external-provider sign-in on the existing OIDC
  integration.
- Immediate individual-client revocation without deleting shared contacts or
  affecting other clients. An inactive authorized client remains eligible.
- Preserve local work and comparison evidence. Explicitly revoked actors stay
  closed; their unaccepted work requires manual reconciliation after reapproval.
- One account synchronization scope. Enrollment into an existing account starts
  with an empty account workspace, not silent merging/relabeling of independent
  local data. No new multi-account desktop switcher or clone detector.
- Hash ordinary credential lookups and encrypt code-replay receipt secrets.
  Preserve oauth-as's existing replay policy and its limitations.

Engineering defaults, rather than additional product requirements:

| Item | Proposed default and rationale |
| --- | --- |
| OAuth registration | One server-owned public application registration, `clacks-desktop`, allowing only authorization-code and refresh grants and scope `sync`. Per-installation authority comes from the grant/enrollment association, not this public ID. |
| Callback | Huskarl's OS-assigned loopback listener and fixed callback path `/oauth/callback`; use `127.0.0.1` or `[::1]` IP literals and upstream variable-port support. |
| Access/code lifetime | Five-minute access tokens and oauth-as's 60-second authorization-code lifetime. Browser sign-in/approval has its own existing pending-flow lifetime. |
| Token-endpoint proofs | Keep oauth-as's five-minute maximum proof age and one-minute future-clock allowance; do not promise configurable proof windows or server nonce support absent from this version. |
| Protected-request proofs | Huskarl validator: 60-second maximum age and five-second clock leeway, replay retention covering that complete acceptance window. These values need not equal issuer constants. |
| Proof algorithm | ES256: Elliptic Curve Digital Signature Algorithm on P-256 with Secure Hash Algorithm 256-bit (SHA-256). The availability of other compiled algorithms does not enable them. Require `dpop_jkt` for the Clacks enrollment route; Huskarl already supplies it. |
| Lookup digest | SHA-256 over a fixed credential-kind prefix and exact UTF-8 credential bytes. Separate domains for codes, access and refresh. PostgreSQL `bytea`, 32 bytes, unique per kind. No slow password hashing, pepper, hash-as-credential convention or normalization. |
| Secret encryption | RustCrypto `aes-gcm`: Advanced Encryption Standard with a 256-bit key in Galois/Counter Mode (AES-256-GCM), library/OS nonce generation with the required per-key nonce uniqueness, authenticated account/object binding. No custom cryptography or new external service. |
| Cleanup | Once per minute, bounded database batches. Expiry is checked on use even when physical cleanup is delayed. Active nonexpiring refresh authorization is excluded. |
| Unfinished issuance | Five-minute deadline from first issuance for a provisional, not-yet-activated family. A delayed final transition must fail if that provisional state was collected. This is not an expiry for an enrolled client. |
| Workspace destination | A new account workspace under the managed data directory, with a directory override. Keep the current workspace intact; do not repurpose the running database. |

No schema/protocol/crypto compatibility guarantee is inferred from package names.
The selected libraries are described in accounts.md; Cargo.lock records exact
dependency resolution. Production oauth-as features are
`dpop`, `jwt-p256`, `http`; defaults are empty. Leave other grants, desktop dynamic
registration, JSON Web Token (JWT) access tokens and refresh response caching disabled.

## 2. Production boundaries

- `clacks-server` owns OAuth HTTP wiring, the oauth-as Storage implementation,
  PostgreSQL schema/transactions, key envelopes, enrollment and revocation.
- `clacks-web` extends the actual Leptos application with desktop approval and
  account-scoped client listing/revocation, using existing login extraction and
  browser mutation-origin protection.
- `clacks-desktop` owns the browser/loopback workflow, native credential store,
  worker lifecycle and workbench presentation. Huskarl owns protocol construction,
  state/issuer checking, PKCE, proof generation, refresh and access-token caching.
- `clacks-client-store` owns durable binding/recovery operations and ordinary
  client state. `clacks-sync` owns existing membership/admission/rejection models.

Use focused modules in these libraries and thin binaries. No disposable host,
in-memory production authorization store, parallel session implementation,
introspection-to-self request, generic workflow engine or new acknowledgment
protocol. Fixed SQL stays in checked query files with offline metadata.

#12 delivers real enrollment and admission persistence that #14 extends with
record transport and complete shared-state installation. Do not simulate a
successful synchronization or replace these components later.

## 3. Relational server schema

Extend the existing `clacks` schema through release-managed migrations. Preserve
#11's accounts, identities, sessions and account-row locking. Clacks entity IDs
use canonical native text; issuer-owned identifiers retain the library's
representation. Counters use PostgreSQL `BIGINT` within the checked Clacks domain,
instants `timestamptz`, digests `bytea` and complete encrypted values `text`.
Account-owned references are account-qualified. Table names below identify the production schema.

| Table | Fields, references and constraints |
| --- | --- |
| `desktop_oauth_clients` | Public `client_id` primary key, display name; child tables for allowed grants, scopes and exact registered redirects. Server-wide public metadata; no desktop client secret or user account authority. |
| `desktop_clients` | `(account_id, actor_id)` primary key, user-visible label, authorization time, nullable closure time, creation and closure synchronization positions. References accounts and actual membership positions. Never delete shared records through this table. |
| `desktop_grants` | `(account_id, family_id)` primary key, application `client_id`, nullable `actor_id` until initial exchange completion, grant-established time, proof-key thumbprint, state `provisional`, `active` or `revoked`, and provisional cleanup deadline. Once associated, a family never changes account or actor. Multiple historical grants can name one actor. |
| `desktop_authorization_codes` | Code digest primary key, account, application client, exact granted redirect and explicit-redirect flag, subject, S256 PKCE challenge, key thumbprint, issued/expiry times, state `issued`, `consumed` or `replayed`. Nullable complete versioned receipt envelope with a generated account-key reference. Ciphertext contains only the optional issued access/refresh strings required by oauth-as, not the entire record. Child tables hold scopes/resources. |
| `desktop_code_approvals` | Code digest primary key, verified account, approval purpose `new`, `recover_active` or `replace_closed`, optional target actor, client label and code expiry. Immutable host context survives the library's remove/put-back transitions; it is not another browser pending-flow store. |
| `desktop_access_tokens` | Access digest primary key, account/family foreign key, issued/expiry times and child scope/resource relations. Client, proof key and original authorization time come from the grant family; the owning account is the subject. No recoverable access-token column. |
| `desktop_refresh_tokens` | Refresh digest primary key, account/family foreign key, nullable expiry, library active/spent state and child scope/resource relations. Immutable client, proof-key and original authorization metadata come from the grant family; the owning account is the subject. Reuse only writes active records; reject unexpected retry-response payloads. No recoverable credential column. |
| `oauth_revocations` | Library barrier scope and identifier, recorded-at and retention deadline. Unique by scope/identifier with the library's merge rules. Family identifiers include the account qualification. Keep this separate from permanent actor closure. |
| `oauth_replay_claims` | Exact library-namespaced replay key primary key and expiry. Shared by all issuer instances. |
| `resource_replay_claims` | Proof-key thumbprint plus proof ID primary key and expiry; all protected-request instances share the same table. Keep issuer/resource namespaces distinct. |
| `account_encryption_keys` | `(account_id, key_version)` primary key, complete versioned wrapping envelope for the random 256-bit account key, generated deployment-key reference, creation time and active designation. Account-owned encrypted objects reference their key version. |

Scope/resource child rows are ordered only where library equality needs order;
map the selected public record fields exactly. Do not add unsupported consent,
device-flow, pushed-request or token-exchange tables. Required unused Storage
methods fail explicitly instead of pretending to persist unsupported records.
Public registration updates can use the existing trait's conditional-update
semantics without opening a dynamic-registration endpoint.

### Synchronization admission persistence

Persist `sync_accounts` with account primary key, current timeline/revision,
current baseline ID and membership generation. `sync_baselines` holds
account/baseline ID, root timeline and selected membership generation;
`sync_baseline_coverage` holds baseline/timeline/revision coordinates. Include
the root timeline identity needed to materialize the initial lineage. These are
the real initial subset of `AccountMetadata` and baseline persistence, not a
second synchronization representation. #14 adds retained ancestry boundaries,
additional baseline interpretations and installed-coverage relations as needed.
`desktop_clients` supplies `MembershipSnapshot`, with creation/closure positions
and nullable acknowledgment reference. Its acknowledgment stays absent in #12;
the referenced checkpoint/coverage tables arrive with actual receipt in #14.
Reuse the logical schema families in client-store.md; the server does not copy
SQLite's physical row IDs or store JSON copies of model structs.

Initialize an existing #11 account lazily under its account-row lock before its
first enrollment, using `AccountMetadata::initialize`. Publish membership changes
under the actual head and generation conditions. A new member has no durable
acknowledgment; a browser callback and token response are not installed coverage.
Closed actors retain the metadata required by the existing compaction rules.

#14 adds accepted records, transfer manifests, journals and complete installation,
using these same heads/membership rows. It installs the existing native default
group data as ordinary server-authored account records before exposing shared
state; #12 does not claim that reference data or an initial snapshot was received.
Accounts created after that integration initialize shared data during provisioning.

### Encrypted receipts and key handling

Use D236's independent random account keys and deployment wrapping key, distinct
from cookie keys and provider secrets. Provision the first account key under the
account-row lock. Secret-key files are operator-managed, outside PostgreSQL and
images; load versioned 32-byte wrapping keys with one active write version and
retain referenced old versions for supported restores. Missing/wrong keys are
explicit failures, never an instruction to generate replacement keys over data.

Use authenticated encryption for the key envelope and code-receipt secret pair.
Bind the receipt to its account, code digest, purpose and format version through
associated data. Preserve that binding when Consumed becomes Replayed. The
nonce and ciphertext belong to one complete stored encrypted value and are never
telemetry. RustCrypto AES-256-GCM uses a fresh OS-random 12-byte nonce for each
encryption and appends its 16-byte authentication tag to the ciphertext. Store
these bytes with their codec/key metadata in the compact versioned envelope
defined by [credential protection](credential-protection-proposal.md#4-stored-objects-and-encoding).
No nonce counter is coordinated across application instances.

Receipt metadata remains queryable and relational. On read, restore the code
string from the presented code; decrypt the token-reference pair only when the
library requests that code record. On ordinary token reads, materialize the
credential string from the presented input, not another stored copy. Encrypted
receipts remain until code expiry cleanup, including delayed cleanup; backups
can retain them longer. There is no additional code-replay or hash-only patch.

Grant lifecycle decoding distinguishes provisional approval with a deadline,
active enrollment with an actor, and revocation retaining either origin. Access
and refresh reads use separate projections with their actual required fields;
access issuance is never represented as a missing refresh field. Incoming library
records must agree with the immutable family metadata before token persistence.
Current actor admission, expiry, proof binding and revocation remain live checks.

## 4. HTTP and browser boundary

| Route | Authority and operation |
| --- | --- |
| Standard authorization-server metadata | Public configured issuer/endpoints and supported capabilities from oauth-as. No provider credentials or account metadata. |
| `GET /oauth/authorize` | Validate through oauth-as, require the native public-client profile and key binding, then use existing browser login/approval UI. |
| `POST /oauth/approve` | Current cookie-backed Clacks principal and same-origin protection; approve/deny the retained validated request. Recheck account and any recovery target under lock. |
| `POST /oauth/token` | oauth-as standard handler, required DPoP and grant-specific checks; no browser-cookie or browser-origin requirement. |
| `POST /oauth/revoke` | Standard oauth-as token revocation, scoped to the grant/token; it does not close a Clacks actor. |
| `GET /api/client` | DPoP-protected ordinary identity read: verified server account, enrolled actor and non-secret status. No acknowledgment or activation side effect. |
| `GET /account/clients` | Existing browser principal; list own clients and actual available diagnostics. |
| `POST /account/clients/{actor}/revoke` | Existing principal and browser same-origin protection; explicit individual-client closure. |

Mount token/protected API routes separately from #11's cookie-mutation-origin
middleware. That middleware currently rejects origin-less POSTs and cannot wrap
native token calls. Do not disable origin checks for existing browser mutations.
oauth-as's metadata builder advertises device authorization and client-credentials
grants even when the one registered native client does not allow them. Build the
public metadata response from its public metadata value and limit advertised
grants/endpoints to the routes actually mounted for Clacks. Omit device and HTTP
introspection endpoints rather than advertising dead routes or implementing them
to satisfy discovery. This is HTTP composition, not another issuer patch.
Keep standard no-store/referrer headers and trace route templates without queries.
Use the configured public origin behind Caddy, not caller-supplied forwarding
headers, to establish proof targets and callback/metadata URLs. Production is
HTTPS; explicit existing development HTTP permission is not a production default.

Approval context uses normal `axum_session` entries, keyed by the existing
authorization attempt's correlation value with bounded expiry. Retain validated
OAuth request, proposed label and requested recovery target; those hints confer
no account authority. Sign in through existing OIDC facilities and resume this
specific action without creating a new identity/session protocol. Before approving,
validate again and recheck the actual login/account. No per-device provider secret.

## 5. Transaction mapping

No database transaction spans browser interaction, HTTP calls or native storage.
Use account-row locking in the same order as #11; afterward lock affected client,
grant and code rows consistently. Always derive account authority from verified
login or stored credential/enrollment association, never a request account alone.

### 5.1 Issue a code

Use oauth-as's actual issue/put-code operation. Before returning its browser
redirect, commit the Clacks approval purpose, verified account, target actor and
label in `desktop_code_approvals` under the account lock, rechecking current login and
target eligibility. If attaching context fails, do not deliver the redirect.
An undelivered code with no complete approval context cannot activate a client.
The immutable approval row is keyed by the same code digest and expires with
that code; it survives a library take and is not another browser-flow registry.

### 5.2 Redeem a new enrollment code

`take_authorization_code` is an atomic delete-and-return, including child data
and encrypted receipt values. `desktop_code_approvals` remains separate because
the public returned library record has no fields for host approval context.
Later put-back/CAS operations resolve that immutable row by the record's code
digest. No thread-local state, hidden token convention or extended library record
is needed. Coordinate take/put-back/cleanup through ordinary row transactions.

The library writes Consumed with no credential references before issuance. Token
storage writes metadata and a provisional family: no resource access or refresh
is granted from a provisional family. `put_token` and `put_refresh_token` enforce
the library revocation barriers within their own write transactions.
These callbacks do not receive the authorization code ID. Do not guess it from
account, key or timestamps. Only the final code transition can reliably associate
that code's approval with its issued family through the supplied token strings.

Use the final `compare_and_swap_authorization_code` call as the enrollment commit
point. Under the account/code/grant locks:

1. Check the expected Consumed-with-empty-references state and retained approval.
2. Resolve the newly issued records through their digests and verify matching
   account, subject, application client, key, family and approved scope/resources.
3. Allocate the new actor for a new enrollment, or select the permitted recovery
   outcome below. Create/update actual membership and advance head/generation as
   needed. Bind the family to that actor and mark it active.
4. Encrypt the token-reference pair and save the consumed receipt conditionally.
5. Commit together, then return success to oauth-as so it may return credentials.

This is application work inside the selected storage callback, not a library
patch or new acknowledgment endpoint. No shared records or installed checkpoint
are invented. A failed conditional transition returns false; the library performs
its existing issuance cleanup and does not return a successful token response.
Unactivated provisional families and their token rows are cleaned after their
own five-minute issuance deadline, without an assumed code link. A
final transition after collection fails instead of recreating the family. These
are not indefinitely offline enrolled clients. Once activation committed,
loss of the response does not silently un-enroll the actor or start an idle timer.

### 5.3 Refresh and authorization

For Reuse refresh, lookup hashes the presented value and returns the active,
currently eligible stored grant plus the input string. Public-client key binding,
scope/resource narrowing and proof checks remain oauth-as behavior. A new access
token write atomically checks current actor/family authorization and revocation
barriers, preserving the existing family and refresh credential. Invalid requests
may invoke the library's refresh-record put-back path; that path must not resurrect
revoked state or overwrite host-owned eligibility.

Resource authorization checks the actual token/family/account/actor association,
expiry, scope and DPoP bindings, and claims resource replay identity atomically.
Database failure is unavailable, not successful anonymous access or unknown-token
success. `GET /api/client` is this same production boundary, not a fixture.

Later change-admission transactions recheck membership while committing the change,
ordered against revocation through the same account lock. #14 must also close or
deny work on already-open WebSocket sessions after revocation, rather than relying
only on token expiry or a handshake. Notifications may accelerate closure but are
not admission authority. There are no WebSocket sessions to simulate in #12.

### 5.4 Explicit client revocation

Lock account and target member; recheck browser authority; plan with
`clacks_sync::membership::revoke`. Commit the closure, account head and membership
generation, invalidation of all actor-associated grant families/credentials and
required barriers together. An already-closed actor stays closed and is an
idempotent result. Other actors and shared records are untouched.

Ordinary oauth-as family/token revocation, including code-replay containment,
invalidates authorization but does not close membership. A shared OAuth client
registration's deletion is a deployment-wide operation, not the implementation
of the account UI's individual-client Revoke action.

### 5.5 Replay, conditional operations and cleanup

Preserve the complete existing code state machine, receipt references, conditional
updates and containment attempts. Initial-response loss followed by code retry
still causes `invalid_grant` and attempts grant revocation; restart browser
authorization. Preserve library event truth, including incomplete containment.

Token/code hashes apply at every insertion, lookup, delete, conditional transition
and compensation call. Replay claims use database uniqueness, not read-then-write.
Physical cleanup uses expiry metadata; keep active nonexpiring refresh records
and retained synchronization evidence. Do not sweep actor membership by last use.

## 6. Desktop enrollment and native persistence

### New enrollment

The workbench offers Connect to server. Enter the server origin and a human
client label; show which server will be opened. Use normal application URL
validation and explicit development HTTP policy. Generate and persist the signing
key before opening the browser, using `keyring-core` and the selected native
macOS/Windows backend. An entry uses a random opaque reference, not a token/key
in a command argument, configuration file or log.

Huskarl owns discovery, callback listener, PKCE, state/issuer validation, code
exchange and proof construction. Cancellation stops the local attempt and listener;
it does not erase local contacts or imply that an already committed remote
approval was canceled. Do not retry a previously consumed code as a recovery API.

After a successful exchange, persist the usable refresh credential with its key,
issuer/client metadata and Huskarl key-binding information in the same native
bundle. Call the protected identity read to obtain the authoritative account/actor.
Check it against the intended existing account for recovery; a mismatch leaves
the existing database binding unchanged and reports the actual account selected.

For first enrollment, create an empty store with that verified server AccountId
using `ClientStore::create`, then install `EnrollmentBinding`. Never seed standalone
definitions here, remap existing account IDs or relax the populated-workspace
guard. The ordinary desktop creates local definitions at startup, so even its
otherwise-unused current standalone database is not this empty enrollment target.

Proposed destination: `data_local_dir()/accounts/<AccountId>/<ClientActorId>`
unless a directory was selected. Show it before creation and use existing
workspace ownership/activation. Open the new workspace through the production
desktop entry point with `--workspace`; the old workspace remains available and
unchanged. This adds no account registry or multi-account switcher. Existing files
are never overwritten or accepted merely because their path looks right.

Bind SQLite only after the native bundle is durably saved. Use the existing exact
prior-binding guard and one immediate transaction. SQLite stores only the native
reference plus non-secret server/account/actor values. Request the actual initial
state through #14. Enrollment completion does not establish synchronization
progress; distinguish enrolled/awaiting initial synchronization from received
account state.

### Interrupted local completion

Native storage and SQLite are separate transactional systems. Keep the small
non-secret pending-attempt context in the initiating workspace's existing
local-state facility, with native reference, origin, proposed destination and
expected binding; do not copy secrets into SQLite to make a distributed
transaction. Only one enrollment attempt per initiating workspace runs at once.
The local attempt is one versioned document with distinct first-enrollment and
recovery alternatives. Recovery carries its existing destination and prior
binding together. First enrollment can select or correct an uninstalled
destination; a bound phase always carries its committed destination. Phase
transitions record completed native-store and database operations; they do not
assert those effects merely because an in-memory value was constructed.

This is local completion state, not a server acknowledgment protocol.

On retry/restart, a saved native credential can refresh and read its actual
identity, then finish the guarded binding. A missing/unusable bundle requires
browser authorization again. The saved-attempt dialog offers explicit
**Authorize again in browser**, preserving an already installed destination and
its actor hint while saving a fresh native key. It never turns a network failure
into an automatic browser loop. A database commit failure preserves the saved
credential for retry and the previous binding. After a replacement binding commits,
remove the obsolete native entry; failure to remove it is reported/retryable,
not a reason to roll back committed data or close the actor.

Do not create an empty destination account until its server identity is known.
Restarting the initiating workspace can resume its saved attempt through the
native reference. After binding the destination, clear that initiating state;
if cleanup was interrupted, recheck the destination's exact saved binding before
retrying and never allocate another actor merely to finish local completion.
No installation registry or public test-only API is needed.
A first-enrollment destination rejected before binding can be corrected through
**Use this destination**, then resumed with the same saved native credential.
`NeedsDestination` records that known pre-binding rejection. A completed or
uncertain binding is not permission to relocate the workspace. Invalid directory
input is checked before creating the initial key, and a recorded completed
binding is never downgraded merely because completion is retried.

An abandoned committed server enrollment remains visible
in the account client list and can be explicitly revoked; no inactivity expiry.

Before credentials have been saved or a destination bound, the connection dialog
also offers **Discard saved attempt**. Availability and removal check the native
entry as well as SQLite's phase: a saved refresh credential may precede the phase
update after an interruption. An unreadable entry does not establish that removal
is safe; an already absent entry can be cleared as unfinished local state.
Discard removes only its uninstalled native key and local attempt, allowing
correction of a mistaken server address. It
does not revoke any authorization already approved remotely. Installed bindings
and attempts with saved credentials must use completion recovery instead.

The session owns terminal status publication after joining the enrollment worker.
Progress may update while it runs, but retry actions become available only after
the previous task has left admission. An occupied destination is activated using
the existing asynchronous activation exchange; no nested runtime is started.

## 7. Reauthorization and revoked work

| Verified server situation | Proposed workflow preserving the existing contracts |
| --- | --- |
| Credential unusable, existing actor still authorized | Browser approval explicitly identifies that account/client. Issue a new family/key-bound credential for the same actor; invalidate replaced grants without closing membership. Local pending work retains normal eligibility. No causal ID change merely because credentials were lost. |
| Actor explicitly closed | Browser clearly shows reauthorization creates a new actor. Never reopen the closed identity. Preserve local data and quarantine formerly pending/prepared work from automatic submission. |
| Actor absent from restored server state | Browser approval creates a new actor, as for an unknown/retired identity. Do not infer that old local authors are authorized or silently rename their vectors. Retain accepted evidence for normal recovery. |
| Account differs | Do not bind or merge the current workspace. Explain the mismatch and allow restarting sign-in. A different account requires a distinct empty account workspace. |
| Server unavailable | Preserve local operation, credentials, pending edits and diagnostics. Retry later; unavailability is not revocation. |

The same-actor recovery outcome is an engineering interpretation of D378's
credential/actor distinction; it does not expand D152's closed-actor admission.
Browser approval is account-owner authority, not a claim that the desktop has
proved its lost key. Show client label/ID and the replacement consequence before
approval. Do not use hostname fingerprints or infer clones.

Fresh-actor reauthorization commits the new binding and the hold on old work
together in SQLite. Extend existing actor-admission review storage/operations;
use `local_review` reason `actor` once shared observations permit ordinary review.
Before initial catch-up, a nullable former-actor hold on the existing
`client_record` rows marks exactly the pending/prepared set at replacement.
Preparation cannot publish held rows, including later edits to those same rows,
until explicit review resolves them. This adds an admission condition to the
existing rows, not a second copy of pending work. No old prepared bytes/counters
are relabeled.

After real shared state arrives, use the existing pending/conflict review to
explicitly retain the shared version or reconcile local values against it. Only
the resolved result can be prepared under the new actor. Accepted observations,
service evidence and comparison originals remain intact; unrelated new work can
continue under the new actor. No new Restore/Clone capability is added.

## 8. Integration, deployment and verification

Use #11's release migration command, runtime role and SQLx metadata workflow.
Extend role grants for the new production tables; runtime still performs no DDL.
Add protected versioned wrapping-key file configuration to the normal server and
document literal key format, generation, permissions, multi-instance sharing,
backup/restore and startup diagnosis in the implementation deliverable. Missing
keys needed by existing rows fail clearly. Do not alter the persistent personal
development environment while specifying or testing this capability.

Expose significant enrollment/credential replacement/client closure at info
with account/actor IDs and bounded outcome categories. Expected proof/token
refusals are structured events at the appropriate boundary; database/decryption
failures remain distinguishable from invalid credentials. Map oauth-as audit
hooks to the existing telemetry approach without duplicating its state machine.
Use default tracing targets, no generic lifecycle wrapper. Exclude keys, tokens,
proofs, authorization URLs, code/state/verifier values and decrypted receipts.
Metric labels use bounded operation/outcome categories, never account/client IDs.

Verification exercises Clacks integration rather than library internals:

- Two clients under one account and a second account: actual scoped requests,
  individual closure and unchanged unrelated access/data; membership and credential
  writes cannot resurrect an explicitly closed actor.
- Real browser approval/code exchange wiring, relational hashed lookup and
  encrypted receipt reconstruction through the standard issuer. Replay retains
  the library's observed grant-revocation consequence; no dependency RSA/JWT tests.
- Separate native-store reload on macOS and Windows, guarded SQLite binding,
  missing local credentials, saved-credential/uncommitted-binding restart, and
  wrong-account recovery without data remapping. Native acceptance is documented
  separately from what can be tested on the current host.
- Lost refresh response followed by a fresh proof; no recoverable replacement
  response cache. Shared replay storage is exercised through actual requests
  reaching separate application instances, not a database uniqueness test alone.
- Fresh-actor recovery preserves originals/prepared data and blocks automatic
  old-work submission; explicit existing review releases only the resolved work.
- One representative observability scenario verifies truthful committed outcomes
  and secret exclusion without repeating the functional suite for telemetry.

Use disposable PostgreSQL and native-store entries for integration tests; preserve
the real development environment. No injected database triggers, manufactured
schema failures, alternate production interfaces for tests, Python tooling or
coverage target. Assert the Clacks consequence of each scenario.

Windows CI is currently disabled and CI availability is separately constrained;
do not claim Windows execution or a green CI run without fresh evidence.

### Boundaries and review points

The principal boundaries are workspace creation rather than remapping,
same-actor credential recovery versus closed/unknown-actor reauthorization,
activation in the final code transition, and retaining existing review semantics.
The adapters use immutable host approval context, native-store interruption
state and the minimum #12 membership schema. These do not introduce a new
protocol, hidden global state or a second pending-work model.

The existing rollback limitations remain: restoring an older database can restore
older credentials/membership, omit later enrollments or lose later revocations.
There is no universal rollback detector or external revocation oracle. Ordinary
sync recovery and explicit reauthorization use the existing contracts; protected
keys and database backups must be restored together.

## Sources

- [Accepted account contracts](accounts.md#desktop-access-and-refresh-credentials-d371),
  [client-store identity and durability](client-store.md), and
  [membership/recovery model](synchronization-model.md).
- oauth-as 1.0.1 `Storage`, `AuthorizationCodeState`, token issuance, refresh and
  revocation implementation; Huskarl 0.11.9 authorization-code and refresh flows.
  The published release supplies the loopback-port correction tracked by
  [upstream issue #26](https://github.com/MattJackson/oauth-as/issues/26).
- [OAuth for native applications](https://www.rfc-editor.org/rfc/rfc8252.html),
  [DPoP](https://www.rfc-editor.org/rfc/rfc9449.html) and
  [RustCrypto authenticated encryption](https://docs.rs/aes-gcm/latest/aes_gcm/).

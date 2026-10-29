# Desktop Client Enrollment (#12) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for native execution, or superpowers:subagent-driven-development if the user selects delegation. Execute task-by-task. Steps use checkbox tracking. This document authorizes neither execution nor pushing.

**Goal:** Deliver real browser enrollment, independently revocable desktop access
and state-preserving reauthorization using the existing Clacks applications.

**Architecture:** Huskarl runs the native authorization-code and persistent-refresh
flows; embedded oauth-as uses a Clacks-owned relational PostgreSQL Storage adapter.
Hashed lookups and encrypted code receipts preserve the library's current behavior.
The final code transition binds credentials to real synchronization membership;
SQLite/native-store completion preserves local work across interruptions.

**Tech stack:** Rust 1.99.1, SQLx 0.9.0, PostgreSQL, SQLite, Axum/Leptos,
egui/eframe, Tokio, Huskarl, oauth-as, Rustls and native keyring backends.

**Spec:** [Client-enrollment proposal](../../client-enrollment-proposal.md).
Read it and [accounts.md](../../accounts.md), especially D370–D380, before coding.
This plan supplies concrete implementation choices; it does not turn its tuning,
schema names or module decomposition into additional product requirements.

## Global constraints

- Rust **1.99.1**, from `rust-toolchain.toml`; library crates under `lib/`, thin
  applications under `bin/`, workspace-inherited metadata/dependencies.
- Huskarl desktop client; embedded oauth-as; standard authorization-code flow,
  required Proof Key for Code Exchange (PKCE) and Demonstrating Proof of
  Possession (DPoP); opaque access tokens and persistent nonrotating refresh.
- Only the reviewed oauth-as loopback-port correction is selected. No family
  reference patch, new acknowledgment protocol, alternate token handles,
  successful-response cache or separate authorization server.
- Keep provider OpenID Connect (OIDC), login identities and browser sessions on
  #11's existing integration. No desktop provider client secret.
- Preserve indefinite authorized offline eligibility, immediate individual-client
  revocation, account isolation and all local original/prepared/comparison data.
- Closed/unknown actor reauthorization gets a fresh actor and explicit old-work
  reconciliation. Credential recovery alone does not close an authorized actor.
- First enrollment uses a new empty account workspace. Never remap independent
  local data, silently replace files or weaken the populated-workspace guard.
- PostgreSQL release migrations; SQLite startup migrations; checked fixed SQL in
  `.sql` files with committed SQLx offline metadata. No custom write queue.
- Follow [desktop UI](../../desktop-ui.md) and [observability](../../observability.md)
  contracts; no new UI styling, polling framework or telemetry lifecycle wrapper.
- Rust-only project tooling/fixtures; no Python, xtask or test-only public APIs.
  Do not use injected triggers/schema damage to manufacture failures or test the
  libraries' cryptography, migration machinery or documented primitives.
- No `mod.rs` outside tests. Use named record fields, focused modules, ordinary
  control flow, doc comments and idiomatic errors; no production unwrap/expect.
- Read user-owned development inventory before running/configuring a server.
  Use disposable test databases and keyring entries, never personal accounts.
- At execution time use the worktree-placement and using-git-worktrees skills.
  Preserve existing unrelated changes. No automatic commit/push permission is
  implied by this planning request; execution follows the user's instructions.

## Engineering defaults carried from the proposal

These are implementation defaults, not new decision-register entries:

| Default | Exact initial choice |
| --- | --- |
| Application registration | `clacks-desktop`, public, authorization-code/refresh grants, scope `sync` |
| Loopback callback | OS-assigned port, IP literal, `/oauth/callback` |
| Access/code lifetime | 300 seconds / 60 seconds |
| Provisional issuance | Five-minute cleanup deadline, never applied to activated clients |
| Issuer proofs | oauth-as constants: 300-second maximum age, 60-second future allowance |
| Resource proofs | 60-second age, five-second leeway, replay retention covering the full window |
| Algorithm | ES256 only: P-256 elliptic-curve signatures with SHA-256 hashing |
| Credential digests | SHA-256, kind-specific fixed prefix, exact presented bytes, 32-byte `bytea` |
| Encryption | RustCrypto AES-256-GCM authenticated encryption; 32-byte keys, fresh random 12-byte nonces, 16-byte tags |
| Cleanup scheduling | Tokio interval every 60 seconds; bounded batches, no active-client idle expiry |
| Destination | `data_local_dir()/accounts/<AccountId>/<ClientActorId>`, with directory override |
| Native entry | Service `org.clacks.log.desktop`; random opaque user/reference; Windows Local persistence |

Initial dependency pins: `oauth-as =1.0.0` (`default-features=false`,
`dpop,jwt-p256,http`), `huskarl =0.11.9` (`authorization-flow-loopback,tracing`),
`huskarl-core =0.10.10`, `huskarl-crypto-native =0.11.3`,
`huskarl-reqwest =0.9.3` (`rustls-tls`),
`huskarl-resource-server =0.11.12`, `keyring-core =1.0.1`,
`apple-native-keyring-store =1.0.2` (`keychain`),
`windows-native-keyring-store =1.1.0` (`default-features=false`),
`aes-gcm =0.11.1`. Use `reqwest-oauth` as a workspace dependency alias for
`reqwest =0.13.5` with Rustls/json/query, rather than changing #11's reqwest 0.12.
Use existing compatible workspace hash/random/URI facilities; add `fluent-uri`
=0.4.1 for the public resource-binding normalization used by the prototype.
Verify lockfile/platform compatibility before committing these pins; a newer
upstream release replacing the patch requires equivalent behavior evidence.

## Review focus

Each item has a Clacks-behavior scenario in the owning task, not a blanket
dependency test suite:

1. Lost initial response versus lost refresh response: the former restarts browser
   authorization; the latter succeeds with the same refresh and a fresh proof
   (Tasks 2–4 and 6).
2. Wrong browser account or already-existing destination: preserve the previous
   workspace and binding; never remap/overwrite it (Tasks 4–7).
3. Stale in-flight writes after client closure: no credentials or admission can
   reactivate the actor; unrelated clients remain usable (Tasks 2–4).
4. Native credential saved but SQLite binding not committed: restart completes
   the real binding without another authorization or actor allocation (Tasks 5–7).
5. Old prepared work after fresh-actor reauthorization: preserve exact values and
   provenance, hold automatic submission, release only explicit resolved work
   (Task 5).

## File and interface map

Create focused owning modules; listed child file families are not permission to
make empty placeholders. Every file added must carry delivered behavior.

| Owner | Files and responsibility |
| --- | --- |
| Server secret envelopes | `lib/clacks-server/src/secrets.rs`, `secrets/{keys,receipt}.rs`; account-key wrapping and code-receipt protection |
| OAuth integration | `lib/clacks-server/src/oauth.rs`, `oauth/{config,digest,rows,store,authorization,tokens,revocation,replay,cleanup,http,resource,error}.rs`; public issuer wiring and concrete storage callbacks |
| Enrollment application | `lib/clacks-server/src/enrollment.rs`, `enrollment/{membership,approval,recovery}.rs`; actor lifecycle and browser authority |
| Persistence | `lib/clacks-server/migrations/0002_desktop_authorization.sql`, `sql/{oauth,enrollment,secrets}/`, `.sqlx/`; production schema and checked queries |
| Shared identity response | `lib/clacks-sync/src/protocol.rs`: `ClientIdentity { account: AccountId, actor: ClientActorId }`, serialized for `/api/client`; plain wire data, no authorization framework |
| Web | `lib/clacks-web/src/{actions,pages}/clients.rs` and `pages/approval.rs`; account client list, approval and revoke actions |
| Client store | `lib/clacks-client-store/src/enrollment.rs`, `identity.rs`, `synchronization.rs`, `read/review.rs`, `metadata/review.rs`, `rows/`; attempt persistence and old-actor admission holds |
| Client migration/queries | `migrations/0011_enrollment_recovery.sql`, `sql/enrollment/`, root `.sqlx/`; existing local-state KV and a nullable prior-actor hold on `client_record` |
| Native client | `lib/clacks-desktop/src/enrollment.rs`, `enrollment/{credentials,flow,completion}.rs`; Huskarl/native store and completion workers |
| Native UI/composition | `ui/enrollment.rs`, existing `ui.rs`, `ui/actions.rs`, `session.rs`, `application.rs`, `workspace.rs`; shared modal/status styling, worker ownership and workspace opening |
| Deployment | Existing server config, role grants, operator CLI, Docker/runtime docs and server-development docs |

Keep existing `actions.rs`/`pages.rs` as named parent modules when adding children;
do not restructure unrelated UI. Server storage rows stay crate-private and are
shared by their owning schema family. Add tests inside existing integration
targets/module lists, not one newly linked binary per scenario.

## Task 1: Account keys and relational authorization schema

**Deliverable:** Release-migrated production tables and account-scoped secret
envelopes that Task 2 uses directly.

**Files:** Server migration, `secrets*`, `sql/secrets/`, `oauth/rows.rs`, config,
role grants, `store/tests.rs` module extensions, workspace/server manifests.

**Interfaces:**
- `secrets::WrappingKeys::load(&WrappingKeyConfig) -> anyhow::Result<Self>`.
- Crate-private `secrets::receipt::seal(&AccountKey, &ReceiptContext,
  &IssuedReferences) -> anyhow::Result<EncryptedReceipt>` and
  `open(&WrappingKeys, &ReceiptContext, &EncryptedReceipt) ->
  anyhow::Result<IssuedReferences>`; ReceiptContext contains account, code digest,
  immutable purpose and format version. The encrypted envelope carries wrapped
  key/version lookup context, nonce and ciphertext; its owner performs SQL reads,
  rather than a cryptography helper opening its own database transaction.
- `oauth::digest::CredentialKind::{Code,Access,Refresh}` and
  `digest(kind, presented: &str) -> [u8; 32]`; no alternative digest input path.

- [ ] Add `receipt_is_account_bound_and_survives_store_reopen`: use two real
  accounts, seal receipt references, reload using separately constructed runtime
  state and require the correct references. A receipt moved to another account
  fails through Clacks's binding; no raw token appears in its stored columns.
  This verifies envelope wiring/context, not AES implementation vectors.
- [ ] Run the focused scenario and observe the missing Clacks operation/schema
  failure before implementation; do not add tests of SQLx migration validation.
- [ ] Create the proposal's tables and scope/resource children. Add the real
  `sync_accounts`, initial baseline/coverage and membership subset, not QSO
  placeholders. Code approvals survive code take/delete independently; receipt
  columns use named row types. Composite foreign keys prevent cross-account
  object references. Never retain raw credentials in token/refresh tables.
- [ ] Implement random per-account keys, versioned wrapping configuration and
  authenticated receipt encoding. Key files contain exactly 32 raw bytes; config
  names an active key ID plus ID-to-file paths. Associated data is a versioned,
  unambiguous encoding of the account/object identity and purpose. Generate a
  fresh nonce per new encryption; do not derive it from revision/time or reuse
  a cached nonce. Unchanged receipt ciphertext may be retained on replay.
- [ ] Under the account-row lock, load/create the account key and retain its
  referenced old versions. Missing/wrong configured keys fail actionably without
  replacing account keys or reusing cookie/provider keys. Make secret Debug
  representations redacted; use standard zeroizing containers where supported.
- [ ] Generate checked metadata against a disposable PostgreSQL database with
  the real migrations, run the scenario, and commit this coherent storage/key
  deliverable following the commit policy below if execution permits commits.

## Task 2: oauth-as Storage adapter and atomic enrollment

**Deliverable:** The published issuer state machine runs against real relational
Clacks persistence, including enrollment activation and code-replay receipts.

**Files:** `oauth/{store,authorization,tokens,revocation,replay,cleanup,config}.rs`,
`enrollment/{membership,recovery}.rs`, `sql/{oauth,enrollment}/`, server manifest,
`vendor/oauth-as/`, root dependency patch, `lib/clacks-sync/src/membership.rs`
and server integration scenarios.

**Interfaces:**
- `OAuthStore { pool: PgPool, keys: Arc<WrappingKeys> }`, crate-private;
  `impl oauth_as::store::Storage for OAuthStore` for the exact selected features.
- `enrollment::clients(state, principal) -> anyhow::Result<Vec<ClientSummary>>`
  and `enrollment::revoke(state, principal, actor) -> anyhow::Result<()>`, public
  application operations used by Task 4; use existing concrete Principal type.
- Crate-private `activate_in(&mut PgConnection, code_digest, issued_refs)` owns
  the final code transition, grant binding and membership write in one transaction.
  `issued_refs` is a named secret-bearing struct, not a tuple or public API.
- `clacks_sync::membership::enroll(actor: ClientActorId, account: &AccountMetadata,
  membership: &MembershipSnapshot) -> anyhow::Result<MembershipUpdate>` returns
  guarded membership/head changes for a genuinely new actor; it refuses an
  existing identity rather than reopening it. This keeps account advancement
  inside the existing model, whose `AccountMetadata::advance` is crate-private.

- [ ] Add `code_exchange_activates_one_member_and_replay_revokes_only_its_grant`:
  obtain a real issuer code through approved host wiring, redeem it, check the
  actor/family association and absent acknowledgment, then repeat that code with
  a fresh proof. Expect rejection, grant invalidation and unchanged actor closure
  state; an unrelated client's credential still authorizes. Inspect relational
  token rows and encrypted receipt reconstruction through real operations.
- [ ] Add `revocation_refuses_stale_issuer_writes`: capture a real library-issued
  record, close its actual client, and attempt the normal storage write with
  that stale record. Assert no reactivation/access and unaffected other clients.
  No failpoints, injected triggers or custom cryptographic implementation.
- [ ] Run focused scenarios, then vendor the published 1.0.0 source and apply only
  the reviewed port correction. Preserve upstream notices/licenses and record
  origin/version and exact patch scope in `vendor/oauth-as/CLACKS-PATCH.md`.
  Exclude the dependency from workspace membership; do not copy the unpublished
  PostgreSQL adapter or retain its schema. Add locked dependencies as needed.
- [ ] Implement all enabled Storage methods. Client public metadata is ordinary
  relational data. Hash every code/token lookup/delete/compensation input; restore
  returned credential fields from that call's presented value. Code reads decrypt
  only their issued-reference pair. Required device methods fail explicitly;
  unused atomic refresh rotation keeps the trait's unsupported default.
- [ ] Implement remove-and-return and conditional code-state writes atomically,
  preserving host approval context in `desktop_code_approvals`. It is attached
  before delivering the browser redirect. Do not modify library record types or
  invent code correlation in token-storage callbacks: they receive no code ID.
- [ ] Initial token writes create a provisional family with its own five-minute
  issuance deadline. No resource/refresh access while provisional. Final code
  compare-and-swap resolves the supplied issued credentials, validates their
  metadata, attaches the approved actor, advances real membership/head as needed,
  encrypts receipt references and commits activation together. Missing/expired
  provisional state or changed expected code state returns false, never recreates.
  Seed both IPv4 and IPv6 literal callback registrations for `/oauth/callback`;
  keep the actual OS-assigned redirect URI in the issued code and redemption.
- [ ] Guard token/refresh puts against current client/family authorization and
  the library revocation barriers under the same transaction. Preserve revoked
  family identity/state so a stale snapshot cannot become a new provisional
  family. Cleanup expires code/access/replay/provisional data, not active refresh,
  actor eligibility or required non-secret revocation evidence.
- [ ] Explicit actor revoke uses `clacks_sync::membership::revoke` and atomically
  closes membership plus all actor grant families. Same-actor credential recovery
  replaces grant authorization without closing membership; closed/unknown recovery
  creates a fresh actor. Keep account-lock order consistent with #11.
  New enrollment uses `membership::enroll`; its acknowledged checkpoint is None.
- [ ] Run focused scenarios, metadata check and server Clippy. Commit the working
  issuer adapter with dependency correction and schema adjustments together.

## Task 3: HTTP issuer and direct resource authorization

**Deliverable:** Actual production token/metadata/revocation routes and protected
identity read, usable behind the existing HTTPS deployment boundary.

**Files:** `oauth/{http,resource,error}.rs`, existing server `http.rs`, `runtime.rs`,
`clacks-sync/src/{protocol,lib}.rs`, server request scenarios.

**Interfaces:**
- `protocol::ClientIdentity { account, actor }`, shared server/desktop wire type.
- Crate-private `oauth::resource::authorize(&ServerState, &HeaderMap, &Method,
  &Uri) -> Result<AuthorizedClient, ClientAuthError>`; AuthorizedClient contains
  account/actor/family context for the current request, not cached authorization.
- `oauth::http::router(ServerState) -> Router<ServerState>` mounts only the routes
  in proposal section 4. `GET /api/client` returns the shared identity type.

- [ ] Add `native_token_post_bypasses_cookie_origin_policy_but_browser_mutations_do_not`:
  real native exchange without Origin succeeds; existing cookie mutation without
  an allowed origin still fails. Do not weaken the global browser policy.
- [ ] Add `protected_identity_is_current_and_replay_is_shared`: use two actual
  app instances with separate pools over one disposable DB. The same proof is
  rejected on the other instance; wrong-key/account access fails; a live request
  after actor closure fails without waiting for expiry. Assert actual instance
  routing, not inferred round robin or a uniqueness constraint in isolation.
- [ ] Run the scenarios, then compose the selected oauth-as standard handler for
  token/revocation and the production database-backed issuer. Separate native
  routes from cookie-only origin checking. Keep no-store/referrer protections.
- [ ] Derive discovery from the public library metadata, then limit grants and
  endpoints to Clacks's mounted capabilities. No advertised device/dead
  introspection endpoint, and no server-to-itself introspection request.
- [ ] Use Huskarl's proof validator plus explicit key/method/public-URI/token-hash
  binding, scope/expiry and shared replay claims. Use fluent-uri normalization;
  trust configured public origin, not arbitrary forwarding headers. Require one
  credential and proof and the DPoP scheme. Database/decryption failures map to
  unavailable; invalid credentials map to standard authorization errors/challenges.
- [ ] Keep configured Reuse, no refresh expiry, zero retry window, required proofs
  and ES256 restriction. Preserve library code replay and timestamp behavior;
  do not add a nonce server or reinterpret grant revocation as actor closure.
- [ ] Run server request scenarios plus the existing web regression boundary;
  commit native HTTP/resource integration without changing synchronization transport.

## Task 4: Browser approval and account client management

**Deliverable:** Real Leptos enrollment approval/list/revoke workflows using #11
sign-in and current browser authority.

**Files:** Server `enrollment/{approval,recovery}.rs`, existing auth pending/callback
integration; web `actions/clients.rs`, `pages/{approval,clients}.rs`, parent
modules/navigation and existing web integration tests.

**Interfaces:**
- `ApprovalPurpose::{New, RecoverActive { actor }, ReplaceClosed { actor }}`;
  closed/absent requested actors share the explicit new-actor result, not reopening.
- `ApprovalRequest` retains the validated OAuth parameters, label and recovery
  hint in the normal browser session. Hints never supply account authority.
- `enrollment::approve(state, principal, request) -> Result<AuthorizationResponse>`
  attaches immutable approval context before exposing the actual code redirect.

- [ ] Extend the real provider/browser scenario with
  `approval_resumes_after_sign_in_and_uses_the_verified_account`: sign in through
  the existing callback, approve one desktop request, redeem with the real issuer
  and assert the returned account. A stale/wrong-account recovery attempt leaves
  the existing client/workspace unchanged; a hidden form ID is not authority.
- [ ] Add client management assertions to that account scenario: client labels/IDs
  identify rows, revoking one actor invalidates its access while another still
  works, and no invented last-successful-sync value appears before actual sync.
- [ ] Run focused web scenarios, then implement the retained request through
  ordinary axum_session typed entries and existing sign-in purpose handling.
  Revalidate request and live login on approval; preserve exact callback, PKCE
  and required key thumbprint. Reject open redirects and expired/missing context.
- [ ] Implement pages/actions using existing account styles, normal Leptos forms
  and shared error/status behavior. Show server/account, proposed client label
  and the consequence of recovery. Approval/denial is explicit; denial issues no
  client authorization. Do not make the user reauthenticate on an invented timer.
- [ ] Mount same-origin guarded approval/revoke actions and server-function routes.
  Use current Principal account predicates in list/revoke/recovery queries. Do
  not accept arbitrary account IDs as authorization or use administrator role as
  implicit access to another account's clients.
- [ ] Run existing plus extended web scenarios and `cargo leptos build`; commit
  the working browser integration and its portable operator documentation.

## Task 5: Durable local enrollment and old-actor admission holds

**Deliverable:** SQLite preserves enrollment completion and forbids automatic
submission of old pending/prepared work after fresh-actor reauthorization.

**Files:** Client-store migration, `enrollment.rs`, identity/synchronization/review
operations and row types; `sql/enrollment/`; existing integration target modules.

**Interfaces:**
- `EnrollmentAttempt` has `credential_reference: String`, `server: String`,
  `destination: Option<PathBuf>`, `expected: Option<EnrollmentBinding>` and
  `phase: EnrollmentPhase::{KeySaved, CredentialSaved, DestinationBound}`.
  This is non-secret serializable state in the existing local-state key/value
  (KV) table, not another credential store.
- `enrollment::attempt(&ClientStore) -> anyhow::Result<Option<EnrollmentAttempt>>`,
  `save_attempt(&ClientStore, &EnrollmentAttempt) -> anyhow::Result<()>` and
  `clear_attempt(&ClientStore, &EnrollmentAttempt) -> anyhow::Result<()>`.
  Clear only the matching saved attempt, preserving a newer attempt. These are
  production completion operations, not a generic public KV interface.
- `enrollment::install(store, expected: Option<&EnrollmentBinding>, next:
  EnrollmentBinding, outcome: RecoveryDisposition) -> anyhow::Result<()>`, an
  atomic binding/hold operation built on the existing identity checks.
- `RecoveryDisposition::{New, SameActor, FreshActor { former_actor }}`.
  `PrepareOutcome::AuthorizationReview { former_actor }` makes a pre-catch-up hold
  explicit without pretending normal shared evidence already exists.

- [ ] Add `fresh_actor_holds_old_work_until_explicit_review`: start with real saved
  and prepared work, install a replacement actor, edit a held record and create
  an unrelated record. Preparation returns the hold for old rows without changing
  frozen bytes/counters/originals; unrelated work follows normal eligibility.
  Install real shared observations, resolve explicitly through the existing review
  operation, and assert only the chosen result becomes preparable under the new actor.
- [ ] Add `enrollment_binding_is_guarded_and_same_actor_recovery_preserves_work`:
  stale/wrong-account/populated initial binding leaves state untouched; same-actor
  replacement changes only credential binding and keeps pending eligibility.
- [ ] Run focused scenarios, then persist non-secret attempt state in the shared
  KV, not a dedicated key-as-table or second account registry. Expose the existing
  binding through the identity module for actual recovery callers.
- [ ] Add a nullable prior-actor hold to existing client-record rows. In one
  immediate transaction recheck prior binding, mark exactly the current
  pending/prepared set when changing actor, and save the new binding. Check holds
  before evidence loading or counter allocation. New edits do not silently clear
  them; ordinary state reload/receive updates retain them.
- [ ] Bridge holds into existing `local_review` reason `actor` after shared receipt.
  Clear only inside the successful exact-basis explicit-resolution transaction,
  including an explicit choice of the shared version. No automatic author/vector
  renaming and no duplicate pending-work representation. Extend the existing
  account-wide review UI only where needed to show this existing review reason.
- [ ] Regenerate SQLite metadata using production migrations; run the scenarios
  in the existing linked integration suite and commit local durability/recovery.

## Task 6: Huskarl client and native credential persistence

**Deliverable:** A real desktop worker authorizes, persists, refreshes and resumes
credentials using selected native backends; no fake authorization service.

**Files:** Desktop `enrollment/{credentials,flow,completion}.rs`, manifests, shared
identity response imports and a focused native/server integration scenario.

**Interfaces:**
- Crate-private `CredentialBundle` stores private JWK material, issuer/client
  metadata and optional complete Huskarl RefreshToken. JWK means JSON Web Key.
  It never derives an unredacted Debug; SQLite sees only its opaque reference.
- `credentials::{create,load,save_refresh,remove}` use the actual native store,
  no production credential-store trait introduced solely for fake tests.
- `flow::authorize(request, cancellation) -> anyhow::Result<AuthorizedEnrollment>`
  and `flow::resume(reference) -> anyhow::Result<AuthorizedEnrollment>` return
  verified identity plus native reference, never a raw-token UI message.

- [ ] Add `saved_native_credential_resumes_the_actual_client`: authorize against
  production routes with a disposable account/keyring entry, drop the client,
  reload native material and refresh/read `/api/client` without another browser
  authorization. Discard a refresh response and retry with a fresh proof; assert
  unchanged refresh credential and correct account. This is Clacks persistence
  and protocol composition, not a generic keyring round trip.
- [ ] Run the scenario, then initialize macOS keychain/Windows credential backend
  using their standard APIs. Disable Windows search feature and request Local
  persistence. Serialize operations on each entry through the owning enrollment
  worker, not concurrent unsynchronized writes; no filesystem-secret fallback.
- [ ] Persist the key before opening the browser. Use Huskarl discovery, listener,
  flow start/completion, token cache and HttpAuthorizer. Keep the complete native
  refresh metadata and verify returned DPoP binding. No handwritten JWT, PKCE,
  browser callback parser, proof generator or refresh state machine.
- [ ] Use the saved origin/client/key on resume; do not discover an arbitrary
  server from callback fields. A rejected/absent credential requires browser
  authorization; network failure preserves it for later. Do not launch automatic
  sign-in loops or replay consumed codes as a response-recovery mechanism.
- [ ] Native stores have real payload limits and access failures. Propagate an
  actionable Clacks error if the actual bundle cannot be saved; do not introduce
  secret splitting/chunk storage or change backends to make a test pass.
- [ ] Gate native-store scenarios to supported macOS/Windows hosts. Linux keeps
  normal workspace builds and server tests, with an explicit unsupported-native-
  store outcome for Connect rather than a silent plaintext fallback or fake
  production backend. Linux desktop secret storage remains post-MVP.
- [ ] Use real native-store acceptance on macOS and Windows. Automated host
  scenarios must be noninteractive and clean only their unique entries; manual
  acceptance runs the shipped app, not an ignored human-driven test harness.
  Record unavailable Windows evidence honestly. Commit native protocol/persistence.

## Task 7: Desktop workflow, completion recovery and workspace opening

**Deliverable:** Connect/reauthorize flows are usable from the real workbench and
restart safely through saved attempt state without changing the original workspace.

**Files:** Desktop `enrollment.rs`, UI enrollment modal, session/application/
workspace integration and existing UI/session scenario modules.

**Interfaces:**
- `EnrollmentCommand::{Begin, Resume, Cancel}` carries only non-secret user input
  or the opaque saved attempt; it is owned by the existing session runtime.
- `EnrollmentStatus::{Idle, Authorizing, Saving, Ready, Failed}` is a typed,
  non-secret UI snapshot with actionable failure text and no credential payloads.
- `completion::finish(initiating_store, authorized, cancellation)` uses Task 5
  installation and production workspace ownership; returns an owned destination
  path for the existing application launch/activation path.

- [ ] Add `saved_credentials_finish_binding_after_restart`: persist a real native
  credential and local attempt, stop before calling install, restart/resume and
  complete the binding. Assert the same verified actor, intact source workspace
  and no second approval/actor. Stop at normal operation boundaries; no failure
  injection or fake DB triggers. Existing destination/wrong identity remains intact.
- [ ] Add one native component interaction scenario using egui_kittest: empty
  invalid server disables Connect; valid inputs submit one request; Saving blocks
  duplicate submission; Cancel sends one cancellation; modal blocks background
  controls. No custom frame driver, paint-text clicking or repeated store suite.
- [ ] Implement Connect/reauthorize with existing dialog/fields/theme/feedback.
  Show server, label, recovery consequence and destination; preserve ordinary
  workbench layout and local logging. Invalid values use the shared debounced
  validation convention; network errors remain actionable without disclosing secrets.
- [ ] Save attempt state in the initiating owned workspace before browser work.
  Save refreshed native bundle before binding. Read `/api/client`, verify the
  intended account, and create the destination only with its verified AccountId
  through the empty `ClientStore::create` path. Never call standalone seeding.
  For same-account recovery, install into the existing store instead: do not
  relocate its directory when its actor changes or allocate another local account.
- [ ] Acquire existing workspace ownership before destination writes. If another
  process owns it, activate that process and preserve the attempt instead of
  racing a write. If created but binding is incomplete, reopen through ordinary
  account-checked store APIs and finish the exact guarded binding; never overwrite.
- [ ] After binding commits, close the temporary destination pool/owner and open
  through the production executable with only `--workspace <path>`; no secret
  arguments. Clear initiating attempt only after observing the expected durable
  destination binding. A launch failure offers retry against that same workspace.
  Existing-instance activation still uses the selected IPC protocol unchanged.
- [ ] Cancellation closes the temporary listener and stops local work but does
  not claim to roll back committed remote approval. Native-save failure preserves
  prior binding; successful replacement removes obsolete native entries afterward,
  with retryable cleanup failure. Never delete the new key because SQL committed.
- [ ] Show enrolled/awaiting initial sync until #13 provides actual receipt.
  No fabricated checkpoint, default groups, last-successful-sync timestamp or
  mock stream. Run actual app visual checks using both appearances and a compact
  window; record Windows interaction separately. Commit complete desktop workflow.

## Task 8: Deployment, observability and whole-feature verification

**Deliverable:** Portable setup/backup instructions and verified production
integration, with current evidence limits stated accurately.

**Files:** Existing server config/example, maintenance CLI, role grants, server/
desktop READMEs, `docs/{accounts,client-enrollment-proposal,backup-recovery,
server-development}.md`; existing observability tests and integration targets.

**Interfaces:** Existing runtime composition owns cleanup jobs and wrapping keys;
existing `clacks-admin` gains a discoverable `keys generate --output <path>`
operation only if needed for the documented 32-byte file setup. It is a thin
library-backed command, not a new executable or generic task framework.

- [ ] Add one representative committed enrollment/revocation telemetry assertion
  to the actual operation scenario. Require truthful outcome/account/actor
  correlation and bounded metric categories; sentinel secret values from the
  exercised request/bundle/receipt must not appear. Do not replay the whole suite
  with telemetry on/off or test subscriber formatting/aggregation.
- [ ] Wire oauth-as hooks to structured events, with significant committed actions
  at info and expected refusals distinguished from database/decryption failures.
  Use normal tracing spans/framework timing; no Completion wrapper, manual span
  lifecycle, redundant operation/component fields or token/proof dump.
- [ ] Document exact raw key-file creation, config IDs/paths, runtime permissions,
  role grants, migration command, same keys across app instances, and joint
  database/key backup/restore. Keys are separate from cookie/provider secrets.
  Restoring an old DB may lose enrollments or restore revoked credentials; do
  not promise rollback-proof revocation or automatic actor reconstruction.
- [ ] Run the actual server, web approval and desktop enrollment using disposable
  state; verify fresh/same-actor/closed-actor workflows and the normal app's
  cancellation/shutdown behavior. Do not re-register or mutate the personal
  development environment unless separately authorized.
- [ ] Run the verification commands below. Review new tests against the concrete
  Clacks mistake each catches; remove dependency-only/scaffolding tests, not
  production behavior to satisfy them. Check focused modules, docs and notices.
- [ ] Reconcile portable docs with delivered APIs/defaults and record factual
  acceptance evidence. Keep review narration and audit inventories outside the
  repository. Commit the complete operational integration and documentation.

## Verification commands and evidence

Use `DATABASE_URL`/`CLACKS_TEST_DATABASE_URL` and runtime-role variables only for
the disposable database. The exact role setup is the existing portable development
guide. Do not create a second metadata-preparation database unnecessarily.

```sh
cargo fmt --all --check
SQLX_OFFLINE=true cargo test -p clacks-server --lib --locked
SQLX_OFFLINE=true cargo test -p clacks-client-store --test integration --locked
SQLX_OFFLINE=true cargo test -p clacks-desktop --lib --locked
SQLX_OFFLINE=true cargo test -p clacks-web --features ssr --test integration --locked
SQLX_OFFLINE=true cargo clippy --workspace --all-targets --locked -- -D warnings
SQLX_OFFLINE=true cargo test --workspace --all-targets --locked
SQLX_OFFLINE=true cargo test --workspace --doc --locked
RUSTDOCFLAGS='-D warnings' SQLX_OFFLINE=true cargo doc --workspace --no-deps --document-private-items --locked
cargo leptos build --release
```

During each task run the named scenario using the target's test-name filter,
then broaden only after its deliverable is ready. Run the server SQL metadata
prepare/check from `lib/clacks-server` against the migrated disposable PostgreSQL
database. Run the client-store metadata prepare/check from the repository root
scoped to `-p clacks-client-store` against its disposable SQLite database; preserve
the current CI command and metadata locations. Expected result: all selected
checks exit zero; metadata check reports no stale query metadata. Do not run
Cargo against production databases or claim a cross-compile proves native behavior.

Windows CI remains disabled; monthly CI availability is constrained. Do not
reenable Windows jobs, push merely to trigger CI, or claim remote green status
without user authorization and actual evidence. Native macOS/Windows key-storage
and browser/desktop acceptance are distinct from Linux build/tests.

## Commit and execution handoff

Keep feature work reviewable across the eight deliverables; merge adjustments
into the commit that introduces the behavior rather than adding cleanup commits.
Before every commit/amend, prepare a substantive message file explaining the
motivation/constraints, inspect physical line lengths and hard-wrap prose at 72
columns, then use `git commit --file <message-file>`. Preserve literal URLs and
machine identifiers. Never stage unrelated existing reference/document changes.

No implementation starts until the user requests execution. Preserve a selected
native execution method rather than asking again after it is supplied. Do not
push, create a PR, change infrastructure or update issue status merely because
this plan exists. Close #12 only after the actual capability and its evidence are
delivered; #13 extends these production components for WebSocket synchronization.

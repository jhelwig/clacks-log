# Accounts, web management and observability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

OpenID Connect is **OIDC**; a radio contact is a **QSO**; minimum viable product is **MVP**; user interface is **UI**; data transfer object is **DTO**. LoTW means Logbook of The World. An issuer/subject pair identifies a login binding, not the Clacks account itself.

**Goal:** Deliver isolated accounts, browser enrollment, protected credentials and the MVP management interface in the one server application.

**Architecture:** Axum owns authentication, synchronization routes and background work; Leptos renders the management interface with Rust browser hydration. External OpenID Connect identities bind to stable Clacks accounts, while enrolled clients have independent revocable credentials.

**Tech Stack:** Rust, Axum, Leptos, PostgreSQL, protected client secret storage; no TypeScript application layer.

**Spec:** [accounts.md](../../../docs/accounts.md), [observability.md](../../../docs/observability.md), [server-credential-protection.md](../../../references/server-credential-protection.md), [web-interface-architecture.md](../../../references/web-interface-architecture.md). Read these with the [coordinating plan](2026-10-17-clacks-log.md).

## Global Constraints

This is a planned file/interface layout, not existing code or authorization to execute. All tasks also inherit the coordinating plan’s constraints and file/dependency contracts. Native records and protocol encodings are pinned by F1/S1 before downstream code; do not invent competing representations in an isolated task. No implementation checks below have run yet.

- Multiple login bindings per internal account; each issuer/subject binds to at most one account (D231).
- Self-service linking/unlinking is MVP; administrator-role assignment is out of band.
- Ordinary log data needs no application unlock/encryption workflow; signing/upload secrets stay server-side.

## Review Focus

- Concurrent first login/link: A1 preserves issuer/subject uniqueness without email-based linking.
- Revocation during an open stream: A2/S3 enforce the changed authorization.
- Swapped ciphertext/account IDs: A3 authentication fails without leaking secrets.
- Callback replay or interrupted enrollment: A2 cannot enroll a client for the wrong browser/account session.
- A provider error body contains a secret: A4 redacts telemetry while retaining actionable diagnostics.

## File structure

The Files entries below use `lib/<crate-name>` for libraries and `bin/<application-name>` for thin executable wrappers. Behavior, configuration/runtime composition and tests belong to libraries; binaries only delegate to the library entry point. The Files entries map the new implementation units and their tests. Brace groups expand to individual files in that directory. Crate manifests/module exports and required schema migrations belong to the task that first needs them; keep related source/tests together. No production file exists at these proposed paths yet.

---

### A1: Stable accounts and login-identity bindings

**Depends on:** F2.

**Files:**

- Create/modify: `lib/clacks-server/src/auth/{mod,oidc,accounts,identities,context}.rs`
- Create/modify: `lib/clacks-server/migrations/0002_identity.sql`
- Create/modify: `lib/clacks-server/tests/account_identity.rs`
- Create/modify: `docs/engineering/authentication-contract.md`

**Interfaces:**

- Consumes: F1 AccountId; F2 account-scoped database transactions; configurable identity provider.
- Produces: AccountContext { account_id, roles }; ClientContext { account: AccountContext, client_id, actor_id }; authenticate_web(input: VerifiedIdentity) -> Result<AccountContext, AuthError>; link_identity(ctx: &AccountContext, proof: FreshIdentityProof) -> Result<(), AuthError>; unlink_identity(ctx: &AccountContext, binding: BindingId, proof: RemainingIdentityProof) -> Result<(), AuthError>. Only a verified protocol adapter can construct VerifiedIdentity/proof types.

- [ ] **Step 1: Add the named regression/acceptance tests and their fixtures.**

account_identity::first_login_provisions_once; multiple_providers_same_account; concurrent_link_unique; cannot_unlink_last; cannot_link_other_account_identity; fresh_remaining_identity_required. Include equal emails with different issuer/subject pairs.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert_eq!(concurrent_first_login_accounts.len(), 1);
assert!(link_already_bound_elsewhere.is_err());
assert!(unlink_last_identity.is_err());
```

- [ ] **Step 2: Run the focused tests and confirm the expected pre-implementation failure.**

Run each focused command listed in Step 4 after creating a compilable minimal target. Confirm failure comes from missing behavior or the explicitly missing module/type, rather than an unavailable database, provider or toolchain. For infrastructure/artifact tasks, use the named reproducible verification scenario instead of adding tests that merely mirror configuration.

- [ ] **Step 3: Implement the listed interfaces and behavior.**

Select a maintained standard OIDC integration at execution, validate issuer/subject and callback/session bindings, and implement account provisioning plus explicit link/unlink workflows. Do not equate an OIDC identity with the internal account or automatically link by email. Keep administrator role assignment explicit and out of band. Document administrator-assisted recovery without automated self-service recovery or account merging.

- [ ] **Step 4: Verify the task deliverable.**

```text
cargo test -p clacks-server --test account_identity
```

**Expected:** all named tests/checks pass; Identity race, linking and recovery-boundary tests pass across isolated accounts.

- [ ] **Step 5: Review and commit the bounded change.**

Check the task diff against its specification and constraints; correct failures, then commit the listed implementation/tests and any contract updates with message `feat: stable accounts and login-identity bindings` (use `docs:` for a documentation-only contract). Update the task checkbox and verification evidence. Do not advance a failing gate or change an accepted policy to make a test pass.

### A2: Browser-callback enrollment and revocable client credentials

**Depends on:** A1.

**Files:**

- Create/modify: `lib/clacks-server/src/auth/{enrollment,clients}.rs`
- Create/modify: `lib/clacks-client/src/{enrollment,credentials}.rs`
- Create/modify: `lib/clacks-server/migrations/0003_clients.sql`
- Create/modify: `lib/clacks-server/tests/client_enrollment.rs`
- Create/modify: `lib/clacks-client/tests/client_credentials.rs`

**Interfaces:**

- Consumes: A1 authenticated browser AccountContext; F1 actor/client identities; protected platform storage adapter.
- Produces: begin_enrollment(server: ServerEndpoint) -> Result<EnrollmentRequest, AuthError>; finish_enrollment(callback: EnrollmentCallback) -> Result<ClientEnrollment, AuthError>; authorize_client(proof: ClientProof) -> Result<ClientContext, AuthError>; revoke_client(ctx: &AccountContext, client: ClientId) -> Result<(), AuthError>. Define credential renewal and independent causal identity lifetime in the authentication contract.

- [ ] **Step 1: Add the named regression/acceptance tests and their fixtures.**

client_enrollment::callback_replay_rejected; account_session_bound; interrupted_flow_safe; individual_revocation; no_offline_expiry. client_credentials::restart_uses_protected_storage; reauthorization_keeps_local_data. Use a local test identity provider; reserve real tsidp checks for R2.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert!(replayed_callback.is_err());
assert!(revoked_client_access.is_err());
assert!(other_client_access.is_ok());
assert_eq!(retained_qsos, before_reauthorization);
```

- [ ] **Step 2: Run the focused tests and confirm the expected pre-implementation failure.**

Run each focused command listed in Step 4 after creating a compilable minimal target. Confirm failure comes from missing behavior or the explicitly missing module/type, rather than an unavailable database, provider or toolchain. For infrastructure/artifact tasks, use the named reproducible verification scenario instead of adding tests that merely mirror configuration.

- [ ] **Step 3: Implement the listed interfaces and behavior.**

Use standard browser-callback authorization support, document the chosen callback mechanism and protect request/session correlation. The native client receives its own revocable credential, not a provider secret or browser password. Renewal must not create a new causal actor on each token refresh. Apply revocation to subsequent requests and existing streams; keep accepted data and preserve offline edits for explicit recovery under S4. Use protected operating-system storage with platform adapters; no device-code fallback in MVP.

- [ ] **Step 4: Verify the task deliverable.**

```text
cargo test -p clacks-server --test client_enrollment; cargo test -p clacks-client --test client_credentials
```

**Expected:** all named tests/checks pass; Enrollment, replay rejection, renewal and individual-client revocation work without data erasure.

- [ ] **Step 5: Review and commit the bounded change.**

Check the task diff against its specification and constraints; correct failures, then commit the listed implementation/tests and any contract updates with message `feat: browser-callback enrollment and revocable client credentials` (use `docs:` for a documentation-only contract). Update the task checkbox and verification evidence. Do not advance a failing gate or change an accepted policy to make a test pass.

### A3: Account envelope encryption and directory credential provisioning

**Depends on:** A2.

**Files:**

- Create/modify: `lib/clacks-server/src/secrets/{mod,envelope,credentials,distribution}.rs`
- Create/modify: `lib/clacks-server/migrations/0004_secrets.sql`
- Create/modify: `lib/clacks-client/src/directory_credentials.rs`
- Create/modify: `lib/clacks-server/tests/secret_boundaries.rs`
- Create/modify: `docs/operations/key-recovery.md`

**Interfaces:**

- Consumes: A1 account context; A2 enrolled-client authorization; protected local storage adapter.
- Produces: CredentialStore::put(ctx: &AccountContext, input: SecretInput) -> Result<CredentialId, SecretError>; remove(ctx: &AccountContext, id: CredentialId) -> Result<(), SecretError>; provision_directory(ctx: &ClientContext) -> Result<DirectoryCredentialUpdate, SecretError>. Secret values must use non-debug-printing wrappers; directory provisioning never includes LoTW keys or publication credentials.

- [ ] **Step 1: Add the named regression/acceptance tests and their fixtures.**

secret_boundaries::independent_account_keys; swapped_account_ciphertext_fails; missing_key_not_regenerated; wrong_key_restore_fails; client_receives_only_directory_credentials; removal_keeps_public_metadata. Include old wrapping-key versions and disconnected clients retaining previously received credentials.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert_ne!(account_a_key_id, account_b_key_id);
assert!(swapped_record_decryption.is_err());
assert!(provisioned.contains_no_signing_or_upload_secrets());
```

- [ ] **Step 2: Run the focused tests and confirm the expected pre-implementation failure.**

Run each focused command listed in Step 4 after creating a compilable minimal target. Confirm failure comes from missing behavior or the explicitly missing module/type, rather than an unavailable database, provider or toolchain. For infrastructure/artifact tasks, use the named reproducible verification scenario instead of adding tests that merely mirror configuration.

- [ ] **Step 3: Implement the listed interfaces and behavior.**

Use established authenticated encryption with independent random account keys wrapped by a deployment key mounted outside PostgreSQL/image. Authenticate account/credential identities as encryption context. Specify key versions, rotation and backup recovery. Removal blocks future permitted signing and preserves public metadata/history; define in-flight handling before wiring jobs. Provision selected directory credentials securely to enrolled clients; do not promise remote erasure of disconnected caches or independent backups.

- [ ] **Step 4: Verify the task deliverable.**

```text
cargo test -p clacks-server --test secret_boundaries
```

**Expected:** all named tests/checks pass; Key/account binding, provisioning restrictions and recoverable key-version procedures are demonstrated.

- [ ] **Step 5: Review and commit the bounded change.**

Check the task diff against its specification and constraints; correct failures, then commit the listed implementation/tests and any contract updates with message `feat: account envelope encryption and directory credential provisioning` (use `docs:` for a documentation-only contract). Update the task checkbox and verification evidence. Do not advance a failing gate or change an accepted policy to make a test pass.

### A4: Leptos management workflows and production observability

**Depends on:** A3, E1; service-specific panels finish after E3.

**Files:**

- Create/modify: `lib/clacks-web/src/{lib,routes,accounts,clients,services,diagnostics}.rs`
- Create/modify: `lib/clacks-server/src/{app,startup,observability}.rs`
- Create/modify: `bin/clacks-log-server/{Cargo.toml,src/main.rs}`
- Create/modify: `lib/clacks-server/tests/management_routes.rs`
- Create/modify: `lib/clacks-server/tests/telemetry_redaction.rs`
- Create/modify: `docs/design/management-workflows.md`

**Interfaces:**

- Consumes: A1–A3 account/enrollment/secret services; E1 diagnostic DTOs; E3 provider adapters when available.
- Produces: clacks_server::main_entry() -> std::process::ExitCode; app(state: ServerState) -> axum::Router; management routes call the same internal account/service operations as the server. Expose account-isolated SyncStatus and UploadDiagnostic views. No second management database or separately deployed Clacks service.

- [ ] **Step 1: Add the named regression/acceptance tests and their fixtures.**

management_routes::cross_account_object_access_denied; management_and_sync_share_state; reauth_required_for_identity_changes. telemetry_redaction::request_and_provider_secrets_removed; trace_job_correlation; bounded_metric_labels. Test production logging configuration, not only a standalone redaction helper.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert_eq!(foreign_account_status, 403);
assert!(!captured_logs.contains(secret_sentinel));
assert!(request_trace_and_job_trace_are_correlated);
```

- [ ] **Step 2: Run the focused tests and confirm the expected pre-implementation failure.**

Run each focused command listed in Step 4 after creating a compilable minimal target. Confirm failure comes from missing behavior or the explicitly missing module/type, rather than an unavailable database, provider or toolchain. For infrastructure/artifact tasks, use the named reproducible verification scenario instead of adding tests that merely mirror configuration.

- [ ] **Step 3: Implement the listed interfaces and behavior.**

Keep startup/runtime, configuration, telemetry and substantive error handling in `lib/clacks-server`; `bin/clacks-log-server/src/main.rs` delegates only to its library entry point. Create concrete workflow mockups for review before UI implementation. Implement sign-in, identity linking/unlinking, client list/revoke, administrator account creation and external-service setup/diagnostics. Full setup opens from desktop; station/profile editing remains native. Keep architecture suitable for future browsing/editing/reporting without adding them to MVP. Add structured logs, tracing and metrics across web/sync/jobs with operator security events, correlation, cardinality limits and redaction. Choose actual telemetry exporters/tooling as execution-time engineering; do not add an account-user security-history page.

- [ ] **Step 4: Verify the task deliverable.**

```text
cargo check -p clacks-log-server; cargo test -p clacks-server --test management_routes; cargo test -p clacks-server --test telemetry_redaction; cargo check -p clacks-web --target wasm32-unknown-unknown --no-default-features --features hydrate
```

**Expected:** all named tests/checks pass; Management and telemetry run in one application; native and browser builds pass and secrets never appear in captured telemetry.

- [ ] **Step 5: Review and commit the bounded change.**

Check the task diff against its specification and constraints; correct failures, then commit the listed implementation/tests and any contract updates with message `feat: leptos management workflows and production observability` (use `docs:` for a documentation-only contract). Update the task checkbox and verification evidence. Do not advance a failing gate or change an accepted policy to make a test pass.

## Completion gate

Management and telemetry run in one application; native and browser builds pass and secrets never appear in captured telemetry.
No later work is authorized merely because this plan is complete. See the coordinating plan for execution and release gates.

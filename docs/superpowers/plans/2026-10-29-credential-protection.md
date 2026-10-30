# Enrollment Protection and Key Rotation (#13) Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans for the established native execution method. Execute task-by-task using the checkboxes. This plan does not authorize implementation, database resets, committing or pushing.

**Goal:** Complete #13 through the existing enrollment path and usable operator
key maintenance, removing service-credential code without production consumers.

**Architecture:** Retain account/deployment keys and encrypted authorization-code
recovery receipts. Select keys within account-locked issuer writes and migrate
receipts in bounded transactions. Both rotations operate on values the issuer
actually creates; later consuming workflows extend these same operations.

**Tech stack:** Rust 1.99.1, SQLx 0.9/PostgreSQL, existing RustCrypto AES-256-GCM
authenticated encryption, Serde/base64, zeroize, Tokio, Clap, shlex and selected
native keyring backends. Inherit workspace dependencies; add none for this work.

**Spec:** [Credential protection](../../credential-protection-proposal.md).
Coordinate with the [modeling/atomic-storage plan](2026-10-30-modeling-and-atomic-storage.md)
for shared codec, grants, schema and verification changes. Both specify the
agreed target for PR #58; check current code rather than assuming completed
checkboxes from an earlier plan establish this scope.

## Global constraints

- Preserve enrollment authorization, proof binding, token hashing and replay
  containment. Encryption rotation does not change client identities or refresh
  credentials.
- No dormant APIs, tables, variants, provider codecs or maintenance branches for
  future consumers. No Clacks-owned dead_code allowances, widened visibility,
  dummy callers or tests added to suppress warnings. Vendored code is excluded.
- Existing development-only migrations may be edited in place. Add no legacy
  decoder, compatibility section, schema-recreation guide or forward-copy path.
  Never reset the persistent environment as part of tests.
- Keep meaningful relational facts/links and whole encrypted envelopes. Use the
  small versioned codec and existing encryption; preserve authenticated context.
- Use account locks, short transactions, checked query files and committed SQLx
  metadata. Do not hold transactions across network/native-store operations.
- Preserve both operator rotation commands, existing 1–100 batch bounds,
  resumability, role separation, retained inactive keys and backup-key obligations.
- Follow the existing observability and testing contracts. Test actual Clacks
  decisions, not cryptography, SQLx locks, constraints or dependency behavior.
- Keep applications thin, modules focused, named fields and standalone rustdoc.
  No Python, xtask, test-only production APIs or direct TrustedQSL execution.

## Review focus

- A receipt taken before rotation must be resealed with the current active key
  when returned, retaining replay containment (Task 2).
- Removing unused service storage must leave no schema/grant/query/report branch
  referencing it, and must preserve actual enrollment values (Tasks 1 and 3).
- Retrying explicit source/target versions after committed progress must resume
  the same target and count only committed changes (Task 2).
- Deployment rewrap must include inactive account keys; key-file loss must not
  silently generate replacements (Task 2).
- Existing native enrollment references and command exporter shutdown must remain
  intact after cleanup (Tasks 2 and 3).

## Task 1: Remove unconsumed service-credential storage

**Files:** `lib/clacks-server/src/secrets/{credentials,rows,envelope}.rs`,
`src/secrets/envelope/` children, `migrations/0003_credential_protection.sql`,
`sql/secrets/credentials/`, `deploy/server/database-grants.sql`, server offline
query metadata, `lib/clacks-model/src/ids.rs` if CredentialId loses every consumer,
and affected server test-support modules.

**Produces:** Only account-key and receipt storage remains in #13; no new
replacement credential interface.

- [x] Remove put_in, remove_in, read_in, credential metadata/status/material
  wrappers and credential-only cryptographic helpers. Remove the service
  credential envelope kind/type, table, exclusive row types, statements and
  privileges. Remove CredentialId only if no other production consumer remains.
- [x] Preserve account-key and receipt formats, generated key projections,
  identity/ownership references and issuer context. Retain any migration content
  needed for those actual objects; do not delete an entire migration by name.
- [x] Remove CRUD-only tests. Replace rotation fixture setup that used credential
  writes with existing production issuer receipt creation, shared in test support
  only where needed. Do not retain a production writer just for fixtures.
- [x] Check the diff for provider-specific types, unused query metadata and
  documentation that still claims #13 supplies generic service persistence.
  Run existing issuer/store tests against disposable production-schema stores.

## Task 2: Keep both maintenance operations on real enrollment data

**Files:** `lib/clacks-server/src/maintenance/keys/{account,migration,references,report,rewrap,command}.rs`,
associated `sql/secrets/rotation/` and reference queries, key tests and
maintenance command-driver scenarios.

**Consumes:** Existing account keys and receipts created by issuer authorization.
**Produces:** The same discoverable generate/status/rewrap/rotate-account
commands with truthful receipt/key-only reporting.

- [x] Remove credential migration arms, counts, labels and error/report fields.
  Replace the single-family enum/dispatcher with direct receipt migration.
  Keep meaningful public versions, source causes and committed receipt counts.
- [x] Retain current active-key cutover, explicit adjacent source/target versions,
  interrupted resume, account locks, SQLx maintenance guard and bounded batches.
  Verify normal issuer writes select active keys inside their transaction.
- [x] Adapt the existing taken-receipt scenario: take through the issuer, rotate,
  put back through the normal path and exercise replay handling. Assert no
  restored old-key reference and unchanged authorization consequences.
- [x] Adapt interrupted rotation/rewrap scenarios to actual issuer receipts.
  Assert reuse of the target, retained meaning, inclusion of inactive keys and
  exact committed/remaining progress. Do not create another synthetic family.
- [x] Preserve key-only configuration, distinct database credentials, keyless
  read-only status and exact retry arguments. Retain real command-driver export
  on success/failure/interruption, and secret exclusion. Extend existing
  scenarios rather than repeating the suite for every telemetry setting.
- [x] Run server and web enrollment scenarios, operator help and existing
  command-driver tests with disposable database roles and production grants.

## Task 3: Native enrollment, documentation and integrated verification

**Files:** Existing desktop credential/enrollment modules and tests, server/
desktop guides, deployment examples, backup documentation and this proposal.

- [x] Preserve native enrollment namespace, references, bundle format and recovery.
  Keep shared entry construction already used by enrollment; remove unused
  directory-specific scaffolding if present. Run the existing native enrollment
  scenario on macOS with disposable entries; report unavailable native platforms.
- [x] Reconcile owning guides with receipt/key-only maintenance. State directory
  persistence/provisioning/lookup ownership in #17, QRZ in #31 and LoTW in #30.
  Keep public metadata/history/removal obligations with those consumers.
- [x] Regenerate and check server SQLx metadata using one disposable database
  with production migrations and runtime/maintenance/migrator roles. Remove
  metadata exclusively associated with deleted SQL files.
- [x] Run formatting, all-target check, strict Clippy and rustdoc, then the
  integrated checks listed in the modeling plan. Inspect actual results; do not
  transfer earlier test counts to the revised implementation.
- [x] Keep #13 open until code and verification meet this narrowed scope. Keep
  PR #58's description explicit about pending scope changes until then.
  No push or commit is implied by writing/updating this plan.

## Verification commands

Use the portable disposable-database/SQLx procedure in the server guide.
Read the user-owned environment inventory before any server startup. Never use
persistent development accounts for tests. Metadata jobs for the two stores
share scratch state and run sequentially.

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo test -p clacks-server --lib --locked -- --test-threads=1
cargo test -p clacks-web --locked -- --test-threads=1
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p clacks-admin --locked -- keys --help
git diff --check
```

Use existing test environment variables and LEPTOS_OUTPUT_NAME as documented by
the owning crates. Preserve the user's disabled Windows CI and quota limits.
Report native Windows/hosted verification as unavailable unless actually run.

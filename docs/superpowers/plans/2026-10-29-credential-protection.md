# Credential Storage and Key Rotation (#13) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for the established native execution method. Execute task-by-task, using the checkbox steps. This plan authorizes neither implementation nor pushing; wait for the user's execution request.

**Goal:** Extend the existing production credential protection with account-scoped
service-secret persistence, resumable online key rotation, and operator commands.

**Architecture:** Keep the existing account/deployment key hierarchy and native
enrollment storage. Select encryption keys inside account-locked write
transactions; migrate encrypted references in bounded transactions through the
existing `clacks-admin` application. Use SQLx's maintenance-lock guard rather
than another coordination service or persisted rotation-job framework.

**Tech stack:** Rust 1.99.1, SQLx 0.9.0/PostgreSQL, RustCrypto `aes-gcm` 0.11.1,
`zeroize` 1.9.1, Tokio, existing `tokio-util` 0.7.16 cancellation, Clap, and
the existing native keyring backends. Inherit dependencies from the workspace.

**Spec:** [Credential-protection design](../../credential-protection-proposal.md).
Read its deployment prerequisites, capability boundaries and proposed defaults,
plus [accounts.md](../../accounts.md), D236/D376/D380/D381 in the
[decision register](../../decisions.md), and the
[enrollment contract](../../client-enrollment-proposal.md).

## Global constraints

- Preserve Rust **1.99.1**, thin applications in `bin/`, libraries in `lib/`,
  workspace-inherited metadata/dependencies and Rust-only project tooling.
- Keep deployment wrapping keys outside PostgreSQL/images. Retain raw 32-byte
  keys, fresh random 12-byte nonces, 16-byte authentication tags and existing
  authenticated account-key/receipt formats. Additional authenticated data is AAD.
- Hash-based enrollment credentials and oauth-as replay behavior remain unchanged.
  Key rotation does not rotate refresh credentials or change client identities.
- Fixed SQL lives in checked `.sql` files, with committed offline metadata.
  Use release-managed server migrations and the existing role separation.
- Use account locks for live writes. Never hold a database transaction across
  external HTTP requests, browser interaction or native credential access.
- Online bounded migration, one maintenance advisory lock, the schema and command
  details below are engineering defaults from the design, not new product decisions.
- Batch size defaults to **100**, accepts **1–100**, and does not limit account
  size. Retain inactive account keys; add no automatic key destruction/scheduler.
- No generic vault/job framework, new source-writer registry, custom cryptography,
  placeholder credential pages, test-only production interfaces or failpoints.
- No directory bundle/codec/provisioning endpoint in #13. Those belong to #17;
  LoTW persistence encoding belongs to #27. Reuse production components later.
- Do not introduce raw LoTW private-key export to populate storage. Protect only
  actual secret material; public metadata stays relational and independently retained.
- Keep module files focused; examine/split any file approaching 600 lines. No
  `mod.rs` outside tests, facade re-exports, anonymous persistence tuples,
  production unchecked indexing or ordinary `expect`/`unwrap` calls.
- Use standalone rustdoc, named fields, `thiserror` for semantic domain errors
  where needed, and `anyhow` for propagated context; no unit wrappers around errors.
- Follow [observability.md](../../observability.md). Record committed effects,
  preserve ordinary control flow and framework spans, and exclude secrets.
- Preserve unrelated local research and the machine-local development environment.
  Tests/metadata preparation use one disposable PostgreSQL database with the
  documented migrator/runtime roles, never personal development accounts.

## Review focus

Each item is exercised by the task named here, rather than a second test matrix:

1. A receipt taken before cutover is put back afterward: it must not restore an
   old-key reference or lose replay containment (Task 2).
2. Removal wins before a stale credential update: stale work must not recreate
   material or cross an account/purpose boundary (Task 1).
3. Cutover/one batch commits before interruption: the same source/target command
   resumes the existing key and migrates only remaining references (Task 2).
4. An inactive account key still has current references: rewrap must include it,
   and missing required wrapping material must fail without silent rekeying (Task 3).
5. Native backend extraction or maintenance-config loading changes unrelated
   behavior: old enrollment entries remain usable and key commands need no
   OpenID Connect (OIDC), cookie or runtime credentials (Tasks 3 and 4).

---

## File and dependency map

Use these responsibilities and paths; adjust only private file splits if size
requires it, preserving the interfaces named by dependent tasks.

| Location | Responsibility |
| --- | --- |
| `lib/clacks-model/src/ids.rs` | Add `CredentialId` using the existing native ID convention; no new synchronized record kind |
| `lib/clacks-server/migrations/0003_credential_protection.sql` | Credential table and ciphertext-reference indexes; retain existing key table |
| `lib/clacks-server/src/secrets/{keys,receipt}.rs` | Existing key/envelope formats; caller-owned key selection and receipt reuse |
| `lib/clacks-server/src/secrets/{credentials,rows}.rs` | Encrypted credential operations and reusable named row fields |
| `lib/clacks-server/src/oauth/authorization.rs` | Seal receipts in the existing account-locked write transaction |
| `lib/clacks-server/src/maintenance/keys.rs` | Existing key-file generation plus named child-module declarations; no re-export facade |
| `lib/clacks-server/src/maintenance/keys/{account,migration,references,report}.rs` | Account cutover, bounded ciphertext migration, reference counts and operator results |
| `lib/clacks-server/src/maintenance/keys/{rewrap,config}.rs` | Wrapping-key migration and loading only required key settings |
| `lib/clacks-server/src/maintenance/cli.rs` | Thin subcommand mapping, distinct credentials, signals and nonsecret output |
| `lib/clacks-server/src/store/tests/{credentials,key_rotation}.rs` | Production-store/issuer/maintenance scenarios, registered by existing unit suite |
| `lib/clacks-server/src/store/tests/support/enrollment.rs` | Shared issuance fixture only if needed by existing and new issuer scenarios |
| `lib/clacks-desktop/src/credentials.rs` | Existing native entry construction extracted without changing storage identity |
| `lib/clacks-desktop/src/enrollment/credentials.rs` | Existing enrollment bundle codec and workflow-specific operations |
| `deploy/server/database-grants.sql`, owning guides | Real privileges, deployment ordering and repeatable operator procedures |

Task order: **1 → 2 → 3** for server work; **4** uses existing enrollment.
Task **5** verifies the integrated result. Do not create disposable applications
or in-memory production substitutes for any deliverable.

## Task 1: Transactional encrypted credential persistence

**Deliverable:** Account-scoped production secret storage, with existing receipt
writes selecting their encryption key inside the committing transaction.

**Files:** Native ID addition; migration and role grants; `secrets.rs` module
declarations; `secrets/{keys,receipt,credentials,rows}.rs`;
`oauth/authorization.rs`; `sql/secrets/credentials/` and existing key queries;
`store/tests/credentials.rs` and suite registration.

**Interfaces:** Keep all generic plaintext operations crate-private. Public
provider-facing operations will be supplied by their owning later capabilities.

- `CredentialIdentity { account: AccountId, credential: CredentialId,
  purpose: CredentialPurpose }`, with the five design purposes represented by
  a closed enum. Do not select client exposure through a mutable visibility flag.
- `SecretMaterial { payload_format: i32, bytes: Zeroizing<Vec<u8>> }`, with no
  secret-bearing `Debug`. The storage layer authenticates a positive format ID;
  its owning provider code supplies and interprets the actual codec later.
- `CredentialMetadata` has named identity, material revision, state,
  creation/change timestamps and optional payload-format fields.
  `StoredCredential { metadata: CredentialMetadata,
  material: Option<SecretMaterial> }` represents present/removed material.
- `keys::active_in(connection: &mut PgConnection, wrapping: &WrappingKeys,
  account: AccountId) -> anyhow::Result<AccountKey>` consumes the caller's
  already account-locked transaction. Keep `load_version`'s exact-version read.
- `credentials::put_in(connection: &mut PgConnection, wrapping: &WrappingKeys,
  identity: CredentialIdentity, expected_revision: Option<i64>,
  material: SecretMaterial) -> anyhow::Result<CredentialMetadata>`;
  `None` expects absence; `Some` expects the exact existing revision.
- `credentials::remove_in(connection: &mut PgConnection,
  identity: CredentialIdentity, expected_revision: i64)
  -> anyhow::Result<CredentialMetadata>` and
  `credentials::read_in(connection: &mut PgConnection, wrapping: &WrappingKeys,
  identity: CredentialIdentity) -> anyhow::Result<Option<StoredCredential>>`.
  Writers require the caller's account lock; reads use its consistent transaction.
- Name row projections in `secrets/rows.rs` and use them directly in both
  storage and migration; do not construct anonymous column tuples.
  Credential queries live in `sql/secrets/credentials/{select,insert,replace,remove}.sql`;
  extend existing `sql/secrets/{current,insert,version}.sql` and add the initial-key
  existence query needed by `active_in`.

- [ ] Add `credential_changes_preserve_account_purpose_and_removal` using accounts
  provisioned through `AccountStore`. Store opaque byte values through the real
  credential operations; these are storage fixtures, not invented provider codecs.
  Assert exact material recovery, no other-account lookup, wrong-purpose refusal,
  replacement advancing revision 1→2, removal clearing material at revision 3,
  and a stale revision-2 replacement leaving removal intact. Exercise explicit
  replacement of the removed row against revision 3. No SQL-trigger injection.
- [ ] Run the new scenario. Confirm failure corresponds to missing credential
  behavior, then implement the table/operations, rather than adding a fake store.
- [ ] Create `service_credentials` with the exact design columns, primary/foreign
  keys and present/removed field consistency. Add indexes for account/key-version
  credential lookup and non-null encrypted receipt references. Do not alter the
  existing active-account-key unique index or add rotation-job/global-setting tables.
- [ ] Seal new credential AAD using a named struct: discriminator, account, ID,
  purpose, material revision, payload format and key version. Preserve current
  account-key/receipt AAD encodings. Provider/public configuration is not serialized
  wholesale into secret material. Clear protected columns on removal; preserve
  public identity/metadata and accept no stale revision overwrite.
- [ ] Move account-key selection/provisioning into `active_in` and update every
  production encryption writer. Provision version 1 only for an account with no
  key rows; do not interpret missing active state as permission to replace keys.
  Remove the obsolete pool-owned key-provisioning path if it has no production use.
- [ ] In `authorization::{put,compare}`, acquire the existing account lock and
  perform guards before sealing on that connection. No nested pool transaction,
  permanent active-key cache, OAuth patch or changed replay behavior. Keep the
  existing issuer scenario with a one-connection pool as integration evidence;
  the actual stale-key regression is added with rotation in Task 2.
  Adapt existing `store/tests/receipts.rs` to the caller-owned transaction without
  retaining the obsolete provisioning API solely for that test. Preserve its
  actual account/code-context assertions; add no generic crypto round trips.
- [ ] Add runtime credential access and maintenance key/envelope privileges.
  Retain separate migrator authority; maintenance updates receipt envelope columns
  without authorization-state transitions. Use actual restricted roles in verification.
- [ ] Instrument meaningful credential operations with normal `#[instrument(skip_all,
  ...)]` spans and direct nonsecret identity/purpose fields. Transaction-consuming
  helpers must not report committed credential changes before their caller commits;
  the eventual owning workflow supplies that event. Do not add a metric per helper
  or log secret-bearing input/errors. Existing receipt boundary events retain their
  actual post-commit meaning.
- [ ] Run `cargo test -p clacks-server --lib credential_changes --locked` and the
  existing `code_exchange_activates_one_member` scenario with the disposable test
  environment. Regenerate/check server offline metadata. Commit this functional
  storage/transaction boundary as one coherent deliverable.

## Task 2: Resumable individual account-key rotation

**Deliverable:** A real production maintenance operation rotates an account key
online and migrates service credentials and existing code receipts without losing
authorization behavior or creating another key on retry.

**Files:** `maintenance/keys/{account,migration,references,report}.rs`, named module
declarations in `keys.rs`, checked `sql/secrets/rotation/` queries;
`store/tests/key_rotation.rs`; `Cargo.toml` for existing workspace `tokio-util`.

**Interfaces:** All are crate-private; the command-line interface (CLI) calls the
owning modules directly.

- `report::EncryptedCounts { credentials: u64, receipts: u64 }`.
- `report::MaintenanceEnd::{Complete, Interrupted}` distinguishes actual finish
  from user cancellation without a generic task/lifecycle abstraction.
- `account::AccountRotationRequest { account: AccountId, from_version: i64,
  to_version: i64, batch_size: NonZeroU16 }`; validate positive versions,
  checked `to = from + 1`, and batch size 1–100 before writes.
- `report::AccountRotationReport { account: AccountId, from_version: i64,
  to_version: i64, end: MaintenanceEnd, migrated: EncryptedCounts,
  remaining: EncryptedCounts }`. Counts describe this invocation, not an
  invented cumulative durable job history.
- `account::rotate(connection: PgConnection, wrapping: &WrappingKeys,
  request: &AccountRotationRequest, stop: &CancellationToken)
  -> anyhow::Result<AccountRotationReport>` owns a dedicated connection and
  SQLx maintenance guard until exit. It never borrows a pool session indefinitely.
- Production internal cutover and single-batch functions take the same
  `&mut PgConnection` and request. They own each real transaction and are called
  by the normal operation loop. Tests may stop at those transaction boundaries;
  add no public stepping API or command-line test controls.
- `migration::CiphertextFamily::{Credential, Receipt}` identifies the two actual
  encrypted schema families; no plugin registry or generic migration framework.
  Queries in `sql/secrets/rotation/` cover cutover state/insert/deactivation,
  per-family batch selection/update and reference counts. Receipt selection uses
  a named `ReceiptEnvelopeRow` in `secrets/rows.rs`, carrying account, stored code
  digest, approval purpose and envelope fields. It does not require the raw code
  string or duplicate the issuer's complete `CodeRow` projection.

- [ ] Add `account_rotation_resumes_and_preserves_credential_meaning`: create an
  account with at least three real credential rows; cut over 1→2 and migrate one
  row using batch size 1 through the production batch operation. Release the
  connection and invoke `account::rotate` with the same 1/2 request. Assert only
  remaining rows migrate, version 2 remains the sole active target, all materials
  remain readable and logical revisions/timestamps are unchanged. A repeated
  completed invocation migrates zero. Test stale/different target refusal without
  changing active state. Do not test PostgreSQL rollback as a separate property.
- [ ] Add `taken_receipt_is_resealed_after_rotation_and_still_contains_replay`:
  issue an actual approved grant through production issuer code; take its consumed
  record using the existing Storage interface, rotate the account and put the
  record back through the normal compensation path. Assert it references version
  2, and replay still invalidates the issued grant while another client's grant
  remains usable. Share existing approval support, not a new fake OAuth service.
- [ ] Run the scenarios and observe the unmet rotation behavior. Implement the
  cutover exactly: current V/new N creates N and switches active flags together;
  existing active N/inactive V resumes; incompatible states fail. Refuse a new
  rotation while older inactive versions retain encrypted references.
- [ ] Acquire `PgAdvisoryLock::new("clacks.key-maintenance.v1")` nonblocking on
  the owned connection. Use the selected library guard and explicit normal-exit
  release; do not invent lock IDs, leases, singleton rows or pool checkout hooks.
  Return an actionable busy result for a competing command. Normal requests take
  account locks only; no account lock is held while acquiring the maintenance guard.
- [ ] Implement deterministic per-family batches under the existing account lock.
  Recheck active target N, decrypt with exact V/context and re-seal with N/fresh
  nonce. Preserve semantic columns and update only envelopes. Select remaining
  source references directly; no saved progress cursor or enumeration snapshot.
  For receipts, reconstruct the existing AAD context from stored account/digest
  and immutable approval purpose, and update only receipt envelope columns.
  Missing context or partial envelopes fail explicitly rather than being filtered
  out of selection or recreated through issuer operations.
- [ ] Recheck all encrypted-family references under the account lock before
  completion. Keep inactive keys. Use normal reads with exact versions during
  migration; failure is not missing credentials and never causes regeneration.
- [ ] Honor cancellation before cutover and between committed batches. Complete or
  roll back the current transaction normally. Return an interrupted report with
  committed counts; on other errors add nonsecret target/count/retry context and
  retain the underlying error source. Do not add a telemetry-only Completion type.
- [ ] Integrate truthful `info` cutover/completion events and the committed
  `clacks_secret_values_reencrypted_total` counter. Add assertions to this real
  scenario for exact account/target/count correlation and absence of exercised
  secret values. Do not repeat the suite with telemetry disabled or test exporters.
- [ ] Run both scenarios plus the existing enrollment/receipt tests. Regenerate
  server SQL metadata and commit the working account-rotation operation.

## Task 3: Wrapping-key rotation, status and real operator commands

**Deliverable:** `clacks-admin` runs both rotations and reports actual current
references using only each operation's required credentials.

**Files:** `maintenance/keys/{rewrap,config,references,report}.rs`,
`maintenance/cli.rs`, checked status/rewrap queries, role grants and owning operator
guides. Extend `store/tests/key_rotation.rs`; do not create a new executable.

**Interfaces:**

- `rewrap::RewrapRequest { from: String, to: String,
  batch_size: NonZeroU16 }` and
  `report::RewrapReport { from: String, to: String, end: MaintenanceEnd,
  migrated_account_keys: u64, remaining_account_keys: u64 }`.
- `rewrap::run(connection: PgConnection, wrapping: &WrappingKeys,
  request: &RewrapRequest, stop: &CancellationToken)
  -> anyhow::Result<RewrapReport>` uses the same maintenance-lock key as Task 2.
- `references::status(connection: &mut PgConnection, account: Option<AccountId>)
  -> anyhow::Result<KeyStatus>`. `KeyStatus` has named deployment/account variants.
  Deployment status aggregates by wrapping-key ID rather than loading every
  account into a vector. Account status lists its versions, active designation,
  wrapping IDs and `EncryptedCounts`; an existing keyless account is explicit.
  Status uses a short read-only snapshot and loads/decrypts no key material.
- `config::KeySettings { wrapping_keys: WrappingKeyConfig }` with
  `KeySettings::load(path: &Path) -> anyhow::Result<Self>` uses the existing
  `config` crate/file source and normal server file layout. It does not validate
  unrelated server/OIDC settings or read their secret files.
- Retain `keys::generate(output: &Path)`. Extend the existing Keys subcommands
  exactly as specified below; `maintenance::cli::run` remains the binary entry.

- [ ] Add `rewrap_preserves_inactive_keys_and_rejects_missing_recovery_material`.
  Begin with W1, present values and a partially migrated account rotation so both
  active and inactive account-key versions have current references. Rewrap W1→W2
  using production operations. Assert both account-key versions move, current
  W1 references become zero, ordinary credential reads and actual issuer replay
  still work with W2-only live configuration, and material revisions are unchanged.
  Repeating the same rewrap moves zero keys.
- [ ] In that scenario, before successful rewrap, try the ordinary W2-only
  configuration while persisted W1 references remain. Assert an actionable failure
  and no new key version or substituted credential state. This is an actual
  operator configuration mistake, not a tampered ciphertext/schema fixture.
- [ ] Run the scenario, then implement one-account-at-a-time account-key batches,
  including inactive rows, with explicit source predicates and target matching
  the configured active wrapping key. Reuse existing account-key AAD/bytes and
  update only wrapping ID/nonce/ciphertext. Report observed remaining source rows.
- [ ] Implement status from the two known ciphertext families and key table.
  Avoid count multiplication when combining credential/receipt references.
  Exercise concrete counts from the rotation scenario, including the partial
  migration; do not test SQL aggregation mechanics in isolation.
- [ ] Add CLI forms:

  ```text
  keys generate --output PATH
  keys status --database-url-file PATH [--account ACCOUNT_ID]
  keys rewrap --config PATH --database-url-file PATH --from W1 --to W2
  keys rotate-account --config PATH --database-url-file PATH --account ID
                      --from-version V --to-version N
  ```

  Both mutating database commands accept `--batch-size` with default 100 and
  range 1–100. Use standard Clap value parsing; add no parser-only coverage.
- [ ] Open a direct `PgConnection` for mutating maintenance commands, with no
  surrounding account-wide transaction or runtime state construction. Status
  uses only the maintenance database file; generate uses no database/config.
  Require no browser cookie key, provider registration or runtime database file.
- [ ] Use Tokio Ctrl-C and, on Unix, termination signals to cancel the operation's
  `CancellationToken`, then await its normal batch exit and guard release. Do not
  spawn an unowned signal listener. Print actual committed/remaining counts and
  the exact safe retry invocation; interrupted/busy/failed commands exit nonzero.
  Log cancellation as expected operation termination, not fabricated failure.
- [ ] Increment `clacks_account_keys_rewrapped_total` after batch commits. Reuse
  the rotation telemetry scenario for meaningful committed outcomes/secret
  exclusion; do not attach account IDs to metric labels or add generic wrappers.
- [ ] Verify the shipped command noninteractively against the disposable database:
  `cargo run -p clacks-admin --locked -- keys …`. Use a key-only configuration
  file without public URL, cookies or OIDC fields. Run status, account rotation,
  repeat account rotation, rewrap, repeat rewrap and status. Confirm real outcomes
  through the production reader/issuer, not source-text checks or a fake runner.
  Do not add a public `run_from` API only for CLI tests or run Cargo inside tests.
- [ ] Document deployment ordering and compatible rollback targets: install the
  corrected writer everywhere and drain older writers before rotation; distribute
  readable W1/W2 before rewrapping; switch/drain W1-writing processes before
  declaring source references eliminated. Keep retained backup keys. No new
  process registry, artificial confirmation flag or automatic deletion is added.
- [ ] Run server scenarios and CLI acceptance with the restricted maintenance
  role, update offline metadata/grants, and commit the complete operator feature.

## Task 4: Preserve native enrollment while sharing the platform adapter

**Deliverable:** Existing production enrollment uses a reusable native-entry
adapter without changing any stored entry identity or inventing directory formats.

**Files:** `lib/clacks-desktop/src/credentials.rs`, library module declaration,
`enrollment/credentials.rs`; existing native enrollment scenario and desktop guide.

**Interfaces:**

- Shared crate-private `credentials::entry(service: &str, reference: &str)
  -> anyhow::Result<keyring_core::Entry>` owns only platform backend construction.
  Enrollment owns its existing `SERVICE` constant, bundle codec and
  `create/load/save_refresh/remove` operations. No facade re-export or store trait.
- Preserve namespace **`org.clacks-log.desktop.authorization`**, all opaque
  references, existing `CredentialBundle` serialization, macOS Keychain selection,
  Windows Local persistence and unsupported Linux behavior.

- [ ] Run the existing real native enrollment scenario before the extraction to
  establish its result; retain its browser/issuer, reload, refresh and completion
  assertions. Do not write another generic keyring round-trip test.
- [ ] Move the platform construction into the shared adapter and use it directly
  from enrollment. Do not add currently unused directory namespaces, data-transfer
  types, payload codecs, SQLite cache tables or provisioning commands. Typed
  directory payload definitions remain #17.
- [ ] Re-run that same native scenario with a disposable issuer and unique entries
  on macOS. For an actual pre-change application entry available in an isolated
  acceptance workspace, verify ordinary resume through the unchanged reference;
  do not use or mutate personal credentials. Record Windows native acceptance as
  unavailable unless it is actually performed, not inferred from compilation.
- [ ] Update the owning guide with the reusable boundary and #17 integration
  obligations, run desktop compilation/Clippy and existing component tests, and
  commit the production adapter extraction.

## Task 5: Integrated verification and portable operating instructions

**Deliverable:** Verified production storage/operator integration with explicit
platform and deployment limits; documentation describes actual implemented APIs.

**Files:** `docs/{credential-protection-proposal,accounts,backup-recovery,
server-development}.md`, `lib/{clacks-server,clacks-desktop}/README.md`,
`deploy/server/{README.md,config.example.toml,database-grants.sql}` as needed.

- [ ] Reconcile the design with delivered signatures/commands/defaults. Keep
  D236/D381 requirements separate from tuning and representation decisions.
  Do not leave the old out-of-transaction writer described as current behavior
  or imply that config-file activation itself rewrites existing key envelopes.
- [ ] Provide a complete disposable operator walkthrough: exact key-only file
  shape, raw key generation, migrator/runtime/maintenance roles and URL-file
  contents, writer rollout/drain prerequisites, both commands, status, interruption
  retry and backup-key retention. Do not record machine-specific paths/secrets.
- [ ] Verify status with a keyless account, a partial migration and completed
  migration. Keep all inactive rows readable; distinguish current-database
  reference counts from external backup/process evidence. Do not promise the
  CLI has verified deployment rollout, remote erasure or old backup disposal.
- [ ] Review added/changed tests against the concrete Clacks mistakes above.
  Remove dependency-only assertions, manufactured faults and unused test-support
  APIs. Keep any shared approval fixture in the test tree and reuse the linked
  server suite; do not duplicate the synchronization or crypto implementation.
- [ ] Run the final commands below. Inspect full results, including ignored cases
  and failures. Record macOS/native and unavailable Windows/CI evidence accurately.
  Only claim what was actually run. Fix actual regressions before finishing.
- [ ] Commit any final operational documentation with the owning deliverable;
  fold branch-introduced repairs into their original commits. Do not check in a
  review inventory or cleanup narrative as a product document.

## Verification commands and environment

Use the existing [disposable PostgreSQL procedure](../../../lib/clacks-server/README.md#disposable-local-postgresql).
One disposable database serves tests and SQLx metadata. Set:

```sh
export CLACKS_TEST_DATABASE_URL=postgres://clacks_migrator@127.0.0.1:5432/clacks
export CLACKS_TEST_RUNTIME_DATABASE_URL=postgres://clacks_runtime@127.0.0.1:5432/clacks
export LEPTOS_OUTPUT_NAME=clacks-web
export SQLX_OFFLINE=true
```

The example uses the documented disposable port 5432; adjust it if occupied.
Supply a separate synthetic maintenance-role URL file for actual operator
commands. Never substitute the persistent development accounts. Read the
user-owned local environment inventory before running/configuring a server;
preserve its provider registration, key files, secrets and Docker volume.

For focused scenarios, run the test-name filters specified in the owning task.
Then run the actual workspace suite serially; earlier parallel desktop pool-close
stalls have no established cause and do not justify a production workaround here.

```sh
cargo fmt --all --check
cargo test -p clacks-server --lib --locked -- --test-threads=1
cargo test -p clacks-desktop --lib --locked -- --test-threads=1
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked -- --test-threads=1
cargo test --workspace --doc --locked
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --document-private-items --locked
cargo leptos build --release
```

All selected commands must exit zero. Run server SQLx metadata preparation/check
from `lib/clacks-server` against the migrated disposable database, then check
client-store metadata using the existing scoped CI command. Run these sequentially:
SQLx shares a scratch directory. Do not create another metadata database or
change CI protocols, triggers, disabled Windows jobs or dependency tooling.

```sh
(
  cd lib/clacks-server
  SQLX_OFFLINE=false DATABASE_URL="$CLACKS_TEST_DATABASE_URL" \
    cargo sqlx prepare -- --all-targets --locked --offline
  SQLX_OFFLINE=false DATABASE_URL="$CLACKS_TEST_DATABASE_URL" \
    cargo sqlx prepare --check -- --all-targets --locked --offline
)
```

For the unchanged client schema, create a disposable SQLite database through
`cargo sqlx database create`, migrate it with source
`lib/clacks-client-store/migrations`, and run from the workspace root:

```sh
SQLX_OFFLINE=false DATABASE_URL=sqlite://clacks-query-check.sqlite \
  cargo sqlx prepare --check --workspace -- \
  --package clacks-client-store --all-targets --locked --offline
```

Use that same explicit SQLite URL for creation and migration. Remove only the
disposable metadata database afterward; do not stage it or modify its schema.

Inspect the actual key-command `--help`, configured privileges, committed results
and failure/retry diagnostics. A new account key changes only encryption, not
provider material revisions, issuance/confirmation facts or client enrollment.
Whole-branch review must compare production behavior with the design, not use
coverage counts as an objective. Keep operator/browser/private data out of fixtures.

## Commits and execution handoff

Keep coherent commits for storage/transactional writes, account rotation,
rewrap/operator integration, and native adapter extraction. A final verification
step does not justify a separate commit unless it delivers substantive documentation.
Before every commit/amend, prepare and inspect a substantive message file,
hard-wrap prose at 72 columns, verify physical line lengths, then use
`git commit --file`. Preserve literal URLs and identifiers. Never stage the
unrelated local toolkit research.

The execution method is native, as established in this conversation. Wait for
the user's implementation request; do not ask them to choose the method again.
At execution time reuse/create an isolated checkout under the user-owned
worktree-placement guidance. No push, PR creation, live infrastructure change
or issue closure follows merely from this plan. #13 delivers production
components; later capabilities extend them and their real integration evidence.

# Credential storage and key rotation proposal

Engineering proposal for [issue #13](https://github.com/jhelwig/clacks-log/issues/13).
This is for review, not an implementation plan or a claim of delivered behavior.
The [account contract](accounts.md), D236/D376/D380/D381 in the
[decision register](decisions.md), and the [recovery contract](backup-recovery.md)
remain authoritative. The [enrollment contract](client-enrollment-proposal.md)
defines the existing authorization integration.

SQL means Structured Query Language; SQLx is the selected Rust database library.
Additional authenticated data (AAD) binds encrypted bytes to their owning record.
An envelope contains ciphertext, a nonce and the key version needed to decrypt it.
A nonce is a fresh value used for one encryption. Key rotation changes encryption
material; it does not change a provider password, renew a certificate or revoke a
client. OpenID Connect (OIDC) deployment credentials remain separate.

## 1. Settled requirements

- Independent randomly generated account encryption keys protect account-owned
  service secrets. A deployment wrapping key protects those account keys and is
  supplied outside PostgreSQL and the application image (D236).
- Include deployment wrapping-key rotation and individual account encryption-key
  rotation through `clacks-admin` in MVP (D381). Deployment automation may invoke
  the commands; an internal rotation scheduler is not required.
- Use established authenticated encryption and bind ciphertext to its account
  and credential identity. Retain recovery material required by supported backups;
  never replace an unreadable key silently.
- Protect desktop secrets in native macOS and Windows storage through the selected
  keyring libraries (D376). SQLite contains nonsecret references. Ordinary log data
  requires no application-level encryption or unlock workflow (D194).
- Only directory credentials may be provisioned to enrolled clients. Publication
  credentials and LoTW private signing material remain server-side (D96/D97).
- Credential replacement/removal preserves public certificate/destination metadata
  and submission history. Disconnected clients may retain previously provisioned
  secrets; no remote-erasure or rollback-proof revocation guarantee is made.
- Preserve the existing opaque, proof-bound enrollment credentials and oauth-as
  code-replay policy (D378/D380). Rotation is not refresh-token rotation.
- Keep one Clacks server application with PostgreSQL, release-managed migrations,
  account-scoped authorization and existing account-row locking. No external key
  service is mandatory, and no custom cryptographic algorithm is introduced.

## 2. Existing implementation and capability boundaries

Issue #12 already provides AES-256-GCM authenticated encryption through RustCrypto,
versioned `account_encryption_keys`, protected deployment key files, encrypted
consumed-code receipts, and native desktop enrollment storage. AES-256-GCM means
Advanced Encryption Standard with a 256-bit key in Galois/Counter Mode. Reuse these
production components; do not add a parallel vault or encryption stack.

#13 extends that implementation with service-secret persistence, operational key
rotation, reference reporting, and reusable native storage wiring. Existing code
receipts are an immediate production consumer of rotation and transaction changes.
The service-secret store is the production component later owners integrate.

| Capability | Owning issue |
| --- | --- |
| Account keys, encrypted persistence, rotation and native protection | #13 |
| Full-account synchronization and its authorization boundaries | #14 |
| Directory selection, credential forms, provisioning and reconnect application | #17 |
| Directory requests and provider-session handling | #20 |
| LoTW archive import, public certificate persistence and removal workflow | #27 |
| Publication configuration and routing | #28 |
| Submission ownership, credential acquisition and dispatch integration | #29–#31 |

#13 adds no placeholder credential page, signed-batch queue, provider adapter,
secret-distribution protocol or public generic secret-download endpoint. Later
capabilities extend its actual persistence and native-store operations.

## 3. Proposed engineering defaults and alternatives

The following are recommendations, not additional agreed product decisions:

| Choice | Proposed default and tradeoff |
| --- | --- |
| Rotation availability | Online cutover followed by bounded migration. Ordinary requests continue; an account's writes can briefly wait for a migration batch's existing account lock. A single large transaction is simpler but can hold that lock for the whole migration. Stopping the server is an available self-hosting procedure, not the core design. |
| Progress | Existing key/version references identify remaining rows. No durable rotation-job table or custom queue. Explicit source/target arguments make retries unambiguous. |
| Maintenance concurrency | One SQLx-provided PostgreSQL advisory lock serializes key-maintenance commands across batches. Ordinary application requests use account locks and never take this maintenance lock. |
| Batch size | At most 100 encrypted rows per transaction; configurable downward with a positive `--batch-size`. Process account keys and each ciphertext family separately. A single unusually large value still requires its own bounded input policy in its owning capability. |
| Retired account keys | Keep inactive wrapped key rows in MVP. Status reports remaining current references; no automatic key destruction or backup-retention inference. |
| Payload encoding | Public metadata is relational. Secret material is encrypted bytes with an explicit owner-defined format; no searchable plaintext projections or JSON copies of whole configuration records. |
| Local stores | Extend the existing native backend adapter; retain separate enrollment and directory entries. No plaintext fallback, new keyring trait for fake tests or general secret-management framework. |

Direct authenticated encryption with the existing hierarchy is selected by D236;
an external key service or disk encryption alone does not replace it. The proposed
bounded migration uses ordinary PostgreSQL and SQLx facilities rather than adding
a separate job engine to satisfy interruption recovery.

## 4. Proposed server schema

Use the existing `clacks` schema and checked query files with committed SQLx
offline metadata. Native IDs are canonical text IDs. Versions are positive
PostgreSQL `BIGINT` values in the checked Clacks counter range. Secret values,
nonces and ciphertext use `BYTEA`; public instants use `TIMESTAMPTZ`.

### 4.1 Account encryption keys: retain the existing table

`account_encryption_keys` retains:

| Column | Meaning |
| --- | --- |
| `account_id`, `key_version` | Existing composite primary key, account foreign key |
| `wrapping_key_id` | Exact deployment key needed for this wrapped key |
| `wrapping_nonce`, `wrapped_key` | Existing authenticated envelope |
| `active` | Key used for newly encrypted values; existing partial unique index permits at most one active key per account |
| `created_at` | Actual key creation time; rewrapping does not change it |

The initial account key is provisioned lazily under the account lock, as today.
When no active key exists, create version 1 only if the account has no key rows.
Existing inactive rows without an active key are an error, not permission to
silently generate a replacement. Rotation switches active rows in one transaction.
No global deployment-key registry or active-key setting is added to PostgreSQL;
the deployment configuration remains the authority for new wrapping operations.

### 4.2 Service credentials: one encrypted material row per logical credential

Add `service_credentials`:

| Column | Meaning |
| --- | --- |
| `account_id`, `credential_id` | Composite primary key; account foreign key; stable native credential ID |
| `purpose` | Closed application-defined purpose: `qrz.directory`, `hamqth.directory`, `qrz.logbook`, `lotw.account`, or `lotw.signing` |
| `material_revision` | Starts at 1; advances on explicit value replacement or removal, not on re-encryption |
| `state` | `present` or `removed` |
| `payload_format` | Owner-defined format version for the protected value; present material requires a supported format |
| `key_version` | Nullable reference to `(account_id, key_version)` in `account_encryption_keys` |
| `nonce`, `ciphertext` | Nullable authenticated envelope, with the tag included in ciphertext |
| `created_at`, `changed_at` | Logical creation/change times; encryption maintenance changes neither |

Present rows require complete material/format/key fields. Removed rows retain
identity, purpose, revision and public change metadata but clear the protected
fields. Public provider configuration/certificates reference the credential's
account-qualified identity from their own relational tables; they do not live
inside its ciphertext. Removing secret material does not cascade-delete them.

Create an index on `(account_id, key_version)` for migration and reference counts.
The account-qualified key foreign key prevents a credential from referencing
another account's encryption-key row. Purpose is immutable for an existing ID.
An explicit subsequent replacement may supply new material for a removed logical
credential under its exact current revision; a stale write cannot undo removal.

`material_revision` identifies changed credentials for later provisioning and
stale-write checks. It is distinct from `key_version`: rotating encryption must
not appear as a provider-credential change, trigger redistribution or change
public routing identity. This is not another synchronization clock.

The store accepts protected bytes only through purpose-specific owning code.
Purpose is not a caller-controlled `client_visible` flag. Directory provisioning
can request directory purposes; it cannot ask the same endpoint for signing or
publication secrets. Internal decryption is crate-private and account-scoped.

LoTW account login material for download queries is separate from its signing
credential; changing the login does not change signed-upload routing. QRZ logbook
credentials are distinct from QRZ directory credentials even if an operator uses
the same provider account. Neither is inferred from another stored purpose.

The exact protected directory/provider fields belong to #17/#28/#30. LoTW persistence
encoding belongs to #27, which must work with the existing importer without
introducing reusable private-key export. #13 does not invent those payloads or
add a public raw-key accessor merely to populate this table.

### 4.3 Existing code receipts remain in their current schema

Keep `desktop_authorization_codes.receipt_key_version`, `receipt_nonce` and
`receipt_ciphertext`, their account-qualified key foreign key and expiry cleanup.
Do not move transient code records into `service_credentials` or change their
oauth-as state machine. Add an account/key-version index for encrypted receipt
references. Migration covers every persisted non-null receipt, including expired
ones not yet collected; expiration is not proof that a reference is gone.

The complete set of encrypted account data in #13 is service-credential material
and these code receipts. Every future encrypted table must join the same rotation
and reference-reporting operations before it becomes a production writer. This
is an explicit list of schema families, not a dynamic plugin registry.

## 5. Encryption and authorization boundary

Reuse raw 32-byte keys, fresh OS-random 12-byte nonces, RustCrypto's 16-byte tags
and zeroizing plaintext/key buffers. Use standard serialization of a named AAD
struct; do not concatenate ambiguous strings or invent a cryptographic format.

Keep the existing `clacks.account-key.v1` account/version binding for wrapped
account keys and `clacks.code-receipt.v1` receipt binding. New credential AAD
contains format discriminator `clacks.service-credential.v1`, account ID,
credential ID, purpose, material revision, payload format and encryption-key
version. Re-encryption changes its key-version field and nonce, preserving all
logical identity fields. Read with the exact stored version, never a fallback key.

No secret-bearing value derives unredacted `Debug` or enters ordinary synchronized
record payloads. OIDC secrets/cookie keys keep their deployment-file handling;
ordinary enrollment credential digests stay hashes and are not re-encrypted.

Browser operations derive authority from the existing authenticated principal;
jobs derive it from their durable account-owned work. Tenant IDs supplied by a
request are not authorization. Keep authenticated account qualification in the
actual access queries, with no new row-level-security policy or per-tenant
database-login scheme. Operator maintenance deliberately has deployment-wide
authority through its separate database credential and wrapping-key access.

## 6. Transaction boundaries and runtime concurrency

### 6.1 Create, replace or remove a service credential

In one ordinary SQLx write transaction:

1. Lock the owning `accounts` row using the existing query and revalidate current
   authorization at the owning browser/job boundary.
2. Check the expected credential revision/state (or expected absence for create),
   immutable purpose and all owning configuration dependencies.
3. For a value write, load/provision the active account key on this connection,
   encrypt the material, and save its new logical revision and complete envelope.
   For removal, clear protected fields and advance the logical revision.
4. Commit related public configuration effects together when the owning workflow
   requires them. Emit committed change telemetry only after commit.

No transaction spans browser interaction, provider requests or native OS storage.
Replacement overwrites current protected material; #13 adds no archive of old
provider passwords/private keys. Public history remains separate. PostgreSQL
recovery logs and backups may still contain older bytes; removal is not secure
erasure of those copies.

### 6.2 Correct the existing receipt write boundary

Today `oauth::authorization::{put,compare}` encrypt receipts before beginning
their write transaction, and `account_key` uses a separate pool transaction.
That can persist an old-key envelope after an account rotation cutover.

Move key selection/provisioning and receipt encryption inside the caller's
account-locked transaction. The key helper takes the existing connection;
it does not open another transaction or reacquire the account lock through a pool.
For conditional final activation, first check the existing code/approval guards,
then seal and commit under the existing code/grant transaction. The library's
take/put-back and replay behavior stays unchanged. An old record returned by an
earlier take is re-sealed using the active key when put back.

All production encryption writes follow this rule. No long-lived active-key cache
can bypass it. Runtime wrapping keys may remain loaded as today because deployment
rollout controls their availability, as described in section 8.

### 6.3 Reads and ordinary credential use

Read ciphertext and its referenced wrapped key from a consistent transaction,
decrypt with their exact versions, then release database resources. Inactive
account keys remain available, so an in-flight read remains interpretable while
other rows migrate. A missing key or failed authentication is an actionable
storage/configuration failure, not an absent credential or anonymous success.

Credential acquisition and removal use the owning workflow's existing account
transaction and task lifecycle. No independent queue of reusable signed batches
is introduced. #27–#31 coordinate signing/submission ownership with removal;
they must not permit removed material to be newly acquired for work. Network
requests never run inside an account-lock transaction. An existing external
effect is not undone by deleting local credentials. #13 provides the storage
operations, not a new dispatch protocol or an extra product policy for that race.

## 7. Individual account-key rotation

### Deployment prerequisite

Before enabling rotation, deploy the section 6.2 writer correction to every
application instance and drain all older writers. An older binary can select V
before its transaction and write V after the new binary's migration completes;
account locking inside the new command alone cannot repair that ordering.
All live readers must also have the wrapping keys needed for the new envelopes.
Deployment automation establishes these prerequisites; the CLI cannot prove
them by inspecting database references or require a new instance registry.

Once rotations are enabled, a supported application-rollback target must retain
the corrected write boundary and required key/format support. A pre-#13 binary
without that correction is not a supported rollback target while rotation may
run. Schema compatibility alone is insufficient. A rollout can first deploy the
compatible writer correction and only later invoke the maintenance commands.

Proposed command arguments explicitly identify source V and target N, with
`N = V + 1` using checked counter arithmetic. These are operational retry
parameters, not secret values or a new rotation-history graph.

### 7.1 Cutover transaction

Acquire the maintenance guard described below, then lock the account row.

- If V is active and N does not exist, require that no encrypted current rows
  still reference an older inactive account key. Create random key N wrapped
  under the configured active deployment key, mark V inactive and N active,
  and commit together. Do not create a key for an account with no key to rotate.
- If N is already active and V exists inactive, this invocation resumes the
  same target. Do not generate another key or increment the version again.
- Any other active/source/target combination is a clear refusal with current
  public versions. Do not roll back an active key or guess another target.

The unique active-key index remains in place; within the cutover transaction,
deactivate V before inserting active N. Readers see either committed state.

### 7.2 Bounded migration transactions

For each encrypted table, repeat account-locked transactions:

1. Recheck that N is active, and select at most the configured batch size of
   rows still referencing V, in stable primary-key order.
2. Decrypt each under V with its original AAD; encrypt under N with new nonces
   and updated encryption-key AAD. Persist ciphertext and reference together.
3. Preserve material revisions, code states, expiry, public metadata and all
   authorization/processing meaning. Commit the batch before reporting progress.

New or replaced values use N after cutover. Removal can delete material awaiting
migration. Neither operation introduces new V references. No saved enumeration
cursor is needed: subsequent batches select remaining V references directly.
Existing receipts returned before cutover cannot reintroduce V under section 6.2.

Under the account lock, confirm zero current references to V in all encrypted
tables before reporting migration complete. Inactive wrapped V remains in the
database. Starting another account-key rotation is refused while any earlier
inactive version still has current ciphertext references; resume that migration.

### 7.3 Interruption and concurrency

- Before cutover commit: no new version is visible; rerun the same command.
- After cutover, before any migration batch: new writes use N and old values
  remain readable under V. Rerun the same V/N invocation.
- After some committed batches: rerun selects only remaining V rows. If the last
  commit succeeded but its report was lost, the retry finds nothing left.
- Failure within a batch rolls back only that batch. Report account/table/key
  context without plaintext and stop; do not skip unreadable material and claim
  completion. Repair configuration/data through ordinary recovery, then resume.

Other accounts continue normally. The affected account's writes may wait for a
short batch, but no lock is held across batches or external calls. Cancellation
uses ordinary transaction rollback/commit semantics; committed progress survives.

## 8. Deployment wrapping-key rotation

The configured active wrapping key is local to each application instance.
Changing it in the CLI cannot update other running processes. Avoid a parallel
database configuration authority or a custom replica-registration mechanism.

Use the normal deployment rollout:

1. Generate W2 outside the database/image. Mount W1 and W2, and deploy that full
   readable key set to every application instance and maintenance runner.
2. Change the configured active key to W2 and roll/drain all W1-writing processes.
   During overlap both versions remain readable. Do not start rewrapping before
   all readers can load W2; finish draining old writers before declaring W1's
   current references eliminated.
3. Run rewrap with explicit `--from W1 --to W2`. Require that W2 matches the
   maintenance configuration's active key and W1 differs from W2.
4. In bounded transactions, lock the owning account row and re-encrypt each
   account-key envelope still wrapped by W1 under W2 with a fresh nonce. Include
   inactive account-key versions; otherwise they still require W1 on restoration
   or reads. Account key bytes/versions/active flags and credential ciphertext
   do not change.
5. Report remaining W1 references from the current database. Repeating the same
   command processes only remaining W1 rows. Preserve every key file required by
   retained backups, even when current references are zero.

Process one account per transaction, with at most the configured batch size of
its key rows. The maintenance guard prevents a competing key-maintenance command;
ordinary fresh-key creation uses W2 after the deployment rollout. Completion is a
statement about observed current database references under those rollout
preconditions, not proof that every process or historical backup was inspected.
No command removes deployment key files or claims to verify external backup
retention. Removing a key from live configuration is an operator deployment step
after current use and rollout are checked; its protected backup may still be needed.

## 9. Maintenance coordination and proposed commands

Use SQLx 0.9's `PgAdvisoryLock` with a stable namespaced key for the mutating
database commands `rewrap` and `rotate-account`. Status is an ordinary consistent
metadata read; key-file generation does not open a database connection.
Acquire it nonblocking on one dedicated PostgreSQL connection for the command's
lifetime, and perform successive short transactions on that same connection.
A competing command reports busy and exits; it creates no persisted job or lease.
Use the guard's explicit asynchronous release on normal exit and its documented
drop/connection-close behavior for interruption. Never return a bare, still-locked
session to a pool. Transaction-pooling proxies cannot supply this session
contract; maintenance uses a direct PostgreSQL connection or session pooling.

Application writes use account locks only. Lock order is maintenance guard, then
one account row, then that account's key/credential/code rows in deterministic
order. No command locks every account at once or waits for another command while
holding an account lock. Default batch size is an engineering default, not an
account-size limit; use ordinary bounded database lock waits and actionable errors.

Proposed CLI shape, using the existing thin `bin/clacks-admin` wrapper:

```sh
clacks-admin keys generate --output /run/secrets/wrapping-w2

clacks-admin keys status --database-url-file /run/secrets/maintenance-db
clacks-admin keys status --database-url-file /run/secrets/maintenance-db \
  --account ACCOUNT_ID

clacks-admin keys rewrap --config /etc/clacks/server.toml \
  --database-url-file /run/secrets/maintenance-db --from W1 --to W2

clacks-admin keys rotate-account --config /etc/clacks/server.toml \
  --database-url-file /run/secrets/maintenance-db --account ACCOUNT_ID \
  --from-version 1 --to-version 2
```

`--batch-size` optionally adjusts the migration batch count. Normal output is a
nonsecret summary: actual target, committed counts and remaining references per
encrypted table. Commands are not remote job submissions. Exit success only
after the requested reference migration completes; on busy, interruption or
failure, report retained progress and the exact retry invocation. A keyless
account is reported as such rather than initialized by a status operation.

| Operation | Required material |
| --- | --- |
| `keys generate` | Protected writable destination directory; no database, cookie key or provider credentials |
| `keys status` | Maintenance database credential; no wrapping keys or encrypted-value decryption |
| `keys rewrap` | Maintenance database credential plus configured old/new wrapping keys |
| `keys rotate-account` | Maintenance database credential plus wrapping keys needed for that account's current/historical envelopes and the configured active wrapping key |

Parse only the key-maintenance settings needed from the normal config file;
do not connect through the runtime database credential or require OIDC setup,
cookie keys or a listening server. Extend the existing maintenance role with
account-key reads/inserts/updates and encrypted-value reads/envelope updates.
Receipt updates need only the receipt envelope columns; maintenance must not
perform authorization-code state transitions or actor mutations. Runtime keeps
its existing account-key permissions and gains only required credential access.
Schema changes remain under the separate migrator role.

## 10. Native client storage integration

Move the existing platform-entry construction into a focused shared desktop
credential module used by enrollment and directory storage. Retain selected
native backends and Windows Local persistence; no alternative secret store,
custom storage trait or Linux plaintext fallback is introduced.

The #13 implementation extracts that adapter and keeps existing enrollment as
its production consumer. It preserves the native namespace, opaque references
and serialized enrollment bundle encoding, and reruns the existing real native
enrollment/reload/completion scenario. It does not add a directory DTO, codec,
cache registry or otherwise unused directory-specific API. DTO means data
transfer object. Those production operations and their integration scenario are
delivered with the typed provider configuration in #17.

Keep the existing enrollment service namespace and entry references unchanged.
Use a separate directory namespace and opaque random entry references. Directory
bundles carry selected server origin, verified account/credential identity,
purpose and material revision. Enrollment retains its existing bundle, which
can be created before browser approval establishes the account identity.
Reject an unexpected owner or purpose before returning directory material to a caller.
Directory and enrollment
entries are separate: replacing/removing a directory credential cannot remove
the enrollment signing key or persistent refresh credential.

#17 owns the typed directory bundle and durable association with its shared
configuration. Its consumer must save the new native material before committing
the local nonsecret reference/applied revision, and clean superseded entries
only afterward. Interrupted application retries from those actual boundaries;
there is no new generic completion registry in #13. This adds no SQLite table
for future provider configuration or secrets. Existing enrollment's proven
attempt/binding recovery remains its own workflow.

Only the selected directory configuration may provision directory material using
current account/client authorization and encrypted transport. #17 supplies that
endpoint and reconnect workflow using #14; ordinary account replication must not
carry server-only secrets. Removing server credentials does not erase copies on
offline clients. Native failure preserves the last installed usable credential
and reports the provisioning problem; it does not erase log data or claim a
successful update. No application-level credential export command is added.

## 11. Observability and meaningful verification

Use existing tracing/structured logging facilities. Significant committed key
cutovers and completed maintenance actions are `info`; an interrupted migration
retains truthful committed counts and an actionable outcome. Handling-boundary
decryption/database failures are distinguishable from absent/removed credentials.
Include operator database identity, public key versions, account ID where scoped,
table and committed counts. Do not log key bytes, plaintext values, native bundle
contents, database URLs, code/token/proof strings or decrypted receipts.

Suggested metrics are `clacks_account_keys_rewrapped_total` and
`clacks_secret_values_reencrypted_total`, incremented after their batch commits.
Bounded ciphertext-family labels may distinguish credential values from receipts;
account/credential IDs are tracing fields, never metric labels. Normal operation
spans use framework timing; no generic telemetry completion abstraction is added.

Verification should catch these specific Clacks mistakes:

| Scenario | Incorrect Clacks behavior it detects |
| --- | --- |
| Existing production issuer receipt is taken, an account key is rotated, then the normal library put-back/replay path runs | Persisting the old encryption version after cutover or losing replay containment references |
| Account-owned credential replacement/removal through the production store | Mixing account/purpose ownership, accepting stale updates over removal, or changing logical credential revision during encryption maintenance |
| Cutover and one real migration batch commit, then the maintenance operation is reopened with the same V/N arguments | Allocating an unintended additional key or losing committed progress on retry |
| Actual rewrap plus subsequent existing issuer authorization/replay | Rewrapping only active keys, changing account-key contents, or requiring an old deployment key after current references migrated |
| Existing native enrollment/reload/completion through the extracted backend (#13) | Stranding enrollment by changing its native namespace, reference or bundle encoding |
| One real rotation with configured telemetry | Reporting uncommitted success/counts or exposing exercised secret material |

Use the actual schema, SQLx transactions and selected native stores with disposable
state. Do not test AES primitives, SQLx advisory-lock mechanics, uniqueness
constraints or keyring round trips as independent Clacks coverage. No injected
triggers/schema damage, failpoints, test-only public APIs, Python fixture tooling
or artificial provider services. Later real service owners extend these tests
with their typed credential and provisioning integration. In particular, #17
verifies that directory replacement/removal preserves enrollment material and
that the native owner/purpose checks apply to its actual bundle. Do not invent
a directory fixture format in #13 to claim that downstream acceptance early.

## 12. Recovery, delivery and remaining engineering work

Back up the database consistently with its versioned account-key envelopes.
Retain every deployment key required by supported backups, regardless of current
migration status. Restoring an older database can restore removed secrets and
older authorization, or omit later replacements. Do not silently merge native
client secrets back into server-only storage or promise erasure from old backups.
Follow the existing upload pause/catch-up/explicit-resume procedure on restore.

The proposal requires no changes to the persistent personal development
environment. Implement using disposable databases and unique native entries;
native Windows acceptance remains distinct from macOS verification and CI status.

An implementation plan should translate these proposed contracts into focused
modules, checked queries/migration grants and tests. Exact Rust names, internal
module splits, query batching details and error-type organization are builder
choices. They must not introduce a generic vault/job framework or choose provider
payload formats that belong to later capabilities. No implementation or push is
authorized merely by this proposal.

## Sources and implementation evidence

- [PostgreSQL row/advisory locking](https://www.postgresql.org/docs/current/explicit-locking.html): transaction row locks and session-scoped maintenance exclusion have different lifetimes.
- [SQLx 0.9 advisory-lock guard](https://docs.rs/sqlx-postgres/0.9.0/sqlx_postgres/struct.PgAdvisoryLock.html): standard acquisition, owned-connection access, explicit release and queued release on drop; checked against the installed 0.9.0 source.
- [OWASP key management](https://cheatsheetseries.owasp.org/cheatsheets/Key_Management_Cheat_Sheet.html): lifecycle, rotation, accountability and recovery considerations; it does not select a Clacks rotation interval or new mandatory external key service.
- Existing [`secrets`](../lib/clacks-server/src/secrets.rs), [account keys](../lib/clacks-server/src/secrets/keys.rs), [receipt context](../lib/clacks-server/src/secrets/receipt.rs), [receipt writer](../lib/clacks-server/src/oauth/authorization.rs), and [native enrollment storage](../lib/clacks-desktop/src/enrollment/credentials.rs) define the production integration being extended.

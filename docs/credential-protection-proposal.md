# Credential storage and key rotation

This proposal defines [issue #13](https://github.com/jhelwig/clacks-log/issues/13):
protect the existing enrollment-recovery path and maintain its account and
deployment keys. PR #58 also applies the modeling and atomic-storage changes in
the [implementation plan](superpowers/plans/2026-10-30-modeling-and-atomic-storage.md).
These are agreed delivery boundaries; the engineering defaults below do not add
product requirements. This document specifies the target, not implementation
completion or verification results.

## 1. Settled requirements

- Use independent random account encryption keys protected by a deployment
  wrapping key outside PostgreSQL and application images (D236).
- Include individual account-key rotation and deployment wrapping-key rotation
  through `clacks-admin` in MVP (D381).
- Preserve existing enrollment, opaque proof-bound credentials, authorization-code
  replay containment, client revocation and reauthorization. Encryption-key
  rotation is not refresh-credential rotation.
- Use established authenticated encryption and retain every key required by
  supported backups. Never silently replace an unreadable key.
- Protect desktop enrollment secrets through the selected native macOS/Windows
  stores. SQLite retains nonsecret references; ordinary log data needs no new
  encryption or unlock workflow.
- Directory credentials may be provisioned to enrolled clients. Publication
  credentials and LoTW (Logbook of The World) private keys remain server-side.
  Disconnected clients may retain previously provisioned secrets.
- Preserve account isolation, public historical metadata and submission history
  when credentials are replaced or removed. Their consuming workflows own those
  transitions.

## 2. Delivery ownership

| Owner | Usable outcome |
| --- | --- |
| #13 / PR #58 | Existing enrollment writes encrypted recovery receipts using account keys; operator commands rotate those keys, rewrap them and report actual references. Preserve native enrollment storage. |
| #14 | Full-account synchronization and its authenticated transactions. |
| #17, incorporating #20 | Web directory setup, typed encrypted credential persistence, protected client provisioning, actual desktop QRZ/HamQTH lookup and synchronized snapshots. |
| #31, incorporating the QRZ/common parts of #28/#29 | QRZ setup, protected credentials, saved-station routing, publication, queries, retries, pauses and essential controls. |
| #30, incorporating #27 and the LoTW parts of #28/#29 | Certificate import/removal/supersession, protected signing/account material, routing, signing, submission and queries extending #31. |
| #33 | Remaining cross-service diagnostics and management, not controls required to operate an earlier service. |
| #32 / #34 | Confirmation reconciliation and integrated service-evidence recovery. |

#13 does not deliver a dormant service-credential table, generic secret CRUD API,
provider codec, credential envelope or service-credential migration branch.
Remove these from PR #58 together with their exclusive queries, role grants,
offline metadata, fixtures and tests. Do not widen visibility or add artificial
callers to retain them. Both rotation commands remain because enrollment already
creates encrypted receipts and wrapped account keys.

The first directory workflow introduces its actual credential representation,
account-authorized write/read/removal operations and extension to both reference
reporting and account-key rotation in the same delivery. Subsequent services
extend that working integration. No generic secret-download endpoint, new
framework or unconsumed future-provider variant is required.

## 3. Engineering defaults

Use existing RustCrypto AES-256-GCM (Advanced Encryption Standard with a 256-bit
key in Galois/Counter Mode), SQLx 0.9, PostgreSQL transactions, native stores and
the existing thin operator application. No new cryptography or serializer
dependency is needed.

Online rotation uses a committed cutover followed by short account-locked
migration batches. The default batch size is 100, configurable within 1–100.
Retain inactive wrapped keys. Explicit source/target arguments and remaining
database references provide retry state; add no rotation-job table, scheduler,
instance registry or generic ciphertext-family dispatch for absent consumers.

## 4. Stored objects and encoding

Retain `account_encryption_keys` with account/version identity, active state,
creation time and one complete wrapped envelope. Retain authorization-code
identity, account, lifecycle, expiry and one optional complete encrypted receipt
on the existing code row. Preserve their account-qualified foreign keys,
uniqueness, generated key-reference projections and maintenance indexes.

The currently supported packed forms are:

```text
$clacks$1$account-key$<wrapping-key-reference>$<nonce>$<ciphertext-and-tag>
$clacks$1$receipt$<account-key-version>$<nonce>$<ciphertext-and-tag>
```

Version 1 fixes the existing cipher, fresh random 12-byte nonces and 16-byte
authentication tags. Use existing base64url without padding. Encode the exact
nonempty UTF-8 deployment key ID as base64url, so it cannot introduce delimiters.
Account-key versions use canonical positive decimal in the checked integer range.
Reject unsupported versions/kinds, wrong field counts, invalid/noncanonical
encoding, wrong nonce length and ciphertext shorter than its tag.

Bind additional authenticated data (AAD) to independently expected account/key
or account/code/purpose context and the storage header. Never trust an envelope
to select its owner. Retain typed fixed-size keys/nonces and zeroized plaintext.
Generated reference columns are projections, not independently writable shadow
state. Do not duplicate closed Rust variants or domain validation in SQL checks.

Service-credential payloads and their envelope type are introduced with #17's
real consumer. They must remain complete encrypted units with owner, purpose,
identity and material-revision authentication. This does not authorize adding
their codec, table or generic APIs in #13.

## 5. Receipt transactions and authorization

Select/load the active account key inside the existing account-locked receipt
write transaction. A receipt taken before cutover and subsequently returned must
be sealed under the current active key, not restore its old envelope. Preserve
the existing code states, issued-reference meaning, expiry and replay behavior.

No transaction spans browser interaction, network calls or native secret access.
Encryption maintenance changes neither semantic revisions nor authorization.
Keep runtime, maintenance and migrator credentials distinct.

## 6. Account-key rotation

The deployment must first ensure every running writer selects the active key
inside the account-locked write transaction and every reader can load the
required wrapping keys. Use normal deployment rollout/draining; the command
cannot prove process compatibility from database references.

With explicit source V and target N, require checked `N = V + 1`:

1. Acquire the maintenance guard, then lock the account row.
2. If V is active and N absent, require no references to older inactive versions.
   Deactivate V and create/activate random N in the same transaction. Do not
   initialize a keyless account just to rotate it.
3. If N is already active and V inactive, resume that same rotation. Refuse other
   combinations; never guess a target or allocate another version on retry.
4. In stable order, migrate at most the selected batch size of receipt rows
   referencing V. Recheck N under the account lock, authenticate/decrypt with V,
   encrypt with a fresh nonce under N and commit before reporting progress.
5. Preserve code state, expiry, approval context and issued references. Recheck
   zero current V references under the account lock before reporting completion.
   Keep inactive V wrapped in the database.

New writes use N after cutover. Retrying the same V/N command selects remaining V
references; it requires no cursor or saved job. A failed batch rolls back only
that batch and stops with contextual, secret-free diagnostics. Cancellation
preserves committed progress. Another rotation is refused while an earlier
inactive key still has current references.

## 7. Deployment-key rewrapping

1. Generate W2 outside the image/database. Deploy W1 and W2 to every reader and
   maintenance runner.
2. Set W2 active through normal deployment configuration and drain W1 writers.
3. Run rewrap with explicit source W1 and configured active target W2.
4. In bounded account-locked transactions, rewrap every W1 account-key envelope,
   including inactive versions, with a fresh nonce. Account-key bytes, versions,
   active state and receipt ciphertext do not change.
5. Report committed counts and remaining current W1 references. Retry selects
   remaining rows. Never delete key files or infer historical backup safety from
   zero current references.

Process one account per transaction. Supported rollback binaries must retain the
writer boundary and formats used by active rotations. Retain protected key files
needed by older backups even after their live references reach zero.

## 8. Commands and coordination

Use SQLx's session-scoped `PgAdvisoryLock` with the existing
`clacks.key-maintenance.v1` name for mutating maintenance commands. Acquire it
nonblocking on one owned direct/session-pooled PostgreSQL connection, then take
one account lock per transaction. Ordinary writers take account locks only.
Use the guard's release/drop facilities; no custom lease or job machinery.

```sh
clacks-admin keys generate --output /run/secrets/wrapping-w2
clacks-admin keys status --database-url-file /run/secrets/maintenance-db
clacks-admin keys status --database-url-file /run/secrets/maintenance-db --account ACCOUNT_ID
clacks-admin keys rewrap --config /etc/clacks/server.toml \
  --database-url-file /run/secrets/maintenance-db --from W1 --to W2
clacks-admin keys rotate-account --config /etc/clacks/server.toml \
  --database-url-file /run/secrets/maintenance-db --account ACCOUNT_ID \
  --from-version 1 --to-version 2
```

`--batch-size` adjusts the bounded row count. Generate requires only a protected
destination directory. Status requires only maintenance database access and must
not create a key or decrypt values. Mutating commands also require the configured
wrapping keys. They do not require provider setup, cookie keys, runtime database
credentials or a running web server. Receipt grants permit envelope replacement,
not code-state or actor mutation; schema changes use the migrator.

Busy, interruption and failure exit nonzero with truthful retained progress and
the exact retry invocation. Existing retry output uses Portable Operating System
Interface (POSIX) shell quoting. Success means current references have migrated
under the stated rollout preconditions.

## 9. Native storage and downstream integration

Preserve enrollment entry namespace, reference, bundle encoding and completion
recovery. Shared native-entry construction may remain where enrollment already
uses it; do not add directory-specific entries, probes or bundles in #13.

#17 owns typed directory material, native owner/purpose validation, separate
entries, configuration revision and reconnect application. Native writes precede
committing the corresponding nonsecret reference; superseded entries are cleaned
afterward. Failure retains the last usable configuration. Include credential
replacement/removal and current account-key selection in that actual workflow.

#31/#30 keep publication/private signing material on the server and implement
account-authorized lifecycle changes with their actual routing and dispatch
consumers. Do not hold account locks across external calls. Removal preserves
public history, prevents new acquisition of removed material, and does not undo
an external effect. Reuse and extend the existing key-maintenance operations;
do not ship a dormant parallel store to anticipate these consumers.

## 10. Observability and verification

Follow [observability.md](observability.md). Use normal framework spans,
significant committed cutover/completion events at info, and handling-boundary
errors retaining their sources. Report account IDs and public versions in
traces, never metric labels. Exclude plaintext, tokens, proofs, key bytes,
envelopes, database URLs and arbitrary payloads.

Keep `clacks_account_keys_rewrapped_total` and
`clacks_secret_values_reencrypted_total` truthful to committed work. Remove labels,
counts and report fields for absent service-credential storage. Do not retain a
family enum or generic migration framework for one currently supported family.

The operator process owns its opt-in OpenTelemetry export and shuts exporters
down on success, failure and interruption. Existing native enrollment, receipt
replay and maintenance scenarios establish:

- Receipt take/put-back across cutover cannot restore an old key reference.
- Retrying after committed batches resumes N and preserves replay containment.
- Rewrapping includes inactive account keys and retains the same receipt meaning.
- Missing required key material produces an actionable failure, not silent rekeying.
- Real command-driver telemetry reports only committed counts and excludes secrets.
- Existing native enrollment/reload/completion retains its storage identity.

Use disposable databases and the production migrations/grants. Keep fixtures in
test support and actual receipt creation in the existing issuer path. Remove
tests whose only purpose is exercising deferred CRUD. Do not substitute generic
round trips, dependency tests, injected triggers, test-only production APIs or
dummy callers.

## 11. Recovery

Back up PostgreSQL consistently with its account-key envelopes and the protected
deployment keys required to recover them. Restoring an older database can restore
older authorization or omit later changes. Existing upload pause/catch-up/explicit
resume procedures still apply when publication is available. Neither rotation
nor revocation can recover facts lost from every surviving copy.

See the [server operator guide](../lib/clacks-server/README.md#credential-storage-and-key-maintenance)
for command/configuration usage. The specification and plans define the target;
operator instructions must be reconciled when implementation changes land.

## Sources and existing integration

- [PostgreSQL explicit locking](https://www.postgresql.org/docs/current/explicit-locking.html)
  describes the distinct transaction-row and session-advisory lock lifetimes.
- [SQLx advisory-lock guard](https://docs.rs/sqlx-postgres/0.9.0/sqlx_postgres/struct.PgAdvisoryLock.html)
  supplies standard acquisition, connection access and release facilities.
- [OWASP key management](https://cheatsheetseries.owasp.org/cheatsheets/Key_Management_Cheat_Sheet.html)
  informs lifecycle and recovery; it does not impose a Clacks rotation interval
  or mandatory external key service.
- The existing [receipt writer](../lib/clacks-server/src/oauth/authorization.rs),
  [account-key implementation](../lib/clacks-server/src/secrets/keys.rs) and
  [native enrollment storage](../lib/clacks-desktop/src/enrollment/credentials.rs)
  are the production consumers preserved by this proposal.

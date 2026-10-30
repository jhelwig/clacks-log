# Backup and recovery

Scope decision D57, 12 October 2026: documented backup and recovery procedures are sufficient for minimum viable product (MVP); built-in backup/restore commands are post-MVP. D240 explicitly calls for a client backup/restore process that accounts for restored or duplicated client state, with implementation outside MVP. Storage formats and exact procedures remain engineering/design work.

## Recovery needs

Amateur Data Interchange Format (ADIF) export is accepted for interchange and contact migration (D44/D56). It does not establish a full-installation backup format for application identifiers/revisions, deletion memory, pending client edits, service observations, publication attempts, download progress, profiles, routing configurations, or signing credentials. D185 explicitly rejects a completely lossless ADIF export/re-import requirement; native backup and synchronization must preserve their own required state.

Server recovery must consider accepted shared records, deletion information, profiles, service state, routing and credentials, and the deployment wrapping-key versions required to decrypt the restored encrypted account keys under D236. The [synchronization model](synchronization-model.md) governs recovery of an older server alongside newer clients; do not assume an ordinary client download can safely overwrite newer local state.

Client recovery must consider offline-only contacts, pending edits/conflicts, and client-local defaults/state. Rejoining an empty client retrieves accepted server state but cannot recover unsynchronized changes that were only on a lost client. D56's ADIF migration path does not claim to preserve every kind of client state.

## Accepted MVP scope (D57)

Provide documented backup and recovery procedures for server and client storage, using the selected storage and credential-protection mechanisms. Built-in Clacks Log backup/restore commands are post-MVP. This does not require a dedicated in-application backup user interface (UI) or scheduler in MVP.

Restore installation data consistently, including the recovery material needed for protected credentials; partial recovery must not silently fabricate publication success or discard pending work. D236 selects per-account encryption keys protected by a deployment wrapping key outside PostgreSQL; recovery needs the wrapping-key versions corresponding to the restored encrypted account keys, with protected key backups. The documented procedures must establish backup consistency, restore verification, required credential material and resumed-job behavior. Built-in client restoration remains deferred; the synchronization model defines timeline recovery and its detection limits.

This document covers installation/device/server failure recovery. D55 still defers the dedicated action to restore individual deleted radio contacts (QSOs). No automatic backup cadence, hosted-service recovery commitment, or claim that synchronization alone is a backup is established here.

## Server rollback through normal sync (D58)

An older restored server is a synchronization condition that the normal protocol must tolerate. Reconcile surviving client-held synchronized state by negotiating shared/missing state; do not overwrite newer client data merely because the server is the coordinator. Divergent edits still follow D04's record-level conflict and field-level resolution rules. Previously synchronized data and genuinely pending client changes must remain distinguishable.

The prior proposal to require an explicitly declared reset, suspend normal sync, and defer automatic reconstruction was rejected. No manual log reconciliation or ADIF re-enrollment is required solely because the server rolled back. D56 still applies to enrolling an unrelated independently populated log; an existing enrolled client reconnecting to a restored server is a different case.

D59 uses symmetric negotiation/transfer of synchronized state, coalescing working edits at synchronization boundaries without requiring every edit as a history event. It does not select a logbook-wide commit, branch, or QSO-as-file model. D60 requires retention/representation sufficient for enrolled clients to return after indefinite offline periods without export or re-enrollment. The [metadata model](synchronization-model.md) defines revision identity, retained ordering evidence and reconstruction; database and service-evidence integration must preserve those contracts. The protocol must not mistake reused server counters for identical revisions. Service state recovery also needs design: an older backup may lack later upload success or download-processing information. Preserve and reconcile surviving synchronized evidence rather than treating rollback as proof of upload failure or a new download event. Protected server-only credential material still needs D57's documented backup/recovery procedures; normal sync does not imply copying private keys onto clients.

D140 requires recovery-relevant publication outcomes, remote observations, and processing decisions to reach enrolled clients in MVP, alongside the existing evidence needed to interpret them. This makes client-held service evidence eligible for normal rollback repair. Recovery must preserve the association between a publication and its actual sent values/destination, and between an observation, its processing decision, and any resulting accepted edit. Missing or uncertain results must not be fabricated. The [service-evidence contract](service-evidence.md) defines retention and combination; persistence and transport must preserve it.

Recovery includes account-owned QSOs with their complete tag sets, tag definitions/deletion state and service associations in the single account synchronization scope (D243/D248). Restore must distinguish active untagged contacts from QSOs marked deleted. A surviving pending QSO with a deleted-tag reference remains local for explicit membership removal/replacement; it must not recreate the tag (D250). Recovered external associations must preserve the one-local-QSO-per-external-record invariant within each account/service destination (D251); divergent claims require resolution rather than duplicate binding or silent replacement.

## Operator-controlled publication during restore (D141)

The user pauses all uploads for rollback/restore, lets normal synchronization recover as current a state as possible from available sources, and explicitly resumes uploads afterward. Restoring an older backup must not inadvertently undo that operator pause; the documented restore procedure must address safe startup, with exact mechanisms/control scope still open. Sync remains available for recovery. This does not mandate a manual sync reset or re-enrollment, waiting for all indefinitely offline clients, or pausing downloads.

Conflict detection and resolution belong to clients. A client announcing a conflict may never return, resolve it, or submit the resolution. Server publication must not depend on receiving that resolution; the per-QSO automatic rollback-conflict pause proposal is rejected. Normal upload eligibility, cooldown, routing, and service-failure pause rules still apply after the user's restore pause is lifted.

D142 provides per-client connection state, last successful synchronization, and current sync error as sufficient MVP diagnostics for this operator judgment. No separate catch-up/currentness metric is required. These do not establish that unavailable clients have no missing state.

## Limits of available recovery evidence (D143)

Rollback restores earlier state. A client revocation stored only in the rolled-back server state can be lost when restoring a backup predating it. Do not assume independently preserved authorization state or promise that later revocations remain effective across full rollback. No rollback-surviving authorization store or additional authorization-review workflow is required. Normal sync may recover facts actually retained by surviving copies; it cannot reconstruct lost facts with no surviving evidence.

## Database backup profiles

The [16 October server persistence comparison](../references/server-persistence.md) compares consistent backup and recovery across the candidate architectures, recommends PostgreSQL, and documents physical base backups with recovery-log archiving as the production profile, plus a self-contained scheduled base-backup option for simpler self-hosting with an explicit between-backup loss window. Established external database tools/procedures satisfy D57; no built-in Clacks backup command is added. Separate configuration and credential-protection recovery material remain part of the recovery set. Exact cadence, retention, recovery-time/data-loss objectives and tooling are unselected.

The comparison keeps PostgreSQL physical recovery separate from Clacks timeline ancestry/client reconciliation. Restore startup must keep uploads paused independently of an older database's saved pause flag, retain normal synchronization for recovery, and require the D141 explicit operator resume. A whole-instance database restore can affect every account; database recovery does not guarantee survival of later revocations or missing upload outcomes under D143. D213 selects PostgreSQL for the server and SQLite for the client; the detailed backup profiles remain recommendations. Client SQLite recovery must preserve local working state, pending edits and comparison evidence, not only synchronized contact values. Use supported consistent backup or a properly closed database copy rather than copying an arbitrary live main file; no built-in backup command is added.

## Explicit client backup/restore process (D240)

Plan an explicit desktop-client backup/restore process covering local database/state, enrollment and causal identity, pending work and comparison evidence, and protected credential handling. Implementing it is not an MVP requirement; documented recovery under D57 remains the MVP scope. The contents and format of a backup, inclusion or reprovisioning of protected credentials, and the identity handling on restore are not yet selected.

Copying all relevant directories can create another copy of the same logical client state. A different physical computer does not inherently make that copy distinguishable to Clacks, and two writable copies can also exist on one computer. Physical relocation alone does not require fresh enrollment. Do not assume a backup contains credentials held in an operating-system store, or assume such a store reliably distinguishes copies. The future explicit process must account for these cases without losing contacts, pending work or their comparison evidence. Existing revocation/manual-reconciliation and server-rollback decisions remain in force; this scope decision selects no new causal mechanism or mandatory clone-detection workflow.

## Browser account recovery

The account server stores stable accounts, linked identities, current login
validity and administrator membership in PostgreSQL. Back up these tables
consistently, along with protected provider registrations, the durable cookie
key and deployment configuration. Cookie-key replacement invalidates browser
cookies without removing account/identity data. An older database restore can
restore previously revoked login/role state; a later revocation cannot survive
rollback without separately retained evidence.

Administrator-assisted identity recovery is an additive, explicit maintenance
operation. Verify the account owner and exact replacement issuer/subject out of
band before running it, and before that identity's first ordinary sign-in creates
another account. It preserves existing identities/logins and refuses an identity
owned elsewhere. See [Linux deployment](../deploy/server/README.md) and the
[operator commands](../lib/clacks-server/README.md#roles-and-exceptional-recovery).
These operations do not implement service-upload recovery.

## Desktop authorization recovery

Back up account encryption-key rows and desktop authorization tables in the
same consistent PostgreSQL backup. Retain every referenced deployment wrapping
key in the protected secret backup; ciphertext alone cannot recover the consumed
code receipts. All application instances need the same version-to-key mapping.
Changing the active version does not rewrap existing account keys or retire old
versions. Do not remove an old wrapping key while any account envelope uses it.

D381 includes wrapping-key rotation and individual account-key rotation through
`clacks-admin` in MVP. Rewrapping or re-encrypting current database contents does
not rewrite existing backups. Preserve the older wrapping-key material needed
to decrypt the account keys in every supported backup, even after current rows
have migrated. Rotation procedures must identify the retained recovery material.
The [credential-protection contract](credential-protection-proposal.md) defines
online migration and retry behavior; the
[operator guide](../lib/clacks-server/README.md#credential-storage-and-key-maintenance)
provides exact commands and key-only configuration. Before rotation, deploy the
account-locked writer everywhere and drain older instances. Supported application
rollback targets must retain that writer and support all live key/envelope formats.
Current zero reference counts do not verify process rollout or backup disposal.

An older database restore can lose client enrollments or restore authorization
that was revoked after the backup. This is the ordinary rollback boundary,
not rollback-proof revocation. Reapply known revocations after restoration.
Missing/closed actors require browser approval for a fresh identity; existing
authorized actors retain their identity when replacing credentials. Local old-
actor pending work remains held for explicit reconciliation after catch-up.

Desktop credentials live in the native macOS/Windows credential store, not in
SQLite or the workspace directory. A database copy alone does not copy them.
Use the workbench connection workflow to authorize again when needed. Interrupted
local completion can resume from the original workspace's saved attempt without
reassigning standalone contacts or inventing a synchronization checkpoint.

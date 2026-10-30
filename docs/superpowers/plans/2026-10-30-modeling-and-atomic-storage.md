# Modeling and Atomic Storage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for the established native execution method. Implement task-by-task using the checkboxes. Writing this plan does not authorize execution, resetting development data, committing or pushing.

**Goal:** Make saved domain values valid by construction and store conceptual
atomic objects as units, preserving Clacks's agreed product behavior.

**Architecture:** Keep native application facts and relationships relational.
Keep coupled domain state behind checked Rust construction and transitions;
decompose it only at actual SQL binding boundaries. Store encryption envelopes,
configuration bodies, frozen field maps, exact remote keys, vectors and coverage
maps atomically through ordinary Serde/SQLx facilities.

**Tech stack:** Existing Rust **1.99.1**, Serde/serde_json, SQLx **0.9.0**,
PostgreSQL, SQLite, RustCrypto aes-gcm, zeroize, Tokio, egui/eframe and the selected
OAuth/OpenID Connect libraries. Inherit workspace dependencies. No new database,
serialization framework, cryptographic algorithm or application is required.

**Spec:** The [agreed target contract](#agreed-target-contract) below consolidates
the approved modeling and atomic-object proposals. Product semantics remain in
[native-model.md](../../native-model.md),
[synchronization-model.md](../../synchronization-model.md),
[service-evidence.md](../../service-evidence.md),
[operating-settings.md](../../operating-settings.md),
[client-enrollment-proposal.md](../../client-enrollment-proposal.md),
[credential-protection-proposal.md](../../credential-protection-proposal.md) and
[desktop-ui.md](../../desktop-ui.md).

**Baseline:** This plan was prepared against PR #58 at `63399d3`, including the
credential-protection work. Execution needs that work or its merged equivalent;
do not start from a checkout that lacks it. Recheck repository state at execution.

## Global constraints

- This is a representation/storage refactor, not renewed product design. Preserve
  matching, fallback, confirmation applicability, upload policy, reconciliation,
  enrollment security, token replay handling, rotation and UI behavior.
- There are no active schema consumers outside development. Revise the existing
  migration files in place as a coordinated clean-schema change. Do not build
  forward-copy migrations, transitional columns, dual readers/writers or legacy
  data converters. Preserve the ordinary SQLx migration mechanism.
- Existing development stores will need explicit reset/recreation before using
  the revised migrations. Never suppress SQLx checksum checks, rewrite its history
  or silently recreate a database on open. Preserve external secrets, provider
  registration, deployment configuration and native credential storage.
- Use disposable stores for verification. Reset of a persistent development
  database or workspace is a separate authorized operational action, not a test
  prerequisite. Future deployed-schema evolution still follows the documented
  release/startup migration policies.
- Keep `lib/<crate>` and thin `bin/<application>` applications. No catch-all
  `crates/`, Python, xtask, executed TrustedQSL reference sources or temporary
  production substitutes.
- Preserve unknown mode/band/reference values, supplied locator precision,
  whole-second QSO times, whole-hertz frequencies, milliwatt power and all agreed
  signed-64-bit storage bounds. Recognition changes remain interpretation only.
- Keep existing native wire representations where a private Serde conversion
  suffices. No synchronization protocol change or new actor/timeline identities.
  Database storage documents are separate codecs, not new synchronized values.
- Preserve authenticated encryption's existing additional authenticated data
  (AAD) bindings, including purpose spelling, ownership, identity and revision.
  Authenticate codec metadata too. Changed AAD requires re-encryption; storage
  conversion and encryption maintenance never change semantic revisions.
- Retain account locks, conditional saves, foreign keys, uniqueness, transactions
  and live authority checks. Rust construction cannot establish current shared
  state, reference existence, authorization or external side effects.
- Keep checked fixed queries in `.sql` files and commit generated SQLx metadata.
  Use existing serialization facilities; no generic object store or custom write
  queue.
- Use focused modules and standalone rustdoc, named record fields, thiserror for
  actual domain errors and anyhow for propagated context. No ordinary production
  `expect`/`unwrap`, unchecked indexing, facade exports or test-only public APIs.
- Follow [observability.md](../../observability.md). Preserve existing spans,
  meaningful events, truthful committed metrics and handling-boundary failures.
  Never log packed bodies, plaintext, ciphertext, nonces, credentials or AAD.
- Tests must exercise Clacks behavior. No injected database triggers, corrupted
  migration history, dependency-behavior tests, compile-fail tests proving Rust
  privacy, or coverage-driven matrices.

## Agreed target contract

### Domain construction

Saved domain values must retain their intrinsic invariants. Private fields plus
checked construction/updates are sufficient; do not invent an enum or wrapper for
every field. Use data-bearing enums where alternatives require different data.
Deserialization must go through the same checked boundary. Raw form values,
provider/library input and SQL decoder rows may be incomplete, but cannot pass
into saved values or ordinary write APIs without conversion.

Do not create an invalid saved `Qso` as an entry draft. Existing form/import state
remains outside the saved-record model. Do not replace genuinely partial provider
evidence with a requirement for complete contacts. In particular, preserve
`Reported<T>` omission/value/clear meanings, partial confirmation bases and
independently optional issued access/refresh references supplied by oauth-as.

### Storage boundaries

| Family | Target storage and actual consumer |
| --- | --- |
| Service credential material | One optional versioned encrypted value, including payload codec and account-key envelope; ownership/revision/purpose/timestamps remain relational |
| Wrapped account encryption key | One versioned wrapping envelope; account/key-version identity, activation and creation time remain relational |
| Consumed-code receipt | One optional account-key envelope; code identity, lifecycle, account and expiry remain relational |
| Identifier rules and catalog-display/lookup binding | Complete configuration documents consumed by the Rust definition engine; group identity/name/location-context property remain ordinary fields |
| Ordinary export bindings and targeted export plans | Atomic rule documents; each targeted plan retains its output-profile relationship and ownership |
| Frozen service field maps | Whole maps consumed by projection/evidence interpretation; record/destination/dependency relationships remain relational |
| Exact provider record key | One canonical provider-specific key; account/destination-qualified equality and uniqueness remain database operations |
| Record vector and timeline coverage | Whole native maps on qualified owner rows; accepted stamps/checkpoints/baselines and retention references remain relational |
| Requested/reported field sets and output-context field sets | Whole sets; current consumers compare/evaluate them in Rust |
| Local preferences and resumable documents | Keep atomic local-state values, tightening their typed contents rather than splitting them into columns |
| Native QSO/station/operator/equipment/directory facts | Keep meaningful fields independently queryable for history, sorting, geographic containment and effective/recorded queries |
| Memberships, activity/reference associations and evidence links | Keep relational relationships used for joins, matching, dependency installation and retention |
| Authorization and synchronization reference structure | Keep addressed entities, eligibility/expiry, replay identities, memberships and reference edges relational |

Do not pack entire QSOs, all service evidence, the whole account or the whole
synchronization graph. Atomic values are not independent record identities or new
retention roots. Retain unknown values, stable IDs, set semantics and ordered rule
sequences without reinterpretation during storage conversion.

### Storage encoding defaults

These are engineering defaults implementing the contract, not new product rules:

- PostgreSQL encrypted values use one `TEXT` column and the small versioned codec
  defined in [credential protection](../../credential-protection-proposal.md#51-versioned-storage-codec).
  Its exact shapes are:

  ```text
  $clacks$1$credential$<account-key-version>$<payload-format>$<nonce>$<ciphertext-and-tag>
  $clacks$1$receipt$<account-key-version>$<nonce>$<ciphertext-and-tag>
  $clacks$1$account-key$<wrapping-key-reference>$<nonce>$<ciphertext-and-tag>
  ```

  Version 1 fixes the existing RustCrypto AES-256-GCM cipher, fresh OS-random
  nonces and combined ciphertext/tag output. No new crypto or serialization
  dependency, algorithm negotiation, JSON envelope or general codec framework.
- Encode nonce/ciphertext with existing base64url without padding. Encode the
  exact nonempty UTF-8 deployment key ID the same way; arbitrary IDs cannot add
  delimiters. Positive account-key versions and credential payload formats use
  canonical decimal within their checked integer ranges. Reject unsupported
  versions/kinds, wrong field counts, noncanonical encodings, wrong nonce length
  and ciphertext shorter than the 16-byte tag. Retain decoded nonces as
  `[u8; 12]` and secret keys as `Zeroizing<[u8; 32]>`. Distinct typed envelope
  shapes avoid discriminator-and-options state; never include secret key bytes.
- Reconstruct AAD from independently expected account/credential/code context,
  preserving current purpose, identity and revision bindings. Authenticate codec
  version, kind, key reference and credential payload format alongside it, using
  named structures and existing Serde serialization. An envelope cannot choose
  another owner's context. AAD bytes may change; no old-ciphertext compatibility
  path is required under the development-schema reset policy.
- Keep indexed generated stored projections from fixed field 5 using
  `split_part(..., '$', 5)`: cast account-key versions to `BIGINT` for existing
  account-qualified foreign keys; retain encoded deployment key references for
  wrapping-key selection. Rust encodes source IDs for lookup and decodes them
  for display. These projections have no independent writer. Do not add a SQL
  envelope decoder, JSON-header function or separately writable key reference.
- SQLite atomic objects use versioned JSON `TEXT` with ordinary Serde. Existing
  native vector/coverage Serde representations already use typed entry sequences.
  Preserve canonical set ordering with existing ordered collections. No generic
  `AtomicObject` trait, custom binary framing or fallback-to-default decoding.
- The exact external-key codec is deliberately canonical, not arbitrary JSON
  text: UTF-8 compact Serde JSON arrays with fields in this order: QRZ uses
  service label `qrz` followed by its canonical decimal ID as a string; LoTW uses
  label `lotw`, own call, worked call, integer start seconds, band, receive band
  and mode. Retain exact native labels and
  callsigns; never normalize against a newer catalog. SQLite uses binary text
  equality and a unique `(account, destination_id, canonical_key)` key. Own this
  private codec in the external-key mapper; do not expose a public test API.
- Raw SQL row types remain for decoding. On writes, bind from the domain variant
  or an encapsulated encoding constructed exclusively from that variant. Do not
  introduce another public/crate-public discriminator-and-options write record.

### Database responsibilities

Remove closed-enum allowlists, domain ranges/text normalization checks and
variant/nullability checks that merely restate validated Rust values. This
includes credential purposes, mode/position variants, counter/locator/zone ranges,
export-expression cases and sync observation/review variant shapes.

Keep native SQL column types/nullability, primary/foreign keys, set and identity
uniqueness, account-qualified ownership edges and active-account-key uniqueness.
Keep fixed-kind qualifications that make a composite foreign key point to the
correct record family; they are relational qualification, not a service enum
catalog. SQLite boolean representation checks may remain only as the physical
type's `0/1` representation, not as additional business policy. Do not add new
`CHECK`s to compensate for weak Rust models or test the database's constraints as
though those were Clacks behavior.

## Review focus

Each item belongs to an existing real scenario below, not a new test matrix:

1. Unchanged native values retain unknown labels, exact text, grid evidence,
   record identity and equality through construction/storage (Tasks 1, 6–8).
2. Profile/equipment selection, active values and unfinished contact corrections
   retain the agreed Apply/Cancel and source-clearing behavior (Tasks 1 and 3).
3. Pending/prepared/conflicting/deleted records and sealed witnesses still drive
   the same interruption, retirement, compaction and rollback decisions (2 and 8).
4. Key cutover/receipt compensation and partial maintenance remain atomic and
   report truthful progress after packing (Task 4).
5. Authorization-code replay, interrupted enrollment completion and independent
   account/client revocation retain their established outcomes (Tasks 3 and 5).

## Execution sequence and file map

Run **1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10**. Each task updates its actual
callers and leaves a compiling, usable workspace; no task ships a placeholder
that another task replaces. Tasks 4–9 revise fresh-schema SQL and checked queries
together. Preserve the existing migration filenames/numbers unless a file becomes
empty; do not retain obsolete tables or no-op conversion work to mimic history.

| Area | Owning files |
| --- | --- |
| Saved native values | `lib/clacks-model/src/{contact,scalar,location,profile,equipment,reference}.rs`; focused `contact/` children for checked input/Serde conversion |
| Native service evidence | `lib/clacks-model/src/service/{lotw,evidence}.rs`, `service/evidence/{download,observation,submission}.rs` |
| Sync envelopes/witnesses | `lib/clacks-sync/src/{record,client,compaction,account,membership}.rs` and `service/download.rs` |
| Local workflows | `lib/clacks-client-store/src/{operating,enrollment}.rs`, `operating/{values,selections}.rs`; desktop `operating/`, `session/`, `enrollment/`, actual entry/editor consumers |
| Server envelopes | `lib/clacks-server/src/secrets.rs`, `secrets/{keys,receipt,credentials,rows}.rs`, new focused `secrets/envelope.rs`; `maintenance/keys/`, `oauth/authorization.rs` |
| Server authorization | `lib/clacks-server/src/{enrollment,oauth}.rs`, `enrollment/{approval,recovery,membership}.rs`, `oauth/{rows,tokens,authorization}.rs`, `auth/{pending,changes}.rs` |
| Atomic client payloads | `lib/clacks-client-store/src/payload/{contact,shared,definitions,service}.rs`, `payload/definitions/{expression,plan}.rs`, `payload/service/{identity,projection,acquisition,assertion,observation}.rs` |
| Client causal metadata | `lib/clacks-client-store/src/metadata/{clock,lineage,account,client,review,transfer}.rs`, `metadata/client/`, `rows/{client,review,service,definitions,operating}.rs` |
| SQL and metadata | Existing migrations and fixed-query directories in both stores; scoped `.sqlx/` metadata and `deploy/server/database-grants.sql` |
| Behavioral checks | Existing `clacks-model`, `clacks-sync`, `clacks-client-store`, `clacks-lotw` integration modules; server store/key tests, web integration and desktop workflow tests |

## Task 1: Valid saved native records

**Deliverable:** Saved native values cannot be freely constructed or mutated into
states that violate their intrinsic requirements.

**Interfaces:** `Qso::try_from(QsoFields) -> anyhow::Result<Qso>` is a checked input
boundary; `QsoFields` is input/Serde data, never a saved `RecordValue`. Keep QSO
fields private with borrowing/copy accessors and an owned `into_fields` conversion
for complete edits. Use focused construction for invariant-bearing operator,
location, equipment, reference and named profile values. Station snapshots, own
operating context and current operating values retain ordinary fields when their
component types independently guarantee validity; do not duplicate those simple
aggregates with input wrappers. Retain existing native scalar constructors.

- [x] Extend existing model/entry tests for end-before-start rejection at
  construction, unknown mode/band preservation and an empty frequency/band input
  rejected through deserialization. Run the focused tests and observe the unmet
  checked boundary before implementation.
- [x] Put checked field/Serde conversion in `contact/input.rs`; preserve current
  serialized native fields. Reject invalid external/native input rather than
  silently repairing it. Entry/import preparation still performs the existing
  explicit normalization before construction.
- [x] Make `BandFrequency` construction/deserialization require frequency or band.
  Keep the current frequency-authority and band-only receive-override semantics;
  do not reclassify an already recorded pair against a recognition catalog.
- [x] Prevent empty present operator/location aggregates and blank optional text
  from entering saved values. Reuse one meaningful checked text value where
  helpful; preserve every character of nonblank recorded text. Do not require
  optional geography, operator identity or equipment descriptions.
- [x] Retain contextual own-station entry validation: four-character minimum for
  newly entered operating locations, without banning valid two-character
  historical/contacted-station locators in the shared saved snapshot type.
- [x] Preserve valid all-empty `EquipmentValues` for a named saved configuration;
  do not invent an equipment-description requirement. Preserve independently
  optional operator callsign/name and the two-character historical locator rule.
- [x] Update profile/current-context construction and all real model/sync/store/
  desktop/signing callers. Remove repeated intrinsic `validate()` obligations
  once their typed construction guarantees the invariant; retain reference and
  current-state checks. Do not add a generic builder/type-state framework.
- [x] Run model integration and relevant desktop manual-entry/operating tests,
  then `cargo check --workspace --all-targets --locked`. Update native-model.md
  and the model guide's old normalize-then-validate instructions. Commit the
  working construction-boundary change with a reviewed, physically wrapped body.

## Task 2: Checked synchronization and service evidence

**Deliverable:** Accepted contexts, witnesses and completed service values retain
their established internal relationships after construction and deserialization.

**Interfaces:** Retain `AcceptedRecord::new(key, state, stamp)` as the real accepted
envelope boundary. Add `VersionStamp::new(accepted_at, baseline, vector)` with
private fields. Introduce `EstablishedBasisRelation::{SameVersion, Before}`;
`seal_basis` remains the producer of sealed witnesses. `AcquisitionState` exposes
phase/finish/completion accessors derived from one state, not independent fields.

- [x] Extend the existing metadata/compaction/download scenarios to demonstrate
  rejected inconsistent restored acceptance context and an acquisition's actual
  active/completed/failed lifecycle. Exercise existing same-version/before sealed
  witnesses; do not add tests proving Rust exhaustiveness or field privacy.
- [x] Protect accepted stamp/envelope fields and coupled client pending/prepared
  state. Use existing transition functions to produce valid updates; decode raw
  fields through checked construction. Keep account/head/reference comparisons
  at their current transaction/application boundaries.
- [x] Narrow witness relation and retain its coupled original/observed evidence.
  Restore the witness through checked construction with its supplied lineage and
  baselines. Do not imply that a type proves ancestry without causal comparison.
- [x] Replace acquisition's phase plus optional finish time plus completion flag
  with `Active`, `Completed { at }`, `Failed { at, reason }`. Derive finish time
  and completion. Update `plan_start`/`plan_finish` without changing watermark or
  partial-input behavior. Preserve the current native serialized shape through
  an explicit private input/output representation.
- [x] Protect `LotwSubmission`'s frozen fields/key/signing identity. Native
  projection and storage restoration construct it through one internal
  consistency check. Move only self-consistency checking out of signing's input
  preparation; keep selected certificate, current eligibility, signature-definition
  compatibility and transport-format checks in clacks-lotw. No dependency cycle,
  re-normalization of old attempts or encoding modes in Rust variants.
- [x] Retain complete/incomplete provider evidence, optional issued references
  and `Reported<T>` as currently defined. For minute-covered observed timestamps,
  retain checked alignment/bounds after construction rather than accepting any
  `UtcSecond` as a minute. Do not alter QSO timestamp resolution.
- [x] Update owning sync/service docs. Run model, sync and LoTW integration suites
  and a workspace check. Commit the usable native evidence refactor.

## Task 3: Coherent operating and enrollment workflow state

**Deliverable:** Source selections, copied facts and resumable phases no longer
rely on unrelated writable options to describe one state.

**Interfaces:** Keep `operating::save` and enrollment attempt/install entry points.
`OperatingState` has checked construction and source/edit transitions with private
fields. `EnrollmentAttempt` retains a new/recovery mode and phase-specific data.
Server approval input stores `Option<RecoveryTarget { account, actor }>`.

- [x] Extend existing operating scenarios for choosing a profile, selecting
  separate equipment, manual adjustment and source deletion while keeping copied
  facts. Retain successful-save reset, explicit contact-specific corrections and
  all Save Profile/Apply/Cancel distinctions. No new GUI harness or layout tests.
- [x] Construct current values and source baselines together. Return named source
  references containing payload/record identity rather than mutating unrelated
  `OperatingStateRow` fields. Keep decoder rows at the SQL boundary.
- [x] Encode a bound enrollment phase with its required destination. Recovery
  carries its existing destination and prior binding as one mode; first
  enrollment retains optional destination while selecting a workspace and alone
  permits `NeedsDestination`. Preserve interruption boundaries and native-store
  side effects; typed state cannot assert those effects committed.
- [x] Update desktop completion/recovery and store persistence to use these
  operations. Keep enrollment attempt atomic in local-state storage, versioned
  explicitly. No legacy attempt converter is needed for the development reset.
- [x] Carry paired server recovery target and existing `ApprovalPurpose` through
  approval/activation; parse raw request/SQL fields once. Keep the current request
  correlation and library validation, with no new authorization machinery.
- [x] Carry already parsed identity IDs in pending identity changes as their
  existing typed IDs instead of discarding them into strings and reparsing.
- [x] Run client-store operating/enrollment tests, desktop workflow tests and web
  enrollment scenarios. Update operating/enrollment guides. Commit this group.

## Task 4: Atomic server envelopes and credential lifecycle

**Deliverable:** Credential, receipt and wrapped-key writes consume coherent
encrypted values; maintenance still operates on real committed references.

**Interfaces:** New crate-private storage types in `secrets/envelope.rs` represent
account ciphertext and wrapped-key envelopes. Credential reads return common
metadata plus `Present(material)`/`Removed`; metadata-only results carry a typed
present-format/removal status. Public provider capabilities remain downstream.

- [x] Extend existing credential/key-rotation scenarios to exercise replacement,
  removal, stale writes, receipt take/put-back across cutover and partial rewrap
  with packed values. Keep their actual authorization/read-back assertions and
  safe operator export checks; do not test AES primitives or dependency codecs.
- [x] Implement the three exact codec shapes above using existing dependencies.
  Exercise Clacks's framing contract with focused round-trip and rejection cases,
  including deployment key IDs containing delimiters/Unicode. Through production
  decrypt operations, verify that changing supported metadata or the expected
  owner/context cannot produce accepted plaintext. Do not duplicate AES tests.
- [x] Replace boolean-plus-options credential state and duplicated payload codec
  with typed variants. Check positive codec/key-version domains on construction;
  retain fixed-size secret key and nonce arrays after decoding. Keep arbitrary
  provider plaintext owned by its actual codec and zeroized as today.
- [x] Give account-rotation requests checked construction retaining explicit
  source/target versions and the existing positive adjacent-version rule. Keep
  the CLI's explicit retry arguments. Retain the existing 1–100 batch bound in
  a shared crate-private maintenance batch-size value; keep active-key,
  outstanding-reference, lock and resume checks against live state.
- [x] Revise server `0002_desktop_authorization.sql` and
  `0003_credential_protection.sql` directly. Replace split envelope columns with
  `wrapped_envelope`, `receipt` and `material` packed `TEXT` values as appropriate.
  Preserve account/key and credential identity/revision/timestamps. Remove the
  mirrored purpose/state/format checks; absence of material represents removal.
- [x] Derive key-reference projections from the codec's fixed reference field;
  retain account-qualified foreign keys and maintenance indexes through generated
  stored columns where required. No independently writable key-version shadow,
  field-by-field re-encryption updates or dual schema paths.
- [x] Update all key/credential/receipt SQL readers and writers, rotation counts,
  batch selection, status, replay compensation and expiry cleanup. Re-encryption
  replaces one envelope inside the existing account-locked transaction; counts
  still advance only after commit. Rewrap includes inactive account keys.
- [x] Preserve typed owner/code/purpose AAD bindings and authenticate the storage
  metadata with them. Keep purpose spelling and logical revisions unchanged;
  reject codec/decryption errors without another-key fallback. Extend rotation
  checks to preserve credential/receipt plaintext and material revisions, and to
  preserve account-key bytes and credential/receipt envelopes during deployment
  rewrap. Keep interruption/resume and completed-operation no-op assertions.
- [x] Use existing encryption, base64 and Serde facilities. Do not add SQLx JSON
  support or an encrypted-message dependency for envelope storage. Update
  column-level maintenance grants to permit whole-envelope replacement while
  preserving runtime/maintenance/migrator separation.
- [x] Regenerate/check server SQLx metadata against a fresh disposable database.
  Run all server store/key tests and web issuer scenarios. Update credential,
  server, deployment and backup-key docs. Commit the coherent server change.

## Task 5: Typed authorization and normalized grant metadata

**Deliverable:** Business logic receives typed access/refresh/grant state, with
one authoritative copy of immutable family metadata.

**Interfaces:** Decode grant status to provisional-with-deadline,
active-with-actor, or revoked-with-retained-origin. Reuse dependency-owned code,
token, scope and identifier types where they already express the required state.

- [x] Use existing issuance, replay, refresh, logout/revocation and independent
  account/client scenarios to pin current outcomes before changing the mapper.
  Preserve revoked provisional grants as well as revoked enrolled grants.
- [x] Separate access/refresh decoder projections so business logic does not
  distinguish them using manufactured NULL issuance/state fields. Convert raw
  code/grant discriminators to typed state before activation/eligibility decisions.
- [x] Revise migration `0002` and token queries so immutable client, proof-key and
  original authorization time come from the grant family. Keep account/family
  qualification and token-specific issuance/expiry/scopes/resources/state.
  Validate incoming dependency-issued family metadata against the resolved family
  at the adapter boundary; do not add another database equality check.
- [x] Keep public-client proof binding, persistent refresh credentials, opaque
  tokens, code replay containment, hashes, locks and current actor authorization
  unchanged. Do not replace oauth-as/Huskarl or add a separate auth service.
- [x] Update generated SQL metadata, role grants and account/enrollment contracts.
  Run server/web authorization scenarios and desktop enrollment tests. Commit.

## Task 6: Atomic client definitions and configuration

**Deliverable:** The Rust rule engine receives the same complete definitions
without reconstructing artificial expression-node and sequence tables.

**Interfaces:** Keep `compile_definition`, reference normalization and export
planning APIs. Definition mappers encode/decode private versioned storage
documents containing existing native identifier, catalog and rule values.

- [x] Extend existing definition/export scenarios to save/reopen a group with a
  capture/join expression, ordered normalization, catalog display/lookup and
  targeted partitions/expansions. Compare actual export/lookup preparation
  outcomes, including unknown associations and explicit-edit normalization.
- [x] Revise client migration `0003_shared_values.sql` in place: keep group
  identity/name/location-context facts; pack identifier configuration, optional
  catalog binding, ordinary bindings and targeted plan bodies. Keep each target's
  output-profile foreign key and owning payload relationship.
- [x] Pack output-profile context-field sets. Remove expression-node,
  normalization/display-detail and plan-field/partition/expansion table creation
  and their SQL. Do not retain separate expression documents referenced only from
  an already packed plan: embed that expression in its owning document.
- [x] Replace `payload/definitions/expression.rs` reconstruction and associated
  row DTOs/orphan traversal with ordinary serialization at the owner boundary.
  Keep the bounded Rust rule compiler, errors and data-driven definitions.
- [x] Give saved `GroupDefinition` checked construction/restoration and protected
  invariant-bearing contents as well. Raw definition input remains data-driven;
  the existing compiler validates regexes, bindings and plans before acceptance.
  Keep legitimate compilation/evaluation errors and account/profile-reference
  checks; do not hard-code program kinds, reinterpret historical associations or
  add a second compiled-definition persistence format.
- [x] Revise affected later migration references/retention queries, update scoped
  SQL metadata and run definition, client persistence and export suites. Commit
  with the matching current contract/guide updates.

## Task 7: Atomic service documents, field sets and exact keys

**Deliverable:** Stored evidence retains the same meaning and relationships
without splitting protocol documents or whole-value identities into row fragments.

**Interfaces:** Keep existing evidence/planning APIs. Private mappers encode
complete `AdifFields` maps and field sets. Exact-key interning accepts the native
`ExternalRecordKey`, using the canonical codec specified above.

- [x] Extend service persistence/recovery scenarios to save/reopen both provider
  keys, frozen field maps, selected/reported coverage and mixed immutable evidence.
  Verify real confident/ambiguous matching, service queries, recovered dependencies
  and at-most-one external association. Include distinct keys differing only in
  receive band or normalized mode, and QRZ decimal IDs beyond signed-64-bit range.
- [x] Revise client migration `0004_service_values.sql`: keep addressed evidence
  records, destination/attempt/member/association/dependency/retention edges and
  meaningful structured provider facts. Pack field maps and complete field sets;
  remove map-entry/coverage-entry tables used only to reconstruct those objects.
- [x] Replace exact provider-key component columns with one canonical key plus
  relational account/destination. Preserve uniqueness and existing external IDs
  as internal reference targets in the fresh schema; update lookup/index/binding
  queries to use that single key. Do not use a hash as the only equality proof.
- [x] Update projection, assertion, acquisition, observation and remote-identity
  mappers. Remove intermediate writable `ProjectionFields`, `ExternalKeyFields`
  and similar option bags; bind from existing variants or their closed encodings.
- [x] Keep the provider fact/report tables needed by effective facts and history
  queries relational. Preserve omission/clear/negative semantics, provenance and
  timestamp ordering; do not pack the whole service record or lose graph links.
- [x] Update metadata and run model service, sync service, client service/recovery
  and history-query scenarios. Update service/client-store guides and commit.

## Task 8: Atomic vectors and coverage with relational reference roots

**Deliverable:** Causal algorithms consume the same native maps, while storage
retains whole-object ownership and collection relationships.

**Interfaces:** Keep `insert_vector`/`load_vector` and existing coverage mapping
signatures. Their owner rows hold a private versioned JSON document containing
the existing native `RecordVector`/`Coverage` values.

- [x] Extend existing pending/prepared/transfer/compaction scenarios through
  reopen. Verify comparison results, preserved unsent work, sealed basis evidence,
  retired actor behavior and fuller rollback reconciliation, not JSON ordering.
- [x] Revise client migrations `0004`/`0005_sync_state.sql`: place the complete
  vector/coverage document on its qualified owner row; remove component-table
  definitions. Keep stamp, checkpoint, baseline, lineage-boundary, witness,
  membership and transfer-window references relational.
- [x] Update `metadata/clock.rs` and coverage/lineage/account/transfer mappers to
  serialize complete maps directly. Counter bounds and duplicate-meaning checks
  belong to the existing native constructors, not another SQL constraint catalog.
- [x] Retain normal SQL root/reference garbage collection. Do not add component
  indexes, lineage history, actor identities or a different synchronization
  algorithm. Packing a vector does not change the retention rule.
- [x] Remove component SQL and metadata; regenerate remaining metadata. Run sync
  suites and client preparation/transfer/service-recovery scenarios; commit with
  updated synchronization/client-store storage descriptions.

## Task 9: Complete constraint and write-shape cleanup

**Deliverable:** Remaining fresh-schema SQL expresses database responsibilities;
remaining write paths consume valid models without recreating invalid options.

- [x] Inspect every remaining `CHECK` in both stores against the database
  responsibilities above. Remove mirrored enum/range/text/variant checks from
  original declarations, including contact/shared/sync/review and server auth
  tables. Retain qualified family references, uniqueness and genuine physical
  type representation. Do not remove keys or indexes indiscriminately.
- [x] Finish observation/original-basis/version-condition/review mapper changes:
  replace writable decoder-shaped records with direct exhaustive variant binding
  or closed encoded variants containing their required references. Keep named
  raw rows solely for SQLx input decoding. No anonymous column tuples.
- [x] Audit repeated intrinsic validation across trusted paths and remove checks
  made unnecessary by Tasks 1–5. Retain raw decoding errors, external library
  conversion, current authorization, stale-save conditions, tag/source deletion
  checks and causal/reference comparisons. Do not replace removed validation
  with `debug_assert!` or unsafe assumptions.
- [x] Update existing behavior tests that manually assembled invalid saved values
  to exercise real input rejection where Clacks has a policy. Remove tests whose
  only assertion proves SQL constraints, Serde, SQLx, private fields or an enum
  match. Preserve valid lifecycle/matching/reconciliation outcomes.
- [x] Run the relevant existing suites and workspace Clippy. Update current docs
  that prescribe optional validation or SQL variant checks. Commit the completed
  constraint/model boundary cleanup; do not check in a review inventory.

## Task 10: Integrated verification and development handover

**Deliverable:** One coherent fresh-schema application, accurate contracts and a
portable development transition procedure, without a production upgrade framework.

- [ ] Create fresh disposable PostgreSQL and SQLite stores using the revised
  production migrations. Use documented roles/grants and current initialization
  paths, not hand-created substitute schemas or persistent development accounts.
- [ ] Run the existing serial workspace suite and focused real server command
  scenarios with opted-in export. Retain ciphertext/key exclusion and truthful
  committed progress tests without duplicating the suite per telemetry setting.
- [ ] Exercise native macOS enrollment, restart with pending completion, manual
  logging, profile/equipment adjustment, reopen, history/query and service evidence
  scenarios through their existing production paths. No new placeholder panes,
  manual interactive test harness or Windows-only dependency.
- [ ] Refresh SQLx offline metadata from the fresh schemas and check both stores.
  Remove metadata for deleted queries. Build release Leptos/server and desktop
  targets with the existing toolchain; run formatting, strict workspace Clippy,
  doctests and private-item rustdoc. Do not enable Windows CI or claim unavailable
  hosted/native acceptance; monthly CI quota limits remain as previously stated.
- [ ] Reconcile native-model, sync-model, client-store, service-evidence,
  operating-settings, enrollment and credential/server guides with delivered
  behavior. Preserve future deployed-schema migration policy, backup-key
  retention and the existing product roadmap. Remove superseded completed plans
  only through the normal post-merge cleanup workflow.
- [ ] Document portable development reset/recreation: stop consumers, identify
  which development data is disposable, recreate the selected databases/workspaces
  with current SQLx migrations/grants, sign in/re-enroll normally as needed, then
  restart one server. Never automatically delete native entries, shared secret
  files, provider registration or Docker volumes. Keep machine-specific inventory
  and commands outside the repository. Do not edit SQLx migration history to
  make an old database appear compatible.
- [ ] Verify the plan's preserved-behavior list against final code and the diff.
  Remove unused transitional types/helpers/SQL and artificial test scaffolding.
  Commit remaining owning documentation with substantive, hard-wrapped messages.
  Report exact checks and limitations; do not push/open another PR without the
  user's instruction.

## Verification commands

Use the existing documented disposable database and scoped SQL metadata procedure
in each crate guide. No developer-specific secrets, provider URLs or paths belong
in this plan. In the authorized execution environment, set `SQLX_OFFLINE=true`,
the existing disposable PostgreSQL test URLs and `LEPTOS_OUTPUT_NAME=clacks-web`.

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo test -p clacks-model -p clacks-sync -p clacks-client-store -p clacks-lotw --locked
cargo test -p clacks-server --lib --locked -- --test-threads=1
cargo test --workspace --all-targets --locked -- --test-threads=1
cargo test --workspace --doc --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --document-private-items --locked
cargo leptos build --release
cargo build -p clacks-log -p clacks-admin -p clacks-server-app --release --locked
git diff --check
```

Run PostgreSQL metadata preparation/check sequentially from `lib/clacks-server`
with `SQLX_OFFLINE=false` and the disposable migrator URL as `DATABASE_URL`:

```sh
cargo sqlx prepare -- --all-targets --locked --offline
cargo sqlx prepare --check -- --all-targets --locked --offline
```

For SQLite, select one fresh temporary SQLite URL as `DATABASE_URL` and use that
same URL throughout, from the workspace root with `SQLX_OFFLINE=false`:

```sh
cargo sqlx database create
cargo sqlx migrate run --source lib/clacks-client-store/migrations
cargo sqlx prepare --workspace -- --package clacks-client-store --all-targets --locked --offline
cargo sqlx prepare --check --workspace -- --package clacks-client-store --all-targets --locked --offline
```

Use the existing role/bootstrap/grant procedure for the fresh PostgreSQL store.
Do not invent a second metadata database for it. SQLx shares preparation scratch
state, so do not run the two metadata jobs concurrently. Delete only the temporary
metadata/test stores created by this execution.

Run focused tests first, observe the intended unmet contract, implement the
owning change, then run the relevant group before committing. Do not repeat broad
checks without a new change or unresolved failure. Preserve the existing native
backend and offline-source test boundaries when reporting ignored entries.

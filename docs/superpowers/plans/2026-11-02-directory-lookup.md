# Directory Lookup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver issue #17's real web setup → encrypted persistence and native provisioning → direct desktop lookup → synchronized contact snapshots workflow.

**Architecture:** Extend the existing concrete server, desktop, model and storage modules. Keep credentials outside replicated records and SQLite payloads; keep directory snapshots outside recorded QSO facts. Use one directory record per QSO with one latest-successful winner per normalized key, carried through existing account synchronization with its directory-specific value policy.

**Tech Stack:** Rust 1.99.1, Serde, SQLx 0.9.0, PostgreSQL, SQLite, Leptos/Axum, Tokio, egui/eframe, reqwest 0.12.28, roxmltree 0.21.1, existing authenticated encryption and native credential backends.

**Spec:** [Directory configuration, provisioning and desktop lookup](../specs/2026-11-02-directory-lookup-design.md). Read it and applicable `AGENTS.md` before executing. This plan authorizes no execution, migration or deployment by itself.

Application programming interface is abbreviated API, Hypertext Transfer
Protocol HTTP, and Transport Layer Security TLS. XML means Extensible Markup
Language; JSON means JavaScript Object Notation. IOTA means Islands on the Air,
AAD additional authenticated data, and DTD document type definition. ID types
use the existing Universally Unique Lexicographically Sortable Identifier
(ULID) convention. One mebibyte is abbreviated MiB.

## Global Constraints

- Preserve Rust 1.99.1 and the existing workspace/library/application layout.
- Query only the selected provider; no fallback, provider-field merging, reusable persistent result cache or automatic bulk catch-up.
- Use 500 ms debounce after the last effective callsign change; save without waiting for lookup.
- Keep recorded facts and directory facts separate; apply existing recorded → eligible LoTW → eligible QRZ → directory precedence and whole-location selection.
- Preserve recorded callsigns; normalize lookup keys independently. Remove terminal `/P`; preserve other slash forms as specified.
- Preserve lookup completion time and lookup identity through retries/recovery; never use server arrival time to arbitrate snapshots.
- Account/deployment keys, native secret storage, existing authenticated connections and ordinary database transactions remain selected.
- No new repository/provider traits, plugin registry, generic secret API, job/cleanup queues, persistent connection epochs or test-only production interfaces.
- Keep current enrollment namespaces, bundles and references intact. Obsolete-secret cleanup cannot block later configuration installation.
- Fixed queries stay in SQL files with checked metadata. Use production migrations/grants and disposable test databases.
- Bulk lookup (#22), WS Suite (#21), publication (#31/#30) and Linux native-secret support are outside this delivery.
- Instrument meaningful production work in its owning task; keep credentials, raw provider payloads and secret frames out of diagnostics.
- Verify assembled production UI against `docs/desktop-ui.md`; state actual platform limits, including unavailable Windows acceptance.
- Commit messages have a substantive body hard-wrapped at 72 columns. Inspect the actual file and use `git commit --file` for every commit/amend.

## Review Focus

- A newer connection/configuration arriving during native access, including reused/lower restored revisions, must not be overwritten by an obsolete delivery; Task 3 tests this.
- Rejection of a lookup using old credentials must not pause newly installed credentials; Tasks 4 and 6 test this.
- Successful sparse results clear omitted old fields, while failure/not-found preserve the eligible winner; Tasks 1 and 5 test this.
- Receipt of an earlier frozen proposal must preserve a later local directory refresh while unrelated QSO edits retain ordinary conflict rules; Task 5 tests this.
- IOTA-only location evidence, partial recorded locations and absent/edited group mappings must obey whole-source selection without forced output or definition repair; Task 1 tests this.

These are verification cases from the specification, not additional product policies.

---

## Execution preparation and commit procedure

Planning made no implementation changes. Before execution, inspect current Git
state and preserve unrelated changes; the planning baseline includes an unrelated
edit to `references/desktop-toolkit.md`. Use the required worktree-placement and
worktree skills if isolation is needed. Carry this plan/spec into the chosen
execution checkout deliberately; they may still be uncommitted. Do not modify
or stage unrelated work.

For database work, use the existing disposable-database procedure in
`lib/clacks-server/README.md`, with `CLACKS_TEST_DATABASE_URL` and
`CLACKS_TEST_RUNTIME_DATABASE_URL` set to the appropriate disposable roles.
Use production migration/grant scripts, including maintenance grants. Follow
applicable machine-local server instructions before starting a server; do not
put machine-specific provider registrations, paths or secrets in the repository.
Set `SQLX_OFFLINE=true` for ordinary builds/tests; metadata commands explicitly
override it against their disposable backend.

Each task follows red → green for its Clacks behavior. Existing dependencies'
documented behavior is not a reason for another test. Tests below are assertion
contracts: implement them using the existing suite's native builders and real
operations; illustrative local variable names are not new production APIs.
New helpers belong in that suite's test support, not production visibility.

At a task's commit step, stage its explicit files, inspect the staged diff and
prepare a message file explaining motivation/tradeoffs. Inspect that file and
physical body line lengths with `awk 'length($0) > 72 {print NR, length($0), $0}'`;
review any literal identifiers/URLs that legitimately exceed the limit. Then
use `git commit --file /absolute/path/to/message-file`. Do not use `-m` or
introduce hooks. Do not commit a noncompiling intermediate slice or hide
temporarily unused implementation behind allowances; combine dependent work
in the working set until it has its planned production caller.

## File and dependency map

All paths below are relative to the repository root. New module files use named
Rust modules, not `mod.rs`. Parent registration changes accompany each new file.

| Task | Main files and responsibility |
| --- | --- |
| 1 | `lib/clacks-model/src/directory.rs`, `directory/values.rs`, `directory/record.rs`: checked facts/key/winners; `effective.rs`, `export/{rules,select,evaluate}.rs`, `service/{qrz,applicability}.rs`: actual source/output consumers. |
| 2 | Server `directory.rs`, `store/directory.rs`, `secrets/directory.rs`, existing key maintenance; web `actions/directory.rs`, `pages/directory.rs`: production account setup. |
| 3 | Sync `protocol/directory.rs`; desktop `directory/{credentials,provisioning}.rs`; client-store `directory/installation.rs`: protected selected-provider installation over existing socket. |
| 4 | Desktop `directory/providers/{qrz,hamqth}.rs`, `directory/providers.rs`: real HTTP mapping/session behavior, used by Task 6's runtime. |
| 5 | Sync `directory.rs`; both stores' directory payload/row/query families; client-store `directory/snapshots.rs`, `save/contacts.rs`: durable snapshots, source reads and all sync paths. |
| 6 | Desktop `directory/lookup.rs`, existing session/logging/editor UI: effective-call trigger, request/save ownership, visible fallback and diagnostics. |
| 7 | Existing integrated server/desktop suites, crate guides and issue delivery evidence: complete workflow, recovery, native and visual acceptance. |

Tasks 2 and 3 also add shared concrete directory provisioning values in
`clacks-sync::protocol::directory`; Task 2 needs their typed material for real
credential persistence, Task 3 adds message dispatch. This is one concrete
consumer family, not a general secret-distribution layer.

Execute 1 → 2 → 3 → 4 → 5 → 6 → 7. This is one capability, not seven independently
shippable services. No task alone establishes completion of #17.

### Task 1: Native directory values and effective-source projection

**Files**

- Create `lib/clacks-model/src/directory.rs`, `directory/values.rs`, `directory/record.rs`, `tests/directory.rs`.
- Modify `lib/clacks-model/src/{lib,ids,effective}.rs`, `src/export/{rules,select,evaluate}.rs`, `src/service/{qrz,applicability}.rs`.
- Modify `lib/clacks-model/tests/{integration,effective_values,reference_export,service_qrz}.rs` and affected existing callers of changed effective/export signatures.
- Mechanically adapt current callers in `lib/clacks-model/src/{catalog,lib}.rs`, `src/service/{evidence,lotw}.rs`, the existing model export/service tests, client-store `src/payload/service/assertion.rs`, `src/read/projection.rs`, `src/read/projection/station.rs`, `src/read/query/derived.rs`, and desktop `src/ui/contact_table.rs`. This task keeps existing behavior compiling with `LocationFacts` and borrowed export input; Task 5 later supplies real directory data to those consumers.

**Interfaces**

- Add `LookupId` and `DirectoryMaterialId` through the existing entity-ID macro.
- In `directory::values`, define `DirectoryProvider::{Qrz,HamQth}`, checked `LookupKey`, `DirectoryFacts`, `DirectorySnapshotFields` and checked `DirectorySnapshot`. Field contents are spec §3; include `supplied_coordinates: Option<Coordinates>` as well as supplied locator/source metadata.
- `LookupKey::from_recorded(call: &RecordedCallsign) -> Self`; `LookupKey::as_str(&self) -> &str`. Preserve unsupported/incomplete forms when stripping would leave no key; restoration validates stored text without repeating the request transformation.
- `DirectorySnapshot::new(fields: DirectorySnapshotFields) -> anyhow::Result<Self>`; readonly accessors expose fields. `DirectoryFacts` contains existing checked component types; no duplicate scalar validation layer.
- `DirectoryRecord::new(qso: QsoId) -> Self`; `record(&mut self, snapshot: DirectorySnapshot) -> anyhow::Result<bool>`; `merge_from(&mut self, other: &Self) -> anyhow::Result<bool>`; `snapshot(&self, key: &LookupKey) -> Option<&DirectorySnapshot>`; `qso(&self) -> QsoId`. `true` means winner values changed.
- Add nonsecret `DirectoryConfiguration { revision: i64, selected: Option<DirectoryProvider>, configured: BTreeSet<DirectoryProvider> }`, validating the spec's counter domain on restoration.
- Extend `StationFacts` with separate provider references. Add `LocationFacts { location: Option<RecordedLocation>, references: BTreeSet<ReferenceAssociation> }` and change `select_location`'s payload to `Option<Sourced<LocationFacts>>`. This is the selected whole location, not a second priority policy.
- `effective_contacted_references(recorded: &StationSnapshot, selected: Option<&Sourced<LocationFacts>>, definitions: &DefinitionSet) -> BTreeSet<ReferenceAssociation>` keeps recorded non-location associations and selects location-bearing associations from the same source as geography.
- In `export::evaluate`, add borrowed `ReferenceExportInput<'a> { qso: &'a Qso, contacted_references: &'a BTreeSet<ReferenceAssociation> }`. Change `plan_reference_export`'s first argument from `&[Qso]` to `&[ReferenceExportInput<'_>]`; other arguments/return type remain. Change `select_mappings` and its compiled helper to consume the same input. Extend `ExpressionContext` with an optional borrowed contacted-reference override; recorded `QsoFieldPath` meaning stays recorded.

- [ ] **Step 1: Add focused failing assertions.** Register `tests/directory.rs` in `tests/integration.rs`. Cover normalization/restore, whole-winner replacement and deterministic ties, using fixed native timestamps/IDs. Extend the existing effective/reference tests for partial location, IOTA-only source and edited/missing mappings.

  ```rust
  assert_eq!(LookupKey::from_recorded(&RecordedCallsign::new(" OK2CQR/p ")?).as_str(), "OK2CQR");
  assert_eq!(LookupKey::from_recorded(&RecordedCallsign::new("EA8/OK2CQR")?).as_str(), "EA8/OK2CQR");
  assert_eq!(serde_json::from_str::<LookupKey>("\"EA8/OK2CQR/P\"")?.as_str(), "EA8/OK2CQR/P");
  assert_eq!(left_then_right, right_then_left);
  assert_eq!(winning_snapshot.facts().name, None); // newer success omitted the old name
  assert_eq!(qso_after_projection, qso_before_projection);
  assert!(!unmapped_output.contains_key("IOTA"));
  assert_eq!(mapped_output.get("IOTA").map(String::as_str), Some("EU-005"));
  ```

  Name the scenarios `directory_key_and_restoration`, `directory_winner_is_whole_and_order_independent`, and `directory_references_follow_whole_source_and_account_mappings`. Include equal lookup ID/different payload rejection and preservation of distinct keys in the winner scenario, not a second generic round-trip suite.
- [ ] **Step 2: Run** `cargo +1.99.1 test -p clacks-model --test integration --locked --offline directory`. Expect the new assertions/APIs to fail before implementation; record the failure.
- [ ] **Step 3: Implement** values and the exact timestamp/lookup-ID ordering. Reuse `UtcTimestamp`, existing ID generation/Serde, checked location/position values and `BTreeMap`. Keep one winner per key and reject differing content for the same lookup identity. Implement shared source/reference selection and change existing projection callers, including eligible confirmation references.
- [ ] **Step 4: Verify** `cargo +1.99.1 test -p clacks-model --test integration --locked --offline` and `cargo +1.99.1 check --workspace --all-targets --locked --offline`; expect all cases/checks to pass. The missing/edited mapping cases must show no forced IOTA emission or account-definition rewrite. Trace winner/source decisions through existing debug-level producers without personal payloads or a metric per selector.
- [ ] **Step 5: Commit** with subject `Model retained directory facts and whole-source fallback`, using the commit procedure.

### Task 2: Web setup, typed encrypted material and key maintenance

**Files**

- Create `lib/clacks-sync/src/protocol/directory.rs`; modify `protocol.rs`, `lib/clacks-sync/Cargo.toml` for the existing `zeroize` dependency.
- Create `lib/clacks-server/src/directory.rs`, `store/directory.rs`, `secrets/directory.rs`, `store/rows/directory.rs`, `store/tests/directory.rs`.
- Modify server `src/{lib,store,secrets}.rs`, `secrets/{envelope,rows}.rs`, `maintenance/keys/{references,migration,account,report}.rs` and relevant tests.
- Create `lib/clacks-server/migrations/0010_directory_configuration.sql`; create `sql/directory/{configuration,material,upsert_configuration,upsert_material,remove_material}.sql`, `sql/secrets/rotation/{directory_material,directory_update}.sql`; extend existing count/status queries.
- Modify `deploy/server/database-grants.sql`, which owns both runtime and maintenance grants. Maintenance gets public configuration/reference reads and encrypted-envelope replacement, not provider-login writes or configuration revision changes. Update `lib/clacks-server/.sqlx/` through SQLx.
- Create web `src/actions/directory.rs`, `src/pages/directory.rs`, `tests/directory.rs`; modify `src/{actions,pages,lib}.rs`, `tests/integration.rs`, and `Cargo.toml` to consume shared nonsecret model values.

**Interfaces**

- In `protocol::directory`, add `DirectoryLogin { username: String, password: String }` with zeroizing drop, Clone/Eq/Serde and redacted diagnostic handling; `DirectoryMaterial { id: DirectoryMaterialId, revision: i64, login: DirectoryLogin }`. Implement redacted `Debug` for these concrete types so existing protocol diagnostics can retain public structure without formatting secret fields. No secret field may appear in output.
- In server `directory`, add `CredentialChange::{Keep, Replace { provider, login }, Remove { provider }}`; `DirectoryUpdate { expected_revision: i64, selected: Option<DirectoryProvider>, credential: CredentialChange }`; `DirectoryUpdateOutcome::{Committed(DirectoryConfiguration), Unchanged(DirectoryConfiguration), Stale(DirectoryConfiguration)}`.
- `pub async fn load(state: &ServerState, principal: &Principal) -> anyhow::Result<DirectoryConfiguration>` and `pub async fn update(state: &ServerState, principal: &Principal, change: DirectoryUpdate) -> anyhow::Result<DirectoryUpdateOutcome>` are the browser-authorized production APIs.
- `secrets::directory::seal(key: &AccountKey, context: &DirectoryContext, material: &DirectoryMaterial) -> anyhow::Result<DirectoryEnvelope>` and `open(key: &AccountKey, context: &DirectoryContext, envelope: &DirectoryEnvelope) -> anyhow::Result<DirectoryMaterial>` mirror receipt ownership using named `DirectoryContext { account, provider, material_id, material_revision }`. Use the spec's packed format and independently expected AAD; do not generalize receipt encryption into a plugin framework.
- Browser server functions `load_directory() -> Result<DirectoryConfiguration, ServerFnError>` and `save_directory(expected_revision: i64, selected: String, operation: String, provider: String, username: String, password: String) -> Result<DirectorySaveOutcome, ServerFnError>` convert posted fields into the typed update in the server body. Define web `DirectorySaveOutcome::{Committed(DirectoryConfiguration), Unchanged(DirectoryConfiguration), Stale(DirectoryConfiguration)}` with Serde and explicitly map the server's outcome to it. Shared model configuration is a normal web dependency; server mutation/material APIs remain server-rendering-only.
- Mount `DirectorySettings` at `/account/directory` using the existing navigation and action forms. Empty password means retain existing material for ordinary selection saves; explicit Replace validates required credential input, explicit Remove deletes it. Do not trim/alter password contents or require a live login to save.

Schema decisions for migration 0010: `clacks.directory_configuration` has
`account_id` as its account-qualified primary key, `revision BIGINT` and nullable
`selected_provider TEXT`. `clacks.directory_material` has primary key
`(account_id, provider)`, material ID/revision, and one `encrypted_envelope TEXT`.
Generate its account-key-version projection from the envelope, with an
account-qualified foreign key to `account_encryption_keys` and an index on
`(account_id, account_key_version)`, following current receipt reference design.
Do not duplicate closed provider/codec validation in SQL or add a writable
shadow key-version column. Plaintext login/password and transient sessions are
not columns. Bind material identity/revision/provider as AAD as well as owner.

- [ ] **Step 1: Add failing real-operation scenarios** `directory_configuration_is_account_scoped_and_stale_safe` and `directory_web_setup_never_returns_saved_password`. Use two actual test accounts, expected revision zero and a unique password sentinel. Assert account B sees no A configuration; stale revision zero cannot overwrite revision one; selection-only/removal no-ops leave revision unchanged; explicit replacement advances material revision and explicit removal preserves selection/unconfigured state. Exercise the browser action under existing same-origin/session rules, not a hand-built privileged store.
- [ ] **Step 2: Run** `cargo +1.99.1 test -p clacks-server --lib --locked --offline directory` and `cargo +1.99.1 test -p clacks-web --features ssr --test integration --locked --offline directory`; expect missing functionality/assertion failures. Use disposable database variables for both commands.
- [ ] **Step 3: Implement** production schema, concrete typed operations and web forms. In short account-locked transactions, compare the displayed revision, encrypt Replace under the active key, mutate intended configuration/material and advance semantic revision once. Selection-only/removal/read must not create a key just to operate. Remove/recreate uses a fresh material identity; encryption maintenance retains identities/revisions. Publish account notification only after commit.
- [ ] **Step 4: Extend existing maintenance scenarios** with material created through `directory::update`: rotate across receipts plus directory values with one total batch bound; count both families; retry/no-op accurately; rewrap active/inactive account keys and then provision/decrypt the same material. Add known AAD mismatch assertions to the concrete codec scenario, not tests of AES internals. Extend significant committed events and existing re-encryption counts here.
- [ ] **Step 5: Regenerate server query metadata** using the metadata procedure below, then run both Step 2 commands plus `cargo +1.99.1 test -p clacks-server --lib --locked --offline key_rotation`. Expect passing scenarios, unchanged semantic versions across maintenance and no password in browser responses/selected logs.
- [ ] **Step 6: Commit** `Configure and protect account directory credentials`, using the commit procedure. Include schema, grants, query metadata and actual web consumers together.

### Task 3: Provision the selected configuration into separate native entries

**Files**

- Modify `lib/clacks-sync/src/protocol/{directory,messages}.rs` and protocol callers affected by redacted formatting.
- Modify server `src/directory.rs`, `src/synchronization/connection.rs`, `src/store/tests/directory.rs`.
- Create client-store `src/directory.rs`, `src/directory/installation.rs`, `tests/directory_installation.rs`; reuse the existing `local_state` queries and register modules. No installation table, migration or new query metadata is needed.
- Create desktop `src/directory.rs`, `src/directory/{credentials,provisioning}.rs`, `src/directory/provisioning/tests.rs`; modify `src/{lib,session,synchronization}.rs`, `src/synchronization/exchange.rs` and `src/enrollment/tests/synchronization.rs`.

**Interfaces**

- Add `ProvisionedConfiguration { account: AccountId, configuration: DirectoryConfiguration, material: Option<DirectoryMaterial> }` in `protocol::directory`. Validate selection/material consistency; None explicitly represents disabled or selected-but-unconfigured state.
- Add `ClientMessage::DirectoryConfiguration` and `ServerMessage::DirectoryConfiguration { configuration: ProvisionedConfiguration }`. Reuse ordered socket request/response handling; no wire-version bump for hypothetical released clients.
- `pub(crate) async fn for_client(state: &ServerState, client: &AuthorizedClient) -> anyhow::Result<ProvisionedConfiguration>` reads a consistent config/material snapshot and checks current native authority before decrypting.
- Client-store `DirectoryInstallation { configuration: DirectoryConfiguration, credential_reference: Option<String> }`; `installation::load(store: &ClientStore) -> anyhow::Result<Option<DirectoryInstallation>>`; `installation::commit(store: &ClientStore, expected: Option<&DirectoryInstallation>, next: &DirectoryInstallation) -> anyhow::Result<bool>`. `false` is a changed local basis, not an instruction to overwrite it; only this owner writes the `directory.installation` local-state value.
- Desktop `credentials::save(binding: &EnrollmentBinding, configuration: &DirectoryConfiguration, material: &DirectoryMaterial) -> anyhow::Result<String>`, `load(binding: &EnrollmentBinding, installation: &DirectoryInstallation) -> anyhow::Result<DirectoryMaterial>`, and `remove(reference: &str) -> anyhow::Result<()>` use a separate directory namespace and bundles with deployment origin/account/purpose/provider/material context. No bundle-supplied origin chooses a server. Save returns a fresh opaque reference; load returns typed material only after context validation.
- Desktop `provisioning::Delivery { configuration: ProvisionedConfiguration, connection_stop: CancellationToken }` identifies the live connection using existing cancellation ownership. Async `provisioning::run(store: ClientStore, binding: EnrollmentBinding, incoming: watch::Receiver<Option<Delivery>>, installed: watch::Sender<Option<DirectoryInstallation>>, status: watch::Sender<ProvisioningStatus>, stop: CancellationToken) -> ()` serializes installations. Define `ProvisioningStatus::{Idle, Failed(String)}` as sanitized directory diagnostics; UI availability follows the installed configuration, not an invented aggregate sync status.

Use one versioned `directory.installation` value in the existing `local_state`
store: public configuration and optional opaque native credential reference.
Validate decoding and compare the typed expected value inside the existing
write transaction before replacement. Workspace ownership already supplies its
single account; no relational identity, SQL filtering or joins require a new
table. This is local workflow metadata like enrollment completion, not serialized
synchronized domain state. Malformed/unsupported metadata is reported and left
untouched on read, but permits replacement by fresh authenticated provisioning.
Database read failures remain errors; valid installations retain the ordinary
expected-value guard. No secret bytes or new server-authority clock are stored.

- [ ] **Step 1: Add failing scenarios** `directory_provisioning_replaces_removes_and_reconnects` in the real enrolled-desktop fixture and `directory_installation_does_not_use_revision_as_freshness` in provisioning tests. Include secret payload changes with identical public revision/identity and a restored lower revision. Assert actual loaded native material matches the server, SQLite retains only references, enrollment still authorizes sync and disabled/unconfigured responses stop new credential acquisition.
- [ ] **Step 2: Add ownership assertions** using ordinary ordered channels and cancellation: an ended connection's queued delivery cannot install; a newer queued configuration wins after admitted older work drains; socket receipt/ack processing does not await native installation. Use the actual unsupported-platform/native environmental failure where reproducible to establish prior reference retention and unrelated QSO progress. Do not add a fake secret-store trait, test failpoint or a new persistent epoch to manufacture these failures.
- [ ] **Step 3: Run** `cargo +1.99.1 test -p clacks-client-store --test integration --locked --offline directory_installation` and `cargo +1.99.1 test -p clacks-desktop --lib --locked --offline directory`; expect the missing workflow/assertions to fail. Register client-store tests in its existing integration target.
- [ ] **Step 4: Implement** current-authority reads and dedicated wire messages. Request on each authenticated reconnect and account notification regardless of record checkpoint. Handle the message in every actual socket response path, including responses interleaved with transfer; deliver into the installation channel, never into history/staging. Compare full typed current material after restore, not just public counters.
- [ ] **Step 5: Implement** fresh native write → guarded SQLite reference commit → best-effort obsolete-entry cleanup. Before starting commit, discard known obsolete/cancelled deliveries; admitted transactions can drain normally. Removal commits unusable/disabled state before cleanup. Report cleanup failure without rollback or admission gates. Use `spawn_blocking`; retain previous usable config on failed new write/commit and isolate its diagnostics from QSO sync.
- [ ] **Step 6: Verify** Step 3 commands, server directory tests, and the existing native enrolled workflow on an available noninteractive backend. Capture the `clacks_directory_configuration_installs_total` count at actual SQLite commit, safe failure events and an unchanged enrollment namespace/reference. State unsupported/native acceptance limits explicitly; do not use interactive ignored tests.
- [ ] **Step 7: Commit** `Provision directory configuration without changing enrollment`, using the commit procedure.

### Task 4: Concrete QRZ/HamQTH adapters and provider sessions

**Files**

- Create `lib/clacks-desktop/src/directory/providers.rs`, `directory/providers/{qrz,hamqth}.rs`, `directory/providers/tests.rs`, and authored fixtures under `directory/providers/fixtures/`.
- Modify `lib/clacks-desktop/Cargo.toml`: promote existing workspace reqwest 0.12 to a production dependency with JSON support; add existing `roxmltree`. Preserve reqwest-oauth's distinct existing use; do not migrate enrollment HTTP incidentally.

**Interfaces**

- `ProviderLookup::{Found(DirectoryFacts), NotFound}`; `ProviderError::{Authentication, RateLimited { retry_after: Option<Duration> }, Unavailable { reason: SafeProviderFailure, source: anyhow::Error }}`. `SafeProviderFailure` is a bounded enum for transport/parse/subscription/refusal/invalid-response categories. Retain internal sources but format only safe categories at handling/UI boundaries, never arbitrary provider text or complete error chains.
- `QrzSession::new(client: reqwest::Client, endpoint: Url, material: DirectoryMaterial) -> Self` and equivalent `HamQthSession`; `async fn lookup(&mut self, key: &LookupKey) -> Result<ProviderLookup, ProviderError>`. These are concrete adapter construction inputs. Production composition supplies fixed provider endpoints; tests can supply a local fixture URL to those same constructors without new end-user endpoint settings or a hidden test mode.
- `ProviderSession::{Qrz(QrzSession), HamQth(HamQthSession)}` dispatches only its selected concrete adapter. It has the same `lookup` signature; no provider trait/object registry. The owning task keeps in-memory session state; it does not persist tokens/results.

- [ ] **Step 1: Add failing provider scenarios** `directory_qrz_maps_sparse_results_and_renews_session` and `directory_hamqth_distinguishes_expiry_rejection_and_rate_limit`, using local HTTP response fixtures and real clients. Assert actual request order/count, encoded callsign, QRZ POST credential body and no password URL, reused returned session key, one renewal/repeat for expiry, no renewed-login loop after rejection, and `Retry-After` preservation.
- [ ] **Step 2: Pin mapping assertions** from spec §3: QRZ `fname=Fred`, `name=Lloyd`, `nickname=The Boss` selects `The Boss`; `dxcc=291` with postal `ccode=1` retains `291`; HamQTH station `dxcc=503` versus `address.dxcc=291` retains `503`. QRZ `geoloc=grid` remains `Position::GridArea`; HamQTH valid six-character grid takes precedence for its projected position while supplied coordinates remain metadata. Invalid zone 0 is absent; IOTA maps to the stable seeded group without writing QSO associations; optional privacy omissions remain absent. Use small independently authored fixtures, not real personal responses.
- [ ] **Step 3: Run** `cargo +1.99.1 test -p clacks-desktop --lib --locked --offline directory::providers`; expect missing adapters/assertions to fail.
- [ ] **Step 4: Implement** concrete parsing/mapping and standard client configuration: 5-second connect, 15-second total timeout, 1 MiB bounded body, no credential forwarding on redirects, existing TLS, normal form/query encoding and roxmltree's DTD-disabled parsing. HamQTH uses v2 JSON login and bearer-token reuse. Retry only expired-session login/lookup once; surface rejection/not-found/refusal/subscription/transport correctly. Remove URLs from retained reqwest errors before deliberate safe reporting.
- [ ] **Step 5: Verify** Step 3 command passes. Existing provider sources are linked from the spec; recheck them if implementation finds differing behavior. Never treat response fixtures as authenticated interoperability evidence. Keep adapter spans secret-safe; logical lookup duration/counts belong to Task 6's runtime, actual dispatch count belongs here.
- [ ] **Step 6: Commit** `Implement direct QRZ and HamQTH lookup adapters`, using the commit procedure when Task 6's real caller is available; no unused-code allowance or dummy consumer.

### Task 5: Durable snapshots through ordinary synchronization and recovery

**Files**

- Create `lib/clacks-sync/src/directory.rs`, `tests/directory.rs`; modify `src/{lib,record,client,acceptance,recovery}.rs`, `src/client/{preparation,reconciliation,returned,state}.rs`, applicable dependency/transfer code, `tests/integration.rs` and existing recovery/compaction cases.
- In both client-store `src/` and server `src/store/`, create `payload/directory.rs`, `rows/directory.rs`; modify payload identity/discriminator/root-count integration and metadata/transfer/recovery consumers.
- Create client-store `src/directory/snapshots.rs`, `src/save/contacts.rs`, `tests/directory_snapshots.rs`, `migrations/0013_directory_snapshots.sql`, `sql/directory/{snapshot_insert,snapshot_select,reference_insert,reference_select}.sql`; modify `src/{save,synchronization,transfer,retention}.rs`, `src/discovery/support.rs`, `src/read/projection.rs`, `src/read/projection/{station,evidence}.rs` and `src/read/projection/evidence/status.rs`.
- Create server `migrations/0011_directory_snapshots.sql`, matching queries under `sql/persistence/directory/`, and `src/store/tests/synchronization/directory.rs`; modify `src/store/{synchronization,retention}.rs`, `src/store/synchronization/{context,receipt,recovery}.rs`, discovery/dependency reads and production grants.
- Modify each store's payload `root_count.sql` and owned `.sqlx/` metadata, client test registration and server `store/tests/synchronization.rs` registration.
- Modify client-store `src/read/query/derived.rs` and the existing effective-source query prerequisites, plus `tests/history_queries.rs`: its current provider-evidence optimization must include directory-only contacts, not just confirmation evidence.

**Interfaces**

- Add `RecordId::Directory(QsoId)` and `RecordValue::Directory(Box<DirectoryRecord>)`; native identity validation checks QSO ID, map/snapshot key agreement and checked payload values.
- Sync `directory::combine(left: &DirectoryRecord, right: &DirectoryRecord) -> anyhow::Result<DirectoryRecord>` applies Task 1's winner operation. Add direct directory branches to existing state receipt/preparation/recovery; no generic value-policy trait. Preserve real causal context while automatically combining directory values, including causal fast paths.
- Add `RejectionReason::DeletedDirectoryParent { qso: QsoId }` in `clacks-sync/src/acceptance.rs`; the client consumes it as obsolete lookup work, not manual provider-fact conflict. Use an ordinary missing-reference outcome/staging for genuinely unavailable parent history.
- Client-store `SnapshotOutcome::{Applied, Unchanged, Ineligible}` and async `snapshots::apply(store: &ClientStore, qso: QsoId, snapshot: DirectorySnapshot) -> anyhow::Result<SnapshotOutcome>`. The transaction checks current working parent/key, combines into the current directory record and stores real pending metadata. Internal `async fn apply_in(connection: &mut SqliteConnection, account: AccountId, qso: QsoId, snapshot: DirectorySnapshot) -> anyhow::Result<SnapshotOutcome>` shares this exact behavior with atomic contact saving.
- Async `save::contacts::save(store: &ClientStore, expected: Option<&ClientRecord>, next: Qso, snapshot: Option<DirectorySnapshot>) -> anyhow::Result<SaveOutcome>` calls existing `save_in` and `apply_in` within one immediate transaction; stale contact save installs no snapshot. Existing generic save operations remain for their existing consumers.
- Read projection loads the current working QSO's eligible directory winner in the same snapshot, supplies its facts to Task 1 selectors and exposes locality/provider/lookup time for details. Update `ContactFacts` location payloads to Task 1's `LocationFacts` and carry selected references. QRZ coverage uses the same directory-eligible effective inputs; do not add upload jobs.

Migrations 0013/0011 add `directory_payload` roots keyed by the existing immutable
payload identity, `directory_snapshot` rows unique on `(payload_id, lookup_key)`,
and `directory_reference` children qualified by account and snapshot identity.
The snapshot row owns typed fact/source columns, integer seconds/nanoseconds
and canonical lookup ID text. Reuse current position column conventions and
account-qualified identity foreign keys, including references to the group's
retained identity rather than requiring a live definition. Parent linkage uses
the retained QSO identity, not a foreign key to one particular QSO version.
Add a key-lookup index for the read projection and cascading child ownership
for existing unreachable-payload collection. These are payload relations, not
independent request logs or a second accepted-state store.

- [ ] **Step 1: Add failing pure and real-store scenarios** `directory_refreshes_combine_without_qso_review`, `directory_snapshots_survive_restart_and_key_changes`, and `directory_history_for_deleted_parent_does_not_block_completion`. Assert two providers/same key choose only the max complete tuple; different keys coexist; later local success survives acknowledgment of an older frozen proposal; a directory refresh alone creates no QSO conflict; a genuine two-sided recorded edit still does.

  ```rust
  assert_eq!(replica_a_winner, replica_b_winner);
  assert_eq!(winner.lookup_id(), expected_lookup_id);
  assert_eq!(winner.facts().name, None); // successful whole replacement
  assert_eq!(winner_after_not_found, winner_before_not_found);
  assert_eq!(current_directory_review_count, 0);
  assert_eq!(saved_qso_facts, recorded_facts_before_refresh);
  ```

  Not-found is an adapter outcome, not a synthetic snapshot passed to a store. The integration reaches it through the real application boundary once Task 6 is available; before then pin winner no-op behavior without inventing a failure-record API.
- [ ] **Step 2: Run** `cargo +1.99.1 test -p clacks-sync --test integration --locked --offline directory` and `cargo +1.99.1 test -p clacks-client-store --test integration --locked --offline directory_snapshots`; expect missing behavior/assertion failures.
- [ ] **Step 3: Implement** relational payloads and actual save/reload/projection paths. Store lookup key/provider/time/ID, optional name/locality, checked position/geographic fields, supplied-position metadata and reference rows using named row structs. Extend identity kind decoding, native roots and fixed queries in both stores. Preserve existing post-commit QSO save events/counters in the new `save::contacts::save` path; calling `save_in` must not silently bypass their measurement. Include directory-only contacts in the existing effective name/grid filtering and sorting prerequisites; add `directory_only_contacts_participate_in_effective_queries` to `tests/history_queries.rs` with actual saved contacts and current query syntax/builders. Assert effective name/grid matches include the directory-only contact while explicitly recorded-field queries keep their existing meaning. No complete-domain JSON payload and no per-refresh entity/table.
- [ ] **Step 4: Implement** direct value-policy branches across local save, pending rebase, frozen receipt, shared acceptance and accepted-history recovery. Reuse preparation, stale-condition replies and automatic client retry. New publication needs a live accepted parent; missing parent waits through existing dependencies; deleted parent produces the typed terminal outcome. Already accepted snapshots with known deleted parents install inertly. Current normalized key gates use, not retained storage or current provider selection.
- [ ] **Step 5: Extend real bounded-transfer/recovery scenarios** through both concrete stores: snapshot before parent page, interrupted page before final receipt, full traversal without old journal, deleted parent tombstone, retired author carried by an authorized client, baseline projection and an older restored server. Assert checkpoints only after durable installation, no empty success for missing parent evidence, preserved lookup tuples and no revival of deleted contacts. Reuse current transfer/restore fixtures and real application operations; no fabricated database rows/triggers.
- [ ] **Step 6: Regenerate both metadata owners sequentially**, then run Step 2 commands plus `cargo +1.99.1 test -p clacks-server --lib --locked --offline directory`. Expect passing scenarios. Run existing client-store incoming/recovery and sync recovery/compaction cases affected by the new branch; do not broaden into unrelated test audits.
- [ ] **Step 7: Commit** `Persist and synchronize directory snapshots independently of QSO edits`, using the commit procedure, with production consumers/grants/metadata included.

### Task 6: Manual-entry lookup, save correlation and effective display

**Files**

- Create `lib/clacks-desktop/src/directory/lookup.rs`, `src/directory/lookup/tests.rs`, `src/ui/logging/directory.rs`.
- Modify `src/directory.rs`, `src/session.rs`, `src/session/{opening,shutdown}.rs`, `src/session/settings/contacts.rs`, `src/logging/{defaults,form}.rs`, `src/ui/logging.rs`, `src/ui/logging/{entry,details,responses,tests}.rs`, saved-contact editor consumers and read-projection field rendering affected by Task 1.
- Extend `src/session/settings/contacts/tests.rs` and `src/enrollment/tests/synchronization.rs` for the actual store/session boundaries.

**Interfaces**

- Reuse `NewEntry.id: QsoId` as the unfinished entry's stable identity; do not add a second entry-ID system. A request captures that QSO ID, normalized key, `LookupId`, provider/material identity/revision and original response ownership.
- Define concrete `LookupCommand::{CallChanged { qso, call }, Saved { qso }, Deleted { qso }, Retry}` and `LookupUpdate { qso: QsoId, key: LookupKey, state: LookupState }`. `LookupState` contains Pending, Found(snapshot), NotFound, PausedAuthentication and Failed(safe diagnostic) variants. Empty calls clear presentation through the corresponding command outcome rather than constructing an empty `LookupKey`.
- Async `lookup::run(store: ClientStore, binding: Option<EnrollmentBinding>, installed: watch::Receiver<Option<DirectoryInstallation>>, commands: mpsc::Receiver<LookupCommand>, updates: mpsc::Sender<LookupUpdate>, changes: watch::Sender<()>, stop: CancellationToken) -> ()` owns direct provider sessions and pending responses under the existing runtime. It loads selected material through Task 3 and dispatches Task 4 adapters.
- Extend `ContactAction::Save` with `snapshot: Option<DirectorySnapshot>` and invoke Task 5's `save::contacts::save`. Publish `Saved` only for a committed/unchanged actual saved QSO, not a submitted form or a stale failure. An arriving late result calls `snapshots::apply`; Applied wakes existing local sync and contact read invalidation.

- [ ] **Step 1: Add failing clock/ownership cases** `directory_lookup_debounces_actual_calls_and_preserves_saved_owner`, `directory_lookup_does_not_pause_new_material_for_old_rejection`, and `directory_pending_save_routes_result_by_committed_outcome`. Use Tokio's simulated time (`test-util` only as a dev feature where needed), egui_kittest semantic input helpers and actual request/callback assertions.

  ```rust
  assert_eq!(lookup_requests_at_499_ms, 0);
  assert_eq!(lookup_requests_at_500_ms, 1);
  assert_eq!(requests_after_unchanged_call_notification, 1);
  assert_eq!(requests_for_empty_call, 0);
  assert_eq!(original_saved_qso_snapshot.lookup_id(), completed_lookup_id);
  assert!(next_entry_snapshot.is_none());
  assert_eq!(current_material_authentication_pause, false);
  ```

  In the same scenarios cover failed/stale save, unchanged normalized key versus changed key, deletion, late result after moving to the next contact and provider replacement while a request is in flight. Do not invent a provider-switch display rule or retain results on a different contact.
- [ ] **Step 2: Run** `cargo +1.99.1 test -p clacks-desktop --lib --locked --offline directory`; expect missing lifecycle/UI assertions to fail. Use component hosts for interaction, real sessions/stores only for the persistence/transport claims.
- [ ] **Step 3: Implement** effective-call observation, 500 ms debounce and captured ownership. Save remains immediate and uses its existing frozen QSO ID; a completed preview can save atomically with the contact. While save is pending, retain the response until the actual save outcome is known. Correlate saved editor changes through the same key rules. Ordinary moving/clearing an unfinished display does not cancel eligibility of a saved original QSO's response.
- [ ] **Step 4: Implement** provider/session replacement and auth-paused/explicit-retry handling. New requests use installed selected material; already-started requests use ordinary contact/key/deletion checks. A rejection can pause only its captured current material. Failures/not-found do not call snapshot replacement; logging and sync continue while disconnected from either provider or sync server. Do not add automatic enrichment catch-up.
- [ ] **Step 5: Implement** effective source-labeled name/location/reference and locality display in existing quick-entry/details/editor composition. Keep recorded text controls separate from fallback display so tabbing, clearing or saving cannot copy a directory value into recorded facts. Show current lookup/provisioning diagnosis and explicit auth retry without a modal save prerequisite. Include visible HamQTH attribution and existing shared UI styling.
- [ ] **Step 6: Instrument and verify** the spec's four metrics at their exact owning boundaries: actual HTTP dispatch, terminal logical lookup, admission-to-terminal duration excluding debounce, committed config install. Use captured LookupId for safe delayed-result correlation, existing tracing context/targets and no payload dumps. Extend current telemetry capture scenarios with selected assertions, not a new telemetry framework or duplicate getter tests. Run Step 2, the model suite, and client-store directory cases; expect passing assertions and selected metrics/events with no sentinel secret.
- [ ] **Step 7: Commit** `Connect directory lookup to manual logging and effective display`, using the commit procedure. Include Task 4 if its real consumer was needed before a compiling commit.

### Task 7: Integrated acceptance, recovery and maintained documentation

**Files**

- Extend the existing enrolled production-flow scenario in `lib/clacks-desktop/src/enrollment/tests/synchronization.rs`, web `tests/directory.rs`, server `src/store/tests/synchronization/directory.rs`, and existing `src/store/tests/synchronization/restore.rs` cases.
- Update `README.md`, `lib/{clacks-model,clacks-client-store,clacks-server,clacks-desktop,clacks-web}/README.md` where behavior/usage changes, `docs/accounts.md`, `docs/credential-protection-proposal.md`, `docs/client-store.md`, `docs/observability.md`, and `references/callsign-directory.md` where older wording conflicts with delivered details. Do not create review-history appendices in product contracts.

- [ ] **Step 1: Complete the real end-to-end scenario** `directory_setup_to_offline_lookup_and_synchronized_snapshot`: production browser credential entry → account-authorized provisioning → actual native entry → real HTTP fixture lookup → actual contact save/late result → second client's snapshot/read projection. Make the sync server unavailable after provisioning and demonstrate a new direct lookup/save still succeeds; reconnect and converge without a QSO conflict. Verify replacement/removal/reconnect, selected-provider-only dispatch and unchanged enrollment authorization in that same workflow.
- [ ] **Step 2: Complete meaningful recovery/maintenance coverage** in existing fixtures: provision after account rotation/rewrap; restore an older database/config, reconnect without a local revision gate, preserve directory snapshot ordering/recorded facts, and complete fuller catch-up with deleted parents. Verify repeated requests/receipts and repeated maintenance become no-ops where specified. Do not retest dependency durability or introduce a separate backup product.
- [ ] **Step 3: Run the final automated gate** below once, after all changes and metadata are current. Investigate real failures; rerun only affected checks after corrections, then the required final gate when justified. These commands are planned execution checks, not claims made while writing this plan.
- [ ] **Step 4: Inspect the assembled production desktop/web** on the actual available platform: configured/unconfigured/disabled setup; empty/pending/found/not-found/auth-paused/provisioning-failed lookup; manual name/grid edits, fallback source, keyboard save, next entry, saved edit/delete and resizing. Record screenshots/platform/exercised states and Windows limits in delivery evidence. Native-store failure checks use actual noninteractive environmental conditions when available; never introduce a test-only backend just to claim native failure coverage.
- [ ] **Step 5: Perform live provider checks only with authorized available test credentials.** Verify login, session reuse/renewal and lookup; record unavailable subscription/error variants honestly. Keep live external requests out of the automated suite. Do not create accounts, register new identity providers or publish/deploy to obtain evidence.
- [ ] **Step 6: Reconcile documentation and commit** `Document and verify the delivered directory workflow`. Use actual delivered behavior and evidence; preserve source provenance and agreed scope. Keep #17 open until the usable capability and meaningful verification are delivered. Any later issue/PR update reports remaining native/live limits; planning documents alone do not establish completion, and this plan does not authorize posting messages externally.

## Query metadata and final commands

Use the currently documented pinned SQLx CLI and disposable production-schema
databases. Do not regenerate either store's metadata against the wrong backend,
or run both metadata writers concurrently: they share scratch output.

For SQLite, from repository root after applying current client migrations to
the disposable query-check database:

```sh
SQLX_OFFLINE=false DATABASE_URL="$CLACKS_SQLITE_DATABASE_URL" cargo +1.99.1 sqlx prepare --workspace -- --package clacks-client-store --all-targets --locked --offline
SQLX_OFFLINE=false DATABASE_URL="$CLACKS_SQLITE_DATABASE_URL" cargo +1.99.1 sqlx prepare --check --workspace -- --package clacks-client-store --all-targets --locked --offline
```

For PostgreSQL, from `lib/clacks-server`, after applying production migrations
and both runtime/maintenance grants to the disposable test database:

```sh
env -u CLACKS_SQLITE_DATABASE_URL SQLX_OFFLINE=false DATABASE_URL="$CLACKS_TEST_DATABASE_URL" cargo +1.99.1 sqlx prepare -- --all-targets --locked --offline
env -u CLACKS_SQLITE_DATABASE_URL SQLX_OFFLINE=false DATABASE_URL="$CLACKS_TEST_DATABASE_URL" cargo +1.99.1 sqlx prepare --check -- --all-targets --locked --offline
```

Final automated gate, from repository root with `SQLX_OFFLINE=true`, the two
disposable test database variables configured, and the ordinary build settings
used by `.github/workflows/rust.yml`:

```sh
cargo +1.99.1 fmt --all --check
cargo +1.99.1 build --workspace --all-targets --locked --offline
cargo +1.99.1 test --workspace --all-targets --locked --offline
cargo +1.99.1 test --workspace --doc --locked --offline
cargo +1.99.1 clippy --workspace --all-targets --locked --offline -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo +1.99.1 doc --workspace --no-deps --document-private-items --locked --offline
cargo +1.99.1 test -p clacks-web --features ssr --test integration --locked --offline directory
git diff --check
```

Also build actual browser server-rendering/hydration assets through the existing
`cargo leptos build` workflow, and run the built desktop's `--help` smoke check
as CI does. Ordinary workspace compilation alone does not prove browser assets
or assembled UI behavior. Expected final results: all selected automated tests
and checks pass, browser build succeeds, actual UI/native evidence is recorded,
and no credentials or unrelated files appear in the staged diff.

## Self-review and coverage

| Spec section | Owning tasks |
| --- | --- |
| 1–2: agreed outcome, existing facilities and proportionate boundaries | All tasks; file/dependency map and global constraints |
| 3: native values, mapping, normalization and effective references | 1, 4, 5, 6 |
| 4: authorized web changes, encryption, stale writes and key maintenance | 2, 7 |
| 5: protected provisioning, restore, native ordering and failure isolation | 3, 6, 7 |
| 6: concrete provider sessions and failure classification | 4, 6, 7 |
| 7: debounce, immediate save, late results and display | 5, 6, 7 |
| 8: durable snapshots, value arbitration, recovery and output | 1, 5, 7 |
| 9: safe tracing, bounded metrics and committed effects | 2–7 at their owning operations |
| 10: real behavior verification and visual/native boundaries | Tests in each task and 7 |
| 11: implementation boundaries and provider/library provenance | Spec remains authoritative; changed documentation in 7 |

Self-review must check exact path/signature consistency, each Review Focus case,
all record-family branches, no placeholder implementation decisions, no extra
approval gates and no test-only abstractions. Compile-time fallout follows the
listed schema/type changes; it does not authorize redesigning unrelated modules.
Execution and any deviations require the ordinary plan-review handoff, not
reopening settled product questions.

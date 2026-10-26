# Durable offline client storage

This consolidated specification defines the production client store for
[issue #6](https://github.com/jhelwig/clacks-log/issues/6). It covers relational
representation, saved-state lifetimes, transaction boundaries, schema upgrades
and verification. The [store guide](../lib/clacks-client-store/README.md) describes
the production APIs and developer commands. The [native model](native-model.md), [synchronization model](synchronization-model.md),
[core integration obligations](../lib/clacks-sync/README.md#real-transaction-obligations)
and [service-evidence contract](service-evidence.md) remain authoritative.

## 1. Scope and selected approach

Deliver a production `lib/clacks-client-store` library that later desktop and
synchronization runtimes use directly. It owns SQLite opening, migrations,
relational mapping, transactional local saves, persisted synchronization
transitions and consistent reads. It does not implement a second synchronization
algorithm, a server, network transport, provider requests, an entry UI or an
artificial executable environment. No new application crate is needed for this
capability. Rust remains 1.99.1; dependencies and package metadata are inherited
from the workspace.

Selected requirements:

- Fully relational native values and relationships, using SQL for filtering,
  joins and aggregation. Users need not write SQL. Ordinary indexes optimize
  queries rather than define the only queryable fields.
- Shared payload table families with explicit accepted, working, original,
  prepared and retained-alternative references. No per-edit event history.
- SQLx with fixed queries in separate `.sql` files, included through
  `sqlx::query_file!` and `sqlx::query_file_as!`. Commit offline query metadata;
  ordinary builds require no database. Dynamically structured queries bind
  values and receive integration coverage.
- Write-ahead logging (WAL), `synchronous=FULL`, standard automatic WAL
  checkpointing, immediate write transactions and ordinary snapshot reads.
- Embedded automatic startup migrations, forward-only and transactional per
  migration. Reject unsupported newer schemas without modifying them. Preserve
  enrollment, local work and comparison evidence through supported upgrades.
- Ordinary SQLx connection/transaction facilities, bounded contention handling,
  no custom write queue and no network/user interaction inside transactions.
- One SQLite database per local account workspace, including every tag-defined
  logbook in that workspace.
- SQLite `INTEGER` for frequency, transmit power and Clacks-controlled counters;
  decimal strings for QRZ book/record IDs, with pagination arithmetic in Rust.
- Stale-save protection checked inside the committing transaction, without
  prescribing additional generation counters or a separate UI conflict workflow.

Serialized authoritative records plus query projections were considered and
rejected in favor of relational querying. Separate schemas for each payload role
were rejected in favor of shared domain tables. Deferred write transactions and
runtime-only checking of fixed queries were considered; immediate transactions
and checked query files were selected. These choices do not change native
record ownership or whole-record reconciliation.

## 2. Storage boundary and encodings

### 2.1. Local store and account identity

Use one database for one local account workspace. All its tags, contacts,
profiles, equipment and synchronization/evidence rows share that database;
there is no database per tag. The caller supplies the database path. Discovering
application directories and desktop startup belong to the
[desktop specification](desktop.md#4-workspace-and-configuration); account switching
and authentication UI are separate capabilities. This layout does not add a
multi-account desktop UI. Desktop reopening discovers the stored account through
the same read-only compatibility preflight; expected-account callers retain their
account check. This integration requires an opening API extension, not desktop
SQL, an identity registry or another database marker.

`store_account` is a singleton containing the native account ID, optional server
binding, optional current client actor ID, and a local-only/enrolled state.
Standalone offline use may allocate a local account ID, but does not fabricate a
server account, accepted timeline, admission or installed checkpoint. Work may
be saved with unavailable shared/original evidence until actual shared evidence
exists. This is not certified absence. D56 still requires exporting independently
populated data and importing it into the empty enrolled workspace; this design
does not silently remap existing local data into an unrelated account.

An unenrolled store has no sendable preparation operation: return a not-enrolled
outcome without allocating a client actor or counter. In an enrolled workspace,
locally created records use certified absence only when established against an
actual complete installed view. Do not replace that check with the absence of a
row in `record_identity`.

Keep account qualification in keys and foreign keys even within a single-account
database. Opening with a different expected account fails. This prevents accidental
cross-account mapping and permits safe account-scoped APIs; paths are not account
authorization. Enrollment secrets are references to protected operating-system
storage, never plaintext credentials in SQLite. Logbook of The World (LoTW)
signing private keys remain server-side. Identity bindings are installed by later enrollment integration;
opening or copying a database does not issue a fresh client actor ID.

### 2.2. Value encodings

Use SQLite `STRICT` tables. In the following table inventory, `?` means nullable,
`A` means the owning account, and `P` means a local payload key. These are notation,
not actual SQL types. Every table has an explicit primary key; child tables have
foreign keys to their owner. Use binary text comparison for identities and exact
recorded labels.

| Native value | Columns and rules |
| --- | --- |
| Native IDs | Canonical 26-character Universally Unique Lexicographically Sortable Identifier (ULID) text, decoded through the native ID type. Account-qualified keys also contain kind, including the service subtype. Same bytes in different kinds are distinct. |
| Internal row keys | SQLite signed integer keys, never exported or used as causal evidence. No requirement that allocation be monotonic or globally unique. |
| Whole-second UTC time | Signed `INTEGER` Unix seconds, including dates before 1970. UTC means Coordinated Universal Time. |
| Operational timestamp | Signed seconds plus an integer nanosecond fraction, `0 <= fraction < 1,000,000,000`. Preserve this precision for lookup tie-breaking and provenance; do not add precision metadata to QSO times. |
| Coordinates | Signed integer millionths of a degree; explicit-point/grid-area discriminator and retained locator length. No stored authoritative grid-center coordinates for a grid area. |
| Boolean/closed state variants | Integer or text discriminators with checks and variant-specific nullability. These describe native structure, not current catalog recognition. |
| Optional recorded text | `NULL` for absent. Preserve nonblank text exactly; no blanket database trimming or case folding. Native entry validation owns normalization. |
| Mode | Kind `parent` or `specialization`, preserved label, nullable recorded parent context. No foreign key to the current recognition catalog. |
| Band | Preserved nullable text. No catalog-membership check. A frequency may be valid without a recognized band. |
| Frequency | One positive SQLite `INTEGER` column per recorded frequency, in whole hertz; `NULL` when absent. Valid range is 1 through 9,223,372,036,854,775,807. |
| Transmit power | One positive SQLite `INTEGER` column, in whole milliwatts; `NULL` when absent. Valid range is 1 through 9,223,372,036,854,775,807. |
| Clacks-controlled counters | SQLite `INTEGER` constrained to 0 through 9,223,372,036,854,775,807: record-vector components, local counter high-water marks, timeline revisions/coverage, membership generations and transfer-progress generations. Explicit vector components and accepted revisions are nonzero; origins and initial generations may be zero. No additional local save/checkpoint generation counters are prescribed. |
| QRZ book and record IDs | SQLite `TEXT` containing validated canonical decimal digits, without leading zeros except for zero itself. Use binary text equality, uniqueness and joins; do not use lexical order as numeric order. Unknown book IDs remain `NULL`. Preserve existing domain validity rules separately from encoding. |

Use ordinary integer predicates, sorting and aggregation for frequency and
transmit power, with positive-value checks on every such column, including
operating defaults and retained service evidence. Do not split these quantities
into high/low columns or encode them as strings. Native quantity validation uses
the signed 64-bit maximum, so every valid native quantity is directly
representable. Reject out-of-range input rather than truncating,
wrapping or converting to floating point. This changes neither whole-hertz nor
one-milliwatt resolution. SQL arithmetic and aggregates retain SQLite's ordinary
overflow behavior; no arbitrary-precision arithmetic is implied.

Clacks-controlled counters use the same signed 64-bit maximum in native validation
and storage. Retain checked increments and exact equality/order comparisons;
exhaustion is an error, never wraparound, saturation, counter reset or a reason
to generate a new actor/timeline automatically. Record-vector joins/comparisons
and ancestry interpretation remain existing Rust-core operations. SQL range
support does not authorize scalar comparison across different timelines.

The [QRZ Logbook API Developer Guide](https://www.qrz.com/docs/logbook/QRZLogbookAPI.html)
defines both `bookid` and `logid` as integers and illustrates decimal wire values,
but specifies no 32/64-bit or signed/unsigned storage contract. The access key
selects the book; `STATUS` can return its `bookid`. Record IDs occur in
`LOGID`/`LOGIDS` responses and `app_qrzlog_logid` fields. `FETCH` supports an
inclusive `AFTERLOGID` threshold, and its documented paging procedure advances
to the greatest returned ID plus one. Record IDs therefore have a numeric paging
use, although that does not require comparisons or arithmetic in client SQL.
Do not infer a provider limit from sample ID sizes.

Store QRZ book and record IDs as decimal strings. Handle pagination comparisons,
selection of the greatest returned record ID and addition of one in Rust, not
SQL. Preserve the numeric identity exactly without conversion through SQLite
`INTEGER` or floating point. Native `QrzBookId` and `QrzRecordId` use validated,
unbounded decimal identities; the book ID remains positive while a record ID may
be zero. Canonicalization
makes different leading-zero spellings of the same integer share one identity;
it does not change destination qualification or record association semantics.

Do not persist domain records as serialized Rust structs, enum trees, vectors
or maps in JSON/blob columns. Application preferences use the separate local
key/value storage described in section 5.2. Opaque external bytes, when an actual
domain type requires them, are bytes; they are not an escape hatch for relational
model mapping. Arbitrary catalog metadata is descriptive input, not a reason to
serialize QSO payloads.

### 2.3. Integration with existing native types

Native types use these selected representations before database binding:

- Frequency and transmit-power validation enforce the positive signed 64-bit
  range while retaining whole-hertz/one-milliwatt units and existing semantics.
- All synchronization counter/revision/generation input and transition boundaries
  enforce the nonnegative signed 64-bit range and the existing zero/nonzero rules.
  A Rust `u64` field may remain a representation choice; it does not permit values
  outside the selected domain. Validate externally supplied/public-field composite
  values before using them, and make increment at the signed maximum fail.
- QRZ book and record identifiers are validated decimal identities in native
  destination/remote-record values, including historical service descriptors.
  Equality uses canonical decimal text. Pagination uses numeric comparisons and
  increment in Rust with no signed/unsigned 64-bit truncation. Existing domain
  restrictions, including rejecting a known zero book ID, remain in force.
- Preserve the rest of the native model and synchronization transition contracts.
  SQLx bindings, storage row structs and local surrogate keys belong to the store
  library. Do not introduce storage dependencies into the pure domain libraries.

These are production integration changes required by this capability; the current
libraries do not already implement them merely because they are specified here.

## 3. Record identity, payloads and role references

### 3.1. Core tables

| Table | Key and substantive columns | Meaning |
| --- | --- | --- |
| `record_identity` | `(A, kind, native_id)` | Known identity only. Presence does not establish live existence, deletion or acceptance. |
| `payload` | `P`, `(A, kind, native_id)`, `state` | One complete live value or explicit deletion. Live values have exactly one matching domain root; deleted values have none. |
| `version_stamp` | `stamp_id`, `(A, kind, native_id)`, accepted timeline/revision, baseline ID, vector ID | Exact accepted context, scoped to its record. |
| `accepted_record` | `envelope_id`, `(A, kind, native_id)`, `P`, `stamp_id` | Complete accepted envelope; may be latest, retained, staged or exact service support. |
| `record_observation` | `observation_id`, record key, kind, `envelope_id?`, `checkpoint_id?`, `unknown_reason?` | Exactly `Accepted`, certified `Absent`, or `Unavailable`; never conflate absence with missing data. |
| `client_record` | Record key, `latest_observation_id`, `pending_id?` | Durable `ClientRecord` root. Stale-save protection checks the edit/review basis inside the write transaction without requiring another generation counter. |
| `retained_accepted` | Record key, ordinal, `envelope_id` | Exact ordered representation of the core's retained incomparable alternatives. Not edit history. |
| `pending_record` | `pending_id`, record key, `original_id`, `working_P`, `prepared_id?`, `basis_witness_id?` | Current unpublished saved work. One per record. |
| `original_basis` | `original_id`, record key, variant, accepted envelope/checkpoint/subsumed payload/vector/witness stamp/reason as appropriate | All four `OriginalBasis` variants, with exact variant-specific checks. |
| `prepared_change` | `prepared_id`, record key, `P`, actor, vector ID, expected-condition ID | At most one unsettled frozen change per pending record. |
| `prepared_dependency` | `(prepared_id, ordinal)`, condition ID | Actual ordered dependency conditions. |
| `version_condition` | Condition ID, record key, kind, stamp ID or absence checkpoint ID | Existing `Is`/`Absent` preconditions; no invented serialized predicate language. |
| `local_counter` | `(A, kind, native_id, actor)`, high-water value | Counter reservation survives preparation, response loss and later actor changes. |
| `local_review` and retained-review children | Record key, native review reason and variant fields, original basis, local payload, shared observation and ordered retained envelopes | Persist core-produced `ReviewInput` with the associated client update so review survives restart. Reuse existing role/reference tables; no new synchronized identity or local generation counter. |

All references crossing these tables include account and record qualification,
either directly in a composite foreign key or through an equivalent unique
qualified parent key. A payload for QSO X cannot occupy a role for QSO Y. A stamp
cannot be shared across different record identities just because its numeric
components match. Optional role references have explicit null/variant checks.

Live payload completeness is enforced by the owning transactional mapper plus
native validation; SQL checks/foreign keys enforce the locally expressible parts.
SQLite cannot express an arbitrary cross-table assertion as a `CHECK`. Do not
claim the type discriminator alone proves a complete live payload. Readers reject
missing/extra/wrong-kind child rows with a concrete integrity error rather than
silently returning an incomplete native record.

Payloads reachable from more than one role are immutable. A changed saved value
gets a new complete payload and the transaction replaces the appropriate role
reference. Sharing identical values is optional: there is no global content hash,
mandatory deduplication, ancestry edge or revision graph. Superseded unreferenced
payloads can be deleted within the committing transaction. Do not keep old local
saves merely to provide a history feature that was not requested.

### 3.2. Foreign keys and missing domain references

Enable foreign keys on every connection. Cascade deletion only for owned
structural children of an otherwise collectible payload or metadata object.
Never cascade deletion of a tag, group or equipment configuration into recorded
contacts, original snapshots or pending work.

Membership and profile-default references point to account-qualified identity
rows, not to a live payload. Allocate a bare identity row for an unfamiliar group
association or received missing reference without inventing its definition or
claiming it exists live. Retain the bare identity as long as any stored reference
needs it. Transactional domain checks use actual observations to decide live
existence and admission. A missing/deleted referenced tag can therefore coexist
with a valid locally saved QSO pending explicit removal or replacement (D250).
A profile's deleted equipment reference follows D183. Neither case is database
corruption or permission to recreate the referenced record.

### 3.3. Native payload table families

All tables below are keyed by `P` or by an explicitly owned child key under `P`.
They are shared across all payload roles. They represent recorded values; provider
fallback and current catalog interpretation never rewrite them.

| Table/family | Columns and relationships |
| --- | --- |
| `qso_payload` | `P`, start/end seconds, own/contacted station child keys, primary frequency integer and band, receive-override presence/frequency integer/band, mode kind/label/parent, sent/received reports, comment, own operator columns, contacted operator columns, transmit-power integer, copied equipment child key. Required primary frequency-or-band and end >= start checks. Receive absence means no override; a band-only override has no frequency. |
| `station_snapshot` | Account-qualified station key, required recorded callsign, optional location row keyed by that station. Used by QSO sides, saved profiles and copied local operating values. |
| `recorded_location` | Station key, account, position kind, area locator or explicit latitude/longitude/grid length, DXCC entity text, primary/secondary subdivisions, CQ/ITU zones. DXCC is DX Century Club; ITU is International Telecommunication Union. Explicit points retain locator precision, not a second independently authoritative locator. |
| `station_reference` | Station child key, group ID, nullable identifier. Unordered set; unique `(station, group, identifier)` for present identifiers and a separate partial unique index on `(station, group)` for null identifiers. SQLite null uniqueness alone cannot enforce deduplicated group-only participation. |
| `qso_tag` | `(P, tag_id)`. Unordered unique membership set, including the empty set. No separate synchronized membership identity. |
| `tag_payload` | `P`, name, per-service `Enable`/`Forbid`/`Neither` policies. Names not unique; new tags use Neither. |
| `profile_payload` | `P`, name, station child key, nullable default equipment ID. Callsign required by the station. |
| `equipment_payload` | `P`, name, copied-equipment child key. |
| `equipment_snapshot` | Account-qualified equipment key, optional equipment description, antenna description, notes. No configuration display name in QSO copies. |
| `group_payload` | `P`, name, identifier-required flag, syntax, location-context flag, optional catalog binding key. |
| `group_normalization` | `(P, ordinal)`, native text transformation. Preserve sequence order. |
| Catalog columns on `group_payload`, `catalog_display_detail` | Owned binding, dataset ID, lookup-expression key, display-name metadata field; ordered detail labels/metadata fields. Descriptive catalog entries themselves remain separate from recorded associations. |
| `output_profile_payload`, `output_context_field` | `P`, name, purpose variant; unordered contextual field set. |
| `export_plan` | Owned plan key, group `P`, ordinary/targeted discriminator, nullable output-profile ID. One ordinary plan; at most one targeted plan per profile. |
| `export_field`, `export_partition`, `export_expansion` | Owned plan key and ordinal; field name/expression/condition; partition variant/expression or side/binding; expansion side/binding/keep-one flag. |
| `export_expression` | Owned expression key, group `P`, native variant, literal/path/side/binding/pattern/capture/separator fields as applicable; child expression key for Capture/Join. This is the existing bounded mapping expression tree, not a generic record-property store. Reject cycles, orphan roots and variant mismatches. |

Position kind/null checks distinguish no position, grid area and explicit point.
Store all supplied locator lengths through twelve characters; own-location entry
validation still requires four. Coordinate/grid consistency and unknown-mode
preservation follow the native constructors rather than current catalog joins.
Inappropriate relational constraints must not make recognition upgrades into
native edits. Maps use unique keys; vectors preserve ordinals; sets have explicit
uniqueness including nullable-element cases.

## 4. Synchronization metadata tables

Metadata is relational too. Locally allocated keys below identify saved objects,
not new wire identities. Reuse the same metadata families for main state,
originals, witnesses, manifests and service support. Keep immutable historical
interpretations distinct when their meanings differ.

| Table/family | Required contents |
| --- | --- |
| `account_metadata` | Account, observed head timeline/revision, baseline, installed-coverage ID, membership generation. Advertised head is not the installed checkpoint. Absent before actual synchronization metadata exists. |
| `record_vector`, `vector_component` | Vector ID qualified by record; unique `(vector, actor_role, actor_id)` with positive integer counter. Server timeline and client actor roles remain distinct. |
| `coverage`, `coverage_entry` | Coverage ID; unique timeline/revision pair per timeline, preserving explicit zero coverage. |
| `checkpoint`, `installed_checkpoint` | Immutable account/baseline/coverage object; singleton current installed checkpoint reference. Use the core's expected checkpoint and progress conditions; no additional local generation or maximum-observed-record shortcut. |
| `lineage`, `timeline_boundary`, `current_lineage` | Lineage ID/account/root timeline, boundary at timeline/revision, kind and included coverage. Preserve the position at which ancestry became known. |
| `baseline` | Account/native baseline ID, coverage, complete lineage snapshot, membership generation. Conflicting contents under one baseline ID fail; do not upsert over them. |
| `membership_snapshot`, `client_membership` | Account/generation, actor, created/closed positions, optional acknowledged checkpoint. Closed actor metadata is evidence, not permission to accept new work. |
| `basis_witness`, `witness_baseline` | Record key, original basis, observed stamp, checkpoint, relation, lineage snapshot, ordered retained baseline references. Preserve actual original/observed contexts; never relabel to a newer checkpoint. |
| `transfer_window` | Window key, account, immutable source checkpoint, mode and incremental-after coverage when present. Logical identity is exact `(source, mode)` in that account. |
| `window_requirement` | Window plus record key, exact required stamp and deletion flag. |
| `window_boundary`, `window_baseline` | Window references to its complete interpretation requirements. |
| `window_unit`, `window_unit_member` | Window, operation ID, exact ordered member version references and deletion flags. |
| `window_progress` | Window, generation value. |
| `window_installed`, `window_staged`, `window_completed_unit` | Installed key set; ordered staged accepted envelopes; completed operation ID set. Staging is not installation. |

Compare decoded manifests using `validate_replacement` under the same source/mode
identity. Surrogate IDs or a source-checkpoint hash do not substitute for checking
all immutable contents. Persist exact dependency stamps/deletion flags rather
than only native IDs. Rows referenced by a witness or staged unit remain reachable
even when they are no longer the latest shared version.

The store retains metadata necessary to replay the existing transitions; it does
not become the authority deciding server admission, retirement or common coverage.
The server selects its common boundary under the oldest authorized client's
acknowledgment rule. Client installation preserves local witnesses before
releasing detail and acknowledging a newer baseline.

## 5. Existing service records and local operating state

### 5.1. Relational service payloads

The existing `RecordValue::Service` uses the same payload and role machinery.
Map each `ServicePayload` variant to its own table, keyed by `P`; do not replace
it with a serialized service blob. Current credentials are not required to read
historical public descriptors.

| Variant/table | Required native fields/children |
| --- | --- |
| `service_destination` | Destination ID, collection variant, LoTW account or optional decimal-text QRZ book ID, description. Frozen copies can also belong to support records. |
| `service_assertion` | QSO, service, optional destination, claim/date, import source/record/mapping, frozen baseline. Baseline children retain native end, projected field map, own location, confirmation basis and covered-field set. |
| `service_attempt` | QSO, projection variant/destination, dispatch phase, operational time, optional batch. LoTW projection retains mapping release/commit, public signing callsign/entity, actual contact/station field maps and exact service key. QRZ projection retains its actual field map. |
| `service_result`, `result_member` | Destination, individual/batch scope and exact attempt membership, outcome and its diagnostic payload, phase/time, separate diagnostic, optional external key and explicitly identified attempt. |
| `service_observation` | Destination, complete/incomplete identity, remote facts, provider ordering times, original acquisition ID/query/observation time. Incomplete identity retains available source-field map. |
| `service_association` | External key, QSO, Active/Superseded/DeletedQso state. Historical values remain in their payload roles. |
| `service_disposition` | Exact observation ID, processing decision, optional association. |
| `service_suppression` | QSO and exact external key, independent of query progress. |
| `service_download`, `stream_progress` | Destination, optional active acquisition, exact stream/filter/selected-field semantics and optional completed watermark per stream. |
| `service_acquisition`, `acquisition_input` | Query, phase including failure text, start/end timestamps, complete flag, unordered retained observation IDs. |
| `service_support`, `support_requirement` | `P`, ordered ExactFact/CoveredEffect version references, optional operation ID, original acquisition stamp and frozen destination copy. |

Use dedicated relational children for query selected-field sets, field maps,
confirmation bases, mode reports, contact reports and station reports. A field
map is `(owner, scope, field_name, value)` with unique keys; it models the native
map, not a generic replacement for typed QSO columns. Reported fields store
`NotReported`/`Clear`/`Value` explicitly with typed nullable values; a null value
alone cannot encode all three. Reference-set reporting also distinguishes an
explicit empty set from unreported/clear. Station reports distinguish own,
counterpart and absent counterpart. Contact start retains second/minute kind.
Provider receipt and confirmation timestamps remain separate ordering scopes.

`external_key` has account/destination/service qualification and either QRZ's
decimal-text provider record ID or LoTW's exact own call, worked call, start, primary band,
receive band and concrete mode. Uniqueness is service-specific with partial
indexes. Matching windows are not keys. `current_external_binding` uniquely maps
that external key to the current accepted association envelope and QSO, retaining
supersession/deletion state. It is a maintained relational index over accepted
association state, updated atomically; pending conflicting bindings do not take
ownership. A recovered incomparable binding remains retained review evidence,
not a second current binding. Validate native destination namespace semantics
transactionally, including unknown QRZ book identities; SQL uniqueness on book ID
alone cannot establish distinct namespaces.

This capability persists existing evidence and supports receipt of complete
coupled units. It does not deliver server submission jobs, downloads or the
provider matching UI. Do not run server acceptance locally to make a pending
client edit appear accepted.

### 5.2. Client-local operating state

Application preferences use shared `local_state(key, value)` storage with a
serialized native value per key. The owning Rust type supplies decoding,
validation and absent-key defaults. For example, `logbooks.selected_view` defaults
to All contacts and is updated atomically with effective selected-tag deletion.
This local preference storage does not replace relational domain records or the
operating snapshots and comparison evidence described below.

`operating_state` is a singleton referencing optional initialized
`local_operating_value` and optional `loaded_profile_baseline` rows.
`local_operating_value` stores the station callsign, optional recorded location
columns using the same position/geography representation as section 3.3,
optional operator callsign/name, transmit-power integer and copied equipment
description/antenna/notes. `local_operating_reference` holds its unordered
group/optional-identifier set, with the same null-aware uniqueness rules as
`station_reference`. These local values have no synchronized native identity.
`loaded_profile_baseline` references a complete copied profile payload and a
`local_operating_value` snapshot immediately after loading. A copied profile
payload here does not create pending shared work. The baseline is not merely a
profile ID. Under D285, a manual station adjustment clears the selected profile
and its baseline reference in the pending state; Apply persists that removal with
the edited values. The detached component retains no source association for
comparison, reload or saved-source indicators. Explicitly loading a profile
creates a new selection/baseline; matching values alone does not.

Client-local operating state independently retains an optional selected equipment
ID and last-loaded configuration/baseline under D281/D285, including selection
through a profile default. Manual equipment adjustment clears only this selection
and baseline in the pending state. Station adjustment leaves equipment selection
intact; equipment adjustment leaves station selection intact. Operator/power edits
clear neither. Apply commits values and optional selections/baselines together;
Cancel writes neither. A profile load without an equipment default preserves
equipment and its optional selection. These comparison copies do not create
shared synchronized work or add fields to historical QSOs.

For selections that remain, compare the latest locally available saved source
against its captured baseline to detect saved-source changes. D284 additionally
distinguishes selection of different equipment from the selected station profile’s
default. Once the station is detached there is no operating profile-default
association to compare, without changing the saved profile’s own equipment link.

The [logbook setup specification](logbooks.md) defines the selected-view value,
catalog/deletion-preview reads and tag-management integration for #8.

Persist the selected view in the local SQLite workspace, separately from
synchronized records and station facts (D304/D308). Named logbooks use stable
tag IDs; built-in views have no tag identity. Restore the last choice on restart,
starting new workspaces in
All contacts. A known-deleted remembered tag falls back to All contacts with an
explanation, while temporary loading failures preserve the remembered choice.
Apply that fallback when selected-tag deletion becomes effective during use,
including local deletion and synchronized deletion (D307). Commit the All
contacts fallback in the same transaction that makes the selected tag's
deletion effective (D308). A deleted selection cannot cause implicit
reassignment of an existing QSO or replace explicit repair of pending
deleted-tag references. Composition uses the existing asynchronous
session/store path rather than a second saved selection in UI preferences.

Selecting or reloading a profile/equipment configuration in the operating-context
editor reads the local working source values and stages the resulting copies
and comparison baselines. Manual adjustment clears only the affected selection
and baseline. Apply commits displayed values and the remaining optional selections/
baselines together in one transaction under the active-state stale-save
guard. D283 permits the saved sources to have changed since selection; their
freshness is not a precondition for applying those displayed local copies. Report
saved-source changes only for still-selected sources, without replacing the form
or advancing their baselines. Ordinary shared record receipt never changes active
defaults. Unsaved manual entry drafts are not
durable QSO records and are outside this store's saved-record API.

Operating-context form edits likewise remain outside durable state until explicit
Apply commits the complete validated context through the guarded operating-state
save. Cancel changes no stored operating values. Saving a reusable profile is a
separate shared-record operation and does not implicitly change active defaults
(D280). Save Profile takes the edited form’s station values under D282, not a
fresh read of the active defaults. It validates and saves the shared profile
under the ordinary new-record/stale-update rules without requiring an operating
state write. Cancelling the form does not undo that separate committed save.
Saving a reusable record does not implicitly select it, reattach detached operating
values or apply them. Station/equipment views share one pending operating edit;
switching views introduces no intermediate acceptance transaction.

Under D287, a known working-state deletion clears a matching active source
selection/baseline in the same local transaction as deletion, preserving copied
values. Effective deletions installed by synchronization require the same cleanup
at the store boundary. Publishing the committed result clears matching selections
in open forms without discarding their edits. Apply must not reintroduce a source
already known deleted; Cancel must not restore it. Keep ordinary active-state
stale-save protection, distinguishing metadata-only deletion cleanup from any
intervening operating-value edit. A missing query result, failed refresh or unknown
source state does not by itself authorize deletion cleanup. See the
[operating-settings specification](operating-settings.md) for transaction boundaries
and referenced-equipment deletion.

Directory snapshots have agreed product semantics but no complete saved carrier
in the present native/synchronization types. This specification does not hide them in
QSO fields or invent a generic service variant. Directory integration must add its
production carrier and relational mapping together, retaining provider/key/time,
separate names/location and deterministic latest-success selection. Geographic
bundles and recognition catalogs remain installed reference artifacts, not
account-owned QSO facts. Their downstream installation/cache work is not required
to complete issue #6.

## 6. Read model and query boundaries

Define ordinary SQL views `local_record_state` and `local_qso` selecting the
pending working payload when present, otherwise the latest accepted payload.
An unavailable/absent latest observation with no pending value has no visible
live record. Select once per record key before joining child sets. Exclude
selected deletion payloads from normal history/Worked Before, and expose them
through pending/conflict review. Pending originals, prepared values and retained
alternatives must not multiply displayed QSOs.

History, prefix Worked Before, tag filters, time ranges and account-wide review
query these relational values. Use `EXISTS` for tag/reference filters or distinct
record identity before pagination rather than duplicating QSO rows through joins.
A QSO in two selected tags is still one QSO. Stable ordering adds native QSO ID
after timestamp. Prefix matching uses bound parameters and escaped wildcard
characters; a recorded `%` is not permission for a different search expression.
Do not infer unknown recognition labels from catalog equality or use text case
folding to modify saved values.

A consistent read session owns one SQLx transaction/connection for the entire
multi-query snapshot. An export reader may page within that session; independent
pool queries do not constitute one snapshot. Network calls and destination-file
publication are outside the store. Composition must bound session lifetime and
release cancelled readers promptly so long readers do not indefinitely impede
WAL checkpointing. No second authoritative working-state projection is stored.

Initial indexes: child ownership foreign keys, QSO start/ID and contacted-call
prefix lookup, tag-to-payload and group/identifier-to-station lookup, pending roots,
stamp/vector/coverage lookups, window requirement/progress keys, service QSO/
destination/observation dependencies and exact external identity. Validate query
plans on representative data. Do not add a generic query language or a public SQL
execution API merely to demonstrate that all fields are relational.

## 7. Transactions and operation boundaries

Every operation below loads its inputs inside its actual SQLite transaction and
validates native rows before passing them to the existing core. Public operations
are purpose-specific store functions; no generic repository/unit-of-work framework
or telemetry wrapper is needed. Composition may reuse private transaction helpers.

| Operation | Reads/checks inside transaction | Atomically committed effects |
| --- | --- | --- |
| Save new/edit/delete native record | Current client row and expected edit basis, original/working/prepared roles, relevant local reference observations; native validation | New working payload, pending original on first edit, unchanged frozen prepared/original on later edits. No invented accepted stamp or installed progress. |
| Save selected-group tag changes | Selected local records and expected edit bases, tag identity; existing whole-QSO policy | Each QSO's complete working tag set and pending state. Groups use bounded transactions; never independent synchronized membership rows. |
| Delete tag with chosen effects | Native deletion plan from current local membership sets and selected choice; current selected view | Tag deletion plus the planned QSO deletions/membership removals and matching selected-view fallback in one local logical-operation transaction. Do not split this operation into partly visible effects. Transaction size is proportional to affected contacts; any future resumable bulk design must preserve the operation contract rather than silently batching it. |
| Change defaults/load profile | Current operating values and optional selected profile/equipment baselines | Values and remaining optional selections/baselines together; manual adjustments clear only the affected selection/baseline under D285. No shared-profile or old-contact edits. |
| Prepare outgoing change | Current client row, actor, counter, native reference conditions, current baseline/lineage/witness | Core `prepare_change` result plus counter reservation and exact frozen payload/dependencies. Commit before exposing a sendable result. Existing prepared version is retried, not reallocated. |
| Receive accepted/rejected response | Current client row, frozen change, current context; `receive_accepted`/`receive_rejection` | Resulting accepted observation and pending/original/retained/witness changes together. A response does not certify completion of an account window. |
| Store manifest | Existing same-source/mode manifest and current interpretation | Complete immutable manifest, requirements/units, initial progress. Unequal replacement fails. |
| Install transfer batch | Persisted manifest/progress, current client rows, support, baseline/lineage and binding conditions; current selected view | `plan_batch` accepted/client updates, staged inputs, installed keys, completed units, next progress generation and any selected-view fallback caused by an effective tag deletion, all together. Never install a partial atomic unit. Staging alone does not make a deletion effective. |
| Complete transfer window | Committed installed rows, requirements/successors, complete units and interpretation, current checkpoint/progress generation | `finish_window` conditional installed-checkpoint update. The first installation requires a full window, no existing checkpoint and the same complete-state verification; never invent an already-installed checkpoint. Commit, then reload checkpoint before returning acknowledgment evidence. |
| Install baseline/projection | Existing interpretations, pending originals, sealed witness requirements, every touched version | New interpretation, projected contexts, required local witnesses, progress/checkpoint changes only when proven. Collect old detail only after new evidence is durable and no remaining root needs it. |
| Resolve review | Exact current original/working/shared/retained versions compared with the basis presented for review | Core `resolve_review` result and retained dependencies. Stale review returns refreshed review input rather than applying an old choice to unseen state. Preparation/counter changes follow core results, not a parallel store algorithm. |

Use SQLx `begin_with("BEGIN IMMEDIATE")` or its selected-version equivalent;
do not manually issue `BEGIN` and then lose SQLx transaction/drop tracking. Read
sessions use normal transactions. As engineering starting defaults, use a small
shared pool (maximum four connections) and a five-second busy timeout,
configurable by composition. These are tuning defaults for verification, not
product guarantees or separately approved requirements. Do not add a
separate writer task. Snapshot holders must not call nested pool operations and
wait for another connection while holding all available connections.

Contention before the transaction starts is bounded. Do not blindly replay a
commit whose outcome is uncertain. Reload durable prepared/progress identities
before retrying; report a storage failure without claiming saved/synchronized.
An error before commit rolls back that transaction. A process can die after
commit but before its caller receives success: reopening discovers the committed
state, and stable native/prepared/window identities make retries interpretable.
No guarantee of exactly-once response delivery follows from SQLite atomicity.

Require stale-save protection: an edit or resolution based on an earlier view
must not silently overwrite intervening saved changes. Re-read and validate the
caller's expected edit/review basis in the same immediate transaction that would
commit the changes. On a stale basis, return an explicit stale outcome with
current values for normal UI refresh/re-evaluation, without changing saved state.
UI cache invalidation and reloading complement this transactional check but do
not replace it. Do not hold a transaction open while the user edits or decides.
Use existing state/version comparisons or another suitable standard precondition;
do not prescribe additional per-record, save or checkpoint generation counters.
The synchronization core's existing membership and transfer-progress generations
retain their defined roles. This is ordinary local concurrency protection, not a
new distributed conflict policy or a separate user-facing resolution workflow.

Imported QSO and imported assertions must eventually share a transaction under
D171. The adapter's private save routines must permit that composition, while the
Amateur Data Interchange Format (ADIF) importer/review/report remains a later
capability. Service receipt honors
`ExactFact` versus `CoveredEffect` dependencies and complete atomic units using
the existing core. No partial progress acknowledgment can stand in for them.

## 8. Worked examples

### 8.1. Save, restart and a later frozen submission

QSO Q has accepted payload P10/envelope E10. Saving a changed comment creates P11,
retains E10 as the original, and sets Q's working reference to P11 in one immediate
transaction. A restart reads P11 in history and retains E10 for comparison.
Preparation reserves actor counter 8 and freezes P11 with its exact expected
stamp and dependencies in one transaction. Only after commit can transport send
it. Saving another comment creates working P12; the frozen change remains P11.
A later accepted P11 response is incorporated by `receive_accepted`, preserving
the genuine P12 tail. The store does not replace that decision with value-pointer
heuristics. Neither local save advances the account's installed checkpoint.

### 8.2. Interrupted transfer and a coupled evidence unit

Window W requires QSO Q and an association/disposition unit. One request delivers
only the association. `plan_batch` stages it; the transaction saves staged input
and progress generation 3 without making the unit current or complete. Restart
reopens generation 3 and the staged envelope. Receiving the remaining unit
members computes a new batch against current rows, installing the complete unit
and generation 4 atomically. A crash before commit leaves generation 3; a crash
after commit leaves the complete generation 4. Only `finish_window` verification
can advance the checkpoint. A lost acknowledgment is regenerated by reloading
that committed checkpoint; no record maximum or observed source head is used.

### 8.3. Stale editor or resolution input

An editor reads QSO Q's working values P11 and retains the relevant edit basis.
Before the operator saves, synchronization installs an update that changes Q's
saved working state to P13. UI invalidation may refresh the editor, but a save
still carrying the P11 basis must remain safe even if that notification has not
been processed. Its immediate write transaction reads current Q and compares the
relevant basis. It returns a stale outcome/current state without saving the old
edit over P13. The same rule applies to a resolution submitted against earlier
original/working/shared/retained values. No additional record generation counter,
long-lived editor transaction or new distributed conflict is introduced.

### 8.4. Tag deletion while local work is pending

QSO Q's original and working payloads reference tag T. Accepted deletion of T is
received while Q has unsynchronized edits. Bare identity T remains, but its live
observation is deleted. The local pending QSO and its membership survive. Normal
review reports the missing/deleted tag; explicit removal or replacement creates
new working values. No foreign-key cascade removes Q, no tag is recreated, and
unrelated incoming records continue installing. Explicit local deletion of T
uses the chosen native deletion plan; marking Q deleted is different from
removing T's membership.

### 8.5. Retirement, compaction and rollback recovery

A returning client may hold an original from an older baseline. Before installing
collected interpretation, save the core-produced `BasisWitness`, its actual
original/observed stamps and required lineage/baselines. That root keeps its
comparison evidence reachable even if current envelopes project out a retired
actor. Revocation blocks new proposals from that actor under the existing rules;
it does not delete accepted envelopes or client-local values.

If a server advertises a regressed history, feed actual advertised/local history
to `check_history`. Preserve installed coverage, accepted envelopes, pending tails
and witnesses separately. Persist received timeline ancestry at its establishment
position; do not fabricate coverage or rewrite original stamps while returning
already distributed accepted data through the authorized hub. Recovery admission,
server timeline creation and upload pausing are runtime/server work under the
existing recovery contracts, not automatic SQLite-open behavior. Reauthorizing a
client never erases its old actor evidence or automatically relabels frozen work.

### 8.6. Schema upgrade interruption

Opening schema 2 with migrations 3 and 4 commits migration 3 and its history row.
Migration 4 fails: its changes/history roll back, store opening fails and ordinary
operations remain unavailable. The next compatible open validates committed
history and retries the still-pending migration. Neither enrollment nor pending
payloads are discarded. Opening schema 4 with software supporting only 3 fails
without running WAL-mode changes, migrations or application writes.

## 9. Opening, migrations, retention and errors

For an existing database, perform one read-only compatibility preflight using
SQLx migration history and the account row in one ordinary snapshot transaction.
Recognize unrelated, newer and failed/modified migration histories before opening
a write connection. Do not rely on `Migrator::run` alone to guarantee a read-only
rejection path. SQLite transactions and SQLx's per-migration transactions handle
concurrent access. Concurrent migration attempts can fail with ordinary database
or migration errors; retrying startup reads fresh history. There is no separate
store startup lock or automatic migration-retry mechanism. The desktop's
[process-lifetime coordination](desktop.md#5-one-process-per-workspace-and-activation)
serves single-instance activation, not database migration correctness; direct
store consumers retain ordinary SQLite concurrency behavior.

New-store creation initializes the schema and account on an exclusively created
file. SQLx migration history is the sole schema-version record; do not maintain
an application marker, separate schema-version table or `user_version` copy.
Upgrade reporting reads the actual migration history. Never rewrite applied migration
files. Add `*.sql text eol=lf` so Windows and Linux migration checksums agree.
A minimal Rust build script tracks additions to the migrations/query directories;
no Python or task dispatcher. Use SQLx's standard Rust CLI for offline metadata
preparation/checking against a temporary migrated SQLite database. Pin matching
library/tool versions during dependency verification. CI workflow changes include
the metadata paths in their trigger filters, retain PR/main-only triggering and
continue deriving Rust from `rust-toolchain.toml`.

Physical reclamation traces actual role references and owned children using
ordinary foreign keys/`NOT EXISTS` statements, not stored reference counters or a
background collection framework. Roots include current rows, pending originals/
prepared/witnesses, retained alternatives, staged/windows, operating baselines
and exact retained service support. Immutable service facts have no age-based
expiry in MVP. Removing an unused synchronization envelope does not authorize
removing its still-needed semantic evidence, deletion suppression or public
namespace. Do not equate local payload cleanup with server retention/actor
collection. Database compaction via manual vacuuming is not a normal-save step.

Use concrete `thiserror` errors for store categories such as incompatible schema,
account mismatch or invalid relational value;
preserve SQLx/native errors as causes through contextual `anyhow` propagation.
Expected business review/stale outcomes remain typed result data. Include table,
record and operation context without exposing payload values, secret-store
references, filesystem paths or raw bound SQL parameters in routine telemetry.
Full disk, read-only filesystem, locked database and corruption fail visibly;
never report a save or synchronization acknowledgment merely because planning
succeeded. Do not automatically delete/recreate a database after failed opening.

This design does not add backup/restore commands, directory copying as an online
backup procedure, machine/clone detection or per-edit audit history. Document
consistent backup/closed-copy procedures and protected credential requirements
under the existing [recovery contract](backup-recovery.md).

## 10. Dependency and observability boundary

Use SQLx SQLite/macros/migrate support and a standard Tokio runtime integration.
Prefer bundled SQLite for reproducible Windows/Linux capabilities, including
STRICT tables. SQLite itself is a C library; this selection does not introduce
OpenSSL, Python, TQSL execution or an external client database service. Confirm
selected versions, native compiler packaging, minimum Rust support and
license notices through native Windows and Linux verification.
Documentation capability checks are not proof of a successful build.
No server SQLx binding is selected by this client choice.

Follow [the observability contract](observability.md). Significant store opening,
completed schema upgrades and explicit saves/resolutions have `info` operation
spans/events with truthful attempted/committed wording. Routine reads/batch
planning detail uses `debug`; warn for incompatible/corrupt state needing
attention. The handling boundary records unexpected storage errors once.
Use `#[instrument(skip_all, ...)]`, default targets and direct safe fields:
account/record/window IDs where useful, schema versions, batch counts, generation
and bounded outcomes. Never dump QSO values, SQL arguments, directory credentials
or protected-store handles. No generic completion observer, lifecycle wrapper,
manual span timer or redundant operation/component fields.

Measure committed local QSO mutations by create/update/delete action only after
commit; installing shared values is a different quantity and is not a local save.
Measure installed sync records and staged input at their actual commit boundaries,
not at plan production. `clacks_client_sync_records_staged_total` counts envelopes
newly added to a window's durable staging set, with no labels. Reprocessing or
retransmitting an unchanged staged envelope does not increment it; completing
the unit does not decrement it. A failed transaction contributes nothing. The
counter measures committed staging transitions, not the current staging backlog
or unique records across windows.
Runtime boundaries own workflow latency and failure
metrics. Use separate coherent families and bounded action/outcome/error-class
labels, never account IDs or SQL text. Durable rows/checkpoints are authoritative;
telemetry can be lost after commit and is not an exactly-once accounting ledger.
Tests check the reporting decisions made by actual store operations, including
no successful-save metric/event on rollback and sufficient error context when
lower-level spans are filtered.

## 11. Acceptance and verification

Exercise normal store operations against the production schema. Do not inject
triggers, drop tables to manufacture failures, or add test-only production
interfaces. Use real inputs and ordinary contention/interruption conditions.
Malformed-database rejection and authored migration transformations use explicit
input fixtures for those contracts.

Required verification for the store:

1. Round-trip every native payload variant, all original/observation variants,
   complete vectors/coverage/lineage/witnesses, unknown labels, optional aggregates,
   unordered sets and ordered mapping rules. Verify frequency and transmit-power
   values at 1 and the signed 64-bit maximum, optional absence, and rejection of
   zero and values above that maximum. Verify counter zero/nonzero rules, the
   signed maximum and checked exhaustion. Verify QRZ decimal-ID round trips,
   canonical equality/uniqueness, invalid decimal rejection and IDs beyond signed
   and unsigned 64-bit limits. Check Rust pagination comparisons and carry when
   adding one, without SQL numeric casts or lexical-order mistakes. No panics,
   range truncation or precision loss. Reject cross-account/record ownership,
   missing live children and invalid discriminator
   combinations rather than silently hydrating incomplete native values.
2. Reopen real temporary databases after local saves/deletions/profile loads and
   frozen preparations. History returns one working QSO, not one per tag/role.
   For operating-state delivery under D285, verify independent station/equipment
   detachment: Apply persists edited values with the affected source/baseline
   cleared, while Cancel retains prior active values and selections. A separate
   shared-record save survives Cancel without reattaching operating values.
3. Exercise stale local saves and stale resolution inputs with an intervening
   committed update. Assert unchanged current state and an explicit stale outcome
   without requiring additional generation columns. Exercise missing/deleted
   references, exact prepared retries,
   accepted-own responses with later edits and incomparable retained alternatives
   through the production core and actual transactions.
4. Kill a Rust child process before/after transaction commits and between batches;
   reopen and verify complete state or prior state. Use test-only coordination
   outside public APIs. These tests prove process-interruption behavior, not
   laboratory power-loss guarantees; SQLite/OS/filesystem durability remains an
   explicit dependency.
5. Exercise staging/complete coupled units, support dependencies, immutable
   manifest replacement, checkpoint completion and lost acknowledgment. Assert
   no false installed prefix after partial transfer or rollback. Verify exact
   external binding uniqueness across occupied, superseded and deleted-QSO
   states; pending alternatives must not replace the current accepted binding.
6. Install baseline projection while retaining a pending original/witness, retire
   an actor, then apply an older-server recovery scenario without rewriting stamps
   or losing the genuine pending tail. Server authorization is supplied evidence,
   not fabricated by the client store.
7. Test multi-connection reader/writer coexistence, writer contention, stale-save
   protection and cancellation. Long readers retain a consistent snapshot;
   dropping a cancelled operation releases its transaction before reuse.
8. Verify that Clacks-authored migrations preserve application values and apply
   the intended domain changes. Test read-only opening rejection for mismatched
   accounts and unrelated files. SQLx owns migration-history validation,
   journaling, per-migration transactions and resuming after failure;
   do not duplicate its machinery tests.
9. Build/check offline query metadata from all migrations and validate it in CI;
   demonstrate ordinary offline builds without `DATABASE_URL`. Check realistic
   query plans and bound dynamic filter values.
10. Run workspace formatting, tests, documentation and Clippy under Rust 1.99.1;
    real Windows and Linux CI must pass. Rust-only fixtures/process tooling,
    reference-only TQSL and existing native/sync regression tests remain required.

## 12. Implementation boundaries

The table families, native representations, transaction obligations and acceptance
cases above define the capability. The production schema and query files live in
`lib/clacks-client-store/migrations/` and `lib/clacks-client-store/sql/`; the
[store guide](../lib/clacks-client-store/README.md) describes the APIs, integration
boundaries and verification commands. Changes to those implementations must
preserve this specification and the existing native/synchronization contracts.

The small-pool/timeout defaults, STRICT tables and bundled-SQLite direction are
engineering choices for this specification, not new product requirements.
Their compatibility and operation require native platform verification.
The concrete stale-save precondition can use existing values/version references
and standard UI state-management facilities; additional generation counters are
not a specification requirement. Existing synchronization generations and counter
reservations remain required for their original purposes.

Implementation does not add a user-facing SQL console, account-switcher UI,
automatic schema downgrade, built-in backup/restore command or temporary store
that later capabilities replace.

## Primary implementation references

- [SQLite STRICT tables](https://www.sqlite.org/stricttables.html): storage types and enforcement.
- [SQLite foreign keys](https://www.sqlite.org/foreignkeys.html): connection enforcement and composite relationships.
- [SQLite isolation](https://sqlite.org/isolation.html): snapshots and immediate write acquisition.
- [SQLx query files](https://docs.rs/sqlx/0.9.0/sqlx/macro.query_file.html) and
  [typed query files](https://docs.rs/sqlx/0.9.0/sqlx/macro.query_file_as.html): file-relative inclusion and checked result mapping.
- [SQLx embedded migrations](https://docs.rs/sqlx/0.9.0/sqlx/macro.migrate.html): embedding, line endings and stable-Rust change tracking.

SQLx 0.9.0 documentation establishes these available interfaces; it is not a claim
that that version has been selected or verified against the workspace.

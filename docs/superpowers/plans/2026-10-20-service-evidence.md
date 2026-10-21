# Service Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for native execution, or superpowers:subagent-driven-development if the user selects that method. Implement task-by-task after plan review and execution authorization. Steps use checkbox (`- [ ]`) syntax for tracking. Read the specification as well as this plan.

**Goal:** Deliver the production service-evidence, applicability, observation and association operations for [issue #3](https://github.com/jhelwig/clacks-log/issues/3), integrated with the existing synchronization library.

**Architecture:** Put service domain values and pure projections in `clacks-model::service`; put accepted-context interpretation, guards, coupled changes and recovery in `clacks-sync::service`. Extend existing acceptance/transfer/compaction operations rather than create a parallel synchronization implementation. Later database/provider integrations commit these same plans and consume the same normalized values.

**Tech Stack:** Rust 1.99.1, edition 2024, standard collections, existing workspace Serde serialization, thiserror and anyhow. Add `time` 0.3 for checked calendar conversion and `deunicode` 1 for best-effort text transliteration, inherited from the workspace. No cryptography, networking or database dependency is needed.

**Spec:** [Service evidence and external-record associations](../../service-evidence.md), supported by the [synchronization model](../../synchronization-model.md) and [native model](../../native-model.md). The request to write this plan advances the consolidated specification to planning; it does not authorize execution.

Logbook of The World is LoTW. Amateur Data Interchange Format is ADIF; ADI is
its text format. Coordinated Universal Time is UTC. DX Century Club is DXCC.
Universally Unique Lexicographically Sortable Identifier is abbreviated ULID.
Application programming interface is API; XML means Extensible Markup Language.
Literal protocol field names and mode labels below retain their source spelling.

## Global constraints

- Rust **1.99.1**, edition **2024**, workspace resolver **3**, Elastic License **2.0**. Member package metadata and dependencies use `workspace = true`.
- Libraries in `lib/<crate-name>` and thin applications in `bin/<application-name>`. This issue extends two existing libraries; no application or empty future signing crate.
- One account synchronization scope; authorized client ↔ server only. Preserve the existing vectors, timelines, baselines and oldest-authorized acknowledgment boundary.
- Keep provider facts separate from recorded QSO facts. Do not add automatic QSO field/tag merging or a server store of unresolved operator proposals.
- Distinct semantic evidence has no age-based expiry in MVP. Unchanged polls add no semantic history; do not add a permanent poll predecessor chain.
- IDs identify records, not chronology. Use checked arithmetic and exact integer conversions; QSO times remain whole seconds, frequency whole hertz, power milliwatts, native coordinates millionths of a degree.
- Standard Serde representations; preserve unfamiliar labels and frozen historical interpretation. Recognition changes do not edit recorded facts.
- No production `expect()`/`unwrap()`, unchecked indexing, facade `pub use` or public test helpers. No `mod.rs` outside tests. Separate Rust items with blank lines.
- Document constants, fields, variants, types and functions. Document each behavior on its owning item, not a remote wrapper or the development process.
- Leaf diagnostics derive `thiserror::Error`; composite operations return `anyhow::Result`, retaining concrete sources. Readiness, stale guards, missing support and unresolved review are distinguishable outcomes.
- No live provider requests, certificate/private-key handling, database/schema work, transport, scheduling/backoff implementation or arbitrary diagnostic expiry. Never include credentials/private keys in synchronized evidence.
- Real stores must enforce atomic groups and uniqueness. Test production transitions and serialized row sets; do not build a fake database/network or claim database crash atomicity.
- Commit messages have a substantive body explaining motivation, constraints and verification. Planning does not close issue #3.

## Review focus

1. Valid native values outside provider limits, including extreme timestamps and unknown bands/modes, must produce contextual readiness diagnostics without changing the QSO: Tasks 2–4.
2. Two configuration entries for one remote collection, and two simultaneous bindings for one external key, must not create independent uniqueness namespaces: Tasks 1, 7–8.
3. A newly requested field, explicit negative and late response are different from an omitted field or newer provider revision: Tasks 5–6, 9.
4. A disposition received before its effects, or after their successors, must neither certify incomplete progress nor resurrect old values: Tasks 7, 10.
5. Revoked authors, unavailable ordering context and compaction must preserve accepted evidence without granting new admission or requiring every offline client to return: Tasks 10–11.

## File structure and sequence

Paths below are repository-relative. Files are introduced with their first real
behavior, not as empty scaffolding. New test fixtures/helpers live only in the
existing `tests/support/` directories.

| Files | Responsibility |
| --- | --- |
| `lib/clacks-model/src/ids.rs` | Distinct service IDs using the existing macro. |
| `lib/clacks-model/src/service.rs` | Owning modules only. |
| `lib/clacks-model/src/service/destination.rs` | Public destinations, remote collection identity, external keys. |
| `lib/clacks-model/src/service/adif.rs` | Shared semantic field conversion, canonical modes, precision and text diagnostics. |
| `lib/clacks-model/src/service/error.rs` | Contextual malformed-data and projection-conversion errors. |
| `lib/clacks-model/src/service/lotw.rs` | LoTW mapping data, projection, key and service-specific confirmation basis. |
| `lib/clacks-model/data/lotw-11.34.json` | Derived pinned mapping/band/station metadata, with provenance in `data/README.md`. |
| `scripts/extract-lotw-data.py` | Reproducible maintenance extraction from the pinned source; not a runtime dependency. |
| `lib/clacks-model/src/service/qrz.rs` | QRZ ordinary-ADIF projection, key and basic-confirmation basis. |
| `lib/clacks-model/src/service/evidence.rs` and `evidence/{submission,observation,processing}.rs` | Domain record kinds, imported baselines, results, reports/coverage and dispositions. |
| `lib/clacks-model/src/service/applicability.rs` | Pure publication coverage and confirmation/source eligibility. |
| `lib/clacks-sync/src/service.rs` | Owning modules and the synchronization-side service carrier. |
| `lib/clacks-sync/src/service/{dependency,combination,association,operation,download,recovery,retention}.rs` | Required support, immutable combination, unique binding, coupled changes, acquisition/progress, recovery and collection protection. |
| Existing `record.rs`, `acceptance.rs`, `transfer.rs`, `client.rs`, `recovery.rs`, `compaction.rs` in `clacks-sync` | Integrate new domains with existing production paths. |
| New model `tests/service_{destinations,adif,lotw,qrz,evidence,applicability,observations}.rs` | Direct domain/projection tests. |
| New sync `tests/service_{admission,operations,downloads,transfer,recovery,retention,scenarios}.rs` | Direct guarded transition/integration tests. |
| Crate READMEs and maintained contracts | Usage, adapter obligations and accurate implementation boundaries. |

Execute **Tasks 1–11 in order**. The tasks are reviewable increments of this
single capability, not separate replacement models. No Rust source or dependency
changes are made while merely preparing this plan.

## Interface decisions

The names and signatures below are implementation decisions for this plan.
They must agree between tasks. Composite durable rows derive ordinary Serde
and equality; leaf validation runs during construction/deserialization and
composite validation runs at transition boundaries. Borrow inputs and return
owned proposed changes. Do not expose aliases or constructors solely for tests.

### Domain vocabulary

- Add `DestinationId`, `AssertionId`, `AttemptId`, `SubmissionResultId`,
  `ObservationId`, `AcquisitionId`, `AssociationId`, `DispositionId`,
  `SuppressionId`, `DownloadStateId`, `OperationId` and `SubmissionBatchId`.
- `Service::{Lotw, Qrz}`; `Destination { id, collection, description }`.
  `RemoteCollection::{Lotw { account: String }, Qrz { book_id: Option<u64> }}`.
  A configured LoTW account identifier is public namespace information, not a
  password. Descriptions and routing metadata do not establish identity.
- `ExternalRecordKey { destination: DestinationId, record: RemoteRecordId }`;
  `RemoteRecordId::{Lotw(LotwRecordKey), Qrz(u64)}`. `LotwRecordKey` contains
  both calls, start, primary/effective receive band and mapped mode, not frequency
  or an award mode group.
- `AdifFields = BTreeMap<String, String>` stores canonical emitted semantic values,
  not serialized ADI bytes. Field presence is meaningful; canonical list fields
  use deduplicated deterministic ordering. Typed native values supply conversions.
- `ProjectionResult<T> { value: T, warnings: Vec<ProjectionWarning> }`;
  `ProjectionDecision<T>::{Ready(ProjectionResult<T>), NotReady(Vec<ReadinessIssue>)}`.
  `ReadinessIssue` identifies service, QSO/destination and field/reason as applicable.
  Put `ProjectionDecision`, `ProjectionResult`, `ProjectionWarning`, `ReadinessIssue`
  and `ServiceError` in `service::error`; do not re-export them from `service.rs`.
  `ServiceError` derives `thiserror::Error` for malformed configuration/identity,
  unrepresentable calendar values and cross-service inputs. Conversion errors keep
  source chains. Ordinary publication readiness is `NotReady`, not an error.
- `LotwSubmission { destination, signing_identity, contact_fields, station_fields }`
  contains one normalized signing input. `PublicSigningIdentity` holds own call
  and DXCC entity, with no certificate-library type. `QrzSubmission { destination,
  fields }`. `SubmissionProjection::{Lotw(LotwSubmission), Qrz(QrzSubmission)}`.
  Mapping/provenance metadata accompanies but is excluded from payload equality.
- `ProjectionContext<'a>` borrows `RecognitionCatalog`, `DefinitionSet`,
  `ProfileSet`, ordinary `ExportContext`, and any per-contact `ContextResolution`.
  `QrzProjectionInput<'a>` adds QSO, destination and `EligibleStationFacts`.
  `LotwProjectionInput<'a>` adds QSO, destination, public signing identity and
  `LotwConfiguration`. Neither input changes native facts.

### Reported facts and evidence

- `Reported<T>::{NotReported, Value(T), Clear}`. A negative confirmation is
  `Value(Unconfirmed)`, not `Clear`. `Clear` must be supported explicitly by the
  supplied provider-field semantics; absence never grants clear permission.
- `ContactReport` contains reported calls, start, primary/receive bands,
  frequencies, concrete mode/submode/group and confirmation information.
  `StationReport` contains separately reported name, position, entity,
  subdivisions, zones and references. `RemoteFacts` separates our contact/station
  report from an optional actual counterpart report. Unknown provider fields
  remain labeled source diagnostics, not invented typed facts.
- `ObservedStart::{Second(UtcSecond), Minute(UtcSecond)}` records supplied time
  coverage only for incoming reports; minute starts must be minute-aligned.
  This does not add precision metadata to saved QSOs.
- `ProviderOrdering` retains separately scoped LoTW QSO/confirmation update
  timestamps or explicitly available QRZ ordering evidence. It has no universal
  numeric revision comparator. `QueryIdentity` contains destination, stream,
  selected fields and public filters. `AcquisitionProvenance` retains its ID,
  query identity and observed time; observed time is not provider ordering.
- `RemoteObservation { id, destination, identity, facts, ordering, acquisition }`;
  `identity` is complete or explicitly incomplete, preserving missing dimensions.
  `ObservationComparison::{Unchanged, Additional, Changed, OrderingOnly, Older,
  Unordered}` returns the changed/unknown field paths with the classification.
  `EstablishedObservation { identity, facts, ordering }` is the derived current
  report view supplied to comparison. Classification carries per-component
  differences/order evidence: newer LoTW QSO information and older confirmation
  information in one response cannot be collapsed into one winning timestamp.
- `ImportedAssertion` retains QSO, service, known destination, claim/date/source,
  `ImportedBaseline` and source coverage. `ImportedBaseline` contains source-only
  service inputs/projected values and `ConfirmationBasis`, not a full permanent
  QSO body or fictitious sent bytes. Missing input remains explicit.
- `SubmissionAttempt` retains ID, QSO, destination, frozen projection,
  intent/possible-send/dispatched phase, time and optional batch correlation.
  Phase advancement is additional evidence, not rewriting immutable attempt
  content. `SubmissionResult` has its own ID, attempt/batch correlation,
  `ResultScope`, `SubmissionOutcome` and credential-free diagnostics.
- `ResultScope::{UploadBatch, IndividualAttempt}` references exact member IDs;
  `SubmissionOutcome::{Success, Rejected, Uncertain}` preserves phase/detail.
  A batch-level success covers its actual dispatched members at upload scope,
  without manufacturing individual processing acceptance.
- `ExternalAssociation { id, external, qso, state }`;
  `AssociationState::{Active, Superseded, DeletedQso}`. `DeletionSuppression`
  retains the known external identity/binding and deleted QSO identity.
- `ProcessingDisposition { id, observation, decision, association }`;
  `ProcessingDecision::{PendingReview, Associated, Added, Rejected, Suppressed,
  CompletedEquivalent}`. Accepted effect references live in synchronization
  support. `CompletedEquivalent` records reuse of already represented effects
  when only ordering support advances, without repeating a business action.
- `ServiceRecordId` is a copyable enum wrapping the corresponding distinct IDs.
  `ServicePayload` owns destination/evidence/association/disposition/suppression/
  download-control payload variants. `id()` gives `ServiceRecordId`; validation
  checks embedded identities and destination/service compatibility.
  `ImportedBaselineInput<'a>` borrows QSO, source coverage/provenance, service,
  optional known destination and projection configuration; `PublicationCoverageInput`
  borrows current service inputs/projection plus already selected assertions and
  attempt/results; `ConfirmationInput` borrows current identity, retained basis,
  selected confirmation/counterpart evidence and explicit ordering/applicability
  information. None of these inputs borrows credentials or chooses between
  causally unordered observations itself.

### Synchronization vocabulary

- Extend `RecordId` with `Service(ServiceRecordId)` and `RecordValue` with
  `Service(Box<ServiceRecord>)`; retain native variants and their serialization.
  `ServiceRecord { payload: ServicePayload, support: ServiceSupport }` lives in
  `clacks-sync::service`, not the model crate.
- `VersionReference { key: RecordKey, stamp: VersionStamp, deleted: bool }` and
  `SupportRequirement::{ExactFact(VersionReference), CoveredEffect(VersionReference)}`
  distinguish immutable support from effects representable by successors/deletions.
  `ServiceSupport { required, operation: Option<OperationId>, acquisition: Option<VersionStamp> }`
  accompanies payloads. Acquisition context is the original accepted query-start
  stamp, not the response installation stamp; retain its interpretation without
  requiring a permanent live row for every completed poll.
  Compaction may re-express equivalent causal metadata; it does not mutate an
  immutable domain fact or assign a new observation ID.
- `ExternalBindingObservation { external, binding }` supplies a unique-index read
  as absent or an association/version. `ExternalBindingGuard` captures that read.
  Guard comparisons are account-scoped. A phantom insert must be excluded by the
  real transaction/index; an empty supplied association list is not proof of absence.
- Extend `AcceptanceInput` with `bindings: &[ExternalBindingObservation]` and
  `AcceptanceCommit` with `external_guards: Vec<ExternalBindingGuard>`. Native-only
  callers supply `&[]`; service operations cannot omit required binding evidence.
- `ServiceView<'a>` borrows accepted records, binding observations and existing
  `EvidenceView`; it is a real read-view projection, not a database abstraction.
  Missing dependencies return `DependencyStatus::Incomplete` listing requirements.
  Destination requirements retain their historical public namespace description
  inline with the support. A compatible current description can supply the same
  interpretation; changing display/routing configuration must not require an
  old enabled configuration or complete old QSO body. `ExactFact` applies to
  immutable semantic support, not byte equality of projected vectors.
- `CoupledRequirement { id: OperationId, members: Vec<VersionReference> }` describes
  an atomic operation's accepted versions. Save it with the commit and retain it
  while interpretation/transfer requires it. It is synchronization metadata, not
  a new causal graph or a domain event. It does not reference itself.
- `CoupledCommit` holds original head/membership conditions, deduplicated target/
  reference/binding guards, accepted envelopes, journal rows, the final account,
  and its `CoupledRequirement`. It cannot certify persistence.
- `ServiceDecision<T>::{Ready(T), Unchanged, Incomplete(...), Stale(...), Review(...)}`
  expresses expected transition results. Errors denote malformed input, integrity,
  overflow and cross-account violations, with identifying context.
  Define this enum in `clacks-sync::service::operation`; detailed conditions use
  existing version observations and explicit dependency/binding reasons. Put
  `SyncServiceError` in `service::dependency`, deriving `thiserror::Error` with
  account, service-record key and missing/unequal support context. Use anyhow to
  preserve existing native/metadata causes rather than wrapper-only variants.

## Task 1: Destination namespaces and production service identities

**Files:** Create `lib/clacks-model/src/service.rs`, `service/destination.rs`
and `service/error.rs` with initial identity diagnostics; modify `src/lib.rs`, `src/ids.rs`; test `tests/service_destinations.rs`.

**Interfaces:** Produce the IDs, `Service`, `Destination`, `RemoteCollection`,
`ExternalRecordKey` and `RemoteRecordId` above. `destination::validate_destinations(
destinations: &[Destination]) -> anyhow::Result<()>` validates a supplied account's
public collection registry. Add `LotwRecordKey` here using normalized strings
for concrete service bands/mode; task 3 constructs it.

- [ ] Write tests `distinct_credentials_do_not_create_destinations`,
  `duplicate_remote_collection_is_rejected`, `unknown_book_id_is_not_inferred`,
  `remote_ids_are_destination_scoped`, and `service_ids_round_trip`.
  Assert that changing description leaves keys equal; two established QRZ
  `bookid=42` entries cannot create separate namespaces; unknown book IDs do not
  establish equivalence or difference from a known book; LoTW certificate identity
  is absent from destination/key structures. Duplicate Clacks IDs with unequal
  descriptors are errors. Unknown QRZ collections require explicit configuration
  reconciliation before treating them as separate established destinations.

  ```rust
  assert!(validate_destinations(&duplicate_book_42).is_err());
  assert_eq!(key_before_credential_change, key_after_credential_change);
  ```
- [ ] Run `cargo +1.99.1 test -p clacks-model --test service_destinations --locked`;
  expect failure because production types/operations are missing.
- [ ] Implement validation and identities using existing `ids.rs` conventions.
  Do not derive causal ordering from IDs or use display names/callsign-date routing
  to invent a QRZ book ID. Keep secrets out by construction.
- [ ] Run the same target; expect all tests pass, including serialized reload.
- [ ] Commit with a body explaining collection identity versus replaceable access.

## Task 2: Reusable ADIF semantic conversions

**Files:** Create `service/adif.rs`, `tests/service_adif.rs`; modify workspace
`Cargo.toml`, model manifest/lockfile and owning module declarations.
Extend `service/error.rs` with the shared projection/error vocabulary.

**Interfaces:** Produce `AdifFields`, projection result/diagnostic types and
`ProjectionContext`. Public functions in `service::adif`:

```rust
pub fn contact_time(value: UtcSecond) -> anyhow::Result<(String, String)>;
pub fn frequency(value: Hertz) -> String;
pub fn power(value: Milliwatts) -> String;
pub fn coordinates(value: Coordinates) -> (String, String);
pub fn mode(value: &Mode, catalog: &RecognitionCatalog) -> AdifFields;
pub fn text(field: &str, value: &str) -> ProjectionResult<Option<String>>;
```

- [ ] Write tests asserting `14_074_001 Hz -> "14.074001"`,
  `1_001 mW -> "1.001"`, UTC `2026-10-20 23:59:59` and next-day zero seconds
  produce separate correct dates, and `UtcSecond(i64::MAX)` returns a concrete
  calendar-range error without panic. Whole hertz/watts never use floats.
  Test native coordinates `-1` microdegree latitude rounding to `S000 00.000`,
  minute overflow at `12_999_999` microdegrees producing `N013 00.000`, poles/
  dateline, and values differing below emitted precision producing equal strings.
  Test `MFSK/FT4` emits both parent/submode; unknown recorded parent/specialization
  labels remain present. `text("NAME", "étude")` yields `etude` with a warning;
  unsupported-only optional text is omitted with diagnostics, original untouched.

  ```rust
  assert_eq!(frequency(Hertz::new(14_074_001)?), "14.074001");
  assert_eq!(power(Milliwatts::new(1_001)?), "1.001");
  assert!(contact_time(UtcSecond::from_unix_seconds(i64::MAX)).is_err());
  ```
- [ ] Run `cargo +1.99.1 test -p clacks-model --test service_adif`; expect failure.
- [ ] Add `time = { version = "0.3", features = ["formatting"] }` and
  `deunicode = "1"` to workspace dependencies, inherit them only in the model,
  and update the lockfile normally. Use checked UTC calendar conversion; reject
  years ADIF cannot express. Use checked wider integer intermediates for angles.
  Transliterate known characters, report loss/unknown characters, and omit a field
  whose conversion contains no meaningful surviving content. Do not substitute
  unknown-character placeholders as recorded text. A required field with no usable
  representation makes the submission not ready; optional omission cannot silently
  remove required identity. Use owning error annotations.
- [ ] Run the same target with `--locked`; expect pass, including maximum integers,
  signed-zero hemisphere policy and preserved native input equality.
- [ ] Commit with context that semantic precision is shared conversion behavior,
  not an ADI parser/writer. Dependency docs: [time](https://docs.rs/time/latest/time/struct.OffsetDateTime.html)
  and [deunicode](https://docs.rs/deunicode/latest/deunicode/).

## Task 3: Data-driven LoTW normalization and shared signing input

**Files:** Create `service/lotw.rs`, `data/lotw-11.34.json`,
`scripts/extract-lotw-data.py`, `tests/service_lotw.rs`; modify data provenance
README and service declarations.

**Interfaces:** `LotwConfiguration::from_json(&str) -> anyhow::Result<Self>`;
`LotwConfiguration::bundled() -> anyhow::Result<Self>`;
`lotw::normalize_mode(mode: &Mode, config: &LotwConfiguration) -> anyhow::Result<String>`;
`lotw::project(input: &LotwProjectionInput<'_>) -> anyhow::Result<ProjectionDecision<LotwSubmission>>`;
`LotwSubmission::external_key(&self) -> ExternalRecordKey`.
Produce `PublicSigningIdentity`, `LotwSubmission`, `LotwProjectionInput`.

- [ ] Write tests for the full fallback chain, including combined MFSK/FT4 and
  bare FT4 → FT4; FT8 → FT8; SSB/USB/LSB → SSB; PSK/BPSK31/QPSK31 → PSK31;
  MFSK/JS8 and bare MFSK → DATA. Unknown unconfigured mode returns readiness.
  Assert key changes on effective RX-band change but not same-band frequency,
  reports/comment/equipment changes; a band-only RX override has no borrowed FREQ_RX.
  Assert absent own entity uses selected certificate public entity, contradictory
  supplied entity is not ready, and no private-key/certificate-library type exists.

  ```rust
  assert_eq!(normalize_mode(&mfsk_ft4, config)?, "FT4");
  assert_eq!(normalize_mode(&mfsk_js8, config)?, "DATA");
  assert_ne!(primary_rx_20m.external_key(), primary_20m_rx_40m.external_key());
  ```
- [ ] Run `cargo +1.99.1 test -p clacks-model --test service_lotw --locked`; expect failure.
- [ ] Extract complete conversion entries, supported modes/groups, service band
  ranges and station field/enumeration/entity bindings needed for represented MVP
  inputs from pinned config 11.34. Follow mapping-table construction in upstream
  `location.cpp`, including combined and bare labels; preserve range units/order
  and subdivision relationships, not merely the example rows. Keep upstream
  notices/provenance and reproducible output; leave upstream files unchanged.
  Python's standard XML library is sufficient for maintenance extraction.
- [ ] Implement validation, data loading and projection. Populate supported saved
  station fields through data bindings and explicit group mappings; do not infer
  POTA→NPOTA or satellite fields. Track every emitted station value in equality.
  Readiness issues identify field/entity and bad value. Mapped history is retained
  as values/provenance rather than recomputed from future catalogs.
- [ ] Run the target and extractor `--check`; expect pass and byte-identical derived
  data. Test malformed/duplicate mapping rows, out-of-service frequency, historical
  grid lengths and changed mapping data without rewriting an earlier submission.
- [ ] Commit explaining that comparison, remote key and future signing consume
  this one value, without introducing a signing dependency.

## Task 4: QRZ projection using existing ordinary reference mappings

**Files:** Create `service/qrz.rs`, `tests/service_qrz.rs`, `service/evidence.rs`
and `service/evidence/submission.rs` for the shared submission enum; modify declarations.

**Interfaces:** `qrz::project(input: &QrzProjectionInput<'_>) -> anyhow::Result<ProjectionDecision<QrzSubmission>>`;
produce `QrzSubmission`, `QrzProjectionInput`, and `SubmissionProjection` in
`evidence/submission.rs` when first needed. Consume Tasks 1–3 and existing
`effective::{select_location, select_name}` and
`export::evaluate::plan_reference_export`, not a second reference engine.

- [ ] Write tests for every field family in spec §5.2. Assert complete MODE/SUBMODE,
  ten/twelve-character grid splits, point versus area output, whole-location
  fallback, recorded location-bearing reference suppression, whole RX overrides,
  selected names, reports, end date, power, equipment and comments.
  Assert reordered sets/equal emitted coordinates compare equal; changed COMMENT
  or MY_RIG changes QRZ projection but not LoTW's. Assert forbidden metadata/status
  fields never enter output and tag changes alone leave output equal.

  ```rust
  assert_eq!(projected.fields.get("MODE").map(String::as_str), Some("MFSK"));
  assert_eq!(projected.fields.get("SUBMODE").map(String::as_str), Some("FT4"));
  assert!(!projected.fields.contains_key("APP_QRZLOG_LOGID"));
  ```
- [ ] Run `cargo +1.99.1 test -p clacks-model --test service_qrz --locked`; expect failure.
- [ ] Implement projection with common conversions and the full specified emission
  set. Require ordinary interchange context, surface existing export decisions
  as readiness choices, and preserve export warnings. No targeted/POTA expansion.
  Optional omission remains a payload difference without claiming remote clearing.
  Never mutate QSO/directory values while computing fallback.
- [ ] Run target; expect pass. Include several contest groups targeting CONTEST_ID:
  unresolved ambiguity is not ready, explicit existing omission resolution reports
  the omitted field, and unrelated references do not expand native QSOs.
- [ ] Commit explaining service-specific payload comparison and remote-clear limits.

## Task 5: Evidence payloads, imported baselines and observation coverage

**Files:** Extend `service/evidence.rs` and `evidence/submission.rs`; create
`evidence/{observation,processing}.rs`, `tests/service_evidence.rs`,
`tests/service_observations.rs`; modify service declarations.

**Interfaces:** Produce remaining report/evidence/payload types from the vocabulary,
including `ConfirmationBasis` in `evidence/submission.rs`. Add download-control
payload/ID enum variants in Task 9 when their actual operations are implemented.
`evidence::imported_baseline(input: &ImportedBaselineInput<'_>) -> anyhow::Result<ImportedBaseline>`
consumes a prepared valid imported QSO, source coverage, service, optional known
destination and explicit mapping data. It has no directory/fresh-certificate input.
`ServicePayload::validate(&self) -> anyhow::Result<()>` validates domain shape.
`observation::compare(current: &EstablishedObservation, incoming: &RemoteObservation)
-> anyhow::Result<ObservationComparison>` compares normalized reported information.

- [ ] Write independent imported-upload and imported-confirmation tests: retain
  dates/source/baselines; neither creates the other claim, an attempt, sent bytes,
  remote key or counterpart report. Absent/unsent status creates no positive claim.
  Destination absent remains absent. Preserve source-unknown fields distinct from
  an explicit absent/clear input. Assert duplicate-ID unequal immutable data fails
  validation/admission later rather than being silently normalized to equality.

  ```rust
  assert_eq!(imported_upload.destination, None);
  assert_eq!(imported_upload.qso, qso.id);
  assert_eq!(imported_upload.baseline, retained_baseline);
  ```
- [ ] Run `cargo +1.99.1 test -p clacks-model --test service_evidence --test service_observations --locked`;
  expect missing production operations.
- [ ] Implement typed payloads and source-only baselines. Retain relevant recorded
  inputs separately from mapped output when unavailable service context prevents
  a complete projection; this preserves immediate imported trust without inventing
  certificate entity/fallback. Unknown context alone is not a demand to verify an
  unchanged imported QSO. Known subsequent service-input/effective-output changes
  remain detectable. Preserve batch-member/result scope and immutable phase facts.
- [ ] Implement field coverage comparison against established current information,
  not a search for any historical equal content. Equality excludes fetch/wire data;
  semantic coverage/provider-order evidence is retained separately. Report path
  differences without treating missing fields as replacement values. Incomplete
  identity remains retainable and cannot provide a binding key.
- [ ] Run targets; expect pass. Include A → A, A → B → A, positive → omitted,
  positive → explicit negative, additional newly requested field, ordering-only
  advance, contradictory same-scoped timestamp and unfamiliar labels. Serialization
  preserves minute-only incoming coverage without altering QSO timestamp types.
- [ ] Commit explaining why stable occurrence identity is distinct from content.

## Task 6: Pure publication coverage and confirmation applicability

**Files:** Create `service/applicability.rs`, `tests/service_applicability.rs`;
extend LoTW/QRZ owning modules for their comparison bases.

**Interfaces:** Consume Task 5 `ConfirmationBasis`; produce `PublicationCoverage::{Covered,
Pending, NotEstablished}`, `ConfirmationApplicability::{Confirmed, Unconfirmed,
NotEstablished, HistoricalOnly}` and separate exact-mode qualification.
`publication_coverage(input: &PublicationCoverageInput<'_>) -> anyhow::Result<PublicationCoverage>`;
`confirmation_applicability(input: &ConfirmationInput<'_>) -> anyhow::Result<ConfirmationApplicability>`.
Inputs borrow current service-relevant recorded inputs/projection and selected
assertions/results/observations; they do not perform causal selection themselves.
`eligible_station_facts(input: &ConfirmationInput<'_>) -> anyhow::Result<Option<StationFacts>>`
exposes only applicable actual counterpart information.

- [ ] Write tests with explicit assertions:

  ```text
  imported positive upload + unchanged baseline -> Covered without verification
  missing/unsent upload + positive confirmation -> Pending upload, Confirmed contact
  A1/FN31 succeeds after current own FN32 -> Pending, retain A1
  current COMMENT change -> LoTW Covered; QRZ Pending
  own 12:00 / hidden other 12:29, corrected own 11:58 -> NotEstablished
  callsign or relevant band correction -> HistoricalOnly
  QRZ same parent/submode edit -> basic Confirmed
  LoTW same group/different mode -> basic applicable, exact qualification not inherited
  missing negative field -> preserve established status; newer explicit negative -> Unconfirmed
  ```

  ```rust
  assert_eq!(publication_coverage(&missing_upload)?, PublicationCoverage::Pending);
  assert_eq!(confirmation_applicability(&corrected_1158)?,
      ConfirmationApplicability::NotEstablished);
  ```

- [ ] Run `cargo +1.99.1 test -p clacks-model --test service_applicability --locked`; expect failure.
- [ ] Implement equality after actual emitted precision; compare imported baselines
  using their actual source coverage without filling gaps from current provider
  data. A service-level imported claim can cover unchanged values before its
  destination is established; it does not assert upload to another known book.
  Only actual service-relevant changes make it pending. Preserve supported success
  scope when deriving upload coverage, independently of confirmation status.
- [ ] Implement the conservative time rule and source eligibility. An actual
  counterpart or fresh provider result may establish changed-time applicability;
  local time difference under 30 minutes alone cannot. Different/unknown geographic
  evidence does not inherit award qualification. Own-returned QRZ facts and QRZ
  LoTW-derived credit do not fabricate a LoTW/counterpart observation.
- [ ] Run target; expect pass, including explicitly unknown required comparisons,
  RX-band changes and imported baselines with unavailable optional service context.
- [ ] Commit explaining applicability versus historical publication coverage.

## Task 7: Admit and combine service records in existing synchronization

**Files:** Create sync `service.rs`, `service/dependency.rs`, `service/combination.rs`,
`tests/service_admission.rs`; modify `record.rs`, `acceptance.rs`, `client.rs`,
`lib.rs` and existing input fixtures/callers.

**Interfaces:** Produce `ServiceRecord`, support/guard/view types and record enum
extensions from the vocabulary. `dependency::check(record: &ServiceRecord,
view: &ServiceView<'_>) -> anyhow::Result<DependencyStatus>`;
`combination::combine(existing: &AcceptedRecord, incoming: &AcceptedRecord,
view: &ServiceView<'_>) -> anyhow::Result<ServiceCombination>`.
`combination::select_observations(input: &ObservationSelectionInput<'_>)
-> anyhow::Result<SelectedObservations>` consumes destination/external key,
accepted observation carriers and the same `ServiceView`. It returns
`SelectedObservations { current: EstablishedObservation, supporting: Vec<ObservationId>,
uncertain: Vec<String> }`, with unknown field paths retained explicitly.
`ServiceCombination::{Identical, ContextJoin, KeepBoth, ReplaceMaintained,
ReviewBinding, Incomplete}` carries required contexts/reasons, not network actions.
Integrate these checks into existing public acceptance/receipt boundaries so the
generic paths cannot bypass evidence integrity or required uniqueness.

- [ ] Write tests for typed enum round trips and `RecordState::validate`, same-ID
  unequal assertion/attempt/observation integrity errors, equal distinct attempts,
  result-before-attempt retention, batch membership, cross-account references,
  missing destination description, and required effect successor/deletion coverage.
  Test ordinary `plan_server_acceptance` rejects unsafe service mutations too;
  calling a specialized wrapper is not the only protection.

  ```rust
  assert!(combine(&o1, &same_id_different_facts, &view).is_err());
  assert!(matches!(check(&unsupported_result, &view)?, DependencyStatus::Incomplete(_)));
  ```
- [ ] Run `cargo +1.99.1 test -p clacks-sync --test service_admission --locked`; expect failure.
- [ ] Extend the real record enums and validators. Resolve model payload IDs through
  their owning modules. Check required support by kind, not caller-declared support
  alone: omitted required dependencies cannot bypass validation. Missing material
  can be staged durably but is not an applied record/completed effect.
- [ ] Add binding reads/guards to existing acceptance structures and update native
  callers to empty bindings. Introduce service receipt dispatch before the native
  business-conflict classifier. Immutable facts compare content before any
  same-version shortcut; identical facts can combine context, unequal same IDs
  cannot be field-resolved. Distinct observation IDs remain separate facts.
  Maintained associations retain existing client resolution when unordered.
  Implement field-scoped observation selection across distinct retained IDs.
  Use original acquisition context and provider timestamp scopes with existing
  lineage interpretation; a later acceptance of an earlier query response is
  not automatically later provider state. Return uncertainty for unsupported
  ordering instead of picking by wall time, arrival or ID. Feed this production
  selection into Task 6 applicability and Task 9 current-observation comparison.
- [ ] Run target plus `cargo +1.99.1 test -p clacks-sync --test acceptance --test client_state --locked`;
  expect all pass. Provider contradictions produce refresh/unknown selection,
  not a manual QSO-field review. Retained support cannot be replaced by a live
  credential reference, and compaction metadata re-expression is not a new fact.
- [ ] Commit explaining production domain extension and bypass prevention.

## Task 8: Coupled acceptance, association guards and processing operations

**Files:** Create sync `service/association.rs`, `service/operation.rs`,
`tests/service_operations.rs`; modify `acceptance.rs` internals only where needed
to reuse its checked account/version/journal allocation.

**Interfaces:** `association::plan_binding(input: &BindingInput<'_>) -> anyhow::Result<ServiceDecision<BindingChange>>`;
`association::classify_candidates(input: &CandidateInput<'_>) -> anyhow::Result<CandidateSet>`;
`operation::plan_operation(input: &ServiceOperationInput<'_>) -> anyhow::Result<ServiceDecision<CoupledCommit>>`.
`BindingInput` supplies account, external key, selected QSO/deletion, current
association/index observation and expected guards. `ServiceOperationInput`
contains operation ID, account/membership/evidence view, source record reads,
binding reads and a typed `ServiceOperation`:

```text
ApplyImport { qso_change, assertions, selected_tag_effects, association }
CompleteObservation { observation, disposition, association, qso_change }
DeleteQso { qso_change, associations, suppression }
PrepareSubmission { attempt }
RecordSubmissionResult { result, batch_members, association, job_effects }
```

Native changes are existing `ServerChange`/approved prepared changes, not another
QSO schema. `job_effects` describe the semantic completed/uncertain attempt state;
they are not a new scheduler or queue. Represent each as
`JobOutcomeUpdate { attempt: AttemptId, result: SubmissionResultId, outcome: SubmissionOutcome }`
in the commit output so the future worker can commit it with its actual job row.
Source-version and support references are
retained alongside payloads. Required groups cannot be split into independent
commit instructions.

- [ ] Write tests for two simultaneous E1→Q1/E1→Q2 plans: both carry the same
  absent-index guard, committing one invalidates the other's guard. Existing
  E1→Q1 plus explicit Q2 duplicate requires review. Agreements combine, superseded
  bindings do not reactivate, Q1 deletion preserves suppression. Reject fabricated
  absent binding evidence and cross-account selected targets.

  ```rust
  assert_eq!(plan_for_q1.external_guards, plan_for_q2.external_guards);
  assert!(matches!(plan_binding(&occupied_by_q1)?, ServiceDecision::Review(_)));
  ```
- [ ] Run `cargo +1.99.1 test -p clacks-sync --test service_operations --locked`; expect failure.
- [ ] Implement binding checks and typed operation validation. Classify candidates
  using `CandidateInput { observation, account_qsos, recognition, lotw_configuration }`;
  all QSOs come from the account read view, never the currently displayed tag.
  `CandidateSet { confident, possible }` contains IDs and explicit missing-information/
  time-difference reasons. Compare full calls, compatible bands and recognized
  mode hierarchy; a parent-only report against a specialization is possible,
  not confident. Identical unknown labels can agree, but do not guess compatibility
  between different unknown labels. An explicitly contradictory identity excludes
  a candidate. Second times compare exactly for confident matching; minute
  coverage includes its 60 seconds. Otherwise compatible time intervals at minimum
  separation **at most 1,800 seconds**, including across midnight, are possible.
  Never round second-precision mismatch into confidence. More than one confident
  candidate needs explicit selection; any possible selection needs it even when
  unique. Validate the selected ID against this classification, not an unchecked
  caller flag claiming uniqueness. Missing callsigns cannot create a saved QSO
  or be supplied from current defaults. This is the production downloaded-record
  classifier, not another ADIF importer.
- [ ] Implement coupled planning using production acceptance internals: validate
  all original guards first, reject duplicate target writes, plan dependency-first
  against a local prospective account/head using the matching existing server or
  prepared-client admission path, allocate checked existing revisions,
  then resolve effect references from produced envelopes. Return one final
  `CoupledCommit`, journals and exact member descriptor. No intermediate plan
  is a durable checkpoint. A reference produced by the same operation is satisfied
  internally; external reads keep their original guards. Refuse cyclic support.
- [ ] Add tests for each operation group, including stale tag/target/head/membership,
  input observation changed since review, deletion during an in-flight attempt,
  record result after local correction (old values stay covered, new values pending),
  result after accepted QSO deletion (retain result/suppression, no reactivation),
  dispatch intent without sent proof, lost
  response then success, and upload-level batch outcome. Verify returned group
  contains disposition and accepted effects together and has no network action.
  Add candidate tests for parent-only mode, missing own call, one versus two
  confident candidates, explicitly different call, exact 1,800/1,801-second
  boundaries, minute coverage and across-midnight intervals. Declining candidates
  leaves explicit add/reject rather than creating or binding automatically.
- [ ] Run target and existing acceptance tests; expect pass. Commit with a body
  stating that a real transaction/index must enforce emitted guards and groups.

## Task 9: Download coordination, observation transitions and durable progress

**Files:** Create sync `service/download.rs`, `tests/service_downloads.rs`;
extend model observation/processing types for real control/progress rows.

**Interfaces:** `DownloadControl` is one maintained row per destination containing
current active acquisition and separate query-stream progress. `AcquisitionState`
retains query, active/completed/failed phase and page/window completeness.
These are `ServicePayload` variants with `DownloadStateId`/`AcquisitionId` keys;
retained observations embed sufficient provenance so completed unchanged polls
need not persist indefinitely.

`download::plan_start(input: &DownloadStartInput<'_>) -> anyhow::Result<ServiceDecision<CoupledCommit>>`;
`download::plan_observation(input: &ObservationInput<'_>) -> anyhow::Result<ServiceDecision<CoupledCommit>>`;
`download::plan_progress(input: &DownloadProgressInput<'_>) -> anyhow::Result<ServiceDecision<CoupledCommit>>`.
Inputs include compared control/acquisition rows, query coverage, current
established evidence/dispositions, proposed new IDs and the existing operation
context. Caller supplies time; no wall clock inside pure transitions.

- [ ] Write tests: check-now and scheduled starts compete on the same destination
  control guard; different destinations proceed independently. Successive A/A
  emits no second semantic observation; A/B/A emits three distinct observations.
  Recovered original A never passes through the fresh-download path.

  ```rust
  assert!(matches!(plan_observation(&same_a)?, ServiceDecision::Unchanged));
  assert!(matches!(plan_observation(&a_after_b)?, ServiceDecision::Ready(_)));
  ```
- [ ] Run `cargo +1.99.1 test -p clacks-sync --test service_downloads --locked`; expect failure.
- [ ] Implement coordinated start/completion/failure transitions through Task 8.
  Do not introduce lease/timeout assumptions or another ordering counter. A timed-out
  acquisition can be closed explicitly; its late response retains its old identity
  and is not promoted over a later established acquisition. Ordering uses existing
  accepted context plus properly scoped provider evidence, not arrival or ULID time.
- [ ] Implement comparison/application: omitted fields preserve prior information;
  new fields retain additional evidence; changed fields enqueue durable review;
  stronger ordering-only evidence reuses represented processing effects without
  reopening an unchanged rejection. Older reports cannot replace newer facts;
  contradictory unordered reports remain retained and request refresh. Incomplete
  keys remain unresolved input, never a fuzzy binding.
- [ ] Implement progress checks for separate LoTW receipt/confirmation streams and
  QRZ book/filter/field selection. Page position is not a permanent change cursor.
  Require retained inputs/outstanding work and declared page/window completeness
  before advancing progress. An incomplete/cancelled response cannot claim full
  coverage, while safely retained unmatched work can outlive the download.
- [ ] Run target; expect pass. Include new field selection, explicit clear denied
  without provider semantics, not-found/empty/failure, stale page, replay after
  serialized restart, A rejected/B changed/A returned and deleted-QSO suppression
  across query changes. Include a response with newer QSO-update evidence but
  older confirmation-update evidence: only the properly ordered components can
  advance established state. Commit with diagnostic/progress boundary context.

## Task 10: Coupled transfer and accepted-evidence recovery

**Files:** Create sync `service/recovery.rs`, `tests/service_transfer.rs`,
`tests/service_recovery.rs`; modify existing `transfer.rs`, `recovery.rs`,
`client.rs` and their fixtures.

**Interfaces:** Extend `WindowManifest` with `units: Vec<CoupledRequirement>`
and include units in `WindowManifest::new` validation/identity. Native callers
pass empty units. Extend durable `WindowProgress` with staged required inputs
and completed-unit bookkeeping; staged data is not installed coverage.
Use `staged: Vec<AcceptedRecord>` and `completed_units: BTreeSet<OperationId>`;
validate duplicates against exact source requirements rather than silently
replacing staged variants. Persist these with the existing progress generation.
`service::recovery::plan_recovered_operation(input: &RecoveredOperationInput<'_>)
-> anyhow::Result<ServiceDecision<CoupledCommit>>` consumes incoming original
accepted envelopes/support, returning actor, current real reads and existing
`EvidenceView`. Installed original envelopes retain their source stamps;
new reconciled maintained state receives normal new acceptance context.

- [ ] Write tests delivering a completed disposition before its QSO/association
  effects and splitting dependencies across batches. Assert no completed operation,
  partial QSO creation or full checkpoint until the coupled unit is representable.
  An unrelated ready unit installs immediately. A covering later QSO/deletion
  satisfies effect support without installing the old QSO. Reject mutation of
  units under the same immutable manifest identity.

  ```rust
  assert!(finish_window(&manifest, &partial, &installed, &previous, &evidence).is_err());
  assert_eq!(local_after.pending.as_ref().map(|p| &p.working),
      local_before.pending.as_ref().map(|p| &p.working));
  assert_eq!(local_after.pending.as_ref().and_then(|p| p.original.state()),
      local_before.pending.as_ref().and_then(|p| p.original.state()));
  ```
- [ ] Run `cargo +1.99.1 test -p clacks-sync --test service_transfer --test service_recovery --locked`;
  expect failure.
- [ ] Extend production manifest/batch/completion operations to validate dependency
  closure and atomic units, using Task 7 combination before native receipt.
  A ready unit may exceed one transport message but its effects install together;
  progress records durable staged inputs without advertising their receipt as a
  full prefix. Shared supports do not require permanent account-wide transactions.
  Keep native pending originals/working values through `receive_accepted`.
- [ ] Integrate the recovered service path with existing returning-actor admission,
  timeline comparison and `plan_recovered_install`. Check immutable content before
  Covered shortcuts. Authenticate the carrier independently of original authors;
  revoked original writers cannot submit new work but their accepted facts remain
  recoverable. Mutable binding conflicts use existing client review and do not
  certify an old prefix until represented. Provider contradictions retain uncertainty,
  not operator QSO review. No recovered intent or effect dispatches a request.
- [ ] Run targets plus existing `transfer`, `recovery`, `client_state` tests; expect
  pass. Add restored-server P1 success/current P2 pending; second client returns
  the same success; unavailable old ordering context; old positive versus known
  newer negative; stale references; excluded carrier; acknowledged-window replay.
- [ ] Commit explaining atomic units versus transfer batches and original acceptance
  versus new installation positions. Do not claim actual database crash tests.

## Task 11: Retention protection, complete scenarios and adapter documentation

**Files:** Create sync `service/retention.rs`, `tests/service_retention.rs`,
`tests/service_scenarios.rs`; modify `compaction.rs` only for surviving-reference
validation, both crate READMEs, root README, `docs/service-evidence.md`,
`docs/integrations.md`, `docs/synchronization-model.md` and `docs/lotw.md` where needed.

**Interfaces:** `retention::requirements(records: &[AcceptedRecord],
units: &[CoupledRequirement]) -> anyhow::Result<RetentionRequirements>` returns
required semantic keys, destination descriptions, support/effect/interpretation
references and removable completed staging. It does not choose a timestamp TTL
(time to live), summarize semantic history or replace the existing `plan_baseline`.
`RetentionRequirements { semantic_keys, support, units, removable_staging }` is consumed when validating proposed collection against
surviving service references. Existing baseline logic still selects the boundary.

- [ ] Write tests for indefinitely offline acknowledgment holding the normal
  boundary back; revocation releasing that obligation but not removing accepted
  attempts/observations; compaction preserving P1 baselines, disposition effect
  coverage and deletion memory. Attempts with equal projections stay distinct.
  Unchanged poll staging may be removed only after retained observations are
  self-contained and no surviving requirement needs it.

  ```rust
  assert!(requirements.semantic_keys.contains(&attempt_a1.key));
  assert!(requirements.semantic_keys.contains(&attempt_a2.key));
  assert_eq!(recovered_attempt_a1.stamp.accepted_at, original_attempt_a1.stamp.accepted_at);
  ```
- [ ] Run `cargo +1.99.1 test -p clacks-sync --test service_retention --test service_scenarios --locked`;
  expect failure for missing retention integration/scenarios.
- [ ] Implement required-reference protection and validate baseline projection
  preserves logical fact identity/support. Keep old interpretations when an
  equivalent summary cannot satisfy a surviving reference. Do not delete raw
  diagnostic material solely because a parsed result exists; that policy remains
  outside this capability. No mandatory receipts for unchanged polls or permanent
  history of complete QSO bodies.
- [ ] Add production-operation scenario tests for all spec §14 cases: independent
  imported claims; late success after edit; repeated/changed/partial confirmation;
  interruptions around input, processing, dispatch and transfer; retirement,
  compaction and rollback. Serialize/reload only actual pre-/post-operation rows.
  Use private fixture builders, not a simulated store/server or public test API.
  Assert each outcome and its guards, not just row counts or serialization success.
- [ ] Run the focused targets; expect pass. Check concrete error messages identify
  account/destination/record and missing guard/support or failure phase, with
  wrapped causes accessible and no credential/key fields in debug/serialization.
- [ ] Update owning docs with actual public entry points, group/index/dependency
  obligations, supported transformations and limits. Mark the service spec as the
  current implemented pure contract only after tests establish it; retain explicit
  pending provider/database verification. Explain how future signing consumes
  `LotwSubmission` without re-normalizing or linking the sync crate. Do not create
  a review report, handoff/status copy or claim issue completion from documents.
- [ ] Run all final checks below. Commit with substantive context and actual
  verification evidence. Request whole-branch review using the selected execution
  workflow; resolve confirmed findings before publication/completion claims.

## Final verification and delivery

Run from the workspace root, using the lockfile produced by the actual execution:

```sh
python3 scripts/extract-lotw-data.py --check
cargo +1.99.1 fmt --all --check
cargo +1.99.1 test --workspace --all-targets --locked --offline
cargo +1.99.1 test --workspace --doc --locked --offline
cargo +1.99.1 clippy --workspace --all-targets --locked --offline -- -D warnings
cargo +1.99.1 check --workspace --all-targets --target x86_64-pc-windows-msvc --locked --offline
RUSTDOCFLAGS='-D warnings' cargo +1.99.1 doc --workspace --no-deps --document-private-items --locked --offline
git diff --check
```

All commands must succeed; record environmental blocks accurately. Fetch newly
selected dependencies before offline checks, rather than repeatedly trying an
uncached offline build. Windows target checks establish compilation, not Windows
runtime or provider interoperability. Validate changed Markdown links/anchors
and inspect source additions for forbidden production panic/indexing patterns.

Review spec coverage before declaring execution complete:

| Specification | Owning tasks |
| --- | --- |
| §§1–4, domain identities, imported claims, remote key/correction limits | 1, 3–5, 8 |
| §5, full submission sets/conversions, one shared LoTW signing input | 2–4 |
| §6, independent coverage, conservative time, provenance/eligibility | 5–6 |
| §7, observation identity/coverage/equality/acquisition/recovery | 5, 7, 9–10 |
| §8, unique binding, review, version rejection, deletion memory | 7–9 |
| §§9–11, combination, dependency closure, groups, interrupted recovery | 7–10 |
| §12, semantic retention versus synchronization compaction | 10–11 |
| §13, production dependencies/module boundaries | 1–11 |
| §§14–15, worked verification, diagnostics, source/live-test limits | Each owning task; full scenarios in 11 |

Issue #3 remains open until the production capability and completion evidence
are delivered. Database/provider issues integrate this implementation and verify
their own real transaction/network boundaries; they do not replace it.

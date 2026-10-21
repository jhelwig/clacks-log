# Service evidence and external-record associations

This specification consolidates the accepted design for [issue #3](https://github.com/jhelwig/clacks-log/issues/3).
It is ready for review before implementation planning; it does not claim that
the described service components or their persistence integration are implemented.
The [integration contract](integrations.md), [native model](native-model.md) and
[synchronization model](synchronization-model.md) remain applicable.

Logbook of The World is abbreviated LoTW. Amateur Data Interchange Format (ADIF)
is the interchange standard; ADI is its text format. Coordinated Universal Time
is UTC. DX Century Club is DXCC. CQ magazine and the International
Telecommunication Union (ITU) define the zone systems used below. A contact
confirmation is a QSL. Application programming interface is abbreviated API.

## 1. Purpose and delivery boundary

Keep recorded QSO facts, imported service claims, actual publication history,
provider observations and processing decisions distinct. Derive current status
from applicable evidence instead of maintaining one ambiguous uploaded/confirmed
flag. Success means imports, synchronization and later provider workers use
these production components directly, extending them rather than replacing an
artificial environment.

Issue #3 supplies domain representations, normalized service projections,
applicability calculations, evidence combination/recovery, dependency checks,
association guards and conditional transition outputs. It extends the existing
production synchronization domains. It does not implement live provider calls,
certificate cryptography, scheduling, database schemas or transport. Later
storage integration must enforce the specified transactions and uniqueness in
PostgreSQL and SQLite; pure transition tests cannot establish database durability.

The design does not select full event sourcing, another clock, automatic QSO
field merging or an independent replication protocol. Account-owned QSOs may
have several tags or none; tags do not duplicate contacts or service evidence.
Every enrolled client synchronizes the account's single scope, subject to the
existing restrictions on server-only secrets.

## 2. Terms, identities and record responsibilities

A **destination** is the remote collection in which an external identity is
meaningful. Its stable Clacks identity is independent of replaceable credentials.
An **external-record key** identifies one remote contact within that destination;
it is not a fuzzy local candidate-matching key. A **projection** is the normalized
semantic field/value set actually intended for one service. A **baseline** retains
the values covered by an assertion so later changes can be evaluated.

Use distinct stable ID types consistent with the native model's Universally
Unique Lexicographically Sortable Identifier (ULID) convention. IDs identify
entities; their timestamp or lexical order is not causal evidence. Every lookup,
reference, comparison and operation is account-scoped.

| Responsibility | Retained domain information |
| --- | --- |
| Imported assertion | QSO, service, recognized claim, supplied dates, import/source-record provenance, frozen imported baseline and source coverage; destination only when established. |
| Submission attempt | QSO, destination, frozen actual or possibly dispatched semantic values, public signing identity, dispatch phase/time and request/batch correlation. Synchronization context identifies the accepted source version. |
| Submission result | Attempt or identified batch members, outcome, phase/scope, available diagnostics and provider-returned identity. Explicit success, rejection and uncertainty are distinct. |
| Remote observation | Destination, full or explicitly incomplete external identity, stable observation ID, reported facts/coverage, provider timestamps and acquisition provenance. It can exist without a local QSO. |
| External-record association | Maintained binding between a destination-qualified external record and a QSO, with accepted binding and supersession/deletion context. |
| Processing disposition | Exact observation reviewed, decision, resulting association and accepted effect references, or durable outstanding review work. |

These are responsibilities, not a requirement for six tables. Assertions,
attempts, results and retained observations describe immutable facts. Associations
and processing state have maintained-state semantics; recovery cannot simply
union competing bindings or reactivate superseded state.

Download coordination/progress and deletion suppression are supporting state,
not additional claims that an upload or confirmation occurred. A retained
destination description includes the service, known remote collection binding
and historical routing identity needed to interpret evidence. It does not
require live credentials or an enabled configuration. Removing a credential
must not destroy public historical interpretation.

```text
QSO ── imported assertions
  ├── submission attempts ── results
  └── associations ── external record at destination
                           └── observations ── dispositions/effect references
```

## 3. Imported claims and dispatched values

### 3.1. Imported baselines

Accept recognized positive imported upload status immediately, without a
verification download. The imported QSO content supplies its service-relevant
comparison baseline. Missing upload status and explicit unsent status are
pending independently for each service. A service-level imported claim with no
known destination does not fabricate one; routing/readiness rules still apply.

Accept recognized positive imported confirmation immediately against the
imported QSO's contact-identity baseline, preserving supplied dates, provenance
and unavailable source information. Upload and confirmation assertions are
independent: neither implies the other.

The baseline is an explicit interpretation of the import, not proof of the
historical serialized payload. Do not invent an actual Clacks attempt, provider
response, counterpart report, remote ID or missing date. Do not retroactively
insert current directory/provider fallback into the imported baseline. Exact
source flag/date recognition belongs to the import adapter; an unfamiliar
nonempty status is not automatically positive.

### 3.2. Attempts and outcome scope

Retain the actual semantic values sent, not only a QSO version reference.
Effective output can depend on separately maintained fallback information.
Distinct dispatches remain distinct attempts even when their projections are
equal. Results refer to the actual attempt or explicit batch membership.

An immediate explicit upload-level success is usable success at that scope.
It is not fabricated individual-QSO processing or counterpart confirmation.
A missing/unusable response is uncertainty, not proof of rejection. Retain
timeout and later success evidence; explicit success can resolve the earlier
uncertainty at the scope it establishes.

Each retry is a new dispatch using current accepted server values. A response
to an older attempt covers that attempt's values; a later correction can remain
pending. Recovery of an intent, attempt, result or disposition never itself
dispatches a network request.

## 4. Destinations, remote identity and correction safety

| Service | Destination | External-record key within the destination |
| --- | --- | --- |
| LoTW | Configured remote LoTW account, independent of login/certificate replacement. | Full own station call, full worked call, UTC start date/time to the second, primary band, effective receive band and actual concrete mapped LoTW mode. |
| QRZ | A particular remote QRZ logbook; retain provider `bookid` when known. | Provider-assigned integer `logid`. |

A second configuration accessing the same collection must not create a second
association namespace. LoTW signing certificates are eligibility/signing inputs,
not destinations. QRZ access keys are replaceable; callsign/date coverage is a
routing attribute, not a substitute remote book identity. Route using saved
station callsign/date, never current client defaults. Missing or ambiguous
routing pauses only affected uploads.

### 4.1. LoTW mode identity

Use the data-driven TrustedQSL conversion sequence from the pinned 2.8.6 source
and configuration 11.34:

1. Uppercase the input mode and submode.
2. With a submode, try the configured `MODE%SUBMODE` mapping, then the submode
   alone, then the parent mode alone.
3. Without a submode, try the mode alone.
4. If no mapping exists, retain the uppercased mode; validate that the result is
   a supported LoTW mode, otherwise return a readiness error.

Examples: MFSK/FT4 and bare FT4 map to FT4; FT8 stays FT8; SSB/USB and SSB/LSB
map to SSB; PSK/BPSK31 and PSK/QPSK31 map to PSK31; MFSK/JS8 and bare MFSK map
to DATA in this pinned configuration. MFSK denotes multiple frequency-shift
keying, PSK phase-shift keying, SSB single sideband, and USB/LSB its upper/lower
sidebands. Literal catalog labels are preserved in native values.

Retain the mapped value and mapping provenance with attempts/keys. A later
mapping update does not recalculate historical keys or edit native mode values.
Do not collapse all digital modes to DATA or substitute an award mode group for
the concrete remote mode.

Prefer downloaded `APP_LoTW_MODE` when supplied. Otherwise use `MODE`/submode
with the service mapping only where the result is unambiguous.
`APP_LoTW_MODEGROUP` is an award classification; `APP_LoTW_QSLMODE` describes
the counterpart in specified circumstances. Neither substitutes for our record's
mode. Incomplete identity remains unresolved rather than guessed.

LoTW's specific confirmation-update instructions include receive band, in
addition to the calls/time/primary-band/mode identity. Identity-changing
corrections can create another remote record. One local QSO may consequently
retain several historical external identities; each external identity still
binds to at most one local QSO.

### 4.2. QRZ duplicate behavior is not identity

QRZ's API provides `INSERT` with `OPTION=REPLACE` and returns the affected ID,
but does not document a conditional update targeting exactly an expected
`logid`. Its documented replacement can overwrite a confirmed entry with an
unconfirmed one pending matching. File-import protection of confirmed entries
does not establish that protection for API replacement.

The file-import duplicate predicate uses callsigns, mode and band within
30 minutes. This is not an equivalence relation or an external key: records at
12:00, 12:20 and 12:40 demonstrate overlapping windows without equivalent
endpoints. Do not bucket time or invent remote uniqueness from this predicate.

Retain actual returned IDs without assuming replacement preserves the old ID.
A duplicate rejection is not proof a corrected payload was accepted. Detect
known competing associations and ambiguous replacement candidates before
dispatch; refuse an unsafe correction and expose the affected work for remedy.
Do not pick among multiple plausible targets or introduce delete-and-reinsert.
A read-before-write check cannot make the provider write conditional or exclude
changes made through another application. A successful response reports what
was touched, not proof that Clacks's intended precondition held remotely.

Exact API mode/submode duplicate comparison, replacement ID continuity and
selection among multiple qualifying remote records require controlled provider
verification or authoritative clarification. This specification does not assert
a generally safe QRZ correction algorithm in their absence.

## 5. Submission projections and equality

Compute one normalized projection from current accepted state, the selected
destination and applicable service mapping/signing inputs. For supported
contacted-station output, select recorded facts, then eligible LoTW information,
then eligible QRZ information, then eligible directory information. Select
location as a whole, including partial recorded location and location-bearing
references; never fill its gaps from a lower source. Own operating context
comes from saved QSO values.

Compare values after service transformations at actual output precision.
Use integer-derived decimal conversions, whole-second times and deterministic
field/collection ordering. Preserve meaningful absence. Field order, encoding,
signature bytes, redundant numeric zeroes, lookup times, synchronization
revisions, mapping-version changes without output changes, tags and credential
serials are not submitted QSO-value changes. Complete callsign slash components
remain significant; directory lookup normalization does not change upload identity.

A payload change can make publication pending without proving the service can
apply it. In particular, omission of a previously supplied optional field does
not establish a remote clear without verified clearing semantics. Readiness,
routing, association safety, tag policy, cooldown and pause state remain separate
from comparison and from whether a native QSO is valid.

### 5.1. LoTW projection

| Part | Included semantic values and transformations |
| --- | --- |
| Signing identity | Saved own callsign and DXCC entity checked against eligible certificate public information. If the recorded entity is absent, explicitly retain the selected certificate entity actually used; a supplied entity must be consistent. |
| Contact identity | Worked callsign, UTC start date/time, service-compatible primary/effective receive bands, actual mapped LoTW mode. |
| Frequencies | Exact primary frequency when known; meaningful receive override. Convert whole hertz to decimal megahertz without inventing a frequency for band-only data. |
| Own location | Supported saved grid, CQ/ITU zones, entity-specific administrative fields and explicitly mapped Islands on the Air (IOTA) references. Use signing configuration validation/mapping, without substituting current profiles. |
| Other supported signing fields | Include any additional value actually emitted by the supported signing definition so it cannot escape comparison. Do not invent currently unrepresented native inputs. |

Absent receive override uses the primary pair for both directions. A band-only
override never borrows the primary frequency. Consistently omit redundant equal
receive values. Derive band from frequency using compatible service definitions;
an unrepresentable required band/mode is a service readiness error, not a reason
to discard the otherwise valid QSO.

The pinned contact definition contains `BAND`, `BAND_RX`, `CALL`, `FREQ`,
`FREQ_RX`, `MODE`, `PROP_MODE`, `QSO_DATE`, `QSO_TIME` and `SAT_NAME`. Own identity
and entity are supplied through certificate/station records. An ADIF-facing
adapter uses `TIME_ON` for the corresponding start time.

Supported station definitions include `GRIDSQUARE`, `CQZ`, `ITUZ`, `IOTA`,
`US_STATE`/`US_COUNTY`, `CA_PROVINCE`, `AU_STATE`, `CN_PROVINCE`,
`JA_PREFECTURE`/`JA_CITY_GUN_KU`, `RU_OBLAST` and `FI_KUNTA`. Preserve unknown
optional location data as unknown; report unusable supplied values. The pinned
single-grid validator supports up to twelve characters. An explicit point
supplies its derived locator; do not invent multi-grid boundary operation.

The native MVP does not represent propagation/satellite identity, multiple
boundary grids or every legacy station field. Listing protocol fields does not
add those product capabilities. Never infer satellite operation or map Parks
on the Air (POTA) membership into the legacy National Parks on the Air (NPOTA)
fields merely because a historical signing configuration contains them.

This format does not submit counterpart name/location, signal reports, end time,
power, equipment, comments or arbitrary contest references. Changes only to
those values do not make LoTW pending. Downloaded frequency precision (five
decimal megahertz places) does not replace whole-hertz dispatched evidence.

### 5.2. QRZ projection

Use ordinary ADIF output, including its agreed reference-context selection,
without POTA-compatible record expansion. This is Clacks's emission/comparison
set, not a claim QRZ interprets every emitted field for awards.

| Native information | Emitted fields |
| --- | --- |
| Identities/start | `STATION_CALLSIGN`, `CALL`, `QSO_DATE`, `TIME_ON` |
| Optional end | `QSO_DATE_OFF`, `TIME_OFF` |
| Primary/receive | `BAND`, `FREQ`, meaningful `BAND_RX`, `FREQ_RX` |
| Mode | Canonical ADIF `MODE` and optional `SUBMODE`, preserving specialization rather than using LoTW's mapping |
| Reports | `RST_SENT`, `RST_RCVD` |
| Operators/names | `OPERATOR`, recorded own `MY_NAME`, `CONTACTED_OP`, effective contacted `NAME` |
| Own location | `MY_GRIDSQUARE`/`MY_GRIDSQUARE_EXT`, `MY_LAT`/`MY_LON`, `MY_DXCC`, `MY_STATE`, `MY_CNTY`, `MY_CQ_ZONE`, `MY_ITU_ZONE` |
| Effective contacted location | `GRIDSQUARE`/`GRIDSQUARE_EXT`, `LAT`/`LON`, `DXCC`, `STATE`, `CNTY`, `CQZ`, `ITUZ` |
| Power/equipment/comment | `TX_PWR`, `MY_RIG`, `MY_ANTENNA`, `COMMENT` |
| Participation/references | Supported ordinary group mappings, such as own/contacted IOTA, POTA, SOTA, WWFF, SIG fields or an unambiguously selected `CONTEST_ID` |

SOTA denotes Summits on the Air, WWFF World Wide Flora and Fauna; `SIG` is
ADIF's special-interest-group field. These are mapping examples, not hard-coded
native group types. Existing context selection/ambiguity rules apply.

- Emit decimal megahertz to whole-hertz resolution and decimal watts to
  one-milliwatt resolution, with whole-second UTC times. Band-only stays band-only.
- An area position emits its locator without asserting a measured point. A point
  emits coordinates and its consistent locator. Split ten/twelve-character grids
  into the first eight characters and the extension fields.
- Convert native millionths of a degree to ADIF hemisphere/degrees/minutes with
  three fractional minute digits, using deterministic nearest rounding with exact
  halfway magnitudes rounded away from zero, carrying minute overflow into
  degrees. Compare emitted values; finer changes with identical output do not
  require a correction.
- Apply the existing best-effort ADI text conversion with meaningful-loss
  warnings. Preserve original native text. Changed emitted text is relevant even
  when basic confirmation is unaffected.
- `MY_RIG` describes our equipment; `RIG` describes the contacted station's.
  Do not concatenate unmapped equipment notes into unrelated comments.
- Exclude credentials, tags, native IDs, catalog/group metadata, lookup times,
  opaque imported fields, provider ID/status fields and upload/confirmation flags.
  A local imported flag cannot assert a provider confirmation through submission.

A comment/equipment change may make QRZ pending while leaving LoTW covered.
Optional removal changes the projection but does not prove QRZ cleared its field.

## 6. Confirmation applicability and source eligibility

Publication coverage and confirmation applicability are different calculations.
A QSO can have historical upload success, applicable basic confirmation and a
pending correction simultaneously. Never imply the new values were already
accepted by the provider merely because earlier confirmation still applies.

| Dimension | LoTW | QRZ |
| --- | --- | --- |
| Station identities | Both complete station callsigns | Both complete callsigns with destination/logbook context |
| Start | Retained confirmed baseline, or actual counterpart/fresh provider evidence | Same |
| Band | Primary and effective receive band | Primary band; receive information remains in submission but is not an established basic-match criterion |
| Mode | Concrete mapped mode and provider mode group, with exact-mode qualification retained separately | Canonical ADIF parent mode; submodes count as their parent for basic confirmation |
| Satellite context | Required when represented; never infer it | No additional basic criterion established by the cited source |

Both services document a 30-minute start-time tolerance between the operators'
reports. LoTW permits exact mode or a shared provider mode group; satellite
matching additionally requires satellite propagation and the same satellite
identity when those inputs are represented. QRZ's basic rule uses the parent mode.

Provider time tolerance compares the two operators' reports. It does not compare
two edits to our own report. For a changed start time, require the baseline time
to remain unchanged unless actual counterpart information or fresh provider
evidence establishes applicability. Our old 12:00 and a hidden counterpart's
12:29 can match; changing ours to 11:58 produces a 31-minute difference even
though the local correction was only two minutes.

Calls/band changes stop old evidence establishing current confirmation.
Unchanged comparison dimensions after service transformation preserve basic
applicability. A QRZ submode-only change within the same recognized parent need
not invalidate it. A LoTW same-group change can preserve group-level applicability
without preserving exact-mode qualification or changing the historical remote key.
Do not guess compatibility for unfamiliar values without supporting interpretation.

Names, reports, comments, frequency changes within the relevant bands, equipment,
power and location details do not independently invalidate basic confirmation.
New geographic values do not inherit old geographic/award evidence. Only
source-identified applicable provider facts may supply fallback. A returned copy
of our uploaded location is not the other station's report because its confirmation
flag is positive. QRZ's provider status may include credit imported from LoTW;
that remains QRZ evidence, not an invented independent LoTW assertion or hidden
counterpart report.

Trust imported positives initially, including their explicit source limitations.
After edits, unavailable required comparison evidence yields applicability not
established, distinct from explicit provider-reported unconfirmed. Preserve all
historical evidence. A known newer explicit unconfirmed observation changes
current status; missing fields, empty result windows and failed queries do not.

## 7. Observation identity, equality and acquisition

### 7.1. Identity and reported coverage

Keep the external-record key, stable observation ID and acquisition ID distinct.
An acquisition is a download operation, not a replacement for provider version
information. Recovery preserves original IDs and accepted context rather than
pretending an old observation was freshly read.

For each supported field distinguish a reported value (including an explicit
negative), an explicit clear supported by provider semantics, and not reported.
Retain source coverage and the distinction between our reported values and the
counterpart's values. Omission supplies no replacement. Newly reported optional
information is evidence even if earlier queries did not request it.

Compare normalized typed facts using service-specific transformations, exact
numeric values and deterministic collection ordering. Preserve unfamiliar values
instead of guessing equivalence. Fetch time, wire encoding, field order and
incidental diagnostics do not change the facts. A content hash may accelerate
lookup, but direct equality is authoritative and a hash proves no ordering.

### 7.2. Fresh-download procedure

Permit one active download per destination. Scheduled runs, check-now requests
and retries share that coordination. A response arriving after its operation
timed out retains the earlier acquisition context; arrival does not promote it.
This establishes successive ordinary acquisitions, not an atomic provider
snapshot or a complete remote history. Uploads may change provider state during
a download.

For each returned record:

1. Establish the external key. If essential identity is missing, retain diagnostic
   information and unresolved work without inventing a key, binding or fuzzy
   cross-record merge.
2. Evaluate provider ordering evidence within its documented scope. A demonstrably
   older report cannot replace newer applicable evidence. LoTW QSO-update and
   confirmation-update timestamps have distinct meanings; QRZ does not establish
   a universal record revision token.
3. Compare reported fields to currently established information for that external
   record, leaving unreported fields untouched. Do not compare against every
   historical content value to decide whether this occurrence is new.
4. Classify unchanged information, additional information, changed information,
   or stronger ordering evidence alone.
5. Retain additional/changed evidence as a new observation for normal processing.
   Unchanged delivery needs no new semantic observation or repeated effect.
   Preserve useful stronger ordering evidence without repeating effects or
   reopening an unchanged rejection.
6. Commit evidence and required pending work together. Advance download progress
   only through retained input with established coverage.

Observed A → A has one semantic observation. Observed A → B → A has three;
the final A is a new occurrence after an observed change, despite equal content
to the first. Clacks cannot distinguish unchanged A from an unobserved A → B → A
between polls. Broader field coverage is not a negative update to omitted fields.

Observation lookup is account/destination/external-key scoped; retained stable
IDs identify evidence directly. Acquisition/query stream and field-selection
identity describe provenance/coverage. Keep duplicate and current-state lookup
support distinct from provider page positions and version-specific dispositions.
Do not require a permanent predecessor chain or a receipt for every unchanged poll.

### 7.3. Recovery equality and ordering

Same observation ID and identical immutable contents is a duplicate. Same ID
with different immutable contents is an integrity error with diagnostics, never
a silent overwrite. Different IDs require evidence combination through provider
ordering and existing synchronization context; equal contents alone do not prove
the same occurrence. Neither ULID timestamps nor local wall-clock/response
arrival order establishes provider revision order.

If surviving ordering evidence is insufficient, retain uncertainty and query
fresh provider state for affected fields. A returned old client copy is not a
fresh read. Contradictory unordered provider facts do not prompt operator
resolution as though they were competing manual QSO edits.

## 8. Association, review and deletion

Enforce the unique binding:

> account + destination + external-record key → at most one local QSO

Different local contacts may have identical matching fields. An explicit
Duplicate does not inherit an occupied association. A competing outgoing contact
must require resolution rather than silently overwriting another contact's
remote record. Different observations of one external record preserve its
established relationship. One QSO may retain multiple historical remote keys.

Use established-association precedence. Otherwise search all account QSOs,
including untagged contacts and contacts ineligible for automatic upload. Apply
the existing precision-aware confident/possible matching policy in
[candidate matching](../references/adif-matching.md#accepted-candidate-matching-d41).
A unique confident match may associate automatically; compatible incomplete
identity and otherwise compatible contacts within 30 minutes either side require
explicit selection even for one candidate. Ambiguity never fans out to several
local QSOs. Local candidate matching is distinct from provider confirmation
matching and from external identity.

Unmatched contacts require explicit add or reject. Addition requires a selected
tag and a valid saved QSO, including both station callsigns. Missing own callsign
requires explicit historical assignment; current defaults cannot fill it.
Optional unavailable historical context stays unknown. Provider detail refresh
does not manufacture an edit to recorded QSO facts.

Rejection is scoped to the reviewed observation version. Identical repeat
delivery remains suppressed; changed reviewed information receives normal
processing. An observed A → B → A can therefore require new review. Stronger
ordering metadata alone does not reopen an unchanged rejection.

Deletion suppression is separate and survives changed observations, full
downloads and query/filter changes. Accepted local deletion prevents further
uploads and preserves sufficient external identity/binding/deletion context to
avoid offering a known deleted contact as new. External copies remain untouched
in MVP. Tag deletion does not discard evidence required by surviving QSOs or
deletion suppression.

## 9. Evidence combination and recovery

All recovery traffic is authorized client ↔ server. A surviving client may
return previously server-distributed accepted evidence to a restored server;
there is no peer-to-peer synchronization. New unsynchronized proposals follow
normal admission. Previously accepted evidence preserves original identity,
acceptance position and interpretation even if its original author is revoked.

| Material | Combination rule |
| --- | --- |
| Same immutable evidence ID/content | Already known; no second effect. |
| Same immutable ID/different content | Integrity error with identifying diagnostics; no overwrite. |
| Distinct attempts/equal projections | Preserve distinct dispatch histories. |
| Timeout followed by success | Preserve both; success resolves uncertainty only at its supported scope. |
| Result missing attempt/baseline | Retain incomplete material without claiming current publication coverage. |
| Completed disposition | Recover its accepted effects/dependencies; do not execute it again. |
| Differing observations | Apply provider order and existing causal evidence where sufficient; otherwise retain uncertainty and refresh. |
| Associations | Agreeing bindings combine; superseded bindings do not reactivate. Incomparable incompatible bindings follow existing client resolution and conditional acceptance. |

A recovered success suppresses publication only when its projection covers the
current accepted values. Newer unconfirmed evidence does not erase upload
success, and an old recovered positive does not displace known later negative
evidence. Mutable binding conflicts do not create a server-side store of
unresolved operator proposals.

Restore publication uses the existing operator pause, synchronization catch-up
and explicit-resume procedure. Do not require all offline clients to return,
add another global acknowledgment round or automatically pause all uploads
because one client has unresolved QSO edits. Existing rollback detection limits
remain: no evidence model can recover facts absent from every surviving copy.

## 10. Recoverable dependencies

Dependencies must accompany evidence or already be durable at the receiver.
Equivalent retained summaries may satisfy interpretation where they preserve
the necessary meaning. Distinguish interpretation dependencies, current-state
application guards and transfer coverage requirements.

Domain payloads retain stable IDs and sufficient semantic values/provenance.
The synchronization-side carrier retains original accepted context and exact
support/effect version references. Preserve necessary lineage/baseline
interpretation directly or through valid existing summaries. Temporary expected
versions used to guard acceptance are not interchangeable with historical support.

| Kind | Required support and conditions for application |
| --- | --- |
| Imported assertion | QSO/service, known destination descriptor, claim/date/provenance, frozen imported baseline and coverage, retained projected values or transformation interpretation. Current QSO/deletion is needed for applicability, not simply retention. |
| Attempt | QSO ID, destination descriptor, frozen actual/possible sent projection and signing identity, phase/correlation. No full original QSO body is required if these are self-contained. |
| Result | Attempt and its projection/destination; explicit outcome scope and batch membership mapping. Do not infer member-level success from an upload-only result. |
| Observation | Destination, full/explicitly incomplete identity, facts/coverage, provider timestamps, original acquisition/ordering context. No local QSO needed merely to retain it. |
| Association | External key, target QSO ID, accepted binding/supersession/deletion context. Application checks current binding or certified absence, target state and reference/account guards. |
| Disposition | Exact reviewed occurrence, decision/context, association and coupled effect references. Completion requires accepted effects, covering successors or deletions durably represented. |
| Deletion suppression | External identity/historical binding or sufficient retained deletion/matching evidence, with accepted deletion context. No live deleted payload is required to satisfy an artificial reference constraint. |
| Download progress | Destination, query stream/field-selection identity, response/page/window completeness and covered observations/outstanding-work references. Retain inputs before advancing provider progress. |

A later accepted successor/deletion can cover an old effect reference through
existing causal coverage. Do not reinstall an old effect over its successor.
Retain incomplete recovery material and explain missing support without blocking
unrelated records. It cannot authorize a positive derived claim, active binding,
completed coupled operation or full prefix dependent on the missing material.
Do not repair missing support with current defaults or relabel it a manual edit.

## 11. Transactions, dispatch and interrupted transfer

### 11.1. Conditional acceptance

Within the same real transaction, recheck account head/admission, compared record
versions, dependencies, target references and association uniqueness. Save the
coupled records, accepted contexts, journal entries and resulting head together.
Use per-record acceptance by default with explicit operation grouping; no
account-wide transaction or custom write queue is prescribed.

| Workflow | Atomic group |
| --- | --- |
| Apply one imported contact | QSO creation/update, assertions/baselines and selected coupled tag/association effects. |
| Retain downloaded input | Observations plus durable pending work; checkpoint advancement only over retained complete coverage. |
| Finish processing | Disposition, association and any resulting accepted QSO creation/update under current guards. |
| Accept QSO deletion | Deleted QSO state and required binding/deletion-suppression changes. |
| Record upload response | Result, attempt/batch correlation, authoritative returned association subject to guards, and durable job state preventing replay. |

Durable unmatched review need not block download progress once its input/work
is safely retained. Provider-only refresh does not create a recorded-fact edit.

### 11.2. Network boundary

Save dispatch intent and frozen candidate values before the external operation.
Intent is not proof bytes were sent. Record actual phase/outcome evidence when
available; interruption around the network call can leave a possible send.
Keep provider calls outside database transactions and do not claim exactly-once
external execution.

After recovery, normal scheduling evaluates current accepted projection,
routing, tags and pauses before creating another attempt. Historical frozen
values are not a replay instruction. Retry unless explicitly successful; clearly
non-transient problems pause only affected uploads until remedy and explicit
resume, while unrelated work continues. A local edit does not silently resume
that pause. Failure classification/backoff and detailed worker implementation
remain downstream integration work.

### 11.3. Transfer and installation

Extend the existing consistent manifest/window to include evidence and its
support. Keep exact requirements stable during the window. Dependencies may
cross messages and bounded batches; incomplete material may be staged durably.
Do not expose completed dispositions/bindings with missing required effects.

Install each complete coupled unit atomically; unrelated units may proceed.
Preserve local working QSO values and genuine original comparison evidence while
installing accepted effects through existing client transitions. Save accepted
state, preserved comparison evidence and installation progress together.
Advance/reload/acknowledge the account checkpoint only after full manifest
completion. Provider download progress is distinct from this synchronization
acknowledgment.

An unrepresented required association conflict prevents certifying its old
prefix complete; unrelated recovery/new-timeline traffic can continue. An
unordered pair of provider observations can be fully durable while current
status remains uncertain; refreshing it does not require resolving a native
QSO conflict first. Authenticate the returning client and recheck account,
destination and binding conditions during installation.

## 12. Retention and collection

Retain distinct semantic service evidence indefinitely for MVP, without
age-based expiry or lossy service-history summarization. Preserve imported
claims/baselines/provenance; distinct attempts, frozen values and outcomes;
meaningful observations/transitions; active and necessary historical bindings;
reviewed-version rejection; deletion suppression; completed processing/effect
references; and required public destination/ordering interpretation.

Unchanged polls do not create another semantic record. Identical copies,
redundant encodings/journal detail and completed staging can be removed.
Historical QSO bodies need not be kept solely for service comparisons when
self-contained projections suffice and no surviving reference needs the body.
Do not replace distinct attempts with a count or latest-status flag.

Synchronization compaction follows the oldest authorized durable acknowledgment,
timeline-aware common coverage, baseline installation before removal and
surviving-reference rules. An indefinitely offline authorized client may hold
that boundary back. Revocation removes its participation obligation but does
not erase accepted evidence. Acknowledgment by everyone makes redundant ordering
detail eligible for collection, not the underlying service facts.

Raw request/response diagnostics need their own provider/observability policy;
no duration is selected here. Parsed outcomes do not automatically make useful
source diagnostics disposable. Exclude credentials/private keys from evidence
and diagnostics. Credential deletion and backup requirements remain separate.
Any future summary must preserve status, provenance, duplicate/replay suppression
and dependency interpretation.

## 13. Production module and dependency layout

Arrows below mean “depends on”:

```text
clacks-sync ───────────────> clacks-model
future clacks-lotw ────────> clacks-model
future server library ───> clacks-sync + clacks-lotw + clacks-model
```

| Module | Ownership |
| --- | --- |
| `clacks-model::service::destination` | Public destination identities/descriptions and external keys. |
| `clacks-model::service::evidence` | Domain evidence/disposition/association payloads. |
| `clacks-model::service::lotw` | Data-driven normalization, normalized submissions, remote keys, service-specific applicability. |
| `clacks-model::service::qrz` | QRZ submission/ADIF projection and service-specific confirmation interpretation. |
| `clacks-model::service::applicability` | Pure publication-coverage and confirmation-applicability calculations consuming selected evidence. |
| `clacks-sync::service` | Admission/combination/recovery, dependency interpretation, association guards and coupled transition plans. |
| Existing synchronization modules | Envelopes, vectors, timeline/baseline interpretation, transfer, acknowledgment and collection. |
| Future `clacks-lotw::certificate` | Certificate/archive/key handling and signing eligibility. |
| Future `clacks-lotw::signing` | Signing specifications, protocol/signature-input serialization and cryptography. |

One shared normalized LoTW submission flows from accepted QSO, selected public
signing identity and explicit mapping data to comparison, retained attempt
values, remote key and signer. The signer does not independently remap the
original QSO. It owns protocol field order, certificate references and signature
bytes, validating that the certificate/key/specification can sign the prepared
values and failing rather than silently changing semantic values.

Certificate parsing supplies ordinary typed public information; certificate
library types/private keys do not enter `clacks-model`. The model does not
depend on synchronization or signing, and synchronization does not depend on
signing. Desktop model/sync consumers do not acquire signing dependencies.
Domain payloads do not import synchronization version stamps/vectors/checkpoints;
their synchronization carrier owns those references and saves/transfers them
atomically. Causal comparison stays in synchronization, not a second model clock.

Extend real production record domains and operations rather than build a
parallel demonstration. No catch-all service crate or empty future signing
scaffold is required. Use `service.rs` and named child modules, without `mod.rs`;
subdivide files as actual implementation warrants. Follow the workspace's
Rust 1.99.1, `lib/`/`bin/`, inherited Cargo metadata, standard serialization,
documentation and checked-error conventions. Exact Rust signatures and physical
storage encodings belong in the implementation plan without changing contracts.

## 14. Worked examples and required verification

### 14.1. Positive imported upload, no attempt history

Import Q1 with a recognized positive LoTW upload flag/date. Retain its imported
service baseline and provenance, not an invented sent request or remote ID.
Suppress resubmission immediately. A later service-relevant change is pending;
an unrelated comment/tag edit is not a LoTW payload change. No confirmation is
established merely by this upload assertion.

### 14.2. Positive imported confirmation, no counterpart report

Import Q2 with a recognized positive confirmation/date. It applies immediately
against imported contact identity. A later callsign/band correction makes the
old claim historical; a comment change leaves basic applicability intact.
A changed time requires the conservative rule in section 6. Missing upload
status remains pending independently; no own-upload history is fabricated.

### 14.3. Response after an edit

Attempt A1 sends V10 with own grid FN31. V11 changes it to FN32 before success
R1 arrives. R1 covers A1/FN31, leaving the current projection pending. If V11
only changes a field omitted from that service, its current projection may
already be covered. Version IDs alone cannot answer this comparison.

### 14.4. Duplicate, changed and incomplete downloads

O1 reports confirmed E1; its retained work establishes E1 → Q1 and records
completion. An identical overlapping download does not repeat matching or
effects. A known newer explicit unconfirmed O2 changes current status without
erasing O1. Missing confirmation fields/failure do not. If O3 returns O1's facts
after the observed O2 change, it is a distinct occurrence. Rejection follows
that reviewed version; deletion suppression still applies independently.

### 14.5. Interruption and dependency recovery

A crash after durable input/work but before processing leaves retryable pending
work. A crash after the atomic disposition/effect commit leaves completed work,
not a second creation. A crash before dispatch records at most intent; one
during the call may leave possible send. Never infer an external exactly-once
guarantee from the database transaction.

A recovered result missing its attempt stays incomplete. Installing its support
can establish coverage without resending. A disposition whose effects already
have covering successors does not restore old QSO values. Lost acknowledgments
allow repeat transfer without new effects; partial windows cannot certify a full
prefix. A pending local QSO edit retains its original/working values throughout.

### 14.6. Retirement, compaction and rollback

Revoking an actor blocks new automatic proposals under that identity, not
recovery of already accepted A1/R1 through another authorized client's server
connection. After the normal retention boundary, baseline projection may remove
redundant retired clock entries while preserving A1/R1 and interpretation.
Frozen projection P1 suffices without retaining every old QSO body.

After rollback, recovering A1/R1 covers current P1 but not corrected P2. Another
copy of A1/R1 adds no attempt or effect. A recovered old confirmed observation
does not replace known later unconfirmed evidence. If surviving context cannot
order them, retain uncertainty and refresh rather than guess or create a manual
provider-fact conflict. Follow operator pause/catch-up/resume for publication.

### 14.7. Validation obligations

Exercise production operations with native values and serialization/reload, not
only isolated evidence fixtures. Verify:

- Independent imported upload/confirmation baselines, missing source coverage,
  current-value coverage and service-specific changed/unchanged projections.
- LoTW mapping fallback, retained historical mapped values and shared signing
  input; QRZ canonical mode, locator/coordinate/text/reference transformations.
- Conservative time edits, primary/receive band differences, mode-group versus
  exact-mode qualification, explicit negative versus unknown applicability.
- A → A and A → B → A, partial coverage, stronger ordering-only evidence,
  late responses, same-ID mismatches and unordered recovery.
- Unique binding races/guards, explicit duplicate, supersession, changed-version
  rejection, deletion suppression across full queries, missing identity/support.
- Atomic transition groups, staged dependencies across batches, covering effects,
  local pending preservation and checkpoint completion after interruption.
- Distinct equal-payload attempts, batch outcome scope, missing results, retirement,
  compaction and rollback under existing timeline semantics.

Errors/conditional outcomes must identify relevant account, destination,
record/attempt/observation, failed guard or missing dependency, and available
provider phase/scope without leaking secrets. Distinguish readiness, integrity,
missing support, unknown ordering and stale acceptance from provider rejection.
Actual database crash/uniqueness tests and authenticated interoperability tests
remain required when those adapters exist; passing pure tests is not that proof.

## 15. Sources and verification limits

The accepted design was developed in the [RFC](https://github.com/jhelwig/clacks-log/issues/3#issuecomment-6280551376)
and its [module/observation continuation](https://github.com/jhelwig/clacks-log/issues/3#issuecomment-6308284405).
This document consolidates the resulting contract rather than requiring readers
to reconstruct it from the discussion.

- [LoTW submission interface](https://lotw.arrl.org/lotw-help/developer-submit-qsos/),
  [queries and fields](https://lotw.arrl.org/lotw-help/developer-query-qsos-qsls/),
  [corrections](https://lotw.arrl.org/lotw-help/submitting-qsos/?lang=en),
  [confirmation updates including receive band](https://lotw.arrl.org/lotw-help/updatingconfirmations/?lang=en)
  and [matching concepts](https://lotw.arrl.org/lotw-help/key-concepts/?lang=en).
- [Pinned TrustedQSL provenance](../references/trustedqsl/source-map.md), upstream
  commit `6ce824363e27da455c6cf6fd440b9c401945bfcb`, version 2.8.6/configuration
  11.34: [conversion](../references/trustedqsl/upstream/src/tqslconvert.cpp),
  [mapping/station/contact processing](../references/trustedqsl/upstream/src/location.cpp)
  and [configuration/signing definitions](../references/trustedqsl/upstream/src/config.xml).
  These are pinned compatibility findings, not a claim of latest upstream data.
- [QRZ API](https://www.qrz.com/docs/logbook/QRZLogbookAPI.html),
  [field/status meanings](https://www.qrz.com/docs/logbook30/adif-standard),
  [file-import duplicate behavior](https://www.qrz.com/docs/logbook30/adif-import),
  [confirmation matching](https://www.qrz.com/docs/logbook30/confirmations-how)
  and [field preservation](https://www.qrz.com/docs/logbook30/lossless-adif).
- [ADIF 3.1.8](https://www.adif.org/318/ADIF_318.htm), existing
  [service research](../references/service-state-model.md) and
  [publication recovery evidence](../references/publication-recovery.md).

The field sets, applicability, observation procedure, recovery/dependencies,
transaction groups, retention and module boundaries above are accepted choices.
No authenticated provider operations were performed as part of this design.
Live QRZ field processing/clearing/ambiguous replacement/ID continuity, LoTW
server acceptance and actual award qualification remain explicit interoperability
boundaries. They do not block implementation of honest evidence/uncertainty and
safe refusal. Backoff, raw-diagnostic retention and actual provider worker/storage
integration remain downstream work, not additional product decisions for #3.

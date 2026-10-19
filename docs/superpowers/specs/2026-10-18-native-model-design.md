# Native contact and shared-data model

**Status: implemented under the user-authorized native-execution plan, 18 October 2026; whole-branch review and regression fixes are complete; integration remains a separate choice.** This
consolidates the accepted issue #1 discussion and decisions through D255. It is
the logical specification; [execution evidence](../../native-model-execution.md) records delivered library behavior and downstream boundaries.
The behavioral requirements below are accepted. Logical field/type names and
the engineering choices explicitly identified in section 2 are proposals for
review in this document, rather than claims of earlier approval.

A radio contact is a QSO. Minimum viable product is abbreviated MVP. Amateur
Data Interchange Format (ADIF) is an interchange standard; its text format is
ADI. Coordinated Universal Time is UTC. Parks on the Air is POTA. Logbook of
The World is LoTW. DX Century Club is DXCC. CQ magazine and the International
Telecommunication Union (ITU) define the two zone systems used below.

## 1. Purpose, scope and authority

[Issue #1](https://github.com/jhelwig/clacks-log/issues/1) establishes the native
model used by offline logging, storage, synchronization and later integrations.
Success means the same recorded facts retain their identity and meaning across
clients, catalog/recognition changes, profile changes and interchange boundaries.
The model must cover contacts, logbook tags, shared station snapshots, saved
profiles, reusable equipment and data-driven participation/reference groups.

The [decision register](../../decisions.md), [product requirements](../../product.md)
and the user's explicit corrections are authoritative. This specification
consolidates their native-model consequences. It replaces the working
[brainstorming notes](../../brainstorming/issue-1-native-model.md) as the design
artifact to review. In particular:

- A saved contact needs **frequency or band**, not an independently populated
  band. This corrects the earlier D112/D166 wording.
- Incomplete entry/import preparation does not create a saved QSO. Validate
  before saving. This is established workflow, not a new record/draft subsystem.
- Native modeling is independent of ADIF's field layout. ADIF is neither a
  completely lossless round trip nor the native backup format.
- There is one account synchronization scope. Tags organize account-owned QSOs;
  they do not own child contact records.
- Dedicated contest exchanges and serial-number assistance remain separate
  from participation/reference associations and remain post-MVP.

This document specifies values and behavior, not physical database tables,
synchronization messages, an entry interface, a catalog downloader or a complete
ADIF importer/exporter. Those implementations have their own backlog items.
Rust 1.99.1, standard Serde serialization, libraries in `lib/<crate-name>` and
thin applications in `bin/<application-name>` remain selected. This document
does not select dependency versions or create a workspace.

## 2. Engineering proposals made by this specification

The following choices make the logical design concrete for review. They must
not be backdated as individually approved conversation decisions:

1. Use distinct ID types for contacts, tags, profiles, equipment configurations,
   groups, catalogs and output profiles. Use Universally Unique Lexicographically
   Sortable Identifiers (ULIDs) for newly created native entities, following the
   user's preference for identifiers of this kind. Identity never depends on
   a display name or timestamp order. Exact Rust library and wire encoding are
   implementation-plan work.
2. When coordinates require rounding to integer millionths of a degree, use
   nearest with exact halfway values rounded away from zero. Validate the
   supplied geographic range before rounding so rounding cannot turn an
   out-of-range input into a valid point. This supplies the previously
   unspecified halfway/validation-order detail.
3. Express group normalization, syntax, catalog bindings and output mappings
   through the bounded declarative operations in section 9. There is no need
   for arbitrary scripts or a general multi-field reference form system.
4. Allow equipment description and antenna description to be independently
   absent in `EquipmentValues`, using the accepted optional-text normalization.
   D72 selects these fields but does not separately settle their requiredness;
   the optionality shown here is a proposal for review, not a new claimed
   prerequisite or previously approved rule.

All other concrete names below describe the logical schema. Physical layout,
crate boundaries and transport/storage encoding can differ while preserving
the specified values, comparisons and behavior.

## 3. Ownership, identities and record boundaries

| Record | Ownership and relationships |
| --- | --- |
| QSO | Account-owned; contains copied station facts and its complete tag set. |
| Logbook tag | Account-owned stable ID, display name and independent automatic-upload policy for each service. |
| Saved location profile | Account-owned named station snapshot with an optional default equipment-configuration reference. |
| Equipment configuration | Account-owned reusable named equipment values, independent of profiles. |
| Reference-group definition | Account-owned shared data, including definitions initially seeded at account creation. |
| Reference catalog | A separate reference dataset associated with a group through an optional binding; its acquisition/distribution is outside this specification. |

Names do not establish identity. Duplicate tag names are explicitly allowed.
No uniqueness requirement for other display names is introduced here. IDs
created offline remain the same when synchronized. Account isolation applies
to all account-owned records and their references.

A QSO's identity is not a callsign/time/band tuple. Distinct contacts can have
equal facts. Import candidate matching is not a uniqueness constraint on the
native contact store. Reordering a tag or reference set is not an edit.

Deletion is a state considered during record comparison; it is not an active
record with all its values cleared. The synchronization design specifies its
physical representation and retained evidence. This model does not introduce
a per-edit history or define causal metadata as ordinary contact fields.

## 4. Contact schema and saved-record invariants

```text
Qso
  id: QsoId
  start: UtcSecond
  end: optional UtcSecond
  own_station: StationSnapshot
  contacted_station: StationSnapshot
  own_context: OwnOperatingContext
  contacted_operator: optional OperatorInformation
  primary: BandFrequency
  receive_override: optional BandFrequency
  mode: Mode
  sent_report: optional Text
  received_report: optional Text
  comment: optional Text
  tags: set<TagId>

StationSnapshot
  callsign: RecordedCallsign
  location: optional RecordedLocation
  references: set<ReferenceAssociation>

OwnOperatingContext
  operator: optional OperatorInformation
  transmit_power: optional Milliwatts
  equipment: optional EquipmentValues

OperatorInformation
  callsign: optional RecordedCallsign
  recorded_name: optional Text

BandFrequency
  frequency: optional Hertz
  band: optional Band
```

Both station callsigns, start, mode, and at least one of primary frequency/band
are required to save a QSO (D112/D166/D252). An included receive override must
likewise contain a band, frequency, or both. An empty receive structure is
absence, not a third receive state. Neither a known parent mode nor a recognized
band label is an additional requirement.

End is optional and must be at or after start. Equal start/end and contacts
crossing midnight are valid. No minimum or maximum duration is introduced.
Reports, exact frequency when band is known, location, power, operator
information and equipment remain optional. Local validity and a service's
upload/submission readiness are separate checks.

Each station uses the same snapshot structure. Our operator, power and
equipment are captured separately in our operating context. Operator callsign
and recorded name are independently optional on either side; do not infer an
operator callsign from a station callsign. An operator structure with no values
is equivalent to absence. Directory names remain provider facts.

Incomplete manual input and read-only import preparation are not saved QSO
records. New historical contacts missing our callsign require explicit
historical assignment; never substitute current operating defaults. Failed
records are reported without preventing other valid records from importing.
Existing-contact updates cannot clear either required callsign.

## 5. Scalar values, normalization and limits

| Value | Representation and meaning |
| --- | --- |
| Recorded callsign | Trim surrounding whitespace and uppercase. Preserve prefixes, suffixes and slash components. |
| Contact time | Full UTC timestamp at whole-second resolution; no separate timestamp-precision metadata. |
| Exact frequency | Positive integer hertz; whole-hertz resolution. |
| Transmit power | Positive integer milliwatts; absence means unknown. Display/export can express watts. |
| Latitude/longitude | Signed integer millionths of a degree; six decimal places of degree resolution. |
| Maidenhead locator | Supported length 2, 4, 6, 8, 10 or 12, retaining the supplied/selected length. |
| Ordinary optional text | Empty/whitespace-only becomes absent; otherwise preserve supplied text, including case, accents and spacing. |

Do not add arbitrary short text limits, nanodegree coordinates or fictional
numeric accuracy. Coordinate resolution is not a claim of measurement accuracy.
Storage types must cover the specified coordinate ranges and report numeric
overflow instead of wrapping. Longitude/latitude use their ordinary geographic
ranges. The plan must verify extrema and grid-boundary arithmetic explicitly.

Callsign lookup-key normalization is separate from recorded callsign
normalization. For example, a recorded `OK2CQR/P` remains `OK2CQR/P` even when
the directory is queried for `OK2CQR`. Detailed portable-call lookup rules
belong to directory integration.

Signal reports remain optional text supporting phone, Morse code and digital
modes such as FT8. They are not limited to one numeric format and are distinct
from contest exchanges.

Import omission is an instruction to leave a value unchanged; an explicitly
empty supported field can be offered as a clear under D42. Do not collapse
those instructions by normalizing optional text too early. That instruction
distinction belongs to import application, not extra persistent QSO states.
Invalid optional import values produce warnings, are omitted on creation and
leave existing values untouched on update under D167.

## 6. Positions and geographic details

Use one optional position type for saved profiles/current operating locations
and both sides of recorded QSOs:

```text
Position
  GridArea(locator)
  ExplicitPoint(latitude_microdegrees, longitude_microdegrees, grid_length)

RecordedLocation
  position: optional Position
  dxcc_entity: optional geographic identifier
  primary_subdivision: optional geographic identifier
  secondary_subdivision: optional geographic identifier
  cq_zone: optional zone value
  itu_zone: optional zone value
```

These location fields describe the already-required geographic concepts:
DXCC entity, state/province, county, CQ and ITU zones. Geographic identifier
encodings and dataset compilation must interoperate with the selected datasets;
this logical schema does not turn a display name into a database identity or
add an address/contact-information subsystem.

`GridArea` stores the locator and derives its center coordinates. `ExplicitPoint`
stores the point and grid length and derives the containing locator. Do not
store independent contradictory authoritative grid/coordinate values or feed
a derived grid center back into an explicit point. Other recorded location
details may exist without a position.
An empty location structure is absence: location presence requires a recorded
position/geographic value or a location-bearing reference, not merely an
allocated optional structure.

User entry of our operating location requires at least a four-character grid;
the initial selected grid length is six. Historical imports can retain
two-character locators as coarse areas, including for our station. All supported
lengths through twelve remain available; do not pad imported locators.

Editing grid selects the area form. Editing coordinates selects the point form
and retains the selected grid length. Imported complete valid coordinates select
the point form, retain a valid supplied grid length and correct an inconsistent
grid with a warning. Grid-only input selects the area form. Without a supplied
valid grid length, use the selected/default length. Invalid optional location
input follows D167.

For offline geographic lookup, use the area for a grid-derived position and
the point for explicit coordinates. Populate unambiguous fields and allow
manual selection for ambiguous fields; the center of an area does not resolve
its geographic ambiguity. Preserve manual corrections while the position is
unchanged. A changed position populates fresh values; a grid-length-only change
to the same explicit point does not erase corrections (D77/D78/D184).
Saved profiles and QSO snapshots retain their recorded geographic values;
dataset refreshes do not silently rewrite historical contacts.

Recorded references whose group declares location context count as recorded
location information even when geographic details are unknown. They suppress
lower-priority provider-location fallback as a whole. Do not fill a park
contact's unknown location with a directory home location or a catalog park
center. Simultaneous multi-park references on one station side describe the
same operating position; they do not create positions or native QSOs per park.

## 7. Band/frequency and mode interpretation

### 7.1 Band/frequency authority

An exact supplied frequency determines the band when recognized. Editing only
band clears that pair's exact frequency; the derived band update caused by
entering frequency must not clear the frequency again. Import retains usable
frequency and derives/corrects band with a warning when needed. Never invent a
representative frequency from a band label. A frequency outside recognized
ranges remains valid without requiring a band.

An absent receive override means the primary pair is used in both directions.
A present override supplies receive information as a whole. A band-only receive
override does not borrow primary frequency, even for the same band. Unknown
primary frequency remains unknown for receive when there is no override.

The initial recognition baseline is the complete
[ADIF 3.1.8 band catalog](https://www.adif.org/318/ADIF_318.htm#Band_Enumeration).
Retain unfamiliar band labels through a text-preserving fallback. Recognition
changes alone affect interpretation, not underlying saved values or edits.

### 7.2 Mode value

Use one native partially hierarchical Rust enum with typed known families and
specializations and a structured `OtherMode` fallback (D177). It must represent:

- A known parent with no specified specialization.
- A known specialization with its family.
- Unfamiliar mode-only information.
- Unfamiliar submode information with optional supplied parent context.

There is no both-absent mode. Preserve unfamiliar submode information even when
the parent is absent. A recognized submode at the ADIF boundary determines its
parent and corrects inconsistencies with warnings under D174/D175. Do not infer
an unrecorded submode from frequency or operating conventions.

Use the complete [ADIF 3.1.8 mode/submode catalog](https://www.adif.org/318/ADIF_318.htm#Mode_Enumeration),
including deprecated labels for historical imports, as the initial recognition
baseline. It is not a two-column native schema or a requirement to implement
all ADIF fields. For example, supported equivalent encodings of FT4 resolve to
the same native specialization, while an unknown submode retains its own value
and any supplied parent context.

Recognition through a known variant or `OtherMode` must not change identity.
Keep the underlying classification/context intact on an older client and during
unrelated edits. Ordinary Serde encoding and equality must preserve this
contract; native enum discriminant order is not a wire identity. Do not design
a custom serialization protocol to achieve it. Exact serialization configuration
is implementation-plan work, with the cross-recognition tests in section 13.

## 8. Participation/reference associations

```text
ReferenceAssociation
  group_id: GroupId
  identifier: optional Text
```

There are two unordered, deduplicated sets: our station's references and the
contacted station's references. There is no contact-wide reference scope.
Identity within one side's set is group ID plus optional identifier. Repeated
identical group-only participation adds nothing; different groups with equal
identifier text remain distinct. The same reference on opposite sides remains
two station-side facts. Multiple contest groups and multiple references in a
group are permitted.

Each contest/event is itself a group, a peer of POTA, with its own definition.
Its ID identifies the activity; it is not an identifier inside a universal
Contests group. A definition can require an identifier or allow group-only
participation. Preserve unrecognized group associations and supplied identifiers
without guessing their properties. Later recognition interprets them without
rewriting the QSO or changing its equality.

References are not contest exchanges. A contact serial, precedence/category,
licensing-year check or other exchanged information does not become part of
reference identity. The [rules survey](../../../references/contest-exchanges.md)
and [logger survey](../../../references/contest-logger-data.md) motivate later
structured exchanges and serial assistance; they do not add them to MVP.

## 9. Group definitions and optional catalogs

### 9.1 Logical schema

```text
GroupDefinition
  id: GroupId
  name: Text
  identifier:
    required: Boolean
    normalization: list<TextNormalization>
    syntax: optional FullMatchRegex
  bears_location_context: Boolean
  catalog: optional CatalogBinding
  export_mappings: GroupExportMappings

CatalogBinding
  dataset_id: CatalogId
  lookup_key: ValueExpression
  display_name: MetadataField
  details: list<MetadataDisplayBinding>

MetadataDisplayBinding
  label: Text
  field: MetadataField

ReferenceCatalog
  id: CatalogId
  provenance: SourceInformation
  entries: map<ReferenceKey, CatalogEntry>

CatalogEntry
  metadata: map<Text, MetadataValue>

SourceInformation
  dataset_name: Text
  publisher: optional Text
  source_url: optional Url
  obtained_at: UtcTimestamp
  upstream_version: optional Text
```

The catalog ID identifies the dataset. `obtained_at` and an available publisher
revision describe the obtained copy. Do not add `ReferenceCatalog.version`, a
second local snapshot-version scheme, or per-entry copies of source information.
`MetadataValue` holds ordinary structured descriptive data, including text,
numbers, booleans and collections. It does not become a QSO field schema.

Seed default definitions into each new account, then synchronize them as ordinary
account-shared records. Clients use locally synchronized definitions offline.
User-defined groups must have full capability parity with defaults. There is
no privileged built-in interpretation path, POTA enum variant or program-name
switch. MVP does not include user management of group definitions.

Apply identifier normalization and syntax validation at entry/explicit edit.
Do not rewrite existing associations because definitions change or arrive.
An optional catalog supplies names, status and other descriptive lookup data;
syntax and catalog findings remain distinguishable. Catalog metadata stays
separate from recorded QSO data and does not assert the actual station point.
The existence of catalog support does not select its downloader, refresh policy,
distribution mechanism or an additional group-management interface.

### 9.2 Proposed declarative operations

| Operation | Meaning |
| --- | --- |
| Text normalization | An ordered list of generic trim/case-conversion operations. |
| Full-match regex | Data-defined identifier syntax, with named captures available for extraction. |
| Value expression | Literal, existing QSO field, group identifier collection, selected reference identifier, regex capture or joined collection. |
| Catalog metadata field | A descriptive lookup/display attribute; not an implicit source of recorded station facts. |
| Field binding | Output field name, value expression, optional presence condition and supported collection encoding. |
| File partition | Partition by an existing value or each reference on a specified station side within the selected group. |
| Entry expansion | Iterate a side's references within the selected group; specify retaining one entry when the set is empty. |

Bindings for `this group` resolve through its ID. Paths and captures must be
defined by the mapping, not opaque functions such as `potaParkIdentifier()`.
For a qualified identifier, a regular-expression capture can supply the base
reference and another the supplied qualifier. A catalog lookup can use the base
reference without rewriting the recorded identifier. Location output requires
an explicit mapping consistent with recorded source/location rules; a catalog
center cannot silently satisfy it.

Malformed expressions, regexes, missing capture names and incompatible output
bindings produce definition/configuration diagnostics. Unknown groups retain
their recorded values; they are not evidence of a malformed user QSO. Exact
regex library and expression encoding are implementation choices to review in
the plan. Catalog collection/refresh and complete program mapping fixtures are
separate integration work, not new native contact fields.

## 10. Export context and mapping contracts

```text
ExportContext
  output_profile_id: OutputProfileId
  target_group_id: optional GroupId

GroupExportMappings
  ordinary_interchange: list<FieldBinding>
  targeted: map<OutputProfileId, GroupExportPlan>

GroupExportPlan
  fields: list<FieldBinding>
  file_partition: list<PartitionRule>
  entry_expansion: list<ExpansionRule>
```

The output profile supplies common QSO field mappings and output requirements.
The group contributes reference mappings and, for a targeted profile, its
group-specific plan. Field names and structure above are the proposed schema;
the following context selection behavior is accepted under D255.

### 10.1 Ordinary ADIF

Without a target, use ordinary interchange mappings, including supported lists,
and emit one entry per selected QSO identity. Do not activate program-specific
entry expansion. Emit `CONTEST_ID` when that QSO has one distinct applicable
contest identifier. If it has several, surface the ambiguity during preparation:
choose a contest context or proceed with that ambiguous field omitted and
reported. Contacts each associated with different single contests remain
individually unambiguous.

With an explicit contest target, that contest determines `CONTEST_ID`; other
contest associations do not compete. Do not infer new participation merely
because a target was selected. A target controls mappings, not a silent change
to the selected contact set. Ordinary reference information still follows the
ordinary interchange mappings. Conflicting assignments within the selected
definition are configuration errors reported before writing output.

Do not replace these rules with a blanket per-binding `omit_with_warning`
collision policy. This specification does not choose the import-side ownership
of ADIF `CONTEST_ID`; that pending mapping decision belongs to import design.
It does not change the native two-side association model.

### 10.2 POTA mappings as a worked example

| Aspect | Ordinary ADIF mapping | POTA-compatible mapping |
| --- | --- | --- |
| Our references | `MY_POTA_REF`, supported comma-separated list | Literal `MY_SIG=POTA`; the partition's selected park in `MY_SIG_INFO`. |
| Contacted references | `POTA_REF`, supported comma-separated list | When present, literal `SIG=POTA` and the entry's selected park in `SIG_INFO`. |
| Files | No reference-driven partitioning | Partition by our park and activation location; preserve correct station identity. |
| Entries | One per QSO | One per contacted park per applicable file; retain one entry when there is no contacted park. |
| Location and common fields | Ordinary profile mappings | Explicit submission mappings, including applicable `MY_STATE`, and submission validation. |

The [POTA technical reference](https://docs.pota.app/docs/activator_reference/ADIF_for_POTA_reference.html)
and [park-to-park guide](https://docs.pota.app/docs/activator_reference/park_2_park.html)
describe submission behavior beyond standard ADIF reference fields. The
[interchange research](../../../references/adif-matching.md#standard-interchange-versus-pota-submission-mappings--checked-18-october-2026)
records that distinction. One native two-park-to-three-park contact produces
two output files with three entries each for the two-park operator. It remains
one native QSO. The target's operations do not multiply unrelated event
associations or alter recorded callsigns/times. No automatic POTA upload is
selected by this file-export feature.

Submission readiness does not change local save validity. For example, native
frequency-only data can remain valid while an output profile needs a band.
Profile-specific field requirements and validation report that output issue
without fabricating a band. Catalog metadata is not an implicit repair source.
Exact qualified-reference transforms, complete field mappings and their
fixtures are part of implementing the interchange contract, not new native
schema approval questions.

Exports read consistent saved local working state, include unsynchronized and
unresolved working values, exclude pending deletions, and retain existing
destinations on cancellation/failure (D210/D211). Report source contact counts
separately from generated entries/files. Preparation and export do not mutate
recorded associations or resolve synchronization conflicts.

## 11. Profiles, equipment and logbook tags

### 11.1 Profiles and current defaults

```text
SavedLocationProfile
  id: ProfileId
  name: Text
  station: StationSnapshot
  default_equipment: optional EquipmentConfigurationId

EquipmentConfiguration
  id: EquipmentConfigurationId
  name: Text
  values: EquipmentValues

EquipmentValues
  equipment_description: optional Text
  antenna_description: optional Text
  notes: optional Text

LogbookTag
  id: TagId
  name: Text
  automatic_upload:
    lotw: Enable | Forbid | Neither
    qrz: Enable | Forbid | Neither
```

A saved profile requires a callsign. Loading/reloading replaces the current
own-station snapshot as a whole, clearing absent values and replacing its
reference set. Preserve the saved values/geographic corrections rather than
leaving data from the previous location. Operator information and power are
excluded from profiles and stay unchanged.

If a profile has a default equipment configuration, copy its equipment values
into current defaults. If it has none, leave current equipment unchanged.
Equipment is independently reusable. QSO snapshots copy values without the
configuration display name or a requirement to retain its ID. Equipment edits
and deletion do not rewrite copied current defaults or contact snapshots.
Power is separate; there is no dedicated amplifier field.

Current operating defaults are client-local and persistent; saved profiles and
equipment synchronize. Explicit reload is required after saved-profile changes
or conflict resolution. Retain last-loaded profile values/revision as the
comparison baseline for distinguishing local adjustments from newer saved
profile values. Offline, use the latest profile state available to the client.
This does not require a global history graph. Logging requires neither a saved
profile nor an operating-session lifecycle. Unfinished manual drafts are not
restart-persistent records.

Before deleting referenced equipment, show affected profiles and require explicit
reassignment or removal of the optional reference (D73). Pending profiles that
refer to deleted equipment follow D183's explicit resolution; unrelated sync
continues. Restore/clone resolution remains post-MVP.

### 11.2 Tags and contact deletion

A QSO may have zero or more tags. The open logbook supplies default membership
for new contacts and filters normal history/Worked Before; it does not change
server work or account-wide synchronization. Individual and selected-group
membership changes are MVP. Removing the last tag leaves an active untagged
QSO. Offline tag creation, naming, rename and deletion retain stable identity.
Tag names are not event references or station profiles.

Both service policies default to Neither. Any associated Forbid wins;
otherwise at least one Enable grants automatic-upload eligibility for that
service. All-Neither and untagged QSOs have no automatic grant. Manual upload,
service readiness, routing and pauses retain their separate rules.

Tag deletion offers leave all QSOs, mark all associated QSOs deleted, or mark
only those unique to the tag deleted. Show the number also tagged elsewhere;
count distinct contact identities. Preserve shared active contacts and remove
the deleted tag where appropriate (D244). Marked-deleted QSOs are hidden across
normal views, not converted into active untagged contacts. A pending QSO with
a deleted-tag reference remains local until explicit removal/replacement of
that membership (D250); unrelated synchronization continues. Native contact
deletion leaves external copies untouched and retains suppression evidence.

## 12. Source separation, equality and component interfaces

Recorded QSO facts, provider observations and catalog metadata have distinct
meaning. For supported effective values, retain the accepted precedence:
recorded QSO, eligible LoTW, eligible QRZ, eligible directory snapshot. Select
location as a whole, never patching gaps from lower-priority sources. A recorded
location-bearing reference establishes recorded location presence. Names also
remain separate from directory-provided names. Fallback changes an effective
view/output, not the stored recorded snapshot.

Directory snapshots retain provider, normalized lookup key and successful
lookup time. Latest successful lookup wins with a deterministic tie-breaker;
concurrent refreshes do not require user resolution. Key changes immediately
stop old-key use. Failure/not-found preserve existing successful data, subject
to key eligibility; cancellation preserves completed updates. This document
does not choose every supported provider field or redefine service-evidence
records, which are the responsibility of issue #3 and provider integration.

The logical components and their contracts are:

| Component | Responsibility and dependency boundary |
| --- | --- |
| Native values and records | Own the schemas/invariants above; do not require a running database, network, UI or service account. |
| Recognition and definition interpretation | Interpret preserved mode/band/reference values using catalogs/definitions without rewriting them. |
| Snapshot/default operations | Copy station/equipment values and apply explicit profile-load behavior; do not update historical QSOs. |
| Effective-value selection | Read separate eligible sources under the accepted precedence and whole-location rule. |
| Reference mapping evaluation | Evaluate declarative definitions against an export context without changing QSO facts or performing catalog acquisition. |
| Storage, sync and integrations | Consume these contracts; supply durability, causal evidence, provider records, adapters and diagnostics through their own specifications. |

These are responsibilities, not a requirement for a crate per row or separate
deployed services. Applications remain thin wrappers around libraries.

For equal-identity record comparison, use recorded normalized values. Callsign
case/outer whitespace normalization and optional-text absence rules apply;
other preserved text differences remain differences. Coordinate equality uses
integer values; area versus point remains significant even with equal center
coordinates. Locators compare by their cell and retained length; letter case
does not identify a different cell. Tag/reference sets compare without order. Mode/band interpretation,
catalog refreshes and derived display values do not enter QSO equality as edits.
Eligible provider changes can change effective output without rewriting facts.

All recorded QSO fields, including the complete tag set, remain subject to
whole-record three-way reconciliation (D136/D137/D243). Equal results and
one-sided changes bypass resolution. Differing changes on both sides can
require resolution even for disjoint fields/tags. Set equality does not select
automatic set merging. Profile and equipment records use their existing
record-level workflow. Pending/conflict indicators and causal comparison
evidence are not new on-air contact facts. Detailed reconciliation and recovery
belong to issue #2, not a new native-record revision graph.

## 13. Acceptance and verification criteria

The implementation plan must provide focused verification of the following
contracts; this table is not a claim that tests or implementation exist.

| Scenario | Required result |
| --- | --- |
| Frequency only, band only, both absent | The first two can save; both absent cannot. Unclassified frequency remains usable. |
| Band edit, frequency edit and mismatched import | Apply the respective authority rules without clearing frequency through feedback. |
| Receive absent, band-only override, frequency override | Primary fallback only when absent; never borrow primary frequency into a present override. |
| Equal times and midnight crossing | Save valid full timestamps; reject end before start without inventing a correction. |
| Missing own callsign in historical creation | Require explicit assignment; do not save a placeholder or use live defaults. |
| Two-character import versus own-location entry | Retain the imported area; enforce four-character minimum for operating-location entry. |
| All supported locator lengths | Preserve length, center/containment semantics and explicit-point coordinates. |
| Coordinate bounds, rounding and grid boundaries | Deterministic integer results, correct range rejection and no conversion feedback. |
| Grid area crossing geographic boundaries | Populate only unambiguous details; do not infer an exact point from its center. |
| Complete recognized and unfamiliar modes/bands | Full baseline coverage and retained unfamiliar values/context. |
| Recognition differs between clients | Round trip and unrelated edits preserve underlying value/equality without a new QSO edit. |
| Optional text and import instruction distinctions | Preserve nonblank text; omission and selected clears remain distinct at application. |
| Operator name without operator call | Preserve it without fabricating an operator callsign. |
| Reference/tag order or repeated identical association | No value change; opposite station sides and different groups remain distinct. |
| Multiple event groups and multi-park reference sets | Preserve one QSO and both station-side sets without contact-wide reference fields. |
| Default and an equivalent user-defined group | The same declared capabilities produce the same interpretation/mapping behavior. |
| Catalog/name/location metadata refresh | No recorded association, position or QSO equality change. |
| Profile load with absent values/equipment default | Replace station snapshot; preserve operator/power and preserve equipment only when no default is selected. |
| Profile/configuration change after logging | Current defaults change only through the agreed explicit operation; historical copies remain intact. |
| Tag removal/deletion and service policy combinations | Distinguish untagged from deleted; apply the three deletion choices and Forbid precedence. |
| Provider fallback with partial recorded location | Use the selected whole source; do not mix directory/catalog geographic fields into it. |
| Concurrent recorded fact/tag edits | Retain whole-record resolution triggers, including disjoint edits; no special set merge. |
| Ordinary export, single/multiple contest associations | Per-QSO unique context succeeds; ambiguity requires selection or reported omission under D255. |
| Targeted mapping conflict | Report a definition error before publishing output; no arbitrary contest selection. |
| Two own parks and three contacted parks | POTA-compatible output has two files with three entries each; ordinary output remains one QSO entry. |
| Missing contacted park | Targeted expansion retains the hunter contact rather than producing zero entries. |

Check serialization preservation using representative old/new recognition
views, not only encode/decode on one current catalog. Check copied snapshots
after source profile/equipment mutation and deletion. Use positive examples,
invalid boundaries and the accepted whole-record conflict cases. Provider
interoperability, full import/export mapping fixtures, large-log performance,
database migrations and deployed recovery are validated in their downstream
capabilities rather than declared complete here.

## 14. Handoff and review boundary

This is the consolidated design underpinning the
[implementation plan](../plans/2026-10-18-native-model.md), written at the user's request.
Review the two together before execution. Issue #1 stays open until its implemented capability and verification
evidence exist. No roadmap phase is marked complete by this document.

Import-side `CONTEST_ID` station assignment is not approved or implemented;
settle it in import mapping work. Full external service history/associations
remain issue #3. Catalog acquisition, update/distribution and detailed export
adapters are downstream work, with optional catalog support and generic group
parity preserved. Physical storage, sync protocol/recovery and concrete crate
composition are not silently prescribed by the logical schema.

The implementation plan supplies the concrete files, interfaces, task ordering
and verification steps. The user subsequently authorized native execution.
See the execution evidence for checks and integration status.

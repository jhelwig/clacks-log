# Amateur Data Interchange Format (ADIF) candidate matching

Accepted planning direction, updated 17 October 2026. D39-D40 select update/duplicate/skip and shared field-level resolution; D41 selects the candidate matching rules below. D42 selects missing/empty-field handling for updates; D225 selects possible matches for incomplete identity information and D226 selects their wider time window.

## Format evidence

The [ADIF specification](https://www.adif.org/316/ADIF_316.htm) allows Coordinated Universal Time (UTC) times with either minute or second precision (`HHMM` or `HHMMSS`). Missing seconds therefore must not be treated as proof that the radio contact (QSO) occurred at second zero. The specification also supplies station-callsign fallback from OPERATOR when STATION_CALLSIGN is absent. Applying that standard interpretation uses supplied historical data; it does not imply filling an unknown station identity from current defaults.

## Accepted candidate matching (D41)

Compare worked callsign, historical station callsign, band, normalized mode/submode, and UTC start date/time. Callsign suffixes remain significant. Location, reports, and other editable details are compared after candidate identification so their differences do not prevent an update being offered.

Use the time precision actually supplied: a minute-only time can identify candidates within that minute, while two second-precision times are compared at second precision. This is Clacks Log's accepted candidate-matching policy, not an ADIF-mandated identity rule. D225 handles incomplete identity information as possible matches requiring review, as described below. D226 adds a wider window only for possible matches requiring explicit selection. Explicitly inconsistent identity fields remain separate from missing information.

Never arbitrarily choose among multiple candidate local QSOs. Show candidates for explicit selection before update; retain duplicate/skip choices. If station identity is unknown, do not silently pick one using current defaults. A candidate with differing values uses D39-D40's existing interface, including the option to create a separate QSO.

Under D225, compatible but incomplete identity information produces possible matches, not confident matches. For example, an incoming record lacking our historical station callsign can be a possible match to an otherwise matching contact; a broad mode can be a possible match to its known specialization. Explain what information is missing and require explicit selection before updating or treating the record as unchanged, including when there is only one candidate. Preserve create-separate-contact and skip choices. Missing information is neither a contradiction nor proof of identity; explicitly different values are not missing information. Never infer historical station identity from current defaults. This review requirement qualifies D165’s automatic unchanged skip: it applies automatically only to confident matches.

Under D226, include otherwise compatible contacts within 30 minutes either side of the incoming start time as possible matches across the account under D247. Compare full UTC dates/times, so this window can cross midnight. Require explicit selection even for one possible match; a unique nearby contact does not become a confident match. Retain D41’s precision-aware confident-match rules and D165’s automatic unchanged skip only for confident matches, alongside update/create-separate-contact/skip choices. Explain time differences as well as incomplete identity information under D225. Exact boundary treatment for minute-precision values remains specification work under these precision-aware principles.

For context, the [LoTW matching rules](https://lotw.arrl.org/lotw-help/frequently-asked-questions/#datamatch) allow start times within 30 minutes, [QRZ confirmation rules](https://www.qrz.com/docs/logbook30/confirmations-how) allow a 30-minute difference, and the [eQSL matching FAQ](https://www.eqsl.cc/qslcard/FAQDisplay.cfm?Topic=1058) describes a one-hour allowance. These services compare the two operators’ accounts of a contact. Their tolerances do not establish local deduplication identity or justify automatic application of a possible import match. D226 explicitly selects the review-only 30-minute import window.

Apply these ADIF import candidate rules to all account QSOs, including untagged contacts (D247). Apply the selected tag when reusing a matched QSO; tag changes are ordinary QSO edits under D243. Preserve Update/Duplicate/Skip and explicit ambiguous/possible selection. Skip changes neither facts nor tags. Report added memberships separately from unchanged contact facts. Unassociated Logbook of The World (LoTW)/QRZ downloads use the same confident/possible distinction with account-wide search and established-association precedence (D227). Each external record has at most one associated local QSO within the account/service destination; ambiguous matching selects at most one (D251), as described in [download reconciliation](service-reconciliation.md). Local candidate rules are distinct from provider confirmation matching; candidate-matching fields are not a uniqueness constraint on the QSO store. Distinct contacts may share those fields while external-record associations remain unique under D251.

D165 selects automatic skipping of confidently matched records that add no information after mapping/normalization. Under D247, adding a missing selected tag is a change even when contact facts are unchanged; skip the whole record only when neither facts nor membership add information. Still open: exact compatibility tables for incomplete input under D225, precision-aware window boundary details under D226, implementation of the accepted complete ADIF 3.1.8 recognition baseline and matching of unrecognized classifications under D174/D175, and detailed repeated-import/report mechanics.

## Accepted missing/empty-field handling for updates (D42)

A field omitted from the incoming record supplies no update; a present, explicitly empty field appears as an optional clear in the shared field-level comparison, requiring user selection. ADIF permits zero-length values, but does not make every empty field a deletion command; field-specific standard defaults and validation still apply. Standard defaults are distinct from the current client defaults excluded for historical imports by D38. Exact field-specific default interpretation remains to be specified.

## Accepted partial import behavior (D43)

Allow partial imports, with a clear report identifying every record not imported and why. Usable records can proceed while records that fail validation remain unimported. Do not silently invent missing facts or silently omit failed records. Duplicate/update choices remain governed by D39-D42.

Missing optional historical DE (our station) context is permitted by D38. Under D252, creating a contact requires both station callsigns; if our callsign is missing, require explicit historical callsign assignment before creation, without silently using current defaults. Contact importability and readiness for a particular publication service are separate checks. D166 selects minimum information for new imported contacts below. D167 selects warnings and omission of invalid optional values, preserving existing values on update. Correction/retry interaction remains open.

The [American Radio Relay League (ARRL) submission guidance](https://lotw.arrl.org/lotw-help/developer-submit-qsos/) identifies callsign, date, start time, band, and mode as core contact data; frequency can supply band in its submission path. This is useful context, not an automatic decision to make every LoTW signing requirement a local import requirement.

D168 separates structural parsing errors from reliably parsed invalid field contents. Under D169, perform parsing during read-only preparation, before any logbook changes. Stop preparation on a structural error; the preceding reliably decoded complete records remain eligible for later application under the existing validation/matching choices, and the remainder is reported as unprocessed. Identify the error and its location; do not guess where parsing resumes or claim a record count for an undecodable remainder. Damaged field lengths are structural errors, not optional-value warnings. D166/D167 still handle invalid contents in reliably parsed records. No heuristic structural recovery is required for MVP; an in-application source-file correction workflow remains unselected.

## Frequency and band normalization (D164)

When a usable frequency is supplied, derive the band from it before candidate matching. If a supplied band disagrees, retain the frequency, use the calculated band, and accept the otherwise valid record with a warning identifying the record, supplied band, and calculated band. Count this as accepted with a warning rather than a failed import. A missing band can be derived from frequency; a band-only record remains eligible under other validation rules. Do not clear imported frequency through the manual band-edit interaction. Unusable frequencies and other validation cases remain open.

## Unchanged imports (D165)

Automatically skip confidently matched records that add no information after normalization and field mapping. Include a clear unchanged-record count in the import report. Compare mapped service history as well as contact values; newly supplied upload/confirmation status or dates may be meaningful even when contact details match. An unchanged skip creates neither a duplicate contact nor a contact edit. D39–D42 still govern differing values and ambiguous candidate selection; this policy does not establish a new uniqueness constraint.

## Minimum information for new imported contacts (D166)

Require both station callsigns, valid UTC start date/time, either a usable frequency or a band, and mode information supplied as mode or submode under D175. Auto-populate band from supplied usable frequency when derivable under D164; band is not independently required, and inability to classify a usable frequency does not prevent import. End time, reports, power, and location details are optional; exact frequency may be unset when a band is supplied. Missing our callsign requires explicit historical assignment under D252; other missing optional historical own-station context remains unknown under D38. Service-specific requirements determine upload readiness rather than local importability. Report and skip new-contact records missing required information while other usable records proceed under D43. This minimum applies to creating contacts; existing-contact updates retain D42 omitted-field behavior. D167 selects invalid optional-value handling; exact validation formats remain open.

## Invalid optional values (D167)

Accept otherwise valid contacts with a warning identifying the record, field, supplied value, and validation reason. For a new contact, omit the unusable optional value. For an existing-contact update, leave the existing value untouched. Invalid input is not an explicit clear under D42. Apply D164 frequency/band normalization where appropriate rather than treating a correctable band mismatch as a failed field. Required information still follows D166, and usable records proceed under D43. Exact field-specific validation remains open; D168 selects structural-error handling. Free-text reports are not restricted to numeric syntax under D128.

## Preparation before application (D169)

Preparation makes no logbook changes. Scan record contents, discover fields across records, determine mappings, normalize and validate values, and identify candidate matches and required choices. Each record can have its own field set; the header or an early record is not a complete schema. Present unsupported fields, warnings, records to skip, and any structural parsing error before application.

D168 defines the eligible prefix after a structural error, not a requirement to commit records while parsing. Do not claim to have inspected fields or counted records in the undecodable remainder. Apply prepared source content and reviewed choices; a changed source must not silently replace what was prepared. The staging representation remains open. D172 selects built-in mappings for MVP: show unsupported fields before application and omit them if the user proceeds. No custom mapping editor is needed for MVP; exact built-in coverage remains open. D171 does not require persistent preparation or import-session resumption.

A prepared comparison can become stale as local edits or synchronization continue. Existing D146 conditional acceptance and D39–D42 comparison choices still apply; preparation is not a lock or permission to overwrite newer values. D170 selects cancellation behavior below; exact local application/revalidation mechanics remain open.

## Cancellation (D170)

Cancellation during preparation/review makes no logbook changes. Once application starts, preserve completed changes, stop at a safe record boundary, and report the remainder as unprocessed. Apply each contact and its associated imported service evidence together; do not leave a partly applied record when cancelling. Whole-import rollback is not required. Re-importing uses D165 unchanged-record skipping and the existing match/update choices. Exact transaction batching remains open. D171 selects rerunning normal import for crash recovery and treats dedicated import-session resumption as unnecessary.

## Crash recovery (D171)

Completed changes survive a crash; an interrupted contact and its imported service evidence cannot be partly applied. Recover by rerunning normal import: prepare the source again, skip unchanged records under D165, and use existing matching/update choices for remaining records. Prior review choices and saved import-session position need not survive. Dedicated session resumption is unnecessary and is not a post-MVP backlog item. Exact storage transactions and prepared-input representation remain open.

## Built-in mappings and unsupported fields (D172)

Use built-in mappings for supported fields, including explicitly supported useful application-specific information. During preparation, show unsupported field names and affected-record counts before any logbook changes. If the user proceeds, omit those fields. Do not retain opaque values solely for re-export. A custom mapping editor is not needed for MVP, with no later-release commitment. Exact field coverage and semantic mappings remain to be specified against the QSO/service model and source evidence.

## Imported coordinates and grid (D173)

Complete valid coordinates determine an inconsistent grid. Preserve the supplied valid grid length when calculating the corrected grid, and show a warning identifying the supplied and calculated grids. Accept the otherwise valid record. Coordinates inside the supplied grid are consistent even when they are not its center; retain that coordinate precision. Grid-only input can derive center coordinates; coordinates-only input derives grid at the selected/default precision under D76. Apply to both supplied own-station and contacted-station fields, without substituting current operating defaults for absent historical facts. Malformed optional values follow D167. The native-model design accepts lengths 2, 4, 6, 8, 10 and 12, integer-millionth-degree coordinates, and imported two-character coarse areas; ADIF field encoding and update application remain adapter work.

## Mode/submode normalization (D174)

At the ADIF boundary, interpret recognized submode names supplied in MODE through the defined parent/submode relationship. D176 does not require storing that pair as native contact fields. A broad parent does not determine a child. For example, MODE=FT4 alone normalizes to MODE=MFSK with SUBMODE=FT4; MODE=MFSK alone remains MFSK with submode unspecified. Here MFSK means multiple frequency-shift keying. MODE=USB alone similarly normalizes to MODE=SSB with SUBMODE=USB, while SSB alone does not establish upper versus lower sideband. SSB means single sideband and USB means upper sideband. These parent relationships come from the [ADIF submode enumeration](https://www.adif.org/316/ADIF_316.htm); accepting a recognized child in MODE is Clacks's tolerant import policy.

When both fields are supplied, a recognized SUBMODE determines the parent MODE. Correct an inconsistent parent with a warning; derive a missing parent from the recognized submode. Normalize before candidate matching under D41. Do not infer missing submodes from frequency or operating conventions. D175 preserves unfamiliar submode values even without a known parent. The initial native recognition baseline is the complete ADIF 3.1.8 mode/submode catalog. Matching of incomplete/unrecognized classifications and field-group behavior when applying selected updates remain interchange specification work.

## Preserve mode information independently of classification (D175)

MODE and SUBMODE convey source information, not a mandatory native two-field representation. Preserve the supplied mode information as native contact data for display, synchronization, and export, independently of whether the classification is recognized. D177 selects the hierarchical native enum and structured fallback below. An unfamiliar submode produces a warning, not omission under D167. If MODE is absent, derive it when D174's recognized relationship is available; otherwise retain the submode and leave the parent unknown. An otherwise valid contact with only an unfamiliar submode still has mode information and is importable under D166 as clarified by D175.

Do not label that local preservation policy as an ADIF requirement. [ADIF 3.1.6 section I.E](https://www.adif.org/316/ADIF_316.htm) recommends exporting dependent fields with their context and leaves required contact fields to applications. SUBMODE is a string with enumerated values recommended for interoperability. Service-specific representability/readiness is evaluated separately; absent parent knowledge does not justify fabricating one or losing local information. D225 requires explicit selection for possible matches from compatible but incomplete classifications; exact compatibility tables remain to be specified. Distinct preserved submode values must not all become the same generic unknown mode.

## Native model versus ADIF boundary (D176)

ADIF is an interchange representation. Its field partition does not define native schema, user entry fields, or a mandatory two-level mode hierarchy. D174's recognized normalization and D175's unfamiliar-information preservation constrain the transforms, not the number of native mode fields.

Define one unambiguous transform from supported ADIF information into the native representation and one from native information into canonical ADIF. Equivalent source encodings may converge on the same native value; export need not reconstruct their original arrangement. Mapping must follow the agreed supported-information policies and avoid invented specificity. D185 does not require the two transforms to be inverses or make ADIF a completely lossless round-trip format. Unfamiliar classifications and any supplied context need an explicit transform policy rather than disappearing because they do not fit two known columns. D177 selects the native enum approach below; full transformation tables and exact encoding remain open.

### Selected native Rust model (D177)

D177 selects a single native mode value represented by a partially hierarchical enum, with typed known specializations and a structured fallback for unfamiliar information. Enums carrying variant-specific data are idiomatic [Rust enum representations](https://doc.rust-lang.org/book/ch06-01-defining-an-enum.html). The following code illustrates the selected model; it is not application implementation, a complete mode catalog, or a storage/wire format.

```rust
enum Mode {
    Cw,
    Ft8,
    Mfsk(Option<MfskMode>),
    Ssb(Option<Sideband>),
    Other(OtherMode),
}

enum MfskMode {
    Ft4,
    Js8,
    Q65,
}

enum Sideband {
    Upper,
    Lower,
}

enum OtherMode {
    Mode(String),
    Submode {
        parent: Option<String>,
        name: String,
    },
}
```

The known branches are illustrative, not a complete supported-mode list. A known family without a supplied specialization uses None; the type restricts a known specialization to its correct family. Exact Rust naming and helper organization remain implementation details.

OtherMode distinguishes an unfamiliar mode label from an unfamiliar submode with optional supplied parent context, while avoiding the two-Option tuple's both-absent shape. A controlled normalization/validation constructor would reject empty names and canonicalize recognized values; do not permit recognized strings to remain duplicate Other representations in equality/matching. This fallback preserves uninterpreted information rather than requiring all native known modes to mirror ADIF fields.

MODE=FT4 and MODE=MFSK/SUBMODE=FT4 both map to Mfsk(Some(Ft4)); canonical export produces MODE=MFSK/SUBMODE=FT4. Parent-only MFSK maps to Mfsk(None), exporting only MODE=MFSK. Submode-only unfamiliar input maps to OtherMode::Submode with parent None. Complete transform tables and behavior for all supported/incomplete values still need specification.

D177 selects the nested approach rather than the previously compared flat known-mode representation. Do not infer a general recursively extensible hierarchy requirement from this sketch. D178 requires recognition differences to affect interpretation only. Preserve the underlying mode value and supplied context even when one client uses a typed known variant and another uses OtherMode. Saving an unrelated field or upgrading a recognition table must not rewrite that untouched value or create an edit/conflict. Exact Rust layout, canonical equality, and stable storage/wire encoding remain open; native enum discriminants must not be assumed to supply protocol identity.


D184 requires retaining the semantic basis of location data: grid-only input represents an area despite calculated center coordinates, while supplied complete coordinates represent a point under D173. Native saving/synchronization must preserve that distinction. D186 exports grid and available coordinates, including grid-derived centers, without an ADIF derivation-preservation guarantee; do not assume derived display coordinates are independently supplied point evidence.

## Interchange limits (D185)

ADIF export/re-import need not preserve all native state or semantics. Deterministic mappings in each direction are not a requirement for inverse transforms. Native synchronization and backups retain the applicable native information; ADIF exports the selected interoperable representation under explicit field policies. In particular, D184 area/point semantics must survive native saving and sync, but this does not select an ADIF rule for retaining that distinction. D186 exports available derived center coordinates alongside the grid; native derivation metadata may be lost. D173 still governs interpretation of supplied complete coordinates on import. Do not infer a completely lossless export/re-import requirement when resolving future mapping choices.

## Location export (D186)

Export the grid and available latitude/longitude, including calculated grid-center coordinates. Native area/point derivation is not guaranteed to survive ordinary ADIF, consistent with D185. Re-import of supplied complete coordinates follows D173. Do not infer explicit-versus-derived origin by comparing coordinate values with a cell center. Native persistence/synchronization and applicable backups preserve D184 meaning independently. Exact formatting and longer-grid field encoding remain open.

## File-format scope (D187)

MVP imports and exports length-delimited ADI, accepting .adi and .adif filenames. ADX, the XML (Extensible Markup Language) representation using .adx, is post-MVP. The native model remains independent of interchange layout and text restrictions. D188 selects best-effort human-readable text transliteration with warnings, omission of optional fields that cannot be represented meaningfully, and preservation of the original native text. Exact conversion tables and field-specific handling remain open. Existing preparation and structural parsing rules apply to ADI. See the [ADIF file-format definitions](https://www.adif.org/316/ADIF_316.htm).

## ADI text conversion (D188)

For human-readable text such as names, comments, and equipment descriptions, use best-effort transliteration into ADI's supported character repertoire and report warnings. For example, a name such as José may export as Jose. Omit optional fields that cannot be represented meaningfully and report those omissions. Preserve original native text in storage and synchronization; export conversion is not a contact edit and does not create a revision.

Callsigns and other identifiers use their identity/format validation path, not transliteration that might change identity. Exact conversion tables/library, diagnostic presentation, and remaining field-specific cases remain open. Do not automatically apply this policy to provider submissions; their encoding requirements are separate. [ADIF's data-type definitions](https://www.adif.org/316/ADIF_316.htm) define ordinary Character/String text using printable ASCII (American Standard Code for Information Interchange), while international-character types belong to ADX.

## Location fallback on export (D200)

ADIF export uses the effective contacted-station location under D199: the QSO's recorded location when present, otherwise available directory-provided location. Keep these sources separate in native data; exporting fallback values does not edit recorded QSO location. The ADIF output may lose that source distinction, consistent with D185's non-lossless interchange boundary. D186 still governs emitting grid and available coordinates. D201 selects whole-location precedence even for a partial QSO location: do not fill missing exported location fields from a different directory location. Derived values follow the selected location and existing precision/ambiguity rules. D203 retains/synchronizes the separate directory-location snapshot, including provider and lookup time, with each QSO, making it available for offline export fallback. D206 selects whole-snapshot replacement on successful lookup and preservation on failure/not found.

D204 extends this source-selection principle to other supported directory information, including names: use recorded QSO information when present, otherwise separate directory snapshot information. ADIF output does not copy fallback values into recorded native QSO fields and need not preserve the source distinction under D185. Exact field mappings remain open.

D207 requires current normalized-key association before a directory snapshot supplies export fallback. A recorded-callsign change that changes the lookup key stops use of the old snapshot; failed lookup for the new station cannot leave old-station data in exported effective fields. If the normalized key is unchanged, retain the snapshot.

## Exported local state (D210)

Export the client's current saved state, including unsynchronized and unresolved local working information. Emit one local working version per QSO and exclude pending local deletions, matching D180/D181 normal history. Directory fallback uses locally available eligible snapshots with D201 whole-location precedence and D207 current-key association. Do not require synchronization or export alternate shared/conflicting versions as additional contacts. Export does not resolve pending work or preserve all native synchronization/conflict metadata under D185. D211 selects a consistent point-in-time local view for membership, QSO values, and eligible snapshots. Logging/sync may continue without changing captured export contents. Cancellation/failure leaves any existing destination untouched; publish the completed destination only on success and report exported contacts/conversion warnings. Exact snapshot/staging/finalization mechanisms remain open.

## Activity/reference export expansion — discussion checkpoint 16 October 2026

D127 already establishes a shared typed activity/reference context for Parks on the Air (POTA) references and contest/event identifiers, with correct own-station/contacted-station reference sets on each QSO and field-specific mapping. D130 permits these values in saved profiles under established loading/snapshot rules; D131 makes manual identifier entry sufficient. Do not restart this as a separate POTA-specific model or bundle those existing decisions into another approval question.

D212 accepts optional POTA-compatible export in MVP alongside default ordinary ADIF export: one native QSO may produce multiple output entries for applicable POTA references when requested. Distinguish this from the native model, profile loading, ordinary QSO identity/history, and automatic service-upload policy. D212 selects POTA-specific file grouping/entry expansion and ordinary export as the default; other destinations’ expansion behavior remains unselected. D210's one local working version per QSO prevents exporting competing conflict versions as duplicates; it does not preclude an explicitly selected reference-expansion output transform.

Source findings checked 16 October 2026:

- The official [POTA activator guide](https://docs.pota.app/docs/activator_reference/activator_guide-english.html#multiple-park-references) documents simultaneous activation of multiple references.
- [ADIF version 3.1.8](https://www.adif.org/318/ADIF_318.htm#QSO_Field_POTA_REF) defines POTA_REF and MY_POTA_REF as lists, whereas CONTEST_ID is a single identifier field. Thus multiple references do not inherently require multiple ADIF records, and representation must be field/type-specific. This source check does not select a new complete ADIF compatibility baseline for Clacks.
- The [POTA ADIF technical reference](https://docs.pota.app/docs/activator_reference/ADIF_for_POTA_reference.html) documents its own SIG/SIG_INFO and MY_SIG/MY_SIG_INFO conventions separately. Standard list-field support does not establish compliance with POTA submission requirements. The POTA rules and park-to-park guide below establish its file grouping and expansion requirements; other destinations still need their own verification.

D212 selects ordinary export with one entry per QSO through supported mappings, and optional POTA-compatible export with the documented POTA file grouping/entry expansion. Do not automatically split every unrelated contest/event/POTA reference or invent a Cartesian product across them. Expansion changes output only, not stored QSO identities. Do not infer extra server uploads from a file-export option.

## POTA submission requirements — checked 16 October 2026

The [POTA rules, activation location/access rule 6 and logging requirement 3](https://docs.pota.app/docs/rules.html) require a separate submitted log for each park in a simultaneous multi-park activation, with each log covering one park in one location. This is a receiving-program requirement, separate from ADIF's ability to represent lists in a single record.

The [official park-to-park guide](https://docs.pota.app/docs/activator_reference/park_2_park.html) says a contact with a three-park activator must be listed three times, using a different SIG_INFO park reference on each entry, to obtain all three park-to-park credits. It explicitly gives the multi-park-to-multi-park case: an operator at two parks contacting an operator at three parks generates two ADIF files, each containing three entries for that on-air contact. The other operator generates three files with two entries each. That specific product of own/contacted park references is documented POTA behavior, not permission to combine unrelated contest/event references automatically.

The [logging guide](https://docs.pota.app/docs/activator_reference/logging_made_easy.html) likewise directs additional entries for each contacted park; timestamps and other contact data remain identical. The program's rules take precedence over guides if inconsistent. Hunters do not submit logs; the above submission workflow concerns activators.

For Clacks, this supports an optional POTA-compatible export transformation that groups files by own park/location and expands each contact by applicable contacted parks. It must not create duplicate native QSOs. General ADIF export can still use supported list fields. D212 accepts this optional POTA-compatible mode in MVP, with ordinary ADIF as the default. Exact export-field validation, naming/grouping details, and multi-file finalization remain open. Do not repeat the earlier incomplete suggestion that ADIF list fields alone resolve POTA submission compatibility.

## Standard interchange versus POTA submission mappings — checked 18 October 2026

Ordinary ADIF 3.1.8 interchange can represent own/contacted park collections
through `MY_POTA_REF` and `POTA_REF`. Those standard fields alone do not define
pota.app submission compatibility. The [POTA ADIF technical reference](https://docs.pota.app/docs/activator_reference/ADIF_for_POTA_reference.html)
documents `MY_SIG=POTA` with `MY_SIG_INFO` for our park and `SIG=POTA` with
`SIG_INFO` for the contacted park. It also documents `MY_STATE` for resolving
activation location when a park spans administrative boundaries. The park fields
are optional for basic processing; the uploader may request missing activation
information. They must not all be described as mandatory fields for every QSO.

The [POTA rules](https://docs.pota.app/docs/rules.html) independently define
required submission fields, including band, and one park/location per log.
The [park-to-park guide](https://docs.pota.app/docs/activator_reference/park_2_park.html)
requires distinct contacted references in expanded entries to obtain all
multi-park credits. Keep ordinary interchange bindings, submission field
bindings, submission validation and output grouping/expansion distinct. Native
saving remains governed by D112/D166, including frequency-or-band validity.
These findings constrain the group-definition schema proposal; they do not
select its concrete representation or expand MVP scope.

The [native-model specification](../docs/superpowers/specs/2026-10-18-native-model-design.md), awaiting user review, consolidates the accepted native values, group/catalog capabilities and D255 export context. It does not select the import-side station assignment of CONTEST_ID or complete every ADIF mapping.

## Accepted export modes (D212)

Ordinary ADIF export is the MVP default: one entry per QSO using supported mappings, including list-valued reference fields where supported. Optional POTA-compatible export creates files by own park/location and expands contact entries by applicable contacted-station park references according to the verified POTA requirements above. Do not create native duplicate QSOs, alter callsigns/timestamps, split unrelated contest/event references, or infer automatic POTA submission.

Both modes use D210/D211 consistent point-in-time local working state, eligible directory fallback, and existing-destination protection on cancellation/failure. Report source contact count separately from generated entry/file counts. Exact export-field validation, file naming/grouping details, and multi-file staging/finalization mechanics remain open. One native working version per QSO under D210 is compatible with explicit POTA output expansion.

## Export-context mapping selection (D255)

Export context consists of an output profile and an optional target group, separate from the selected contact set. Group definitions distinguish ordinary interchange mappings from mappings for exports targeting that group. Ordinary ADIF without a target uses ordinary interchange mappings, including supported reference lists, and emits one entry per QSO without activating program-specific expansion. Emit CONTEST_ID when a QSO has exactly one distinct applicable contest identifier. If it has several, identify the ambiguity during export preparation and allow selecting a contest context or proceeding with those ambiguous CONTEST_ID fields omitted and reported; contacts belonging to different single contests do not create ambiguity with each other. For ordinary ADIF targeting a contest, that contest determines CONTEST_ID and other contest associations do not compete. A program-compatible profile uses its selected group's submission mappings, validation, partitioning and expansion. Conflicting assignments within a selected definition are configuration errors reported before writing output. Do not silently choose a contest, add participation, rewrite native associations or change the selected contact set. This replaces the unaccepted generic per-binding omit_with_warning proposal with context selection and explicit handling of representation limits. It refines D212/D254 without changing D210/D211 local-state and output-publication safeguards.

# LOG4OM Amateur Data Interchange Format (ADIF) import sample

Inspected 12 October 2026: user-supplied `Log4OM_ADIF_20261012135355.adi` from Downloads, exported by LOG4OM 2.42.1.0. The header declares ADIF 3.1.0. The original contains personal/contact information and has not been copied into this repository.

This is source evidence for minimum viable product (MVP) import planning, not a field-support or opaque-preservation policy. ADIF import is accepted as D31; D38-D40 define historical context and matched-record choices. Other detailed behavior remains open.

## Inventory

A length-aware scan found 374 complete records, 78 distinct record field names, no repeated field names within records, and no unfinished record. This is a structural scan, not full ADIF semantic validation.

- Contact dates: 18 May 2019 through 9 October 2026.
- All 374 contacts use FT8; 372 are on 20m and two on 40m.
- Station callsign is W7HEX for 132 records and KD8CGM for 242.
- Own grid is CN85ko for 354 records and CN85nl for 20.
- Standard fields cover contact facts, station snapshots, contact location, reports, power, service submission/confirmation status, and other metadata.
- `POTA_REF` occurs in 27 records. No `MY_POTA_REF` or `OPERATOR` occurs in this sample.
- Logbook of The World (LoTW) sent status is Y for 370 contacts, and received status is Y for 245. Imported status is historical evidence, not a Clacks Log job receipt or independently verified remote state.

The [ADIF 3.1.6 specification](https://www.adif.org/316/ADIF_316.htm) defines the non-APP field names found here, including Parks on the Air (POTA) and service-specific fields. A service-specific field name does not necessarily mean it is application-defined. The header's older version declaration does not establish that every exported field belongs to that older specification. ADIF does not require applications to support every field.

## LOG4OM-specific fields

Each of these four fields appears in all 374 records:

| Field | Observed content | Import implication, not a decision |
| --- | --- | --- |
| `APP_L4ONG_SATELLITE_QSO` | N throughout | No positive satellite cases to investigate in this sample. |
| `APP_L4ONG_CONTEST` | N throughout | No positive contest cases to investigate in this sample. |
| `APP_L4ONG_QSO_CONFIRMATIONS` | JavaScript Object Notation (JSON) entries for contact confirmation (QSL), eQSL, QRZ, HamQTH, HRDLOG, Club Log, and LoTW | Some information overlaps standard status/date fields, but QRZ received status appears only here in this sample. |
| `APP_L4ONG_QSO_AWARD_REFERENCES` | JSON award references and status labels | Many references overlap ordinary radio contact (QSO) fields; some represent additional information. |

### Confirmation JSON

All JSON values parsed successfully. Each contact has seven service entries. The observed keys are `CT`, `S`, `R`, `SV`, `RV`, and optional `SD` and `RD`.

For LoTW and eQSL, the observed sent/received categories correspond to the standard-field status counts; JSON dates match corresponding standard dates where present. QRZ entries report received Yes on 54 contacts; the sample has no separate standard field carrying that QRZ received state. QRZ sent timestamps retain time of day on 373 records, whereas the standard upload-date field contains a date.

These are observations of LOG4OM's export representation. They do not establish the precise remote meaning or trust level of each status. Do not promote an imported value to a newly verified confirmation or fabricate publication jobs from it.

### Award-reference JSON

All JSON values parsed successfully. POTA references appear on the same 27 contacts as `POTA_REF`, with identical reference values in this sample. Japan Century Cities (JCC) references appear on 17 contacts and have no separate dedicated standard JCC field in this export. Some JCC entries carry `VALIDATED` labels.

Do not treat those labels as independent confirmation or verified Japanese location evidence. For example, one record's free-text station location (QTH) mentions JCC-2810 while its JSON JCC reference is 2801. The export alone does not resolve that disagreement.

All observed award `SUB` and `GRA` arrays are empty. The following award labels are literal export identifiers, not an explained award catalog or a list of supported Clacks features; their presence alone does not establish their meaning or validation. Additional award names include DX Century Club (DXCC), `DXCC_QSL`, `MARATHON`, `VUCC`, `WAC`, `WAS`, `WAZ`, `WITUZ`, `USA-CA`, `WPX`, Islands on the Air (IOTA), `WAE`, `CCC`, and `WUST`. Award support remains later scope; finding a value here does not move it into MVP.

## Planning implications

The sample supports considering explicit mapping of useful LOG4OM-specific information, particularly QRZ confirmation history. It does not establish a need to retain every private field unchanged for later re-export.

Retain imported upload and confirmation history when reasonable. Exact mapping and whether LOG4OM exports sufficient information depend on the external-service model, whose foundation is accepted but whose detailed representation remains open. D50 accepts a positive mapped imported upload status for immediate suppression of resubmission without service verification; service-relevant changes make it pending again. D51 makes missing upload status pending like explicit unsent status, independently per service. D52 accepts recognized positive imported confirmations immediately, preserving available dates and provenance without verification. Exact field mapping remains open. Do not infer a complete Clacks Log attempt or exact historical payload from that flag.

Still open: exact supported import fields and semantic mappings, service-status interpretation under D50–D52, exact historical station-context validation, and whether later award information is retained in any form. D172 selects built-in mappings, unsupported-field disclosure before application, and omission if the user proceeds; no opaque preservation for re-export or MVP custom mapping editor is required. D41 governs candidate matching.

## Historical operating context (D38)

Historical ADIF imports use supplied historical station context. D252 requires explicit historical callsign assignment before creating a contact when our callsign is missing; other missing optional our-station (DE) fields remain unknown. Do not fill missing historical fields from current client defaults. This also applies to downloaded QSOs explicitly added locally; D28's default fallback applies to live QSO capture. The sample's differing station callsigns/grids and uneven MY_STATE coverage illustrate why current defaults are not a safe historical fallback. Validation and later explicit correction remain separate work.

## Matched ADIF records (D39-D40)

For an incoming record confidently matched to an existing QSO but containing differing values, offer update of the existing QSO, creation of a separate QSO (duplicate), or skipping the incoming record. Do not silently choose an outcome. This is separate from D37's add/reject decision for unmatched service downloads. D40 selects the same field-level conflict-resolution interface as other conflicting updates: the user chooses which differing fields to apply. D41 establishes candidate matching by historical station/worked callsigns, band, normalized mode/submode, and Coordinated Universal Time (UTC) start time at shared supplied precision, with explicit selection among multiple candidates. D42 leaves existing values untouched for omitted fields and offers explicitly empty values as selectable clears, subject to field-specific ADIF default interpretation and validation. D43 permits partial imports with a clear report identifying records not imported and why. D165 automatically skips confidently matched records that add no information, with a clear unchanged-record report count. Other validation requirements remain to be designed.

D164 resolves imported frequency/band precedence: a usable frequency supplies band; correct a mismatched band with a warning and accept the otherwise valid record. Derive missing band from frequency; band-only records remain eligible. This is an import policy, not an observed property of the LOG4OM sample. See [frequency/band normalization](adif-matching.md#frequency-and-band-normalization-d164).

D166, as refined by D252, requires both station callsigns, valid UTC start date/time, either a usable frequency or a band, and mode information to create a new imported contact; D175 permits supplied submode without a known parent. Auto-populate band from supplied usable frequency when derivable; band is not independently required and inability to classify the frequency does not prevent import. Missing our callsign requires explicit historical assignment before creation; other operating/contact details remain optional, including missing optional historical own-station context under D38. Existing-contact updates retain D42 omitted-field handling.

D167 permits otherwise valid records with warnings for invalid optional values: omit unusable values on creation and preserve existing values on update. Warnings identify the record, field, supplied value, and reason. This is distinct from a selectable explicit clear and from D164 band correction.

D168/D169 stop read-only preparation at structural parsing errors, leave preceding reliably decoded complete records eligible for application under existing validation/matching rules, and report the remaining portion as unprocessed. No logbook changes occur during preparation. Do not guess record boundaries after damaged lengths. This is distinct from D167 warnings for reliably parsed invalid optional values.

D169 requires preparing mappings, validation, and matching before application, showing unsupported fields and structural errors up front. Discover fields across the parsed records; records have independent field sets, so an initial header/record cannot establish the full field inventory.

D173 resolves imported location precedence: complete valid coordinates determine an inconsistent grid, retaining supplied grid precision and warning about the correction. Agreement means the coordinates fall in the grid cell, not that they equal its center. This policy applies to our station and the contacted station without current-default fallback.

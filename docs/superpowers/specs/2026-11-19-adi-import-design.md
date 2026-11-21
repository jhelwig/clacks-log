# Prepared ADI import and account-wide matching

[Clacks Log #24](https://github.com/jhelwig/clacks-log/issues/24) delivers a reviewable migration path for existing contacts. Amateur Data Interchange Format (ADIF) text files (`.adi` and `.adif`) use built-in mappings. The workflow has no custom mapping editor, saved import session, or opaque field archive. See [import requirements](../../../references/adif-matching.md), [product scope](../../product.md#import-and-export), and [service evidence](../../service-evidence.md).

## Prepare and review

Choose the destination tag and source file. Acquire a shared workspace sync pause guard before reading candidates. Wait for any active exchange to finish. Multiple operations may hold guards, and the worker can start an exchange only when no guards are held. It releases its exclusive access before waiting idle. Hold the import guard through review and application. Completion, cancellation, and failure release it. The worker then resumes its existing receive-first exchange and conflict workflow. Ordinary transactional save guards still apply.

Preparation is read-only and maps to the existing native model. Extensions to the native model require explicit scope and approval. Use qso-io's permissive ADI parser to keep record and field locations, distinguish missing and empty fields, and preserve initially undecoded bytes where needed. Prepare against the selected saved account state. Keep prepared source values in memory so Apply uses what the user reviewed. A framing error ends parsing at the last complete record. Report the location and unreliable remainder without inventing a remainder count. Report unsupported fields and affected-record counts across the readable records. Invalid optional values warn and supply no edit. A supported explicit empty can offer a clear, subject to the field-specific rules and validation. An omitted or deliberately omitted disputed value supplies no edit.

Resolve overlaps by field meaning and documented exporter provenance. Merge agreeing values and retain compatible details, including an exact representable timestamp when it describes the same event. When two usable values genuinely disagree and neither has established authority, show both, including their source fields. Offer either value or omission of the disputed fact. The rest of the row remains eligible under the ordinary save requirements. If an identity choice changes candidate eligibility, recompute the affected review before application. Keep a previously reviewed assignment only if its reviewed comparison still applies.

## Candidate assignment

Search a snapshot of saved working QSOs across the account, including untagged contacts and pending local edits. Exclude pending deletions, source rows, and contacts created by the current import. Compare worked callsign, historical own-station callsign, band, normalized mode identity, and UTC start date and time. Retain callsign suffixes. Frequency determines the band when derivable. Compare time in both directions at the lowest supplied precision, truncating both values to minutes when either is minute-only. Use that precision for equality and the inclusive 30-minute window across UTC dates. Saved contact time uses the existing whole-second representation. When an imported start or end supplies only the same UTC minute, retain any seconds in the saved time.

An eligible candidate with compatible incomplete identity or a start-time difference needs explicit selection, even when unique. An explicitly conflicting identity excludes the candidate. A broad mode and its known specialization can qualify for review. The same preserved unfamiliar mode hierarchy and labels compare equal. For the same unfamiliar submode, a parent supplied on only one side requires review, and differing supplied parents exclude the candidate. Location, reports, and other editable details enter the subsequent field review.

Require explicit review when one source row has multiple candidates or multiple source rows contend for one saved QSO, including an otherwise automatic unchanged or tag-only reuse. Keep each candidate available until the user resolves the assignment. Each row can be assigned to at most one saved QSO and each saved QSO can receive at most one row for this import. For example, if A can target X or Y and B can target only X, the user may assign A to Y and B to X. The user may drop a row or unmatch it to create a distinct saved QSO. There is no durable row-to-QSO matching history.

Make each candidate’s recorded QSO details and logbook memberships available in review before assignment. Show all recorded fields for the chosen saved QSO and reuse the shared field-level controls for proposed changes. Retain saved values for omitted, invalid, or omitted-by-choice input. Reusing or updating the QSO adds the selected tag if needed. Creating a distinct QSO includes the tag and requires both station callsigns, a valid start, radio band or usable frequency, and mode information. If historical station identity is missing, require an explicit assignment for creation. Never apply current station, provider, directory, or certificate defaults to historical facts. A row that needs no explicit selection can be counted as unchanged automatically if it contributes no contact facts, membership, or service evidence. Drop changes nothing.

## Built-in contact and activity mappings

| Input | Mapping |
| --- | --- |
| `CALL`, `STATION_CALLSIGN`, `OPERATOR`, `CONTACTED_OP`, `MY_NAME`, `NAME` | Station callsigns and separate operators/names. Use `OPERATOR` for our station only when `STATION_CALLSIGN` is absent, per ADIF. |
| `QSO_DATE` / `TIME_ON`, `QSO_DATE_OFF` / `TIME_OFF` | Required start and optional end in UTC. For supplied end time without an end date, use ADIF's under-24-hour rule, including midnight rollover. End must be at or after start. |
| `FREQ`, `BAND`, `FREQ_RX`, `BAND_RX` | Native primary pair and optional receive pair. A usable frequency supplies band within a recognized range and takes precedence over a mismatched band with a warning. Apply independently on receive and preserve band-only source values. When a band-only input agrees with a saved explicit frequency, keep the frequency. |
| `MODE`, `SUBMODE` | A recognized submode supplies its parent, including when supplied in `MODE` alone. Warn on correcting a conflicting supplied parent. Preserve unfamiliar labels and recorded parent context. |
| `RST_SENT`, `RST_RCVD`, `COMMENT`, `TX_PWR`, `MY_RIG`, `MY_ANTENNA` | Optional report text, comment, power in native milliwatts, and recorded equipment descriptions. |
| `GRIDSQUARE`, `GRIDSQUARE_EXT`, `LAT`, `LON`, and own-station equivalents | Combine valid base and extension. Grid-only is an area. A complete valid coordinate pair records a point and corrects an inconsistent grid with a warning, retaining the supplied valid grid length. |
| `DXCC`, `STATE`, `CNTY`, `CQZ`, `ITUZ`, and own-station equivalents | Recorded geographic identifiers and zones. `COUNTRY` and `MY_COUNTRY` are names and have no mapping in #24. |
| `POTA_REF`, `MY_POTA_REF`, and documented POTA `SIG`/`SIG_INFO` pairs | Combine compatible park references independently per side. A submission pair represents one park. A list can contain multiple parks. Deduplicate equivalent values and review incompatible qualifiers for the same park. |
| `SOTA_REF`, `MY_SOTA_REF`, `IOTA`, `MY_IOTA`, and other supported station-side mappings | Resolve to existing account reference groups and validate with the corresponding definition. Use supported explicit mappings for other activity fields (such as Worldwide Flora and Fauna fields or a documented `SIG` pair) when the group and semantics are unambiguous. Report unsupported or ambiguous values. |
| `CONTEST_ID` | When it is the only contest information, resolve an applicable literal contest code through the account group definitions and add group-only participation to our station. Dedicated exchange fields remain disclosed as unsupported. |
| `ADIF_VER`, `PROGRAMID`, `PROGRAMVERSION` | Source metadata for diagnostics and provenance. |

[ADIF definitions](https://www.adif.org/318/ADIF_318.htm#QSO_Fields) · [Reference groups](../../native-model.md#8-participationreference-associations) · [POTA conventions](https://docs.pota.app/docs/activator_reference/ADIF_for_POTA_reference.html) · [Whole-location fallback](../../decisions.md#d238)

## Imported service evidence

| Claim | Input |
| --- | --- |
| Logbook of The World (LoTW) upload | `LOTW_QSL_SENT=Y`, with `LOTW_QSLSDATE` if supplied. |
| LoTW confirmation | `LOTW_QSL_RCVD=Y` or legacy verified `V`, with `LOTW_QSLRDATE` if supplied. LOG4OM's agreeing LoTW entries corroborate the standard fields without adding a second claim. |
| QRZ upload | `QRZCOM_QSO_UPLOAD_STATUS=Y`, with `QRZCOM_QSO_UPLOAD_DATE` if supplied. An agreeing LOG4OM `CT=QRZCOM, S=Yes` with `SD` can supply an exact whole-second timestamp. Standard status `M` records changed values after upload. A historical sent flag cannot certify that the current values were accepted. |
| QRZ confirmation from QRZ | `APP_QRZLOG_STATUS=C`, with `APP_QRZLOG_QSLDATE` if supplied. Authored status reports the current state of the exported QRZ record. Use its authority only for that established scope. QRZ can pass through inputs from other exporters. The QRZ date describes the first accepted confirmation. |
| QRZ confirmation from LOG4OM | `APP_L4ONG_QSO_CONFIRMATIONS` with `CT=QRZCOM, R=Yes` and optional `RD`. The exporter documentation establishes the received-status meaning, corroborated by row-level comparison to standard flags for other services. The date describes the supplied LOG4OM assertion and need not equal QRZ's first accepted date. |

Upload and confirmation claims are independent. D50–D52 accept applicable recognized positives immediately, with independent pending status when upload is missing or unsent. Store a supplied timestamp when it represents an exact whole second in UTC. Report calendar-only dates and timestamps with nonzero fractional seconds during preparation. Retain the recognized claim and source field provenance when its timestamp is unavailable. Disclose contradictions without established precedence and apply the review choice to the disputed claim or date. For a confirmed claim with a disputed date, omitting the date can retain the otherwise agreed claim. Retain source field provenance and frozen source-only comparison baselines. A match assignment identifies where to retain the claim. The existing coverage and applicability rules decide current status using the imported baseline and the resulting QSO, including when review retains a different saved time or the source lacked a required comparison value. Preserve the historical claim if current applicability cannot be established. Never fabricate a dispatch, provider observation, destination, remote ID, or external association from a status flag. A download flag describes download only. Existing association uniqueness remains in force.

[Imported evidence](../../service-evidence.md#31-imported-baselines) · [QRZ export behavior](https://www.qrz.com/docs/logbook30/adif-standard) · [LOG4OM confirmation documentation](https://www.log4om.com/l4ong/usermanual/Log4OMNG_ENU.pdf)

## Disclosed fields and follow-ups

| Purpose | Fields |
| --- | --- |
| Recorded locality text | `QTH`, `MY_CITY`. Report as unsupported and continue processing the row's usable fields. |
| Contact and mailing details, [#51](https://github.com/jhelwig/clacks-log/issues/51) | `ADDRESS`, `EMAIL`, `MY_STREET`, `MY_POSTAL_CODE` |
| Supplemental geography, [#52](https://github.com/jhelwig/clacks-log/issues/52) | `COUNTRY`, `MY_COUNTRY`, `CONT`, `IOTA_ISLAND_ID`, `MY_IOTA_ISLAND_ID` |
| Antenna, radio path, and conditions, [#53](https://github.com/jhelwig/clacks-log/issues/53) | `ANT_AZ`, `ANT_EL`, `DISTANCE`, `A_INDEX`, `K_INDEX`, `SFI`, `APP_L4ONG_SATELLITE_QSO` |
| Completion and scheduling, [#54](https://github.com/jhelwig/clacks-log/issues/54) | `QSO_COMPLETE`, `QSO_RANDOM` |
| Generic QSL correspondence, [#55](https://github.com/jhelwig/clacks-log/issues/55) | `QSL_SENT`, `QSL_RCVD`, `QSLSDATE`, `QSLRDATE`, `QSL_SENT_VIA`, `QSL_RCVD_VIA`, `QSL_VIA` |
| Additional services, [#56](https://github.com/jhelwig/clacks-log/issues/56) | eQSL, HamQTH, Club Log, and HRDLog status and dates, including their LOG4OM entries |
| Award information, [#57](https://github.com/jhelwig/clacks-log/issues/57) | `PFX`, `CREDIT_SUBMITTED`, `CREDIT_GRANTED`, `APP_L4ONG_QSO_AWARD_REFERENCES` |
| Contest exchange and source flags, [#58](https://github.com/jhelwig/clacks-log/issues/58) | `STX`, `SRX`, `STX_STRING`, `SRX_STRING`, other dedicated exchanges, and `APP_L4ONG_CONTEST` |

## Application, synchronization, and recovery

Apply each reviewed row in one guarded local transaction covering the QSO,
selected tag membership, and imported assertions. The resulting records enter
ordinary per-record synchronization after the pause guard releases. A stale
basis or an error before commit leaves the row unapplied. An uncertain commit
result leaves the outcome unknown until saved state is checked. Ordinary
re-import reads that state and recognizes already incorporated facts, membership,
and evidence before applying reviewed changes.

Cancellation during preparation or review makes no changes. During application,
let the current row settle and keep completed rows. Report unchanged facts,
added memberships, updates and creations, warnings, failures, dropped rows,
and unprocessed work without conflating these counts. The import guard releases
on every terminal path.

Reuse the existing working-QSO model, generic group definitions, field-level comparison, local save guard, and service claims. Import matching lives in the import review. [Client-store save contract](../../client-store.md) · [Service evidence transactions](../../service-evidence.md#11-transactions-dispatch-and-interrupted-transfer)

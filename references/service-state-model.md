# External-service state model and research

Checked 12 October 2026. Documentation research only; no authenticated requests, live uploads, or provider tests were performed. This starts the external-service modeling discussion before Amateur Data Interchange Format (ADIF) history mapping. The separation of current radio contact (QSO) data, publication history, and remote observations is accepted as D32. Detailed fields, storage, matching, recovery, and import behavior remain to be designed; this is not a database schema.

## Primary evidence

- [Logbook of The World (LoTW) acceptance/confirmation queries](https://lotw.arrl.org/lotw-help/developer-query-qsos-qsls/): separate QSO-receipt and contact confirmation (QSL)-update queries; separate progress timestamps; optional own/counterpart location detail; records may originate from other applications. Remote reports do not inherently identify a Clacks Log revision.
- [LoTW submission guidance](https://lotw.arrl.org/lotw-help/developer-submit-qsos/): suppress routine submission of unchanged contacts.
- [LoTW corrections](https://lotw.arrl.org/lotw-help/submitting-qsos/): corrections matching specified identity fields update an existing record; identity changes can create another record. Submitted contacts cannot be deleted. Award constraints can affect corrections.
- [QRZ application programming interface (API) developer guide](https://www.qrz.com/docs/logbook/QRZLogbookAPI.html): API keys select callsign/date-bounded logbooks; records have remote IDs. INSERT reports insertion/replacement outcomes. REPLACE can overwrite a confirmed record with an unconfirmed record until matching is verified. FETCH supports confirmed/all selection and modification-date filtering. Exact returned confirmation fields and ambiguous-write recovery still need investigation.
- [QRZ confirmation explanation](https://www.qrz.com/docs/logbook30/confirmations-how): confirmation is based on matching independently supplied reports; the counterpart's complete report is not exposed to the other party.

Do not assume a download from QRZ exposes a counterpart record just because it exposes confirmation. Do not treat successful file transfer to LoTW as proof of individual QSO acceptance.

## Accepted foundation (D32)

Preserve three related kinds of information per QSO and service destination:

1. **Current local QSO:** the operator's editable contact and operating context, subject to accepted synchronization/conflict rules.
2. **Publication history:** the accepted local revision and actual service-relevant values sent, destination, attempts, times, and outcomes. A lost response leaves an uncertain outcome; it does not establish failure or success. Distinguish transport acknowledgement from individual acceptance where the provider does.
3. **Remote observations:** returned record identifiers where available, reported acceptance/confirmation, associated values, provider timestamps where available, and when Clacks Log observed them. Link to local contacts and submissions only as far as matching evidence supports. Imported history may eventually contribute partial evidence without fabricating missing Clacks Log attempts.

Publication need is determined per service by changes to the values it receives. Remote observations remain separate from the local QSO. D237 supersedes D33–D36 automatic replacement: retain downloaded other-station (DX) details separately and protect manually recorded contact facts. Downloaded our station (DE) context does not change our recorded operating context. D34 sets LoTW above QRZ for conflicting provider details in minimum viable product (MVP); priority configuration is post-MVP. D238 selects logical fallback in this order: recorded QSO information, eligible LoTW information, eligible QRZ information, then the eligible directory snapshot. Select location as a whole from the first available source, including partial recorded locations; do not mix location fields across sources or change recorded QSO facts. Apply this to display and supported export/upload fields, retaining provenance and existing association/applicability requirements. Exact field coverage and representation remain engineering work. Historical confirmation observations and the latest known remote state must be distinguishable if replacement or correction changes confirmation.

Example: changing our grid on a previously confirmed contact leaves a local correction pending. Clacks Log still knows what it previously submitted and what confirmation it previously observed. A later job records the corrected submission. The latest service state is updated from subsequent provider evidence; the earlier confirmation does not prove acceptance of the new grid.

Simpler mutable flags would lose which values were submitted; a complete event-sourced model of the whole application would add scope beyond this immediate need. Retaining publication records and remote observations alongside mutable QSO data is the accepted direction.

## Correction: delivery versus new information

Repeated delivery of an already processed remote observation is a duplicate, not a new enrichment event. It must not reapply the previous enrichment, advance the QSO revision, or undo a later edit or conflict resolution merely because the local value now differs. Deduplication must distinguish remote updates to the same contact from repeated delivery of an existing update; exact observation identity and recovery mechanics remain to be designed.

The earlier discussion incorrectly assumed that receiving the same confirmation again could automatically recreate an enrichment edit after conflict resolution. That conflated repeated delivery with new remote information. No separate field override or repeat-overwrite policy was accepted.

[LoTW query documentation](https://lotw.arrl.org/lotw-help/developer-query-qsos-qsls/) specifies inclusive since filters, separate LASTQSL/LASTQSORX progress values, and record-level RXQSL/RXQSO update timestamps. Windows can therefore overlap at their boundaries; duplicates must have no second effect. This is not permission to ignore all future updates to a previously seen QSO.

## Synchronized recovery evidence (D140)

MVP sync includes recovery-relevant stored publication outcomes, remote observations, and processing decisions (matching, rejection, enrichment), with the existing context needed to interpret them. Surviving clients can return missing evidence to an older restored server through normal sync. Success for an older sent payload does not establish publication of later changes; a processed observation must not execute its enrichment again after recovery. Exact evidence encoding, retention, and consistency with recovered QSO state remain open.

## Proposed download recovery

This is a design proposal derived from the accepted durability and duplicate-processing requirements; its mechanics are not yet selected.

- Keep download progress scoped to the destination and query stream. LoTW receipt and confirmation streams have separate progress; QRZ progress belongs to the selected logbook and filter.
- Durably retain downloaded observations and the work needed to process them before advancing the download checkpoint. Incomplete responses or failed storage must not move the checkpoint past unretained records. Paginated runs need coverage tracking, not a checkpoint taken from the first page.
- Process retained observations independently. Record the processing result and any resulting accepted QSO edit together, so interruption cannot leave a committed edit with no durable indication that its observation was processed. A retry of completed work has no second effect.
- Keep unmatched or ambiguous observations pending for matching rather than dropping them or preventing unrelated contacts from being processed. Download progress can advance once this pending work is durably retained. D37 requires explicit confirmation to add an unmatched QSO locally or reject it locally; no automatic local creation or remote deletion is implied. D38 uses supplied historical station context for explicitly added QSOs and leaves missing DE fields unknown, without current-default fallback.

The [QRZ API guide](https://www.qrz.com/docs/logbook/QRZLogbookAPI.html) documents MODSINCE and paging by remote log ID. A page position is not a permanent change cursor: a previously seen record can change. Exact time-boundary semantics, concurrent-change coverage, returned revision fields, and provider-specific observation identity require validation. LoTW supports inclusive since filters; deliberate overlap is compatible with duplicate-safe processing.

No database, transaction layout, retention period, hashing scheme, or queue technology is selected. Checkpoint/processing coordination must meet the recovery behavior regardless of those choices.

## Accepted imported-success scheduling (D50)

A mapped positive imported upload status immediately suppresses resubmission to that service without prior verification. Retain the fact that this came from the source log; do not fabricate a Clacks Log attempt, provider response, or exact historical payload. Subsequent changes to service-relevant values make the contact pending for that service under the accepted correction rule. A confirmation flag alone does not establish our own upload status.

The supplied LOG4OM export includes sent/upload flags and dates but no complete history of the exact values sent in each prior attempt. Using imported current values as a future change-comparison baseline is therefore an explicit scheduling policy, not proof that these were the historical submitted values. Exact per-service status mapping and correction comparison remain open; prior verification is not required to suppress resubmission. D51 makes missing upload status pending, like explicit unsent status, independently per service. Exact mapping of unfamiliar provider-specific status values remains open.

## Accepted imports without positive upload status (D51)

An imported contact explicitly marked unsent or missing upload status becomes pending for that service under normal scheduling and cooldown. Evaluate each service independently; a positive QRZ flag does not suppress a LoTW upload. Destination configuration, valid signing inputs, and D47-D49 pause rules still apply. Exact interpretation of provider-specific status values remains to be researched rather than treating every nonempty value as success.

## Accepted imported confirmations (D52)

Mapped positive imported confirmation statuses immediately mark the QSO confirmed for that service without prerequisite verification or download. Retain supplied confirmation dates and source provenance; do not invent missing dates, fresh service observations, or the counterpart's submitted values. Missing confirmation data does not establish a positive confirmation. For updates to existing QSOs, selection of differing imported values follows D40-D42. Confirmation and our upload status remain independent under D32 and D50-D51. Exact service-specific flag/date mapping remains open.

## Remaining design work

- Define per-service accepted/confirmed representation, exact payload comparison, and destination selection.
- Establish matching and proof of acceptance for corrections, including provider-normalized values.
- Specify retries and uncertain outcomes with provider-specific recovery evidence.
- Define observation/update identity, durable processing, and cursor recovery so duplicate delivery has no second effect and new remote updates are not lost. D237 separates provider detail updates from recorded contact edits, superseding D36 replacement behavior; duplicate downloads must not become new updates.
- Set retention/history scope and reconcile external edits. D37 establishes explicit add/reject for unmatched downloads; D218 scopes rejection to the reviewed version, suppressing identical repeats while processing changed versions normally. Define retained disposition and provider update identity accordingly.
- Only then determine which LOG4OM history values map usefully and which necessary information is unavailable.

## Synchronization ownership (D248/D251)

All synchronized service evidence shares the account's single scope with account-owned QSOs and tags. Preserve publication attempts/outcomes with actual sent-value context, associated and unresolved remote observations, processing decisions, download progress and durable rejection dispositions. One external record has at most one associated local QSO within the account/service destination; different updates of that external record retain its established association. A QSO can independently associate with records in other services/destinations. Ambiguous local candidates require explicit selection of at most one; possible matches still require selection when unique. A competing outgoing QSO requires resolution rather than another association or silent external replacement. Preserve atomic processing-result/QSO-edit effects and required evidence after deletion. See [integration ownership](../docs/integrations.md#synchronization-ownership-d248).

## Automatic-upload eligibility (D245/D246)

Each tag independently selects Enable, Forbid or Neither for LoTW and QRZ, defaulting to Neither. Evaluate the QSO's full tag set: any Forbid blocks automatic uploads; otherwise at least one Enable is required. All-Neither and untagged contacts have no grant. Imported unsent/missing status remains pending under D51, but pending status does not override tag policy, routing or pauses. A shared QSO is not uploaded once per tag, and tag changes alone do not change its service payload. D109 explicit manual uploads retain existing eligibility/pause rules. Downloads search all account QSOs regardless of tag membership or upload policy.

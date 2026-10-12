# External logbook services

LoTW and QRZ are MVP integrations. The server executes publication and download jobs so connected desktop applications need not remain running. eQSL remains a later integration candidate.

## Operations and distinct states

The intended service workflows cover publication, recording service acceptance, retrieving confirmations, reconciling returned contact information, publishing corrections, and exposing incoming records that have no local match.

Publication acceptance is not remote confirmation. Received confirmation can exist regardless of whether we have published our own copy. Avoid one ambiguous confirmation-status field covering upload, acceptance, and a matched QSL.

Downloads of our submission/acceptance history and downloads of counterpart submissions or confirmations are distinct data sources. Their meaning and API availability must be defined per service, not conflated under download remote data.

## Accepted schedule

| Action | Behavior |
| --- | --- |
| Automatic upload | Periodic checks for eligible pending QSOs on both services; interval configurable. Discussion working default: one minute. |
| Cooldown | Working default: five minutes since the last edit. An edit resets eligibility time. |
| Upload pending now | Manual trigger bypasses cooldown, using accepted server state. |
| Automatic download | Configurable interval, default 30 minutes. |
| Check now | Manual download trigger. |

These defaults are product decisions, not claims that providers mandate these numbers. Provider recommendations should continue to be tracked with current documentation. A special startup schedule, retry/backoff policy, global rate limits, and exact behavior during provider outages are not settled.

An accepted edit to data included in a service submission makes that QSO pending for that service again. Which fields each service accepts and how corrections propagate need service-specific definitions. Changing unrelated local information should not automatically imply an external correction.

## Reconciliation context

The user requested a survey of established loggers' service reconciliation behavior before inventing new policies. The preserved handoff discusses candidate applications, but a comprehensive survey of every available logger's LoTW/QRZ/eQSL reconciliation behavior has not been completed or recorded here. Continue from concrete incumbent behavior and primary evidence rather than treating proposed policies as findings from that survey.

Jacob has observed LOG4OM correcting local county, DXCC, and ITU information from downloaded confirmation data. LOG4OM also offers an option to create local records for incoming confirmations without a match. Preserve those observations when evaluating behavior; do not assert they are universally implemented or make their exact policies requirements without a decision.

Confirmation data can supply missing or corrected grids, county, city, or other contact information. This is not only a status update. Per-field provenance, replacement policy, matching rules, user intervention, and unmatched-record creation remain open.

The premise that a confirmed QSO routinely needs identity/time correction was challenged: confirmation already implies matching within the service's tolerance. Investigate realistic metadata corrections and exceptional identity corrections separately.

## Service ownership and credentials

The server needs access to the credentials/capabilities required by each service: LoTW signing certificates/private keys, appropriate LoTW query credentials, and QRZ API access. Exact subscription/access requirements, credential scoping, storage, and revocation UX remain to be designed.

Only accepted shared QSO state is publishable. Clients can log offline but cannot run an independent competing publication pipeline. Server jobs must preserve per-service outcomes and make actionable failures visible.

## WS Suite capture

[The WS source investigation](../references/ws-suite-udp.md) verifies both primary-protocol completed-QSO messages and the separate plain-ADIF broadcast in WS 3.2.1 build 261010. Primary type 5 and type 12 are emitted for the same contact; the ingestion design must avoid duplicate records.

WS supplies our station callsign, grid, and optionally operator identity, alongside the contact facts. It does not generate our administrative location, zones, or POTA references in the inspected ADIF path. Prefer supplied field values when present; use the client's current defaults to fill missing operating context. Do not add discrepancy notices or reconciliation for now. Automatic catch-up from WS's local ADIF log is outside MVP. Empty-value semantics, application routing, and manual import scope remain open.

## Primary references

- [ARRL submission guidance](https://lotw.arrl.org/lotw-help/developer-submit-qsos/)
- [ARRL acceptance and confirmation queries](https://lotw.arrl.org/lotw-help/developer-query-qsos-qsls/)
- [QRZ Logbook API documentation](https://www.qrz.com/docs/logbook30/api)

ARRL distinguishes upload from subsequent acceptance/confirmation and discourages routine resubmission of unchanged contacts. The discussion did not establish a provider-mandated numeric upload/download cadence. Earlier references to QRZ semi-random delays or LoTW live-upload discouragement should not be presented as verified requirements without supporting current documentation.

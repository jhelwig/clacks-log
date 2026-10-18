# Service destination routing

Accepted product/architecture direction (D53), 12 October 2026. Exact configuration, credential eligibility/overlap rules, and schema remain to be designed; no implementation has started.

## Evidence

The inspected LOG4OM export includes 132 W7HEX contacts and 242 KD8CGM contacts. Historical station identity matters independently of the client's current profile.

[QRZ's application programming interface (API) guide](https://www.qrz.com/docs/logbook/QRZLogbookAPI.html) documents callsign-specific logbooks with configured date ranges. Callsign suffixes are significant, and each logbook's API key supplies its routing context. [American Radio Relay League (ARRL) submission guidance](https://lotw.arrl.org/lotw-help/developer-submit-qsos/) documents certificate selection/validation from the radio contact (QSO)'s station callsign and eligible QSO dates. Exact Logbook of The World (LoTW) portable-callsign handling and overlapping-certificate selection must come from the reference rules, not an assumed QRZ-style exact string comparison.

## Accepted routing direction (D53)

Minimum viable product (MVP) supports multiple QRZ logbook configurations and multiple imported LoTW certificates within the user's log. Each upload uses the QSO's saved station identity, date, and operating context to select an eligible configured destination/signing credential. Current client defaults are not used to reroute historical QSOs.

QRZ routing uses the configured logbook callsign and date range. LoTW credential eligibility follows the source-guided port and D17's exclusion of expired/superseded certificates; D229 requires explicit supersession with coverage gaps shown before applying it; import must not infer supersession from matching callsign or later expiration. Multiple eligible credentials remain subject to D53’s ambiguous-routing pause. Exact eligibility and overlap configuration remain specification work. LoTW query credentials identify the account, not necessarily a single station callsign.

Missing or ambiguous routing pauses only affected uploads with diagnostic information under D47-D49. Do not silently fall back to another station identity or destination. Unrelated eligible uploads continue. Exact enrollment/configuration, ambiguity resolution, and service-download coverage remain open.

D53 accepts multiple service configurations and automatic per-QSO routing in MVP. Missing/ambiguous routing follows the affected-upload pause and explicit remedy/resume rules of D47-D49.

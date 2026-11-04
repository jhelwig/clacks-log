# WS Suite (the WSJT-X family of weak-signal radio applications) User Datagram Protocol (UDP) logging fields

Source investigation dated 11 October 2026. Findings below are from the official **WS 3.2.1 build 261010 standard graphical user interface (GUI) source package**, a beta release. Source inspection does not establish that every older WS/WSJT-X build has identical fields.

## Sources

- [Official WS source package](https://sourceforge.net/projects/wsjt-x-improved/files/WS_v3.2.1/Source%20code/ws-3.2.1_261010.tgz/download). The distribution archive contains a nested `src/ws.tgz`; source paths below are inside that archive's `ws/` directory.
- [WS User Guide](https://wsjt-x-improved.sourceforge.io/ws-main_en.html), Reporting and General sections.
- Upstream WSJT-X source was also inspected at commit `b4f9a431bcf6449df8f37b56de79d48b665b044c`; WS-specific findings were verified against the WS package, not assumed from upstream.

This note records protocol behavior rather than importing WS implementation code into Clacks Log.

## Logging paths

The primary UDP server carries the WSJT-X-style binary protocol. A completed radio contact (QSO) emits both **QSO Logged, type 5**, and **Logged Amateur Data Interchange Format (ADIF), type 12**. The latter wraps an ADIF record in the binary message protocol; it is not a standalone plain-text UDP record.

A separately configured Secondary UDP Server can send a plain ADIF record ending with `<eor>`, intended for destinations such as N1MM Logger+. That path is separate from type 12.

References: `widgets/mainwindow.cpp`, `MainWindow::acceptQSO`, lines 12339 onward; `Network/MessageClient.cpp`, `qso_logged` and `logged_ADIF`, lines 700 onward. `Network/NetworkMessage.hpp` describes the base protocol, including Qt serialization.

## Supplied field map

| Meaning | Type 5 field | Type 12 ADIF representation |
| --- | --- | --- |
| Other station | DX (the other station) call | `CALL` |
| Other station grid | DX grid | `GRIDSQUARE` |
| Start and end | Date and Time On/Off | `QSO_DATE`, `TIME_ON`, `QSO_DATE_OFF`, `TIME_OFF` |
| Frequency | Frequency integer in Hz | `FREQ` in MHz; `BAND` also generated |
| Mode | Mode string | `MODE`, with `SUBMODE` for FT2, FT4, FST4, and Q65 in this build |
| Reports | Report sent/received | `RST_SENT`, `RST_RCVD` |
| Our station callsign | My call | `STATION_CALLSIGN` |
| Our grid | My grid | `MY_GRIDSQUARE`, omitted if empty |
| Operator callsign | Operator call | `OPERATOR`, omitted if empty |
| Power entered in the logging dialog | Tx power | `TX_PWR`, omitted if empty; not the computer-aided transceiver control (CAT) power reading |
| Comments and name | Comments, Name | `COMMENT`, `NAME`, omitted if empty |
| Contest exchange | Exchange sent/received | Derived `STX`, `SRX`, and activity-dependent fields; not a general verbatim copy of both strings |
| Propagation | ADIF Propagation mode | `PROP_MODE`, omitted if empty |
| Satellite | Appended satellite string | `SAT_NAME`, omitted if empty |
| Satellite mode | Appended satellite-mode string | `SAT_MODE`, omitted if empty |
| Receive frequency | Appended receive-frequency string | `FREQ_RX`, omitted if empty |

Reference: `logbook/logbook.cpp`, `LogBook::QSOToADIF`, lines 119-238; `Network/MessageClient.cpp`, lines 700-735. Empty fields are not uniformly omitted: for example the ADIF generator emits `GRIDSQUARE` even if its value is empty.

The satellite and receive-frequency fields are appended by the WS serializer but are absent from the type 5 header comment's field list. Treat the serializer as evidence for this build and preserve compatibility with older shorter messages.

## Where values come from

`widgets/logqso.cpp` initializes station callsign and grid from WS configuration and operator identity from Op Call. The logged operator value is taken from the log dialog. Automatic logging uses this same logging path without requiring the dialog to remain visible.

Frequency semantics are configurable: WS can log dial frequency or dial frequency plus transmit audio offset. The logged frequency is therefore not universally the dial frequency. Preserve the supplied frequency rather than adding an offset again.

WS also offers logging transformations including shortened grids, a missing-grid placeholder, and a JT9-to-RTTY option. A field being supplied does not prove its value was directly received over the air or that it retains the maximum available precision. Preserve source/configuration context when interpreting it.

The ADIF record sent via type 12 and the record appended to the configured Logbook location originate from the same `QSOToADIF` result. UDP ADIF does not inherently provide extra fields absent from that generated record. Optional fields in the map above describe protocol capabilities.

`TX_PWR` comes from the log dialog's Tx Power text field (`m_txPower = ui->txPower->text()`), optionally retained using SaveTxPower/TxPower settings. It is not populated from CAT telemetry in this logging path. WS can separately display CAT-reported output power while transmitting; that display is not wired into the logged Tx Power field in the inspected build.

Jacob reports that his current ADIF log contains `CALL`, `GRIDSQUARE`, `MODE`, sent/received reports, start/end dates and times, `BAND`, `FREQ`, `STATION_CALLSIGN`, `MY_GRIDSQUARE`, and sometimes `STATE`. This is deployment information supplied by the user, not a packet capture; absent optional fields are consistent with empty logging values.

References: `widgets/logqso.cpp`, lines 269-282 and 365 onward; `widgets/mainwindow.cpp`, lines 12308 onward; WS User Guide Reporting settings.

## Enrichment and absent fields

The ADIF generator can provide the **other station's `STATE`** from an RTTY exchange, or from a configured grid/callsign lookup when not in a special activity. Field Day can produce `CONTEST_ID`, `SRX_STRING`, `CLASS`, and `ARRL_SECT`; contest serials can produce `STX`/`SRX`.

The inspected generator does not supply our `MY_DXCC`, `MY_STATE`, `MY_CNTY`, `MY_CQ_ZONE`, `MY_ITU_ZONE`, or Parks on the Air (POTA) references (`MY_SIG`/`MY_SIG_INFO` or counterpart equivalents). Those may come from Clacks Log context or later enrichment; do not confuse counterpart `STATE` with our operating location. Do not reinterpret a free-form comment or contest number as a POTA reference without an explicit rule.

## Planning implications and open decisions

- Station callsign, own grid, and operator can overlap Clacks Log defaults. Prefer supplied fields when present and fill missing operating context from current client defaults. No discrepancy notices or reconciliation workflow are planned for now. D80 treats empty live station-context fields like omitted fields for current-context fallback.
- D79 selects ADIF type 12 as preferred completed-QSO input, type 5 fallback when ADIF is absent, and correlation of paired notifications into one contact. Type 12 carries optional enrichment and explicit mode/submode fields. Exact correlation and fallback timing remain open.
- D79 requires one QSO per logging event, including paired or repeated notifications. Handling forwarded/repeated packets and duplicate capture across clients needs design.
- The message Id identifies the sending application instance; it is not a stable per-QSO universally unique identifier (UUID). Create our own stable QSO identifiers and define import deduplication separately.
- A nonempty supplied operator takes precedence over client defaults. An absent or explicitly empty operator may be filled from the current client context under D80.
- Do not change current logging defaults simply because an incoming QSO contains different values.
- D81 configures one capturing Clacks client per WS source, using its defaults and delivering the QSO to other clients through sync. Exact source setup/identity and accidental duplicate capture remain open.
- UDP message reception is not a verified durable replay mechanism. Automatic catch-up from the WS ADIF log is explicitly outside minimum viable product (MVP); ADIF file import is MVP under D31, with historical-context and comparison rules under D38-D43.

No live packets or local WS settings were inspected. Field population, routing, forwarding, and configuration on the actual deployment need later validation.

## In-progress DX call and lookup (D91)

D91 requires Clacks to follow WS DX-call UDP updates and run directory lookup after a debounce on actual call changes, including locally edited Clacks calls. QSO completion is not the primary lookup trigger. D92 makes local edits lookup-only in MVP and defers sending WS DX-call changes to post-MVP. D93 makes incoming WS DX-call values authoritative over local edits. D94 sets lookup debounce to 500 ms after the last effective Clacks DX-call change. D116 makes empty incoming calls authoritative, clearing Clacks's call and associated lookup/history display; stale display responses must not repopulate it.

[Official upstream WSJT-X protocol source](https://raw.githubusercontent.com/WSJTX/wsjtx/master/Network/NetworkMessage.hpp), checked 2026-10-12, documents Status type 1 with DX call and DX grid, and status emission when DX Call changes. This verifies the upstream protocol capability, not a live capture of the installed WS build or a new inspection of WS-specific status serialization. Prior WS source investigation in this note focused on completed-QSO paths.

Status updates that leave Clacks's effective call unchanged must not continually postpone lookup. Under D93, a repeated WS call still replaces a different locally entered Clacks call and restarts debounce because the effective call changed. Status changes only the shared entry callsign; Worked Before reads that field.
Capture independently requests directory information for its own projected
QSO's key, reusing a 24-hour/128-result transient success or joining in-flight
work, and starting on a miss without delaying save. The actual save caller
retains its shared handle with its QSO identity; Status has no completed-QSO
association. This corrected #21 contract supersedes the earlier form-owned
lookup interpretation. In-progress display state is not a completed QSO.
Authored protocol/receiver tests verify Clacks wiring; they do not establish
installed GridTracker Generate Messages forwarding or WS configuration.

## Activity references versus exchanges

Planning clarification 2026-10-12: a contest identifier is activity/reference metadata and does not imply a dedicated contest subsystem. D127 accepts a shared typed context pattern with POTA references. D128 explicitly includes optional phone/continuous-wave Morse code (CW)/digital signal reports in the QSO model, separate from contest exchanges. [ADIF 3.1.6](https://www.adif.org/316/ADIF_316.htm) defines CONTEST_ID, MY_POTA_REF/POTA_REF, and separate STX_STRING/SRX_STRING exchange fields. A shared internal pattern must retain those distinct export meanings and cardinality; D129 places dedicated sent/received exchange recording post-MVP, allowing free-text comments in MVP; exact historical-import field retention remains open.

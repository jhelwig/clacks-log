# GridTracker 2 callsign-selection research

Checked 2026-10-12. This is source/documentation research, not a live capture or a chosen implementation. The integration under investigation is GridTracker callsign selection driving Clacks's Worked Before view.

## Provenance

[Official site](https://www.gridtracker.org/) links to [GridTracker 2](https://gitlab.com/gridtracker.org/gridtracker2). Inspected commit `8c421906cf9a97b45b30f2a2943e1ac51963dd5a`, committed 2026-10-11. The older GridTracker repository has a 2024 master head and was not used as evidence for current GridTracker 2 behavior. Source files were fetched through the public GitLab application programming interface (API) for inspection; application code was not copied into this repository or executed.

## Verified paths

- [`wsjtx.js`, lines 1372-1462](https://gitlab.com/gridtracker.org/gridtracker2/-/blob/8c421906cf9a97b45b30f2a2943e1ac51963dd5a/src/renderer/lib/wsjtx.js#L1372): `startGenMessages` calls `setCallAndGrid`. For a valid WS instance, with Tx not enabled, this sends Configure type 15 with callsign/grid and Generate Messages set. It refuses the generation path when Tx is enabled. This is distinct from `initiateQso`, which sends Reply type 4. These distinctions matter when identifying the supported selection action.
- [`wsjtxUdp.js`, lines 224-241 and 485 onward](https://gitlab.com/gridtracker.org/gridtracker2/-/blob/8c421906cf9a97b45b30f2a2943e1ac51963dd5a/src/renderer/lib/wsjtxUdp.js#L224): incoming WS User Datagram Protocol (UDP) is forwarded when forwarding is enabled. An identical-packet suppression check runs first. Forwarding uses configured destinations/port. This supports a path where Clacks receives WS status through GridTracker.
- [`wsjtx.js`, lines 58-76](https://gitlab.com/gridtracker.org/gridtracker2/-/blob/8c421906cf9a97b45b30f2a2943e1ac51963dd5a/src/renderer/lib/wsjtx.js#L58): GridTracker itself reacts to changed status DX (the other station) call with callsign lookup. Clacks's desired behavior is separately selected by D91/D93/D94.
- [`qso/adif.js`, lines 1662-1670](https://gitlab.com/gridtracker.org/gridtracker2/-/blob/8c421906cf9a97b45b30f2a2943e1ac51963dd5a/src/renderer/lib/qso/adif.js#L1662): the named LOG4OM integration sends completed contacts in Amateur Data Interchange Format (ADIF); a contact is called a QSO to its configured endpoint. The inspected LOG4OM references do not establish a dedicated direct lookup-only selection protocol.
- [LOG4OM manual, p. 53](https://www.log4om.com/l4ong/usermanual/Log4OMNG_ENU.pdf), primary-source indexed section, describes callsign entry by manual input, spot selection, or UDP from programs including WS/GridTracker (GT). [Advanced manual](https://www.log4om.com/l4ong/usermanual/Log4OMNG_ENU_ADV.pdf) describes JT_MESSAGE rebroadcast setup. These are documentation evidence, not evidence of a deployment's actual settings.

## Selected integration action (D115)

D115 selects Generate Messages via WS DX-call status to Clacks, driving its call field, directory lookup, and current-logbook history. Source supports this path; actual forwarding/source setup still needs validation. A separate direct lookup-only GT-to-LOG4OM protocol is not needed for this identified workflow.

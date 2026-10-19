# Contest loggers: serial assistance and exchange data

Researched 18 October 2026 from official project manuals and help. This complements [the organizer-rule survey](contest-exchanges.md). A radio contact is called a QSO; Morse code operation is abbreviated CW, and radio teletype is RTTY. Amateur Data Interchange Format (ADIF) and Cabrillo are interchange/submission formats, not descriptions of a logger's internal database schema.

Dedicated contest-exchange recording remains post-minimum viable product (MVP) under D129. The findings inform boundaries in the native model; they do not add contest operation to MVP.

## Finding

The user's distinction is supported: these loggers provide **serial-number operating assistance as well as recorded exchange information**. A next-number allocator, reservation, message substitution and formatting are different responsibilities from remembering the particular number sent or received in a completed QSO. They also provide contest-specific exchange fields/defaults beyond serials. Separate facilities do not establish that these must be separate database tables, nor that every exchange element should become an activity-reference identifier.

## N1MM Logger+

- **Contest setup and serial allocation.** Each contest log represents an occurrence of a contest. Its Sent Exchange setting primarily controls Cabrillo output; message templates need not transmit it unless they use the appropriate macro. A serial placeholder enables numbering. A serial-number server coordinates multi-radio/multioperator operation, reserving a number during entry before transmission. Reserved numbers can produce gaps or appear out of chronological order; the documented priority is avoiding duplicate sent numbers and logging the number actually sent. The setup page's cancellation descriptions differ between its main explanation and later addendum, so this survey does not assert one universal release/reuse rule. [Contest setup, Sent Exchange and serial server](https://n1mm.hamdocs.com/setup/contest-setup/).
- **Stored/contact-entry values and other elements.** The User Defined Contest mechanism exposes distinct auto-numbered sent and numeric received number fields, alongside report, additional exchange, section, grid, name and miscellaneous fields. Definitions select and label fields; they are not unlimited named-field schemas. The guide warns that user definitions cannot express every programmed contest behavior. [User Defined Contest editor](https://n1mmwp.hamdocs.com/appendices/udc-editor/).
- **Transmission assistance.** `#` represents the serial number; `{EXCH}` expands the configured exchange, while `{SENTRST}` takes the entered signal report. Macros also support number abbreviations and voice operation. A configured exchange string and an arbitrary hardcoded message can disagree; configured data alone is not proof of the literal transmission. [Function keys, messages and macros](https://n1mm.hamdocs.com/setup/function-keys/).
- **Corrections and prefill.** Editing the last logged sent number lets numbering continue from that correction. Contest-specific validation can be overridden to log what was received. Call History can prefill empty exchange fields from earlier logs or supplied data; Sweepstakes check/year and section are examples. Prefill is an entry aid, not independent evidence of what the other station transmitted. [Frequently asked questions](https://n1mm.hamdocs.com/faq/), [Call History lookup](https://n1mm.hamdocs.com/setup/call-history/).
- **Export.** ADIF export may use application-specific exchange tags: Field Day class is documented as `APP_N1MM_EXCHANGE1`. Exporting all contests together can lose expected contest-specific mappings; individual-contest export is recommended for those fields. Cabrillo output uses contest setup as described above. Do not infer a native schema from those export tags. [Entry window, export](https://n1mm.hamdocs.com/manual-windows/entry-window/).

## Log4OM 2

The official basic manual describes contest/date-range setup with either a fixed sent string or automatically generated numbers, including starting number and prefix/suffix. The displayed sent number increments automatically. Its CW keyer uses contest sent/received fields: `<CONT_TX>` and `<CONT_RX>` are distinct from the signal-report substitutions `<STTX>` and `<STRX>`. This verifies separate serial assistance and contest exchange values alongside ordinary reports; it does not establish arbitrary structured multi-element exchanges or coordinated network allocation. [Official basic manual, printed pages 94–96](https://www.log4om.com/l4ong/usermanual/Log4OMNG_ENU.pdf).

**Documentation caveat:** that manual illustrates an older separate contest window. Official release notes say version 2.14.0.0 integrated its fields/functionality into the main interface. The capabilities above are documented, but the old screenshot is not evidence of the current layout. [Official release notes, 7 June 2021](https://www.log4om.com/page/5/). Exact present-day allocation/reuse rules, network coordination and complete exchange-export mapping were not established by this source set.

## DXLog

DXLog provides a particularly explicit data-driven comparison:

- Its contest definitions specify starting serial and sequence scope: across the contest or per band, mode, period, and supported combinations. Received exchange slots have contest-defined names, types, visibility, requiredness and validation; number types must actually mean serials because types affect export. Definitions separately map fields into Cabrillo lines and ADIF, including application-specific fields and additional mappings. These are documented definition/interface semantics, not an independently audited database design. [Contest definition reference](https://dxlog.net/docs/index.php?title=Contest_definition).
- A number server reserves serials for multiple stations or two-radio operation. Reservation can happen with the space key, when sending the exchange, or with `$RESERVENR`; the number remains reserved until logging or clearing entry, subject to a setting protecting an already-sent reservation while correcting a callsign. The documentation warns that its network number-server functionality is not robust across stations joining, leaving or restarting. That warning limits what can be concluded about failure recovery. [Additional information, using a number server](https://www.dxlog.net/docs/index.php?title=Additional_Information).
- Message substitutions distinguish `$SERIAL`, `$RST` and `$EXCHANGE`; number abbreviation settings govern their transmission rendering. Reservation may block logging until a number is obtained. This is operating machinery around contact values, not an alternative to recording those values. [Menu options, number sending and number server](https://www.dxlog.net/docs/index.php/Menu_Options).

## Win-Test

Win-Test separates configured station exchange from serial numbering: its new-log instructions leave Exchange empty for serial-only events while using it for a zone or state in other contests. Its message variables distinguish `$SERIAL`, `$RST`, `$STATE`, `$YEAR` and `$CHECK`; `$RAWSERIAL` controls unabbreviated, padded output. A logged QSO's sent number can be corrected, separately from editing the ongoing sequential number. The network documentation describes log synchronization rather than a proof of a particular reservation algorithm. [New log](https://docs.win-test.com/wiki/Menu%3AFile_new), [Message variables](https://docs.win-test.com/wiki/Message_variables), [QSO editing](https://docs.win-test.com/wiki/Menu%3AWindows), [Serial correction and synchronization](https://docs.win-test.com/wiki/Multi-op/Issues/Editing_serials).

## WriteLog

WriteLog's `SEQ` entry displays the number to transmit and log. It calculates a candidate while entering a callsign and locks it when leaving that field or transmitting a function-key message, so another station's new QSO does not change this contact's sent number. A logged serial can be corrected, with a separate choice to reset the allocator. Its contest modules configure exchange fields; Sweepstakes has received number, precedence, check and section, while other contests use different elements. `%N` sends the serial with optional padding; `%P0` repeats the previous logged serial. Numbering scopes include all bands, per band and per network radio. [Official 12.94 help, QSO Entry, Serial Number Correction, message memories, serial numbering and exchange-field sections](https://writelog.com/Downloads/writelog1294.pdf).

Voice transmission can assemble digits from recordings, another serial-specific aid that does not change the meaning of the saved number. [Official voice-file transmission instructions](https://writelog.com/wav-files-for-transmission).

## Consequences for Clacks

These are research recommendations, not newly accepted requirements:

1. **Keep participation/reference associations conceptually distinct from exchanges.** An event association identifies the activity; a park/island identifier identifies a reference. A QSO serial is a per-contact exchange value, not the event's identity or a persistent station reference.
2. **Distinguish serial operating state from completed-contact facts.** Future automation needs a sequencing policy and allocation/reservation lifecycle. The saved contact needs explicit sent and received values where recorded. A serial must not be reconstructed later from its chronological position or current QSO count: reservations, missing contacts, corrections and concurrent stations make that unreliable.
3. **Do not reduce all other exchanges to one universal number.** Contest-specific additional elements still exist, and received values may be optional, conditional or text. Defaults and prefill can assist entry; message macros can render them. Neither removes the need to preserve the selected per-contact exchange when dedicated recording is implemented.
4. **This research does not choose physical storage or a general schema engine.** Dedicated fields, event-defined values, or composed structures remain possible. Existing logger interfaces support separation of responsibilities, not a claim that a particular Rust type or database organization is required.

The immediate reference-group decision can therefore be made without turning every association into a contest-exchange record. Preserve the agreed account-shared group definitions, group-only participation, group-defined reference-value requirements, and unknown-association behavior. Design dedicated exchange recording and serial automation together when their post-MVP scope is taken up.

## Evidence limits

This was a representative manual survey, not runtime testing or an internal-schema/source audit. No claim is made that every supported contest follows an identical storage layout, that all exports retain all exchange values, or that allocator behavior remains safe during arbitrary disconnection. The source-specific limits above should remain visible when these observations inform a later design.

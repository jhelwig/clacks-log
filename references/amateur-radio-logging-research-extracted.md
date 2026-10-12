# Amateur radio logging research extracted text

Convenience extraction of the supplied DOCX, including external hyperlink targets. Paragraphs and table cells are flattened; the original document is authoritative for layout and structure. Historical findings do not override current Clacks Log decisions.

What operators want from amateur radio logging software

Research brief for Clacks Log  |  11 October 2026

Main finding. The recurring need in this qualitative sample is a trustworthy contact history that survives real operating routines: specialist applications, portable capture, imports, station changes and confirmation services. A simpler interface can attract attention, but reliable integration, familiar operating behavior and confidence in the record often decide whether someone moves or stays.

This brief supports product discovery. It examines demand, switching behavior and operating sequences; it does not prescribe an implementation or select a winning logger.

How to read the demand evidence

The tiers below describe patterns in a purposive, nonrepresentative sample of public operator accounts, support discussions, issues and manuals. They are not a numerical ranking. Recurrence means independent examples across sources; intensity means demonstrated consequences or effort. A severe accessibility failure can matter greatly even when only one person reports it.

Broad recurring need: reliable records and interoperability. Duplicate round-trips, missing award fields, incomplete backup scope and upload-state repair recur across Ham2K, Wavelog and QLog. The intensity comes from lost information, repeated manual correction and backup scripts, rather than votes or praise. [2, 3, 6, 7]

Broad recurring need: preserve the operating routine. Actual comparisons repeatedly favor working capture and integrations over visual appeal. Radio compatibility, platform transitions and confidence in established tools can outweigh a cleaner screen. [9–14]

Recurring within a segment: device and location continuity. Portable and multi-station operators ask for consistent history across devices. Others deliberately export and discard their field logs. Shared online access, portable history and independent offline edits must be investigated separately. [1, 5]

High intensity but narrower needs. Self-hosted integration and keyboard or screen-reader access have concrete advocates. The former includes an operator already building a private server; the latter can prevent basic navigation. These signals justify focused research, not claims of mass demand. [4, 8]

What this research cannot quantify

Public contributors overrepresent people with strong opinions and publicly indexed projects. Manuals establish behavior, not popularity. The 2020–2021 MacHamRadio usage poll allowed multiple selections and sampled an Apple-oriented readership; it cannot rank current feature priorities. No representative current feature-priority survey was established in this research pass. [26]

What makes operators switch or stay

Behavior is stronger evidence than a wishlist. The cases below separate a completed change from a trial, retention or stated preference. Historical accounts describe the situation at their publication date; they are not current comparative product tests.

Completed changes

HRD to Wavelog, April 2025. VK7BEN reports moving after roughly 20 years on HRD. Windows 10 retirement and dissatisfaction with development motivated the change. Self-hosting reduced concern about a hosted service disappearing; keeping one-click WSJT-X logging made adoption easier. Older contest and portable imports were still unfinished. A working new daily routine can precede complete migration. [9]

HRD to Log4OM, December 2025. An operator who already owned and liked HRD changed after reporting inadequate support for a new radio. The specific rig and missing functions were not identified. This is a compatibility-triggered switch, not a verified present-day defect. [10]

Retention and unsuccessful alternatives

Automation beat a cleaner interface, December 2024. A multimode operator liked QLog’s appearance but kept Log4OM’s automatic ingestion of other programs’ ADIF files, uploads and spot-to-rig tuning. Their comparison involved setup difficulties and historical versions. It shows the value of the existing routine; it does not establish today’s QLog limitations. [11]

Even a self-built replacement did not displace the incumbent, December 2024. A technically capable operator built a smaller logger with several integrations yet continued exporting into and using Log4OM, despite lag. The account does not isolate the precise missing capability. Treat it as evidence of retention and migration complexity, not proof that new products cannot succeed. [12]

Depth and support reward learning, 2024–2025. DXLab reviewers describe peer influence, support, award work, search and bulk editing as adoption or retention benefits. Other reviewers describe setup effort or repeated station-data entry. The same depth can produce loyalty for an experienced DX operator and friction for a newcomer. Favorable review-site selection limits generalization. [13]

An attractive interface does not guarantee adoption, May 2025. A Mac operator found Wavelog beyond their understanding and preferred MacLoggerDX; another contributor reported a successful Wavelog move. The first account does not reveal whether deployment, administration or operating UI was the obstacle. [14]

Contradictory segments are genuine

One PoLo user wants phone and Chromebook history, while another exports each activation to a home log and deletes local field history. Prior-contact knowledge matters to some; disposable capture suits others. Likewise, technically capable self-hosters and operators seeking minimal setup need different onboarding paths. A single “modern logger user” persona would hide these differences. [1, 4, 14]

Field operation and choosing the next contact

These are reported or documented task sequences. They distinguish preparation, on-air attention and later record maintenance. They should be observed directly in follow-up sessions before treating them as universal patterns.

Portable capture can defer work intentionally

K4SWL describes setting up for a filmed activation, listening and writing contacts on paper, photographing the pages, then entering them through POTA’s manual form with shared session details already set. Simultaneous paper-and-phone logging caused fumbling between contacts. Callsign checks help during transcription. He uses PoLo when not filming and computers for longer sessions. Minimizing interruption during the exchange can matter more than minimizing total work. [15]

Cold and wet conditions change error recovery

In a SOTA field account, an operator who forgot paper logged with frozen fingers, retyped frequency and took screenshots for reassurance. Replies explained existing persistence and shortcuts. The friction was partly discoverability and trust. Test whether an operator can find the relevant shortcut outdoors and recognize a saved contact without relying on an improvised backup. [25]

Multiple people and award identities complicate export

A two-operator outing with two POTA references and one VEFF reference produced six export files. Reciprocal contacts were absent from one operator’s outputs. The confirmed fix was entering both callsigns so exports distributed contacts while excluding self-QSOs. The operator, transmitting station identity and award reference are separate pieces of context. [16]

A different two-park activation produced a long manual repair routine after use of a full/master ADIF export. Support distinguished that file from award-expanded POTA-specific exports and advised selecting the relevant export files. The operator had finished via the workaround; successful use of the new advice was not confirmed in the thread. Test export purpose and previews, not only whether ADIF is produced. [17]

Listening knowledge may never become a QSO

A CW DX hunter tunes, copies a call or partial prefix, decides the station is not needed, and wants a private bandmap marker before moving on. This avoids repeatedly identifying the same signals. The proposed skimmer workflow did not fit his preferred listening process; the maintainer disputed parts of his technical explanation. The supported need is preserving what he heard, not a finding that skimmers are inferior. [20]

A later contact can be about remembering a person

SKCCLogger’s manual describes entering a call, retrieving membership and prior contacts, checking same-band history, attaching notes and reopening a previous record for editing. Optional end time supports ragchew duration. This establishes a workflow in which history, notes and corrections accompany contact capture; the manual does not establish how widely those features are valued. [24]

Contest and digital workflows expose hidden state

A contest contact follows an operating state

In N1MM’s documented Enter Sends Message flow, a Run operator calls CQ, enters a responding call, sends the exchange, enters the received exchange, then completes and logs. An invalid exchange can request a repeat. Search and Pounce changes the order of calling and exchanging. Corrected-call handling differs between CW and voice. Enter is therefore an action in an evolving conversation, rather than a universal save command. [18]

Rob Locher describes a specific exception: after requesting a repeat in Search and Pounce, he manually sends the exchange, then presses Enter to log. The program sends the exchange again, requiring Escape. His notes also cover editing the previous QSO and changing operator. Observe deviations from the happy path, highlighted next actions, keyboard focus and the ability to recover without losing the contact. [19]

Digital operation often has a deliberate chain of owners

A January 2026 operator discussion describes WSJT-X forwarding through GridTracker to Log4OM, which uploads to several services. Other operators defer portable records until later. A successful upload and another station’s eventual confirmation are separate events. Ask which program owns capture, enriched metadata, the lifetime record and each remote-service status. [21]

GridTracker documentation recommends a separate master log and explains that lookup enrichment does not backfill WSJT-X’s ADIF. A contact can therefore be present in two places with different metadata. Merely comparing record counts will not establish that the histories agree. [22]

A saved QSO passes through several distinct states

Use these states as research vocabulary, then ask operators how they recognize each one. This is a synthesis of the workflows above, not a mandated architecture:

Captured locally, with the actual station, time, location and exchange.

Copied or imported elsewhere, with duplicates and field preservation checked.

Queued and sent to each destination, with failures visible and retry behavior understood.

Accepted by that destination, which is different from being matched or confirmed.

Corrected later, with a known way to propagate or deliberately reconcile the correction.

QLog issue 1029 described an import that required per-contact LoTW/eQSL queue-state changes. The retrieved issue was closed and marked Done/testing_0.51, so it is historical evidence of consequential status semantics, not a claim of a current defect. [7]

Backup confidence includes more than an ADIF export

A Wavelog user praised the migration yet scripted nightly backups and asked for clearer whole-system recovery, including configuration and scanned QSL material. The issue was closed when retrieved. The case shows both willingness to stay and substantial effort protecting the complete station record. [6]

Offline continuity and the next research step

Separate six meanings of sync

These distinctions are essential when interviewing users or evaluating software claims:

Offline capture. New contacts can be recorded without internet; lookup databases, maps or services may still need preparation or connectivity.

Shared online database. Several clients reach one master. A Log4OM multi-station request led to centralized database advice; that does not demonstrate independent offline writing. [5]

Live forwarding. One program sends a contact to another. Wave-Flex Integrator documents no retry, buffering or offline queue when Wavelog is unavailable. [23]

Import and export. Files move records. A PoLo–Wavelog round-trip created duplicates; a separate import discussion showed lost SOTA context. Neither proves automatic conflict reconciliation. [2, 3]

Service exchange. LoTW, eQSL or QRZ upload/download states describe service interactions. They do not establish agreement between two local editable logs.

Offline multi-device reconciliation. Independent devices can reconnect and converge while preserving changes, identities and metadata. Duplicate detection alone does not define what happens to conflicting edits or deletions.

Validation implications

The most relevant validation is a real disconnected outing followed by return to the shack. Include new contacts, edits to different records, competing edits to the same record, location changes, repeated transfers and a service outage. Check the resulting history and recovery effort. These are questions to validate, not a decision to build a particular database or protocol. The independent private-LoFi-server case supports exploring this niche. [4]

Questions for the next operator interviews

Show the last contact that was difficult to capture or correct. What happened on the radio while you repaired it?

Which programs do you intentionally keep? What would break if one disappeared?

When did two copies of your log last disagree? Show the recovery and estimate the time spent.

What must work without internet, and what may wait until you return home?

What finally caused your last switch, or stopped a trial? What did you leave behind?

Which matters more in a forced trade-off: setup simplicity, familiar controls, device continuity, integrations, cost or full control of hosting? Why?

Recruit satisfied incumbents as well as switchers, portable operators, contesters, casual CW users and digital operators. Include accessibility needs and less technical users. Record denominators within each segment before attempting frequency claims.

Source ledger

Direct sources checked 11 October 2026. Dates below are publication or discussion dates; “undated” identifies documentation without a verified publication date. Operator accounts are self-reports. Issue closure does not by itself prove every related behavior is fixed.

[1] [Ham2K  Multiple devices](https://forums.ham2k.com/t/multiple-devices/1289)  |  12–23 Sep 2025

[2] [Ham2K  Merge QSOs on ADIF import](https://forums.ham2k.com/t/feature-request-merge-qsos-when-importing-adif/1678)  |  7 Jun 2026

[3] [Ham2K  SOTA reference lost on import](https://forums.ham2k.com/t/sota-ref-information-lost-on-imported-qsos/277)  |  Nov 2024 and Sep 2026

[4] [Ham2K  Custom LoFi server](https://forums.ham2k.com/t/lofi-use-custom-server-in-logger/1832)  |  4 Oct 2026

[5] [Log4OM  Multi-station master log](https://forum.log4om.com/viewtopic.php?t=10235)  |  3–11 Sep 2025

[6] [Wavelog issue 1802  Full backup](https://github.com/wavelog/wavelog/issues/1802)  |  25 Mar 2025; closed

[7] [QLog issue 1029  Import upload status](https://github.com/foldynl/QLog/issues/1029)  |  5 May 2026; closed

[8] [Wavelog issue 1572  Keyboard accessibility](https://github.com/wavelog/wavelog/issues/1572)  |  28 Jan 2025

[9] [VK7BEN  Switching QSO logging to Wavelog](https://www.vk7ben.au/2025/04/09/switching-qso-logging-to-wavelog/)  |  9 Apr 2025

[10] [Reddit  New radio prompts HRD replacement](https://www.reddit.com/r/amateurradio/comments/1pf1kp0/comment/nsgly6d/)  |  5 Dec 2025

[11] [Reddit  Log4OM and QLog comparison](https://www.reddit.com/r/amateurradio/comments/1h6m8oz/best_free_logging_software/)  |  4 Dec 2024

[12] [Reddit  Building a logger and retaining Log4OM](https://www.reddit.com/r/amateurradio/comments/1hoywg0)  |  30 Dec 2024

[13] [eHam  DXLab reviews](https://www.eham.net/reviews/detail/3139)  |  Mar–May 2024 and Jan 2025

[14] [Reddit  Mac logger exploration](https://www.reddit.com/r/amateurradio/comments/1km9ipl/comment/msdc7s1/)  |  15 May 2025

[15] [K4SWL  Why I still log to paper](https://qrper.com/2026/03/pota-and-sota-why-i-still-log-to-paper/)  |  15 Mar 2026

[16] [Ham2K  Two operators and reciprocal QSOs](https://forums.ham2k.com/t/p2p-bug-or-procedural-error/1112)  |  1–2 Jul 2025

[17] [Ham2K  Log uploads with multiple parks](https://forums.ham2k.com/t/log-uploads-with-multiple-parks/1343)  |  19–21 Oct 2025

[18] [N1MM  Function keys and Enter Sends Message](https://n1mm.hamdocs.com/setup/function-keys/)  |  undated manual

[19] [W7GH  N1MM Morse operating notes](https://www.roblocher.com/technotes/n1mm-morse-notes.html)  |  undated operator notes

[20] [Log4OM  Manual bandmap workflow](https://forum.log4om.com/viewtopic.php?t=10218)  |  24–25 Aug 2025

[21] [Reddit  FT8 logging pipelines](https://www.reddit.com/r/amateurradio/comments/1qgdakg/where_are_you_guys_logging_your_ft8_qsos_do_you/)  |  18 Jan 2026

[22] [GridTracker  Settings and master logs](https://docs.gridtracker.org/latest/GridTracker-Overview/Settings-Sub-Menus.html)  |  undated documentation

[23] [Wave-Flex Integrator  Known limitations](https://github.com/tnxqso/wave-flex-integrator)  |  undated project README

[24] [SKCCLogger  Logging a QSO](https://www.ac2c.net/skcclogger/userguide/Subjects/Logging_a_QSO.htm)  |  undated manual

[25] [SOTA  Cold-weather phone logging](https://reflector.sota.org.uk/t/bad-times-on-gm-cs-094-happy-times-on-gm-cs-083/27660)  |  activation 4 Dec 2021; replies 7 Dec

[26] [MacHamRadio  Reader poll archive](https://machamradio.com/reader-poll-archive/)  |  22 Jul 2020–30 Jan 2021

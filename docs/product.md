# Product scope and operating workflows

Product and architectural planning for the agreed minimum viable product (MVP) is complete (D242). The [native-model specification](native-model.md) defines the contact and shared-data contracts. The [native-model library](../lib/clacks-model/README.md) implements their pure value and selection operations; desktop/server applications and persistence remain downstream capabilities.

Clacks Log should replace the everyday shack logger while supporting independently writable offline logs on any number of clients. Fixed and portable operation use the same product model.

## Product direction

- Native, graphical desktop operation targeting Windows, macOS, and Linux (D83). D162 selects Windows desktop support for MVP, with macOS/Linux releases post-MVP and their exact timing open. Architecture, tooling, and platform abstractions remain cross-platform from the start.
- Rust wherever practical. D213 selects PostgreSQL for the server and embedded SQLite for clients. D215 selects egui for the desktop interface, with eframe and egui_extras as the starting integration direction. Wire protocol, cryptographic libraries, exact dependency versions and physical schemas remain open.
- Dense, useful operating context and efficient use of desktop space. Avoid interfaces that resemble a simplified mobile application inside a desktop window.
- Integrate with specialist applications rather than becoming an all-in-one operating suite.
- Self-hostable synchronization and integration services, with an optional hosted subscription offering. D241 permits hosted-subscription launch after MVP; the first release does not require a commercial hosted-service launch. D63 requires multiple independent accounts per server in minimum viable product (MVP), with account-isolated log data, profiles, clients, and service credentials.
- D221 selects standard Elastic License 2.0 for Clacks’s own application code, without a free/community-hosting exception, following D219/D220. Third-party code and assets retain their applicable licenses and notices. Optional hosted-subscription packaging remains open.
- D222 requires the web architecture to accommodate eventual logbook browsing/editing and reporting, with logging a possibility. MVP administration/setup/diagnostics scope is not the long-term interface boundary; these future capabilities are not added to MVP. D223 selects Leptos/Axum with server-rendered initial pages and Rust/WebAssembly browser interactivity through hydration, within the one Clacks server application.
- Working name: **Clacks Log**, a Discworld reference. The name is provisional.

## Desktop workspace startup (D270)

Ordinary startup automatically creates a default local workspace on first use and
reopens it on subsequent launches, using the platform's per-user application-data
directory. Provide a path override for development and troubleshooting. Offline
startup requires neither a server connection nor enrollment. The local-only
account identity is not a fabricated server account or synchronization context.
An existing workspace that fails to open must not be silently replaced or
recreated. Platform directory discovery and application startup compose the
existing client store; they do not introduce a second persistence mechanism.

## Desktop asynchronous runtime (D271)

The desktop application owns one multi-thread Tokio runtime alongside egui's
main-thread event loop. Asynchronous storage operations and later background
work run on that runtime. The UI receives results without blocking its event
loop, using standard channel facilities and egui repaint requests. UI state
remains owned by the UI thread. Store operations use the existing SQLx pool and
transaction contracts; this does not introduce a custom database write queue.
Runtime ownership and shutdown belong to application composition in the library,
with a thin executable entry point. Concrete task/result contracts and runtime
tuning remain engineering work for issue #46. D272 defines normal shutdown
behavior.

## Desktop shutdown (D272)

Normal closing stops admission of new work, requests cancellation of background
operations at their supported safe boundaries and allows already-started local
save transactions to finish. Close the store, flush telemetry and shut down the
application-owned runtime. Keep the window responsive during shutdown. If exit
is delayed, offer the operator the choice to continue waiting or explicitly
force quit; do not automatically force exit after a deadline.

Saved pending changes remain durable without waiting for synchronization to
complete. Unfinished QSO drafts remain nonpersistent under D113. D82 continues
to require closing to stop the client, with no tray/background-running behavior.
A forced exit does not guarantee completion of unfinished work; existing atomic
transaction and restart-recovery guarantees still apply. Exact delayed-shutdown
threshold and coordination mechanisms remain engineering work for issue #46.

## Contacts, logbooks and tags

The account owns each radio contact (QSO). A Clacks logbook is a tag-defined view of those contacts, not a container that owns separate copies. A QSO can have zero, one or multiple tags; its complete tag set is part of its logical record (D243). Each tag has a stable ID, name and service settings. Names may be duplicated, with enough interface context to distinguish tags; matching names never merge identities (D119/D120). The physical database representation remains engineering work.

Opening a logbook selects its tag as the default for newly logged/captured QSOs and filters normal history and Worked Before. Selection is client-local and occurs before entry. Switching views does not change operating defaults, existing contacts, server jobs or synchronization participation. Profiles, reusable equipment and directory credentials remain account-shared. Every enrolled client synchronizes all account contacts, tags and shared synchronized data through one account scope (D248); selective synchronization is post-MVP.

For example, QSO Q42 can have both Main and POTA 2026-10-17 tags. Both views show the same contact, directory snapshot, confirmation evidence and publication history. Editing through either view changes Q42. A tag name is organizational metadata, not a recorded Parks on the Air (POTA) reference or an operating-location profile.

### Membership and tag lifecycle

Add or remove existing tags on individual QSOs or selected groups in MVP (D249). These operations preserve QSO identity, facts and service evidence. Removing the last tag leaves an active, untagged contact that must remain accessible; the exact layout for accessing it is not selected. To organize activity contacts into Main, add Main to the existing contacts, then remove the activity membership or delete the activity tag as desired. Export/re-import is not required to apply another tag. Broader bulk editing of contact facts remains post-MVP under D45.

Create, name, rename and delete tags offline on enrolled clients, synchronizing them and their contacts on reconnect (D118/D121). Duplicate names are allowed. Automatic tag assignment or creation based on capture source, activity references or date remains outside MVP.

Deleting a tag prompts for one of three choices (D244):

| Choice | Effect on associated QSOs |
| --- | --- |
| Leave all QSOs | Preserve every QSO and remove only the deleted tag; formerly unique contacts can remain untagged. |
| Delete all QSOs | Mark every associated QSO deleted, including those with other tags. Show how many also have other tags before confirmation. |
| Delete only QSOs unique to this tag | Mark QSOs with no other tag deleted. Preserve shared contacts and remove only the deleted tag. |

The tag is deleted in every case. Count distinct QSO identities, not memberships. For 100 associated contacts with 30 also tagged elsewhere, the choices mark 0, 100 or 70 contacts deleted. Marking a QSO deleted excludes it from all normal contact views; it is not an active untagged contact. Explicit QSO deletion is also available independently of tag deletion. The deleted-state analogy to Trash does not select a Trash interface or change post-MVP restoration scope (D55). External copies remain untouched and deletion memory suppresses later downloaded-add offers (D54).

A pending QSO referencing a deleted tag remains local until the user explicitly removes that membership or replaces it with an existing tag (D250). Removing its last membership can leave it active and untagged. Do not silently discard the contact, recreate the tag or rewrite its memberships; unrelated synchronization continues. An existing QSO that was itself marked deleted still follows ordinary QSO deletion/edit reconciliation.

### Editing and reconciliation

The tag set participates in ordinary whole-record three-way comparison (D243). A tag change concurrent with a contact-fact change, or different concurrent tag changes, can require user resolution when both sides changed and their resulting records differ. Equal results and one-sided changes bypass that flow. Do not split synchronized membership records or automatically merge tag sets to avoid these accepted MVP conflicts; refine behavior from real use. See [synchronization](synchronization.md#accepted-conflict-semantics).

### Import and export

Choose an import tag, then match Amateur Data Interchange Format (ADIF) records against QSOs across the account, including untagged contacts (D247). Reuse a confidently matched unchanged QSO and add the chosen tag if missing. Report unchanged contact facts separately from added memberships. For differing records, preserve Update/Duplicate/Skip: Update changes the shared QSO and adds the tag; Duplicate creates a separate QSO with that tag; Skip changes neither facts nor tags. Ambiguous and possible matches retain explicit candidate selection. Validation, read-only preparation and partial-import behavior remain as specified in the [ADIF notes](../references/adif-matching.md).

Ordinary export emits one entry per selected QSO identity from a consistent local working state, including unsynchronized/unresolved values and excluding pending deletions. Overlapping tag selections do not create native copies or repeat the same QSO in ordinary output. Optional POTA-compatible expansion uses actual activity fields, never tag names. ADIF is not a lossless native backup or membership-preserving round trip; native synchronization preserves tag identity and membership. Existing destination files remain untouched on cancellation/failure (D210–D212).

### External services and identity

Each tag independently selects Enable, Forbid or Neither for Logbook of The World (LoTW) and QRZ automatic uploads, defaulting to Neither even when created offline (D245/D246). Any Forbid blocks automatic publication; otherwise at least one Enable is required. All-Neither and untagged QSOs have no tag-based automatic-upload grant. A QSO is not uploaded once per granting tag. Route by saved station callsign/date and account-managed credentials. Manual uploads retain their existing eligibility, cooldown-bypass and pause/remedy rules (D109); the tag settings govern automatic uploads. See [publication policy](integrations.md#clacks-logbooks-and-publication-scope).

Downloads search all account QSOs, including untagged contacts and those without automatic-upload eligibility. Current client view does not affect this server process. Established associations take precedence; otherwise apply a unique confident match automatically. Possible matches require explicit selection even when unique; ambiguous candidates require choosing at most one QSO. Within an account and service destination, an external record may be associated with at most one local QSO (D251). A QSO may independently associate with records in different services/destinations. Separate local duplicates can remain, but cannot inherit an occupied association or silently upload over its external record; require resolution.

Unmatched downloads require explicit add/reject, for one record or a selected group of different records (D37/D105). Addition requires selecting the tag to apply. Rejection needs no tag and leaves the external record untouched. Identical repeats of a rejected version stay suppressed; changed versions undergo normal matching/review (D218). Multiple tags on one QSO are not multiple candidates or separate external associations.

## Persistence

D213 selects PostgreSQL for server persistence and embedded SQLite for desktop-client persistence, following the [persistence comparison](../references/server-persistence.md). The client requires no external storage service. Use ordinary database-library transactions and contention handling, without a custom write queue. MySQL and MariaDB are outside the selected storage stack. Web, synchronization and background jobs are internal parts of one Clacks application owning its schema, alongside PostgreSQL. Linux-container server deployment and configurable OpenID Connect sign-in are selected (D189/D190). Concrete schemas, dependency versions, the server database binding and deployment tooling remain engineering work.

Client persistence uses a fully relational schema, representing saved native values and relationships in columns and related tables. User-facing querying should use SQL's filtering, joins, grouping and aggregation capabilities without requiring Clacks to anticipate every query through separate application-maintained projections. Ordinary database indexes support performance; they do not define which stored fields and relationships are queryable. This storage decision does not require users to write SQL or add a SQL console. Relational decomposition preserves the existing native record and synchronization boundaries, including whole-QSO tag-set reconciliation.

Use shared relational payload tables with explicit references identifying latest accepted, local working, retained original, frozen prepared and retained incomparable accepted values. These roles use the same domain table structures rather than parallel table families. Preserve each role's values independently when they differ, and retain payloads only while required by current state or comparison obligations. Storage references do not introduce synchronization identities, per-edit history or a revision graph. History queries select local working values when present, otherwise accepted values, subject to the existing deletion-visibility rules.

SQLx is selected for the client store. Use its standard asynchronous database access, connection management and transaction facilities while preserving the existing native-model and synchronization operations. Library-managed execution of blocking SQLite calls does not introduce a custom application write queue. The server binding, SQLx version and connection-pool sizing remain engineering work.

Keep fixed SQLx queries in separate `.sql` files, included with `sqlx::query_file!` or `sqlx::query_file_as!`, rather than inline SQL in Rust. Use compile-time checking, with generated offline query metadata committed to the repository so ordinary builds require no database. CI recreates a temporary SQLite database from the migrations and checks that the metadata is current. Queries whose structure varies at runtime use bound parameters and integration coverage. Compile-time query checking supplements behavioral and transaction tests; it does not replace them.

Configure client SQLite connections for write-ahead logging (WAL) with `synchronous=FULL`, using SQLite's standard automatic WAL checkpointing. Preserve the existing requirements that saved work and synchronization acknowledgments follow durable commits. WAL checkpointing maintains database files; it is distinct from Clacks's synchronization checkpoints.

Use immediate transactions (`BEGIN IMMEDIATE`) for writes, acquiring the writer reservation before reading the state used to calculate and validate changes. Use ordinary snapshot read transactions for reads. Keep write transactions short, with bounded busy-timeout handling and no network requests or user interaction inside them. Read compared state and commit its resulting transition within the same transaction, following the synchronization model's existing guard and atomicity requirements.

Embed forward-only SQLx schema migrations and apply them automatically when opening the client store, before ordinary store operations. Commit each migration together with its migration-history entry. A failed migration rolls back that migration and fails store opening; earlier completed migrations remain committed. Reject an unsupported newer schema without modifying the database. Apply corrections through additional forward migrations rather than automatic downgrades. Upgrades preserve enrollment, local data, pending edits and comparison evidence under D179.

Use one database per local account workspace. Store frequency in whole hertz, transmit power in whole milliwatts and Clacks-controlled counters as SQLite `INTEGER`, with native validation aligned to the signed 64-bit maximum. Store QRZ book and record IDs as validated decimal strings; pagination arithmetic belongs in Rust. Require transactional stale-save protection for edits and resolutions without prescribing additional local generation counters.

The [client-storage specification](client-store.md) defines relational table families, role references, transaction boundaries and interruption verification for issue #6. Initial connection tuning and concrete SQL/module details remain engineering work within that contract.

## Live contact capture

WS Suite (the WSJT-X family of weak-signal radio applications) supplies completed
contacts through its User Datagram Protocol (UDP) interface.

D79 selects ADIF type 12 as the preferred live QSO input, with structured type 5 fallback when ADIF is absent. Paired notifications create one contact, and repeated deliveries do not create additional contacts. Exact correlation/fallback timing and cross-client capture remain open.

D80 treats empty station-context fields like omitted fields for live fallback to the current client context, including operator and transmit power. Nonempty supplied values take precedence. Reception does not change current defaults. This does not change historical-import omission/explicit-clear rules under D38/D42.

D81 requires one capturing Clacks client for each WS Suite source. That client supplies missing station context from its own current defaults; other Clacks clients receive the completed QSO through synchronization. The capturing client need not be on the same computer as the source and capture must continue offline from the sync server. Source identification, setup, accidental duplicate capture, and reassignment remain open.

D82 rejects close-to-tray/background-running behavior. Closing the client stops it; a confirmation before closing is permitted. Exact confirmation and platform-specific window/application lifecycle remain to be designed, without imposing Windows conventions on macOS/Linux. Capture diagnostics (source reception, last contact, and receive/save failures) remain a proposal, not yet accepted. Automatic missed-contact catch-up remains outside MVP; manual ADIF import remains the available recovery path.

Clacks Log's agreed scheduling is in [External logbook services](integrations.md).

GridTracker's Generate Messages action reaches Clacks through WS status updates; D115 defines the resulting callsign, directory-lookup and Worked Before behavior.

## Manual logging and rig integration

D84 places computer-aided transceiver control (CAT) integration, including direct rig frequency/mode reading, post-MVP. Live inputs continue to supply their frequency/mode; manual logging would use entered values. Exact later CAT capabilities and implementation remain open.

D85 rejects separate live/historical manual-entry workflows or station-context rules triggered by QSO age. Entering contacts from paper after a five-minute pile-up is ordinary manual entry with supplied timestamps. D38 concerns historical import fallback and does not impose a corresponding manual-entry classification.

Revised manual-entry proposal, not yet accepted: one MVP entry workflow starts with current operating defaults, with editable station details, Coordinated Universal Time (UTC) start/end timestamps, frequency/mode, callsign, reports, and optional details. Adjusting QSO fields or entering an earlier timestamp does not change current operating defaults. Exact fields and validation remain open.

D86 selects the LOG4OM-style time interaction described by the user: start and end fields independently update live until the lock icon beside that field is clicked. Locking freezes that field and enables manual editing. This is an accepted Clacks behavior based on the user's description, not a separately verified claim about LOG4OM versions. The proposed callsign-entry/save timing triggers were not accepted and are withdrawn. D87 returns both fields to unlocked, live-updating operation after a successful save, preserving the timestamps on the saved QSO. A failed save must not reset the unfinished entry. Other timestamp interaction details remain open.

D88 includes callsign-directory lookup during manual entry in MVP. D89 selects QRZ and HamQTH with a user-selected provider; field application remains open. [Directory research](../references/callsign-directory.md) distinguishes provider access requirements from logbook publication.

D89 includes QRZ and HamQTH in MVP; D196 makes provider selection account-level. D197 uses only that selected provider in MVP, defers automatic fallback to post-MVP, and excludes merging results from multiple directory providers. D97 settles account-level web credential setup/distribution; D98 excludes a reusable persistent directory-result cache for future lookups; D203 separately retains/synchronizes directory-location snapshots with individual QSOs.

D204 keeps all directory information separate from recorded QSO information, including names. Retain and synchronize supported directory information in each QSO's separate snapshot with provider and lookup time under D203/D204. Use recorded QSO information when present, with directory information as the logical fallback for display and supported export/upload fields. Directory lookup never fills or replaces recorded QSO fields; this supersedes the earlier D90 fill-empty model. D201 selects location as a whole, including partially specified QSO locations, without filling gaps from directory location. D237 applies the same source-separation principle to confirmation details: retain them separately and preserve manually recorded QSO information. D238 selects logical fallback in this order: recorded QSO information, eligible LoTW information, eligible QRZ information, then the eligible directory snapshot. Select location as a whole from the first available source, including partial recorded locations; do not mix location fields across sources or change recorded QSO facts. Apply this to display and supported export/upload fields, retaining provenance and existing association/applicability requirements. Exact field coverage and representation remain engineering work. D206 replaces snapshots as a whole on successful lookup, preserves them on failure/not found, and keeps completed updates on cancellation. Exact supported directory fields and calculation details remain open. D208 has no explicit fallback-suppression control in MVP: clearing recorded information exposes eligible directory fallback. Such a control is undecided for the future and would be post-MVP if ever added.

Recorded location-specific references also establish a present recorded location for the whole-location fallback rule, as agreed during issue #1 native-model design. This includes POTA, Summits on the Air (SOTA), Islands on the Air (IOTA), and other location-specific contest/event references. Unknown geographic details remain unknown instead of being filled from a lower-priority provider location. A reference set need not resolve the actual position; simultaneous multi-park references for a station describe the same operating position. Participation identifiers that do not identify a location do not establish location presence by themselves.

D91 requires the in-progress WS Suite flow in MVP: Clacks's DX (the other station) call field fills and follows WS DX-call UDP updates, and lookup normally happens before the completed-QSO notification. Both WS-driven and locally entered changes to the Clacks call field trigger lookup after a cooldown/debounce since the last actual call change; repeated status packets carrying an unchanged call do not restart that timer. D204's source separation/fallback and existing stale-result protection apply. Completion remains the save event under D79; a slow lookup does not hold up capture. Correlate entry/lookup data with the contact actually completed, never merely with whichever call is currently displayed. D92 makes local Clacks call edits lookup-only in MVP; outbound changes to WS's DX call are post-MVP. D94 sets lookup debounce to 500 ms after the last effective DX-call change. D95 defines late-result application and D96 selects client-direct directory requests; interaction with partially entered manual QSOs remains open. The earlier completion-first lookup model is superseded.

D93 makes incoming WS DX-call values authoritative over Clacks's displayed call, including a repeated WS value after a local Clacks edit. The proposal to preserve local edits until WS's own call changes was rejected and is withdrawn. Debounce follows changes to the effective Clacks call: an authoritative update that replaces a local edit restarts it, while packets leaving that value unchanged do not. D116 makes an empty incoming WS call authoritative too, clearing Clacks's call and associated lookup/history display. Cancel the cleared entry's pending debounce/lookup and suppress stale display results; D95 late enrichment of an already saved original QSO remains separate. Do not query an empty call. Exact partially entered manual-QSO field handling and normalization remain open.

D95 requires saving/capturing immediately with available information. D203/D204 retain a late lookup result in the same saved QSO's separate directory snapshot rather than writing recorded QSO fields. Associate results with the original entry/QSO and requested callsign; do not apply them to the next contact or a deleted QSO. D209 selects directory snapshots automatically by successful lookup time and a stable deterministic tie-breaker; snapshot refresh does not invoke user conflict resolution. D206 governs whole-snapshot replacement on success and preservation on failure/not found. Exact response lifecycle remains open.

D202 normalizes directory request callsigns independently of the QSO's recorded callsign: query `OK2CQR` for `OK2CQR/P`, retaining the recorded callsign unchanged. Lookup normalization is not an edit or a provider-driven callsign replacement. D95/D116 request/contact correlation and stale-result protection still apply; exact rules for other forms remain open. D207 retains the snapshot across recorded-callsign changes only when the normalized lookup key is unchanged. A changed key immediately stops use of the old snapshot and triggers lookup of the new station. Failed/not-found or late old-key results cannot restore old-station fallback. This applies to display, export, and server uploads; D206 failure preservation only applies to a snapshot associated with the current key.

D96 requires clients to query the chosen directory directly whenever they can reach it, even if the sync server is unavailable, with directory credentials securely available on each client. Server-owned upload/confirmation credentials and LoTW signing remain under their existing model. D97 selects account-level web setup and secure distribution to enrolled clients; secure-store implementation remains open.

D97 requires configuring directory credentials once per account in the web UI and securely provisioning them to that account's enrolled clients for protected local storage. Clients use their last received credentials while the sync server is unavailable; updated credentials arrive on reconnect. Account isolation applies to provisioning. Exact storage, update/deletion, and client-revocation consequences remain to be designed.

D98 excludes a reusable persistent directory-result cache for future lookups from MVP, without committing to future caching. D203 accepts separate contact-associated directory-location snapshots, including provider and lookup time, synchronized with each QSO. MVP includes bulk directory lookup/population for QSOs captured without provider access. This is a specific enrichment operation; unrestricted bulk editing remains post-MVP under D45. Authentication session reuse and in-progress lookup results are distinct from the rejected persistent callsign-result cache.

D99 selects user-triggered bulk directory lookup on selected/filtered QSOs using the account-selected provider, with partial-failure reporting and D209 automatic snapshot arbitration. D204 makes this an operation on separate directory snapshots, not recorded QSO fields. Recorded QSO information retains precedence in logical fallback. No automatic reconnect-driven bulk population is selected. D205 applies results to the scoped contacts’ separate directory snapshots as they arrive and provides a clear report afterward; no preview/approval stage is required. Report updated/new snapshots, unchanged results, missing directory records, failures with reasons, and unprocessed contacts. D206 replaces snapshots as a whole on successful lookup, preserves existing snapshots on failure/not found, and keeps completed updates on cancellation while reporting the remainder unprocessed. Exact supported directory fields, report presentation, and implementation mechanics remain open.

D112, as refined by D252, requires both station callsigns, UTC start date/time, either a frequency or a band, and mode to save a manually entered QSO. Auto-populate band from a supplied frequency when derivable; band is not independently required, and a frequency outside recognized band ranges does not prevent local saving. End time, reports, power, and location details may remain unset; exact frequency may be unset when a band is supplied. Missing service-required station information pauses only the affected upload under existing rules, rather than preventing local saving. D166 specifies the corresponding historical-import minimum; detailed field mappings remain integration work. D128 explicitly includes optional sent/received report text supporting phone, continuous-wave Morse code (CW), and digital modes such as FT8, mapped to ADIF RST_SENT/RST_RCVD. Reports are distinct from sent/received contest exchanges. Exact field formats and defaults remain to be designed.

D163 applies edited-input authority to manual frequency/band entry: changing only the band clears the exact frequency; changing frequency sets the band. A derived band update must not feed back and clear the frequency just entered. Band-only saving remains permitted under D112. This follows D75's consistency principles without supplying a representative/default frequency. Historical-import validation remains separate.

D164 makes usable frequency authoritative during ADIF import: derive missing band or correct a mismatched band, retaining frequency and accepting the otherwise valid record with a reported warning. Band-only records remain eligible. This differs from the D163 interactive band-change clear; other import validation remains open.

Use the complete ADIF 3.1.8 band catalog as the initial recognition baseline, with a text-preserving fallback for unfamiliar labels. Recognition changes alone affect interpretation without rewriting saved band values, creating edits or synchronization conflicts, as agreed during issue #1 native-model design.

D253 includes an optional receive band/frequency pair in MVP, alongside primary operating information containing a frequency, a band, or both under D112/D166. Both pairs use whole-hertz frequency resolution and the same frequency/band consistency rules. An absent receive pair means no override, using the primary pair for both directions. A present receive pair supplies receive information as a whole; band-only overrides do not borrow the primary frequency, even when bands match. Receive details are optional, including for otherwise valid manual and imported contacts. This supports recorded split-frequency and cross-band information without adding transceiver control to MVP.

D165 automatically skips confidently matched imported records that add no information after mapping and normalization, including service history. The import report includes a clear unchanged-record count. Differing imported values retain the existing update/duplicate/skip workflow.

D166 applies the same core minimum to new ADIF contacts: both station callsigns, valid UTC start date/time, either a usable frequency or a band, and mode information (mode or submode under D175). Auto-populate band from a supplied usable frequency when derivable; inability to classify the frequency does not prevent import and band is not independently required. Other details remain optional. Under D252, imports missing our callsign require explicit historical callsign assignment before creation; other missing optional historical own-station context remains unknown. Report and skip records still missing required information while importing other usable records. Existing-contact updates retain omitted-field semantics. D167 accepts otherwise valid contacts with warnings for invalid optional values: omit them on creation and preserve existing values on update. Warnings identify the record, field, supplied value, and reason; invalid input is not a clear. Exact field validation remains open. D169 prepares and validates imports without changing the logbook, discovering fields across records and showing unsupported fields and structural errors before application. D168 stops preparation at structural errors; preceding reliably decoded complete records remain eligible under existing rules, and the remainder is reported as unprocessed without guessing boundaries or undecodable record counts.

D170 preserves completed import changes when cancelling, stops at a safe record boundary, and reports the remainder as unprocessed. Cancelling read-only preparation/review leaves the logbook unchanged. A contact and its associated imported service evidence apply together; whole-import rollback is not required. D171 uses normal re-import for crash recovery, preserving completed changes and record atomicity. Dedicated import-session resumption is unnecessary, with no post-MVP commitment. Prior import review choices need not persist.

D172 selects built-in field mappings for MVP. Show unsupported fields before application and omit them if the user proceeds; do not retain opaque values solely for re-export. No custom mapping editor is needed for MVP. Exact built-in coverage and semantic mappings remain open. D185 makes ADIF interchange explicitly not completely lossless: deterministic import/export transforms need not be inverses. D186 includes grid and available coordinates in ADIF export, including calculated centers; derivation metadata may be lost. Other exact export coverage remains open. D187 limits MVP import/export to ADI (.adi/.adif); ADX (.adx) is post-MVP. Native text capabilities are not restricted to ADI. D188 uses best-effort transliteration with warnings for ADI text export, omits optional fields that cannot be represented meaningfully, and preserves original native text unchanged. Identity fields use their own validation; conversion is not a contact edit.

D174 interprets recognized submodes placed in imported MODE using their parent/submode relationship at the ADIF boundary. D176 explicitly leaves the native mode model independent of ADIF's field layout; no native two-column model is selected. A parent alone leaves submode unspecified. A recognized supplied SUBMODE determines the parent when both fields are present, with warnings for corrections. Normalize before matching. D175 preserves supplied submode independently of parent recognition: retain unfamiliar values with a warning and leave an absent parent unknown. Submode-only data satisfies the D166 mode-information requirement; do not drop it or reject an otherwise valid contact merely because its parent is unknown. D256 supersedes D177's concrete enum variants: store one generic mode value with parent-only or specialization-with-optional-parent structure, preserving familiar and unfamiliar labels alike. Interpret mode/submode/band labels and relationships directly from supplied catalog data. Known parent-only values remain representable. Exact storage/wire encoding, bidirectional transformation tables, and matching of incomplete classifications remain open.

D178 makes mode recognition interpretation-only: preserve underlying mode identity and supplied context across clients, unrelated edits, and upgrades. Recognition versus unfamiliar interpretation does not constitute a value change. Recognition updates alone create neither contact edits nor synchronization conflicts. Use ordinary Serde serialization/deserialization under D216, with preservation and comparison handled as routine schema work. D179 allows an incompatible schema/protocol to require an upgrade before sync resumes, preserving enrollment, local data, pending edits, and comparison evidence.

D113 rejects persistence/restoration of unfinished manual entries across closing/restarting at any stage. Do not create a draft-recovery backlog item. Saved QSOs and pending synchronization edits remain durable under existing requirements; operating-default persistence is distinct from restoring an unfinished contact.

## Logging defaults and saved location profiles

Current logging defaults belong to each client independently. Saved location profiles synchronize across clients. A shack client can keep Home defaults while another client operates from a park; changing one client's current defaults must not change another client's defaults.

Each client has one current operating context. Multiple operators each use their own client; managing multiple simultaneous operating contexts within a client is outside the design.

Profiles are optional conveniences for recurring locations. The operator can enter a one-off location without first creating a profile. Repeated locations may include multiple POTA references, such as a 2-fer.

Station identity belongs to the location profile. A callsign is required when saving a profile, as agreed during issue #1 native-model brainstorming. Operator identity is distinct and must be easy to set or clear in the current operating context. D68 keeps operator identity client-local and persistent across restarts, excluded from shared location profiles. Loading/reloading a profile leaves it unchanged; the user explicitly sets or clears it. Do not infer it from station callsign.

QSOs capture their operating context as snapshots at logging time. Editing a saved profile or current defaults does not change previously recorded contacts. Corrections to existing QSOs are explicit edits; bulk editing is the intended interface for applying the same correction to multiple records and is post-MVP (D45).

Logging does not require an operating-session start/end lifecycle. Current defaults can remain active between intermittent logging activity. Session-oriented organization must not become a prerequisite for logging.

Loading a profile copies its values into that client's current defaults. Later profile edits, including synchronized edits, do not refresh those defaults automatically; the user must explicitly reload the profile. The interface must clearly indicate when current defaults differ from the associated profile, whether because of local adjustments or a newer saved profile revision. The exact indication and reload interaction remain to be designed.

Compare current defaults against the latest saved profile values. Retain the last-loaded profile revision and its values as the comparison baseline: defaults versus that baseline identify local adjustments, while the latest saved profile versus that baseline identifies changes to the profile. Both kinds of change can exist simultaneously. When offline, latest means the latest saved profile revision available to that client; unseen server updates cannot be reflected until synchronization.

The operating-context editor should not permanently occupy desktop space. Profile synchronization and edited state must remain visible even when the editor is closed. The OrcaSlicer saved-preset versus current-values model is a useful interaction reference, but exact layout and docking are deferred.

D70 keeps transmit power separate from saved location profiles. D71 makes equipment configurations independent and reusable, with optional default references from location profiles. A station can have multiple equipment configurations.

Loading or explicitly reloading a profile replaces the current own-station snapshot as a whole, including callsign, recorded location and reference set. Values absent from the profile are cleared rather than inherited from the previous station snapshot. Later synchronization of the saved profile does not automatically reload current defaults. A profile may reference a default equipment configuration; the operator can select another configuration independently in current operating defaults. Loading a configuration copies its equipment details into current defaults, including when loaded through a profile. Loading a profile with no default equipment configuration leaves current equipment unchanged; it requests no equipment selection. Operator information and transmit power remain unchanged when loading a profile. Configuration edits do not automatically change active defaults or historical QSO snapshots. D72 defines the MVP configuration fields, and D73 defines synchronization and referenced-configuration deletion behavior.

D72 accepts MVP configurations with a name, equipment description (including the rig and optional other equipment), antenna description, and optional notes. QSOs copy the equipment description, antenna description and notes; preserving the configuration's display name in historical QSO snapshots is not needed for MVP, as agreed during issue #1 native-model design. Transmit power remains a separate operating value. The earlier dedicated amplifier-field suggestion is withdrawn: inspected logger documentation and ADIF do not establish it as a common logging field. See [equipment research](../references/equipment-modeling.md). Separate catalogs of individual equipment items remain outside the accepted model.

D73 requires configurations to synchronize within the account using the shared record-level conflict and field-level resolution workflow. When deleting a referenced configuration, show affected profiles and require explicit reassignment or removal of their optional default references. Deletion does not erase copied current operating values or historical QSO snapshots. D183 keeps a profile edit targeting deleted equipment pending with an explicit choice of existing configuration or removal of the optional default. If a reference is accepted before deletion, refresh and handle affected profiles under D73. Post-MVP resolution can additionally offer restoration or cloning to a new configuration. Exact future identity/retention and transaction details remain open.

D74 accepts optional explicitly supplied latitude/longitude alongside the Maidenhead grid in MVP profiles and current operating context. This permits recording a specific operating position without making coordinates mandatory.

D75 selects bidirectional location calculation in profiles/current context: editing the grid sets latitude/longitude to that grid cell's center, and editing latitude/longitude calculates the grid. The edited representation determines the other; there is no separate mismatch-resolution prompt for these edits. Conversion does not feed back and reduce explicitly entered coordinates to the resulting grid center. The earlier proposal to leave coordinates unset after grid entry and prompt on mismatches was not accepted and is withdrawn. D173 selects imported location consistency: complete valid coordinates determine an inconsistent grid, preserving supplied valid grid precision and warning about the correction. Preserve exact coordinates when consistent; grid-only input can derive center coordinates and coordinates-only input derives the selected/default grid precision. Exact malformed-value and update-grouping mechanics remain open.

D76 retains the entered/selected grid length when calculating a grid from coordinates, uses six characters initially when no precision has been selected, and allows another supported precision to be selected. Issue #1 accepts locator lengths 2, 4, 6, 8, 10 and 12; retain imported two-character areas while requiring at least four characters for own-station operating-location entry. D184 preserves area-versus-point meaning. The native-model specification consolidates the accepted GridArea/ExplicitPoint representation and coordinates in signed integer millionths of a degree.

D77 requires MVP automatic population of station DX Century Club (DXCC) entity, state/province, county, and CQ magazine zones and International Telecommunication Union (ITU) zones from operating location, with manual corrections available. D239 selects Census for US states/counties, HA8TKS for CQ/ITU zones, and Natural Earth with an explicit DXCC mapping; D133 sets coverage priorities. Dataset compilation and verification remain delivery work; see [location lookup research](../references/location-lookup.md).

D78 keeps manual corrections while the operating position remains unchanged; changing position populates fresh location-derived values so corrections from the previous position do not carry over. A grid-display-precision-only change that preserves the explicit position does not reset corrections. Profile loading copies the saved station snapshot as a whole, including absent values and saved geographic corrections; it does not preserve values from the previous location.

D132 requires station-location lookup to run offline using installed geographic reference data, with manual corrections available. D133 requires United States (US) coverage and desires worldwide coverage, with DXCC, CQ, and ITU zones as the minimum worldwide scope and full worldwide fidelity unnecessary for MVP. D239 selects datasets; dataset implementation and validation remain delivery work. D134 separately defines update scope. D184 uses the grid area for grid-derived coordinates and the point for explicit coordinates. Populate unambiguous fields independently and allow manual selection for ambiguous fields. The calculated center does not supply exact-position precision. D260 makes a point on a shared boundary ambiguous; grid areas require overlap containing area, ignoring edge/corner-only touches and never selecting by largest overlap. Uncovered portions prevent confident autofill. D112 permits local saving with location details unset.

D69 applies the established record-level conflict and shared field-level resolution workflow to concurrent edits to a saved profile. Rejected changes stay pending on the editing client; current client defaults and logging continue unchanged. Resolving saved-profile data does not reload defaults automatically. Other records continue synchronizing.

D134 requires geographic data to be independently versioned/replaceable and updatable separately from the application, while also bundling it with application releases. The separate update mechanism is not required in MVP. Data updates preserve existing QSO snapshots and manual corrections under existing rules. D257 selects Rust ingestion, `geo` and indexed FlatGeobuf layers in compatible versioned bundles. D258 selects `proj4rs` for identified transformations; D259 permits a documented no-shift approximation in remaining areas with unestablished accuracy. D261 permits reviewed, disclosed omissions but stops publication on unexpected processing/validation failures, preserving the existing artifact. D262/D263 select compiled mapping exceptions and reviewed complete replacement geometries. D264 defines #5's representative acceptance set; #23 extends coverage. D265 selects the production library/executable split. D266/D267 select per-field candidates/coverage with separate source identities, labels and recordable values. D268 separates bundle-schema version from data-release identity. The [geographic-pipeline specification](geographic-pipeline.md) consolidates these requirements and proposes concrete engineering contracts for review; [location research](../references/location-lookup.md) retains source evidence. Dataset compilation and validation remain delivery work.

## Contact-history search

D114 requires MVP filters in the current logbook by worked callsign, UTC date/time range, band/mode, own-station callsign, grid/POTA references, and per-service upload/confirmation status. Filtered/selected results support the already accepted export and bulk-directory actions. Exact matching semantics, field availability, presentation, and sorting remain open; this does not add award tracking.


## Saved contact values in history (D180)

History and Worked Before use this client's saved working values, including newly saved offline contacts and saved edits pending acceptance or conflict resolution. Show pending/conflict status and one entry per QSO identity. Do not show the accepted and proposed versions as two contacts; the accepted shared values remain available through conflict resolution. Separate copied/duplicate contact identities remain separate records. Unresolved proposals remain local, so other clients show their own working values or accepted shared state. Unsaved manual drafts are not saved contacts. D181 hides pending deletions from normal history and Worked Before immediately, including offline, while preserving their evidence and access through pending/conflict review. Existing deletion-versus-edit rules apply; a detected conflict does not automatically return the contact to normal views. Accepted-deletion restoration remains post-MVP under D55.

## Pending and conflict review (D182)

Provide account-wide review of this client's pending and conflicted work, with tag and record-type filters. Include hidden pending contact deletions, QSOs referencing deleted tags, and pending/conflicted shared profiles and equipment configurations. Untagged QSOs and records without tag membership remain accessible through the account-wide review. Do not require selecting a deleted tag to find its pending references. Review scope does not change current-logbook history/Worked Before or publish unresolved local proposals to other clients. Exact layout remains deferred.

## Worked Before

- A today indicator uses the **UTC calendar day**, not a rolling 24-hour window.
- Prior contact information must not be restricted to today.
- Show available grid and POTA reference information so multiple park activations by the same operator remain distinguishable.
- D100 scopes Worked Before to the current Clacks logbook, showing all matching contacts across saved own-station callsigns. The account may have multiple logbooks under D101. D115 makes Worked Before follow the effective Clacks callsign using local contact history independently of directory lookup, including GridTracker Generate Messages via WS DX-call status; D180 selects local saved working values for pending/conflicting contacts, with status indicators and one entry per QSO identity. D181 hides pending deletions from normal views while preserving access through pending/conflict review. Exact filter presentation remains open.
- D117 makes Worked Before a prefix search using the text in Clacks's call field against recorded worked callsigns in the current book. W7HEX can match W7HEX/P; W7HEX/P does not match plain W7HEX. The base/portable-variant expansion proposal was replaced. Empty input clears the view under D116; do not interpret it as a request for every contact. Retain recorded callsigns and grid/POTA details. Case/format normalization remains open. This does not change canonical QSO identity or import/service matching rules.
- Selection-driven access from the existing operating workflow matters. D115 identifies GridTracker Generate Messages → WS DX-call status → Clacks as the selection path. Actual forwarding/source setup and exact UI layout remain to be specified.

D127 makes POTA references and contest identifiers share a typed activity/reference context pattern, carried into QSO snapshots, with field-specific ADIF mapping and correct own-station/contacted-station reference sets on each QSO. A contest identifier alone does not imply a separate contest subsystem, scoring, or contest-management workflow. D129 places dedicated sent/received contest-exchange recording post-MVP, with general QSO comments available for free-text exchange information. D128 optional signal reports remain in MVP. D254 and issue #1 refinements define data-driven peer groups, two station-side unordered reference sets, and group-ID-plus-optional-identifier associations. Exact historical import mappings remain interchange work.

D254 makes reference groups and their behavioral properties data-driven. Each contest/event is its own group, a peer of POTA. A station-side association contains a group ID and optional identifier; groups define identifier requiredness, normalization, syntax validation and location context. Sets are unordered and deduplicated. Preserve unrecognized associations and do not rewrite saved values or create synchronization conflicts when definitions or recognition change. Contest exchanges and serial assistance remain separate and post-MVP under D129.

Seed default definitions into each account as ordinary shared records, synchronize them to enrolled clients and use the locally synchronized definitions offline. Default and future user-defined groups have full capability parity through the same generic engine; program names do not select privileged behavior. MVP does not include group management.

Optional reference catalogs supply descriptive lookup data separately from recorded QSO facts. Catalog locations do not assert the station position. Provenance records dataset name, optional publisher/source URL, obtained-at time and optional upstream version; the stable catalog ID identifies the dataset, without a separate local version scheme. Catalog acquisition/update/distribution remain separate specification work. The [native-model specification](native-model.md) consolidates the accepted capabilities and defines the group schemas and declarative mapping rules.

D130 allows supported activity/reference values in current operating defaults to be optionally saved in a location profile, including contest identifiers as well as own POTA references. Loading the profile copies these values under existing profile rules; new QSOs snapshot them. Keep setting/clearing references explicit and do not infer an operating-session lifecycle from an identifier. Profiles load the whole station snapshot, including clearing absent location/reference values; the native-model specification consolidates this behavior. D131 makes manual entry of known identifiers sufficient for MVP; manual identifier entry remains sufficient; optional catalog bindings are accepted under D254, with acquisition/update/distribution specified separately. D254 makes normalization and syntax validation group-defined and accepts optional catalogs without making catalog metadata recorded QSO data. Geographic station-location autofill under D77 is a separate requirement.

D126 places manually recorded paper-contact confirmation (QSL) sent/received status and dates post-MVP. Card-management scope and exact historical-import field retention remain open.

## MVP boundaries

Established MVP scope includes independent offline QSO use, reconciliation, WS Suite logging integration, and LoTW and QRZ publication/confirmation workflows. The Worked Before workflow is a central operating requirement; its detailed first-release interaction design is not complete.

Certificate import is in MVP; certificate requests and renewals are later scope. D53 supports multiple QRZ logbooks and LoTW certificates, routing each QSO by its saved station callsign and date; missing/ambiguous routing pauses only affected uploads. A server connection is required for certificate import.

Automatic catch-up of contacts missed while Clacks Log was not receiving WS UDP messages is outside MVP. ADIF file import is part of MVP. D38 requires historical imports to use supplied station context without filling gaps from current client defaults. D252 requires explicit historical callsign assignment before creating a contact when our callsign is missing; other missing optional our-station (DE) fields remain unknown. This also applies to downloaded QSOs explicitly added locally. D39 gives matched ADIF records with differing values explicit update, duplicate (create a separate QSO), or skip choices. D40 uses the same field-level conflict-resolution interface as other conflicting updates, letting the user choose which differing fields to apply. D41 compares historical station/worked callsigns, band, normalized mode/submode, and UTC start time at shared supplied precision, with explicit selection among multiple candidates. D42 leaves existing values untouched for omitted fields and offers explicitly empty values as selectable clears in the shared field-level comparison, subject to field-specific ADIF default interpretation and validation. D43 allows partial imports with a clear report of records not imported and the reason for each. Exact validation and detailed comparison semantics remain open. Excluding automatic catch-up does not change the requirement for durable client-to-server synchronization of contacts already captured by Clacks Log.

D44 includes ADIF export of the whole log or selected contacts in MVP. Export current supported QSO values, including historical station context and supported confirmation information; exact service-history fields still depend on the detailed external-service model.

eQSL is relevant to the existing workflow but has not been committed to MVP. Bulk editing of recorded contact facts, explicit restoration of deleted QSOs, and built-in backup/restore commands are post-MVP; direct tag assignment remains MVP. D57 accepts documented backup and recovery procedures for MVP. D62 includes individual-client revocation without deleting shared QSOs or affecting other clients. D191 selects automatic isolated-account provisioning on first successful OIDC sign-in, reusing existing issuer/subject bindings without administrator pre-creation or a separate approval step. D64’s earlier sufficient/can-be statement was permissive, not a required workflow. D65 requires browser-based client authorization and a deployment-provided web UI. D213/D223 place web, synchronization and background workers in one Clacks server application alongside PostgreSQL; exact callback/protocol integration and deployment tooling remain engineering work. D66 includes sign-in, client authorization/listing/revocation, administrator account creation, external-service configuration, and upload diagnostics in the MVP web UI. Under D67, desktop external-service setup opens that web UI; station defaults/profiles remain native, alongside live service status and routine operational actions. D56 excludes direct enrollment-time merging of independently populated logs; MVP migration exports existing contacts to ADIF and re-imports them after joining synchronization with an empty local log. Detailed ADIF import behavior, remaining migration requirements and detailed export behavior, backup/export user experience (UX), and other ordinary logger functions still need explicit scoping rather than assuming every incumbent capability belongs in the first release. D192 makes manual out-of-band server-administrator assignment sufficient; an in-app role-assignment interface is not required for MVP.

## Later interests

Heard-station views and award tracking/hunting are explicitly **not MVP**. Future interests include wanted-station handling similar in clarity to GridTracker 2, American Radio Relay League (ARRL) award opportunities, and Japan Amateur Radio League (JARL) district/prefix tracking. Do not assume GridTracker 2 can ingest external award-interest information; that capability remains to be verified.

## Research use

The attached qualitative research brief supports product discovery. Its examples are not a representative ranking of demand or a product specification. Preserve the distinctions between actual user workflows, other operators' accounts, documented software behavior, and proposed Clacks Log features.

## Directory snapshot synchronization (D209)

Select the winning complete directory snapshot independently for each QSO and normalized lookup key, using successful lookup time recorded by the fetching client and a stable deterministic tie-breaker for equal timestamps. No user resolution is required for competing directory refreshes. Arrival order, retransmission, and server receipt time do not change lookup ordering. Failures/not-found preserve the winning successful snapshot under D206. Recorded QSO facts retain existing whole-record conflict rules; a snapshot for an old key cannot displace the current key's snapshot under D207. Clock accuracy affects freshness selection rather than convergence. Exact timestamp/tie-breaker encoding and physical storage remain open.

## Export of current client state (D210)

ADIF export uses the client's current saved local working information, including unsynchronized contacts/edits and unresolved changes. Export one local working version per QSO, consistent with history, rather than emitting alternative conflict versions as duplicate contacts. Locally pending deletions remain excluded under D181. Use local eligible directory snapshots for fallback under D204, preserving whole-location selection and normalized-key association under D201/D207. Export works without successful synchronization and does not resolve pending work. D185 still makes ADIF interchange non-lossless, not a native backup of conflict/comparison metadata. D211 captures scoped membership, QSO values, and eligible snapshots from consistent point-in-time local state while logging/sync may continue. Cancellation/failure leaves any existing destination untouched; publish the completed file only on success and report exported contacts/conversion warnings. Exact storage/output mechanisms remain open.

D127’s shared activity/reference model remains separate from output expansion. D212 selects ordinary ADIF export as the MVP default, with optional POTA-compatible export that groups files by our park/location and expands entries by applicable contacted-station park references. Output expansion does not duplicate native QSOs, alter callsigns/timestamps, multiply unrelated contest/event references, or imply automatic POTA submission. D210 prevents exporting competing unresolved versions as duplicate contacts and does not prohibit a separately chosen reference-expansion transform. See the activity/reference export findings in references/adif-matching.md.

D255 makes export context an output profile plus an optional target group, independent of the selected contacts. Ordinary ADIF without a target uses ordinary interchange mappings and emits a QSO's single distinct applicable contest identifier as CONTEST_ID. Multiple distinct contest identifiers on one QSO are surfaced during preparation: select a contest context or proceed with those ambiguous fields omitted and reported. Ordinary export retains one entry per QSO without program-specific expansion. With an explicit contest target, that contest determines CONTEST_ID; other contest associations do not compete. Program-compatible profiles select the target group's submission mappings, validation, partitioning and expansion. Conflicting assignments within a selected definition are configuration errors reported before output is written. Do not add participation, rewrite associations or silently change contact selection.

For D212, report source contact count separately from generated output entry/file counts. Apply D210/D211 consistent point-in-time local state, eligible fallback, and preservation of existing destinations on cancellation/failure. Exact export-field validation, file naming/grouping details, and multi-file finalization mechanics remain open.

## Deferred and uncommitted scope boundaries


The following are deferred or uncommitted, not missing MVP decisions: bulk editing of contact facts (direct tag assignment is MVP); computer-aided transceiver control (CAT); paper contact-confirmation (QSL) workflows; dedicated contest exchanges; individual-contact restoration and equipment restore/clone resolution options; selective sync; ADX; a separate geographic-data updater; built-in backup/restore commands; outbound WS control; device-authorization fallback; automatic directory-provider fallback; and macOS/Linux releases. Preserve each decision's exact scope rather than treating every item as a guaranteed later release.

Other eQSL, award/heard-station, GridTracker wanted/award-data, archive and persistent reusable directory-cache capabilities remain uncommitted. Provider-result merging is excluded; unsaved manual-draft restoration and dedicated import-session resumption are not future backlog commitments. Use [decisions](decisions.md) for the exact boundaries, rather than promoting this parking list into requirements.

Hosted subscription launch, pricing, packaging, billing and hosted operational
commitments are future work (D241); they do not gate the self-hosted MVP.
Offline conflict resolution from cached shared values is permitted but is not a
hard MVP requirement or a committed post-MVP deliverable (D139).

## Workflow integration requirements

Import adapters preserve field-specific defaults, historical callsign assignment,
omitted-versus-cleared values, normalization warnings and invalid-optional-value
diagnostics. Candidate comparison and stale-result revalidation use the shared
resolution workflow. Import-side `CONTEST_ID` station assignment remains an import
mapping decision, without adding another native reference scope.

Export adapters define complete supported-field mappings, transliteration
warnings, activation-location partitions and source/output counts, with consistent
reads and cancellation-safe multi-file finalization. Catalog acquisition,
refresh and distribution are separate from catalog interpretation; provenance
uses obtained-at time and optional upstream version, without a local version scheme.

Geographic artifacts retain source pins, redistribution notices, datum and
date-line handling, explicit DXCC mapping/exceptions and review of the six ITU
zone topology corrections identified by the [dataset audit](../references/geographic-data-audit.json).
Required U.S. coverage and desired worldwide coverage retain their different
priorities; a selected source is not a verified shipping dataset.

Desktop verification covers large logs, keyboard/input methods, accessibility,
display scaling, live updates, partial failures/cancellation and packaged Windows
operation. Cross-platform foundations do not expand MVP release platforms.

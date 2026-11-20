# Decision register

This register records explicit decisions from the planning conversation. Unselected implementation details and historical alternatives remain separate from commitments. Current behavior is described directly in the product and architecture documents. The [desktop specification](desktop.md) consolidates D270–D279, distinguishing agreed choices from engineering defaults. D243–D251 replace child-logbook ownership, per-logbook synchronization scopes, boolean publication settings and multiple external-record associations; earlier entries are historical where superseded.

## Accepted decisions

Decisions retain their chronological IDs. Each entry has a stable heading for linking; later decisions may refine or supersede earlier entries.

### Decisions D01–D25

#### D01

Working project name is Clacks Log.

#### D02

Support an arbitrary number of independently writable clients, including offline operation.

#### D03

Client reconciliation uses a self-hostable synchronization server. An optional hosted subscription service is part of the product direction.

#### D04

Clients detect conflicts at the record level. D137 uses whole-record three-way comparison to decide whether user resolution is needed: one-sided record changes or equal resulting records need no conflict flow; differing records changed on both sides require field-level resolution, even for disjoint edits. Keep unresolved updates pending on the originating client; field choices are made there.

#### D05

Treat deletion as an edit for conflict detection and resolution.

#### D06

Preserve specialist application workflows and substantial operating context. Avoid an all-in-one shack application and a simplified web-app-in-a-desktop-window interface.

#### D07

Prefer Rust wherever practical.

#### D08

Logbook of The World (LoTW) and QRZ are minimum viable product (MVP) integrations; heard-station views and award tracking/hunting are later scope.

#### D09

Both services publish periodically after a cooldown. Working defaults recorded in discussion are a one-minute upload check and five minutes since the last edit; upload interval is configurable. An edit resets the cooldown.

#### D10

Upload pending now bypasses the cooldown. It still operates on accepted server state.

#### D11

Downloads default to a configurable 30-minute interval and support a manual trigger.

#### D12

The synchronization server owns external service upload/download execution.

#### D13

LoTW behavior will be ported to Rust using tqsllib source as the reference. Preserve protocol compatibility, not its filesystem/global-state/graphical user interface (GUI) assumptions.

#### D14

MVP supports importing existing certificate/private-key exports. Certificate requests and renewals are deferred.

#### D15

Any connected client can initiate certificate import. Offline certificate import is unnecessary.

#### D16

Protect stored private keys with server-managed secrets. This does not select a particular secret store, permanent file layout, hosting provider, or key-access boundary.

#### D17

Keep expired and superseded certificates visible with status, while excluding them from signing.

#### D18

Worked Before uses the Coordinated Universal Time (UTC) calendar day for today, retains earlier history, and shows available grid/Parks on the Air (POTA) information.

#### D19

Each radio contact (QSO) captures its operating context at logging time. Profile/default changes do not rewrite prior QSOs; corrections use explicit edits. Bulk editing for repeated corrections is post-MVP (D45).

#### D20

Saved location profiles are optional and synchronize across clients. One-off locations do not require profile creation.

#### D21

Current logging defaults are independent for each client.

#### D22

Logging does not require distinct operating-session start/end times. Continuous application use with intermittent operating is a first-class workflow.

#### D23

Loading a saved profile copies values into client-local defaults. Subsequent profile edits do not change those defaults until explicitly reloaded. Indicate saved-source changes while the profile remains selected; manual station adjustments detach that selection under D285.

#### D24

While a saved profile remains selected, retain its last-loaded revision and values to detect changes in the latest saved profile available to the client. D285 supersedes retaining this association and comparison baseline across manual station adjustments: those adjustments clear the selection and baseline.

#### D25

Operating-context editing should not permanently occupy desktop space. Selected sources, saved-source changes for those selections, and unassociated operating values must remain apparent when the editor is closed. Layout and docking are deferred.

### Decisions D26–D50

#### D26

Station identity belongs to the location profile. Provide an easy way to set or clear the distinct operator identity.

#### D27

Each client has a single current operating context. Multiple operators each use their own client.

#### D28

For live incoming QSO capture, prefer supplied field values when present. Use the client's current defaults to fill missing operating context.

#### D29

Do not add notices or a reconciliation workflow for discrepancies between supplied QSO fields and client defaults for now.

#### D30

Automatic catch-up of missed WS contacts from its local Amateur Data Interchange Format (ADIF) log is outside MVP.

#### D31

ADIF file import is part of MVP. Detailed import behavior and migration requirements remain to be designed.

#### D32

Model current QSO data, per-service publication history, and remote observations separately. Preserve what was sent and what the service reported so later edits/corrections do not conflate historical confirmation with acceptance of current values. Detailed representation and import mapping remain open.

#### D33

Automatically apply downloaded details about the DX (the other station) (contacted) station to the local QSO. Do not use downloaded data to change our our station (DE) operating context, including our grid or other location details. Exact eligible fields remain open; source precedence is in D34.

#### D34

For conflicting DX-station details, prefer LoTW over QRZ by default. MVP uses this priority; user configuration of source priority is post-MVP.

#### D35

Apply DX-station source priority per field: use LoTW when it supplies a value, otherwise use QRZ when it supplies one. Preserve the existing value if neither supplies a value; omission does not clear it.

#### D36

Automatic DX-station enrichment that changes QSO values is a normal accepted server edit and advances the QSO revision. Pending client edits undergo D04/D136/D137 whole-record comparison; differing results changed on both sides require field-level resolution, including disjoint-field edits. Downloads that change no QSO values do not advance the QSO revision. Repeated observations must not reapply enrichment or undo subsequent edits/resolutions.

#### D37

Unmatched downloaded records require explicit user confirmation to add the QSO locally or reject it locally. Do not automatically create local QSOs. Local rejection does not delete the external service record. D218 later scopes rejection to the reviewed version; changed versions receive normal processing.

#### D38

Historical imports, including ADIF and downloaded QSOs explicitly added locally, use supplied historical station context. Leave missing optional DE-station fields unknown; do not fill them from current client defaults. D252 requires explicit historical callsign assignment before creating a contact when our callsign is missing. D28's default fallback remains for live QSO capture.

#### D39

When an ADIF import record confidently matches an existing QSO but has differing values, offer explicit choices to update the existing QSO, create a separate QSO (duplicate), or skip the incoming record. Candidate matching is in D41; field selection is in D40.

#### D40

Updating a matched QSO from ADIF uses the same field-level conflict-resolution interface as other conflicting updates. The user chooses which differing fields to apply.

#### D41

Identify ADIF match candidates using historical station callsign, worked callsign, band, normalized mode/submode, and UTC start date/time at the precision both records supply. Editable contact details do not prevent candidate identification. Require explicit selection among multiple candidates; retain duplicate/skip choices and do not infer unknown historical station identity from current defaults.

#### D42

During ADIF updates, omitted fields leave existing values untouched. Present, explicitly empty values are offered as selectable clears in the shared field-level comparison; they do not automatically erase existing values. Apply field-specific ADIF default interpretation and validation where applicable.

#### D43

Allow partial ADIF imports. Provide a clear report identifying which records were not imported and why. Invalid records must not silently disappear or block otherwise usable records. Exact validation and structural-error recovery remain to be designed.

#### D44

ADIF export of the whole log or selected contacts is part of MVP. Export current supported QSO values, including historical station context and supported confirmation information. Exact service-history field representation remains dependent on the detailed external-service model.

#### D45

Bulk editing is post-MVP. The MVP does not include applying the same field corrections to a selected batch of QSOs.

#### D46

Every new external submission attempt, including retries, uses the current accepted server QSO values when dispatched, not the values retained from the first attempt. Publication history records what each attempt actually sent; an in-flight response describes that attempt and cannot establish publication of a newer edit.

#### D47

Retry uploads automatically unless they were explicitly successful. Missing/unusable responses do not require a manual retry decision. Clearly non-transient failures pause upload attempts until the user remedies the problem and explicitly resumes. Retry timing remains open; D49 scopes pauses and D46 applies to every new attempt.

#### D48

Make as much upload diagnostic information available as possible, including provider responses and the context/outcome of attempts, retries, and pauses. Exact presentation and retention remain open.

#### D49

Pause only uploads affected by a non-transient failure; let unrelated uploads continue. Scope the pause to affected destination credentials, signing certificates, or individual QSO/service failures as appropriate. D47's remedy and explicit resume requirement applies to the paused uploads.

#### D50

A positive imported upload status immediately suppresses resubmission to that service; no prior service verification is required. Upload again when service-relevant fields change. Retain imported provenance without fabricating a Clacks Log attempt or exact historical submitted payload.

### Decisions D51–D75

#### D51

Missing imported upload status is pending, like explicit unsent status, independently for each service. A positive status for one service does not suppress submission to another. Normal scheduling, upload requirements, and affected-upload pause rules apply.

#### D52

Accept recognized positive imported confirmation status immediately, preserving available confirmation dates and provenance without prior service verification. Do not invent missing dates, a fresh download, or counterpart submitted values. Confirmation remains independent of our upload status.

#### D53

MVP supports multiple QRZ logbooks and LoTW certificates. Route each QSO by its saved station callsign and date, respecting service eligibility rules. Missing or ambiguous routing pauses only affected uploads; unrelated uploads continue.

#### D54

MVP QSO deletion leaves external service copies untouched. Once the server accepts deletion, prevent future uploads of the deleted QSO. Remember deleted QSOs and their external associations so subsequent downloads do not offer to add those contacts again. Exact retention, matching, and in-flight handling remain open.

#### D55

Explicit restoration of deleted QSOs is post-MVP. MVP still retains the deletion information required by D54 to suppress later downloaded-add prompts. This does not remove D05's deletion-conflict handling.

#### D56

Directly joining an independently populated local log to an existing synchronized log is outside MVP. Export existing contacts to ADIF, join synchronization with an empty local log, then re-import contacts through the accepted import workflow. No automatic populated-log merge is required.

#### D57

Documented backup and recovery procedures are sufficient for MVP. Built-in backup/restore commands are post-MVP. Procedures must cover the application state and recovery material needed by the selected storage/credential design; exact recovery mechanics remain open.

#### D58

Normal synchronization must inherently tolerate an older restored server and reconcile surviving synchronized state from clients, without requiring manual log reconciliation or ADIF re-enrollment solely because of rollback. Negotiate shared/missing state while preserving ordinary record-level conflict semantics. Git pack negotiation is an analogy, not a selected protocol or storage design.

#### D59

Use the same state/history negotiation and transfer mechanism in both synchronization directions. The commit-and-push analogy describes coalescing accumulated working changes at synchronization, not a logbook branch, QSO files, or a logbook-wide atomic commit. Do not require a retained history event for every individual edit. Local durability and accepted record-level conflict rules remain required. Exact revision representation and retained synchronization evidence remain open.

#### D60

Enrolled clients must synchronize after an indefinite offline period without exporting contacts or re-enrolling solely because of elapsed offline time. Preserve pending work and enough synchronization evidence to reconcile older clients; this does not require a history event for every edit.

#### D61

Synchronization performs batch catch-up on connection/reconnection, followed by streaming updates while connected when updates are available. Use the same negotiation/transfer mechanism in both directions. Before publishing any local changes or retrying prepared proposals, the client must durably ingest all server updates covered by completed inbound catch-up and compare pending work against that shared state. Local logging/saves continue during catch-up; after it completes, eligible changes can publish while unresolved records stay local. Later shared changes remain subject to D146 conditional acceptance. Reconnection catches up any missed state; D235 selects encrypted WebSockets, with exact handoff mechanics remaining engineering work.

#### D62

Individual-client revocation is in MVP. Revoke a client's server access without deleting shared QSOs or affecting other clients. Its offline copy is not remotely erased; explicit revocation is distinct from elapsed offline time under D60. Exact enrollment/reauthorization mechanics remain open.

#### D63

MVP supports multiple independent accounts on one server. Isolate each account's log data, profiles, enrolled clients, and external-service credentials from other accounts. Multi-tenancy is a first-release architecture requirement, not a later retrofit.

#### D64

Administrator-created accounts are sufficient for MVP; self-service registration can be post-MVP. This scopes the required account-creation feature, not a mandatory per-user approval queue or a selected OIDC provisioning/admission policy. D190 later selects OIDC web sign-in and D191 selects automatic provisioning on first successful sign-in.

#### D65

Client authentication/enrollment uses a browser-based authorization experience like Tailscale or GitHub command-line interface (CLI). The server deployment provides a web user interface (UI), which may be served by the sync process or a separate component. Authorize the client through that UI; exact OAuth/OpenID Connect (OIDC)/device-authorization protocol and identity provider remain open.

#### D66

The MVP web UI includes account sign-in, self-service identity linking/unlinking, client authorization, enrolled-client listing/revocation, external-service configuration, and upload diagnostics. D191 selects normal first-sign-in account provisioning rather than requiring administrator account creation; D357 makes administrator-role assignment an out-of-band maintenance operation. Account isolation applies throughout. D67 defines the desktop/web division of service setup and diagnostics.

#### D67

In MVP, the desktop opens the web UI for full external-service setup. Station defaults and saved location profiles remain in the native logging workflow. Live service status and routine operational actions remain available in the desktop; exact presentation and native diagnostic detail remain open.

#### D68

Operator identity is client-local, persists across restarts, and is excluded from shared saved location profiles. Loading/reloading a profile leaves it unchanged; the user explicitly sets or clears it. Station callsign remains a profile field.

#### D69

Loading or explicitly reloading a saved profile replaces the current own-station snapshot as a whole, clearing values absent from the profile. Operator information and transmit power remain unchanged; equipment is copied from the optional default configuration when present and otherwise remains unchanged. Saved-profile conflicts use the same record-level conflict and shared field-level resolution workflow as QSO conflicts. Rejected profile edits remain pending on the editing client; unrelated records continue synchronizing. Current operating defaults and logging remain unchanged until the user explicitly reloads the profile, including after conflict resolution.

#### D70

Transmit power remains separate from saved location profiles. Equipment modeling remains under discussion: the user suggests equipment is used by profiles rather than owned by them, allowing multiple configurations.

#### D71

Equipment configurations exist independently of saved location profiles and are reusable. A profile may reference a default configuration; the operator can select another independently while operating. Loading a configuration copies its equipment details into current defaults. Later configuration edits do not automatically change active defaults or historical QSO snapshots. Exact fields, synchronization, and MVP scope remain open.

#### D72

MVP equipment configurations have a name, equipment description (including the rig and optional other equipment), antenna description, and optional notes. Power remains separate. A dedicated amplifier field is not included; amplifier details can appear in the equipment description.

#### D73

Equipment configurations synchronize within the account using the shared record-level conflict and field-level resolution workflow. Before deleting a referenced configuration, show affected profiles and require explicit reassignment or removal of their optional equipment defaults. Copied current operating defaults and historical QSO snapshots remain unchanged. D183 defines concurrent/offline missing-reference handling; D287 defines deleted operating-source cleanup. The [operating-settings specification](operating-settings.md) defines local review and transaction boundaries.

#### D74

MVP saved location profiles and current operating context support optional explicitly supplied latitude/longitude alongside the Maidenhead grid. Coordinates are not mandatory. Derivation and consistency behavior remain to be decided.

#### D75

In profiles and current operating context, editing the Maidenhead grid calculates latitude/longitude at that grid cell's center; editing latitude/longitude calculates the grid. The representation being edited determines the other, keeping them consistent. Conversion must not feed back and replace explicitly entered coordinates with the resulting grid center. Supersedes the unaccepted mismatch-prompt/grid-only-unset proposal; grid precision and historical-import handling remain open.

### Decisions D76–D100

#### D76

When coordinates change, preserve the entered/selected Maidenhead grid length. Use a six-character default when no grid precision has been selected; allow another supported precision to be selected. Exact supported lengths remain to be specified.

#### D77

MVP populates station DX Century Club (DXCC) entity, state/province, county, and CQ magazine zones and International Telecommunication Union (ITU) zones from the operating location, with manual corrections available. Geographic data sources, offline coverage, uncertainty handling, and the lifetime of manual corrections remain to be specified.

#### D78

Manual station-location corrections persist while the operating position is unchanged. Changing the position populates fresh location-derived values so prior-location corrections do not carry over. Changing only grid display precision while preserving the explicit position does not reset corrections. Offline lookup, unresolved results, and exact profile-load behavior remain open.

#### D79

For MVP live capture, prefer WS ADIF type 12, use structured QSO Logged type 5 as fallback when ADIF is absent, and correlate paired notifications into one QSO. Repeated deliveries must not create another contact. Exact correlation, fallback timing, and capture across clients remain open.

#### D80

For live QSO capture, empty station-context fields are treated like omitted fields and can be filled from current client defaults, including operator and transmit power. Nonempty supplied values take precedence for the QSO; incoming records do not change current defaults. Historical-import rules under D38/D42 remain separate.

#### D81

Configure one capturing Clacks client per WS Suite (the WSJT-X family of weak-signal radio applications) source. Its current defaults fill missing live station context, and other clients receive the QSOs through synchronization. Capture does not require co-location with WS Suite or connectivity to the sync server. Source identification, setup, accidental duplicate capture, and reassignment remain open.

#### D82

No system-tray/background-running requirement. Closing the client stops it; a close confirmation is permitted. Exact confirmation and platform-specific window/application lifecycle remain to be designed. The close-to-tray proposal was rejected. Capture diagnostics remain unaccepted.

#### D83

The desktop client must be designed for eventual Windows, macOS, and Linux support. Windows is the initial deployment environment, not an exclusive platform requirement. Select tooling and platform abstractions accordingly; macOS/Linux release timing and MVP platform coverage remain open.

#### D84

computer-aided transceiver control (CAT) integration is desired but post-MVP, including direct rig frequency/mode reading for manually logged contacts. MVP retains frequency/mode supplied by live logging inputs; no CAT implementation/library or broader rig-control scope has been selected.

#### D85

Do not divide manual QSO entry into live versus historical workflows based on contact age. Delayed entry, including paper notes from a recent pile-up, uses the same workflow with now or supplied start/end timestamps. The proposal to require different station-context selection for historical manual entry was rejected; import-specific D38 rules do not establish a manual-entry age classification.

#### D86

Manual start and end time fields independently update live until the lock icon beside that field is clicked. Locking freezes that field and allows manual editing. Adopt the LOG4OM interaction as described by the user; the proposed callsign-entry/save timing triggers were not accepted and are withdrawn. Unlock/reset and lock persistence between contacts remain open.

#### D87

After saving a manually entered QSO, return both start and end time fields to unlocked, live-updating operation. The saved QSO retains the timestamps submitted; reset affects the next entry.

#### D88

Callsign-directory lookup during manual QSO entry is part of MVP. Providers, credentials, lookup triggers, field precedence, caching, and offline/error behavior remain open. Directory metadata is distinct from contact-specific confirmation observations.

#### D89

MVP supports QRZ and HamQTH callsign-directory lookup, with a user-selected provider. Later decisions settle credentials, field precedence, caching, and authentication behavior; D196 makes selection account-level and D197 defers automatic fallback to post-MVP while excluding provider-result merging.

#### D90

D204 supersedes the earlier directory-to-QSO fill-empty model: retain directory information separately, including names, and use recorded QSO information when present with directory information as fallback. Directory lookup does not write into recorded QSO fields. Preserve original-contact correlation and protection against stale responses; exact supported directory fields remain open.

#### D91

MVP directory lookup also supports the WS Suite flow before QSO completion. Clacks's DX call field follows WS Suite DX-call User Datagram Protocol (UDP) updates. Both WS-driven and locally entered changes to the Clacks DX call field trigger lookup after a cooldown/debounce since the last actual call change. Completion is not the primary lookup trigger. Use D90 field precedence and protect against stale results. Exact debounce duration, local-versus-WS editing interaction, outbound WS control, and late-result handling remain open.

#### D92

In MVP, locally editing Clacks's DX call only drives Clacks lookup; it does not change WS Suite's DX call. Outbound WS DX-call changes are post-MVP. Incoming WS-follow behavior remains under D91; exact interaction with local editing is open.

#### D93

Incoming WS Suite DX-call values are authoritative over the Clacks call field, including when WS repeats its previous value after a local Clacks edit. The proposal to preserve local edits until WS changes its own call was rejected. Lookup debounce follows actual changes to the effective Clacks call; packets that leave that effective value unchanged do not restart it. Exact empty incoming-call behavior remains open.

#### D94

Directory lookup triggers after 500 ms without a change to Clacks's effective DX call. Both local edits and authoritative incoming WS values participate; packets that leave the effective value unchanged do not restart debounce.

#### D95

Save/capture immediately when directory lookup is pending. D203/D204 retain late directory results in the original QSO’s separate synchronized directory snapshot, rather than filling recorded QSO fields. Preserve recorded information, correlate the response with the original contact/requested callsign, and do not apply it to the next contact or a deleted QSO. D209 resolves directory snapshots automatically by latest successful lookup and deterministic tie-breaker; recorded QSO facts retain ordinary conflict rules. D206 selects snapshot replacement/failure behavior, and D207 requires association with the current normalized lookup key. Exact response lifecycle remains open.

#### D96

Clients perform direct requests to the selected callsign directory, with directory credentials securely available on each client. Lookup must work whenever the client can reach the directory, even if the sync server is unavailable. Server-owned publication/download/signing remains under the existing model. Directory credential setup/distribution and secure-store implementation remain open.

#### D97

Configure directory credentials once per account in the web UI and securely distribute them to that account's enrolled clients for protected local storage. Clients retain their last received credentials when sync is unavailable and receive changes on reconnect. Account isolation applies to provisioning. Exact secure stores, update/deletion, and revocation consequences remain open.

#### D98

A reusable persistent directory-result cache for subsequent lookups is excluded from MVP; no future caching commitment is made. D203 separately accepts retained directory-location snapshots attached to individual QSOs. MVP instead includes bulk directory lookup and population for QSOs captured without provider access. This specific enrichment operation does not bring unrestricted bulk editing (D45) into MVP. Exact bulk selection, trigger, field application, and reporting remain open.

#### D99

MVP bulk directory lookup is user-triggered on selected or filtered QSOs and reports partial failures. D204 supersedes filling recorded QSO fields: results populate/update separate directory snapshots, with recorded QSO information taking precedence in logical fallback. D209 gives directory snapshots automatic latest-successful-lookup arbitration; recorded QSO information retains ordinary conflict rules. No automatic reconnect-triggered bulk population is selected. D205 selects applying bulk results as they arrive with a clear report afterward, without a preview/approval stage. D206 settles snapshot replacement/failure semantics. Exact supported directory fields and report presentation remain open.

#### D100

Worked Before searches the current Clacks logbook and shows all matching contacts within it across saved own-station callsigns, retaining D18's UTC-today and earlier grid/POTA context. Named-logbook searches do not extend to other logbooks; D302 also provides the selected All contacts and Untagged views. The [history specification](history.md) defines local matching/presentation; live GridTracker forwarding remains capture-integration work.

### Decisions D101–D125

#### D101

An account must support more than one Clacks logbook. Clacks logbooks are distinct from provider logbooks such as QRZ destinations. MVP scope, ownership of profiles/equipment/settings, per-client selection, capture routing, and synchronization boundaries remain to be specified.

#### D102

MVP supports multiple Clacks logbooks per account. Each QSO belongs to one logbook; location profiles, equipment configurations, and directory credentials are account-shared. Each client independently selects its current logbook for logging and Worked Before. Switching logbooks leaves operating defaults and existing contacts unchanged. Provider logbooks remain distinct; external enablement/download routing and logbook lifecycle remain open.

#### D103

Enable LoTW and QRZ uploads independently per Clacks logbook, using account-managed credentials and D53 station-callsign/date routing. One logbook can remain unpublished while others upload normally. This does not require one-to-one Clacks/QRZ logbook mapping. D217 later selects automatic uploads off for new logbooks; D109 defines manual-upload behavior and D110 defines account-wide download matching.

#### D104

For downloaded records, use the existing candidate-selection workflow only when the local match is genuinely ambiguous. Established associations or an otherwise unique confident match do not require a choice merely because multiple logbooks exist. Show logbook context for ambiguous candidates. Exact account-wide search scope and unmatched destination remain open.

#### D105

MVP unmatched-download review supports explicit add/reject for one record or a selected group. Adding requires choosing a destination Clacks logbook for that selection; rejecting needs no destination. Genuinely ambiguous matches still use D104 candidate selection. No automatic addition is selected.

#### D106

Moving an individual QSO between Clacks logbooks is post-MVP. The proposed MVP move operation was not accepted; its identity/history preservation and publication behavior remain future design proposals rather than accepted mechanics. D249 subsequently replaces this restriction with MVP individual/selected-group tag assignment in the tag model.

#### D107

MVP permits deleting populated Clacks logbooks without first deleting each QSO manually. The current logbook is selected before QSO entry. The empty-only deletion/mandatory archive proposal was rejected. Support the export/import-to-main-logbook then discard-source-book workflow; native QSO moves remain post-MVP under D106. Exact aggregate deletion/conflict mechanics remain open; archive scope is unselected.

#### D108

Choose the destination Clacks logbook before ADIF import and match/deduplicate within that destination. Copies in other logbooks do not prevent importing into the chosen destination. Existing update/duplicate/skip and ambiguous-candidate selection apply within it. This enables activity-log export/import into a main logbook before deleting the activity book. D247 subsequently replaces this destination-only matching scope for the tag model.

#### D109

Per-logbook LoTW/QRZ enablement controls automatic uploads. Explicit manual upload remains available when automatic upload is off, retaining accepted-server-QSO values, routing/credential requirements, and failure-pause/remedy rules, while bypassing edit cooldown under D10. Manual upload does not silently resume a paused failure.

#### D110

Download matching covers all logbooks in the account, including those with automatic uploads disabled. A downloaded confirmation may match equally against multiple local records, including intentional copies from activity-log export/import. Multiple-record association/application semantics remain open; do not assume every match is one-to-one or equate all multiplicity with D104 ambiguity. D251 subsequently supersedes the multiple-association/multiple-selection behavior: at most one local QSO per external record within an account and service destination.

#### D111

In MVP, a downloaded confirmation with multiple matching local records is treated as ambiguous, including possible copies of the same QSO. The candidate-selection workflow permits selecting more than one match. Automatic application to all inferred copies was rejected because matching rules are not yet sufficiently established. Retain explicit selections as associations; exact evidence and one-to-many association mechanics remain open. D251 subsequently supersedes the multiple-association/multiple-selection behavior: at most one local QSO per external record within an account and service destination.

#### D112

Saving a manually entered QSO requires both station callsigns, UTC start date/time, either a frequency or a band, and mode under D252. A supplied frequency auto-populates the band when it can be derived; band is not independently required. A frequency outside recognized band ranges does not prevent local saving. Reports, end time, power, and location details may remain unset; exact frequency may be unset when a band is supplied. Missing external-service-required information pauses the affected upload rather than preventing local saving. D166 supplies the historical-import minimum; detailed field mappings remain integration work.

#### D113

Do not persist/restore unfinished manual QSO entries across closing or restarting Clacks. The user does not expect this behavior at any stage; this is not a post-MVP deferral. Durable saved QSOs, pending synchronization changes, and persistent operating defaults are separate requirements.

#### D114

MVP contact-history filters cover worked callsign, UTC date/time range, band/mode, own-station callsign, grid/POTA references, and per-service upload/confirmation status within the current logbook. This set is sufficient for MVP. Selected/filtered results support existing export and bulk-directory operations. The [history specification](history.md) consolidates expression matching and presentation under D329–D351, with engineering defaults identified separately.

#### D115

Support GridTracker's Generate Messages action through WS DX-call status updates reaching Clacks: update its call field, trigger D94 debounced directory lookup, and drive Worked Before under D100 current-logbook scope. No separate direct GridTracker lookup protocol is required for this identified path; actual deployment forwarding/settings still need validation.

#### D116

Empty incoming WS DX calls are authoritative too: clear Clacks's call and associated directory/history display. Cancel pending lookup for that cleared in-progress entry and prevent stale responses from repopulating it; D95 late enrichment of an already saved original QSO remains distinct. No directory request is made for an empty call.

#### D117

Worked Before treats text in the Clacks call field as a prefix match against recorded worked callsigns in the current logbook. Do not replace this with automatic base/portable-variant expansion. For example, W7HEX can match W7HEX/P, while W7HEX/P does not match plain W7HEX. An empty call still clears the display under D116. The [history specification](history.md) defines prefix input and presentation, distinguishing engineering defaults from agreed behavior.

#### D118

MVP permits offline creation and naming of a logbook on an enrolled client. The logbook is usable for recording contacts immediately; the book and its contacts synchronize on reconnect. Naming uniqueness, offline rename/deletion, and exact record/identifier mechanics remain open.

#### D119

Duplicate logbook display names are allowed within an account. Logbooks remain distinct by internal identity, and the UI supplies enough context to distinguish them. Equal names do not merge independently created books. D306 selects short ID suffixes for duplicate names and full-ID access in Manage logbooks.

#### D120

Prefer internal unique identifiers such as Universally Unique Lexicographically Sortable Identifiers (ULIDs), as recommended by the user, for entities throughout the system; identity is independent of display names. Exact identifier format/library and revision representation were left open by this decision. The issue #1 native-model plan subsequently selected typed ULIDs with string serialization; causal revision representation remains separate. Sortable identifier timestamps do not replace existing conflict/revision evidence or establish trusted cross-client edit order.

#### D121

MVP supports logbook rename and deletion while an enrolled client is offline, reconciling those changes through the existing conflict-resolution workflow on reconnect. Exact conflicts between whole-logbook deletion and concurrent contained-QSO additions/edits remain to be designed.

#### D122

**Contained-contact conflict policy superseded by D157.** Logbook deletion is handled like any other edit for conflicts. When a pending whole-book deletion encounters conflicting contained-QSO changes, keep deletion pending until the existing workflow resolves them; unrelated sync continues. If deletion is already accepted, conflicting incoming offline changes remain pending locally. Exact aggregate revision/transaction and retained-book resolution mechanics remain open.

#### D123

A separately managed database service is acceptable for self-hosting, but is not required or selected. Requirements are not yet sufficient to choose an appropriate database, if any. Establish persisted state, query/transaction/recovery behavior, and deployment needs before comparing engines. The later comparison is in references/server-persistence.md; D213 subsequently selects PostgreSQL for the server and SQLite for the client.

#### D124

Preserve prior confirmation evidence/provenance when a QSO is edited. Current confirmed status applies only while that evidence matches the current QSO under service matching rules. A correction that makes it no longer match leaves historical evidence intact without confirming the edited contact. Exact thresholds and imported-status baseline representation remain open; D52 initial acceptance and D46 current-value uploads remain unchanged.

#### D125

A newer explicit unconfirmed service result updates current confirmation status while retaining earlier positive evidence/history. Missing fields, no records in a query window, and failed requests do not erase confirmation. Exact provider fields, observation ordering, and mapping remain open.

### Decisions D126–D150

#### D126

Manually recorded paper-contact confirmation (QSL) sent/received status and dates are post-MVP. This does not add card printing/mailing/management scope or settle retention of paper-QSL fields from historical imports.

#### D127

POTA references and contest identifiers share a typed activity/reference context pattern carried into QSO snapshots, with field-specific ADIF mapping and correct own-station/contacted-station reference sets on each QSO. Identifier recording does not imply a separate contest subsystem. D129 and D254 settle exchange separation, data-driven groups, cardinality and station-side association shape; the native-model specification consolidates later refinements.

#### D128

Optional sent/received signal reports are part of the QSO model in MVP, supporting phone, continuous-wave Morse code (CW), and digital formats such as FT8. Preserve report text rather than restricting values to one numeric report format; reports remain optional under D112. Contest exchanges are separate data; D129 places dedicated exchange recording post-MVP.

#### D129

Dedicated contest-exchange recording is post-MVP. MVP provides a general QSO comment, which can hold exchange information as free text. Do not automatically reinterpret that comment as structured exchange fields. D127 activity/reference identifiers and D128 optional signal reports remain separate. Exact historical-import retention of structured exchange fields remains open.

#### D130

Saved profiles may optionally include contest identifiers alongside POTA references, using existing profile-loading and QSO-snapshot rules. These values load into current defaults and snapshot into new QSOs without changing previous contacts. D254 and subsequent issue #1 discussion define data-driven groups, optional catalogs and validation, with profile station snapshots loaded as a whole.

#### D131

Manually entering known activity/reference identifiers is sufficient for MVP. Park-name/reference catalog lookup is not required for MVP. D254 later accepts optional catalog support in group definitions; acquisition/update/distribution workflows are separate work, not prohibited by this sufficient-manual-entry baseline. This does not change D77 station-location geographic autofill, which is a separate requirement.

#### D132

MVP station-location autofill operates offline using installed geographic reference data, with manual corrections available. Required geographic coverage, datasets/rights/versioning, boundary handling, and update/distribution mechanics remain open.

#### D133

United States (US) geographic autofill coverage is required. Worldwide coverage is desired; full worldwide fidelity is not needed for MVP, but the worldwide scope should at least include DXCC entity and CQ and ITU zones. Detailed worldwide administrative subdivisions are not a stated MVP requirement. Exact datasets and feasibility remain open.

#### D134

Geographic data must be updatable independently of application releases and also distributed with application releases. MVP does not need the separate data-update mechanism. Design versioned/replaceable data boundaries now; exact packaging, compatibility, and future update mechanism remain open. Existing QSO snapshot/manual-correction preservation remains under existing rules.

#### D135

Research distributed replication broadly before selecting synchronization representation. conflict-free replicated data types (CRDTs) and vector clocks are examples, not exhaustive scope. Learn from published approaches without necessarily adopting them; per-account replica counts are expected to be modest, but lifetime clock/actor IDs need a viable garbage collection (GC) story. No protocol or metadata scheme is selected.

#### D136

Retain the originally observed values needed for an effective three-way field comparison during sync conflict resolution. Distinguish shared-only changes, local-only changes, and changes on both sides. Original values need not be presented directly. Exact storage, field equality, layout, and resolution defaults remain open; this does not select a base-revision representation or automatic acceptance/merge policy.

#### D137

In MVP, apply the resolution-flow trigger to whole records: when only one side changed the record, use that result without a conflict flow; when both sides reach the same record, they agree without a flow; when both changed and resulting records differ, require field-level resolution, including disjoint-field edits. A broader interdependent-field conflict model is post-MVP. Exact equality, validation, and protocol remain open.

#### D138

Apply whole-record comparison to deletion: deletion versus an unchanged record proceeds without a conflict flow; two deletions agree without a flow; deletion versus a changed record requires resolution. Deleted is a comparison state, not a selected storage representation. D54 deletion memory/external-copy policy remains required; D157 supersedes D122 for contained-contact changes; tombstone retention and encoding remain open.

#### D139

Offline resolution of a known conflict from the last received shared values is acceptable, but not a hard MVP requirement. If provided, save the resolution locally and revalidate through the same whole-record comparison on reconnect; intervening shared changes may require resolution again. This does not commit the feature to post-MVP or change durable pending-work requirements.

#### D140

Include recovery-relevant external-service evidence in MVP client synchronization: stored publication outcomes and their existing sent-value/destination context, remote observations, and processing decisions such as matching, rejection, and enrichment. Surviving clients can return this evidence to an older restored server to avoid resubmitting known successful unchanged publications or reapplying processed observations. Exact representation, retention, and consistency mechanics remain open; do not fabricate unavailable outcomes or imported attempt history.

#### D141

During rollback/restore, the user pauses all uploads and explicitly resumes them after re-syncing as current as possible from available sources. Normal sync performs recovery; this is not a manual reset/re-enrollment requirement. Conflict detection and resolution are client-side. Client conflict notification must not create a server publication dependency on that client ever resolving/submitting its work. Exact pause control/scope and restore procedure remain to be designed.

#### D142

MVP per-client sync diagnostics include connection state and any current sync error. Do not show an overall last-successful-synchronization timestamp: one timestamp cannot represent the state of bidirectional synchronization, pending work and conflicts. No replacement aggregate success indicator, catch-up/currentness metric or progress requirement is selected. Prioritize working synchronization; these diagnostics support operator recovery judgment without proving that unavailable sources have been recovered.

#### D143

Do not redefine rollback to assume separately preserved authorization state. A revocation present only in rolled-back server state is lost when restoring a backup predating it; no guarantee that later revocations remain effective is selected for MVP or later. The assistant's proposed rollback-surviving revocation requirement is withdrawn. Recovery can use genuinely surviving evidence, but cannot reconstruct facts absent from every available copy or assume an independent authorization store. No additional review/reauthorization workflow is selected.

#### D144

Permit automatic fuller state reconciliation through normal sync when incremental transfer history is unavailable. Increased catch-up comparison/transfer cost is acceptable; retain enrollment and pending work. This permits a fallback contract for transfer-history compaction without selecting causal encoding, snapshot format, retention intervals, actor-ID retirement, or deletion/service-evidence GC rules.

#### D145

An explicitly reauthorized client retains its existing synchronized local data and pending offline work; revocation alone does not force contact migration or empty-log enrollment. D152 clarifies that revocation closes automatic acceptance from the old actor: recovering previously unsynchronized work requires explicit manual reconciliation, not automatic submission on reauthorization. Exact authorization/causal-identity mapping and manual recovery mechanics remain open; this is distinct from D56 enrollment of unrelated independently populated data.

#### D146

Use atomic conditional acceptance for a client submission: accept only while relevant shared state still matches what the client compared. If it changed, the client receives newer state and repeats whole-record comparison, showing resolution only under the established rules. Business conflict interpretation/resolution remain client-side. Exact condition encoding, transaction boundary, and response protocol remain open; no base-revision/clock format is selected.

#### D147

For genuine divergence between previously synchronized states during recovery, explicit two-way field resolution is acceptable when common original values cannot be recovered from available copies. Show that change attribution is unavailable; do not guess which side changed each field. Ordinary pending work still retains original values for three-way comparison under D136. This does not authorize treating unavailable causal ordering as proof of divergence or select causal/deletion GC rules.

#### D148

When causal ordering cannot be established for differing synchronized states, conservative explicit reconciliation is acceptable, even if complete metadata would have shown a one-sided change. Clearly explain that resolution is needed because ordering evidence is unavailable, rather than claiming proven concurrent edits. Do not silently choose a winner. This is distinct from D147's known-divergence/missing-original-values fallback; exact metadata retention/GC remains open.

#### D149

Use agreed properties to assess existing replication models before proposing implementation mechanisms. D150 specifies the conceptual research scope. No implementation, clock scheme, or model is selected.

#### D150

The requested research is a survey of distributed replication approaches, abstract data models, and data structures, not selection of pluggable distributed filesystems/databases/frameworks. Redo the analytical comparison at that level; verified prior primary-source findings may be reused, but software-fit conclusions do not answer the request. Systems are case-study evidence, not the comparison's organizing units. Preserve agreed properties; defer software/library selection and custom protocol mechanics until the conceptual survey is complete.

### Decisions D151–D175

#### D151

Continue synchronization design with the vector-based approach. Dynamic version vectors are the current baseline; exact vector representation, factoring, retirement encoding, and implementation remain open. This selects the causal-tracking direction, not every component of the research recommendation or a custom protocol.

#### D152

Revoking a client closes automatic acceptance of further updates from its actor. Treat the actor's admissible update history as closed without waiting for the lost/offline installation to synchronize or reveal its final local counter. If recovered, its previously unsynchronized edits require explicit manual reconciliation; reauthorization alone does not submit them automatically. Previously accepted shared records and evidence remain shared and recoverable through other authorized clients. Indefinitely offline clients that have not been revoked remain eligible for normal synchronization. This clarifies D145; exact retirement metadata/compaction and manual recovery workflow remain open. D143's rollback limits still apply.

#### D153

Eventual consistency and user-resolved conflicts remain the recovery contract. Detecting rollback and requiring explicit manual client reconciliation is acceptable; transparent atomic-register recovery is not required. Do not infer a storage-quorum requirement from stronger recovery papers. The exact detector, metadata repair, and scope of manual reconciliation remain open. The overall-server-revision comparison is a proposal for evaluation, not an accepted complete detector.

#### D154

Proceed with timeline-level ancestry for accepted shared-state ordering and recovery. Use timeline/revision references and shared ancestry/coverage boundaries rather than per-record version-history graphs or new actor identities on ordinary server starts. Record actual restored fork points and later incorporated old-history prefixes at their corresponding new-timeline positions; do not infer coverage from an advertised counter alone. Exact representation, scope, detection coverage, and collection policy remain open. D151 vector-based direction, D152 revocation cutoff, D153 manual recovery, and indefinite offline eligibility remain in force.

#### D155

Retain detailed ordering evidence from the oldest authorized client’s durably acknowledged synchronization point onward; compact earlier history into an equivalent shared baseline. Determine common coverage through timeline ancestry rather than comparing unrelated revision numbers. Unresolved local work must retain its comparison evidence, either by holding collection back or by preserving sufficient self-contained evidence. Preserve the interpretation of surviving record references. An indefinitely offline authorized client may delay collection indefinitely; revocation removes it from the participant set. Older backups below the collected baseline use surviving evidence and the accepted manual-recovery fallback. Deletion memory and external-service evidence retain their independent obligations. Exact checkpoint, baseline, and collection encoding remain open.

#### D156

Before acknowledging a synchronization checkpoint to the server, atomically save the received shared state, preserved pending-work comparison evidence, and checkpoint locally. A crash leaves either that durable checkpoint or an older checkpoint requiring safe repeat transfer. Unresolved conflicts remain pending while unrelated synchronization progresses; their required comparison evidence survives checkpoint advancement. Exact storage and dependency representation remain open.

#### D157

Deleting a logbook supersedes outstanding additions/edits to its contained contacts rather than making those changes block deletion or offering to restore the logbook. An incoming contact change targeting a deleted logbook is a missing-destination conflict: reject shared acceptance and preserve the pending work locally; do not resurrect the book or silently discard/reassign the work. Unrelated synchronization continues. Future cross-logbook contact-move support can offer an explicit choice of another destination for the edited contact; it remains post-MVP under D106. Copies in other books and external copies remain independent. This supersedes D122’s contained-contact conflict policy, not the general individual-record comparison rules. Exact deletion/admission transaction mechanics remain open. D244/D250 subsequently replace the contained-owner/missing-destination model with explicit tag-deletion effects and pending deleted-tag reference handling.

#### D158

Use per-logbook synchronization timelines, with a separate account-level synchronization scope for shared data. Maintain ancestry, client acknowledgments, and D155 retention boundaries per scope; unrelated scopes do not share a single collection boundary. D156 durable acknowledgment applies within each scope. Current display selection does not determine synchronization scope, and this does not select partial account replication or cross-scope transaction mechanics. Exact placement of logbook catalog/lifecycle metadata and service evidence remains to be designed. Account isolation and credential-distribution restrictions remain in force.

#### D159

Store the logbook catalog in the account-level synchronization scope: stable logbook identity, name, and deletion state. Contacts remain in their corresponding per-logbook scope. Clients discover created books and learn deletion through the account scope, including after a deleted book’s detailed history is collected. Validate destination existence atomically with contact acceptance under D157. Exact storage, cross-scope synchronization ordering, and transaction encoding remain open.

#### D160

Keep contact-specific external-service evidence in the contact’s logbook synchronization scope: publication attempts/outcomes with sent-value context, associated observations, and per-contact processing decisions. Keep account-wide download coordination/progress and unresolved incoming observations in the account scope, with durable rejection dispositions where applicable. An observation explicitly associated with multiple contacts may retain the same stable observation ID in each affected book, with distinct association/processing results; this does not automatically apply observations to inferred copies. Preserve required deletion/replay evidence after book deletion. D140 recovery synchronization and atomic processing-result/contact-edit requirements remain in force. Exact representation and cross-scope handoff mechanics remain open. D251 subsequently supersedes the multiple-association/multiple-selection behavior: at most one local QSO per external record within an account and service destination.

#### D161

MVP synchronizes all logbooks and account-shared synchronized data to every enrolled client. Selective synchronization is post-MVP. Current display selection does not limit replication. D158 scopes retain independent timelines, checkpoints, and collection boundaries; every enrolled authorized client participates in each applicable scope’s retention boundary. Existing restrictions on server-only credentials and client-local operating state remain unchanged.

#### D162

Windows desktop support is sufficient for MVP. macOS and Linux desktop releases are post-MVP. Keep architecture, tooling, and platform abstractions cross-platform from the start under D83; this scopes release support rather than authorizing Windows-exclusive dependencies. Exact later release dates and desktop toolkit selection remain open.

#### D163

Manual frequency/band entry follows edited-input authority without inventing an exact frequency. Changing only the band clears the exact frequency. Changing frequency sets the band, preserving the entered frequency; the resulting derived band update must not feed back and clear that frequency. D112 continues to permit band-only contacts. This applies D75’s consistency principles rather than its representative-center behavior; no representative/default frequency is selected or needed for band changes. Supersedes the assistant’s incorrect interpretation of the previous discussion.

#### D164

For ADIF import, a usable supplied frequency determines band. Derive a missing band from frequency; if the supplied band disagrees, calculate band from frequency, accept the otherwise valid record, and report a warning identifying the correction. Band-only records remain eligible for import. This normalization does not invoke D163’s interactive band-change frequency clear. Other validation, D39–D42 matching/update choices, and D43 partial-import behavior still apply; unusable frequency handling and other invalid optional fields remain to be designed.

#### D165

Automatically skip confidently matched ADIF import records that add no information or changes after mapping/normalization, and show a clear unchanged-record count in the import report. Consider mapped contact values and service history: matching contact details alone do not make newly supplied upload/confirmation status or dates redundant. Differing values retain D39–D42 update/duplicate/skip and explicit candidate-selection rules. An unchanged skip does not create a duplicate contact or a contact edit.

#### D166

Creating a new contact from ADIF requires both station callsigns, valid UTC start date/time, either a usable frequency or a band, and mode information (mode or submode, clarified by D175). Auto-populate band from supplied usable frequency when derivable under D164; band is not independently required, and inability to classify a usable frequency does not prevent import. End time, reports, power, and location details remain optional; exact frequency may be unset when a band is supplied. D252 requires explicit historical callsign assignment when our callsign is missing; other missing optional own-station context remains unknown under D38. Service-specific requirements affect upload readiness separately from local importability. Report and skip new-contact records missing required information while other usable records proceed under D43. Existing-contact updates retain D42 omitted-field behavior. Exact field-format validation and invalid optional-field handling remain open.

#### D167

Import otherwise valid contacts with warnings for invalid optional values. Omit an unusable optional value when creating a contact; when updating an existing contact, leave its existing value untouched. Warnings identify the record, field, supplied value, and reason. Do not treat invalid input as an explicit clear or silently discard it. D164 frequency/band correction remains its specific normalization rule; D166 required-field validation and D43 partial-import behavior still apply. Exact field validation and structural-file recovery remain open.

#### D168

Stop ADIF import on a structural parsing error. Import the preceding reliably decoded complete records, subject to existing validation and match/update choices, and report the remainder as unprocessed. Identify the structural error and its location; do not guess subsequent record boundaries or claim a record count for an undecodable remainder. Reliably parsed invalid field contents remain governed by D166/D167 and do not themselves constitute structural parsing errors. No heuristic recovery past damaged field lengths is required for MVP.

#### D169

Prepare and validate ADIF imports before making any logbook changes. Scan record contents to discover fields (records may have independent field sets), determine mappings, validate values, and prepare matching/update choices. Show structural errors and unsupported fields before applying the import. Under D168, stop preparation at a structural error: only the preceding reliably decoded complete records are eligible for application, and the remainder is unprocessed. Do not infer the complete field set from a header or early records. Apply the prepared input and choices rather than silently rereading changed source content; exact staging representation remains open. This does not select opaque unsupported-field storage, custom mapping tools, or persistent/resumable preparation.

#### D170

Cancelling an import preserves completed changes, stops application at a safe record boundary, and reports the remainder as unprocessed. Cancellation during read-only preparation/review leaves the logbook unchanged under D169. Apply a contact, its selected tag membership and its imported service evidence in one local transaction so cancellation does not leave a partly applied record. The resulting records synchronize through ordinary per-record acceptance. Whole-import rollback is unnecessary.

#### D171

After an import-time crash, rerunning the normal import is sufficient for MVP recovery. Completed records remain durable and interrupted records must not be partly applied. Reprepare the source and use existing unchanged-record skipping and match/update rules; prior review choices or an import-session position need not survive. Dedicated import-session resumption is unnecessary. Recovery uses the local transaction boundary in D170.

#### D172

Built-in ADIF field mappings are sufficient for MVP, including explicitly supported useful application-specific mappings. Show unsupported fields during read-only preparation before application. If the user proceeds, omit those unsupported fields; do not retain opaque values solely for re-export. No custom mapping editor is needed for MVP. Exact built-in field coverage and semantic mappings remain to be specified; this does not defer a mapping editor to a committed later release.

#### D173

Imported complete valid latitude/longitude takes precedence over an inconsistent grid square: calculate the grid from those coordinates, preserve the supplied valid grid precision, and report the correction as a warning while accepting the otherwise valid record. When both agree, preserve exact coordinates rather than replacing them with a grid center. Grid-only input can derive center coordinates; coordinates-only input derives grid using the selected/default precision under D76. Apply to supplied own-station and contacted-station location data without using current defaults to fill historical station facts. Exact supported grid lengths, malformed-value handling, and location-field update grouping remain to be specified under existing rules.

#### D174

Normalize a recognized submode supplied in ADIF MODE into its defined parent-mode/submode pair. A broad parent mode alone leaves submode unspecified; do not infer a child. When both fields are supplied, a recognized SUBMODE determines its parent MODE; correct a missing/inconsistent parent and warn about corrections. Normalize before D41 matching so equivalent supported representations can match. Exact recognized-mode tables, unrecognized-value handling, and existing-contact update grouping remain open.

#### D175

Preserve supplied submode information independently of whether its parent mode is known. An unfamiliar submode is not invalid merely because it is absent from Clacks’s recognition table; retain it as first-class contact data and warn about unrecognized values without inventing a parent. If MODE is absent and the submode is unrecognized, leave the parent unknown rather than rejecting an otherwise valid contact or dropping its submode. D166’s import minimum means mode information supplied as mode or submode, not a mandatory known parent MODE. D174 still derives/corrects parents for recognized submodes. Service-specific encoding/readiness remains separate. Exact matching of incomplete/unrecognized classifications remains open; do not collapse distinct submodes into a generic unknown identity.

### Decisions D176–D200

#### D176

ADIF interoperability does not require a one-to-one mapping between ADIF fields and Clacks’s native model. Define a single unambiguous import transform and a single unambiguous export transform for the supported semantics. Model native contact information independently of interchange-field layout. D174/D175 select normalization and preservation behavior at that boundary, not mandatory native MODE/SUBMODE columns or a fixed two-level hierarchy. Canonical export need not reproduce the source’s original field arrangement; preserve agreed information without inventing missing facts. Exact native mode representation and complete transforms, including unfamiliar values/context, remain open.

#### D177

Initially selected a single native Mode value represented by a partially hierarchical Rust enum, with typed known-family specializations and a structured OtherMode fallback. **D256 supersedes the type-encoded catalog:** retain the generic hierarchy and unfamiliar labels, but recognize concrete families/submodes from data. Known parent-only values remain representable without inventing a specialization. OtherMode distinguishes unfamiliar mode-only information from unfamiliar submode information with optional supplied parent context; preserve those values under D175. Avoid a both-absent fallback and validate nonempty labels; canonicalize recognized forms at the boundary under D174/D176. The documented variants are illustrative, not a complete catalog or selected storage/wire encoding. D216 supplies the ordinary Serde-based implementation direction; exact schema/equality details must satisfy D178.

#### D178

Differences in mode recognition between clients or application versions affect interpretation only. Preserve the underlying mode value and supplied context across synchronization, unrelated edits, and upgrades. Recognition-table changes alone do not create contact edits or sync conflicts. A client may recognize the value from its supplied catalog or preserve it as unfamiliar without changing its identity; do not serialize a changed interpretation over an untouched underlying value. D216 treats stable representation and comparison as ordinary schema work using standard serialization tooling. This does not select indefinite compatibility for every future application schema or protocol.

#### D179

Incompatible schema or synchronization-protocol changes may require a client upgrade before sync resumes. Preserve enrollment, local data, pending edits, and comparison evidence through that upgrade; do not require re-enrollment or ADIF migration merely because the client was offline or incompatible. D178 recognition differences alone remain interpretation-only and do not require an upgrade. Exact compatibility negotiation, supported-version policy, and migration/storage mechanisms remain open.

#### D180

Contact history and Worked Before show this client’s saved local working version, with pending/conflict indicators and one entry per QSO identity. Newly saved offline contacts and pending saved edits are visible immediately. Unresolved proposals stay local; other clients show their own working state or accepted shared state. The accepted shared version remains available in the resolution workflow rather than appearing as another contact. This does not merge distinct copied/duplicate QSO identities or select pending-deletion presentation. Exact indicator layout and deletion-review access remain open.

#### D181

Hide locally pending contact deletions from normal history and Worked Before immediately, including offline. Keep deletion/comparison evidence durable and keep pending or conflicted deletions accessible through pending/conflict review. Existing deletion-versus-edit resolution applies; a detected conflict does not automatically return the contact to normal views. This does not add accepted-deletion restoration under D55 or change D157’s missing-logbook behavior. Exact review presentation remains open.

#### D182

Review is account-wide and covers this client's pending/conflicted contact work,
including hidden pending deletions and deleted-tag references, plus shared-record
work such as profiles and equipment. Account-wide scope does not publish
unresolved local proposals or make them visible as a shared server conflict
queue. Normal contact history and Worked Before remain current-logbook scoped.
D383 defines the window and sections, retains record-type filtering, and removes
the previously selected logbook filter.

#### D183

For MVP, a profile edit referencing an already deleted equipment configuration remains pending with a missing-reference issue; explicitly select an existing configuration or remove the optional equipment default. If a new reference is accepted before configuration deletion, refresh the affected-profile set and handle the reference under D73 before accepting deletion. Do not automatically recreate the configuration. Preserve current operating copies and historical QSO snapshots. Post-MVP, offer restoring the deleted configuration or cloning it to a new configuration as additional resolution options. Exact future identity, recoverable-content/retention, and transaction mechanics remain open; this is equipment-reference scope, not a general restoration-policy change.

#### D184

For geographic autofill, use the grid area when coordinates are grid-derived and use the point when coordinates were explicitly supplied. Populate unambiguous geographic fields independently; allow manual selection for ambiguous fields. A calculated grid center must not imply an exact supplied position. Preserve the grid-area versus explicit-point basis through saved profiles, QSO snapshots, and synchronization so lookup semantics survive reload/reconnect. Manual corrections retain D78’s position-change lifetime; unset location details do not block local saving under D112. Exact geometry/boundary rules, native representation, and export encoding remain open.

#### D185

ADIF export/re-import is not required to be completely lossless. Deterministic transforms in each direction under D176 need not be inverses or preserve every aspect of the native model. Preserve D184 area/point semantics in native storage, profiles, QSO snapshots, synchronization, and applicable backup/recovery, without imposing an ADIF round-trip guarantee. Exact export field coverage and derived-coordinate treatment remain open; the assistant’s proposed omission of grid-center coordinates was not selected. Existing explicit mappings, disclosure, normalization, and native-information preservation decisions remain in force. ADIF is interchange rather than a full application-state backup.

#### D186

ADIF export includes the grid and available coordinates, including calculated grid-center coordinates. Ordinary ADIF may lose the native distinction between grid-derived coordinates and explicitly supplied points; re-import follows D173 rather than assuming derivation can be reconstructed. Preserve that distinction in native storage, synchronization, and applicable backups under D184. D185’s non-lossless interchange boundary applies. Exact field formatting/long-grid encoding remains open.

#### D187

MVP ADIF import/export supports the length-delimited ADI format only, accepting .adi and .adif filenames. XML-based ADX (.adx) import/export is post-MVP. Apply existing preparation, validation, mapping, partial-import, and cancellation rules to ADI. This does not constrain the native model to ADI field layout or text capabilities; exact text-conversion/export-warning behavior remains open.

#### D188

ADI export uses best-effort transliteration with warnings for human-readable native text outside ADI’s character repertoire. Omit optional fields that cannot be represented meaningfully and report those omissions. Preserve the original native text unchanged; export conversion does not edit contacts or cause sync revisions. Callsigns and other identifiers follow identity/format validation rather than transliteration into potentially different identities. Exact conversion tables/library, diagnostics, and remaining field-specific export behavior remain open. This policy is ADI export scope, not a selected provider-upload conversion rule.

#### D189

A documented Linux container deployment is sufficient for the MVP server. Native Windows server hosting is not required for MVP. This server release target is separate from D162 Windows desktop support and does not change cross-platform client architecture/tooling. Exact container runtime/orchestration, image architectures, server/web/worker process packaging, and D123 database choice remain open. Native Windows server support is not a committed later-release feature.

#### D190

Use OpenID Connect (OIDC) for MVP web sign-in. Keep Clacks provider-configurable through standard OIDC. The unaccepted built-in username/password recommendation is withdrawn; local password authentication is not required for MVP. D65 browser-based desktop authorization remains in force; D191 later selects first-sign-in provisioning. Exact administrator bootstrap and desktop grant/credential mechanisms remain open.

#### D191

With OIDC, use self-service local account provisioning on first successful sign-in through the deployment’s configured trusted provider: create an isolated Clacks account for a new verified issuer/subject identity, or reuse its existing binding. Do not require administrator pre-creation or a separate Clacks approval queue. D64’s earlier “sufficient”/“can be” language was a permissive scope boundary, not a required workflow or prohibition on MVP registration; OIDC changes that scope. Ordinary provisioning does not itself grant server-administrator privileges. Exact role/bootstrap and identity-provider migration policies remain open.

#### D192

Manual, out-of-band server-administrator role assignment is sufficient. MVP does not require an in-application role-assignment interface or automatic provider-claim role mapping. This is separate from D191 automatic account provisioning and must not introduce an approval step for ordinary sign-in. Exact deployment/command/configuration method remains open; do not reinterpret “can be” as requiring one particular manual mechanism.

#### D193

Use browser-callback desktop authorization for MVP: the desktop opens the browser for sign-in/client approval and receives completion through a callback. Device authorization is a post-MVP fallback option. Both require client/server reachability; a separate computer is not required. Preserve D62 individually revocable client authorization and D190/D191 provider-based web sign-in/account provisioning. D370 subsequently selects the temporary loopback callback; standard protocol/library integration and per-client credential lifecycle remain to be specified.

#### D194

The local log itself does not warrant separate application-level encryption or an unlock/passphrase workflow for MVP. Directory credentials are sensitive and require protected local storage under D96/D97. Keep secret protection separate from ordinary log storage; exact operating-system credential-store integration remains open. This does not weaken account isolation or protection of synchronization credentials and server-held service secrets.

#### D195

Rejected callsign-directory credentials pause lookups for the affected provider with a clear diagnostic. Updated credentials or an explicit retry resume lookup. Logging, WS Suite capture, and synchronization continue normally. Missed enrichment remains available through D99 user-triggered bulk lookup; authentication recovery does not introduce automatic bulk catch-up. This decision covers authentication rejection, not every transient provider/network failure. Exact error classification and retry implementation remain open.

#### D196

Callsign-directory provider selection is account-level, shared by that account’s enrolled clients, rather than a client-local preference. D89 still provides the choice of QRZ or HamQTH and D96 keeps requests client-direct. Clients use their last received selection while disconnected from synchronization and receive changes on reconnect. Exact configuration interface and in-flight request handling remain open; automatic provider fallback/merging is not selected by this decision.

#### D197

MVP directory lookup queries only the account-selected provider. Automatic fallback to another provider is post-MVP. Combining/merging lookup results from multiple providers is excluded from the product direction, not deferred to a later release. D204 later supersedes directory-to-QSO fill-empty behavior with separate snapshots and logical fallback. Provider selection and independently sourced contact-confirmation enrichment remain distinct. Exact transient-failure retry mechanics remain open.

#### D198

Keep callsign-directory location information separate from the QSO’s actual contacted-station location. Directory location does not inherently establish where that contact took place, regardless of coordinate precision or whether a directory user supplied it. Do not automatically populate QSO location from directory data, including during entry, late-result enrichment, or bulk lookup; an empty QSO location does not change that rule. This separation includes directory grid/coordinates, locality, and location-derived administrative/entity/zone details. D204 extends source separation to all directory information, superseding D90/D95/D99 directory-to-QSO fill-empty behavior. D199 selects a logical fallback instead of explicit copying: use QSO location when present, otherwise available directory location. D200 includes exports and external uploads in fallback use. D201 selects whole-location precedence without field mixing; D203 selects synchronized per-QSO directory-location snapshots; contact-specific confirmation enrichment remains separate under D33-D36.

#### D199

Use a logical location fallback while retaining D198 source separation: when the QSO specifies where it happened, use that location; otherwise fall back to available directory-provided location information. Do not copy directory values into the QSO’s recorded location or require an explicit copy action for this fallback. Keep the source distinction available so fallback information is not represented as contact-specific evidence. D200 includes exports and external uploads. D201 selects whole-location precedence, including partially specified locations. D203 selects synchronized per-QSO directory-location snapshots. Exact geographic-calculation behavior remains open. D197’s rejection of merging directory-provider results is unchanged.

#### D200

Exports and external-service uploads also use D199’s logical location fallback: use the QSO’s recorded location when present, otherwise available directory-provided location. Native data keeps these sources separate; output transformation does not copy fallback values into the recorded QSO location. Existing service-specific field support/validation still applies. Ordinary ADIF or service output may not preserve the native source distinction, consistent with D185. D201 selects whole-location precedence. D203 selects retaining directory-location snapshots with their QSO and synchronizing them. How changed effective output affects upload scheduling remains open.

### Decisions D201–D251

#### D201

Resolve QSO-versus-directory location as a whole, not independently per field. When a QSO location is present, use it without filling missing location fields from directory data, including when the QSO location is only partially specified. Only fall back to directory location when QSO location is absent. Apply this source selection consistently to D199 fallback and D200 export/upload use. Geographic details derivable from the selected QSO location may be calculated under existing precision/ambiguity rules; gaps remain unknown rather than being patched with a different directory location. D203 selects retained/synchronized per-QSO directory-location snapshots; exact native representation remains open.

#### D202

Normalize the callsign used for directory lookup while preserving the QSO’s recorded callsign. For the discussed example, query OK2CQR for recorded OK2CQR/P. Lookup normalization is a request transformation, not a QSO edit, and does not replace the worked callsign with a provider-returned identity. Preserve request-to-original-contact correlation for delayed results under D95/D116. Exact normalization rules for other callsign forms and provider alias provenance remain to be specified; do not infer indiscriminate removal of all slash-separated components.

#### D203

Retain a separate directory-location snapshot with each QSO, including its provider and lookup time, and synchronize that snapshot with the QSO. Keep it distinct from the QSO’s recorded location under D198/D201; use it for D199/D200 fallback, including offline exports and server-side uploads. Later directory changes do not silently alter saved snapshots. This is contact-associated retained data, not a reusable persistent lookup cache for future contacts under D98. A provider-returned callsign is not required by this decision. D206 selects snapshot replacement/failure/cancellation behavior. Exact representation and resulting upload scheduling remain open.

#### D204

Keep all directory information separate from recorded QSO information, including names. Use recorded QSO information when present, with directory information as the logical fallback; retain D201 whole-location precedence rather than mixing location fields. Extend D203’s retained/synchronized per-QSO snapshot to supported directory information generally, including provider and lookup time. Entry lookup, late results, and bulk lookup update separate directory information rather than writing recorded QSO fields. Apply the same source-selection principle to display and supported export/upload fields. This supersedes the earlier directory-to-QSO fill-empty model in D90/D95/D99. D205 selects bulk application as results arrive and a clear report afterward. D206 selects snapshot replacement/failure/cancellation behavior. D208 excludes explicit fallback suppression from MVP without committing to a future feature. Exact supported fields remain open.

#### D205

Bulk directory lookup applies results to the scoped QSOs’ separate directory snapshots as results arrive, with a clear report afterward. No preview/approval stage is required before application. Recorded QSO information remains untouched under D204, and D209 synchronizes snapshot changes with automatic latest-successful-lookup arbitration rather than user conflict resolution. The report distinguishes updated/new snapshots, materially unchanged results, no directory record, lookup failures with reasons, and any unprocessed contacts. D206 selects whole-snapshot replacement on success, preservation on failure/not found, and retaining completed updates on cancellation. Exact report presentation and implementation mechanics remain open.

#### D206

A successful directory lookup replaces the QSO’s separate directory snapshot as a whole, including provider and lookup time; fields absent from the new result do not retain stale values from the prior snapshot. Lookup failure or callsign not found preserves any existing snapshot and reports that outcome. Cancellation preserves completed updates, stops further application at a safe boundary, and reports remaining contacts as unprocessed. Recorded QSO information stays untouched. D209 automatically selects the latest successful snapshot without user resolution; exact transaction/cancellation implementation and report presentation remain open.

#### D207

When a saved QSO’s recorded callsign changes, retain its separate directory snapshot if the normalized directory lookup key is unchanged. If that key changes, stop using the old snapshot immediately and look up the new station. A failed/not-found lookup for the new key must not leave the old station’s snapshot supplying fallback. Apply snapshot-to-current-key association consistently to display, exports, and server uploads; delayed responses for a previous key must not restore obsolete fallback. D206 preserves an existing snapshot on failure only when it is associated with the current normalized key. Exact invalid-snapshot retention and request lifecycle mechanisms remain open.

#### D208

MVP has no explicit “leave this unknown despite available directory information” control. Clearing recorded QSO information therefore exposes eligible directory fallback under D204; D201 whole-location precedence still applies. A future explicit fallback-suppression feature is undecided, and if ever added is post-MVP. Do not treat it as an accepted later-release commitment.

#### D209

Resolve directory snapshots automatically using the latest successful lookup time recorded by the fetching client, with a stable deterministic tie-breaker for equal timestamps and no user resolution. Preserve lookup time and tie-break identity through synchronization/retry; arrival at the server is not a new lookup. Apply selection separately for each QSO and normalized directory lookup key so old-key results cannot displace new-key snapshots. Treat provider-supplied snapshot arbitration separately from whole-record user resolution for recorded QSO facts. Failure/not-found does not replace a successful snapshot; the winning snapshot replaces values as a whole without field merging. Clock accuracy affects which lookup is truly freshest, not deterministic convergence; accept that freshness tradeoff for directory data. Exact timestamp representation and tie-breaker encoding remain open.

#### D210

ADIF export uses the client’s current saved local state, including unsynchronized contacts/edits and unresolved local working information, rather than restricting output to server-accepted state. Export one local working version per QSO; do not emit competing conflict versions as duplicate contacts. Exclude locally pending deletions consistently with D181 normal history. Use the local eligible directory snapshot and D204 fallback, preserving D201 whole-location precedence and D207 current-key association. Export requires no successful synchronization; it does not resolve conflicts or constitute a lossless native-state backup under D185. D211 selects consistent point-in-time local state and preservation of any existing destination file on cancellation/failure; exact output mechanics remain open.

#### D211

Export uses a consistent point-in-time view of local state, including scoped contact membership, recorded QSO values, and eligible directory snapshots. Logging and synchronization may continue without changing that export’s captured contents. Leave any existing destination file untouched if export is cancelled or fails; publish the completed destination only after successful export. Report exported contacts and applicable conversion warnings. Exact database snapshot/staging/finalization mechanisms remain open; this does not select a storage engine or add a server synchronization prerequisite.

#### D212

MVP offers ordinary ADIF export as the default and optional POTA-compatible export. Ordinary export emits one entry per QSO using supported reference fields/lists. POTA-compatible export groups files by our park/location and expands entries by applicable contacted-station park references under POTA submission requirements; a two-park to three-park contact produces two files with three entries each for the two-park operator. Expansion changes output only, preserving native QSO identities, recorded callsigns, timestamps, and the shared activity/reference model. Do not expand unrelated contest/event identifiers or infer automatic POTA submissions/server-upload expansion. Apply D210/D211 local-state and consistency/destination-protection rules. Report native contact count separately from output entry/file counts. Exact export-field validation, file naming/grouping details, and multi-file finalization mechanics remain open.

#### D213

Use PostgreSQL for server persistence and embedded SQLite for desktop-client persistence. The client requires no external storage service. Use ordinary database-library transaction management, short transactions, appropriate contention handling, and bounded batches where needed; do not prescribe a custom client write queue. Client storage must preserve offline working state, pending edits, comparison evidence, and durable synchronization checkpoints under the existing decisions. Server and client physical schemas may differ while preserving shared domain semantics. Exact schemas, versions, libraries, tuning and migration mechanisms remain open.

#### D214

GPL distribution is incompatible with Clacks’s intended licensing direction. Do not select a desktop toolkit by assuming its GPL route is acceptable merely because Clacks may be source-available. No exact Clacks license, permissive-only dependency rule, blanket exclusion of LGPL, commercial-license purchase, or acceptance of mandatory branding is selected.

#### D215

Use egui for the desktop interface, following the Rust-oriented toolkit comparison. Use the recommended eframe native application integration and egui_extras table facilities as the starting direction. Keep application and interface behavior in Rust, with embedded SQLite under D213. Preserve Windows-first delivery and cross-platform tooling under D83/D162. D273 subsequently selects wgpu as the initial renderer. Exact compatible dependency versions/features, optional workflow components, permanent interface layout and implementation remain engineering work; toolkit selection does not authorize a prototype or implementation.

#### D216

Use ordinary Serde-based serialization/deserialization of Rust data types as the starting point. Treat enum naming, unfamiliar-value preservation and schema compatibility as routine schema/implementation work, not a novel serialization architecture or a separate product-approval exercise. Preserve D175/D177/D178 mode values/context and interpretation-only recognition behavior, and D179 upgrade/data-retention rules. Exact data format, schema and serializer configuration remain implementation details; no bespoke encoding or extremely constrained-storage design is selected.

#### D217

Newly created Clacks logbooks default to automatic uploads disabled independently for both LoTW and QRZ, including offline-created logbooks. Upload enablement remains a per-logbook, per-service choice under D103. D109 manual upload remains available with its existing eligibility/pause rules; account-wide download matching under D110 is unchanged.

#### D218

Rejection of an unmatched downloaded contact applies to the version reviewed, not permanently to every version of that external contact. Suppress repeated delivery of the same rejected version. A changed downloaded version is evaluated through normal matching/review; if still unmatched, offer the existing explicit add/reject workflow again. Do not automatically add it. This does not change D54 suppression of known deleted QSOs. Exact provider update identity and comparison mechanics remain specification work.

#### D219

Reserve the commercial right to offer competing paid hosting of Clacks Log. The application license must not grant that use generally. Self-hosting remains supported under D03. Exact license text and permissions for modification, redistribution and other commercial uses remain unselected; this decision does not establish a blanket noncommercial-use restriction.

#### D220

Hosting Clacks for others is competing hosting even when provided free of charge; personal self-hosting for an individual remains allowed. D219’s hosting reservation is not limited to paid services. Do not propose a free/community/club-hosting exception on the assumption that lack of payment makes it noncompeting. Exact application license adoption remains unselected.

#### D221

Use the standard Elastic License 2.0 for Clacks Log’s own application code, without a free/community-hosting exception. This selects the license following D219/D220. Preserve separately applicable third-party licenses and notices, including those of adapted TrustedQSL/OpenSSL source, dependencies, fonts and data. This does not select a license-key/activation system, contributor agreement or subscription packaging.

#### D222

Design the web interface architecture for eventual logbook browsing and editing, reporting, and potentially logging. Its modest MVP administration/setup/diagnostics workflows are not the limit of its intended scope. Do not select an architecture on the assumption that the web interface will remain an administration interface. These future capabilities do not become MVP commitments; logging remains a possibility, not a guaranteed feature. No web framework or rendering architecture is selected by this decision.

#### D223

Use Leptos with Axum for the web interface, following the D222 broader-scope comparison: server-rendered initial pages with Rust/WebAssembly browser interactivity through hydration. Keep the web interface within the one Clacks server application and share framework-independent Rust domain logic with the desktop/server. No TypeScript application layer is required. Exact versions, optional components, styling and layouts remain open; this does not add future web features to MVP, require browser offline operation, or authorize implementation.

#### D224

Use a pure-Rust option for LoTW certificate import, certificate/key interpretation and signing. The user rejects the assessment that OpenSSL’s integration/support benefits outweigh its native packaging and maintenance costs. Remove OpenSSL as the recommended backend for this subsystem; do not reintroduce it as a fallback or mixed signing backend without a changed requirement. Use established Rust libraries and preserve current/legacy export and signature compatibility. Exact crate/version/feature selection and signing-path security/interoperability verification remain engineering work; this does not authorize implementation or change historical upstream license obligations.

#### D225

For ADIF imports, incomplete identity information may produce possible matches when the supplied information is compatible with an existing QSO. Missing historical station callsign or a broad mode without the existing contact’s specialization does not establish a confident match. Show these possible matches with the missing information explained and require explicit selection before updating or treating the incoming record as unchanged, even when only one candidate exists. Retain create-separate-contact and skip choices. Do not fill missing historical station identity from current defaults or treat explicitly different values as missing. D41’s confident-match rules and D165’s automatic unchanged skip remain applicable to confident matches; this decision does not select wider time tolerances or service-download matching rules.

#### D226

For ADIF imports, use a 30-minute window either side of the incoming contact’s start time to find possible matches, retaining the other compatible identity criteria and destination-logbook scope. Require explicit selection for these possible matches, even when only one exists. Preserve D41’s stricter precision-aware confident-match rules and D165’s automatic unchanged skip for confident matches; a wider-window candidate is not a confident match merely because it is unique. Retain update/create-separate-contact/skip choices. This does not select external-service download matching rules.

#### D227

For unassociated LoTW and QRZ downloads, reuse the strict confident-match versus review-only possible-match distinction from D41/D225/D226. Established associations take precedence. Otherwise apply a unique confident match automatically; compatible incomplete identity information or a start-time difference within 30 minutes either side produces possible matches requiring explicit selection, even when only one exists. Search all account logbooks, including upload-disabled books, and retain D111’s explicit selection of multiple local matches. Do not infer all copies automatically. If no candidate is selected, retain the explicit add/reject workflow, with destination choice on addition. D218 reviewed-version rejection and D54 known-deletion suppression remain unchanged. Provider confirmation matching and D124 applicability after edits are distinct from this local association policy. D251 subsequently supersedes the multiple-association/multiple-selection behavior: at most one local QSO per external record within an account and service destination.

#### D228

Include removal of imported LoTW private-key credentials in MVP. Prevent further signing with removed credentials while preserving public certificate metadata and submission history. If removal leaves contacts without an eligible credential, pause only the affected uploads under the existing routing rules. Local removal is distinct from revocation with the certificate issuer. Defer reusable certificate/private-key export from Clacks to post-MVP; documented native backup/recovery remains required and is distinct from that export feature. Exact in-flight operation, secret-store removal and backup-retention mechanics remain specification work.

#### D229

Make LoTW certificate supersession an explicit user action rather than inferring it during import. Import adds a validated credential; an equal callsign or later expiration alone does not supersede another certificate. Show resulting coverage gaps before applying explicit supersession. Retain superseded certificate metadata and submission history and exclude superseded certificates from signing under D17. If multiple credentials remain eligible and routing is ambiguous, pause only affected uploads under D53 until routing is resolved. Supersession is distinct from D228 private-key removal; exact coverage calculation, eligibility and overlap configuration remain specification work.

#### D230

Include full production-grade server observability in MVP: tracing, structured logs and metrics for server operators, including account-security events. This is separate from account-user-facing diagnostics and security-event history. Defer a retained account-scoped security-event history view in the web interface to post-MVP. Keep passwords, tokens and private keys out of telemetry. Exact instrumentation, export protocols, collection/visualization backends, retention and deployment integration remain specification/engineering work; this decision selects no observability vendor or additional application service topology.

#### D231

Clacks accounts have stable internal identities independent of external authentication identities. One Clacks account can be linked to multiple OIDC identities, including identities from multiple providers; each verified issuer/subject identity can be linked to at most one Clacks account. Do not model the relationship as one account per provider identity or derive the Clacks account ID from that identity. Authentication resolves an external identity to its linked Clacks account; account data, roles, credentials and enrolled clients belong to the internal account. D191 describes normal first-sign-in provisioning for an unbound identity, not a prohibition on linking additional identities to an existing account. Provider changes can use additional/removable login bindings rather than migrating the account itself. Linking/unlinking authorization, interfaces and recovery procedures remain to be designed; no administrator-only migration requirement, email-based automatic linking or account-merging feature is selected.

#### D232

Include self-service login-identity linking and unlinking in the MVP web interface. From an authenticated Clacks account, adding an identity uses a deployment-configured provider, requires recent authentication to the existing account and authentication of the additional identity, and binds it to the current internal account without new-account provisioning. Refuse linking an identity already bound to a different Clacks account; do not transfer ownership or merge accounts automatically. Unlinking requires recent authentication through an identity that will remain linked, and ordinary account settings must prevent removing the last login identity. Recovery when no linked identity remains accessible is a separate administration workflow whose scope remains open. Exact freshness thresholds, callback/session safeguards and interface layout remain specification work.

#### D233

Documented administrator-assisted account recovery is sufficient for MVP when none of the linked login identities is accessible. Establish ownership of the existing Clacks account out of band, then explicitly link a newly verified external identity to that same internal account. Preserve its data, service credentials and enrolled clients; record recovery in operator security logs under D230. Defer automated self-service account recovery to post-MVP. This exceptional recovery path is distinct from D232 ordinary self-service identity linking/unlinking and does not require administrator involvement in routine provider changes. Exact ownership-verification and administration mechanisms remain specification work.

#### D234

Leave substantive contributor-agreement modifications and legal drafting/review to a qualified lawyer. The assistant's proposed JetBrains adaptations are not an adoption-ready agreement or accepted drafting instructions. Retain the agreed business requirements, published-source findings and questions for counsel; remove the proposed clause rewrites from the planning recommendation. No contributor agreement, modified JetBrains form, original-license-continuity policy or unrestricted future relicensing requirement is selected. D221's public Elastic License 2.0 selection remains unchanged.

#### D235

Use encrypted WebSocket connections for MVP synchronization within the existing Axum server application. Carry bounded catch-up batches and ongoing bidirectional changes, acceptance results and acknowledgments over the same connection, following D61. Preserve the selected causal vector/timeline model, durable-installation-before-acknowledgment rule and record-level reconciliation semantics in both directions. Reconnect using durable checkpoints and fuller-state reconciliation where required. Exact message encoding, request correlation, backpressure, authorization/revocation handling and gap-free catch-up/stream handoff remain specification work. No additional service, alternative-transport fallback or browser offline-sync feature is selected.

#### D236

Use independent randomly generated per-account encryption keys for sensitive server-held service credentials and LoTW private keys. Protect those account keys with a deployment wrapping key, supplied through a protected key file outside PostgreSQL and the application image. Store encrypted account keys alongside encrypted credential records in PostgreSQL. Use established authenticated-encryption tooling with account/credential binding; no custom cryptographic algorithm or mandatory external key service is selected. Document key/database backup and recovery together, retaining the key versions needed for supported restores; do not silently substitute a new key when existing credentials cannot be decrypted. Account keys limit the scope of an exposed account key but do not protect all accounts from compromise of the deployment wrapping key together with the database, or of the fully authorized running server. Exact tooling, encoding, rotation and key-version retention remain engineering work. Client-local secret protection and server-only LoTW signing-key placement remain unchanged.

#### D237

Protect manually recorded QSO information from automatic replacement by downloaded confirmation details, consistently across fields rather than making IOTA a special exception. Retain provider-supplied details separately from recorded QSO facts, following the source-separation principle established for directory information. This supersedes D33–D36 wherever they authorize automatic replacement of recorded facts or treat such replacement as a QSO edit. Keep provider observations, confirmation status, association, duplicate-safe processing and exclusion of our station's operating context. Exact logical fallback ordering across confirmation and directory sources, whole-location selection and supported field coverage remain to be settled; do not infer an accepted combined precedence rule from this correction.

#### D238

Use recorded QSO information first, then eligible LoTW-provided information, then eligible QRZ-provided information, then the eligible directory snapshot as logical fallback. Keep all provider information separate; fallback does not fill or overwrite recorded QSO facts. For location, select the first available location as a whole, including a partial recorded location; never patch missing location fields from lower-priority sources. Recorded location-specific references also establish location presence, including park, summit, island and other location-specific contest/event references; leave unresolved geographic details unknown rather than using lower-priority provider locations. This is not limited to one reference program and does not treat participation-only identifiers as locations. Apply this source selection consistently to display and supported export/upload fields, retaining provenance and existing association/applicability rules. This completes D237's combined fallback policy and refines D34/D35 and D199–D204 accordingly. Exact supported fields, effective-output comparison and representation remain engineering work.

#### D239

Use Census TIGER/Line for US state/county geometry, HA8TKS's CQ/ITU zone boundary data, and Natural Earth geographic subunits/minor islands with an explicit Clacks-maintained DXCC correspondence/exception layer. Preserve source provenance and applicable redistribution notices. This selects the source combination recommended in references/location-lookup.md for D133's required US and desired worldwide coverage, retaining D134's independently versioned data bundled with releases and D184's grid-area versus explicit-point lookup behavior. Actual county-layer selection is routine data preparation, not a separate user policy. Source-vintage selection, DXCC mapping/exception authorship, reviewed topology corrections, date-line/datum handling, packaging and release validation remain engineering/data work; no complete worldwide accuracy guarantee or shipping-ready derived dataset is established. No application implementation is authorized by this selection.

#### D240

Provide an explicit desktop-client backup/restore process that accounts for restoring or duplicating client state, including enrollment/causal identity, pending work, comparison evidence and protected credentials. Implementing that process is not required for MVP; retain D57's documented MVP backup/recovery scope. Do not infer a new client identity merely from a change of physical computer or claim copied directories reliably identify themselves as a different installation. No particular identity-reset rule, clone detector, backup format, credential inclusion policy or recovery protocol is selected here. Distinguish server recovery from client-state restoration; the existing server-rollback and revocation policies remain unchanged.

#### D241

Launching the first-party hosted subscription can be post-MVP. MVP must support the selected self-hosted deployment; a commercial hosted-service launch is not a release requirement. Preserve D03's optional hosted product direction and D63's multi-account isolation architecture. Hosted pricing, billing, onboarding, service limits and operational commitments remain unselected; this does not select hosted/self-hosted feature differences or a launch date.

#### D242

Mark product and architectural planning for the agreed MVP complete. No known unresolved product or architecture decision blocks that scope. Reconcile the handoff and remaining-work index so concrete schemas, synchronization messages, transaction handling, service-field mappings, dependency selection/configuration, geographic-data preparation and validation are classified as implementation specification/design or verification work, not unresolved product decisions or an approval queue. Preserve deferred product scope, lawyer-led contributor-agreement work and all accepted decisions. A demonstrated contradiction, missing requirement or material tradeoff may warrant a focused future decision; unfinished engineering detail alone does not reopen planning. Completion of this phase does not authorize application implementation, a prototype, execution of an implementation plan, migrations or publication.

#### D243

Within the logbook/tag membership model under examination, keep the tag set as part of the QSO record and apply the existing MVP whole-record reconciliation rules. Accept resolution when one side changes tags and another changes other QSO fields, or when both change different tags and resulting records differ. Do not introduce separate synchronized membership records or special automatic tag-set merging merely to avoid these conflicts. MVP reconciliation is intentionally simple; refine it based on problems observed in real use rather than speculative conflict avoidance. This settles the record-boundary/conflict choice for the proposal, not its remaining ownership, synchronization-scope, deletion, import or publication-policy changes. No implementation is authorized.

#### D244

Within the logbook/tag model, deleting a logbook/tag prompts the user to choose among: leave all associated QSOs; delete all associated QSOs, showing how many also have other tags; or delete only QSOs unique to the tag being deleted. In every choice the selected tag is deleted. Leaving a QSO removes only that tag and preserves the contact, its other tags and service evidence; a formerly unique QSO can remain untagged. Deleting a QSO marks the account-owned contact itself as deleted, including when it has other tags; it is excluded from normal contact views, not converted to an active untagged QSO. This is a deleted state (analogous to Trash), distinct from removing tag membership. Untagged active QSOs arise only from preserving a contact while removing its last tag. This clarification does not add a Trash interface or change D55’s post-MVP restoration scope. Preserve D54's external-copy and deletion-memory policy and the applicable ordinary record reconciliation rules. This replaces the prior proposal of tag deletion always preserving all contacts and refines D107/D157 for the tag model; it does not approve other pending tag-model choices or bypass D243's whole-record conflicts. Exact preview/revalidation and storage mechanics remain engineering work.

#### D245

For each tag and external service, use a three-state automatic-upload setting: Enable, Forbid or Neither. For a QSO's current tags, any Forbid takes precedence over Enable; otherwise at least one Enable authorizes automatic uploading, subject to existing routing, cooldown and pause rules. Neither contributes no grant or prohibition; all-Neither or an untagged QSO has no tag-based automatic-upload authorization. A QSO is not uploaded separately for each granting tag. This replaces the proposed boolean/positive-grants-only policy in the tag model and refines D103/D217 for overlapping membership. It concerns automatic uploads; existing explicit manual-upload policy remains unchanged. The default state of newly created tags remains to be settled; do not silently equate the earlier boolean disabled default with either Neither or Forbid. No implementation is authorized.

#### D246

Default newly created tags to Neither independently for LoTW and QRZ automatic-upload policy under D245, including tags created offline. A new tag neither grants nor forbids uploading; its presence alone does not initiate automatic uploads or veto another tag's Enable. Any Forbid still takes precedence, and otherwise at least one Enable is required. This refines D217's automatic-upload-off default for the tag model without making Forbid the default or changing explicit manual-upload behavior. Other pending tag-model consequences remain under examination; no implementation is authorized.

#### D247

Use account-wide ADIF import matching in the logbook/tag model and add the selected tag to matched QSOs when applying the import. Reuse a confidently matched unchanged QSO rather than creating another identity; add the tag if missing and report added memberships separately from unchanged contact facts. Preserve Update/Duplicate/Skip for differing records: Update changes the shared QSO and adds the selected tag, Duplicate creates a separate QSO with that tag, and Skip changes neither facts nor tags. Ambiguous matches still require explicit selection; retain existing explicit selection for possible matches even when only one candidate exists. Tag-set changes remain ordinary QSO edits under D243. This supersedes D108's destination-only matching scope, retaining the existing matching confidence, validation and partial-import policies. It does not automatically collapse distinct QSO identities or settle the remaining synchronization-scope proposal. No implementation is authorized.

#### D248

Use one synchronization scope per account, replacing D158's per-logbook timelines and separate shared-data scope. The scope contains account-owned QSOs with their tag sets, tag definitions/lifecycle, contact-specific service and directory evidence, download coordination/dispositions and synchronized shared configuration. This supersedes D159/D160's division of contacts and evidence among book/account scopes, and D161's independent per-scope checkpoint/collection wording; full-account replication to every enrolled client remains. Maintain timeline ancestry, durable acknowledgments and the oldest authorized client's retention boundary at account scope, preserving indefinite authorized offline operation, actor retirement and recovery rules. Conflicts remain per record and acceptance may use bounded batches; one scope does not require an atomic account-wide commit. Current tag selection does not affect replication participation. Account isolation, server-only credential restrictions and client-local operating defaults remain unchanged. Independent per-logbook checkpoints/collection boundaries are relinquished. Remaining tag-membership controls and deleted-reference behavior are not silently settled by this scope choice. No implementation is authorized.

#### D249

Include adding and removing existing tags for individual QSOs and selected groups in MVP. Removing the last tag leaves an active, untagged QSO; it does not mark the contact deleted. Preserve the QSO identity, recorded facts and service evidence when changing membership. These are ordinary QSO edits under D243, including the accepted whole-record conflict behavior. This supersedes D106’s post-MVP restriction on native inter-logbook organization for the tag model: adding Main and removing an activity tag needs no export/re-import or ownership transfer. Broader bulk editing of QSO facts remains post-MVP under D45. Automatic assignment/creation rules remain outside MVP. No implementation is authorized.

#### D250

When synchronizing a QSO that references a deleted tag, preserve the pending QSO locally and require explicit removal of that membership or replacement with an existing tag before shared acceptance. Removing the last membership may leave the QSO active and untagged under D249. Do not silently discard the QSO, recreate the deleted tag or automatically rewrite its tag set. Unrelated synchronization continues. Apply ordinary whole-record reconciliation to existing QSOs; if the QSO itself was marked deleted, the pending edit remains a QSO deletion/edit case, not merely a missing-tag issue. This replaces D157’s missing-owner-logbook framing for tag references, retaining D243 reconciliation and D244’s selected QSO deletion effects. No implementation is authorized.

Validate references in the reverse direction as well: flag a pending tag deletion if an active QSO would still reference that tag in accepted shared state. This is a reference-validity issue, not a competing edit to the tag record. Recheck surviving references in the deletion's acceptance transaction so a concurrently accepted membership cannot bypass the check. If deletion is accepted first, a later QSO proposal referencing that tag follows the forward check above. Reconciliation uses resulting record states; it must not infer a retained delete-all action or automatically delete an additional QSO. Nonconflicting QSO deletions and membership additions retain their ordinary record-comparison classification; unrelated synchronization continues.

#### D251

Within an account and external-service destination, allow at most one local QSO associated with a given external record. A QSO can still have its own associations to different services/destinations; this is not a global one-service-only rule. Preserve established-association precedence, automatic association for a unique confident match, and explicit selection for possible matches even when unique. When multiple local candidates are ambiguous, select at most one QSO for that external record. This supersedes D110/D111’s multiple-association direction, D160’s multiple-contact observation associations and D227’s multiple-selection refinement. Distinct local QSOs may still share candidate-matching fields, and Duplicate remains available; a duplicate does not inherit an external association already held by another QSO. If its publication would target an already-associated external record, require resolution rather than silently creating another association or replacing that record’s contents. Preserve deletion memory, imported service evidence and account/destination isolation; historical upload/confirmation claims are not authority to bind one external record to multiple QSOs. Exact provider identity, atomic uniqueness enforcement and collision diagnostics remain engineering work. No implementation is authorized.

### Subsequent design refinements

The [native-model specification](native-model.md) defines the native contracts incorporating these refinements.

#### D252

Require both own-station and contacted-station callsigns in every saved QSO. Imports missing our callsign require explicit historical callsign assignment before creating a contact, including when a service download is explicitly added locally; never silently substitute current operating defaults. Existing-contact updates retain omitted-field semantics under D42, and no update may clear a required callsign. Records still missing required callsigns are not created and are identified in the partial-import report. Incomplete entry/import working data may exist before saving. This refines D38, D112 and D166, replacing permission to save a contact with unknown own-station callsign while retaining unknown optional historical context. Incomplete incoming identity can still produce review-only matches under D225; choosing Duplicate requires a valid new contact. This choice does not require distinct station snapshot types.

#### D253

Include an optional receive band/frequency pair in MVP, alongside primary operating information containing a frequency, a band, or both under D112/D166. Frequencies use whole-hertz resolution. Apply the existing frequency/band consistency rules independently to each pair: editing frequency determines band, editing only band clears that pair's exact frequency, and usable imported frequency determines or corrects that pair's band with a warning for mismatches. An absent receive pair means no override, using the primary pair for both directions. A present receive pair supplies receive information as a whole; preserve band-only overrides without borrowing the primary frequency, even when the bands match. If the primary frequency is unknown with no receive override, the effective receive frequency is also unknown. Receive information is optional and does not change the other minimum saved-contact requirements. This adds native contact/interchange representation for split-frequency and cross-band contacts; computer-aided transceiver control remains post-MVP.

Use the complete ADIF 3.1.8 band catalog as the initial recognition baseline, with a text-preserving fallback for unfamiliar labels. Preserve underlying band values across clients/upgrades; recognition changes affect interpretation without rewriting saved values, creating edits or synchronization conflicts.

#### D254

Model reference groups and their behavioral properties as data. Individual contests/events are groups themselves, peers of POTA, with their own reference definitions and properties. A participation/reference association contains a group ID and one optional identifier; each group defines whether that identifier is required, allowing group-only participation where appropriate. Keep contest exchanges and serial-number assistance separate from participation/reference information; they do not require named fields in a reference association. Dedicated exchange recording remains post-MVP under D129. Group definitions specify normalization and syntax validation for entry and explicit edits, without silently rewriting existing references; syntax checks do not require a reference-value catalog.

Group definitions also support optional reference catalogs. Catalog provenance records dataset name, optional publisher/source URL, obtained-at time and optional upstream version. The stable catalog ID identifies the dataset; do not introduce a separate catalog-version field or version scheme. Keep catalog metadata separate from recorded QSO information; catalog descriptions and reference locations do not assert the actual station position or add fields to the saved group-ID-plus-optional-identifier association. Default and future user-defined groups have the same catalog capabilities. Catalog acquisition, refresh and distribution workflows remain specification work; D131 makes manual entry sufficient rather than prohibiting catalog support.

Preserve unrecognized group associations and their supplied values without guessing their properties. Later definitions may interpret them without rewriting recorded values, creating edits or synchronization conflicts, or changing stored equality/reconciliation solely because recognition became available. A reference belongs to a group, and the application uses the group definition to interpret properties such as whether its references bear operating-location context.

User-defined reference groups must support the full behavior available to groups included by default through the same generic engine; do not encode each program as an application-specific reference kind or select privileged behavior by program name. Default groups are definitions using the same capabilities, including those needed for their location and interchange/export behavior. Retain own-station/contacted-station sets and deduplication by group ID and optional identifier, including group-only associations. This refines D127 and the location-reference presence rule under D238.

Seed default group definitions into each new account as ordinary account-owned shared records and synchronize them to enrolled clients, where locally synchronized definitions support offline use. Default groups do not have a separate client-bundled interpretation path. MVP does not include user management of groups. Future user-defined groups retain full capability parity; deferring management does not permit hardcoded default-group privileges. The native-model specification defines the logical group schema and mapping contracts; D131 does not require reference-value/park catalog lookup in MVP.

#### D255

Export context consists of an output profile and an optional target group, separate from the selected contact set. Group definitions distinguish ordinary interchange mappings from mappings for exports targeting that group. Ordinary ADIF without a target uses ordinary interchange mappings, including supported reference lists, and emits one entry per QSO without activating program-specific expansion. Emit CONTEST_ID when a QSO has exactly one distinct applicable contest identifier. If it has several, identify the ambiguity during export preparation and allow selecting a contest context or proceeding with those ambiguous CONTEST_ID fields omitted and reported; contacts belonging to different single contests do not create ambiguity with each other. For ordinary ADIF targeting a contest, that contest determines CONTEST_ID and other contest associations do not compete. A program-compatible profile uses its selected group's submission mappings, validation, partitioning and expansion. Conflicting assignments within a selected definition are configuration errors reported before writing output. Do not silently choose a contest, add participation, rewrite native associations or change the selected contact set. This replaces the unaccepted generic per-binding omit_with_warning proposal with context selection and explicit handling of representation limits. It refines D212/D254 without changing D210/D211 local-state and output-publication safeguards.

### Decision D256

#### D256

Mode, submode and band recognition must be data-driven. Keep the parent-only versus specialization-with-optional-parent structure and recorded labels independent of recognition. Concrete families, specialization names, deprecation and band definitions/ranges belong to catalog data, not Rust enum variants or generated code mappings. Adding valid catalog rows must extend recognition without a source-code change. Preserve the existing recorded values, serialization, equality and ordering when catalogs differ or change. Interpretation may expose catalog metadata without rewriting recorded context. This supersedes D177's type-encoded known-family representation while retaining D174–D176/D178 preservation and interchange-boundary behavior.

### Decision D257

#### D257

Use indexed FlatGeobuf layers with `geo` geometry operations and Rust source-ingestion tooling for offline geographic data. Package compatible layers with a versioned manifest recording source pins/digests, redistribution notices, transformations, mappings, corrections and coverage limits. The producer and reader/query components must be production components that later offline assistance integrates and extends. Retain D239's source selection and D184's point versus whole-grid-area semantics. Spatial indexes select candidates; geometry predicates establish actual coverage, including holes, overlaps and uncovered portions. No JavaScript execution, Python tooling or runtime geographic database service is required. D258/D259 supply the subsequent regional coordinate handling and documented approximation; this decision does not certify a shipping dataset. See [the geographic research](../references/location-lookup.md).

### Decision D258

#### D258

Use `proj4rs` for producer-side execution of the identified geographic coordinate transformations: the mainland, Aleutian and Hawaiian three-parameter operations and the Puerto Rico/U.S. Virgin Islands grid operation documented in [the coordinate investigation](../references/geographic-coordinate-operations.md), within their established applicability. Use explicit parameters and pinned correction data rather than generic datum-name defaults or silent fallback. Preserve D257's Rust ingestion, `geo` and FlatGeobuf direction. Numerical/real-grid validation and source-specific applicability remain delivery obligations. This does not establish a transformation for Pacific territories or outlying/offshore areas whose source metadata or applicable operation remains unresolved.

### Decision D259

#### D259

Use a documented no-shift approximation for remaining Pacific-territory, outlying-island and offshore Census components without an established applicable transformation under D258. Preserve their supplied longitude/latitude numerically in the common artifact convention, retaining original source declarations and explicit approximation provenance. Record that accuracy is unestablished: modern-frame differences plausibly have metre-scale effects, but this is not a measured bound on these inputs or their boundaries. Do not infer legacy island datums or apply historical corrections merely from geography. Retain ordinary point/whole-grid lookup and manual-correction behavior; this choice does not alter recorded positions, invent missing geometry, add uncertainty buffers or require source-producer clarification before proceeding. The approximation is an explicitly assigned operation for these components, not a fallback for failed supported transformations, missing correction grids, malformed inputs or unexpected source-coordinate declarations. See [the coordinate investigation](../references/geographic-coordinate-operations.md).

### Decision D260

#### D260

For geographic lookup, an explicit point exactly on a shared boundary returns the adjoining distinct geographic values as ambiguous. For a grid-area query, a region contributes a candidate only when the intersection includes area; an edge-only or corner-only touch does not introduce ambiguity. Any area in multiple distinct geographic values is ambiguous, regardless of relative overlap size; do not choose the largest overlap. Retain D184's independent field resolution and the requirement that uncovered portions prevent confident autofill. Evaluate ambiguity by geographic value, not by the number of source features or date-line fragments representing that value. These rules concern topology against the compiled reference geometry, not a guarantee about real-world boundary accuracy; D259's approximation and manual-correction rules remain unchanged.

### Decision D261

#### D261

Geographic-data publication permits explicit, reviewed omissions with their affected areas and reasons recorded. Unexpected parsing, transformation, topology or mapping failures stop publication and preserve the existing artifact; do not silently drop failed inputs and publish the rest. A newly defective zone must be corrected or explicitly excluded before publication. Validate against each bundle's declared coverage: representative artifacts satisfy #5's initial scope, while #23 extends the same pipeline to the required coverage. This does not make complete worldwide coverage a release requirement, permit undisclosed reductions of declared coverage, or classify D259's accepted approximation as a failed transformation.

### Decision D262

#### D262

Compile DXCC mappings, overrides and exclusions into the delivered geographic features during artifact production. Retain authored rules and their source/provenance as build inputs; runtime readers query the compiled features without interpreting source-specific mapping precedence. A reviewed exclusion removes the affected area from the parent entity's geometry; without a supported replacement it remains uncovered rather than inheriting a known-wrong parent. Supported replacement geometry becomes the appropriate entity's feature in the produced bundle. This does not invent missing exclusion/replacement geometry or claim that the DXCC crosswalk is already complete. Preserve D260's boundary/coverage behavior and D261's publication validation policy.

### Decision D263

#### D263

Store reviewed geometry corrections as complete replacement geometries in ordinary GeoJSON, accompanied by the expected source revision, feature identity, correction rationale and supporting references. The producer verifies the expected source before applying a replacement; upstream changes require review rather than silently replaying a correction against different input. Do not introduce a custom vertex-edit patch language. Replacements still require review for intended geographic meaning and validation through the production pipeline; valid topology alone does not establish correctness. Retain D262's compilation of mapping exceptions into artifacts and D261's publication safeguards.

### Decision D264

#### D264

Issue #5's acceptance set covers adjacent U.S. counties and state boundaries; Connecticut county-boundary source selection using its eight actual counties rather than planning regions; every selected coordinate-transformation path, no-shift components and date-line crossings; at least one reviewed replacement of real defective zone geometry, with other included defects corrected or explicitly excluded; ordinary DXCC mappings, a sourced exclusion/replacement case and an honestly reported coverage gap; point and complete-grid queries covering shared boundaries, edge/corner touches, holes, overlaps and partial coverage; and publication failures that preserve an existing artifact. Real-source cases pass through ingestion, compilation, FlatGeobuf output and the production reader. Independently authored small geometries may supplement exact boundary tests, without replacing real-source evidence or introducing a disposable pipeline. This exercises the selected mechanisms without moving #23's expanded-coverage obligation into #5.

### Decision D265

#### D265

Use `lib/clacks-geography` for artifact contracts, validation, indexed reading and point/grid-area queries; `lib/clacks-geography-builder` for source acquisition, ingestion, transformations, corrections, mapping compilation and publication; and `bin/clacks-geography-build` as a thin, documented command-line entry point delegating to the builder library. The builder depends on the reader library and validates generated artifacts through its production query interface. Desktop integration depends on the reader library without source-ingestion/download/transformation dependencies in its normal dependency graph. Prefer this explicit split to a single library separated by build/runtime feature combinations; do not introduce an additional shared abstraction crate. This selects a layout, not authorization to implement before the specification and plan are agreed.

### Decision D266

#### D266

The production geographic lookup contract returns distinct candidate values and complete/partial/no coverage independently for each field. One candidate with complete coverage is eligible for autofill; multiple candidates with complete coverage are ambiguous; candidates with partial coverage may be presented but do not permit autofill; no candidates with no coverage means unknown. Coverage concerns the requested point or whole grid area under D260, not the fraction of the world included in a bundle. Resolve fields independently so county ambiguity does not block a fully covered, unique state result. Artifact corruption and read failures remain errors rather than unknown-location results. Identify the bundle used on the lookup result for provenance without changing recorded QSO information. D267 refines candidate contents; concrete contracts are proposed in the geographic-pipeline specification.

### Decision D267

#### D267

Geographic lookup candidates carry source identity for provenance, a display label and an explicitly mapped recordable value as separate information. The producer supplies these mappings; applications must not infer recordable values by stripping label suffixes or guessing abbreviations. For example, King County's Census identity, display label "King County, Washington" and recordable value `WA,King` serve different purposes. Use values compatible with existing recorded geographic fields and adapters without introducing a universal source-ID encoding into QSO records. This defines lookup-generated values, not normalization of existing imported/manual text. Preserve existing text and snapshot/update rules; changed labels or mappings in a later dataset do not rewrite saved records. Distinct candidate values and coverage retain D266's meaning when multiple source features contribute to the same value.

### Decision D268

#### D268

Use one explicit geographic bundle-schema version, separate from the data-release identity. The schema governs manifest structure, layer properties and their interpretation. New data releases using a supported schema do not inherently require an application upgrade; incompatible encoding or interpretation changes require a new schema version. Readers reject unsupported schemas explicitly before lookup rather than guessing compatibility from application release numbers or using per-layer capability negotiation. Verify the manifest's layer identities and checksums so mixed or damaged bundles produce errors. Preserve D134's independent data-release boundary and D266's distinction between reader failures and unknown geographic results. A standalone updater remains outside MVP scope.

## Intentions with details still open


D221 selects standard Elastic License 2.0 following the D219/D220 hosting policy. Optional subscription packaging and contributor-rights arrangements remain open; application-license selection is settled. D215 selects the desktop toolkit. ADIF workflows are largely settled; mapping and workflow contracts live in [product.md](product.md), with remaining implementation tracked in [GitHub](https://github.com/jhelwig/clacks-log/issues). The hosting subscription is optional; self-hosting must remain possible.

Retain imported upload and confirmation history when reasonable. Exact mapping and whether LOG4OM exports sufficient information depend on the external-service model, whose basic separation is accepted in D32 but whose detailed representation is not yet defined. D50 accepts positive imported upload status for scheduling without prior verification; D52 accepts recognized positive imported confirmations immediately with available dates and provenance; exact field mapping remains open. Do not infer that an imported status is a complete Clacks Log publication or confirmation record.

## Superseded approaches and corrections

- This project is planning a new logger.
- Fixed two-client wording is superseded by D02.
- LOG4OM's manual LoTW batching versus automatic QRZ/eQSL upload is an observation, not a requirement. D09-D11 describe Clacks Log scheduling.
- A preconfigured TrustedQSL (TQSL) signing tool installation supplied by the operator is not the service architecture.
- Direct tqsllib foreign function interface (FFI) workers and temporary materialized stores were explored. Temporary stores seemed feasible for unmodified tqsllib, but D13 selects a source-guided Rust port instead.
- The Rust port is not black-box reverse engineering. A reference implementation and configuration definitions are available.
- The earlier 4-8 person-week estimate was withdrawn because it was insufficiently grounded. No replacement effort estimate has been accepted.
- Remote database access, live QSO forwarding, and synchronizing whole SQLite files do not establish application-level offline reconciliation.
- Do not manufacture a separate policy for compatible precision refinements versus different values if both would receive the same field-reconciliation behavior.

## Planning discipline

When a decision changes, update the current design documents and this register together. Record substantive supersession explicitly. Do not turn an assistant suggestion, an attachment's instruction, or incumbent behavior into an accepted requirement.

### Decision D269

#### D269

Keep each prescribed geographic coordinate operation exact at boundaries between different operations, including adjacent no-shift areas. Report the resulting seams, including gaps or overlaps, rather than adjusting the no-shift boundary, blending operations or snapping geometry to force agreement. Shared-edge consistency applies within the same operation. The pinned San Juan land-mask example exhibits approximately 1.02 m displacement between the grid-shifted boundary and its no-shift counterpart; this is distinct from the segment-subdivision computational tolerance.

### Decision D270

#### D270

Use an automatically managed default desktop workspace in the platform's per-user application-data directory, creating it on first use and reopening it on later launches. Provide a path override for development and troubleshooting. This supplies the runtime workspace-discovery behavior left open by the client-store specification. Preserve independent offline use, the local-only versus enrolled account distinction and existing open-failure preservation; do not silently replace an existing workspace after an opening error. The [desktop specification](desktop.md#4-workspace-and-configuration) defines directory/override and startup integration, with engineering defaults identified separately.

### Decision D271

#### D271

Use an application-owned multi-thread Tokio runtime for the desktop alongside egui's main-thread event loop. Background tasks progress independently of UI rendering. Deliver results through standard nonblocking channel integration and request egui repaint when needed, leaving UI state on its owning thread. Reuse the existing asynchronous store APIs, SQLx pool and transaction semantics. Application composition owns the runtime lifetime; do not introduce a separate database execution queue or async framework. The [desktop specification](desktop.md) defines runtime ownership and shutdown coordination; concrete task types and runtime tuning belong to the implementation plan.

### Decision D272

#### D272

Use graceful desktop shutdown with an explicit force-quit option when shutdown is delayed. Stop admitting new work, cancel background operations at supported safe boundaries and let already-started local save transactions finish before closing the store, flushing telemetry and shutting down the runtime. Keep the window responsive and let the operator continue waiting or explicitly force quit rather than applying an automatic forced-exit deadline. Do not require synchronization completion before exit. Preserve D82's close-stops-client behavior, D113's nonpersistent unfinished drafts and existing durable pending-work/transaction recovery guarantees. Force quit may interrupt unfinished work. The [desktop specification](desktop.md#7-graceful-shutdown-and-failure) defines shutdown coordination and identifies its delay threshold as an engineering default.

### Decision D273

#### D273

Use wgpu as the initial desktop renderer through eframe, refining D215's previously open renderer choice. Keep the interface in ordinary egui components and retain the Windows MVP and eventual macOS/Linux architecture. Eframe defaults to wgpu; its supported native platform backends fit that direction. Glow remains an evaluated alternative with a documented binary-size advantage, not the selected initial renderer. Exact compatible dependency versions, enabled backend features and actual native rendering verification remain engineering work for issue #39. See [eframe features](https://docs.rs/eframe/latest/eframe/#feature-flags) and [wgpu platform support](https://github.com/gfx-rs/wgpu#supported-platforms).

### Decision D274

#### D274

Use local rotating structured JSON logs as the desktop default, with `info` as the normal level following the existing observability policy. Retain tracing fields and span context; use rotation, a retained-file limit and a bounded off-thread writer. Provide optional console output and configurable filtering for development and troubleshooting. Use standard tracing formatting/appender facilities directly. The application remains usable offline without a collector. D275/D278 select optional export and transport. The [desktop specification](desktop.md) defines composition, configuration and engineering defaults for retention/buffering.

### Decision D275

#### D275

Support opt-in OpenTelemetry Protocol (OTLP) export for desktop traces, metrics and logs to explicitly configured destinations. Retain D274's local rotating logs as the default and continue them when export is enabled. Use existing adapters for the current `tracing`/`metrics` producers and standard OpenTelemetry exporters, preserving bounded buffering, failure independence and graceful shutdown. Collector outages do not block ordinary logging or storage. Normal offline operation requires no collector, and no hosted destination is enabled automatically. D278 selects transport. The [desktop specification](desktop.md) defines configuration/composition; compatible dependency versions/features require implementation-plan verification. This does not select server export protocols or a mandatory collector/vendor.

### Decision D276

#### D276

Use one running desktop process per workspace. Subsequent launches targeting the same workspace request activation of the existing window and then exit. A path override selecting a different workspace may run separately. Scope coordination to workspace identity rather than imposing a global one-process restriction. The desktop runtime owns its workspace's background activities, while existing SQLite transactions and stale-save guards continue to establish database correctness. D279 selects the coordination mechanisms; the [desktop specification](desktop.md#5-one-process-per-workspace-and-activation) defines ownership and activation behavior.

### Decision D277

#### D277

Use an optional local `config.toml` file for desktop startup and diagnostic settings, with its default location respecting operating-system application-configuration conventions. Support explicitly selecting the configuration-file path on the command line, separately from D270's workspace-path override. Use explicit command-line settings, then standard logging/OpenTelemetry environment settings, then file values, then built-in defaults. A missing default configuration file uses defaults. Preserve the existing storage contracts for station profiles, current operating defaults and account data. The [desktop specification](desktop.md#4-workspace-and-configuration) defines path/precedence semantics and engineering defaults; exact typed keys and CLI help belong to the implementation plan.

### Decision D278

#### D278

Use HTTP/protobuf with Rustls for desktop OTLP export. HTTP carries binary Protocol Buffers payloads for traces, metrics and logs; HTTPS uses Rustls. Use the standard Rust OpenTelemetry exporter and preserve D275's explicit opt-in, configured destinations, bounded buffering, failure independence and continued local logs. Concrete compatible dependency versions, feature selection and HTTP-client/processor integration remain engineering work for issue #39. This refines D275's open desktop transport choice.

### Decision D279

#### D279

Use a standard-library lifetime file lock plus `interprocess` for desktop activation. Hold an exclusive `std::fs::File::try_lock()` lock on a separate workspace coordination file for the owning process's lifetime; file existence alone does not indicate an active owner. Use `interprocess` local sockets for activation requests, with its Windows named-pipe and Unix-domain-socket implementations. Scope both to the same workspace and coordinate endpoint recovery/cleanup under ownership. Preserve the existing SQLite transaction and stale-save protections. This selects D276's concrete coordination approach. The [desktop specification](desktop.md) defines ownership-sensitive startup/shutdown and endpoint requirements; exact platform naming/security settings belong to the implementation plan.

### Decision D280

#### D280

Use explicit Apply/Cancel for the full operating-context editor. Keep edits in a temporary form; logging uses the previously applied context until Apply validates and commits the complete replacement. Cancel discards unapplied edits without changing active defaults. Saving a reusable location profile is a separate explicit action and does not implicitly apply or reload the active context. Preserve client-local persistent defaults, explicit profile loading, the separate operator/power scopes and historical QSO snapshots. This defines the operating-context editing interaction for issue #7; it does not introduce persistent unfinished forms. D288 adds compact-summary shortcuts, including inline power editing with its own commit/cancel boundary.

### Decision D281

#### D281

Track a selected equipment configuration and its last-loaded baseline using persistent client-local metadata, including selections made through a profile default. Compare the latest locally available saved configuration against that baseline to identify saved-source changes. Under D285, manual equipment adjustments clear the selection and baseline rather than retaining a local-adjustment association. Loading a profile without an equipment default preserves current equipment and its selection metadata. Commit staged equipment and its optional selection metadata together on Apply under D280; Cancel preserves the previously active state. Saved equipment edits require explicit reload, and historical QSO snapshots retain copied equipment facts without requiring a source configuration identity or display name.

### Decision D282

#### D282

Save Profile uses the edited form values without applying them to the active operating context. The action does not require Apply first. Validate the saved profile independently and retain its existing station snapshot and optional equipment-configuration reference; operator, transmit power and equipment descriptions retain their separate scopes. Save Profile is a shared-record save using ordinary new-record/stale-update rules. Apply remains the action that activates the edited operating context, and cancelling that form does not undo a separate profile save already completed. This refines D280’s separation of profile saving from operating-context application for issue #7.

### Decision D283

#### D283

Apply preserves the displayed operating-context values and any still-selected profile/equipment comparison baselines when a saved source changes while the form is open. Indicate changes in those selected sources; do not silently load newer values, advance their baselines or require a refresh solely because a shared source changed. D285 clears the affected selection and baseline on manual adjustment, ending its source comparison. Apply writes client-local operating state, preserving its existing stale-save protection. Save Profile and other shared-record updates retain their ordinary stale-save checks.

### Decision D284

#### D284

Selecting saved Rig B instead of selected Home’s default Rig A is an equipment override, independent of the station-profile selection. It does not edit either reusable record or detach Home. D285 supersedes the local station/equipment modified-state indicators: manual adjustments detach only the affected component, removing its source selection and comparison baseline. Saved-source changes remain independently visible for selections that still exist under D281/D283.

### Decision D285

#### D285

Manual changes to operating station values clear the station-profile selection and its comparison baseline; manual changes to operating equipment values independently clear the equipment-configuration selection and its comparison baseline. Retain the edited values without a source association, including no “based on” label, comparison, reload action or saved-source-change indicator for that detached component. Matching saved values again does not implicitly reattach it; selecting/loading a saved record explicitly establishes a new selection and baseline. Operator/power changes do not detach station or equipment selections. Loading a different saved equipment configuration leaves the station selection intact; station adjustments leave the equipment selection intact. Saved profiles retain their own optional default-equipment association.

Detachment is part of the pending operating edit under D280. Apply atomically commits values and optional selections/baselines; Cancel preserves the previously active values and selections. Separate completed reusable-record saves remain saved and do not implicitly select or activate their values. This supersedes retention of source associations across local adjustments in D24/D281/D284 and limits D23/D25/D283 source indicators to still-selected records.

### Decision D286

#### D286

Use separate editors for reusable station profiles and equipment configurations, distinct from editing the current operating context. Make a station profile’s optional default-equipment association explicit; editing equipment does not implicitly change that association. Station, equipment and operator/power views share one pending operating edit and one Apply/Cancel pair, without an intermediate “Return with changes” confirmation. Switching views preserves the pending edit. Reusable-record editors have their own Save/Cancel; they may be opened with pending operating values without first applying those values. Each save targets an existing record explicitly or creates a new one. A completed inner save survives cancelling its caller, and does not implicitly load, select or apply the saved values. This fixes interaction boundaries; exact labels and permanent window layout remain implementation design.

### Decision D287

#### D287

Deleting a selected station profile or equipment configuration clears only that component’s selection and comparison baseline, preserving copied operating values. Preserve the other component’s selection. Apply the same cleanup when a deletion becomes effective through synchronization; unavailable data or a failed read is not evidence of deletion. Clear affected selections in an open pending operating form as well, preserving its edits. Cancel does not restore a deleted-source selection or undo the deletion; normal Apply/Cancel continues to govern unapplied value changes. Saved-profile equipment-reference repairs still follow D73/D183. This extends D285’s independent detachment rule to deleted sources without changing historical QSO snapshots.

### Decision D288

#### D288

Use a compact desktop workbench with QSO entry and single-row-per-contact Worked
Before side by side above full-width current-logbook contact history, with an
active operating summary above them. Dedicated entry and Worked Before windows
add views without hiding or rearranging the workbench; entry views share the
same in-progress QSO on that client. Full operating editing uses a dialog.
Provide quick profile, equipment and operator changes from the summary, preserving
the existing loading, selection and reusable-record boundaries. Edit power inline:
Enter, Tab or leaving the field commits valid input; Escape restores committed
power. Use an explicit Operating settings… action beside the summary controls.
The [desktop UI contract](desktop-ui.md) owns the detailed behavior. The shared contract defines colors, typography and component behavior;
exact layout dimensions remain implementation choices.

### Decision D289

#### D289

Use the Defined sections visual direction for the desktop: panel headers,
restrained borders and recessed editable controls establish hierarchy without
reducing the accepted compact density. Apply the same visual language to the
workbench, dialogs and additional windows. This selects the surface treatment;
exact colors, typography, dimensions and shared component interaction states
remain under design in the [desktop UI contract](desktop-ui.md).

### Decision D290

#### D290

Use debounced live validation. Unapplied edits require a visible distinction or
highlight, rather than text alone. Keep changed, invalid, focused and selected
states distinguishable, including when a changed value is invalid. Validation
does not change the established save/apply boundaries. Exact debounce timing and
highlight styling remain under design in the [desktop UI contract](desktop-ui.md).

### Decision D291

#### D291

Highlight unapplied field changes with an amber tint and edge marker, alongside
an explicit text indication. Preserve that highlight when the value is invalid,
with validation errors and keyboard focus independently apparent. This selects
D290's visual treatment; exact color tokens and debounce timing remain under
design in the [desktop UI contract](desktop-ui.md).

### Decision D292

#### D292

Disable Save/Apply while validation errors remain, with a clear indication of
the fields and corrections needed. Debounced live validation updates action
availability; validation does not itself save or apply values. The
[desktop UI contract](desktop-ui.md) defines the shared presentation rule.

### Decision D293

#### D293

Preserve contact selection across filtering and sorting. Show an explicit hidden
selection count and provide Show selected. Selection targets stable contact IDs,
not row positions. The [desktop UI contract](desktop-ui.md) owns the presentation
and operation-scope rules.

### Decision D294

#### D294

Use quiet, local feedback for routine success and persistent actionable feedback
for failures and partial results. Bulk operations expose progress, cancellation
and record-specific outcome reports under their existing workflow contracts.
Distinguish local saves, synchronization and external-service success. The
[desktop UI contract](desktop-ui.md) owns these shared feedback patterns.

### Decision D295

#### D295

Editor-dialog close and Escape discard unapplied edits just like Cancel, without
an additional discard confirmation. Preserve separate completed saves and the
existing nested-editor boundaries. The [desktop UI contract](desktop-ui.md) owns
the shared dismissal rule.

### Decision D296

#### D296

Double-click or Enter on a contact opens its editor directly, without a read-only
detail view first. The [desktop UI contract](desktop-ui.md) owns contact-table
activation behavior.

### Decision D297

#### D297

Switching logbooks clears any contact selection. Cross-logbook selection uses a
search/view that is not logbook-scoped. Preserve selection across filtering and
sorting within a view under D293. The [desktop UI contract](desktop-ui.md) owns
the selection-boundary rule.

### Decision D298

#### D298

Follow OS light/dark appearance by default, with an application-level Light/Dark
override and a System choice to restore OS following. The
[desktop UI contract](desktop-ui.md) owns the shared appearance rule.

### Decision D299

#### D299

Use unified application-level UI scaling for text, controls, spacing and table
rows together, on top of OS display scaling. Radio data uses monospace by
default, with selectable font. The [desktop UI contract](desktop-ui.md) owns
the presentation and scaling requirements.

### Decision D300

#### D300

Make workbench panes user-resizable, remember their sizes locally and provide a
reset to the default arrangement. Minimum sizes keep controls usable. The
[desktop UI contract](desktop-ui.md) owns resizing and restoration behavior.

### Decision D301

#### D301

Both contact history and Worked Before support column visibility, ordering,
resizing and remembered sorting. Keep independent local settings for each table,
with a reset to defaults. The [desktop UI contract](desktop-ui.md) owns the
shared table-configuration contract.

### Decision D302

#### D302

Include All contacts and Untagged as built-in views alongside named logbooks.
All contacts shows every active QSO once per identity; Untagged shows active
QSOs with no tags. Neither view is a stored tag, has a tag identity or carries
an automatic-upload policy. Pending deletions remain excluded from normal
views. The [product contract](product.md) owns view semantics, and the
[desktop UI contract](desktop-ui.md) includes these choices in contact
navigation.

### Decision D303

#### D303

Default new QSOs to untagged when either All contacts or Untagged is selected.
Neither built-in view supplies a default tag, and a previously selected named
logbook does not remain as a hidden membership default. A named logbook supplies
its own tag for new contacts. Saved contacts and station/equipment defaults are
unchanged by view selection. The [product contract](product.md) owns these
selection semantics.

### Decision D304

#### D304

Remember the last selected view per local workspace and restore it on restart.
Use stable tag identity for named logbooks so renaming does not disrupt
selection. Start new workspaces in All contacts. If the remembered tag is known
to be deleted, open All contacts and explain the fallback. Do not overwrite the
remembered choice or infer deletion from a temporary loading failure. The
[product contract](product.md) owns restart selection behavior.

### Decision D305

#### D305

Provide a compact main-window logbook/view selector with immediate switching and
a Manage logbooks action opening a dedicated dialog. The dialog manages named
tags, their names and per-service automatic-upload policies through the existing
Save/Cancel contract. Built-in views are not editable tags. Delete opens a
separate impact review with the agreed three choices and contact counts. Keep
the dialog open while a submitted management operation is in progress. Show
failures in the dialog so the user can recover there. The
[desktop UI contract](desktop-ui.md) owns this interaction boundary.

### Decision D306

#### D306

Distinguish duplicate tag display names with short, collision-checked ID
suffixes. Add these suffixes only when the name is duplicated; unique names
remain unsuffixed. Make the full tag ID available to view and copy in Manage
logbooks. Do not change stored names or substitute shortened IDs for native
identity. This completes D119's display-disambiguation choice. The
[desktop UI contract](desktop-ui.md) owns its presentation.

### Decision D307

#### D307

When the selected tag's deletion becomes effective while the application is
running, switch to All contacts and explain the fallback. Apply this to both
local deletion and deletion received through synchronization. New QSOs then
default to untagged under D303. The fallback itself does not retag contacts or
repair pending references; preserve D244's chosen local-deletion effects and
D250's explicit repair of pending deleted-tag references. This extends D304's
restart fallback to live selection. The [product contract](product.md) owns
these semantics.

### Decision D308

#### D308

Persist selected-view state in the local workspace's SQLite database, separately
from synchronized records and station/equipment state. When the selected tag's
deletion becomes effective locally, commit the All contacts fallback in the
same transaction as that deletion, including deletion installed through
synchronization. Reuse the existing asynchronous session/store machinery; do
not maintain a second saved selection in framework presentation preferences.
The [client-store contract](client-store.md) owns the transaction boundary.

### Decision D309

#### D309

Do not retain a new-contact entry's proposed membership when the selected view
changes. Its default membership follows the current view even while entry is
in progress: a named logbook supplies its tag, while All contacts and Untagged
supply none. Preserve other entered values. This does not retag saved contacts
or change an existing contact through its editor. The
[product contract](product.md) owns the entry-default rule.

### Decision D310

#### D310

After successfully saving a new manual QSO, clear the contacted station's
callsign, location, operator and references, both signal reports, and the
contact comment. Retain primary and receive frequency/band pairs and mode.
Use active own-station, equipment, operator and transmit-power settings for
the next contact. Return both timestamps to live operation under D87 and use
the current view's default membership under D309. Failed saves preserve the
entry. The [product contract](product.md#manual-logging-and-rig-integration)
owns this behavior.

### Decision D311

#### D311

Enter saves the QSO from single-line contact-entry fields; Tab moves between
fields. Keep normal dropdown and multiline-text editing behavior. Keyboard
saving uses the same validation and submission path as the visible Save
action. The [desktop UI contract](desktop-ui.md#operation-feedback) owns the
interaction; this does not introduce a global shortcut in unrelated editors.

### Decision D312

#### D312

Allow clearing a locked end-time field to record an unknown end time. Keep
it blank while locked and save an absent end timestamp; unlocking resumes
live updating from the current time. Start time remains required. Successful
saving returns both fields to live operation under D87. The
[product contract](product.md#manual-logging-and-rig-integration) owns this
timestamp interaction.

### Decision D313

#### D313

Unfinished manual QSOs follow active operating settings while preserving
explicit contact-specific corrections. This applies to own-station,
equipment, operator and transmit-power values. Contact-specific corrections
do not change active defaults, and changes to defaults do not rewrite saved
QSOs. The [product contract](product.md#manual-logging-and-rig-integration)
owns this interaction.

### Decision D314

#### D314

Confirm individual QSO deletion with the contacted callsign, UTC start time,
and band/mode visible. State that deletion affects the contact across all
logbooks. Use Delete QSO and Cancel, with initial focus on Cancel. This is
contact deletion, not removal of a tag. The
[desktop UI contract](desktop-ui.md#shared-component-contract) owns the
confirmation interaction.

### Decision D315

#### D315

Make the submitted contact entry read-only while saving locally, with a
Saving indicator and duplicate submission disabled. The rest of the workbench
remains responsive. Success performs the agreed reset; failure restores
editing of the preserved entry and displays the error. The
[desktop UI contract](desktop-ui.md#operation-feedback) owns this interaction.

### Decision D316

#### D316

Include an optional contact-name field in quick QSO entry, with later
directory names using the existing separate-source fallback rules. Keep
the contacted operator callsign optional and secondary; knowing whether it
differs from the contacted station is not a routine logging prerequisite.
Group our station, operator, power and equipment as operating settings in
the full contact editor. Provide an explicit return to quick entry that
updates the form inputs without saving a QSO, distinct from contact saving
and cancelling modal edits. The
[desktop UI contract](desktop-ui.md#contact-entry-and-full-editing) owns these
interaction requirements.

### Decision D317

#### D317

Use adjacent contact details rather than expandable details. Worked Before
must be one selectable tab in the same adjacent area, keeping two upper
panes rather than adding a third section. Quick QSO entry stays visible on
the left, with contact history below. Tab navigation preserves unfinished
contact information without saving the QSO. This refines D288's upper-right
area and D316's non-saving return from details. Exact tabs and any future
dockable presentation remain design work; this does not select docking for
MVP. The [desktop UI contract](desktop-ui.md#workbench-composition) owns the
arrangement.

### Decision D318

#### D318

Use combined UTC date-and-time controls for contact Start and End. Each
control retains its independent live/locked behavior, with an absent locked
End still permitted. Display timestamps in contact history and Worked Before
as combined date-and-time columns and sort by the complete timestamp. Do not
provide separate default date and time-of-day columns. A date-only
calculated/custom column is a distinct presentation capability, not a reason
to split the timestamp. The
[desktop UI contract](desktop-ui.md#contact-entry-and-full-editing) owns entry
presentation and its [column rules](desktop-ui.md#columns-and-sorting) own
table presentation.

### Decision D328

#### D328

Worked Before defaults to UTC start, Callsign, Band, Mode, Contacted grid,
and Contacted activity references, with a compact Today indicator and
pending/conflict markers. Own-station information and signal reports remain
available through column configuration. Preserve one row per QSO and the
independent table preferences. The
[desktop UI contract](desktop-ui.md#columns-and-sorting) owns these defaults.

### Decision D329

#### D329

Expose history filters without requiring a dialog, using an always-visible
query bar with Gmail/GitHub-style named tokens and support for more complex
expressions. The
[desktop UI contract](desktop-ui.md#history-filter-entry) owns this direction;
the concrete expression grammar and editing behavior remain to be specified.

### Decision D330

#### D330

Default unqualified history-query text to worked-callsign matching. Provide
explicit `name:` and `comment:` tokens for contact-name and comment searches.
Unqualified text uses prefix matching against recorded worked callsigns,
without automatic base-call or portable-variant expansion. The
[desktop UI contract](desktop-ui.md#history-filter-entry) owns this behavior.

### Decision D331

#### D331

Valid history-query expressions automatically update results after a
debounce, without requiring Enter to apply them. Invalid or incomplete
expressions retain the applied query and its results while inline validation
identifies the issue. The
[desktop UI contract](desktop-ui.md#history-filter-entry) owns this behavior.

### Decision D332

#### D332

Use case-insensitive substring matching for `name:` and `comment:` history
tokens. Quoted values preserve multiword phrases. For example, `name:alice`
matches Alice Smith and `comment:"first contact"` matches My first contact
on 40 m. The
[desktop UI contract](desktop-ui.md#history-filter-entry) owns these rules.

### Decision D333

#### D333

Adjacent history-query predicates mean AND, even when they refer to the same
field. OR must be explicit; parentheses group alternatives and a leading
minus negates a predicate. Predicate combination is independent of field. The
[desktop UI contract](desktop-ui.md#history-filter-entry) owns this behavior.

### Decision D334

#### D334

Use `field:value` tokens with optional comparison prefixes inside the value.
For example, `qso_start:>2026-10-27T18:00:00Z` means strictly after that
timestamp; `qso_start:>=2026-10-27T18:00:00Z` includes it. Keep named tokens
without prefixes, such as `mode:CW` and `name:alice`, under their matching
rules. Do not require users to write SQL-like comparison expressions. The
[desktop UI contract](desktop-ui.md#history-filter-entry) owns this syntax.

### Decision D335

#### D335

History-query precedence is NOT, then AND (including adjacency), then OR.
Parentheses override precedence. The `NOT` keyword and leading minus express
negation. The
[desktop UI contract](desktop-ui.md#history-filter-entry) owns this grammar.

### Decision D336

#### D336

Date-only values in timestamp query tokens denote complete UTC days.
`qso_start:2026-10-27` matches starts anywhere on that day;
`qso_start:>=2026-10-27` includes that day and later times;
`qso_start:>2026-10-27` begins after that day ends. Full timestamps retain
exact comparisons. The
[desktop UI contract](desktop-ui.md#history-filter-entry) owns this behavior.

### Decision D337

#### D337

Include `has:<field>` for optional-field presence tests. Ordinary negation
finds missing values. For example, `has:comment` finds contacts with comments
and `-has:qso_end` finds contacts without a recorded end time. The
[desktop UI contract](desktop-ui.md#history-filter-entry) owns this behavior.

### Decision D338

#### D338

Provide automatic context-aware history-query suggestions with keyboard
selection and dismissal. Suggest field names, supported comparison prefixes
and appropriate values, with brief field descriptions. Suggestions assist
entry without requiring users to select them. The
[desktop UI contract](desktop-ui.md#history-filter-entry) owns this behavior.

### Decision D339

#### D339

The `tag:` query token matches exact tag names, including all tags with an
identical name. Return each matching QSO once. Disambiguating individual
same-name tags through query tokens is deferred. The
[desktop UI contract](desktop-ui.md#history-filter-entry) owns this behavior.

### Decision D340

#### D340

Use case-insensitive whole-value matching for `=` on text fields. For
example, `name:=alice` matches Alice but not Alice Smith. Apply this prefix
consistently to callsigns and other text fields while retaining each token's
default matching when no prefix is supplied. The
[desktop UI contract](desktop-ui.md#history-filter-entry) owns this behavior.

### Decision D341

#### D341

Default query frequency values to MHz and transmit-power values to watts.
Accept explicit compatible unit suffixes, including Hz/kHz/MHz for frequency
and mW/W for power. Suggestions show each field's default unit. The
[desktop UI contract](desktop-ui.md#history-filter-entry) owns query units;
saved native values and their resolution remain unchanged.

### Decision D342

#### D342

Use dotted names for nested query fields, while retaining established short
tokens. Examples include `own.call`, `contact.call`, `own.grid`,
`contact.grid`, `receive.frequency`, and `lotw.confirmed`. Naming does not
implicitly determine matching behavior. The
[desktop UI contract](desktop-ui.md#history-filter-entry) owns this convention.

### Decision D343

#### D343

Scoped collection-member query groups are unnecessary for this pass and are
excluded from the current query scope. This does not commit to adding them
later. The
[desktop UI contract](desktop-ui.md#history-filter-entry) owns this boundary.

### Decision D344

#### D344

Query ordinary name/location fields as effective displayed values, with
explicit `recorded.` variants for saved QSO facts. Preserve the existing
fallback order, eligibility and whole-location selection rules. Station
callsigns and operator information remain recorded facts. Primary radio
fields query the primary pair; receive fields use the native effective
receive pair, while recorded receive variants inspect the optional override.
`has:` follows the target field's binding. Power, reports, comments and
equipment query saved QSO values. The
[query contract](history.md#field-source-bindings--agreed-d344) owns this
mapping.

### Decision D345

#### D345

For mode queries, `mode:SSB` and `mode:=SSB` have the same meaning. An
explicit equality prefix does not select a different mode interpretation.
The [query contract](history.md) owns the mapping; D346 defines the
mode-family matching behavior.

### Decision D346

#### D346

`mode:SSB` includes USB and LSB contacts, while `mode:USB` narrows to USB.
Use data-driven mode-family relationships without requiring users to
navigate parent/submode projections. An explicit `=` retains the same
meaning under D345. Querying does not change saved mode identity. The
[query contract](history.md#mode-matching--agreed-d345d346) owns this
behavior.

### Decision D347

#### D347

Grid query tokens mean known to be within the specified grid: include
explicit points inside it and grid areas equal to or wholly contained within
it. Broader or merely overlapping areas do not qualify; a grid area's center
does not establish an exact position. An explicit `=` retains the same
geographic meaning. Ordinary fields use the effective location; `recorded.`
variants apply the same predicate to recorded location information. The
[query contract](history.md#grid-matching--agreed-d347) owns this rule.

### Decision D348

#### D348

Use `own.activity:` and `contact.activity:` to search activity participation.
Use `own.reference:` and `contact.reference:` with group-qualified values,
such as `POTA/US-0001`, to search particular references. Group names resolve
through ordinary synchronized definitions without special behavior for POTA.
Activity queries also cover groups without individual reference identifiers.
No collection-member syntax is required. The
[query contract](history.md#activity-and-reference-searches--agreed-d348)
owns this mapping.

### Decision D349

#### D349

Use Boolean `lotw.uploaded`, `lotw.confirmed`, `qrz.uploaded`, and
`qrz.confirmed` predicates. Uploaded means positive evidence covers current
service-relevant values; confirmed means positive confirmation applies to the
current QSO. `false` is ordinary negation, including missing or inapplicable
evidence, not an assertion of provider rejection or explicit unconfirmed
status. Upload and confirmation remain independent; preserve historical
evidence and existing imported-positive/applicability rules. The
[query contract](history.md#upload-and-confirmation-searches--agreed-d349)
owns this mapping.

### Decision D350

#### D350

Apply service-specific contexts to upload/confirmation queries. QRZ uses the
QSO's applicable book, identified by its saved station callsign/date and
existing association. Do not aggregate unrelated books or replace historical
book context with the currently open book. Preserve evidence and report
inconsistent book metadata/associations rather than inventing a migration.
LoTW evidence applies to the QSO's saved station identity; certificates are
signing inputs, not separate remote logbooks, and changing a download login
does not invalidate retained evidence. Accept applicable imported positives
without requiring destination provenance. Credential changes, upload pauses,
disabled automatic uploads and failed downloads do not invalidate existing
positives. Query status is distinct from upload-job eligibility. The
[query contract](history.md#service-context--agreed-d350) owns these rules.

### Decision D351

#### D351

Preserve one shared client-local history-query state across views and
application restarts. Retain unfinished/invalid edited text and the last
valid applied query. Restore the edited text with inline diagnostics and a
clear indication of the applied query, evaluating results against the current
view and saved data. Existing view-change selection clearing remains in
force. The [query contract](history.md#shared-query-state--agreed-d351)
owns this behavior.

### Decision D352

#### D352

Use a Gmail-style searchable checkbox picker for individual and selected-contact
logbook membership editing. In the contact editor, keep a compact Assigned
logbooks summary with Edit opening a bounded picker; do not permanently list the
account's full catalog. Its changes remain pending until the editor's existing
Save changes action, with no additional membership Apply/Done step. In history,
show Edit assigned logbooks only when QSOs are selected, without an unlabeled
numeric suffix; retain the existing selection count and identify the selected
scope within the picker. Its Apply action changes that selected set, including
hidden selected contacts. Checked, mixed and blank represent all, some and none;
untouched mixed membership stays unchanged, checking requests inclusion on all
targets and clearing requests removal from all. Preserve existing Save/Cancel,
whole-QSO reconciliation, stale-save and bulk-operation contracts. The
[desktop UI contract](desktop-ui.md#editing-assigned-logbooks-d352) owns the
interaction and scalability requirements.

### Decision D353

#### D353

Use PostgreSQL-backed browser sessions with an opaque random session identifier
in a secure, HTTP-only cookie. Keep account binding and authenticating login
identity in server-side session state. D366 selects expiration, D367 separates
authoritative login validity, and D368 selects `axum_session` with its PostgreSQL
adapter. D354 defines identity-unlink invalidation. Browser sessions remain separate
from independently revocable desktop-client credentials. See
[browser sessions](accounts.md#browser-sessions-d353).

### Decision D354

#### D354

When a login identity is unlinked, invalidate browser sessions authenticated
through that identity. Preserve sessions authenticated through identities still
linked to the account and preserve enrolled desktop clients' independently
revocable account-bound credentials. Retain D232's requirement for recent
authentication through an identity that will remain linked. See
[browser sessions](accounts.md#browser-sessions-d353).

### Decision D355

#### D355

For login-identity linking and unlinking, perform a standard OIDC round trip with
`max_age=300` whenever recent authentication to the existing account is required.
The provider determines whether existing authentication satisfies the request
and handles any necessary reauthentication. Use the OIDC library for standard
identity-token validation and verify the returned identity's account binding.
Clacks does not independently require or age-check `auth_time`; honoring
`max_age` is the provider's responsibility. Linking authenticates an
already-linked identity separately from the new identity; unlinking uses an identity
that will remain linked. Do not implement custom freshness bookkeeping, reuse
cached proofs to bypass the round trip, or add provider-specific workarounds.
Provider interoperability remains an integration obligation. See
[login-identity management](accounts.md#self-service-login-identity-management-d232).

### Decision D356

#### D356

Default browser sessions to 30 days between authentications, with no separate
inactivity timeout; allow deployment administrators to configure these limits.
The expiration policy is superseded by D366's 14-day sliding window; the earlier
30-day absolute limit is not a continuing mandatory cap.
Ordinary activity does not extend the overall authentication lifetime. Do not
introduce idle-warning dialogs under the default policy. Explicit logout
invalidates the server-side session immediately. Preserve D354 invalidation on
identity unlinking, D355's five-minute authentication requirement for identity
changes, and independently revocable desktop enrollment. See
[browser sessions](accounts.md#browser-sessions-d353).

### Decision D357

#### D357

Use server maintenance commands to grant or revoke the server-administrator role
for an existing Clacks account ID, with deployment-controlled database access and
operator logging. Initial administrator setup follows normal sign-in/account
provisioning, then explicit role assignment. Store assignments in the database
rather than a deployment-configured administrator allowlist. Ordinary account
provisioning remains independent of role assignment; no web role-management
interface or automatic provider-claim mapping is required. See
[administrator-role assignment](accounts.md#server-administrator-role-assignment-d192).

### Decision D358

#### D358

Use a server maintenance command for administrator-assisted recovery. Identify
the existing Clacks account by its internal ID and add a newly verified OIDC
issuer/subject identity, refusing identities bound to another account. Require
documented out-of-band operator verification of account ownership and the
replacement identity; do not infer ownership from email or merge accounts.
Preserve account data, service credentials and enrolled clients, and log the
recovery for operators. No web recovery-request system is required for MVP. See
[account recovery](accounts.md#recovery-without-an-accessible-login-identity-d233).

### Decision D359

#### D359

Administrator-assisted recovery addresses lost access to all linked login
identities without assuming compromise. Add the newly verified identity while
preserving existing bindings, browser sessions and enrolled clients. Any
compromise-related revocation is a separate explicit action. Verify the
replacement identity out of band before normal Clacks sign-in, which would
otherwise provision a separate account. Refuse an identity already bound to
another account rather than transferring it or merging accounts. Explain this
distinction in the recovery procedure. See
[account recovery](accounts.md#recovery-without-an-accessible-login-identity-d233).

### Decision D360

#### D360

Use SQLx for server PostgreSQL persistence as well as client SQLite persistence.
Keep fixed queries in separate `.sql` files through `query_file!` or
`query_file_as!`, with compile-time checking and committed offline query metadata.
Reuse tooling and conventions without coupling the independent server and client
schemas, migrations or queries. See
[server persistence access](accounts.md#server-persistence-access-d360).

### Decision D361

#### D361

Use release-managed server migrations designed for production software as a
service (SaaS). Provide a migration command in the immutable release artifact for
deployment automation, separate from normal application startup, using a
schema-changing role unavailable to normal server processes. Use SQLx's existing
migration machinery. Stage compatible schema additions, application deployment,
substantial resumable/bounded data conversion and later removal of obsolete
schema through expand–migrate–contract. Preserve compatibility during supported
old/new application overlap, including writes and background work. Normally
roll back the application to a compatible version against current database
contents rather than automatically reversing migrations or restoring old data.
Plan lock/operation bounds and observe migration outcomes. #11 establishes this
contract without requiring an orchestrator, multiple replicas or a generic
backfill framework. Self-hosting may stop the app, run the same migration command
and restart. See [server migrations](accounts.md#release-managed-server-migrations-d361).

### Decision D362

#### D362

Use deployment-managed public HTTPS termination and certificates at a reverse
proxy or load balancer, with Axum on a restricted internal endpoint and upstream
transport protected appropriately for the deployment network. Configure the
public base URL explicitly for OIDC callbacks instead of deriving it from
arbitrary host headers. Trust forwarded client information only through an
explicitly trusted proxy configuration. Serve web and application endpoints
under one public origin. No particular ingress product or built-in public
HTTPS/certificate management is required for MVP. See
[HTTPS boundary](accounts.md#public-https-boundary-d362).

### Decision D363

#### D363

Support OpenID Connect Dynamic Client Registration (DCR) for configured providers
in MVP. Clacks registers as an OIDC
client for its deployment, distinct from user-account provisioning, identity
linking and desktop enrollment. Use standard provider discovery/registration and
protect returned registration credentials as deployment secrets. Concrete
registration lifecycle and provisioning mechanics remain engineering work. See
[dynamic registration](accounts.md#oidc-dynamic-client-registration-d363).

### Decision D364

#### D364

Support explicit preconfigured-client and dynamic-registration modes per OIDC
provider. Run DCR through an automatable deployment-setup operation, separately
from SQL schema migrations, using the configured public callback URL. Persist
and protect returned registration credentials for durable reuse across restarts,
application replicas and releases. Report invalid or expired credentials clearly
rather than silently creating another registration after authentication failures.
See [dynamic registration](accounts.md#oidc-dynamic-client-registration-d363).

### Decision D365

#### D365

Use `openidconnect` with asynchronous `reqwest` and Rustls for server OIDC
integration, including discovery, authorization-code authentication, standard
maximum-authentication-age requests and response validation, and DCR. Clacks
supplies account behavior and Axum integration rather than a protocol
implementation. Follow the library's HTTP-client security guidance, including
disabling automatic redirects. D368 selects session middleware; concrete
integration remains specification work. See
[OIDC protocol library](accounts.md#oidc-protocol-library-d365).

### Decision D366

#### D366

Use a deployment-configurable 14-day sliding browser-session expiration window
in place of D356's earlier 30-day absolute expiration policy. The sliding
duration must be smaller than that previously selected absolute duration; this
does not require retaining an independent absolute deadline. Activity can extend
the sliding deadline. Do not introduce idle-warning dialogs or change the
standard OIDC recent-authentication flow for identity changes. D368 selects the
middleware, with authoritative login validity separated under D367. See
[browser sessions](accounts.md#browser-sessions-d353).

### Decision D367

#### D367

Separate authoritative login validity from general middleware session data.
Successful OIDC sign-in creates a PostgreSQL login-session record tied to the
account and authenticating linked identity; middleware data holds a reference.
Every authenticated request checks the live authoritative record. Logout
invalidates it, and unlinking invalidates login sessions established through the
removed identity. Ordinary middleware saves cannot create or reactivate those
records. Apply D366's 14-day sliding expiration to authoritative login validity;
middleware retention/refresh cannot extend authentication independently. Preserve
other identities' sessions and desktop enrollment, and retain library cookie
handling and general session machinery. D368 selects the middleware; concrete
schema and transaction integration remain specification work. See
[browser sessions](accounts.md#browser-sessions-d353).

### Decision D368

#### D368

Use `axum_session` with `axum_session_sqlx` for PostgreSQL-backed browser
sessions, retaining native cookie handling and general session machinery.
Apply D367's separation: middleware stores a reference, while every authenticated
request checks authoritative PostgreSQL login validity. Middleware cache or
stale saves cannot restore authorization. Initialize middleware tables through
D361 release-managed migrations rather than schema creation by the runtime
database role. The evaluated dependency baseline is `axum_session` 0.22.0 and
`axum_session_sqlx` 0.12.0 with SQLx 0.9; concrete schema and transaction
integration remain specification work. See
[browser sessions](accounts.md#browser-sessions-d353).

### Decision D369

#### D369

Use account-row locking in short PostgreSQL transactions for changes to an
existing account's identity bindings, including administrator-assisted recovery.
After locking, recheck the applicable identity and authentication requirements,
including preservation of a remaining login identity. Commit binding changes
and affected authoritative login-session invalidation atomically. Keep provider
authentication and other network requests outside the transaction. Retain
issuer/subject uniqueness across accounts through database constraints; the
account lock does not replace that enforcement. This does not serialize ordinary
account requests or QSO writes. See
[identity-management transactions](accounts.md#identity-management-transactions-d369).

### Decision D370

#### D370

Use a temporary loopback listener on an operating-system-assigned port for the
MVP desktop browser-authorization callback. The initiating desktop process
receives completion directly, without an application-specific URL-handler
registration or another computer. Keep the listener limited to the authorization
attempt. D193's browser sign-in/client approval and post-MVP device-authorization
fallback remain unchanged. This settles callback transport; authorization-server
library integration and ongoing per-client credential format, storage and renewal
remain engineering decisions. See
[browser-callback authorization](accounts.md#browser-callback-authorization-d193).

### Decision D371

#### D371

Use short-lived access tokens with a persistent per-client refresh credential
for desktop authorization. An authorized client can obtain a fresh access token
without another browser sign-in after an indefinite offline period; inactivity
alone does not expire its refresh authorization. Keep desktop credentials separate
from browser sessions and external-provider credentials. Revocation blocks refresh
and authorized access without waiting for access-token expiration, preserving
other clients and local work. D374 selects opaque access tokens. D378 selects
persistent refresh credentials bound through Demonstrating Proof of Possession
(DPoP) and binds access tokens as well, superseding the earlier rotation and
retry-grace choices.
Refresh-token representation, protected storage, library integration and exact
access-token lifetimes remain engineering choices. See
[desktop credentials](accounts.md#desktop-access-and-refresh-credentials-d371).

### Decision D372

#### D372

Superseded by D378. Ordinary refresh retains the persistent key-bound refresh
credential rather than rotating it. See
[desktop credentials](accounts.md#desktop-access-and-refresh-credentials-d371).

### Decision D373

#### D373

The rotation-specific retry grace and its deadline are superseded by D378.
Retry an interrupted refresh with the same key-bound credential and a fresh
proof. Preserving local data/comparison evidence and distinguishing credential
recovery from explicit actor revocation remain required. See
[desktop credentials](accounts.md#desktop-access-and-refresh-credentials-d371).

### Decision D374

#### D374

Use opaque access tokens backed by server-side authorization records for desktop
access. Resolve the credential and current enrolled-client authorization through
PostgreSQL, retaining immediate individual-client revocation rather than relying
on token expiration. Existing stream enforcement and admission checks remain
required. This avoids access-token signing-key management in the single-server
application architecture; it does not change provider-issued identity tokens,
indefinite offline eligibility or rollback limitations. D378 adds DPoP binding. Exact
storage schema and refresh-token representation remain engineering choices. See
[desktop credentials](accounts.md#desktop-access-and-refresh-credentials-d371).

### Decision D375

#### D375

Superseded by D379's Huskarl client and oauth-as embedded authorization server
selection for D378's full DPoP requirement.
Preserve required PKCE, the standard loopback-IP/variable-port redirect policy,
SQLx persistence and the existing external-provider `openidconnect` integration.
The authorization server is embedded in the Clacks application; a separate
authorization-server service is excluded. Exact compatible dependency selection
and integration belong to the concrete specification and plan. See
[desktop credentials](accounts.md#desktop-access-and-refresh-credentials-d371).

### Decision D376

#### D376

Use `keyring-core` with native macOS and Windows backends for desktop credential
storage: `apple-native-keyring-store` on macOS and
`windows-native-keyring-store` on Windows. D391 supersedes the original macOS
`keychain` feature selection with Data Protection Keychain. Deliver both integrations during MVP;
macOS is the primary development platform and cannot defer its credential path
until the post-MVP macOS release. Keep enrollment secrets in protected OS storage
and only non-secret references in SQLite. Linux desktop release support remains
post-MVP; headless server secret storage is a separate concern. Exact dependency
pins, entry naming and integration mechanics belong to the specification and
plan. See [desktop credentials](accounts.md#desktop-access-and-refresh-credentials-d371).

### Decision D377

#### D377

The replacement-credential retry policy is superseded by D378's persistent
key-bound refresh credential. The prohibition on retaining recoverable replacement
credentials or replayable successful responses for retry recovery remains. See
[desktop credentials](accounts.md#desktop-access-and-refresh-credentials-d371).

### Decision D378

#### D378

Use persistent refresh credentials bound through Demonstrating Proof of
Possession (DPoP), and bind access tokens as well. Ordinary refresh issues a
fresh short-lived opaque access token while retaining the same key-bound refresh
credential. Require proof of the bound private key for code exchange, refresh
and access-token use, with standard request binding, freshness and replay checks.
Share replay protection across application instances.

Retry interrupted refreshes and lost responses with the same refresh credential
and a fresh proof. Supersede D372's rotation, D373's rotation-retry grace deadline
and D377's replacement-credential retry policy. Do not retain recoverable
replacement credentials or cached successful token responses for retry recovery.

Preserve indefinite offline eligibility, immediate individual-client revocation,
account isolation and local pending work/comparison evidence. Credential recovery
alone does not establish explicit actor revocation; D145/D152 still govern
revoked-actor reauthorization. DPoP does not protect a client when both its key
and credentials are compromised. D379 selects Huskarl with oauth-as and the
loopback-port behavior now supplied by oauth-as 1.0.1.
See [desktop credentials](accounts.md#desktop-access-and-refresh-credentials-d371).

### Decision D379

#### D379

Use Huskarl for desktop authorization and oauth-as for the authorization server
embedded in Clacks, superseding D375. Preserve D370's temporary variable-port
loopback callback, D374's opaque access tokens, D376's native credential stores
and D378's persistent DPoP-bound refresh credentials and bound access tokens.
External-provider sign-in remains on the existing `openidconnect` integration.

Accept the required oauth-as loopback-port correction as a pinned, reviewed patch
until a compatible upstream release is available. Permit the callback port to
vary without relaxing the remaining redirect matching, and require the exact
granted redirect URI at code redemption. Track upstream issue #26; acceptance
does not assume upstream delivery or authorize an unrestricted fork.
The published oauth-as 1.0.1 release now supplies this correction; use it in place
of the local vendored patch.

Use standard Huskarl client facilities. Clacks owns protected credential
persistence, relational SQLx issuer storage, current account/client authorization,
transaction boundaries and direct PostgreSQL resource authorization. Huskarl's
complete opaque-token validator requires HTTP introspection; its lower-level
proof validator leaves Clacks responsible for request/key/token binding checks
and shared replay exclusion. Preserve immediate revocation at admission and on
existing streams. Do not adopt the unpublished reference PostgreSQL adapter.

Accept the evaluated dependency cost: a larger graph, unused crypto algorithms,
parallel RustCrypto generations and release-candidate crypto dependencies.
Keep accepted proof algorithms explicit; compiled support does not widen policy.
Concrete dependency pins, storage security and transaction integration belong to
the specification and plan. See
[library integration costs](accounts.md#selected-libraries-and-integration-costs-d379).

### Decision D380

#### D380

Hash ordinary authorization-code and access/refresh credential lookup values
where oauth-as's existing interface supports it. Protect the recoverable issued
credential references in consumed/replayed authorization-code records with the
per-account encryption/deployment-wrapping-key arrangement from D236. These
short-lived receipts are not refresh-response recovery caches. The desktop keeps
its usable refresh credential and signing key in D376's native credential store.

Preserve oauth-as's existing code-replay rejection and grant-revocation behavior,
including its consequence for a repeated initial exchange after a lost response.
Do not add a grant-reference patch, alternate token-handle convention or modified
replay policy to obtain fully hash-only persistence. D379's bounded loopback-port
correction is supplied by the published oauth-as 1.0.1 release.

Apply normal secret-handling, infrastructure and incident-remediation practices.
Encryption limits a database-only disclosure; it does not protect credentials
from an authorized running server or database-plus-key compromise. Retention
cleanup does not erase older backups. Evaluate additional controls against
realistic exposure and remediation costs rather than inventing stronger security
requirements. See [credential storage](accounts.md#credential-storage-d380).

### Decision D381

#### D381

Include deployment wrapping-key rotation and individual account encryption-key
rotation in MVP through the existing `clacks-admin` command. Wrapping-key
rotation rewraps account keys; account-key rotation re-encrypts the protected
values belonging to that account. Deployment automation may invoke these
operations; no internal automatic rotation schedule is required. Retain the key
versions needed by supported backups and restores. Concrete commands,
transaction boundaries and interruption recovery belong to issue #13's design.
See [encryption-key lifecycle](accounts.md#encryption-key-lifecycle-d236d381).

### Decision D382

#### D382

In synchronization conflict review, differing local/shared scalar fields start
without a selected side. Require an explicit choice for each differing field before
saving the resolution. Provide **Use all local** and **Use all shared** shortcuts
to populate those choices; selecting a shortcut does not itself save. Fields
whose local and shared values agree require no choice. Preserve the existing
whole-record review trigger and three-way change attribution; this does not
introduce automatic field or tag merging.

D383 defines editable set selection for logbook membership rather than an
exclusive local/shared choice for that field. D384 also permits entering a new
scalar value directly instead of choosing either compared value.

### Decision D383

#### D383

Present account-wide **Unsynchronized changes** as an exclusive review in a
separate window. While it is open, suspend other desktop interactions and
preserve the workbench's selected view, arrangement and unfinished QSO for when
review closes. The presence of pending edits or conflicts alone does not block
ordinary logging or other workbench actions.
Separate **Conflicts** from **Pending sync** in independently collapsible
sections. Retain record-type filtering and omit a logbook selector: logbooks are
QSO memberships subject to conflict resolution, not synchronization buckets.
Ordinary pending changes need no resolution.

For a membership conflict, show local and shared sets plus an editable resolved
membership using ordinary checked/unchecked boxes. Field-level **Use local**
and **Use shared** shortcuts replace the checked set without saving. Individual
changes can combine either side's memberships or leave the QSO untagged. The
checked set always defines the resolved value; omit indeterminate checkbox
states and a separate **Use selection** action. **Save resolution** saves the
chosen membership with the rest of the resolved record. Whole-record detection,
three-way attribution and conditional shared acceptance remain unchanged; this
does not authorize automatic tag merging.

### Decision D384

#### D384

Allow entering new values directly during conflict resolution, rather than
limiting scalar fields to the local or shared value. Keep compared values and
their change attribution available, with local/shared selection shortcuts.
Entering a new resolved value satisfies that field's explicit-choice requirement.
Apply the existing field and whole-record validation rules; invalid values do
not become saved records. **Save resolution** saves the chosen record locally,
subject to the existing stale-save protection and conditional shared acceptance.
Editing a resolved value does not itself save or publish it.

### Decision D385

#### D385

If a resolution's comparison becomes outdated while the user is editing,
preserve the draft and displayed comparison, indicate that newer values are
available, and require a refreshed comparison before saving. Refresh retains
entered values for reconsideration; it neither discards them nor silently
approves choices against unseen changes. Existing transactional stale-save
checks remain authoritative, including when a change notification has not yet
reached the UI. A locally saved resolution still undergoes conditional shared
acceptance during synchronization.

### Decision D386

#### D386

Preserve each conflict's unfinished resolution draft when switching between
records in the Unsynchronized changes window. Returning to a record restores
its draft, with D385's refresh requirement if its comparison has changed.
Switching records does not save a resolution. Closing the window discards its
unsaved resolution drafts under the existing Cancel behavior; completed saves
remain saved. This does not require persisting unfinished resolution forms
across application restarts.

### Decision D387

#### D387

Initialize resolved QSO logbook membership from the local working set. The
checked memberships directly define the resolved value from the outset, without
requiring an extra confirmation for that field. **Use local** restores that
set, **Use shared** replaces it with the compared shared set, and individual
checkbox changes modify the result. Do not initialize from the intersection or
union of the compared sets. **Save resolution** remains the explicit save for
the whole chosen record; other differing scalar fields still require choices.

### Decision D388

#### D388

Run routine synchronization-metadata collection automatically in the server,
rather than requiring an operator to invoke routine cleanup. Existing retention
and recovery rules remain authoritative: automatic execution does not expire
offline clients, advance acknowledgments, or make protected comparison, deletion
or service evidence disposable. Scheduling, batch sizes and resource limits
remain engineering choices; this decision does not require a new operator
command or background-job framework.

### Decision D389

#### D389

Bound the incremental synchronization journal by an entry-count limit per
account, rather than a time-based retention window. A client whose checkpoint
predates the retained journal uses the existing fuller-state reconciliation
path, without expiration or re-enrollment. This journal limit does not replace
D155's ordering-evidence retention boundary or permit collection of required
comparison, deletion or service evidence. The initial limit and maintenance
batch sizes remain engineering choices.

### Decision D390

#### D390

Start with atomic baseline installation, including the required metadata
conversion and guarded account update in one transaction. Keep subsequent
physical cleanup separate. Do not introduce incremental installation based on
an assumed performance problem.

Provide production observability that distinguishes installation duration,
account-lock acquisition wait, application-observed lock-holding duration and
the amount of metadata work performed. Report committed conversion and cleanup
effects truthfully, with bounded metric dimensions and account-specific context
in traces/structured logs. Use measurements to determine whether later
optimization is warranted; no lock-duration estimate or performance threshold
has been established by this decision.

### Decision D391

#### D391

Follow Apple's modern application-secret storage guidance on macOS: use the
Data Protection Keychain through `apple-native-keyring-store`'s `protected`
backend for enrollment and directory secrets. Do not add migration machinery
or a legacy file-based Keychain fallback. Use local app-specific storage with
default When Unlocked access, without adding user-presence requirements or
iCloud synchronization. This supersedes D376's macOS backend feature selection;
its Windows backend and other storage boundaries remain in force. Native macOS
credential acceptance requires the correctly signed/profile-authorized host
described in the [desktop guide](../lib/clacks-desktop/README.md#native-credential-integration).

### Decision D392

#### D392

Advance release-mode desktop builds and appropriate installation artifacts ahead
of complete MVP workflow readiness. Deliver Windows first, then macOS as a
separate milestone following the Windows installer. Windows executable/installer
signing is not required for the early milestone, but adding it must extend the
same release pipeline without redesign. macOS retains D391's inherent signing
and provisioning requirements for protected credentials. Early artifacts package
available functionality rather than waiting for remaining MVP capabilities.

This refines D162's release timing: macOS is now explicitly scheduled after the
early Windows installer, with both platforms included in final release-readiness
validation; Linux desktop releases remain deferred. Complete integrated MVP
validation and final distribution-trust acceptance remain separate from early
packaging. Package formats and tools remain engineering choices; no separate
updater or Microsoft Store distribution is added.

### Decision D393

#### D393

Technosorcery, LLC is the owning entity for all of Clacks Log. Use its exact
name for first-party publisher/company metadata and the project ownership
notice. The application remains Clacks Log; preserve stable application,
package-upgrade, workspace and credential identities. Standard Elastic License
2.0 remains the application license, and third-party ownership, licenses and
notices remain intact.

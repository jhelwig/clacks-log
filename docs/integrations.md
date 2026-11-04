# External logbook services

Logbook of The World (LoTW) and QRZ are minimum viable product (MVP) integrations. The server executes publication and download jobs so connected desktop applications need not remain running. eQSL remains a later integration candidate.

## Operations and distinct states

The intended service workflows cover publication, recording service acceptance, retrieving confirmations, reconciling returned contact information, publishing corrections, and exposing incoming records that have no local match.

Publication acceptance is not remote confirmation. Received confirmation can exist regardless of whether we have published our own copy. Avoid one ambiguous confirmation-status field covering upload, acceptance, and a matched contact confirmation (QSL).

Downloads of our submission/acceptance history and downloads of counterpart submissions or confirmations are distinct data sources. Their meaning and application programming interface (API) availability must be defined per service, not conflated under download remote data.

## Accepted model foundation

[Service evidence and external-record associations](service-evidence.md) defines the evidence records, destinations/keys, submission projections, confirmation applicability, observation equality, recovery dependencies, transaction groups and MVP retention. [Service research](../references/service-state-model.md) retains provider findings. Source-specific import flag/date recognition and live adapter behavior remain downstream work; matching and association policies are specified below.

The production evidence and association components implement these contracts.
Imports, synchronization and provider jobs integrate and extend those components.
Persistence integration must enforce the required atomicity and uniqueness in
the actual stores.

## Synchronization ownership (D248)

Use the account's single synchronization scope for contact-specific publication attempts/outcomes, sent-value context, associated observations, directory snapshots and per-contact processing decisions, alongside download coordination/progress, unresolved observations and durable rejection dispositions. D251 allows at most one local QSO associated with an external record per account/service destination. Ambiguous candidates require explicit selection of one QSO; do not fan out an observation to inferred copies. A competing outgoing QSO must require resolution rather than create a second association or silently replace the associated external record. Deleting a tag does not remove evidence still needed for surviving contacts, deletion suppression or duplicate-safe processing. Preserve atomic processing-result/contact-edit effects and durable incoming-work tracking. Retain distinct semantic evidence without age-based expiry in MVP; the [evidence retention contract](service-evidence.md#12-retention-and-collection) distinguishes that evidence from collectable synchronization detail.

## Imported service history

Retain recognized imported claims with source coverage, supplied dates and provenance. The imported QSO content supplies the frozen upload/confirmation comparison baseline; it does not prove an exact historical serialized payload or establish missing counterpart information. See [imported baselines](service-evidence.md#31-imported-baselines). D50 accepts positive imported upload status as immediately suppressing resubmission to that service without prior verification. Service-relevant changes make the QSO pending again. D51 makes missing upload status pending, like explicit unsent status, independently for each service. Normal scheduling and upload requirements still apply. D52 accepts recognized positive imported confirmations immediately, preserving supplied dates and provenance without verification. Exact field mapping remains open; do not fabricate a Clacks Log attempt or exact historical payload from an imported flag.

The [LOG4OM sample](../references/log4om-adif-import.md) supplies evidence about available data, not the target model. Import support does not imply preserving opaque application-specific fields for re-export.

## Accepted schedule

| Action | Behavior |
| --- | --- |
| Automatic upload | Periodic checks for eligible pending QSOs on both services; interval configurable. Discussion working default: one minute. |
| Cooldown | Working default: five minutes since the last edit. An edit resets eligibility time. |
| Upload pending now | Manual trigger bypasses cooldown, using accepted server state. |
| Automatic download | Configurable interval, default 30 minutes. |
| Check now | Manual download trigger. |

These defaults are product decisions, not claims that providers mandate these numbers. Provider recommendations should continue to be tracked with current documentation. D47 selects automatic retries unless an upload is explicitly successful, with pause until remedy and explicit resume for clearly non-transient failures. D48 requires detailed diagnostics. Special startup scheduling, exact backoff timing, detailed pause dependency mapping, global rate limits, and provider-specific failure handling remain open.

An accepted edit to data included in a service submission makes that QSO pending for that service again. The [service projections](service-evidence.md#5-submission-projections-and-equality) define compared values; QRZ correction targeting and remote clearing retain explicit interoperability limits. Changing unrelated local information should not automatically imply an external correction.

## Reconciliation context

The user requested a survey of established loggers' service reconciliation behavior before inventing new policies. The preserved handoff discusses candidate applications, but a comprehensive survey of every available logger's LoTW/QRZ/eQSL reconciliation behavior has not been completed or recorded here. Continue from concrete incumbent behavior and primary evidence rather than treating proposed policies as findings from that survey.

[Focused reconciliation research](../references/service-reconciliation.md) records DXKeeper's documented replacement/preserve/review options and the limits of current LOG4OM verification. This is not a completed comprehensive survey; D237 refines the earlier D33–D36 enrichment policy, as described below.

Jacob has observed LOG4OM correcting local county, DX Century Club (DXCC), and International Telecommunication Union (ITU) information from downloaded confirmation data. LOG4OM also offers an option to create local records for incoming confirmations without a match. Preserve those observations when evaluating behavior; do not assert they are universally implemented or make their exact policies requirements without a decision.

D237 supersedes D33–D36's automatic replacement of recorded contact facts. Keep downloaded other-station details in separate provider observations; do not overwrite manually recorded information. This applies consistently across fields, including Islands on the Air (IOTA) references, rather than creating an IOTA-specific replacement exception. Our station's operating context remains excluded. Updating provider evidence is distinct from editing recorded contact facts. D238 selects logical fallback in this order: recorded QSO information, eligible LoTW information, eligible QRZ information, then the eligible directory snapshot. Select location as a whole from the first available source, including partial recorded locations; do not mix location fields across sources or change recorded QSO facts. Apply this to display and supported export/upload fields, retaining provenance and existing association/applicability requirements. Exact field coverage and representation remain engineering work.

D37 requires explicit confirmation to add an unmatched contact locally or reject it locally; there is no automatic creation. Rejection leaves the external record untouched and is scoped to the reviewed version under D218. For downloaded contacts explicitly added locally, D38 uses supplied historical station context; current defaults must not fill gaps. D252 requires both station callsigns before creation, with explicit historical callsign assignment when ours is missing. Other missing optional own-station fields remain unknown. Neither provider association nor confirmation status authorizes changing contact identity, time, band or mode.

The premise that a confirmed QSO routinely needs identity/time correction was challenged: confirmation already implies matching within the service's tolerance. Investigate realistic metadata corrections and exceptional identity corrections separately.

## Incremental downloads and duplicate processing

Repeated delivery of an already processed remote observation is a duplicate, not a new enrichment event. It must not reapply the previous enrichment, advance the QSO revision, or undo a later edit or conflict resolution merely because the local value now differs. Deduplication must distinguish remote updates to the same contact from repeated delivery of an existing update; the [observation procedure](service-evidence.md#7-observation-identity-equality-and-acquisition) distinguishes unchanged delivery, observed transitions and recovered evidence.

LoTW provides separate receipt/confirmation progress timestamps and inclusive since filters. Boundary overlap or retries can redeliver records. Query progress limits normal downloads; duplicate processing protection ensures those repeated deliveries have no second effect. The [transaction contract](service-evidence.md#11-transactions-dispatch-and-interrupted-transfer) requires durable observations/pending work before download progress advances and atomic completion with any coupled QSO effects. Download progress is distinct from synchronization acknowledgment.

## Service ownership and credentials

The server needs access to the credentials/capabilities required by each service: LoTW signing certificates/private keys, appropriate LoTW query credentials, and QRZ API access. Exact subscription/access requirements, credential scoping, storage, and revocation user experience (UX) remain to be designed.

D46 requires each newly dispatched submission or retry to use the current accepted server QSO values. Recorded attempt payloads are history, not stale payloads to replay. An in-flight response describes the values actually sent; later edits can still require a subsequent submission. D63 scopes external jobs, destination configurations, and credential access to the owning account. Only accepted shared QSO state within that account is publishable. Clients can log offline but cannot run an independent competing publication pipeline. Server jobs must preserve per-service outcomes and make actionable failures visible.

## Web management and diagnostics (D66)

MVP external-service configuration and upload diagnostics are available in the web user interface (UI). They operate on the same account-scoped server configuration, credentials, and job state used by synchronization-connected clients. D67 makes the desktop open the web UI for full external-service setup, while keeping station defaults/profiles native and service status/routine actions available in the desktop. Exact presentation and native diagnostic detail remain open.

## Destination routing (D53)

MVP supports multiple QRZ logbooks and LoTW certificates. Route each QSO using its saved station callsign and date, respecting the service's eligibility rules. Missing or ambiguous routing pauses only affected uploads under D47-D49. Current client defaults must not reroute historical contacts. See [service routing](../references/service-routing.md) for evidence and remaining selection/configuration details.

## Upload recovery (D47-D48)

Automatically retry uploads that were not explicitly successful, including attempts with missing or unusable responses. Use current accepted server QSO values at each dispatch under D46. Clearly non-transient failures pause upload attempts until the user remedies the problem and explicitly resumes. Provide as much diagnostic information as possible. D49 pauses only affected uploads, allowing unrelated uploads to continue. Failure classification, retry timing, worker pause dependencies and presentation remain adapter work. [Service evidence](service-evidence.md) specifies dispatch uncertainty, current-value coverage, recoverable dependencies and semantic retention; raw diagnostics have a separate future retention policy. See also the [publication recovery evidence](../references/publication-recovery.md).

## Accepted deletion behavior (D54)

[LoTW submission guidance](https://lotw.arrl.org/lotw-help/submitting-qsos/) says submitted QSOs cannot be deleted. [QRZ's API guide](https://www.qrz.com/docs/logbook/QRZLogbookAPI.html) provides DELETE by remote log ID. D05 treats deletion as an edit for Clacks Log synchronization. D54 leaves external service copies untouched in MVP.

A deletion accepted by the synchronization server prevents further uploads of that QSO, while leaving already submitted external copies unchanged. Preserve enough deletion/remote association information to avoid presenting a known deleted contact as a new unmatched download. The [evidence contract](service-evidence.md#8-association-review-and-deletion) retains deletion suppression independently of query progress and preserves in-flight attempt/outcome history without replaying old dispatches. External deletion propagation is not included in MVP.

## WS Suite (the WSJT-X family of weak-signal radio applications) capture

[The WS source investigation](../references/ws-suite-udp.md) identifies primary
binary completion types 5 and 12 in WS 3.2.1 build 261010. Configure reception
through **Settings → WS Suite…** on the capturing desktop. It supports direct
unicast binding and multicast group/interface membership; preferences are local
to that workspace. Only one Clacks client captures each WS source. Other clients
receive saved contacts through ordinary synchronization. Other applications may
join the same multicast group/port. WS and Clacks normally run on the same
computer; multicast sending must use the matching interface with local delivery
enabled. Clacks does not change WS settings or forward its packets.

The first completion starts a 100 ms pairing window. Both usable representations
end it early and select type 12 ADIF as a whole. At expiry the available usable
representation is saved, with structured type 5 as fallback. Same-type repeats
do not extend the wait. Source identity uses sender IP and the opaque protocol
Id, excluding source port. Correlation uses normalized contacted callsign, UTC
start second, exact supplied frequency (otherwise band) and leaf mode. Keep the
latest successful completion identity per source until its next success, in
memory only, including across receiver configuration changes. There are no
persistent receipts or restart/deletion replay guarantees.

Supplied own facts win; missing or empty live own fields use the captured active
operating defaults. Invalid nonempty optional values are omitted with feedback,
not replaced by defaults. Incoming contacts receive the selected named tag, or
none for built-in views, without changing defaults or the unfinished manual
contact. Supplied decimal watts truncate below milliwatts (`5.0007 W` becomes
`5000 mW`). Source contest codes resolve eligible literal ordinary CONTEST_ID
bindings in current account definitions, never seeded IDs. Received exchange
fields do not become our geography.

Capture uses the existing local contact save and synchronization path, without
waiting for server/provider access or directory lookup. Reception failures are
separate from the checkbox/unapplied state; Apply retries unchanged failed
settings. Workspace shutdown/reconfiguration settles admitted pairing windows
and saves before releasing their resources. Automatic catch-up from WS's local
ADIF log, historical import and #21's status/selection workflow are separate
capabilities. ADIF file import remains part of MVP; its historical import
semantics are separate from this live capture path.

## Primary references

- [American Radio Relay League (ARRL) submission guidance](https://lotw.arrl.org/lotw-help/developer-submit-qsos/)
- [ARRL acceptance and confirmation queries](https://lotw.arrl.org/lotw-help/developer-query-qsos-qsls/)
- [QRZ Logbook API documentation](https://www.qrz.com/docs/logbook30/api)

ARRL distinguishes upload from subsequent acceptance/confirmation and discourages routine resubmission of unchanged contacts. The discussion did not establish a provider-mandated numeric upload/download cadence. Earlier references to QRZ semi-random delays or LoTW live-upload discouragement should not be presented as verified requirements without supporting current documentation.

## Clacks logbooks and publication scope

Clacks logbooks are tag-defined views of account-owned QSOs. A QSO has one set of recorded facts, directory/provider evidence and publication history, regardless of how many tags display it. Tag names do not supply station context or activity references, and Clacks tags need not map one-to-one to external QRZ logbooks. Route each QSO by saved station callsign/date under D53.

Each tag has an independent automatic-upload setting for LoTW and QRZ (D245/D246):

| State | Effect |
| --- | --- |
| Enable | Grants automatic upload unless another current tag forbids it. |
| Forbid | Blocks automatic upload even if another current tag enables it. |
| Neither | Neither grants nor forbids automatic upload; the default for new tags, including offline-created tags. |

Evaluate all of the QSO's current tags. Any Forbid takes precedence; otherwise at least one Enable is required, subject to routing, cooldown and pause rules. All-Neither and untagged contacts have no automatic-upload grant. A QSO is not uploaded separately for each enabling tag. For example, Main (Enable) plus Activity (Neither) grants eligibility; Main (Enable) plus Private (Forbid) blocks it. A tag edit can change eligibility without changing the service payload or requiring resubmission of already covered unchanged values.

Explicit manual uploads retain existing routing, accepted-QSO, cooldown-bypass and pause/remedy rules (D109). Tag settings govern automatic uploads; manual dispatch does not silently resume a paused failure.

### External-record associations

Within an account and service destination, each external record may associate with at most one local QSO (D251). A QSO may independently associate with records in other services/destinations. Distinct local QSOs may share matching fields; they are not automatically merged. An explicit Duplicate import does not inherit an occupied external association. If its upload would target an external record already associated with another QSO, require resolution rather than a second association or silent replacement. Historical imported positive statuses remain trusted under D50/D52, but do not establish authority to share one external record between local QSOs.

Downloads run server-side and search all account QSOs, including untagged contacts and those without automatic-upload eligibility. Current desktop view does not affect matching. Established associations take precedence; otherwise apply a unique confident match automatically. Compatible incomplete identity and the 30-minute-either-side window produce possible matches requiring explicit selection even when unique. Ambiguous candidates require choosing at most one QSO, never fan-out to inferred copies. Multiple tags on a QSO produce one candidate and one association, not copies per view. The [destination/key contract](service-evidence.md#4-destinations-remote-identity-and-correction-safety) and [transaction guards](service-evidence.md#111-conditional-acceptance) specify identity and uniqueness obligations; real persistence must enforce them.

Unmatched incoming records retain explicit add/reject (D37/D105). Adding requires selecting the tag to apply; rejection needs no tag. A selected group of different incoming records can be reviewed together without assigning one external record to multiple QSOs. Preserve repeated-observation suppression, version-specific rejection and deletion memory.

## Confirmation after QSO corrections (D124)

D124 requires preserving prior confirmation evidence and provenance when a QSO is edited. Current confirmed status applies only if the evidence still matches the current QSO under the selected service matching rules. If a correction makes it no longer match (for example, a change of worked callsign or band), retain the old confirmation as historical evidence without treating the edited contact as currently confirmed. The [applicability contract](service-evidence.md#6-confirmation-applicability-and-source-eligibility) specifies imported baselines, mode/band dimensions and conservative time edits. Unknown applicability is distinct from explicit provider-reported unconfirmed. This does not require initial verification of D52 imported positive confirmations, erase history, or conflate confirmation with D46 publication of current values.

D125 requires a newer service observation that explicitly reports unconfirmed to update current service status while retaining previous positive confirmation evidence/history. Missing fields, an empty result window, and a failed request do not by themselves erase confirmation. The [observation procedure](service-evidence.md#7-observation-identity-equality-and-acquisition) preserves provider-specific timestamp scope and unknown ordering. [QRZ API documentation](https://www.qrz.com/docs/logbook/QRZLogbookAPI.html), checked 2026-10-12, states INSERT with REPLACE can overwrite a confirmed QSO with an unconfirmed one pending a new match. This status behavior does not select a QRZ correction algorithm.

## Contacted-station information fallback (D238)

Recorded location-specific references establish a present recorded location even without a known point, grid or other geographic fields. This applies to park, summit, island and other location-specific contest/event references, not just a single program. The whole-location rule therefore leaves unresolved recorded geographic details unknown instead of filling them from lower-priority provider locations. A participation-only contest/event identifier does not establish a location by itself. Reference classification and supported mappings must preserve this distinction.

External-service uploads use D238's logical location fallback: recorded QSO location, then eligible LoTW location, then eligible QRZ location, then eligible directory location, subject to service field support/validation. Native data preserves those sources separately; preparing an upload does not copy directory values into recorded QSO location. D201 selects whole-location precedence: a present QSO location, including partial information, excludes directory location from filling gaps. D203 retains a separate directory-location snapshot, including provider and lookup time, with each QSO and synchronizes it to make fallback available to server uploads. D206 selects whole-snapshot replacement on successful lookup and preservation on failure/not found. Resubmission behavior must follow the existing service-relevant-change policy applied to effective output; the [service projection contract](service-evidence.md#5-submission-projections-and-equality) defines field sets and semantic comparison.

D204 separates all supported directory information, including names, from recorded facts; D238 supplies the combined fallback order for these and confirmation-provider details, not only location. The separate directory snapshot is synchronized with its QSO. Use effective values for supported output fields without copying them into recorded QSO facts; D201 whole-location source selection remains unchanged.

D207 requires a snapshot to belong to the QSO’s current normalized directory lookup key before it can supply upload fallback. A key-changing callsign edit immediately stops old-snapshot use; failed lookup for the new key does not preserve old-station fallback. Retain a snapshot when the normalized key is unchanged.

D209 selects snapshots automatically per QSO/normalized key by successful lookup time with deterministic tie-breaking, without user resolution. Apply the existing service-relevant-change upload policy to effective serialized service values: provider/lookup metadata changes alone do not justify resubmission; changed effective service fields do. Use the [service projection contract](service-evidence.md#5-submission-projections-and-equality) for compared output values.

## Service adapter invariants

Define provider record/update identity, download cursors and page/window coverage
so observations and their processing effects are atomic and repeat-safe. Preserve
actual dispatched values/destinations with publication outcomes; imported statuses
do not fabricate historical attempts or exact sent values. Deletion memory and
version-specific rejection have separate retention obligations.

Only provider information with established source and applicability participates
in fallback. A returned copy of our own uploaded values is not relabeled as the
other station's report because a confirmation flag is positive. In particular,
QRZ keeps the parties' reports independent and does not disclose the other party's
report; see [service evidence](../references/service-state-model.md#primary-evidence).

Acceptance must atomically enforce external-association uniqueness and selected
native references, including competing or in-flight attempts. Provider calls stay
outside database transactions. Define service-relevant effective-value comparison,
correction evidence, retry/backoff and coordinated provider limits without
changing current-value attempts, affected-only pauses or imported-status policy.

Directory adapters define portable-callsign normalization, eligible metadata,
request correlation, obsolete-key cleanup and effective service-output comparison.
Provider data stays separate from recorded facts; failure/not-found preserves
existing snapshots and completed bulk updates.

## Pure service component boundary

The model library's `service` modules own domain evidence, shared LoTW/QRZ
submission projections and applicability. The synchronization library's `service`
modules own dependency/combination rules, guarded coupled operations, download
coordination, accepted-evidence recovery and semantic-retention requirements.
[The synchronization crate guide](../lib/clacks-sync/README.md#service-processing-and-retention)
defines the actual adapter obligations. Worker/provider calls remain outside real
database transactions; returned plans do not prove persistence or external success.

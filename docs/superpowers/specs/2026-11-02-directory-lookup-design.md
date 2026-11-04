# Directory configuration, provisioning and desktop lookup

Engineering specification for [issue #17](https://github.com/jhelwig/clacks-log/issues/17),
including #20. Status: proposed for review; this document does not authorize
implementation or close the capability issue. Product and architecture decisions
remain those in the decision register, especially D88–D99, D116, D194–D209,
D236, D238 and D242. Concrete representations and defaults below are engineering
proposals, not additional agreed product requirements.

A radio contact is a QSO. Coordinated Universal Time is UTC. Application
programming interface is API. Extensible Markup Language is XML; JavaScript
Object Notation is JSON. Logbook of The World is LoTW; DX Century Club is DXCC.
CQ magazine and the International Telecommunication Union (ITU) define the two
zone systems. Universally Unique Lexicographically Sortable Identifiers are
ULIDs. Additional authenticated data is AAD. Hypertext Transfer Protocol is HTTP;
HTTPS is HTTP protected by Transport Layer Security (TLS).

## 1. Outcome and authority

An authenticated account owner configures QRZ or HamQTH through the production
web application. Enrolled desktops receive protected directory configuration
and query that provider directly during manual logging. Successful results
become separate, durable, synchronized contact snapshots. Logging never waits
for provider access or synchronization. Removing/replacing credentials and
reconnecting clients must work without altering enrollment material.

| Requirement | Authority |
| --- | --- |
| Account-level provider selection, client-direct requests and protected provisioning | D89, D96–D97, D196–D197; [accounts](../../accounts.md#client-direct-directory-lookup) |
| 500 ms after the last effective call change; no request for empty input | D94, D116 |
| Save immediately; correlate late results with their original contact | D95; [manual logging](../../manual-logging.md#6-save-and-next-entry-behavior) |
| Separate provider facts, whole-location selection, unchanged recorded values | D198–D204, D238; [native model](../../native-model.md#12-source-separation-equality-and-component-interfaces) |
| Whole-snapshot replacement, failure preservation and current-key eligibility | D206–D207 |
| Latest successful client lookup time, deterministic ties, no user conflict resolution | D209 |
| Account keys, deployment wrapping keys and maintenance integration | D236; [credential protection](../../credential-protection-proposal.md) |
| Existing logging, synchronization, recovery, visual and observability contracts | [product](../../product.md), [synchronization](../../synchronization-model.md), [desktop UI](../../desktop-ui.md), [observability](../../observability.md) |

Bulk lookup stays in #22 and WS Suite integration in #21. Publication credentials
and LoTW signing material stay server-only in #31/#30. This delivery adds no
reusable persistent callsign-result cache, provider fallback/merging, automatic
bulk catch-up, secret unlock workflow, new synchronization scope or new service.
Directory credentials are not QRZ logbook publication keys.

## 2. Existing components and approaches

The implementation baseline inspected on 2 November 2026 includes:

- `clacks-model::effective::{StationFacts,EligibleStationFacts,select_name,select_location}`:
  separate name/location projections and provenance-aware selection. Callers
  establish eligibility. These functions do not store snapshots or arbitrate
  concurrent refreshes.
- `clacks-model::{scalar,position,location}`: checked callsigns, operational
  `UtcTimestamp`, grid-area/point values, geographic identifiers and zone values.
- `clacks-sync::{record,client,acceptance,recovery}`: record envelopes, pending
  work, frozen proposals, conditional acceptance and accepted-state recovery.
  Directory payloads do not yet exist. Ordinary QSO reconciliation is not the
  directory value policy.
- Concrete SQLite/PostgreSQL payload, metadata, discovery and transfer modules:
  relational values and atomic durable receipt under the existing account scope.
- `clacks-server::secrets::{keys,envelope,receipt}` and
  `maintenance::keys`: authenticated encryption, account locking, reference
  reporting, bounded account-key rotation and deployment-key rewrapping.
  Receipt storage is not a directory credential store.
- `clacks-desktop::{credentials,enrollment::credentials}`: native secret bytes,
  platform entry construction and the separate enrollment bundle. Windows and
  macOS are implemented; Linux protection remains a later platform integration.
- The authenticated WebSocket connection, existing reconnect task, account
  change notification, application-owned Tokio runtime, contact forms and save
  operations. There is no directory provisioning message or provider adapter.

Three storage approaches were considered. Putting directory values inside QSO
facts would make refreshes participate in recorded-fact conflicts and violate
source separation. Creating one permanently retained record per request would
add unnecessary per-refresh history. The proposed approach is one directory
record per QSO containing one winning snapshot per normalized lookup key. This
directly implements D209's QSO/key partition while reusing `QsoId` and the
existing record infrastructure. It requires no composite-ID codec or extra
per-request synchronized entities. These are engineering choices subject to
review, not a reopening of the architecture.

## 3. Native directory values and provider mapping

Add focused `clacks-model::directory` types:

| Value | Contents and invariant |
| --- | --- |
| `DirectoryProvider` | Closed QRZ/HamQTH enum; no unused future providers. |
| `LookupKey` | Checked nonblank uppercase request text, distinct from `RecordedCallsign`. Derive it from a recorded call through the shared request transformation; restoration validates saved text without transforming it again. |
| `DirectorySnapshot` | Provider, lookup key, successful client completion time as existing `UtcTimestamp`, freshly generated lookup ULID, and typed returned directory facts. |
| `DirectoryFacts` | Name, optional native location, separate provider reference associations, locality, returned callsign, supplied locator and coordinate-source metadata. Reuse checked `Locator`, `Coordinates` and geographic types. |
| `DirectoryRecord` | QSO ID and ordered map from lookup key to its winning complete snapshot. Map key and embedded snapshot key must match. |

Generate a lookup ULID once when admitting a request; a successful response
retains it in its snapshot. Provider session renewal does not generate a new ID.
Order snapshots by `(lookup_time.seconds, lookup_time.nanoseconds, lookup_id)`.
Retain this tuple unchanged through saves, retries, transfer and recovery. ULID
ordering breaks ties only; it is not causal evidence. Equal lookup identity with
different content is invalid data, not an invitation to choose arbitrary bytes.

Directory facts project into `StationFacts` for name/location selection.
Use existing checked native values rather than copying the QSO schema or
introducing a generic provider-field map. Preserve provider-returned identity
and coordinate-source metadata when needed to interpret the result; neither
changes the recorded worked callsign or request key.

| Native meaning | QRZ XML | HamQTH v2 | Mapping |
| --- | --- | --- | --- |
| Name | `fname`, `name`, `nickname` | `nick`, `name` | Prefer nonblank on-air nickname, then supplied name; combine QRZ first/last names with one separator. Preserve nonblank text and Unicode. |
| Returned identity | `call`, `xref` | `callsign` | Provider provenance only; not a replacement QSO identity. |
| Grid/coordinates | `grid`, `lat`, `lon`, `geoloc` | `location.grid`, `location.lat`, `location.lon` | Checked position and retained source meaning as below. |
| DXCC entity | `dxcc` | `dxcc` | Canonical decimal entity identifier; not QRZ `ccode` or HamQTH `address.dxcc`. |
| State/province | `state` | `region.usState` | Preserve the native geographic value; no inference from unrelated regional programs. |
| County | `county` with `state`; `fips` is source metadata | `region.usCounty` with `region.usState` | Match existing subdivision conventions, including state-qualified US county values. |
| Zones | `cqzone`, `ituzone` | `cq`, `itu` | Existing checked CQ/ITU values. |
| Island reference | `iota` | `iota` | Separate provider `ReferenceAssociation` for the seeded IOTA group, never a recorded QSO reference. |

The current native effective view supports name and these geographic concepts;
its reference integration must be extended for the supplied island reference.
Provider address, biography, photographs, email, license details, social links
and QSL preferences do not justify a new contact-information subsystem. Ignore
unsupported fields rather than retaining arbitrary raw responses. Directory
participation indicators are not contact confirmation. For Islands on the Air
(IOTA), use the seeded group's stable ID `01M5QZFPZZ9GPDY819J1WMTJAK`, not its
editable display name. Apply its identifier normalization/syntax when available;
retain unknown-group associations under existing native rules rather than
recreating a removed definition. The current seed has no scalar IOTA export
mapping. Extend the existing export context to read the selected effective
contacted-station references and evaluate the account's current ordinary group
mappings. No mapping means the existing omission/warning behavior, not an
implicit adapter mapping. Do not rewrite existing account definitions or
recorded facts to force an output field. Other provider regional labels such as
HamQTH `dok`, `district` or `oblast` do not establish a mapping to an arbitrary
account reference group.

Extend the existing effective station projection to carry provider references
alongside its optional location, and select their location source once. An
eligible source with location-bearing references establishes location presence
even if its geographic fields are absent, just as recorded references do.
Choose location and location-bearing references from that same selected source;
a recorded partial location cannot acquire directory IOTA. Keep recorded
non-location participation and own-station references untouched. Use this shared
selection in client read projections and QRZ/export preparation; extend eligible
confirmation projection to pass through its already modeled references as well,
so the accepted LoTW/QRZ/directory precedence remains consistent. This is an
extension of an existing source selector and its consumers, not a second
source-priority system or a temporary mutated QSO used as output input.

Locality/QTH text is separate directory metadata, not a geographic identifier.
Retain QRZ `addr2` and HamQTH `qth` as optional locality text in `DirectoryFacts`
and make it available in source-labeled directory details. Do not map it into
county/state, treat a postal address as the actual QSO location, or add a new
recorded-QSO field. Current export/service projections have no locality target;
their supported field set remains unchanged.

QRZ documents grid-derived coordinates and postal/state/entity approximations.
Use a valid supplied grid as a grid area when `geoloc=grid`; do not reinterpret
its center as an explicit point. Otherwise project QRZ's valid coordinate pair
as the directory's reported point, retaining its `geoloc` classification; if
the coordinates are absent/invalid, use a valid supplied grid. A reported point
is not a claim of measurement accuracy or operator-recorded QSO position.
For HamQTH, whose callbook response has no equivalent coordinate-origin field,
prefer a valid supplied grid area at its supplied precision; use a valid
coordinate pair only when there is no valid grid. Retain the supplied locator
and coordinate pair in the typed directory result when both exist, including
when one was not selected for the native position. Coordinate conversion uses
existing range checks and nearest-millionth rounding with ties away from zero.
Do not calculate a more precise locator than supplied from a grid-derived
center. Neither provider's location establishes the location of this QSO.

Absent/blank/invalid optional values map to absence with a bounded conversion
diagnostic where useful; do not fail a valid callsign result because an optional
zone is malformed. Failure to parse the response structure or establish a
successful provider result is a lookup failure and preserves the old snapshot.
A valid successful result with missing fields replaces the old snapshot as a
whole. There is no field-by-field retention of old successful values.

### Lookup-key normalization

Trim and uppercase without changing the recorded callsign. Remove terminal
`/P` for D202's specified portable form. As a conservative engineering default,
preserve other slash forms, prefixes and ambiguous components and pass them
through the provider's correctly encoded callsign parameter. Neither arbitrary
slash stripping nor an amateur callsign-licensing validator is required. Thus
`OK2CQR/P` queries `OK2CQR`, while `EA8/OK2CQR` remains `EA8/OK2CQR`.
Use one pure function for requests, snapshot eligibility and callsign-edit
comparison on clients and server. Do not recompute stored keys from the
provider-returned identity or silently migrate saved associations.

## 4. Account configuration and encrypted persistence

Use account-authorized concrete directory operations in `clacks-server`; the
web component calls them through the existing Leptos/Axum action pattern,
browser session authorization and same-origin protection. Derive the account
from authorization, not a posted account ID. No new provider authentication,
reauthentication policy or generic secret CRUD service is needed.

Persist a nonsecret account directory configuration with a checked semantic
revision and optional selected provider. Persist typed credentials by account
and provider, with stable material identity, material revision, and one complete
encrypted envelope. Credential entry contains provider login and password;
provider sessions/tokens are transient desktop state. Provider credentials can
be retained server-side when another provider is selected, but only the selected
provider's material is provisioned for lookup. No automatic fallback follows
from having another saved credential.

Use the existing signed-64-bit counter domain. Missing configuration reads as
revision zero without a write; committed semantic changes advance it once.
New material has a fresh ULID identity and positive revision; replacement
advances its material revision, while removal deletes that material. Recreating
removed material assigns a new identity. No-op public changes/removals leave
the configuration revision unchanged. These counters protect stale mutations;
they are not replica clocks or proof of post-restore freshness.

Setup has one lookup-provider selector with Save selection and independent QRZ
and HamQTH credential forms. Each shows the saved username and a fixed masked
password placeholder when a password exists, never its value or length. Save
credentials adds or updates automatically; a blank password input retains the
saved password, allowing username-only and password-only edits. Remove
credentials clears the login and its inputs. There are no add/replace modes or
separate credential-status labels. Failed submissions retain entered values and
show feedback beside the relevant form. Selection saves preserve both logins;
credential saves preserve the selected provider. Each form submits the current
configuration revision, without displaying that internal counter to the user.
Under the existing account-row lock, check current authorization and revision,
load/create the active account key, and commit the complete intended change.
Stale writes return current nonsecret state without overwriting newer changes.
Blank password input does not mean removal; removal is explicit. Do not require
a live provider login to save credentials; client lookup establishes usability.

Bind authenticated encryption to independently expected account, directory
purpose/provider, material identity, material revision and envelope header.
Use the existing Advanced Encryption Standard with a 256-bit key in
Galois/Counter Mode (AES-256-GCM), randomness, zeroized buffers and canonical envelope
facilities; introduce only the concrete directory envelope/payload consumed
here. Account key selection occurs inside the locked write transaction.
Use the concrete packed envelope
`$clacks$1$directory$<account-key-version>$<nonce>$<ciphertext-and-tag>` with
the current canonical encoding/length checks and independently expected AAD.
Encryption maintenance does not change semantic configuration/material
revisions. Removing selected credentials preserves selection with a clear
unconfigured state; selecting no provider disables new lookup requests.

Notify the existing account change channel after commit. Database state is
authoritative; notifications only wake readers. Never hold a database lock or
transaction across provider HTTP, WebSocket sends or native secret access.

Extend key status to count actual directory envelopes per account/key version.
Extend bounded account rotation to migrate receipts and directory material
within the existing total batch limit and deterministic family/row order.
Use explicit source/target versions and remaining references for retry; no
rotation jobs, family registry or generalized ciphertext visitor is necessary.
Rotation completion and the guard against rotating again inspect both families.
Deployment rewrapping already covers every account-key envelope; retain that
behavior and verify directory values remain decryptable. Update production
migrations, runtime/maintenance grants, fixed query files and SQLx offline
metadata together. Keep retained deployment/account keys for supported backups.

## 5. Protected provisioning and reconnect

Extend the existing authenticated WebSocket with concrete directory configuration
request/response messages. They are not ordinary replicated record payloads:
secret bytes must never enter accepted-history journals, transfer staging,
pending work or SQLite payloads. Use the existing current account/client checks,
TLS connection and account notification/reconnect path. No alternate transport,
enrollment round trip, generic secret endpoint or application encryption
handshake is required.

The server reads configuration and selected material consistently, authenticates
the envelope using stored owner/purpose/identity/revision, and responds only to
the authorized account's admitted client. A response contains current public
configuration revision, selected provider, optional selected credentials and
their material identity/revision. It also explicitly represents disabled or
missing material. Secret-bearing messages/types have no payload `Debug` or
automatic trace formatting. Serialized secret buffers are short-lived and
zeroized where ownership permits.

Fetch current configuration on every authenticated reconnect and on relevant
account changes while connected. Do not skip the read because the account's
ordinary record checkpoint is unchanged. Do not assume matching revision alone
proves equality across a restored database: compare selected configuration and
complete typed material with the installed native bundle without logging secret
values. This also handles reused revisions after rollback; public revision and
identity alone are insufficient. Server-restored configuration is authoritative;
clients do not upload old credentials or reject it solely because its revision
is numerically lower than a locally retained one.

Use one session-owned installation task and ordinary Tokio channels for ordered
deliveries from the current authenticated connection. Coalesce queued updates
to the latest complete response; do not queue unrelated work or invent durable
connection epochs. Ended connections cannot deliver new installation work.
Check current connection/cancellation and the latest delivery immediately before
starting the native update. Serialize installation so an older response cannot
complete after a newer installation. A native operation already started may
complete; the next current delivery follows it on the same worker.
Native access/installation is not awaited in the socket's receive/acknowledgment
loop. Directory installation failures are delivered as directory diagnostics;
they do not tear down the working synchronization connection.

On the desktop, reuse `credentials::entry` and `spawn_blocking`. macOS uses the
selected library's Data Protection Keychain backend, local storage in the signed
app's default access group and its default When Unlocked access policy. Do not
require user presence or enable iCloud synchronization. The macOS app and native
integration-test host require signing entitlements authorized by an embedded
provisioning profile, following [Apple's guidance](https://developer.apple.com/documentation/technotes/tn3137-on-mac-keychains).
Windows retains Credential Manager Local persistence. Do not migrate or fall back
to the older file-based macOS Keychain. The fixed directory namespace and bundle
owner checks bind the stored value to the enrolled deployment and account.

Store the complete public configuration and optional selected material as one
protected native value. Allocate a workspace-local entry name once under
`directory.entry` in the existing `local_state` store, before native access.
This locator survives reauthorization and separates concurrently open workspaces;
it is not changed during provisioning. There is no second installation record,
configuration/reference transaction, per-update entry or obsolete-entry cleanup.

Use the backend add/update operation to replace that complete value. Disabled
and selected-but-unconfigured are complete values without material. A failed
operation retains the last usable runtime configuration and reports a directory
storage diagnostic. An explicit Retry directory storage action wakes the same
worker to retry local reads and the retained current delivery; it does not
reconnect or reauthorize, and creates no timed retry queue. Lookup response
ownership compares captured provider and actual material, including restored
content whose IDs/revisions may have been reused.

Disconnected clients retain their previously installed configuration until they
receive a change. Revocation blocks further provisioning through existing authority checks; it
cannot erase credentials already received by a disconnected client.

Native storage failure is not a synchronization failure. Directory installation
must not prevent acknowledgments for unrelated durably installed QSO records.
On restart, load the fixed entry and report an unreadable value
without manufacturing credentials or damaging enrollment. Local-only logging
continues without account provisioning; this issue does not add a second,
desktop-only credential-configuration workflow.

## 6. Direct provider adapters

Place concrete QRZ and HamQTH adapters under the desktop directory module,
called by the actual session-owned lookup task. Shared pure types live in the
model; provisioning messages live with the existing native protocol. Keep
application crates thin. Two closed provider variants and ordinary Rust methods
suffice; no plugin registry, generic provider repository, custom HTTP stack,
new crate hierarchy or task framework is justified.

Reuse the locked `reqwest` 0.12.28 client, `serde_json`, `roxmltree` 0.21.1 and
Tokio. `reqwest` provides form/query encoding, pooling, TLS, timeouts and redirect
policy; `roxmltree` supplies XML parsing and disabled-by-default document type
definitions (DTDs). `keyring-core` supplies secret-byte operations. Configure/use those
facilities rather than wrapping them in a parallel transport or storage layer.
Do not log URLs containing credentials/session keys or unfiltered error bodies.

QRZ: HTTPS XML login using username/password and an application/version agent;
use POST form encoding, which the provider supports, to avoid secret query
URLs. Reuse the current returned session key and replace it when the provider
issues a new one. On session expiry, authenticate again and repeat the lookup
once. Distinguish incorrect credentials, callsign not found, subscription
limitations, service refusal and transport/parse failures. A limited subscriber
response is not permission to fabricate missing data or silently switch providers.

HamQTH: current v2 JSON API, POST login body, bearer token for callbook requests,
and application identification. Reuse the token until expiry. A lookup `401`
permits one login-and-repeat; a rejected login pauses authentication. Treat
`404` as not found and `429` using `Retry-After`. Do not mistake a returned home
record for confirmation of a portable station's contact location.

Own each provider session in the existing asynchronous desktop runtime; avoid
concurrent login storms through ordinary task ownership. In-memory session reuse
is distinct from the prohibited persistent callsign-result cache. Drop sessions
on credential replacement. Default to bounded network requests with a 5-second
connect timeout and 15-second total timeout, a one-mebibyte (1 MiB) response bound, and no
redirect forwarding of credentials. These are adjustable engineering defaults,
not provider guarantees or new contact-validity limits. Use library facilities
and bounded body collection; do not add custom retry schedulers or queues.

Ordinary network/provider failure reports an outcome and preserves any eligible
successful snapshot. Do not continuously retry a failed old contact: subsequent
normal entry requests or explicit retry can attempt again. Honor provider retry
timing. Authentication rejection pauses lookup until credentials change or the
operator explicitly retries (D195). Expired sessions alone do not cause that
pause. Authentication recovery never triggers bulk population.
Associate an authentication rejection with the request's material identity and
revision: a late rejection of superseded credentials must not pause the newly
installed credentials. This uses the captured request context, not a new epoch.

## 7. Entry, save and delayed response lifecycle

The corrected [#21 specification](2026-11-04-ws-suite-selection-design.md)
supersedes form-owned request correlation. WS Status changes only the shared
entry callsign; Worked Before observes that field independently of directory
lookup. Preserve every other unsaved field. Incoming calls, including repeated
values overriding local edits and empty calls, are authoritative.

Debounce entry lookup 500 ms after an effective trim/uppercase text change.
Unchanged input does not restart the timer; slash changes can restart it even
when the normalized lookup key stays the same. Empty input or Clear removes
only that entry's display interest. An old result cannot restore cleared hints.

Entry, saved-contact editing and WS capture obtain shared handles by lookup key.
Join an in-flight lookup or reuse a successful transient result. Retain reusable
successes for 24 hours from completion, with a 128-result limit per workspace;
hits do not extend age. The cache is memory-only and is not reconstructed from
contact snapshots. Provider/material changes govern new dispatch and preserve
cached successes. There is no success-refresh or cache-bypass action; existing
failure/login retries remain available.

WS capture independently requests its actual projected contact's key, starting
lookup on a miss immediately without the entry debounce. Never delay either
manual or capture saving for directory completion. A ready result can be saved
atomically with the QSO. Otherwise the actual save caller retains its immutable
key/deadline/handle alongside its QSO identity; only a successful save permits
late application. A failed save installs no late snapshot. Directory operations
do not own forms or search for corresponding completed contacts.

At application, the existing store transaction checks the current live QSO and
normalized key. Deletion or a key change prevents application; other recorded
edits do not. Opening an unchanged saved contact requests no refresh. Clearing
or changing entry does not cancel another consumer's eligible saved result.
Provider requests and result application are owned by the workspace lifetime,
independently of receiver reconfiguration/draining.

Provider selection/removal controls new dispatch after installed configuration
changes. Started operations retain their captured material; replacement does
not add a rejection epoch or suppress eligible successes from old credentials.
Provider changes do not invalidate retained contact snapshots.
All snapshots for one QSO/key compete under D209 regardless of provider;
disconnected clients can legitimately return a later successful old-provider
lookup. There is no current-provider gate on saved snapshot eligibility.

Present effective values with their source through the existing Contact details
and quick-entry composition. Recorded-name/grid controls continue to edit only
recorded facts; fallback display must not become their saved text accidentally.
Keep source attribution visible, including HamQTH attribution requested by its
API documentation. Use existing shared sections, typography, density and status
treatment. Lookup pending/not-found/error or authentication pause is local
feedback, never a modal prerequisite to logging or synchronization.

## 8. Snapshot persistence, synchronization and recovery

Add `RecordId::Directory(QsoId)` and a corresponding boxed `RecordValue` variant.
Directory record identity derives from its parent QSO; no new entity-ID mapping
or per-refresh record is needed. Add relational payload rows for the per-key
winning snapshots and their typed facts in both concrete stores. Keep causal
roles and native schema in the existing payload/metadata architecture. Do not
serialize the entire directory domain into an opaque database JSON field.

For each QSO/key, combine successful snapshots by maximum of the tuple in
section 3. Union distinct keys; replace each winning snapshot as a whole. This
operation is deterministic, associative, commutative and idempotent for valid
inputs. Apply this policy at local insertion, incoming receipt and recovery;
generic causal fast paths must not bypass per-key winner selection. A losing
lookup changes no effective facts, while required acceptance/support context
still follows the existing synchronization contract.
It is not a merge of facts from different providers or a field merge.
Retaining one winner per QSO/key supplies D209's partitioning; it does not create
a reusable cache for another QSO or retain every refresh. Superseded payloads
follow ordinary pending/accepted-support retention and collection, not a new
directory history-retention service.

Extend the concrete directory value branch in client reconciliation and
accepted-state recovery. Directory divergence never produces a manual review
of provider fields. Keep existing vector/baseline/lineage validation: lookup
timestamps choose directory values, not causal ancestry or account coverage.
When a combined result is needed, use existing preparation, guarded acceptance
and reconciled-context facilities; preserve frozen proposals on retry and keep
later local refreshes when their earlier proposal is acknowledged. A stale
server condition returns current state for automatic recombination/retry through
the ordinary cycle. Do not add a server conflict queue, second clock, background
merge worker or separate replication protocol.

New lookup application requires a live parent QSO; a locally created parent is
published before its snapshot. When the server observes an already deleted
parent, reject new lookup publication with a typed deleted-parent outcome;
the client consumes that outcome without a manual provider-fact conflict and
without endlessly retrying the obsolete pending lookup.
Installing already accepted history is different: a known deleted parent
permits inert retained directory state. The existing dependency/staging path
waits only while required parent history is genuinely missing, not for a
deleted parent to become live. Certified parent absence is not a deleted-parent
proof and must not silently establish a valid live parent. Fuller-state and
rollback recovery can therefore complete when the parent has been deleted.
Parent deletion always suppresses directory use and prevents a late result
from recreating the parent. Read eligibility joins the current working
QSO's normalized key with its directory winner, not the key of an old accepted
QSO version. Parent liveness/current key, not current provider selection, governs
fallback. Keep inactive-key facts separate; they cannot displace current-key
data or affect current effective values.

Extend both ordinary journal and fuller-state discovery, payload decoding,
staging/dependency reads, no-op receipts, retirement/compaction and rollback
recovery for the new actual record family. Acknowledge complete account coverage
only after the directory state and its interpretation are durable. Lookup
failure/not-found/authentication rejection create no replacement snapshot or
synchronized failure tombstone. Pending local snapshot values remain visible
offline and synchronize later without changing recorded QSO conflict rules.

Feed eligible facts into the existing effective selectors for display and
supported output projections. Source changes can alter effective values without
editing the QSO. Use existing service projection/equality contracts to determine
service-relevant changes; do not implement upload jobs or new resubmission policy
in #17. Backups/recovery preserve directory payloads and required key material
under the existing server/client recovery boundaries.

## 9. Observability

Extend existing runtime composition; libraries install no subscribers/exporters.
Use meaningful `tracing::instrument(skip_all, ...)` spans for account mutation,
provisioning read/install, provider login/lookup and snapshot application/merge.
Keep module-path targets and framework-owned span timing/context. Record safe
account/QSO IDs, provider, public revisions, lookup identity and bounded outcome
where useful. HTTP/provider operations and native access occur outside database
spans/transactions. Existing connection/session context links provisioning work;
lookup identity links delayed application without logging the callsign payload.

Configuration commit, authentication pause/resume and provisioning failure are
significant events. Ordinary lookup/not-found and unchanged arbitration are
debug-level detail. The owning runtime boundary reports handled failures once,
retaining error sources internally without unfiltered provider/HTTP text.
Exclude passwords, tokens, session keys, native bundles, encrypted envelopes,
raw provider responses, personal directory fields and secret-bearing frame dumps.

| Metric | Boundary and bounded labels |
| --- | --- |
| `clacks_directory_lookup_requests_total` | Actual directory lookup HTTP dispatch, labeled by provider; login is not a lookup. |
| `clacks_directory_lookups_total` | One completed logical lookup, labeled by provider and success/not_found/authentication/unavailable/cancelled; internal session retry is not a second logical lookup. |
| `clacks_directory_lookup_duration_seconds` | Runtime logical lookup from admission to terminal outcome, same provider/outcome labels; excludes debounce. |
| `clacks_directory_configuration_installs_total` | Changed complete configurations successfully stored natively; repeated equal values do not count; selected/disabled/unconfigured outcomes. |

Use existing storage/sync metrics for their ordinary operations rather than
inventing a directory metric for every helper. Extend existing maintenance
counts to include actually committed directory re-encryptions. IDs, lookup
keys and revisions never become metric labels. Per-request telemetry is bounded
by actual requests; no redraw-loop instrumentation or full payload formatting.

## 10. Verification tied to Clacks behavior

Use the existing integration suites, disposable databases with production
migrations/grants, actual application/store operations, native wire values and
authored provider-response fixtures. Do not exercise the persistent development
accounts or add test-only production APIs, query shortcuts or fake database
failpoints. Combine related assertions in existing scenarios.

| Requirement and concrete Clacks mistake | Verification |
| --- | --- |
| D202/D207: accidentally edit recorded callsign, strip a prefix, or keep old-key fallback | Fixed normalization examples, actual save/edit/reload and effective output. |
| D204/D238: copy directory name/grid into recorded facts, mix partial locations, or use an ineligible key | Existing selector scenarios extended through real persisted directory inputs and supported projections. |
| Provider mapping: confuse postal entity with station entity, upgrade grid center to a point, or treat missing fields as old values | Fixed QRZ/HamQTH responses with expected typed facts, whole replacement, invalid optional fields and provenance. |
| D94/D95/D116: restart debounce for unchanged input, wait to save, or update another/deleted contact | Simulated time and real request/callback ownership; pending-save success/failure and delayed response cases. |
| D195: pause on session expiry or retry rejected credentials indefinitely | Adapter request sequence against a local HTTP fixture using real clients; expired session, login rejection, rate limit, not found and transport failure. |
| D96/D197: require server access for lookup or query another provider | Actual desktop lookup with sync unavailable and only the selected adapter contacted. |
| D97/D236: cross-account provisioning, secret in SQLite/journal, stale replacement or broken removal | Authorized server and desktop provisioning through actual native entry/store paths; account isolation, stale writes, restart/reconnect and replacement/removal. |
| Native failure: discard the last usable installation on failed native access, block QSO sync, or change enrollment | Actual platform/native environmental failure where available, retained usable config and unrelated sync progress; no manufactured failpoints. |
| D209: choose by server arrival, require manual resolution, lose a newer pending refresh or skip an older restored server | Two-client real-store exchanges, equal-time ties, duplicate receipt, stale conditions, concurrent QSO edits, frozen retry and rollback recovery. |
| Parent dependency/deletion: acknowledge an unusable partial transfer or revive a deleted QSO | Interrupted bounded transfer, fuller catch-up and late-result/deletion scenarios using production operations. |
| Key maintenance: omit directory references or change material/config revisions | Existing rotation/rewrap command scenarios with actual directory credentials, repeat/no-op, committed counts and post-rotation lookup provisioning. |
| Observability: leak secrets, count rolled-back effects, or lose correlation | Configured filters/export capture with sentinel secrets; assert selected events, labels, committed metrics and delayed-result correlation. |

Provider fixtures test Clacks mapping, classification and request construction,
not whether reqwest implements TLS or the OS stores bytes. Native environmental
coverage must identify what the actual platform can reproduce; unavailable
Windows acceptance is reported, not replaced by a passing mock. Inspect the
assembled desktop/web production flow, empty/ready/pending/paused/failure states,
keyboard interaction and resizing against the existing UI contract. Preserve
screenshots and platform limits in delivery evidence, not this product contract.

Live authenticated provider checks remain an interoperability boundary: verify
login, lookup, session reuse/renewal and limited-access behavior with authorized
test accounts when available. Do not make real accounts or network availability
a prerequisite for the noninteractive test suite. Never claim fixture checks
establish live provider acceptance.

## 11. Implementation boundaries and sources

The next artifact is a task-level implementation plan after specification
review/approval. It must resolve exact module/query names, enumerate affected
record-family switches, field/reference mappings and migrations, and select
focused checks for each delivered slice. Those are engineering tasks. No known
product question is promoted from these details, and implementation is not
authorized by this document alone.

Source documentation inspected on 2 November 2026:

- [QRZ XML specification](https://www.qrz.com/docs/xml/current_spec.html):
  authentication, POST support, session reuse, subscription limits, returned
  fields and coordinate sources. This is directory access, not logbook API use.
- [HamQTH developer guide](https://www.hamqth.com/developers): v2 JSON login,
  bearer-token reuse, portable/home-record behavior, fields, privacy omissions,
  errors/rate limits and attribution. The linked OpenAPI document was not
  retrievable during this inspection; do not claim generated-client verification.
- [reqwest client configuration](https://docs.rs/reqwest/0.12.28/reqwest/struct.ClientBuilder.html)
  and locally installed locked source: TLS, pooling, timeouts, redirect policy
  and form encoding. The existing lockfile selects 0.12.28.
- Locked `roxmltree` 0.21.1 source: `ParsingOptions`, DTD rejection by default,
  node limits and `Document::parse_with_options`.
- Locked `keyring-core` 1.0.1 source: `Entry::{set_secret,get_secret,delete_credential}`;
  existing selected platform backend construction in desktop `credentials.rs`.

Provider/library documentation establishes supplied facilities; Clacks owns
field interpretation, eligibility, authorization, storage ordering and production
wiring. No authenticated provider calls, implementation tests, migrations or
deployment were performed in preparing this specification.

# WS Suite station selection and shared directory lookup

Engineering specification for [issue #21](https://github.com/jhelwig/clacks-log/issues/21).
Status: draft for review. The shared-handle approach is selected; the concrete
engineering defaults below are proposals. This document does not authorize
product implementation or close the capability issue.

WS Suite means the WSJT-X family of weak-signal applications. Its other-station
callsign is called the DX call. User Datagram Protocol is UDP; Coordinated
Universal Time is UTC; Amateur Data Interchange Format is ADIF. Logbook of The
World is LoTW. A QSO is a radio contact.

## 1. Outcome, authority and boundaries

Preserve the operator's GridTracker Generate Messages workflow through incoming
WS status and Clacks's existing shared entry. Directory information is fetched
once for concurrent consumers of the same lookup key. Completed WS contacts
continue to be built and saved independently of the manual entry.

| Requirement | Authority |
| --- | --- |
| Incoming WS calls overwrite the entry call, including repeated WS values after a local edit | D91/D93 |
| Debounce directory demand for 500 ms after the last effective entry-call change; unchanged packets do not restart it | D94 |
| Empty incoming calls clear the entry call and associated lookup/history display; cancel its pending lookup interest | D116 |
| Worked Before follows the entry field, using local prefix history in the selected view independently of directory access | D100/D115/D117; [history](../../history.md) |
| Locally edited calls do not control WS; preserve Generate Messages through WS status rather than a direct GridTracker protocol | D92/D115 |
| Save without waiting for directory access; late information is a separate snapshot associated with the saved contact | D95/D203/D204 |
| Recorded information takes precedence; whole-location selection, key eligibility and automatic snapshot arbitration remain unchanged | D198–D209/D238 |
| Status changes only the entry callsign; preserve all other unsaved fields | Explicit user decision in this #21 discussion |
| Clear sits immediately left of Save QSO and performs the existing post-save reset without saving | Explicit user decision in this #21 discussion |
| Status handling and QSO capture remain independent, sharing in-flight lookups and transient results by lookup key | Explicit user direction and selected approach in this #21 discussion |
| Retain successful transient directory results for up to 24 hours, subject to the 128-result eviction limit; no successful-result refresh behavior | Explicit user decisions in this #21 discussion |
| No explicit handling of simultaneous WS instances is needed for MVP | Explicit user scope decision in this #21 discussion |

The dependency is exactly:

```text
WS status -> shared entry callsign -> Worked Before
                                 -> debounced directory demand

WS completion -> independently built QSO -> local save
                                      -> reuse/join directory data by its key

Directory demands -> shared in-flight lookup / transient result by key
```

Worked Before has no WS-status input. A status packet neither identifies a
completed contact nor updates one. A directory request belongs to a lookup key,
not to a form or QSO. Each consumer decides where to display or retain its result.

Preserve #19's completion pairing, repeat recognition, source identity, supplied
facts, operating-default fallback and tag selection. Capture does not submit,
read contact-specific values from, or reset the manual form. Keep the existing
provider selection, protected credentials, direct client requests, authentication
pause/retry and synchronization behavior.

No persistent reusable directory cache, provider merging/fallback, automatic
bulk catch-up, new synchronization record family, source selector, outbound WS
control or background/tray behavior is added. User-triggered bulk lookup remains
#22. Existing provider/service snapshot persistence is not the transient cache.

## 2. Approaches and selected direction

Both viable approaches make entry and capture independent consumers and remove
QSO ownership from the directory request:

| Approach | Benefits | Costs |
| --- | --- | --- |
| Shared lookup handles, selected by the user | Consumers obtain the same asynchronous operation by key and own their result application. Standard shared futures supply completion delivery without a central consumer registry. | Short synchronized access to the keyed collection, transient-result retention and workspace request lifecycle wiring. |
| Dedicated lookup task with request/reply channels | One task owns the collection and provider work without locking collection access. | Commands and pending replies must be maintained for consumers. Those are implementation costs, not requirements. |

Use shared lookup handles. Do not retain the existing form-owned request model
and add source-to-draft or status-to-capture association to compensate for it.
There is no need to infer which status packet belongs to a completed QSO.

## 3. Existing facilities and actual integration changes

- `ws_suite::protocol` already checks the binary envelope and decodes completion
  messages. Extend it to expose Status type 1 as a separate decoded variant.
- `ws_suite::runtime` already receives packets, pairs completions and saves
  projected native contacts directly. Add status delivery and independently
  reuse ready/in-flight directory data for usable capture projections.
- `LoggingUi` owns one shared unfinished `NewEntry`; `contact_call()` is the
  existing input to Worked Before. Keep this dependency and the dedicated
  window's shared entry behavior.
- `directory::lookup` currently owns entries and requests by `QsoId`. Replace
  that request-ownership model with key-based sharing. QSO-specific result
  application belongs to its actual manual-save, saved-editor or capture caller.
- `ProviderSession` already supplies the selected provider, transport deadlines,
  pooled connections, authentication renewal and safe failures. Keep it as the
  provider boundary. Its login implementation already uses shared futures.
- `DirectorySnapshot` already contains lookup key, actual provider, successful
  completion time and lookup ID, without an embedded QSO ID. It is suitable for
  sharing unchanged among consumers.
- `save::contacts::save` accepts an optional directory snapshot atomically with
  the contact. `directory::snapshots::apply` checks the current local working
  contact's existence, deletion and key and arbitrates winners transactionally.
  Reuse these operations and their ordinary read/synchronization notifications.

The locked `futures-util` 0.3.34 implementation provides `Shared`, including
cloneable handles, shared completion wakeups and `peek()` for an available
result. It does not drive an unpolled future by itself. Tokio 1.54.1 supplies
workspace task ownership, cancellation and ordinary channels. Its mutex guidance
supports a standard mutex around short data-only operations. No lock is held
across provider requests, database operations or other awaits.

## 4. Shared lookup component

### 4.1. Ownership and operations

Create one shared lookup facility for the active local account workspace. Its
collection is keyed by the existing `LookupKey`. Provider credentials belong to
network request admission, not to the identity of cached station information.
Derive keys through the existing shared transformation and retain recorded
callsigns unchanged. For example, `W7HEX/P` and `W7HEX` share the directory key
`W7HEX` but remain different entry/Worked Before inputs.

Expose an operation for established entry-driven lookup demand and an operation
to reuse an existing result or join an existing request without starting one.
The former is used after the entry debounce and by existing failure-retry actions;
the latter is used by independent capture. An ordinary entry-driven request:

1. Returns an unexpired successful transient result when available.
2. Otherwise joins the in-flight operation for that key.
3. Otherwise admits one provider lookup and returns its shared handle.

A lookup handle yields a cloneable terminal outcome containing either one
immutable successful `DirectorySnapshot`, not-found, or a safe failure. It
contains no target QSO or manual-entry ID. Store shared successful values behind
ordinary reference-counted ownership rather than repeatedly copying all facts.

The collection lookup/insertion is atomic under one short lock so concurrent
entry and capture demands do not dispatch duplicate requests. Workspace-owned
tasks drive admitted operations to completion; retaining an unpolled future in
the collection alone is insufficient. Use the existing runtime/task facilities,
not a custom executor, consumer registry or durable request queue.

Generate one lookup ID when admitting an actual operation. Construct its snapshot
once at successful completion, using the actual provider and completion time.
Sharing, cache hits, saving and synchronization preserve that identity and time.
Never claim that attaching an old result performed a new lookup.

### 4.2. Transient retention

Retain successful results for **24 hours from successful completion**, with at
most **128 reusable successes per workspace**. The duration and count limit are
explicit user decisions. Use monotonic time for local expiry; cache hits do not
extend the lifetime or change the recorded lookup time. Remove expired reusable
entries during collection access. When inserting beyond the count limit, evict
the least-recently-used success. Lazy cleanup and this eviction order are
engineering choices; no periodic cleanup task or cache settings UI is needed.

A consumer that already holds a result can continue to display it or finish
saving it after reusable-cache expiry or eviction; neither revokes a handle or
deletes a contact's durable snapshot. The cache is memory-only and ends with the
workspace session, so 24-hour retention does not imply persistence across
application exit.

In-flight operations remain joinable until terminal completion and do not count
against the 128 completed successes. Keep the existing bounded provider request
deadlines. As an engineering choice, not-found and failures are delivered to
current consumers but are not retained as reusable negative cache entries. They
do not replace an existing successful display or saved snapshot for the same
eligible key.

Keep the existing Retry directory lookup and Retry directory login actions for
lookup failures and rejected credentials. They are not general successful-result
refresh controls. Retry uses the same keyed operation: reuse a still-valid success
if one exists, otherwise join existing work or admit a request subject to the
existing authentication and provider quiet-period rules. Authentication retry
still resumes the affected provider under D195. Add no refresh action, cache
bypass, scheduled refresh or automatic failure-retry loop.

### 4.3. Configuration, cancellation and shutdown

Scope sharing to the active account workspace. Newly admitted network operations
use the current selected provider and protected material. Admitted handles finish
with their captured provider/material; a late rejection of old credentials must
not pause replacement credentials. Existing native provisioning and retry rules
remain the owners of protected material and authentication recovery. Updated
credentials resume affected lookup under D195 without introducing bulk catch-up.

Changing credentials or provider selection does not erase successful station
information, reset its 24-hour age or give it a new lookup identity. Reused results
retain their actual provider attribution and complete snapshot; do not merge
providers' facts. Only newly dispatched queries use the current provider. This
preserves the existing separation between configuration for new requests and
eligibility of information already obtained. Saved snapshots retain D209
arbitration and key eligibility without a current-provider gate.

Clearing/changing an entry cancels its debounce and detaches its display interest.
It does not cancel another consumer's handle for the same key. As an engineering
default, already admitted provider work may finish within its existing deadline
and warm the transient cache after the last display consumer detaches. No
reference-count cancellation protocol is needed.

Receiver-only reconfiguration does not reset workspace lookup state or wait for
network enrichment. Workspace shutdown stops admitting directory work, cancels
unfinished provider/enrichment tasks and joins them before closing the store.
Finish admitted QSO saves under the existing capture/manual-save contract;
directory access must not hold up those saves or become part of #19's pairing
drain. Pending directory enrichment is not durable across process exit. Existing
explicit bulk lookup remains the recovery path for contacts lacking enrichment.

## 5. Status, entry and Worked Before

Decode Status's instance ID and DX call using the existing checked binary
primitives. Structurally decode/skip the fields leading to DX call, including
dial frequency and mode, without applying them to entry or defaults. Zero
frequency and blank/unrecognized mode do not invalidate a structurally readable
status call; they are not being used as native QSO radio facts here. Ignore
unsupported appended fields. Distinguish malformed/truncated input from a valid
empty/null call; malformed input must not clear the entry. No polling command,
heartbeat negotiation or WS control message is needed to consume incoming status.

Deliver received status calls as ordinary ordered events to the shared entry
owner and wake the existing native UI. Preserve repeated events: comparing only
against the receiver's previous WS call would incorrectly suppress a repeated
WS value after a local edit. Process status through lifecycle polling, including
while the workbench is hidden, rather than depending on a particular pane being
drawn. The receiver continues to process completion messages independently.

For each event, apply the received call using the entry's trim/uppercase
conventions and preserve slash components. Compare it with the current effective
entry call. If unchanged, do not restart debounce. If changed, replace only the
call, invalidate affected entry validation and follow the same entry-call-change
path as a local edit. Keep other unsaved fields and active defaults unchanged.

Worked Before continues to observe `contact_call()`. It uses literal recorded-call
prefix matching within the selected view, not directory-key normalization. Empty
input clears it. Directory demand observes that same entry field through its own
500 ms debounce; provider/cache timing does not control Worked Before.

An effective change from `W7HEX` to `W7HEX/P` restarts the entry debounce even
though both demands can reuse the same directory key/result. A local edit that
WS later replaces also restarts debounce. Do not reduce effective-call comparison
to `LookupKey` equality.

At debounce expiry, obtain the key's shared handle. Apply presentation only to
the entry consumer still requesting that key. Empty input or Clear removes its
pending handle and displayed directory information immediately; old responses
cannot restore that display. Retaining same-key successful information while a
new demand is debouncing remains allowed by the existing eligibility rules.

An admitted manual save retains its immutable submitted facts. Read-only form
controls prevent user edits during that save; incoming WS status still follows
the authoritative call-update rule. Updating the displayed entry call cannot
rewrite the admitted payload or its directory key. No status deferral queue or
replay-after-save behavior is specified. Successful save performs the agreed
fresh-entry reset. A failed save preserves the other submitted form values and
any newer authoritative incoming call rather than restoring an obsolete call.
This applies the existing independent submission and incoming-call requirements;
it does not permit changes to the QSO that was actually submitted.

Do not add multi-instance arbitration or source-selection UI. Received events
use ordinary arrival order; source identity remains relevant to #19 capture
recognition, not to directory-key sharing.

## 6. Independent capture and result application

For each usable capture candidate, derive the directory key from its projected
QSO's actual contacted callsign. Independently reuse an unexpired success or join
an in-flight lookup for that key. This does not depend on the entry, displayed
call, status source, manual draft ID or manual corrections. If neither exists,
save without directory information. D91 establishes entry-driven lookup; D95
establishes retention of pending results, not a new completion-driven request
trigger. This specification therefore adds no provider query on a capture miss.
User-triggered bulk lookup remains available under D99 for missed enrichment.
Provider absence/failure never prevents saving.

If success is already available, pass it to the ordinary atomic contact/snapshot
save. If a request is in flight, save without its result and retain its handle
alongside that capture operation's existing QSO ID. On a miss there is no handle
or enrichment continuation. Do not await directory completion before saving.

After a successful save with a joined request, a ready result is applied
immediately; otherwise an ordinary workspace-owned continuation awaits that
handle and applies its success through `directory::snapshots::apply`. A result
that completes during the database save remains available through the handle.
Enrichment continuation
is separate from the receiver's pairing/save drain. Release the pairing candidate
and mark successful capture when the contact save commits, regardless of lookup.

The target identity comes from the actual capture save. At application the store
checks that this QSO remains live and its current lookup key matches. Changes to
other recorded fields do not invalidate eligible directory information. A failed
capture save installs no snapshot; dropping its handle does not affect entry or
other captures. A directory persistence failure is reported separately from a
successful QSO save, without falsely reporting the contact as uncaptured.

Multiple captured contacts with the same directory key can independently retain
the same lookup result. That is reuse of station information, not contact
deduplication or a search for matching saved QSOs. Apply only to explicitly
requesting callers' QSO IDs; never scan history and enrich every matching call.

Manual save and saved-contact editing use the same facility. Freeze the requested
key and retain the handle with the actual submitted QSO through its save outcome.
Use ready success atomically or apply after commit. Resetting/changing the entry
ends only display interest.

If a contact is submitted before its entry-call debounce expires, preserve that
original due time in the submitted contact's consumer work. At that deadline,
obtain the key's shared lookup handle even if the save is still running or the
entry has already reset. Saving does not move the deadline or bypass debounce.
Use ordinary workspace tasks/timers for this consumer work; no provider request
exists before admission, and the shared directory collection remains keyed only
by lookup key. A successful result can wait in the consumer's handle until its
save commits. A definitively failed save releases that contact's interest and
cancels any still-unadmitted demand; an unsuccessful manual entry retains its
own lookup/display behavior for correction or retry. If the key was already due
at submission, join/obtain its operation immediately. This also covers a saved
contact edit submitted before its changed-call debounce expires. Do not wait for
either the timer or directory result before acknowledging the QSO save.

Opening an unchanged saved contact still displays its retained snapshot without
requesting refresh; edited calls and explicit retry follow their existing
triggers. Remove the old form-keyed request ownership and its global
`Saved`/`Deleted` lookup lifecycle commands as their consumers migrate.

## 7. Clear and shared reset

Place **Clear** immediately to the left of **Save QSO** in the fixed entry header.
Keep both reachable while the form scrolls, with existing shared styling. Clear
does not depend on form validity and is disabled only while that form's save is
pending, matching the existing read-only submission boundary. No confirmation
dialog or new keyboard shortcut is proposed.

Extract/reuse the existing post-save reset as one fresh-entry operation shared by
Clear and successful manual saving:

- Clear contacted callsign, recorded location, operator/name, references, reports
  and comment; clear entry-specific directory display, validation and feedback.
- Retain primary and receive frequency/band and mode.
- Restore current applied own-station/operator/power/equipment values and the
  current view's default membership, dropping contact-specific corrections.
- Return both timestamps to live operation, assign a fresh draft QSO identity
  and focus Callsign.

Clear makes no save request or saved-data change. Worked Before clears because
the entry call is empty. Incoming status can populate the call again under the
ordinary authoritative rule; Clear does not pause following. Reset does not
cancel directory handles retained by already submitted/saved contacts. Apply
the action to the shared entry, including any additional view of that entry;
do not add a second draft.

## 8. Observability and acceptance

Preserve the production telemetry contract. Count actual provider operations and
HTTP requests once, not once per handle or cache hit. Reusing results preserves
lookup-ID correlation; QSO IDs appear only at consumer save/application boundaries.
Trace cache reuse/in-flight joining at debug level and use existing lookup,
capture and snapshot spans/events for outcomes. Ordinary receiver failures and
capture commits retain their existing levels. A malformed status uses bounded
diagnostics; normal status traffic does not generate info-level packet logs.
Do not log raw status packets, callsign payloads, directory facts or credentials.
Do not add source IDs, QSO IDs or lookup keys as metric labels.

Define acceptance around concrete Clacks decisions and wiring:

Each check must identify the agreed behavior and a specific Clacks mistake it
can expose. An unperformed check is not itself a risk or an acceptance gate.
Use existing coverage where it establishes that behavior; do not require a
separate manual or native-platform repetition without a concrete gap. Apply
general UI guidance to the affected Clacks behavior, not as an exhaustive list
of states, window conditions or external applications to exercise.

1. Fixed independently authored Status bytes establish envelope/field decoding,
   empty/null call handling, ignored appended fields and malformed-input safety.
   Include zero frequency and unfamiliar/blank mode to establish that unused
   radio fields do not acquire native-QSO validation requirements.
   Reuse existing completion protocol coverage rather than duplicating it.
2. Exercise a real shared entry with local edits and incoming status: repeated WS
   authority, unchanged packets not restarting 500 ms, empty clearing, slash-form
   changes with equal directory keys, preservation of other fields, and lifecycle
   delivery independent of which Clacks pane is shown. This catches processing
   incorrectly placed inside a pane's rendering path; it does not require a
   separate minimized-window test. Worked Before receives only the entry field and
   remains local/provider-independent. Extend existing semantic UI and controlled
   clock cases rather than creating a parallel form harness.
3. Use existing local provider fixture endpoints and real Clacks lookup APIs to
   show concurrent entry/capture demand issues one actual request, reuses one
   lookup identity/time, and handles 24-hour expiry, 128-result eviction, existing
   failure retry and provider/material changes. Test Clacks's keying and dispatch decisions,
   not `Shared` cloning or mutex correctness in isolation.
4. Real capture/save/read cases cover ready success, success during save, late
   success after entry changes/Clear, cache miss without a new request, save
   failure where ordinary real inputs permit it, QSO deletion and key-changing
   edit before application,
   and two distinct QSO IDs consuming one result. Include manual save/edit before
   debounce expiry: demand occurs at the original deadline, including during a
   pending save or after reset, without delaying save acknowledgment. Preserve
   recorded facts and current operating defaults. Verify saved snapshots through
   existing public operations and their ordinary synchronization path. No test
   SQL or failpoints.
5. Clear and successful save share the same reset behavior. Verify no save request
   on Clear, invalid-form availability, fresh draft identity, timestamps, retained
   radio fields, current defaults/membership, focus and pending-save disablement.
   Include incoming status during a pending manual save: the field changes,
   the admitted payload stays fixed, and reset/failure follows section 5.
6. Exercise received WS Status through the Clacks receiver, shared entry and local
   Worked Before, using independently authored protocol packets. Assert Clacks's
   decoded-call mapping and entry-driven history behavior. External GridTracker/WS
   forwarding and installation settings are not Clacks acceptance gates. Use the
   controlled provider and local-save scenarios above to establish that directory
   access does not delay or prevent capture; do not require live external outages.
7. Inspect the assembled production UI for the new Clear control immediately left
   of Save QSO, its use of the existing shared styling, and Callsign focus after
   reset. Use the interaction cases above for invalid-input availability,
   pending-save disablement and shared-entry reset. These checks expose incorrect
   Clacks layout, action wiring or reset ownership. Use isolated development
   workspace/configuration/log paths for native inspection. Resizing, minimized
   windows, unchanged failure/fallback visuals and additional platforms are not
   separate #21 gates without an identified Clacks concern. Existing #19 receiver
   platform checks do not become new #21 gates merely because capture is reused.
8. Verify actual-request counts, reuse correlation and failure/commit reporting
   through configured production telemetry, with bounded labels and safe data.

Run relevant existing suites and required repository checks during implementation.
This specification itself adds no executable code and claims no runtime evidence.
Assess #21 delivery against the Clacks behaviors above, not an inventory of
unperformed checks. Planning documents alone do not establish delivery.

## 9. Contract integration and builder discretion

On approval, integrate the new agreed entry/Clear behavior into the owning product,
manual-logging and desktop-UI contracts. Replace form-owned directory lifecycle
wording in the directory specification with key-based requests and caller-owned
application. Update #19's paragraph that requires prior exact-QSO association:
capture now obtains eligible directory information independently by key and then
uses its own QSO ID to retain it. Likewise remove any implication in product or
WS research wording that a completed contact must be correlated with a status
selection. Preserve source findings and the accepted independent capture model.

Those are changes to application integration, not native snapshot semantics or
historical protocol compatibility requirements. No schema migration is currently
justified. The builder chooses focused module/type names, concrete shared-handle
aliases, channel capacities and task placement using existing facilities. Keep
lookup ownership by key, consumer application by actual target identity and
workspace lifetimes explicit. Do not introduce a generic broker, custom executor,
durable enrichment queue, periodic cache service or tests solely to preserve the
replaced form-owned implementation.

## Sources

- [Issue #21](https://github.com/jhelwig/clacks-log/issues/21),
  [decision register](../../decisions.md), [product](../../product.md),
  [history](../../history.md) and [directory design](2026-11-02-directory-lookup-design.md).
- [GridTracker source investigation](../../../references/gridtracker-selection.md)
  and [WS protocol investigation](../../../references/ws-suite-udp.md). These
  document inspected source, not a capture of the installed applications.
- [Pinned WSJT-X protocol header](https://github.com/WSJTX/wsjtx/blob/b4f9a431bcf6449df8f37b56de79d48b665b044c/Network/NetworkMessage.hpp),
  Status type 1 and its field order; consulted for the approach comparison.
- [`futures-util` 0.3.34 `Shared`](https://docs.rs/futures-util/0.3.34/futures_util/future/struct.Shared.html),
  checked against the locked local implementation, including its polling and
  output-sharing behavior.
- [Tokio mutex guidance](https://docs.rs/tokio/1.54.1/tokio/sync/struct.Mutex.html),
  checked in the locked local source; ordinary channels, task sets and
  cancellation integrate with the existing desktop workspace runtime.

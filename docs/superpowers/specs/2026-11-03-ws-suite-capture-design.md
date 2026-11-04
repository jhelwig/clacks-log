# WS Suite completed-contact capture

Approved engineering specification for [#19](https://github.com/jhelwig/clacks-log/issues/19),
approved by the user on 3 November 2026 after the receiver-configuration update.
The pairing behavior below was agreed during brainstorming. The other concrete
choices are engineering resolutions, not additional project-wide product
requirements. Implementation planning follows this approval; the capability
issue remains open until delivered behavior and acceptance evidence exist.

WS Suite means the WSJT-X family of weak-signal applications. UDP is User
Datagram Protocol; ADIF is Amateur Data Interchange Format, whose length-framed
text syntax is ADI. UTC is Coordinated Universal Time. A radio contact is a QSO.

## 1. Outcome, authority and boundaries

One configured Clacks desktop receives completed contacts from a WS source,
saves each completion once locally, and distributes saved contacts through the
existing account synchronization. WS and Clacks normally run on the same
computer. Remote reception remains possible; it does not justify discovery,
handoff, leases or distributed capture ownership.

| Requirement or decision | Authority |
| --- | --- |
| Prefer type 12 ADIF; structured type 5 fallback; one QSO for paired/repeated completion messages | D79; [product](../../product.md#live-contact-capture) |
| Supplied station facts win; missing/empty live station context uses capturing-client defaults; never change those defaults | D28, D80 |
| One capturing client per source; other clients receive QSOs through normal sync; server access is unnecessary for capture | D81 |
| New contact receives the selected named logbook's tag, or none in All contacts/Untagged | D303, D309; [logbooks](../../logbooks.md) |
| Directory access never holds up capture; provider facts remain separate and associated with the correct QSO | D95, D203–D204; [directory specification](2026-11-02-directory-lookup-design.md) |
| Configure and enable UDP reception through the desktop UI, including unicast and multicast; editing a file is not a prerequisite | Explicit user requirement in the #19 design discussion |
| Use existing native values, local storage, shutdown and observability contracts | [native model](../../native-model.md), [client store](../../client-store.md), [desktop](../../desktop.md), [observability](../../observability.md) |

The following decisions were accepted in this discussion and are preserved:

- The first completion message starts a **100 ms pairing window**, regardless
  of which type arrives first. Keep a pending candidate, rather than save the
  first representation immediately.
- A matching counterpart upgrades that candidate to type 12 and ends the wait
  early. Both received types produce one save.
- At timeout, save the available representation. Repeats of the same type
  neither count as a counterpart nor restart the timeout.
- Keep the latest completed identity per source **until the next completion**.
  Suppress subsequent matching messages of either type; do not automatically
  rewrite the saved QSO from a late counterpart.
- This state is in memory. No persistent capture receipts or arbitrary old
  replay guarantee is established.

#21 owns WS status-driven call selection, lookup, Worked Before and the
GridTracker Generate Messages workflow. #19 supplies their actual receiving
boundary without implementing dormant status engines or replacing manual entry.
Automatic missed-contact catch-up, background/tray operation, rig control,
historical file import, automatic tag routing and external-service publication
are outside this delivery. Closing Clacks stops capture.

## 2. Approaches and selected direction

| Approach | Fit and cost |
| --- | --- |
| Select type 12 or type 5 per source | Eliminates paired-format duplication, but requires configuration and supplies no per-completion fallback when type 12 is absent. Not selected. |
| Hold whichever arrives first, prefer type 12, save once | Selected in this discussion. Needs the 100 ms candidate and recent completion identity. Saves early when the pair is present. |
| Save type 5 immediately, then update from type 12 | Needs a second mutation and rules for intervening changes. No requirement justifies that cost. Not selected. |

Accepting and saving the first message without pairing would normally select
type 5 in the inspected sender, defeating D79's ADIF preference.

## 3. Evidence and existing facilities

The [WS protocol investigation](../../../references/ws-suite-udp.md) is the
starting evidence. The official WS 3.2.1 build 261010 source was re-inspected
for this specification. In its nested `ws/` source:

- `widgets/mainwindow.cpp`, `MainWindow::acceptQSO`, calls `qso_logged` and then
  `logged_ADIF` consecutively for the same accepted logging values.
- `Network/MessageClient.cpp` serializes both through the same socket and
  application instance `Id`. Neither has a per-completion event ID or sequence.
  The send path has no completion acknowledgment/retransmission loop.
- `logbook/logbook.cpp`, `QSOToADIF`, writes six-digit start/end times and six
  fractional MHz digits. FT2, FT4, FST4 and Q65 use MFSK plus SUBMODE. It returns
  Latin-1 ADI bytes, even though the enclosing protocol describes text as UTF-8.
- `Network/NetworkMessage.hpp` specifies big-endian Qt serialization, schemas
  2/3, nullable length-prefixed byte arrays, and ignoring unknown messages and
  added trailing fields. Type 12 is a binary wrapper around ADI, not a raw
  plain-text datagram. The separately configured secondary ADIF output is not
  this capture path.
- `widgets/logqso.ui` sets both time editors to UTC. Qt's `QDateTime` stream
  operators use the same date/time/specification layout for versions 5.2 and
  5.4, with an additional signed offset only for fixed-offset values.
- The Field Day CONTEST_ID emission is gated by the local
  `config_->special_op_id() == FIELD_DAY` setting and the received-exchange
  branch. CLASS and ARRL_SECT are extracted from the received exchange; the
  contest label is the fixed local activity. `mainwindow.cpp` separately
  obtains our sent Field Day exchange from `m_config.Field_Day_Exchange()`.
  The source establishes the local operating activity rather than requiring
  a new user choice of which station owns that activity.

These are source findings, not measurements of packet timing or a capture of
the operator's installed settings. The agreed 100 ms is an application choice,
not a claimed network guarantee.

Existing integration points inspected for this draft:

- `clacks-desktop::session`: owns the Tokio runtime work, workspace lifetime,
  background-task joins, local-change notifications and graceful close.
- `clacks-client-store::settings::snapshot`: reads applied operating facts,
  selected view, tag roles and reference definitions consistently.
- `clacks-model::contact::{input::QsoFields,Qso,BandFrequency}` and scalar,
  location, position and mode types: native construction and validity.
- `clacks-client-store::save::contacts::save`: ordinary transactional QSO save
  with an optional already-available directory snapshot; retrying identical
  creation under the same QSO ID is already recognized as unchanged.
- Locked Tokio 1.54.1 supplies UDP receive/readiness, `try_recv_from`, monotonic
  deadlines, cancellation and task ownership. Locked winnow 1.0.3 supplies
  checked binary numbers, length-taking and textual parsing. No additional
  runtime, write queue or parser framework is needed.
- The shared `menus::MenuCommand` route, `ui::dialog`, `ValidationState`, themed
  controls, field feedback, opener-focus restoration and asynchronous session
  requests already supply the settings interaction pattern. Appearance shows
  pending Apply/Cancel behavior; logbook/settings requests retain actual
  asynchronous outcomes independently of whether their dialog remains open.
- `clacks-client-store::local_state` supplies typed settings modules' existing
  key/value persistence boundary. Receiver preferences need no new table.
- Locked socket2 0.6.5 supplies pre-bind reuse options and multicast membership;
  create a nonblocking standard socket and pass it to Tokio `UdpSocket::from_std`.
  Tokio alone does not expose the required pre-bind sharing configuration.
- Examined network-interface 2.0.5 supplies interface names, addresses and
  current system indexes on Windows/macOS/Linux. Use its enumeration for the
  interface chooser instead of implementing platform enumeration or a registry.
  Socket2 and network-interface become direct workspace-managed dependencies
  only when implementing their actual receiving/UI consumers.

The existing `clacks-model::service::adif` module projects semantic output fields;
it does not parse incoming ADI. The examined `adif` 0.1.3 parser splits around
record markers and parses values with a regex rather than consuming declared
lengths; it also panics on some malformed inputs. It cannot satisfy faithful
length-framed parsing and non-panicking input rejection. Use the selected
winnow facilities for the small WS/ADI input adapter, not a new generic import
framework. Keep transport and capture composition in `clacks-desktop`; place
only genuinely shared native conversion operations in their existing model
modules when they have a delivered caller.

## 4. Desktop receiver configuration and lifetime

### 4.1. Existing settings UI pattern

Add **Settings → WS Suite…** through the existing shared menu command route:
native menus on Windows/macOS and the existing Linux in-window fallback.
It opens one **WS Suite** settings dialog using `ui::dialog`, shared theme,
field controls, `ValidationState`, feedback and opener-focus restoration.
Use ordinary sections and separators, not a new settings shell or card layout.

The dialog has **Receive WS Suite updates**, a **Unicast / Multicast** selector,
the relevant connection fields, and **Apply / Cancel**. A new workspace starts
with reception off and a unicast proposal of `127.0.0.1`, port `2237`.
No arbitrary multicast group is invented; the operator enters the group used
by WS. The mode and connection controls show saved values but are disabled
while the pending receive toggle is Off. Turning it On enables editing; no
socket changes until Apply. Changing fields does not alter active reception.

| Mode | Controls and meaning |
| --- | --- |
| Unicast | **Local address** and **Port**. Default address is loopback; allow a selected local interface address or an explicitly entered wildcard address for remote reception. |
| Multicast | **Group address**, **Port**, and **Network interface**. The group determines IPv4/IPv6; interface choices show the OS name and a matching-family local address. Include **System default**. |

Accept IPv4 and IPv6 literals and ports 1–65535. A unicast address must not be
multicast or broadcast; a multicast group must be in the family's multicast
range. The chosen explicit interface must exist and have an address in that
family. IPv6 link/interface-local groups require an explicit interface; do not
guess a scope. Users select a named/addressed interface, not a numeric index.
In multicast mode, explain that other applications
may join the same group/port while only one Clacks client captures each source.
No TTL, outgoing-interface, packet-filter or port-sharing controls are exposed:
this is a receiver, and the socket options below follow from the chosen mode.

Enumerate local interfaces when opening the dialog, on its **Refresh interfaces**
action and when applying an explicit interface. Run enumeration off the UI
thread using existing background-work facilities. Keep a disappeared selection
visible as unavailable; do not silently select another adapter. Do not add a
network monitor, discovery service, periodically polling interface cache or
automatically rewriting configuration when an address changes.

Use the existing 300 ms live-validation behavior, unapplied-value treatment,
field errors and action summary. Apply revalidates the current input and is
disabled while validation/request processing is pending. Cancel, Escape and
the close button discard unsubmitted edits; clicking outside does not.
An already-submitted Apply retains its actual outcome under the existing
asynchronous-dialog rules; dismissal does not undo a committed setting.
Turning reception off must remain possible even if a former interface is now
unavailable: applying Off preserves the last committed connection values and
does not require their current availability. Exclude the disabled connection
fields and any stale errors from Apply validation in that state. Apply Off
saves only `enabled=false`; unsubmitted endpoint edits are discarded while
the last committed connection remains available for later re-enabling.
No change requires restarting the application, reaching the server or
configuring account-shared defaults.

### 4.2. Client-local persistence and Apply

Engineering choice: use the existing SQLite `local_state` setting mechanism,
with one typed `ws_suite.receiver` value owned by the local workspace. Store
`enabled` plus a named unicast or multicast connection value. Unicast contains
address/port; multicast contains group/port and System default or the selected
OS interface name plus local address. Both are needed to distinguish adapters
with equal IPv6 link-local addresses. Match that pair against current enumeration
when applying or starting reception, resolving the current index for IPv6
membership. A missing or ambiguous match is an actionable unavailable-interface
error, not permission to guess another adapter. Do not persist an interface
index as a durable identity. Closing/reopening restores the saved configuration.

This reuses the workspace's existing immediate transaction and local-setting
read/write pattern. It adds no table, synchronized record, revision counter,
generic settings repository or config-file writer. Appearance's framework
storage remains presentation-only; these receiver settings belong to the
workspace that will receive the contacts. `config.toml` and `--config` retain
their existing startup roles and do not become a second authority for receiver
settings. File editing is never required for this feature.

Apply uses the existing session request/response pattern:

1. Validate the proposed fields, resolving any explicit interface. Invalid
   input leaves saved settings and the current listener unchanged.
2. Save the validated settings in one ordinary local transaction. If this
   fails, preserve edits and the current listener and report the failure.
3. After commit, stop admission on the previous listener, settle its admitted
   candidates under the existing 100 ms/save rules, close it, and start the
   newly configured listener, or remain stopped when disabled. No-op Apply
   leaves a healthy listener alone; when configured reception is failed, it
   retries that saved configuration without a synthetic settings mutation.

Settings persistence and socket activation are distinct outcomes, not a
cross-resource transaction. A bind/join failure after saving means **settings
saved, reception unavailable**. Keep the dialog open with actionable feedback
and its saved values, allowing correction or Apply to retry; do not pretend the
old listener is still active, silently roll back saved settings or introduce
staged listeners and a rollback protocol. A brief gap while changing endpoints
is an ordinary consequence of reconfiguration, not seamless-handoff scope.

The receive checkbox and existing unapplied-change treatment convey the normal
configuration state. Do not add a separate Off/Listening status or repeat the
configured endpoint. Show a separate actionable message when reception fails
to start or continue. Clear that failure after successful activation or an
applied disable. Preserve runtime failure feedback outside a closed dialog
through the existing nonmodal feedback pattern, with a way to reopen
**WS Suite…**. No source/last-contact dashboard or
new permanent workbench panel is introduced. Neither a reception error nor
interface enumeration failure blocks manual logging or synchronization.
An unexpected terminal receive error stops that listener and reports failure;
Apply retries the saved settings. No automatic rebind supervisor is introduced.

### 4.3. Standard socket construction

Use socket2 for construction/options and Tokio for asynchronous receiving.
Choose one address family for the configured endpoint; an IPv6 socket is
IPv6-only rather than implicitly creating a second IPv4 receiving path.

| Mode | Socket setup |
| --- | --- |
| Unicast | Bind the configured local address/port without requesting address/port reuse. A normal bind conflict is an actionable error; never silently choose another port. |
| IPv4 multicast | Set address reuse before binding `0.0.0.0:port`, then join the configured group using the chosen local IPv4 address, or the OS default membership interface. |
| IPv6 multicast | Set address reuse before binding `[::]:port`, then join the configured group using the resolved interface index, or index zero for a suitable OS-default scope. |

For multicast on macOS, also use the standard `SO_REUSEPORT` option before
binding so other local receivers can share the port. Do not use Linux's
reuse-port load-balancing option. On Linux, disable `IP_MULTICAST_ALL` or
`IPV6_MULTICAST_ALL` for the multicast socket so it receives its own joined
group rather than other groups joined elsewhere on the machine. These socket2
options require its `all` feature and target-specific application, not new
platform networking wrappers. Report construction/bind/join errors truthfully.

Keep same-computer multicast delivery enabled. On Windows, multicast-loopback
options affect reception; on Unix they affect the sender. Enable the applicable
loopback option on Clacks's socket and document that WS must send on the same
interface with local delivery enabled. Clacks does not change WS settings or
rebroadcast its packets to compensate. Joining a group is necessary; merely
binding a group address is not the receiving implementation. Closing the owned
socket releases its membership, with no parallel membership registry.

Multicast port sharing lets Clacks coexist with GridTracker or other receivers;
it does not create multiple capturing Clacks clients or an account-wide capture
ownership protocol. Configure WS's primary binary UDP destination, or an
existing forwarder of that unchanged protocol, to match the selected mode.
Do not create a forwarding service, configure WS automatically, or substitute
its secondary plain-ADI output for the primary binary completion messages.

### 4.4. Source and workspace ownership

Within the active receiver, source identity remains `(sender IP address,
protocol Id)`. The source port is routing information, not identity: it can
change when WS restarts. Treat `Id` as opaque and case-sensitive. Independent
instances with the same `Id` on one host need distinct receiving configurations
or corrected WS instance names; there is no identity registry. Only one
receiver configuration is active per workspace, not an independently managed
listener for every observed source.

Start enabled reception after the intended workspace opens and saved settings
load. Keep its lifetime attached to that workspace; stop and join capture
before enrollment closes/reopens or replaces the workspace and before the
store closes. Retain the latest-completion identities during a receiver-only
settings change in the same workspace; do not restart that state merely to
replace the socket. Workspace exit retains the agreed in-memory boundary.
Graceful-close admission rules also apply to receiver reconfiguration; saves
already admitted are allowed to settle. Explicit isolated workspace,
configuration and log paths remain mandatory for development/validation.

## 5. Decoding, correlation and pairing

### 5.1 Wire decoding

Use a reusable full-datagram buffer large enough for a normal UDP payload
(65,535 bytes). Check magic, schema, type, lengths and available bytes before
reading values. Support schemas 2 and 3 used by the inspected sender; reject
schema 1 and unsupported encodings with a bounded diagnostic. Ignore unrelated
message types. Do not make heartbeat discovery a prerequisite for completion.

Decode type 5's known fields, permitting absent optional appended fields from
older shorter messages and ignoring unknown trailing fields. A truncated field
is malformed; it is not an absent optional field. Required native facts must
still be available. Convert Qt timestamps to UTC seconds using the represented
time specification/offset, never the receiver's local timezone. In both schemas
read a signed 64-bit Julian day, unsigned 32-bit milliseconds since midnight,
and one-byte time specification. Accept UTC (1) and fixed offset (2, followed
by signed 32-bit offset seconds); subtract that offset to obtain UTC. Reject
invalid dates/times and local (0) or named-zone (3) values instead of inferring
the sender's timezone. The inspected WS time editors produce UTC. Drop
fractional seconds consistently with the native whole-second contact model.

Parse type 12 as one length-framed ADI record, allowing its header and EOH/EOR
markers case-insensitively. Consume field values by declared length, so literal
angle brackets or EOR-like text inside a comment are data. Zero-length and
omitted station fields both mean missing for live fallback. Reject truncation,
multiple records or conflicting duplicate fields. Unknown well-framed fields
can be skipped without keeping the raw datagram.

Keep the wrapped ADI as bytes until field decoding. ASCII identity/numeric
fields need no encoding guess. For optional text in this inspected WS dialect,
decode the Latin-1 bytes the serializer actually emits; do not demand UTF-8 of
the whole ADI payload. Type 5's UTF-8 strings retain their supplied text. ADIF
preference does not authorize recovering characters the sender already lost
or mixing type 5 names/comments into the selected type 12 record.

### 5.2 Exact completion identity

Engineering choice: within a source use this named correlation value:

```text
CompletionIdentity
  contacted_callsign: trimmed, uppercase, preserving every slash component
  start: UTC whole seconds
  primary: exact integer Hz when supplied; otherwise normalized supplied band
  mode: uppercase SUBMODE when present, otherwise the supplied mode label
```

For type 5, its mode string is the mode discriminator; thus FT4 matches ADIF
MFSK/FT4 without conflating it with another MFSK specialization. Interpret
decimal MHz exactly into Hz, without binary floating point. A supplied
frequency remains authoritative over BAND through `BandFrequency::new`. Use
the existing exact decimal radio-input conversion, allowing at most six
fractional MHz digits rather than rounding a correlation key.

Do not include defaults, tag selection, own callsign, optional end time, grids,
names, comments, reports or operator identity in the key. These can be absent,
filled from defaults or additional ADIF information. In particular, own-call
fallback must not make otherwise identical source notifications fail to pair.
Do not match by arrival adjacency alone, fuzzy times or callsign/band alone.
This identifies a source completion, not all equivalent contacts in the account.

The inspected sender supplies the same key values to both representations.
Changed key values identify a different completion. A source emitting two
distinct logging actions with identical key values supplies no event ID that
would distinguish those actions from repeated delivery; this design treats
them as one while the identity remains recognized.

### 5.3 State and transitions

| Transient state | Purpose and lifetime |
| --- | --- |
| Pending candidate per received completion | First receipt time/deadline, type 5/type 12 representations, one fresh QSO ID and captured local context. Exists until its save/rejection settles. |
| In-progress save's candidate/QSO ID | Prevent a counterpart or repeat from starting a second save while the ordinary save future is running. |
| Latest completed identity per source | Suppress either message type after successful saving, until the next successful completion for that source. |

Pending candidates may be keyed by identity so distinct contacts or sources do
not overwrite each other's 100 ms wait. This is the pairing buffer itself, not
a durable intake queue. No per-packet history, synchronized source record,
capture table, schema migration or new synchronization scope is introduced.
The persistent receiver preference in section 4 is configuration, not retained
completion-processing evidence.

1. Decode enough to identify the source/completion. A message matching the
   latest completed identity or a saving candidate cannot start another QSO.
2. On the first usable representation, allocate one QSO ID, record a monotonic
   deadline 100 ms later, and capture the applied operating context, selected
   view and applicable recognition/definitions from one local read snapshot.
   Do not use an unfinished manual form or its contact-specific corrections.
3. Matching arrivals join that candidate. Keep the first representation of
   each type; identical repeats do not change it or move its deadline. If the
   same type has changed optional values under the same identity, keep the
   first and diagnose the discrepancy; this is not a correction protocol.
4. When both usable types exist, cancel the deadline and finalize immediately
   using type 12 as a whole. Do not merge extra fields from type 5 into it.
5. At timeout, finalize the available usable representation. Type 12 alone
   waits 100 ms too; do not silently restore the rejected immediate-save path.
6. Mark completion only after the ordinary local save reports committed or
   acknowledged unchanged. Replace that source's latest completed identity;
   repeats of it do not refresh a separate retention clock.

The deadline is anchored to first receipt, not completion of the local context
read. Continue consuming socket data while local reads/saves run. When a timer
becomes ready, process already-readable datagrams before finalizing, so an
application scheduling pause does not manufacture a missing counterpart. Use
ordinary Tokio readiness/timers and the existing session task facilities;
do not block receiving on a database operation or add a custom write queue.

Recognizing a new pending completion does not immediately forget the previous
completed identity. Replacement happens when the new completion succeeds.
After replacement, restart or process exit, an old notification is outside the
accepted recognition bound. Do not imply persistent exactly-once delivery or
account-wide duplicate capture prevention.

## 6. Native values and live fallback

Build `QsoFields` directly through checked native types; the UI's editable form
is not the incoming protocol model. Require both station callsigns after own
fallback, start, mode and primary frequency or band. End, reports, locations,
power, operator and equipment are optional under the native contract.

| Input meaning | Type 12 fields / type 5 counterpart | Native effect |
| --- | --- | --- |
| Contact identity/time/radio | CALL, QSO_DATE/TIME_ON, optional QSO_DATE_OFF/TIME_OFF, FREQ/BAND, MODE/SUBMODE | Contacted station, UTC start/end, primary pair and preserved mode hierarchy |
| Our station | STATION_CALLSIGN, MY_GRIDSQUARE and other supported MY_ geographic fields / My call, My grid | Prefer supplied own facts; fill missing/empty own facts from captured active defaults |
| Other station location | GRIDSQUARE, STATE and supported geographic fields / DX grid | Recorded contacted location, without own/default or directory enrichment |
| Own operator and power | OPERATOR, TX_PWR / operator call, power | Supplied nonempty values win; otherwise captured active operator/power |
| Other operator name | NAME / Name | Optional recorded contacted-operator name; not a directory snapshot |
| Reports and comment | RST_SENT, RST_RCVD, COMMENT / corresponding fields | Native optional text; reports remain separate from exchanges |
| Receive frequency | FREQ_RX / appended receive-frequency text | Optional native receive override |
| Equipment and own references not supplied | Captured active context | Copy applied values, not profile/configuration identities |
| New-contact membership | Captured selected view | One selected named tag, or an empty set for built-in views |

For the geographic fields with native destinations, support DXCC/MY_DXCC,
STATE/MY_STATE, CNTY/MY_CNTY, CQZ/MY_CQ_ZONE and ITUZ/MY_ITU_ZONE with existing
checked values. A supplied grid is a grid area at its actual precision, not an
invented point. Live own-context fallback is field-by-field under D80; directory
whole-location fallback remains separate and never writes recorded facts.
Do not derive extra geography here; #23 owns geographic assistance. Do not
modify active operating state, reusable profiles/equipment, tags or the manual
entry when a capture commits.

Use native mode construction: ADIF SUBMODE records a specialization with any
supplied parent; MODE alone records a parent. Recognized standalone type 5
specialization labels use the existing catalog to construct their hierarchy.
Unknown nonblank modes remain valid.

For type 12 TX_PWR and type 5 power, parse decimal watts exactly and convert to
integer milliwatts, truncating any precision below one milliwatt toward zero.
For example, `5.0000 W` and `5.0007 W` both produce `5000 mW`; `0.0019 W`
produces `1 mW`. Extra fractional digits are not a validation failure and do
not cause default substitution. Use decimal/integer conversion rather than
binary floating point. Apply the native positive-milliwatt and signed-64-bit
range checks to the converted value. A value that truncates to zero, such as
`0.0007 W`, cannot be represented as positive native power and follows the
invalid-optional-value policy below; do not round it up or invent a minimum.

Do not apply the manual-entry helper's three-fractional-digit text restriction
to capture. Reuse or extract shared decimal conversion where appropriate while
preserving the distinct input policies. The sub-milliwatt truncation rule is
explicitly agreed for capture; it does not change manual-entry validation.

Dedicated exchanges STX/SRX/STX_STRING/SRX_STRING and contest serial assistance
remain post-MVP under D129. PROP_MODE/SAT_NAME/SAT_MODE have no native QSO slots
in the inspected model: report unsupported supplied values as conversion
warnings rather than inventing storage or hiding them in comments. Preserve
ordinary supported facts regardless. Section 10 defines the source-grounded
CONTEST_ID mapping; no contest subsystem is implied.

If optional supplied values cannot be represented, report the field and omit
that invalid value; do not replace a nonempty invalid own value with a default
as if it had been absent. Missing/invalid required values make that
representation unusable. A usable counterpart remains eligible; invalid ADIF
does not prevent a valid type 5 fallback. Structurally malformed packets that
cannot establish identity do not create a candidate or reset another timer.
If neither representation is usable, report the failed capture without making
a QSO or changing the latest completed identity.

## 7. Saving, failures and shutdown

Save once through `save::contacts::save`, using the candidate's stable QSO ID.
Reuse existing local-change notification and contact-read invalidation so
history sees the real local record and enrolled synchronization discovers it.
Do not fan out a save to other clients or gate it on enrollment, server/provider
access, successful lookup or external-upload readiness. Once committed, the
ordinary SQLite and synchronization contracts supply durability and recovery.

The corrected [#21 specification](2026-11-04-ws-suite-selection-design.md)
extends capture with independent directory demand. Derive its key from the
projected QSO: reuse an unexpired shared success, join an in-flight lookup or
start on a miss. Never wait for directory completion before saving. A ready
success enters the existing atomic save; a pending handle stays with that actual
save's QSO ID and is applied only after success through the store's live/key
guards. Entry and Status participate only through optional shared station-result
reuse, never through capture identity or form ownership. Capture does not change
or reset the manual entry. Enrichment does not extend receiver pairing/drain.

A store failure is reported through existing nonmodal error treatment and
structured diagnostics; do not mark it captured. Retain the candidate's ID
through the save operation and any retry already performed by that operation.
After terminal failure reporting, release that candidate. No separate failed
capture cache, automatic retry loop, durable backlog or recovery screen is
introduced. A subsequent notification is processed under the same admission
rules. Show failure distinctly from a successful save followed by a
history-refresh error. Manual ADIF import is the existing recovery path for
contacts not saved; it is not automatic catch-up.

On graceful close or a workspace transition, stop admitting new identities,
allow counterparts for already admitted candidates until their original
deadlines, finish those saves, and join capture work before closing the store.
The added pairing wait is at most the remaining part of 100 ms; actual saves
retain the existing shutdown wait/force-quit behavior. A crash or explicit
force quit before commit can lose a pending candidate. The agreed in-memory
pairing model does not promise durable receipt of every socket read.
Apply's receiver replacement uses this same drain boundary rather than adding
a second shutdown/reconfiguration state machine.

## 8. Observability and acceptance evidence

Use existing tracing, logs, metrics and feedback components. Record receiver
configuration save, start/bind/join failure/stop, committed capture, rejected
capture and store failure
at ordinary diagnostic levels with safe source/QSO correlation. Pair selection,
repeat suppression and ignored protocol types are debug/trace detail. Do not
log raw packets, whole contacts, arbitrary comments or provider credentials.

Measure finalized outcomes and first-receipt-to-finalization duration; count
durable saves only after successful commit. Metric labels are bounded format,
outcome and error categories, never instance IDs, callsigns or QSO IDs. A failed
capture must be visible without debug enabled. This does not add an unaccepted
source dashboard, last-contact panel or background-capture feature.

Acceptance must establish Clacks's decisions and wiring, rather than test
Tokio timers or SQLite durability in isolation:

1. Independently authored fixed wire examples for types 5/12, schemas 2/3,
   exact field decoding, mode equivalence and supported shorter messages.
   Include length-delimited comment text and the inspected Latin-1 output.
   Fixture tools, if needed, are Rust; do not execute upstream code.
2. Controlled-clock pairing scenarios: both arrival orders; save before 100 ms
   once both arrive; either type alone saves at 100 ms; same-type repeats do
   not extend the wait; queued counterparts are handled before timeout.
3. Real QSO save/read scenarios for supplied/missing/empty own fields, stable
   context/tag capture, one QSO from a pair, invalid-ADIF/valid-type-5 fallback,
   recent counterpart/repeat suppression, and replacement of the latest
   completed identity. Repeat while a save is in progress must not allocate
   another QSO ID. Include the Field Day association mapping, respecting
   binding conditions and counting distinct matching groups rather than
   bindings. A contacted-only mapping must neither create an own association
   nor introduce ambiguity against an eligible own mapping. Verify that
   received exchange fields do not become our station's geographic facts.
   Verify power conversion with redundant fractional zeros, sub-milliwatt
   truncation (`5.0007 W` to `5000 mW`), and native zero/range rejection.
   Account-history matching is not part of these tests.
4. Production runtime reception through an actual loopback socket and the real
   workspace save path, with server/provider unavailable. An enrolled capture
   must reach another client through the existing synchronization workflow.
   Demonstrate a distinct bind conflict without stopping ordinary local work.
   Exercise multicast with a second ordinary receiver on the same group/port:
   both receive the packet and Clacks saves one QSO. Verify the chosen interface,
   IPv4/IPv6 membership, same-computer delivery and group isolation using the
   actual socket configuration. Windows multicast sharing/local delivery need
   native Windows evidence, not an assumption from Unix behavior.
   Ordinary automated checks cover IPv6 configuration and socket construction;
   they do not establish packet-delivery acceptance. Run IPv6 delivery,
   coexistence, group isolation and capture explicitly on native hosts using
   an existing IPv6 interface, with interface-local test traffic. Missing
   prerequisites or failed delivery remain an explicit platform verification
   gap. Do not provision adapters or routes solely to preserve a test setup.
5. Close with a pending candidate, finish its save, reopen the isolated
   workspace and read the contact. Preserve the in-memory-only recognition
   boundary; do not add restart/deletion replay guarantees to the suite.
6. Render and inspect any added error feedback in the assembled application
   under the existing desktop UI gates. Keep Windows runtime evidence distinct
   from macOS/Linux automation and cross-compilation.
   Exercise Settings → WS Suite through the real command route, both mode forms,
   interface selection/refresh, validation, Apply/Cancel, enable/disable and
   reception failure feedback. Verify that normal states use the checkbox and
   unapplied-change treatment without a separate reception status or endpoint
   summary. Verify effective rebinding without application restart,
   no mutation on Cancel/invalid input, and settings restored on workspace
   reopening. Distinguish saved-settings success from bind/join failure; retry
   of unchanged failed settings must not fabricate a settings mutation.
   Applying Off must work with an unavailable saved interface or stale field
   errors. Resolve explicit interfaces by name/address, including equal
   link-local addresses on different interfaces, without persisting indexes.
7. Verify ordinary-level failure/commit diagnostics and bounded metric labels
   using the production telemetry path and configured filters.

Integration cases extend existing suites and production APIs. Use temporary
workspaces and disposable test databases. No test-only SQL, failure triggers,
unused engines or dormant capture storage establish delivery. #19 remains open
until the actual desktop workflow and its acceptance evidence are delivered.

## 9. Builder discretion

The builder selects module boundaries, named candidate structures and ordinary
Tokio task/channel composition, keeping ownership explicit and the receiver
responsive. The existing store and native representation remain owners of
their invariants. Persist receiver configuration through existing local-state
facilities only; no separate persistent capture-processing state is authorized.

Resolve routine parsing, snapshot reads, save notification and lifecycle wiring
within this specification. A new question is warranted only for a demonstrated
contract contradiction or a materially different observable behavior.

## 10. Source-specific CONTEST_ID mapping and remaining ambiguities

For the inspected live WS Field Day path, map
`CONTEST_ID=ARRL-FIELD-DAY` to a group-only association on **our station**.
Resolve the supplied external contest code through the account's current group
definitions, not through a hard-coded seeded group ID or mutable display name.
The existing definitions express that code as a literal ordinary-interchange
CONTEST_ID binding. A matching binding must also be applicable to the proposed
own-station association under its existing `BindingCondition`:

- `Always` is eligible: source semantics supply the own-station assignment.
- `AssociationPresent(Own)` is eligible: the proposed association satisfies it.
- `AssociationPresent(Contacted)` is not eligible for this own-side mapping.
- `BoundPresent` is not eligible: this direct capture lookup has no export
  partition/expansion bindings.

Find the unique distinct group with an eligible matching literal and identifier
rules permitting group-only participation, then use that group's actual ID to
construct the native association. Multiple eligible bindings within one group
still produce one candidate. Ineligible bindings do not create candidates or
ambiguity. Keep contest codes case-insensitive with surrounding whitespace
removed for this lookup. This requires a direct lookup of existing definition
data and condition types, not a general inverse of the export expression engine
or a separate contest registry.

The association has no reference identifier because the resolved group itself
identifies this contest; ARRL-FIELD-DAY is not an identifier inside a generic
Contests group. Add the association to our copied reference set using ordinary
set semantics. Do not replace other active references or infer the contacted
station's association from this field alone.

This is an engineering inference from concrete source semantics: local Field
Day mode selects the contest label, while the adjacent CLASS/ARRL_SECT fields
describe the received exchange. It is also consistent with the existing
Field Day definition in `lib/clacks-model/data/account-definitions.json`, whose
ordinary CONTEST_ID export is conditional on our station's association. It is
not a claim that all ADIF CONTEST_ID values universally belong to our station.

If no eligible definition matches, multiple groups match, or the contest code
is only computed by an expression that cannot be matched directly, report the
unmapped/ambiguous activity field and save the otherwise valid contact without
inventing an association. Do not fall back to a seed ID or create a group while
receiving a contact. The supplied CLASS, ARRL_SECT and exchange strings do not
become our location or a reference identifier; dedicated exchange recording
remains post-MVP under D129.

The native-model contract's section 10.1 still leaves general historical ADIF
import ownership open for that workflow. That does not block this
source-specific live mapping. Other source-supported contest labels use the
same account-definition lookup; there is no Field Day-specific ID branch.

No unresolved product ambiguity remains. Source configuration, exact identity,
validation/fallback behavior, failure reporting, shutdown and the source-specific
mapping above define the approved engineering direction.

## Sources

- [Official WS 3.2.1 build 261010 archive](https://sourceforge.net/projects/wsjt-x-improved/files/WS_v3.2.1/Source%20code/ws-3.2.1_261010.tgz/download),
  nested files identified in section 3; [maintained investigation](../../../references/ws-suite-udp.md).
- [WSJT-X protocol header](https://raw.githubusercontent.com/WSJTX/wsjtx/b4f9a431bcf6449df8f37b56de79d48b665b044c/Network/NetworkMessage.hpp).
- [Tokio 1.54.1 UDP facilities](https://docs.rs/tokio/1.54.1/tokio/net/struct.UdpSocket.html),
  checked against locked local source, plus its standard time/task facilities.
- [winnow 1.0.3 binary facilities](https://docs.rs/winnow/1.0.3/winnow/binary/index.html),
  checked against locked local source.
- [Examined adif 0.1.3 parser](https://docs.rs/adif/0.1.3/src/adif/parser.rs.html).
- [Qt 5.15 date/time stream implementation](https://github.com/qt/qtbase/blob/5.15/src/corelib/time/qdatetime.cpp),
  `QDateTime` stream operators and their explicit version branches.
- [socket2](https://github.com/rust-lang/socket2), checked against locked 0.6.5
  source for pre-bind reuse, membership and Linux multicast delivery options.
- [network-interface 2.0.5](https://docs.rs/network-interface/2.0.5/network_interface/),
  checked against its published source for Windows/macOS/Linux interface
  enumeration. Standard enumeration fills the UI chooser; no discovery service
  or custom platform interface registry is needed.
- [Microsoft socket reuse and multicast delivery](https://learn.microsoft.com/en-us/windows/win32/winsock/using-so-reuseaddr-and-so-exclusiveaddruse)
  and [multicast loopback semantics](https://learn.microsoft.com/en-us/windows/win32/winsock/ip-multicast-2).

# Clacks desktop

Production native desktop composition for [the desktop contract](../../docs/desktop.md).
The `clacks-log` binary is a thin entry point. Local account data stays in
`clacks-client-store`; UI callbacks do not perform database or network work.

## Launch and workspace selection

```sh
cargo run --bin clacks-log -- --help
cargo run --bin clacks-log -- --workspace /absolute/path/to/workspace
```

Without an override, `directories::ProjectDirs::from("", "", "Clacks Log")`
selects the OS-local data directory, with `workspace` beneath it. Configuration
is `config.toml` beneath the corresponding OS application configuration directory.
Use `--config <file>` to select another file. Command-line relative paths use
the launch directory; file-relative paths use the configuration file's directory.
The workspace contains `workspace.sqlite`, SQLite sidecars, `instance.lock` and
(default) `logs/`. Never remove a live instance lock file.

| OS | Default configuration | Default workspace |
| --- | --- | --- |
| Windows | `%APPDATA%\Clacks Log\config\config.toml` | `%LOCALAPPDATA%\Clacks Log\data\workspace` |
| macOS | `~/Library/Application Support/Clacks-Log/config.toml` | `~/Library/Application Support/Clacks-Log/workspace` |
| Linux | `~/.config/clackslog/config.toml` | `~/.local/share/clackslog/workspace` |

Linux respects absolute `XDG_CONFIG_HOME` and `XDG_DATA_HOME` overrides.

A missing database is exclusively created with a fresh native account and the
production default group/output-profile definitions. Existing accounts are
opened without reseeding. Invalid or incomplete files are preserved and reported;
a failed open is never permission to replace the file. After correcting the cause,
Retry reopens it. To start separately, use another `--workspace` directory.
A standalone workspace is not enrolled and has no fabricated server checkpoint.

One process owns each physical workspace lock file for its lifetime. A subsequent
launch sends a bounded activation request and exits without opening the store.
Foreground focus is best effort. Different workspaces can run simultaneously.
Activation failures return an error after three seconds rather than bypassing
ownership. Unix sockets live in a checked user-only directory; Windows pipes have
a protected current-user-only access list. Stale socket cleanup requires the lock.

## Logbook setup

The application-bar **Logbook** selector offers **All contacts**, **Untagged** and named
logbooks. Choosing a view commits it immediately in this workspace's SQLite
database and restores the same tag ID on restart, including after a rename.
Both built-ins supply no default new-contact tag. A named view supplies that tag;
future QSO entry follows subsequent view changes without retaining an old proposed
membership. Saved contacts and existing-contact editors are unaffected.

**Manage logbooks…** opens a dedicated dialog with New/Edit and Save/Cancel
boundaries. A save changes only the tag, never the selected view or station and
equipment settings. New tags use **Neither** for both LoTW and QRZ upload policies.
Each service independently supports Enable, Forbid and Neither; any Forbid wins
when contact eligibility is evaluated across tags. Duplicate names are allowed.
Only duplicate names display a short collision-checked ID suffix; full IDs are
visible and copyable in the manager. A tag named All contacts remains distinct
from the built-in view in the selector's named-logbook group.

Deletion first reads a consistent review showing distinct affected, shared and
unique contact counts. Explicitly choose whether to leave all contacts, delete all
contacts, or delete only contacts unique to this tag, then confirm. A changed tag
or contact makes the review stale and requires reloading and confirming again.
Deleting the selected tag atomically selects All contacts and explains why.
Known incoming deletion uses the same fallback; unknown/unavailable targets and
live pending tag alternatives are preserved instead. External copies are untouched.

Debounced validation and unapplied highlights use the shared controls. Cancel,
Escape and the dialog close discard unapplied edits. Admitted operations settle
even after dismissal. Their exact committed results remain visible if the next
catalog read fails; a failed refresh never retries a mutation. Use **Refresh
logbooks** to retry the read. Saved-contact assignment uses the separate
[membership workflow](../../docs/contact-membership.md).

## Operating settings

The main screen follows the [accepted workbench composition](../../docs/desktop-ui.md):
logbook selection and its manager share the application bar; the operating strip
sits immediately beneath it. **Saved settings** in the application bar opens
station-profile/equipment management and refresh actions. A compact footer reports
local workspace state. **Workspace & diagnostics** expands file locations and
account identity when needed. Opening
failures, retry actions and degraded diagnostics stay visible independently of
that disclosure. These are the existing production tools; no future contact or
history panes are shown as placeholders.

The operating strip shows station/profile, station identity, equipment and
operator values as compact quick-change controls, alongside inline power and
**Operating settings…**. The station, equipment and operator value controls open
narrow Apply/Cancel dialogs. A profile previews its associated equipment consequence;
these controls never edit the reusable source records. Equipment override and
changed-source indicators describe applied context, separately from unapplied edits.

The shared theme supplies both reference palettes, recessed fields/value controls,
thin outlines, small corners and the application/dialog, label, action and radio
type roles. The application installs it on startup and during appearance changes.
Save/Apply actions use shared accent emphasis; ordinary controls keep their native
interaction states. Appearance choices change those visual roles consistently
across the main window and existing dialogs.

Power is directly editable beside its **W** unit. Enter, Tab or leaving the field
commits valid input; Escape restores the active value. Leading-decimal input such
as `.5` is accepted. Blank clears optional power. Invalid input and failed writes
retain editable text and leave the active value intact. A late acknowledgment
does not replace newer typing. Before station setup, only a complete profile or
the full operating editor can establish valid operating values.

If a newer value is explicitly committed while a previous power write is pending,
it applies once after a successful acknowledgment, using the accepted basis.
Further typing or Escape cancels that waiting intent. A failed or ambiguous
earlier request leaves the text available for review and explicit retry.

After the workspace opens, the active summary shows copied station/equipment
values and their independent optional source selections. Open **Operating
settings** to edit one pending context across **Station**, **Equipment** and
**Operator / power** views. **Apply** saves that context locally; **Cancel
operating changes** discards it. No unfinished form is persisted.

Loading a profile replaces its station values as a whole. Its associated default
equipment loads with it; a profile without a default leaves equipment unchanged.
Operator and power remain independent. Loading a configuration changes equipment
only. A manual station/equipment edit clears that component's selection; matching
the old values later does not reattach it. Explicit loading reattaches a source.
Source changes show an indicator while applied/copied values remain unchanged.

**Station profiles** and **Equipment configurations** open separate reusable
record lists and editors. Profile editors show their associated equipment
configuration explicitly and can open equipment management while preserving the
unfinished profile. Save affects only the displayed reusable record; it never
selects, loads or applies the saved result. Cancelling an outer editor does not
undo an inner Save. Saving operating values as a reusable record opens the same
editor with an explicit new-versus-existing target choice. Names can repeat;
station context and native IDs distinguish saved records.

Station entry includes callsign, grid area or explicit coordinates, manual
geographic corrections and account-defined references. Own-station locators
require 4–12 characters. Transmit power uses watts with up to three decimal
places. Geographic autofill, QSO entry and the general synchronization/conflict
review UI remain separate capabilities.

Equipment deletion first reviews every referring profile. Choose a replacement
or remove its default for each, then confirm the atomic deletion. Changed review
inputs require another explicit review. Deleting a selected source clears only
its selection, preserving copied operating values and historical contacts.

Writes run on the application runtime. Failed or stale saves preserve form text;
competing saved values are shown before an explicit retry. If a profile saves
with an unavailable default, it stays open with saved-local/missing-reference
feedback and replacement/None choices. A failed read after a completed write is
reported separately: the committed result stands and the previous summary is
marked stale until **Refresh settings** succeeds. An acknowledged Apply supplies
the summary and next editor's operating basis even when that read failed. Do not repeat the write merely
to refresh its view. Confirmed deletions remain authoritative while that view is
stale: deleted sources are excluded from load/target choices, and new operating
forms start without their selections. A later successful read of a live working
source can establish its availability again. Graceful shutdown drains admitted
writes.

Accepted profile/equipment saves also retain their exact returned records after a
failed view read. The saved record remains available with its acknowledged save
basis, including when its editor was dismissed, while the remaining view is
marked stale. Recovery refreshes that view without repeating a successful save.

Editors use scrollable modal dialogs. Close, Escape and Cancel discard that
form's unapplied edits without another prompt; backdrop clicks do not discard it.
Escape closes an open menu before the editor, and dismissal returns focus to its
opener. Admitted writes keep their response owner after dismissal and finish
normally. A failed or ambiguous acknowledgment remains visible; it never causes
an automatic write retry. Existing application shutdown drains admitted work.

Fields validate after 300 ms without typing. Save/Apply waits for current valid
input. Amber fill, an edge and **Not applied** distinguish edited values from
the native focus indicator and field errors. Reusable-record validation remains
independent of unrelated pending operating values.

## Appearance

**Appearance…** provides System (default), Light and Dark choices, unified UI
scaling from 75% through 200%, and separate installed-family choices for ordinary
UI and radio data. Defaults use bundled proportional UI and monospace radio
fonts. Valid changes preview immediately; Apply retains them and Cancel restores
the committed appearance. Reset scale restores 100%. Scaling composes with native
display scaling and does not alter recorded values.
Apply, Cancel, close and Escape return keyboard focus to the Appearance opener.

Preferences and native window geometry are workspace-local in the `desktop-ui`
file, written through eframe's native storage on its save lifecycle. They are
not synchronized. Installed families are discovered and loaded on bounded
session-owned workers. Missing or failed selections retain the requested family,
show a notice and use bundled fallback; **Retry font loading** explicitly retries.
Choosing a font does not change text, validation or operating-source selections.

## Configuration

Command-line options override supported environment values, which override file
values, then defaults. Signal-specific options win within their own precedence
tier; a generic CLI endpoint still overrides an environment-specific endpoint.
Unknown file keys, invalid supplied values and missing explicitly selected files
are errors. A missing default file is normal.

The `config` crate provides source merging, environment collection and typed
deserialization. Clacks maps the standard logging/OpenTelemetry names, resolves
path origins and validates the selected values. Header dictionaries merge by
header name; higher-priority sources replace matching values. Configuration
errors identify settings or source categories without echoing values or excerpts.
Defaults belong to the typed settings through Serde's default handling.

```toml
# Optional workspace path, relative to this file.
workspace = "local-workspace"

[logs]
filter = "info"
console = false
directory = "diagnostics"
retained_files = 7
buffered_lines = 8192

[telemetry]
enabled = false
endpoint = "http://localhost:4318"
timeout_ms = 5000
# headers = { authorization = "Bearer ..." }

# Signal-specific full endpoints, headers and timeout_ms are optional.
[telemetry.traces]
endpoint = "http://localhost:4318/v1/traces"
```

`--console` and `--otlp` enable console reporting and telemetry export respectively.
Use `--console=false` or `--otlp=false` to override enabled file settings; optional
Boolean values require `=`. Export endpoints use the separate `--otlp-endpoint`
and signal-specific endpoint options. See `--help` for log controls and generic/per-signal
export endpoints. Header values are configured in the file or standard environment
settings, never in diagnostic fields. Protect configuration file permissions when
it contains collector credentials.

## Diagnostics and export

Default output is daily rotating JSON Lines at `info`, with seven matching files
retained. The standard off-thread writer has an 8,192-line lossy queue; overload
can drop diagnostics rather than block storage or drawing. The UI reports writer
loss and initialization degradation. File initialization failure falls back to
stderr; console output can also be selected explicitly. Shutdown retains the
writer guard until producers and exporters finish.
If an initialized file later fails to write or flush, a persistent UI diagnostic
reports that failure and subsequent output uses stderr. This indication is
independent of the queue-loss counter. Correct the disk/permission problem and
relaunch to restore file output; previously lost bytes cannot be recovered.

OpenTelemetry Protocol (OTLP) export of traces, metrics and logs is explicitly
opt-in through `--otlp` or `[telemetry].enabled = true`. Endpoint settings alone
do not enable export. Export uses HTTP/protobuf over Rustls; local logs continue.
No collector is required for offline operation. SDK and HTTP backend debug output
is capped at `warn` because it can expose request/response data; backend events
are excluded from log export to avoid feedback. SDK delivery warnings remain local.
Collector-provided rejection messages are suppressed even at `warn` because
they can echo sensitive values. Safe SDK status/network diagnostics remain available.
Literal file header values are sent unchanged; environment header values are
percent-decoded once before being sent.

Supported environment settings are `RUST_LOG`, `OTEL_SDK_DISABLED` and
`OTEL_EXPORTER_OTLP_{ENDPOINT,HEADERS,TIMEOUT,PROTOCOL}`, including their
`TRACES_`, `METRICS_` and `LOGS_` signal-specific forms. Only `http/protobuf` is
accepted. Headers use standard comma-separated, percent-encoded values. Generic
endpoints append `/v1/<signal>`; signal endpoints are complete. Explicit
`--otlp` or `--otlp=false` overrides `OTEL_SDK_DISABLED`;
`OTEL_SDK_DISABLED=false` alone does not opt in. SDK batch,
metric reader and sampler environment controls retain their standard behavior.
The application resource identifies service/version/process; account and path
values are not metric labels. Safe account IDs can correlate domain events.

## Shutdown

Ordinary close begins shutdown and keeps the window responsive until work drains,
the pool closes, activation stops and telemetry flushes. Repeated close requests
do not restart cleanup. After five seconds the UI offers continued waiting or
explicit Force Quit. Force Quit terminates the process without destructor waits;
unfinished work and diagnostics can be lost. There is no tray process or automatic
forced-exit deadline. Native-window/renderer failure still joins session cleanup.
Beginning shutdown cancels an unapplied Appearance preview and disables reopening
it, leaving delayed-shutdown controls accessible.

## Dependencies and verification

The UI uses eframe/egui with wgpu and platform accessibility/input integration.
The selected OpenTelemetry family uses standard threaded processors and the
blocking Rustls HTTP client, independently of the application Tokio runtime.
Desktop startup explicitly installs AWS-LC as the process-wide Rustls provider
before telemetry or background tasks start, retaining an already installed
provider. This keeps secure synchronization working when workspace builds
enable both the AWS-LC and ring features.
`metrics-opentelemetry` uses the Boost Software License 1.0; the selected egui,
tracing and OpenTelemetry libraries use their upstream permissive licenses.
Transitive platform/graphics and TLS assets retain their upstream notices;
there is no OpenSSL/native-TLS or TQSL execution dependency.
The default embedded fonts retain the upstream `epaint_default_fonts/fonts/`
notices: Ubuntu Font Licence, SIL Open Font License, Hack's combined MIT/Bitstream
Vera terms and icon-font MIT notices. Distribution must include these notices;
the application license does not replace font licenses. Rustls's selected crypto
backends include ring and AWS-LC with their permissive native-code notices.

The resolved Linux windowing/font features use dynamically loaded system
libraries. Headless CI checks do not need a display or additional GUI development
packages; native Linux running still requires the relevant desktop/graphics
libraries and drivers. Windows native builds use the normal Microsoft C/C++ build
tools for bundled SQLite and transitive TLS code. Merely installing the Rust MSVC
target on another OS does not supply those headers/libraries.
wgpu's default Direct3D shader-compiler selection can fall back to the system
FXC compiler; this configuration does not require shipping `dxcompiler.dll`.
Native acceptance must still establish the actual adapter/backend works.

Headless component tests use egui_kittest with native snapshots and typed
request/response boundaries. Store/session tests separately exercise real SQLite
work; process and telemetry tests use actual subprocess ownership and a bounded
Rust HTTP/protobuf receiver. These do not replace native Windows graphics,
input, activation and delayed-close acceptance. Installer qualification belongs
to issue #35.

## Native Windows acceptance

Use a Windows Rust/MSVC development environment and the pinned checkout. These
checks require a visible desktop; headless CI is separate evidence.

1. Run `cargo build --bin clacks-log --locked`. In PowerShell start
   `target\debug\clacks-log.exe --workspace "$env:TEMP\clacks-desktop-ui" --log-filter info`.
   Verify the window renders, tab/keyboard and pointer controls work, and a
   standalone account reaches Ready. Record the account ID and OS/GPU/backend.
2. Minimize it and run the same command again. Verify the second process exits
   and the original window restores. Start another override workspace and verify
   both can run independently. Close both normally, then reopen the first and
   verify the same account ID. Inspect its JSON logs for readiness, shutdown and
   work-drained milestones.

Automated lifecycle and delayed-close tests run noninteractively with
`cargo test -p clacks-desktop --locked`. They do not open native windows or wait
for operator input. Manual native smoke checks use the production executable.

Record observed results with the change's review evidence. Compilation does not establish native rendering or input behavior.

For operating-settings acceptance, use the production executable in a disposable
workspace. Create profiles/configurations, exercise keyboard entry and selection,
manual detachment, separate Save/Apply/Cancel boundaries, nested saves, equipment
deletion review and close/reopen. Automated rendered/session tests are separate
evidence; they do not establish native Windows focus, graphics or input behavior.

For appearance/shared-control acceptance:

- Preview and Apply/Cancel Light/Dark, UI/radio fonts and 100%/125%/150%/200%
  scale. Resize the window; verify field values, errors and actions remain
  reachable by reflow or scrolling. Reopen and check committed preferences.
- With System selected, change OS appearance and verify the application follows
  it. Check a selected installed font in radio fields; when a requested family
  is unavailable, verify the retained choice, fallback notice and retry action.
- Check debounced invalid-field feedback, edited markers, keyboard traversal,
  disabled Save/Apply and corrections. Exercise nested Save followed by outer
  Cancel, Escape, close and ignored backdrop clicks.
- Apply power through Enter, Tab and ordinary focus departure; use Escape to
  restore it. Check `.5`, blank and invalid values. Preview/Apply/Cancel profile,
  equipment and operator choices without changing reusable records.
- Close normally during a request. If cleanup is delayed, verify Continue
  Waiting/Force Quit are accessible, including when Appearance was open.

For logbook-setup acceptance, use the same production executable and a disposable
workspace. Create two Main tags, compare/copy their full IDs, change independent
upload policies, select one and verify its ID after restart and rename. Switch
both built-in views, cancel edits through Cancel/Escape/close, and delete selected
and unselected empty tags after explicit review. Check All contacts fallback and
its dismissible explanation in both themes/scales. Populated and incoming deletion
use real automated store/session tests until logging/network consumers arrive.

## Manual contact logging

The workbench keeps quick QSO entry beside Contact details and Our settings,
with saved-contact history below. Both upper panes edit one unfinished contact.
Start and End contain complete UTC dates and times, independently live until
locked. A locked End can be blank. Enter in a single-line contact field saves;
multiline Comment and selection controls retain their normal keyboard behavior.

Save QSO writes locally without a server/provider connection. Successful saving
clears contacted-station facts, reports and comment, retains radio information,
and starts the next contact from current operating settings and selected view.
Failure preserves input. Named logbooks supply their tag; All contacts and
Untagged supply none. A committed save followed by a failed refresh remains saved.

Double-click a history row or press Enter on its focused row to edit the saved
contact. Save changes affects that record; Cancel/close/Escape discards form edits
without changing new entry or operating defaults. Stale records require explicit
reload before overwriting their current stored facts. Delete QSO confirms the
contact's deletion across every logbook, with Cancel initially focused.

History reads bounded pages with combined UTC timestamp sorting and configurable
columns. Its always-visible expression bar applies valid input after 250 ms;
invalid or unfinished input retains the last complete results and identifies
their applied expression. Suggestions use the shared field catalog and current
mode/band, tag and group data. Enter/Tab accepts a suggestion; Escape dismisses
it. Literal query text, including unfinished edits, survives restart independently
of the last applied expression. Normal shutdown awaits the final local write.

Worked Before occupies the adjacent tab or an optional dedicated window. Both
use the same unfinished entry callsign, view, rows and column choices, with
window-local scrolling/focus. Clearing the call clears the list. Row activation
uses the existing saved-contact editor; opening or closing the extra window
does not create another draft or change the workbench's selected tab.
Pane sizes and table presentation are local preferences with reset controls.
History and Worked Before column choices are independent. Directory lookup and
geographic autofill retain their separate scopes. The
[manual-logging contract](../../docs/manual-logging.md) defines these boundaries.

The contact editor's **Assigned logbooks** summary opens a bounded searchable
picker through **Edit…**. Checkbox changes remain pending in the form until
**Save changes**; dismissing the picker preserves them, while cancelling the
editor discards them. No separate membership Apply step is required.

History shows **Edit assigned logbooks…** only when contacts are selected. Its
picker includes off-page/outside-filter selected IDs, preserving untouched mixed
memberships. **Apply** submits guarded membership changes. **Cancel processing**
stops between contact saves; retained results distinguish saved, unchanged, stale
and unprocessed contacts from a failed acknowledgment or subsequent refresh.
Removing the last membership leaves the contact active and untagged.

Native Windows acceptance uses the production executable with a disposable
workspace: enter an overnight contact and one with unknown End, change active
power/view during unfinished entry, switch detail tabs, save, restart, edit/cancel
and individually delete a multi-tag contact. Verify layout, keyboard operation,
combined timestamp ordering and durable local outcomes. Automated tests do not
wait for human interaction or expose delay flags.

## WS Suite capture

Open **Settings → WS Suite…**, enable **Receive WS Suite updates**, choose
Unicast or Multicast, and Apply. Unicast receives on the selected Local address
and Port. Multicast joins the supplied Group address and Port on System default
or an explicitly selected Network interface. Refresh interfaces updates the
available names/addresses. IPv6 local-scope multicast requires an explicit
interface. No file editing or application restart is needed.

The checkbox and normal unapplied-field treatment convey configuration. Only
failures have a separate reception message. Apply retries failed reception even
without changes; successful retry clears the message and closes the dialog.
Failed activation retains saved values and keeps the dialog open. Applying Off
preserves the last committed connection and works despite unavailable interfaces
or invalid disabled inputs. Cancel/Escape/close discard unsubmitted edits.

Preferences belong to the current local workspace. Reception starts when that
workspace opens if enabled. Closing the application stops capture. Unicast
requires an available port; multicast supports other local receiver applications
sharing the group/port. WS must use a matching destination/interface and enable
same-computer multicast delivery. Enable capture in one Clacks client per source.

Type 5/type 12 completions share a 100 ms pairing window; both usable forms save
early as one ADIF contact, otherwise the usable form saves at expiry. Recent
repeat suppression is in memory per source until its next successful completion;
receiver-only changes retain it. Saved contacts follow ordinary history and
synchronization. Active defaults, the selected tag and current definitions are
captured once; the unfinished manual contact remains independent. The
[integration contract](../../docs/integrations.md#ws-suite-the-wsjt-x-family-of-weak-signal-radio-applications-capture)
defines field handling and correlation.

## Server connection

**Connect…** opens the server and client-label form. New connections default to
`https://clacks.w7hex.com` and “Desktop”, so hosted users can choose Connect without
entering anything. Saved and installed connections retain their own server. The
client label identifies the installation in the account's client list.

Connect owns authorization and completion recovery. It uses saved credentials when
possible and opens browser sign-in/approval when required; users never manage saved
attempts or choose credential-repair operations. Huskarl owns the temporary loopback
callback and proof-bound exchange. Protected native storage holds the signing key
and refresh credential; SQLite retains only nonsecret references and completion
context. Linux native credential storage is not implemented; no plaintext fallback
is used.

First enrollment connects the current application instance and existing empty
workspace. Unchanged bundled definitions are replaced by the account's synchronized
definitions; local operating facts and preferences remain. Populated standalone logs
are rejected before authorization and checked again during installation under D56:
export contacts, connect an empty log, then import through the accepted import
workflow. Connect does not create another workspace or launch a process.

Before enrollment, the server can be corrected and Connect safely replaces only
uninstalled connection material. **Connection…** offers Reauthorize for the existing
account/server. Previously unsent work retains its recovery restrictions if the
writer was replaced. Native access failures and transient server failures preserve
credentials instead of reopening browser approval. A verified rejected credential
or wrong-account saved credential uses the existing browser flow internally.

While connecting, duplicate submission is disabled. Open browser is available during
approval. Cancel, Escape and close stop local work without promising to undo remote
approval. Failures retain inputs and permit retry through the same primary action,
with a concise explanation and sanitized expandable details. Successful completion
closes the dialog; the existing status display reports synchronization separately.
Committed installation remains installed if cleanup fails, and retry finishes the
remaining local work without another authorization. Account-qualified services reload
when the durable binding changes, including when cleanup remains pending.

### Native credential integration

The shared `credentials::entry` adapter constructs entries through the selected
native backend without changing the global keyring store. Enrollment retains its
`com.w7hex.clacks-log.authorization` namespace, opaque references and serialized
bundle format. The adapter selects macOS Data Protection Keychain or Windows Local persistence;
unsupported Linux storage returns an error and never falls back to plaintext.

On macOS, the selected library's `protected` backend uses the app's default
access group, local storage and When Unlocked access policy. It does not request
user presence or enable iCloud synchronization. Both enrollment and directory
material use this backend; there is no migration or fallback to file-based
Keychain storage. Ordinary access to the correctly provisioned app's own items is
silent while the device is unlocked.

Data Protection Keychain access requires a signed application bundle with an
embedded provisioning profile authorizing its application identifier/access
group. A bare `cargo run` executable can run local logging but is not a
credential-enabled macOS distribution. A signing certificate alone, an ad hoc
signature or an entitlement file without a matching profile is insufficient.
See [Apple's implementation guidance](https://developer.apple.com/documentation/technotes/tn3137-on-mac-keychains)
and [provisioning profiles](https://developer.apple.com/documentation/technotes/tn3125-inside-code-signing-provisioning-profiles).

The native enrollment and directory integration scenarios require their test
executable in an equivalently signed/profile-authorized application bundle. Run
that executable directly with the normal test filters and disposable database
variables; do not rebuild or replace it until the test process exits. Keep
`LEPTOS_OUTPUT_NAME=clacks-web` consistent across desktop builds. Missing profile
authorization is a native integration prerequisite failure, not a reason to use
another backend. The backend-selection unit test needs no stored secret or profile.

#### macOS development/test host

The repository supplies the ordinary [app metadata](../../bin/clacks-log/macos/Info.plist).
Use Apple's native bundle/signing tools with a certificate and Mac provisioning
profile authorizing `com.w7hex.clacks-log` and its default Keychain access
group. Keep the private signing key and profile outside the repository. This is
a local development/test host, not a distribution or notarization workflow.

Build with `LEPTOS_OUTPUT_NAME=clacks-web cargo +1.99.1 build --bin clacks-log
--locked --offline`. For native tests, instead use `cargo +1.99.1 test -p
clacks-desktop --lib --no-run --locked --offline` with that same environment,
then set `EXECUTABLE` below to the actual executable path printed by Cargo.

Keep the application identifier, display name and bundle path stable. Stop the
development/test host before updating its executable; never replace a running
executable. From the workspace root, set the existing signing inputs and use
this same bundle path on subsequent builds:

```sh
SIGNING_IDENTITY='your existing signing identity'
PROFILE='/absolute/path/to/your.provisionprofile'
EXECUTABLE="$PWD/target/debug/clacks-log"
mkdir -p target/macos
BUNDLE_ROOT="$PWD/target/macos"
APP_BUNDLE="$BUNDLE_ROOT/Clacks Log.app"
mkdir -p "$APP_BUNDLE/Contents/MacOS"
cp bin/clacks-log/macos/Info.plist "$APP_BUNDLE/Contents/Info.plist"
cp "$EXECUTABLE" "$APP_BUNDLE/Contents/MacOS/clacks-log"
PACKAGE_ID="$(cargo +1.99.1 pkgid -p clacks-log --locked --offline)"
VERSION="${PACKAGE_ID##*#}"
VERSION="${VERSION##*@}"
/usr/libexec/PlistBuddy -c "Add :CFBundleVersion string $VERSION" "$APP_BUNDLE/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleShortVersionString string $VERSION" "$APP_BUNDLE/Contents/Info.plist"
cp "$PROFILE" "$APP_BUNDLE/Contents/embedded.provisionprofile"
security cms -D -i "$PROFILE" > "$BUNDLE_ROOT/profile.plist"
PREFIX="$(plutil -extract ApplicationIdentifierPrefix.0 raw -o - "$BUNDLE_ROOT/profile.plist")"
TEAM="$(plutil -extract TeamIdentifier.0 raw -o - "$BUNDLE_ROOT/profile.plist")"
APP_ID="${PREFIX}.com.w7hex.clacks-log"
ENTITLEMENTS="$BUNDLE_ROOT/entitlements.plist"
/usr/libexec/PlistBuddy -c 'Clear dict' "$ENTITLEMENTS"
/usr/libexec/PlistBuddy -c "Add :com.apple.application-identifier string $APP_ID" "$ENTITLEMENTS"
/usr/libexec/PlistBuddy -c "Add :com.apple.developer.team-identifier string $TEAM" "$ENTITLEMENTS"
/usr/libexec/PlistBuddy -c 'Add :keychain-access-groups array' "$ENTITLEMENTS"
/usr/libexec/PlistBuddy -c "Add :keychain-access-groups:0 string $APP_ID" "$ENTITLEMENTS"
codesign --force --sign "$SIGNING_IDENTITY" --entitlements "$ENTITLEMENTS" --timestamp=none "$APP_BUNDLE"
codesign --verify --strict "$APP_BUNDLE"
"$APP_BUNDLE/Contents/MacOS/clacks-log" --help
```

The identifier prefix comes from the actual profile, rather than assuming it is
the team ID. The profile must authorize these requested entitlements; adding an
entitlement file alone does not grant access. For native acceptance, run the
copied test executable directly with the usual test filter and disposable
database variables. Keep its bundle intact until the process exits. Profile
availability and successful native secret operations remain separate from a
successful unsigned build or metadata validation.

The fixed executable name also avoids renaming the app when Cargo's test hash
changes. Native UI automation must attach to the already running host with its
explicit temporary workspace arguments. Do not read UI state after quitting if
the automation API can relaunch the app without those arguments; verify process
exit through its owning runner instead.

Directory provisioning uses this same adapter with its own namespace and one
complete configuration/login bundle checked against deployment and account.
The workspace allocates a fixed native entry name once in `local_state`;
replacement and removal update that protected value directly, without a SQLite
installation commit or obsolete entries. **Retry directory storage** retries
local reads and the retained current delivery without reconnecting or changing
enrollment. Native access failure retains the last usable runtime configuration.
Directory replacement/removal preserves enrollment entries. Configuration is
received on the authenticated connection independently from record checkpoints;
the last installed material remains usable during sync outages. The lookup actor
watches actual enrollment/configuration changes without restarting unfinished
entry ownership when synchronization reconnects.

### Direct directory lookup

Configure the account's selected provider and credentials at **Account →
Directory** in the web interface. A manual callsign edit admits lookup after
500 ms of unchanged normalized input; empty input makes no request. Opening an
unchanged saved contact shows retained facts without refreshing it. QRZ and
HamQTH use separate concrete client-direct sessions with shared transient
authentication and independent station requests. Session expiry renews once;
credential rejection pauses that material until replacement or explicit retry.
Provider failure/not-found preserves successful facts, with no bulk catch-up.

Directory name, location, references and locality are attributed separately from
recorded controls, including HamQTH attribution. Tabbing or saving never copies
them into recorded facts. Save is immediate; an available success commits
atomically with the QSO. A late response retains its original QSO/key and checks
the actual live parent inside the snapshot transaction. Saved original contacts
can receive late results after the form moves on; changed-key/deleted contacts
cannot. Committed snapshots invalidate local history even while sync is offline.
One whole winner per contact/key synchronizes separately from recorded edits.
Bulk lookup, WS Suite capture and external-service publication remain separate
capabilities.

## Account synchronization

An enrolled session owns its authenticated WebSocket task and reconnects using
the existing protected Huskarl authorization. Every cycle completes incoming
account discovery before publishing any pending proposal or frozen retry. Local
saves remain independently durable offline. Accepted recovery facts travel
separately from new pending edits, preserving original author/context.

After committed receipt, normal settings/history/Worked Before views reload.
Copied active operating values retain their existing selection and adjustment
rules. The status bar shows connection state and the current error; there is no
aggregate successful-sync timestamp. Ordinary unresolved records remain local
without blocking unrelated publication. The Unsynchronized changes window owns\nexplicit local conflict resolution.

Server-reported failures retain their operational category for structured logs
and show the safe server explanation as the current connection error. Every
acknowledgment wait, including accepted-facts completion, uses the same response
handling for failures, account change notices and returned review inputs.
Reconnect uses the existing protected authorization and durable checkpoints.
Dependency support is included with ordinary pages rather than requested by a
separate exact-version exchange.

## Unsynchronized changes

The application-bar action opens a separate non-modal, account-wide window.
Conflicts and Pending sync are independently collapsible, with type filtering
and bounded continuation. Ordinary pending deletions remain visible here while
hidden from contact history. Logbook membership does not scope synchronization.
The window reads committed workspace snapshots through the normal session; it
leaves the selected history view and unfinished logging entry in place.

Conflicting contacts, saved station profiles, equipment configurations and
logbook tags use explicit per-field choices and direct validated edits. Source
choices copy native values, including unfamiliar labels and imported location
precision. Membership starts with the local set and is edited directly.
Resolving a saved source does not load it into active operating settings.

Drafts remain in the review window while navigating. New comparison evidence
marks a draft outdated; explicit refresh retains its inputs and requires
reconsidering fields whose compared values changed. Saving checks the exact
record, review and live references in one transaction, and creates pending local
work for the normal synchronization path.

Blocked deletions show accepted referring records, including locally repaired
records awaiting acceptance. Open their ordinary editor or their own conflict
review, then explicitly refresh references and retry the deletion. Missing tag
or equipment references require explicit removal/replacement; review does not
restore them. Previously authorized work stays held until usable shared
comparison evidence exists and its matching review is explicitly resolved.

## Application menus

Windows and macOS use native **Settings**, **View** and **Help** menus. Settings
contains logbook management, saved station profiles/equipment, settings refresh,
connection management and appearance. View resets the workbench layout. Help
opens workspace/account/log-location details. On macOS, the conventional
application menu remains available and Command-Q uses the same orderly shutdown
as closing the window.

The in-window toolbar retains immediate logbook selection and Unsynchronized
changes. Synchronization status remains in the existing footer. The Linux build
uses an egui menu bar with the same commands; it does not require GTK menu
integration. Native-menu initialization failure on Windows/macOS is reported as
a startup error rather than silently substituting an in-window toolbar.

For native acceptance, run a separate disposable local workspace with
`cargo run --bin clacks-log -- --workspace /absolute/path/to/disposable-workspace`.
Check menu placement, idle activation, disabled items during modal editing,
focus restoration, unchanged contact drafts and orderly Quit. On Windows also
check Alt menu access and different display scales; on macOS check the
application menu, Help search and Command-Q. Open Unsynchronized changes and
check that macOS application-menu commands target the main workbench when that
auxiliary window has focus. Component tests and cross-compilation do not replace
these platform checks.

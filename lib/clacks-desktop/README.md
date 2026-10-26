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
logbooks** to retry the read. QSO entry/basic contact presentation, richer history,
and ordinary individual/group membership editing belong to #9, #10 and #51.

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

Headless private tests exercise real SQLite work, subprocess ownership and a
bounded Rust HTTP/protobuf receiver. They do not replace native Windows graphics,
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

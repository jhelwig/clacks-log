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

## Configuration

Command-line options override supported environment values, which override file
values, then defaults. Signal-specific options win within their own precedence
tier; a generic CLI endpoint still overrides an environment-specific endpoint.
Unknown file keys, invalid supplied values and missing explicitly selected files
are errors. A missing default file is normal.

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

See `--help` for explicit Boolean options, log controls and generic/per-signal
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
opt-in through `--otlp true` or `[telemetry].enabled = true`. Endpoint settings alone
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
`--otlp` overrides `OTEL_SDK_DISABLED`; false does not itself opt in. SDK batch,
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
   `target\debug\clacks-log.exe --workspace "$env:TEMP\clacks-native-46" --log-filter info`.
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

Record observed results with the change's review evidence. These steps remain
required before declaring issue #46 delivered; compilation does not establish
native rendering or input behavior.

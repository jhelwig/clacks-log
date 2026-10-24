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

Headless private tests exercise real SQLite work, subprocess ownership and a
bounded Rust HTTP/protobuf receiver. They do not replace native Windows graphics,
input, activation and delayed-close acceptance. Installer qualification belongs
to issue #35.

# Desktop application foundation

This specification defines the production desktop foundation for
[issue #39](https://github.com/jhelwig/clacks-log/issues/39), carrying forward
D270–D279 and the selected egui architecture. The [desktop guide](../lib/clacks-desktop/README.md)
documents runtime usage and validation. The [product contract](product.md),
[client store](client-store.md), [native model](native-model.md) and
[observability contract](observability.md) remain authoritative for their domains.

## 1. Outcome and boundaries

Deliver the Windows desktop application that subsequent logging workflows extend.
It starts offline, creates or reopens a real account workspace, presents useful
startup/readiness/error information, activates an existing instance and closes
cleanly. It uses production libraries and durable storage throughout.

Windows is this foundation's delivery target. D392 advances early Windows
installation artifacts ahead of complete MVP workflow readiness, followed by
macOS packaging in a separate milestone. Final release-readiness validation
covers both platforms; Linux desktop delivery remains deferred. Keep Rust 1.99.1 from
`rust-toolchain.toml`, workspace-inherited dependencies and Elastic License 2.0.
Project-owned tooling and non-trivial fixtures are Rust-only.

The foundation does not deliver profile/equipment workflows (#7), tag setup
(#8), contact membership editing (#40), contact entry/editing/deletion (#9),
history/Worked Before (#10), enrollment (#12), live service adapters or installed
release packaging (#35). Those capabilities integrate into this application.
The [logbook setup specification](logbooks.md) owns #8's selector and management
workflow. Do not create demonstration stores,
a synthetic logging UI, a second synchronization algorithm or temporary
application components. No server connection, provider connection or enrollment
is needed for local startup. Closing stops the client; there is no tray mode.
The [desktop UI contract](desktop-ui.md) owns the workbench arrangement and
interaction requirements beyond this foundation; it does not require docking.

## 2. Selected approach and alternatives

| Area | Selected approach and reason | Alternative and tradeoff |
| --- | --- | --- |
| Presentation | egui/eframe with wgpu; Rust UI with cross-platform renderer foundations (D215/D273). Use egui_extras when the delivered workflow needs it. | Glow was considered for a smaller binary; it is not the initial renderer. Toolkit alternatives remain in the [comparison](../references/desktop-toolkit.md). |
| Async work | One application-owned multi-thread Tokio runtime alongside the main-thread native event loop (D271). | A current-thread runtime pumped by drawing would tie progress to UI activity; a custom executor/DB queue is unnecessary. |
| Workspace | Automatically managed default, with a path override (D270). | Mandatory file selection adds first-launch work; server enrollment first would break standalone use. |
| Process ownership | Standard-library lifetime file lock plus `interprocess` activation (D276/D279). | File existence/process-ID checks are not ownership; database locks alone do not implement window activation. |
| Diagnostics | Local rotating JSON logs; opt-in OpenTelemetry Protocol (OTLP) export of all three signals over HTTP/protobuf and Rustls (D274/D275/D278). | Collector-required operation conflicts with offline use. Export replaces neither local logs nor authoritative application evidence. |

One desktop library is sufficient for this capability. Separate UI/runtime
libraries with no independent consumers would add public boundaries without a
current integration need. This is an engineering choice, not a new product rule.

## 3. Production components and ownership

Use `lib/clacks-desktop` for application behavior and `bin/clacks-log` for a thin
entry point. The library owns argument/configuration interpretation, startup,
workspace and instance coordination, runtime lifetime, UI integration and
telemetry composition. Organize these as focused modules, not a generic command,
event, dependency-injection or telemetry framework. Public APIs have application
callers; test support stays private or in tests. Do not introduce `mod.rs` outside
tests or facade/glob re-exports.

Existing libraries retain their responsibilities:

- `clacks-model`: native values, validation and domain operations.
- `clacks-client-store`: relational persistence, migrations, account identity,
  snapshot reads, guarded saves and durable synchronization/evidence transitions.
- `clacks-sync`: synchronization metadata and pure transitions. The desktop
  foundation does not start a network synchronization service.

Application composition owns one runtime, one workspace ownership handle, one
store pool, background task lifetimes, the native window and process-wide
telemetry guards/providers. Shutdown releases them deliberately. A second
workspace is a separate process, not another account in the same pool.

Later workflow modules use the same store and runtime. Keep domain values and
SQL in their existing owning libraries; do not move persistence into UI callbacks
or use eframe UI-state persistence for account records or operating defaults.

## 4. Workspace and configuration

### 4.1. Files and discovery

Use `directories::ProjectDirs` for OS-conventional paths. The engineering default
is an empty qualifier/organization and application name `Clacks Log`; use
`data_local_dir()/workspace` for the default workspace and
`config_dir()/config.toml` for configuration. This respects Windows local data
versus roaming configuration and Linux configuration/data conventions. Do not
substitute the current directory when OS directory discovery fails.

A workspace path denotes a directory containing `workspace.sqlite`, the separate
`instance.lock` coordination file and its `logs` directory. SQLite owns its normal
sidecar files. `--workspace <directory>` changes the selected workspace;
`--config <file>` independently selects configuration. Resolve relative command-line
paths against the launch directory; resolve paths in configuration relative to
that file's directory. Log-location overrides follow the same origin rule.
These spellings/layout are engineering defaults for review, not extra account
metadata or database markers.

Create missing selected workspace directories and exclusively create a missing
database through the existing store API. Allocate a native local account ID only
for new-store creation. Existing files are opened, never treated as new after an
error. Failed initialization can leave an incomplete file; report it and preserve
it rather than silently resetting it. A workspace override has the same opening
and preservation rules as the default.

The store can discover the stored account within its read-only compatibility
preflight, returning an account-qualified store and its non-secret identity.
The expected-account opening path remains available for callers that already
know the account. Both paths share preflight/migration logic; do not add an
extra discovery connection, duplicate validation, account registry, application
marker, schema-version copy or desktop SQL. Fixed queries remain checked SQLx
query files with committed offline metadata.

Standalone account initialization also owns provisioning the default reference-group
seed dataset required by the native model. Supply reviewed production data through
the ordinary definition engine/store, never model-test fixtures or program-name
switches. Validate and install the seed set transactionally before reporting a new
workspace ready. Give it stable definition identities; reopening never overwrites
or reinstalls definitions merely because the application was upgraded. The plan
must identify the seed manifest/content and source basis, rather than invent a
new catalog downloader or group-management UI. Include account-row creation and
the initial seed installation in one transaction after schema initialization,
extending the production store creation path as needed. A failed transaction leaves no initialized account; subsequent
opening fails visibly under the incomplete-file policy above. Do not add a
desktop initialization marker or automatic destructive retry.

Connect enrolls the current application instance in its existing workspace when
the local log is empty. Unchanged unpublished bundled definitions do not make
the log independently populated: replace that initialization with the enrolled
account's definitions through synchronization. Preserve local operating facts
and preferences. Reject automatic joining of independently populated local data
under D56, preserving that data for the export/import workflow. Enrollment does
not require another workspace or process. Production seed provisioning is an
application/account-initialization responsibility that later server provisioning
can reuse, not behavior of the pure native-value library.

One database remains one local account workspace. Discovery grants no server
authorization, creates no client actor/timeline/checkpoint, and does not remap
an enrolled/copied database. Enrollment and backup/restore retain their existing
contracts. The desktop lifetime lock is application coordination, not a new
store migration lock or database safety claim.

### 4.2. Configuration resolution

Configuration is optional and typed TOML. Resolve explicit command-line interface
(CLI) settings first, then applicable standard environment settings (`RUST_LOG`, OpenTelemetry settings),
then file values, then built-in defaults. Distinguish an omitted option from an
explicit false/zero value when resolving precedence. Unknown or invalid supplied
settings produce actionable diagnostics rather than silently changing meaning.
A missing default file uses defaults; an explicitly selected missing/unreadable
file is an error. Do not create a configuration file merely by launching.

The desktop guide documents the supported configuration keys; the executable’s
`--help` lists command-line options. This specification fixes their meanings:

| Setting | Contract/default |
| --- | --- |
| Workspace | Managed default or explicit directory override. |
| Log output | Workspace-local files, `info` filter, optional console, optional directory override. |
| Rotation/retention | Daily rotation; at most seven matching log files by default. Configurable positive retained-file count. This is not a hard byte quota. |
| Writer queue | Standard bounded nonblocking appender, 8,192 buffered lines by default, lossy on saturation to keep work responsive. |
| OTLP export | Disabled by default; explicit opt-in, independently configured signal destinations and standard exporter controls. |

The numeric defaults above are engineering choices, not previously approved
product decisions. Startup reads configuration once; live reload and a settings
editor are not required. Saved profiles, operating defaults and account data
remain SQLite data, not duplicated TOML configuration. There is no server login,
LoTW private key or enrollment secret in this file.

An OTLP endpoint alone does not implicitly opt in. Explicit application opt-in
is required; standard `OTEL_SDK_DISABLED` can disable export unless a higher-priority
explicit CLI setting overrides it. Resolve this before constructing exporters.
Honor supported standard per-signal settings ahead of generic settings within
the same precedence tier. Programmatic builder values must reflect the resolved
result, since exporter builders can override environment variables. Unsupported
transport selections must fail clearly rather than silently enable another
protocol. Never print configuration wholesale or echo headers/authentication
values in configuration errors.

## 5. One process per workspace and activation

Resolve workspace identity before opening its database. Canonicalize the selected
directory, including symlink aliases; same-directory aliases must address the same
owner and activation endpoint. Test Windows case/path aliases as well as ordinary
canonical paths. Coordination is per workspace, not per account ID: copying a
workspace is not client reauthorization or a supported backup/restore flow.

Open `instance.lock` without truncating it and attempt an exclusive
`std::fs::File::try_lock()`. Keep the owning handle alive through cleanup.
A leftover file is normal; lock ownership comes from the OS. Never unlink/recreate
the lock file to defeat contention. Permission/filesystem errors are failures,
not evidence of another owner and not permission to continue unlocked.

If ownership is unavailable, send a bounded activation request through
`interprocess` and exit on acknowledgment. This process must not open/migrate the
database or start a second workspace runtime. The message requests activation
only; it is not arbitrary command execution or an enrollment callback channel.
Use a small versioned message, bounded framing/read time and a bounded retry
period for the interval between owner election and listener readiness. The
engineering default is a three-second activation deadline. If delivery fails,
report that activation could not be completed; do not start a second owner.
A retry may acquire ownership if the previous owner actually exited.

Use Windows named pipes and Unix-domain sockets through `interprocess`.
Choose endpoint names from stable workspace identity, with bounded length and
current-user access. Restrict Unix directory/socket permissions and Windows
pipe access to the intended user; an opaque endpoint name alone is not access
control. Endpoint names, platform security settings and framing are implementation
plan obligations, not reasons to introduce a network listener or credential
exchange. No secrets/account payloads are sent through activation.

Only an owner may recover a stale endpoint, and only its own endpoint. Do not
use socket-overwrite behavior as a substitute for the lifetime lock. Acquire
ownership before cleanup/listen; close and clean up the listener before releasing
ownership. A crash may leave Unix socket paths, but releases the OS lock.

Start listening before lengthy workspace initialization. Remember activation
requests arriving before window readiness; request repaint and then restore/show
and focus the window as permitted by the platform. Acknowledge receipt without
claiming the OS granted foreground focus. During closing, acknowledge and expose
the existing closing window; do not reopen admission or cancel shutdown implicitly.

## 6. Startup, UI and background operations

Start the native event loop on the main thread with eframe/wgpu. The window must
remain responsive while workspace preflight, migration and initial reads run on
the application-owned multi-thread Tokio runtime. Background work progresses
when drawing is idle or the window is minimized. Never run synchronous waits or
`block_on` for store/network work in rendering/input callbacks.

Use standard channels and egui repaint requests for task results. UI state stays
on the UI thread. Bound queued work; coalesce repaint/activation notices where
appropriate. Track admitted tasks with existing Tokio facilities, propagate
tracing context, observe task errors/panics, and retain handles needed to join
work. Do not build a custom executor, generic message bus or database write queue.

Present startup/opening, ready, opening-failed and closing states. Ready means
the actual store is open and initial required reads succeeded, not merely that
a window was created. Show the selected workspace, standalone/enrolled status
without secrets, and how to find diagnostic logs. No fabricated online/sync
status appears before a transport exists. The initial ready view need not
contain unfinished future workflows or controls that do nothing.

On opening failure, retain an actionable error, retry using fresh startup state,
and allow ordinary close. Explain path overrides for selecting another workspace;
an in-app workspace picker/account switcher is not required. Never automatically
repair, replace or clear data. Do not retain a failed pool/task across retries or
repeat initialization on every UI frame. Close requests during initialization
follow the same shutdown policy as later operation.

Results carry enough request/context identity to avoid applying stale replies to
changed UI state. Account saves still use the store's transaction-level stale-save
guards. These are ordinary application integration obligations, not a new record
generation scheme or conflict workflow. Later issues implement their own real
operations within this ownership/admission model.

## 7. Graceful shutdown and failure

Intercept the native close request while graceful shutdown is in progress:

1. Stop admission of new application work. Keep the window responsive and keep
   ownership of the workspace.
2. Request cancellation at supported safe boundaries. Let already-started save
   transactions complete; cancellation is not permission to drop their futures.
   Join admitted work and account for failures, without waiting for network sync
   or a provider to become available.
3. Close the store after its users finish. Then stop activation listening and
   clean up the owned endpoint.
4. Flush/shut down telemetry using framework facilities, release its guards,
   stop the runtime after its tasks complete and release the lifetime lock.
   Permit the native event loop to exit.

Operations that block, including OpenTelemetry software development kit (SDK)
shutdown or writer flushing, run outside UI callbacks. Do not rely on runtime
destruction or `on_exit` to perform async cleanup after the UI is gone. Every startup failure path releases what it owns
in the same dependency order, including when window/renderer creation fails.

After five seconds of delayed shutdown, offer Continue Waiting and explicit
Force Quit. This threshold is an engineering default, not an automatic timeout.
Force Quit may interrupt unfinished work and lose buffered telemetry; saved
transactions retain SQLite's recovery guarantees. It must actually terminate the
process rather than wait on a stuck runtime/thread destructor. Do not equate
Tokio shutdown timeouts with terminating running blocking work. Repeated close
requests do not start parallel shutdowns or silently force exit.

Unfinished QSO drafts stay nonpersistent (D113). Saved pending edits/evidence
stay durable; normal exit never requires publishing them. No application-owned
work continues deliberately in a tray/background process after normal close.

## 8. Observability composition

Use ordinary `tracing-subscriber` formatting/filtering and `tracing-appender`
rotation/nonblocking output directly. Retain JSON event fields and span context;
keep guards alive through application shutdown. Queue saturation drops telemetry
rather than blocking store/UI operations. Use the appender's dropped-line count
for degradation diagnostics; do not recursively log each dropped event. Logging
initialization/write failures must be visible through startup diagnostics or the
running UI and available fallback output, without refusing ordinary local work
solely because telemetry is unavailable. Do not promise recovery of lost logs.

Opt-in OTLP export uses existing adapters: `tracing-opentelemetry` for spans,
`opentelemetry-appender-tracing` for events, and a compatible `metrics` 0.24
recorder adapter such as `metrics-opentelemetry`. Keep the producer APIs intact.
Use a mutually compatible OpenTelemetry SDK/exporter family, HTTP with binary
Protocol Buffers and Rustls. A tracing-event metrics layer is not a replacement
for the existing metrics recorder.

Use bounded standard SDK batching/queues, export timeouts and retry behavior;
export must not run in a UI callback or a store transaction. Match HTTP client
and processor execution models: the ordinary SDK thread-based processors/readers
need a client that can operate there, rather than assuming an async client can
borrow the application runtime. Collector failure degrades export only; local
logs and storage continue.
Avoid exporter-diagnostic feedback loops into the same log exporter. Shutdown
flush attempts are bounded by exporter configuration and report loss/failure;
they do not justify an automatic forced application exit.

Use standard resource metadata for application/version/process identity. Keep
account/record correlation out of metric labels. Normal operation has no hosted
destination and opens no metrics scrape server. With export disabled, metrics
producers may use the facade's ordinary no-recorder behavior; do not add an
unbounded local metrics archive or a custom in-memory recorder.

### 8.1. Diagnostic questions and measurements

| Question | Clacks evidence |
| --- | --- |
| Did this launch open the workspace or hand off to its owner? | Significant `info` startup/ready/activation events; attempted startup span and classified failure at its handling boundary. |
| Why is opening failing or taking time? | Opening phase and safe error class/cause; existing store/migration spans; runtime startup-duration histogram measured from owner startup to ready/failure. |
| Why is close delayed? | Closing milestone events with outstanding work category/count and elapsed wait; `warn` when delayed choice first becomes necessary, explicit force-quit event when selected. No per-frame logs. |
| Is telemetry being lost? | Appender dropped-line count/degraded state and standard exporter diagnostics, reported without recursion or secret values. |

Startup duration is seconds with a bounded `outcome` dimension (ready/failed/
cancelled), excludes activation-only launches and is recorded once per owner
startup attempt. Delayed-shutdown reporting is diagnostic state, not a generic
operation-labelled metric family. Existing store metrics keep their committed
measurement boundaries. Do not count startup/readiness as a saved QSO.

Use `#[tracing::instrument(skip_all, ...)]` on meaningful runtime operations,
default module targets, native Debug/Display fields, and framework lifecycle
support. No custom completion structures, timing wrappers or telemetry-only
formatting helpers. Do not trace frames, getters or channel polls individually.
At `info`, significant milestones and handled failures remain intelligible when
diagnostic spans are filtered out. Log an error once at the handling boundary.
Never log configuration dumps, arbitrary payloads, headers, protected-store
references or authentication values. File paths displayed to the local user
are not automatically exported as telemetry fields; error chains need the same
selection/redaction discipline as event fields.

## 9. Verification and acceptance

All delivered code and test tooling use the locked Rust toolchain. Verify:

1. First launch creates a real local-only workspace, reports ready offline and
   closes normally. Reopening discovers the same account and values without a
   second account-ID source. Existing store-backed fixtures exercise persisted
   state; no temporary production-facing save API or artificial QSO UI is needed.
   Check production default definitions, all-or-nothing account/seed installation,
   and preservation of established definitions on reopen.
2. Default/config/CLI paths and precedence, absent default versus explicit missing
   config, invalid values and secret-safe diagnostics. Overrides remain independent.
3. Existing incompatible/corrupt/unreadable databases fail visibly without
   replacement. Read-only preflight preserves the store's rejection guarantees.
   Creation/migration failures leave preserved, diagnosable state.
4. Real process contention: one owner, acknowledged activation, different
   workspaces allowed, startup/closing activation races, same-path aliases,
   owner termination and stale endpoint recovery. Verify Windows pipe permissions
   and Unix endpoint permissions where those platforms are exercised.
5. Store operations/results do not block input/rendering. Background work
   progresses with the UI idle/minimized. Real store integration exercises the
   production task/result boundary without implementing later workflows.
6. Closing during initialization and admitted operations, delayed shutdown,
   repeated close requests, explicit force quit and reopening after interruption.
   In-flight saves use real store transactions in integration tests; they do not
   require shipping contact-entry UI in #39.
7. Through real startup/activation/error/close operations, assert truthful domain
   events, correlation at ordinary filtering, measurement boundaries and deliberate
   sensitive-data exclusion. Exercise the runtime's actual export path against a
   Rust local receiver, including unavailable/slow collectors, all three enabled
   signals, continued local output and graceful termination. Do not retest generic
   formatter/SDK functionality or mirror the entire suite with telemetry toggled.
8. Run formatting, Clippy, documentation, existing regression suites and required
   Linux/Windows CI using `rust-toolchain.toml`. Trigger workflows under the
   repository's PR/main policy, never on every feature-branch push.
9. Run the actual native Windows application with wgpu: render/input, first launch,
   reopen, activation, ordinary close and delayed-close/force-quit behavior.
   Record environment, renderer/backend and observations. Cross-compilation,
   headless logic tests or a software-rendered CI test alone do not establish
   this native acceptance. Installer/distribution qualification remains #35.

Use subprocess and headless tests for deterministic ownership/lifecycle behavior,
and actual native interaction for window/renderer claims. Exercise delayed work
through ordinary production inputs and environmental conditions, without
test-only production pause or fault hooks. Platform checks not performed remain
explicitly unverified.

## 10. Integration references

The [desktop guide](../lib/clacks-desktop/README.md) documents configuration,
platform integration, dependency obligations and native verification. Resolved
dependency versions are pinned in `Cargo.lock`. Desktop and exporter paths must
remain compatible with Rust 1.99.1 and must not introduce OpenSSL.

Primary API references supporting the integration boundaries:

- [eframe App](https://docs.rs/eframe/latest/eframe/trait.App.html): native UI/logic and repaint integration.
- [Tokio GUI bridging](https://tokio.rs/tokio/topics/bridging) and [shutdown](https://tokio.rs/tokio/topics/shutdown): runtime progress and task ownership.
- [Standard-library file locks](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock): OS ownership tied to the lock handle.
- [interprocess local sockets](https://docs.rs/interprocess/latest/interprocess/local_socket/): cross-platform activation transport; endpoint recovery remains ownership-sensitive.
- [OS project directories](https://docs.rs/directories/latest/directories/struct.ProjectDirs.html): platform-specific configuration and local data locations.
- [tracing-appender rotation](https://docs.rs/tracing-appender/latest/tracing_appender/rolling/struct.Builder.html): time rotation and retained-file count.
- [OTLP Rust exporter](https://docs.rs/opentelemetry-otlp/latest/opentelemetry_otlp/) and [metrics adapter](https://docs.rs/metrics-opentelemetry/latest/metrics_opentelemetry/): standard export and producer integration; resolved versions are pinned in `Cargo.lock`.

## Desktop installation artifacts

See the [Windows packaging guide](../deploy/desktop/windows/README.md) for the
manual development artifact and tag-triggered draft release workflow. The
installer targets Windows 11 x64 and current-user scope, preserves application
state/credentials on uninstall and never initializes a workspace. The desktop
continues to own migrations and graceful shutdown. Unsigned early packaging
does not establish native Windows acceptance or final distribution trust.

The [macOS packaging guide](../deploy/desktop/macos/README.md) covers the native
Apple Silicon DMG for macOS 26 Tahoe and newer. Both platforms share source/tag
eligibility and one draft release; manual development builds select a platform.
Mac downloads require Developer ID signing, profile-authorized protected
credentials, hardened runtime, notarization and a stapled disk image. Account
and signing inputs require owner setup.
Installing, replacing or removing the app bundle preserves application state;
quit normally before replacement. Packaging checks do not establish native
installed-product acceptance or the complete MVP release workflows.

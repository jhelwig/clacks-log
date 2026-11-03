# Native Windows and macOS application menus

Status: proposed implementation specification for review. The Settings / View /
Help organization and scope are accepted; the integration below is the proposed
engineering design. No application implementation or runtime validation is
claimed by this document.

## Intent and scope

Move management commands out of the increasingly crowded in-window toolbar
into operating-system-native menus. Keep the selected egui/eframe desktop stack.
Use precise terms: the in-window toolbar contains workbench controls; the native
menu bar is window-attached on Windows and screen-level on macOS.

Preserve the logbook selector, existing synchronization status and direct
Unsynchronized changes/conflict-resolution access in their existing locations.
Do not move status from the footer merely to match a toolbar description.
Improving synchronization communication is a separate issue. Contact entry,
Save, operating controls, data semantics and synchronization are outside scope.
There is no new combined settings screen or speculative command inventory.

## Accepted command organization

| Menu | Items, in order |
| --- | --- |
| Settings | Manage logbooks…, Station profiles…, Equipment configurations…, Refresh settings; separator; Connect… or Connection…; separator; Appearance… |
| View | Reset workbench layout |
| Help | Workspace & diagnostics… |

Retain the existing Connect… / Connection… label according to enrollment state;
“account/server management” describes that existing workflow, not a new dialog.
Separators are an implementation choice for grouping. Do not add a Logbooks
menu. View and Help are appropriate single-entry menus because their extensions
are obvious; this does not authorize adding future commands now.

macOS additionally retains its conventional application menu and system actions.
The accepted Settings menu remains a top-level menu on both platforms; do not
silently move Appearance or the other accepted items into the application menu.
Do not add Edit or Window menus as part of this change.

## Approaches and recommendation

1. **Muda through eframe's public hooks (recommended).** Muda owns native menu
   objects, item updates and native activation. Clacks wires commands to its
   existing presentation owners. This retains eframe and uses documented window
   and event-loop integration facilities.
2. **Direct Win32/AppKit integration.** Technically possible through the same
   handles, but makes Clacks maintain menu construction and platform behavior
   already supplied by muda. No demonstrated requirement justifies that cost.

An egui-rendered menu bar does not satisfy the Windows/macOS native-menu
requirement. Replacing or forking eframe is not supported by the source findings.
The user requested this specification after the muda recommendation; the exact
dependency version and integration remain engineering choices, not prior user
decisions.

## Source findings and evidence boundary

The installed sources establish eframe 0.37.0 and locked winit 0.30.13:

- `eframe::NativeOptions::event_loop_builder` receives the winit builder before
  construction. Winit supplies Windows `with_msg_hook` and macOS
  `with_default_menu`.
- `eframe::CreationContext` implements `HasWindowHandle`; the private raw-handle
  field need not be accessed.
- `egui::Context::request_repaint` supports waking eframe from outside rendering.
- Muda supplies native attachment, item IDs, labels, enabled states, shortcuts
  and activation callbacks. Its upstream winit example demonstrates both target
  platforms. This is source evidence, not a tested Clacks integration.

Sources: [eframe native-menu request](https://github.com/emilk/egui/issues/3411),
[eframe 0.37.0 source](https://docs.rs/eframe/0.37.0/src/eframe/epi.rs.html),
[muda documentation](https://docs.rs/muda/latest/muda/),
[muda winit example](https://github.com/tauri-apps/muda/blob/dev/examples/winit.rs),
[muda item implementation](https://docs.rs/muda/latest/src/muda/items/normal.rs.html).
Upstream `latest` and `dev` references move. During implementation select and
lock a released muda version, inspect that release's equivalent APIs and record
the tested version. Do not assume an example on `dev` matches a published release.

## Integration and ownership

Keep composition in `lib/clacks-desktop/src/application.rs`; the executable
remains thin. Add a focused native-menu module in the desktop library. Retain
native menu objects on the UI thread for the window/application lifetime and
detach them before their associated handles become invalid. Do not recreate
menus each frame. Use target-specific dependencies for Windows/macOS.

On Windows, construct the menu and install the accelerator hook through
`NativeOptions::event_loop_builder`, following the selected release's winit
example. Attach the menu to the main window's HWND through `CreationContext`.
Pass messages through normally unless `TranslateAcceleratorW` handled them.
Keep native handles and their safety/lifetime justification inside this module.

On macOS, create and attach menus on the main thread after the application
exists. Disable winit's default menu only when installing its complete
replacement. Preserve standard application actions such as Hide, Hide Others,
Show All and Quit. Register Help as the application Help menu through muda's
facility. Use Clacks's shutdown action for Quit rather than assuming a predefined
native termination action runs the existing orderly-close path.

The root workbench owns these commands. Auxiliary native windows do not get
independent copies of settings state. A command activated while an auxiliary
window has focus targets the workbench and brings it forward when opening a
dialog. Do not attach a second Windows menu to every auxiliary window.

### Command delivery

Define a small internal command enum for the actual listed actions and Quit.
Use explicit stable menu IDs mapped to those variants; do not add a command
registry, plugin mechanism or generic action framework.

Use muda's activation callback to send the command through an ordinary
in-process channel and request a repaint of the root viewport. Drain commands
in the root application's logic phase, including when the window is idle or
hidden. The callback must not mutate dialog state or run storage operations.
Use one callback delivery route: do not also consume muda's default receiver
and risk duplicate dispatch. No polling timer, background menu worker,
persistent queue or retry subsystem is needed.

The application rechecks current command availability before execution, since
state can change between native activation and dispatch. Rejected stale events
do not mutate the workbench. Repeated opens reuse the existing destination and
preserve its edits. Muda 0.21.2 stores the callback in a OnceLock and cannot remove it. The callback
holds a weak reference to a window-owned delivery target; dropping that target
at teardown releases the sender and application context. No replacement callback
or global routing registry is needed for this one-application process.

### Existing workflows and focus

Extract callable open/action methods from the existing toolbar handlers rather
than duplicating their logic. Keep the current logbook, operating-management,
enrollment and appearance owners responsible for their state and validation.
Refresh settings sends the existing settings request and retains its existing
completion/error presentation. Layout reset changes only the existing pane sizes.

Workspace & diagnostics currently renders text inside an egui popup. Replace
that presentation with a small dismissible egui dialog containing the same
workspace path, diagnostic-log path and available account identity. This is the
necessary presentation change for a native menu action, not a new diagnostics
workflow. Existing visible errors and retry controls remain visible.

Native items have no egui widget ID. Capture the previously focused workbench
widget when opening a dialog and restore it on dismissal if still available.
Allow no prior focus; never invent an ID for the removed toolbar button. Reuse
the shared dialog first-focus, Escape, Save/Cancel and validation conventions.
Do not reset text, selection, operating settings or an unfinished contact.

### Availability and shortcuts

Use a single application-side availability calculation for native item state
and dispatch checks. Apply existing busy checks, including logbook pending
requests and operating-management responses. Workspace-dependent management,
refresh and layout actions require a ready workspace and settings snapshot.
Appearance and diagnostics remain available during opening or an opening failure.
Disable management actions while closing. An active modal blocks opening an
unrelated management destination; do not stack competing modal editors. Quit
continues through the existing shutdown handling even when a dialog is open.

Use Windows menu mnemonics for keyboard access. Do not invent shortcuts for
every management command; none are required by the accepted structure. Preserve
existing egui editing shortcuts. macOS Quit uses Command-Q and routes through
`DesktopApp::begin_close`; native close and Quit share query-flush, shutdown and
telemetry completion behavior. If additional accelerators are introduced later,
native handling must not also trigger an egui action for the same key event.

## Linux compatibility and failure handling

This delivery validates native menus on Windows/macOS. Muda's GTK menu attachment
does not directly fit eframe's Linux winit window. Keep Linux compiling and
usable with an egui menu bar carrying the same Settings / View / Help commands
and availability rules; this is an explicit platform fallback, not a claim of
native Linux support. Do not add GTK or replace Linux's window backend here.

On Windows/macOS, inability to initialize or attach menus is a desktop startup
error: return it through the existing startup error and cleanup path. Do not
silently restore the crowded toolbar. Keep callback registration and attachment
cleanup correct on partial initialization failure. Normal command-operation
errors continue through their existing UI and telemetry paths.

## Observability

Follow `docs/observability.md`. Use an initialization span with bounded platform
and outcome fields, a successful initialization event, and one handling-boundary
error for initialization/attachment failure. Use debug command-dispatch events
with the bounded command and accepted/stale-disabled outcome; warn for a broken
delivery channel while the application is otherwise running. Preserve downstream
operation spans and their existing success/failure semantics.

Count native-menu initialization attempts by platform/outcome and dispatched
commands by command/outcome through the existing metrics facilities. These count
menu infrastructure outcomes, not completed saves or refreshes. Do not emit
per-frame menu-update telemetry. Do not log workspace paths, account identity,
contact data, credentials or raw native-message payloads. Use the existing
desktop tracing context and exporter lifecycle; install no new subscriber.

## Validation and completion criteria

- Production-component tests dispatch real command variants and establish that
  the correct existing destination opens, repeated opens preserve edits, stale
  disabled commands do nothing, and focus returns without a removed button ID.
- Test the ready/opening/failed/closing and relevant busy/modal availability
  transitions. Exercise Refresh through the existing request path, not a fake
  parallel implementation. Verify existing logbook selection and visible sync
  access remain present while the old management buttons are absent.
- Verify Quit and native close enter the same existing graceful-close flow,
  including pending query persistence. Keep tests at the application boundary;
  do not duplicate the shutdown suite or mock operating-system internals.
- Verify bounded telemetry and truthful outcomes at initialization/dispatch
  boundaries; no tests of library formatting or per-frame logging.
- Run focused desktop tests, formatting, linting and the repository-required
  compile/documentation checks for the changed scope. Keep Linux build coverage.
- On actual Windows and macOS, verify menu placement, mouse and keyboard
  activation, disabled items, idle-window wake-up, main-window routing from an
  auxiliary window, dialog focus, preserved drafts, and normal shutdown. On
  Windows check menu geometry at different display scales and window sizes;
  on macOS check application menu actions, Command-Q and Help registration.
  Cross-compilation and egui interaction tests do not substitute for these runs.
- Report separately which platform checks actually ran and any unavailable
  environments. Do not claim Windows runtime verification from this Mac.

Implementation updates conflicting toolbar wording in maintained UI/logbook
contracts and relevant code comments. Preserve unrelated directory-lookup work.
No schema changes, server startup or persistent development-data changes are
required. The next artifact after review is the implementation plan; this
specification does not claim that the menus are delivered.

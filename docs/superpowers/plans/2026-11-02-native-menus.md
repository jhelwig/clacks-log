# Native Application Menus Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for inline execution, or superpowers:subagent-driven-development if the user chooses delegation. Steps use checkbox syntax for tracking.

**Goal:** Move existing management commands into native Windows/macOS Settings,
View and Help menus while preserving direct logbook selection and sync access.

**Architecture:** A focused desktop menu adapter owns native resources and sends
typed commands to the root application's logic phase. Existing presentation
owners implement the actions; one availability function governs both menu state
and dispatch. Linux uses an egui menu bar with the same command mapping.

**Tech Stack:** Rust 1.99.1, egui/eframe 0.37.0, locked winit 0.30.13, a verified
released muda version, existing Tokio channels and telemetry facilities.

**Spec:** [Native menus design](../specs/2026-11-02-native-menus-design.md).
The user requested this plan after reviewing the specification.

## Global Constraints

- Settings: Manage logbooks…, Station profiles…, Equipment configurations…,
  Refresh settings; separator; Connect… / Connection…; separator; Appearance….
- View: Reset workbench layout. Help: Workspace & diagnostics….
- Keep Settings top-level on both platforms; macOS also retains its conventional
  application menu. Do not add Logbooks, Edit or Window menus.
- Preserve existing sync status locations, Unsynchronized changes access,
  logbook selection, contact drafts and operating controls.
- No server startup, schema changes or persistent development-data changes.
- Select and lock a released muda version after checking its actual APIs;
  upstream `dev` examples are not a version contract.
- Keep native objects on the UI thread. No custom event loop, generic command
  registry, persistent queue, polling timer or background menu worker.
- Preserve unrelated working-tree changes, including directory-lookup work.
- Before implementation, use the worktree-placement and using-git-worktrees
  skills. Transfer only reviewed menu documents into the isolated checkout.
- Commits are review boundaries, not authorization to push or merge. When
  authorized, prepare each message in a file, inspect physical body wrapping at
  72 columns, and use `git commit --file`.

## Review Focus

- A command queued just before shutdown or a modal opens must not bypass its
  current availability rules: test in Task 1.
- Repeated menu activation must preserve an existing editor's unsaved contents
  and original focus target: test in Task 2.
- An idle or hidden workbench must receive the command without mouse movement;
  activation from an auxiliary window must target the root: Task 3 native checks.
- Partial initialization and callback teardown must not retain stale handles or
  bypass session cleanup: Task 3 inspection and real startup/close checks.
- Quit while a history query is awaiting persistence must use the same orderly
  shutdown path as native close: Task 4 application test and native check.

## File and interface map

Create `lib/clacks-desktop/src/menus.rs` for shared command types and a
platform-selected menu adapter; `menus/windows.rs` and `menus/macos.rs` own
native attachment. Create `ui/menu_commands.rs` for dispatch/availability and
`ui/menu_commands/tests.rs` for production-application tests. Create
`ui/menus.rs` for the Linux fallback and `ui/diagnostics.rs` for the existing
diagnostic content's dialog presentation. Use named modules, not `mod.rs`.

Modify `application.rs`, `lib.rs`, `ui.rs`, `ui/logbooks.rs`, `ui/operating.rs`,
`ui/operating/home.rs`, `ui/appearance.rs`, `ui/enrollment.rs`, `ui/workbench.rs`,
`ui/startup.rs`, and the affected existing UI tests. Modify root/member
`Cargo.toml` and `Cargo.lock` only when native integration is introduced.

Proposed crate-private interfaces (keep them private to the desktop crate):

```rust
enum MenuCommand {
    ManageLogbooks, StationProfiles, EquipmentConfigurations, RefreshSettings,
    Connection, Appearance, ResetLayout, Diagnostics, Quit,
}
impl DesktopApp {
    fn menu_enabled(&self, command: MenuCommand) -> bool;
    fn dispatch_menu(&mut self, context: &egui::Context, command: MenuCommand);
}
```

`MenuCommand` derives Copy, Clone, Debug, PartialEq and Eq. It is the identity
shared by native IDs, dispatch and Linux items; do not maintain a second enum.
Native objects are never added to `UiResources` used by headless tests.
The root app owns an optional `MenuAdapter` and a command receiver. Initialize
them through `install_menus(&mut self, menus: MenuAdapter,
receiver: std::sync::mpsc::Receiver<MenuCommand>)`; ordinary test construction
has neither. Tests can call the production dispatch method directly.

## Task 1: Establish command dispatch and availability

**Files:** create `menus.rs`, `ui/menu_commands.rs`,
`ui/menu_commands/tests.rs`; modify `lib.rs`, `ui.rs` and existing UI fixtures.

**Interfaces:** produce `MenuCommand`, `DesktopApp::menu_enabled` and
`DesktopApp::dispatch_menu`. Existing dialog owners remain the state owners.

- [ ] Add failing production-application tests named
  `menu_commands_respect_workspace_state`,
  `queued_command_rechecks_modal_and_closing_state`, and
  `busy_management_commands_do_not_mutate_state`. Use existing `Window` and
  ready-workspace fixtures; assert actual destination state and existing request
  receivers, not only enum mappings.
- [ ] Run `cargo +1.99.1 test -p clacks-desktop --lib menu_commands --locked --offline`.
  Record the expected missing-dispatch or behavior failure before implementation.
- [ ] Implement the shared command type and application-side policy. Management,
  refresh and reset require Ready plus a settings snapshot; preserve each owner's
  actual busy predicate. Appearance/diagnostics work during Opening/Failed.
  Closing disables management. Modal editors block unrelated destinations;
  repeated activation of the current destination is a no-op, not reinitialization.
  Quit remains routed to `begin_close`.
- [ ] Implement dispatch through the owner entry points completed in Task 2;
  until those are present, finish this task and Task 2 as one local test cycle
  rather than committing non-compiling stubs. Do not expose test-only interfaces.
- [ ] Add one debug dispatch event and
  `clacks.desktop.menu.commands` counter with bounded command and
  `accepted` / `stale_disabled` outcome. The accepted outcome means dispatch,
  never successful persistence. Use the existing telemetry context.
- [ ] Rerun the command tests after Task 2, with all policy and real-effect
  assertions passing; review Tasks 1–2 together as the first commit boundary.

## Task 2: Make existing destinations callable without toolbar widgets

**Files:** modify the UI owners in the map, `ui/tests.rs`,
`ui/tests/main_screen.rs`, `ui/tests/appearance.rs`, and
`ui/menu_commands/tests.rs`; create `ui/diagnostics.rs`.

**Interfaces:** owners expose crate-internal or parent-visible methods:
`LogbooksUi::open_management(opener: Option<egui::Id>)`,
`AppearanceUi::open(opener: Option<egui::Id>)`,
`EnrollmentUi::open(opener: Option<egui::Id>)`, and
`Workbench::reset_layout()`. Use the existing concrete logbook type name if
different when implementing; do not rename a type solely to match this plan.
Operating management exposes `open_profiles`, `open_equipment`, and
`refresh_settings`, retaining its existing stack/request ownership. Opening
methods take `Option<egui::Id>`; refresh takes the existing settings sender.

- [ ] Add failing tests `menu_open_preserves_editor_draft`,
  `menu_dialog_restores_previous_focus`, `menu_open_without_focus_is_valid`,
  `refresh_menu_uses_existing_request_path`, and
  `diagnostics_menu_discloses_existing_details`. Assert a real unsaved value
  survives repeated activation, focus returns to the original live widget,
  refresh produces `SettingsAction::Refresh`, and diagnostics shows the same
  path/account fields only when opened.
- [ ] Run the Task 1 command plus
  `cargo +1.99.1 test -p clacks-desktop --lib ui::tests --locked --offline`;
  establish the relevant failures.
- [ ] Extract the existing open/action bodies into the owner methods. Existing
  toolbar handlers may temporarily call them so this step remains independently
  runnable before native menus land. Ensure enrollment state/label observation
  runs even after its toolbar opener is removed.
- [ ] Change appearance's required opener ID to optional focus state and update
  existing callers. Capture current root focus before opening; retain it across
  repeated opens. Restore only when a valid destination remains; otherwise
  follow the existing first-focus behavior without fabricated IDs.
- [ ] Replace `startup::details` popup content with the diagnostics dialog;
  preserve the existing always-visible `startup::diagnostics` and retry paths.
  Implement normal dismissal and focus restoration, not a second diagnostic model.
- [ ] Wire dispatch to these methods. Bring the root window forward for menu
  commands opening destinations. Reset layout through the existing pane reset
  flag; do not modify contact state or other presentation preferences.
- [ ] Run Tasks 1–2 tests and the existing logbook/operating/appearance tests.
  Review telemetry assertions for bounded fields and truthful dispatch counts.
  Commit this coherent refactoring only after all affected tests pass.

## Task 3: Attach native menus and deliver events

**Files:** root/member manifests, lockfile, `application.rs`, `menus.rs`,
`menus/windows.rs`, `menus/macos.rs`, `ui.rs`.

**Interfaces:** `MenuAdapter::prepare() -> anyhow::Result<Self>` constructs
native resources; `configure(&self, options: &mut eframe::NativeOptions)` installs
builder hooks; `attach(&mut self, creation: &eframe::CreationContext<'_>,
sender: std::sync::mpsc::Sender<MenuCommand>) -> anyhow::Result<()>` attaches and
registers delivery. `set_enabled(&self, command: MenuCommand, enabled: bool)` and
`set_connection_label(&self, enrolled: bool)` update native items. Use a no-op
adapter on Linux; its visible menu is Task 4. Methods hide platform handles.

- [ ] Inspect and select a released muda version with the required APIs. Add it
  to workspace dependencies and Windows/macOS target dependencies. Add direct
  winit 0.30.13 and raw-window-handle dependencies compatible with eframe, and
  Windows messaging features to the existing windows-sys dependency. Preserve
  unrelated dependency versions; inspect the lockfile diff.
- [ ] Build the exact accepted menu labels/order and stable IDs in `MenuAdapter`.
  Do not create objects per frame. Configure Windows `with_msg_hook` and attach
  via `HasWindowHandle`; consumed accelerator messages must not reach egui too.
- [ ] Configure macOS default-menu replacement and attach after application
  creation on the main thread. Include conventional application actions; make
  Command-Q an application command routed to `MenuCommand::Quit`, not direct
  native termination. Register the Help menu using the released library API.
- [ ] Register a single muda callback that maps its ID, sends once and calls
  `request_repaint_of(egui::ViewportId::ROOT)`. Drain the receiver in
  `DesktopApp::logic`, recheck availability and dispatch. No timer or dual receiver.
  Ensure root logic can observe readiness and settings changes without relying
  on a preceding painted frame.
- [ ] Refresh native enabled states and connection label from current owner
  state, including after commands and while hidden. Reuse `menu_enabled`;
  avoid a second policy implementation in native code.
- [ ] Implement explicit menu detach/callback cleanup while the window handle
  is valid, using eframe's lifecycle facilities verified in the pinned source.
  Retain resources until detach and clean partial attachment on errors. Return
  setup failures through the existing application cleanup path. Do not install
  an egui fallback silently on Windows/macOS.
- [ ] Add an initialization span and one
  `clacks.desktop.menu.initializations` counter per attempt with bounded platform
  and success/failure outcome. Report failure once at the handling boundary;
  warn on unexpected delivery-channel loss while running. No sensitive values
  or per-frame events. Verify telemetry through real initialization where
  available, without native failpoints or a fake operating system.
- [ ] Run `cargo +1.99.1 check -p clacks-desktop --all-targets --locked --offline`
  and the Windows target equivalent with `--target x86_64-pc-windows-msvc`.
  Missing dependencies/targets are setup limitations, not passing checks.
- [ ] On macOS, launch with a disposable local workspace and validate idle/hidden
  wake-up, root routing from an auxiliary window, disabled items, Help and Quit.
  Inspect all early-return paths for callback/handle cleanup. Perform the same
  native checks on Windows when an environment is available; record evidence
  separately. A successful compilation does not prove runtime support.
- [ ] Review and commit the native integration once its available checks pass;
  explicitly retain any outstanding Windows runtime validation in the report.

## Task 4: Remove toolbar commands and validate the completed flow

**Files:** `ui.rs`, `ui/menus.rs`, `ui/logbooks.rs`, `ui/operating.rs`,
`ui/operating/home.rs`, `ui/workbench.rs`, `ui/enrollment.rs`, `ui/startup.rs`,
`ui/menu_commands/tests.rs`, `ui/tests/main_screen.rs`, `ui/tests.rs`;
`docs/desktop-ui.md`, `docs/logbooks.md`, `lib/clacks-desktop/README.md`.

**Interfaces:** `ui/menus.rs` renders the Linux menu bar from the same enum,
labels and `menu_enabled` policy, dispatching through `dispatch_menu`.

- [ ] Add failing component tests `toolbar_retains_logbook_and_conflict_access`
  and `linux_menus_route_management_commands`. Assert old management buttons and
  egui toolbar menu triggers are absent on native platforms while selector and
  Unsynchronized changes remain; assert footer sync status is unchanged.
  Use target configuration rather than a production test-only platform switch.
- [ ] Add `menu_quit_waits_for_pending_query` beside the existing lifecycle
  tests: send Quit through production dispatch, assert the current query flush
  gate and single BeginShutdown request, and assert repeated Quit/native close
  preserves the original shutdown deadline. Extend fixtures only as necessary
  to exercise the real query path.
- [ ] Run affected tests to record failures, remove the old management buttons
  and menus, and render Linux Settings/View/Help with the same grouping and
  commands. Remove dead opener code; preserve the operating Home frame if still
  required for refresh/stack semantics. Do not redesign the operating strip.
- [ ] Update the existing tests that clicked removed buttons to activate the
  production command and assert the resulting destination. Keep separate native
  acceptance evidence for the actual operating-system menu click path.
- [ ] Correct remaining current-contract/code-comment references to the old
  application bar and adjacent Manage logbooks button. Preserve historical
  decisions as history with the current UI contract as the placement authority.
  Document native platforms, Linux fallback and verification procedure in the
  desktop README without recording machine-specific environment details.
- [ ] Run `cargo +1.99.1 test -p clacks-desktop --lib --locked --offline`,
  `cargo +1.99.1 fmt --all --check`,
  `cargo +1.99.1 clippy -p clacks-desktop --all-targets --locked --offline -- -D warnings`,
  and `RUSTDOCFLAGS='-D warnings' cargo +1.99.1 doc -p clacks-desktop --no-deps --document-private-items --locked --offline`.
  Run the desktop binary check and Windows compile check; retain Linux build/test
  coverage through the existing CI environment. Run any additional repository
  checks required by the final changed scope, without starting the server.
- [ ] Complete native acceptance: mouse/keyboard invocation, Windows mnemonics
  and display scaling, macOS Command-Q/Hide/Help, idle delivery, busy/modal states,
  repeated opens, restored focus, preserved drafts, auxiliary-window routing,
  and graceful shutdown. Record OS/build, action and observed result. If Windows
  is unavailable, mark runtime acceptance incomplete and identify the exact
  checks needed; do not claim the feature verified on both platforms.
- [ ] Inspect `git diff --check` and scope, then review the complete branch
  against the specification. Commit the verified final change with the required
  substantive, physically wrapped message. Leave integration/push/merge to the
  user's authorization and report actual tests and remaining platform limits.

## Plan self-review

The four tasks cover command organization, platform ownership, wake-up and
dispatch, availability/focus, existing workflows, Linux behavior, initialization
failure cleanup, observability, documentation and native acceptance. Tasks 1–2
form one compiling refactoring boundary; Tasks 3–4 complete platform integration
and removal of the toolbar controls. No product decisions remain queued.
The remaining release/API and lifecycle checks are implementation verification,
not grounds for inventing a replacement architecture before a demonstrated gap.

## Implementation result

Implemented on `feature/native-menus` in one compiling change, with command
coverage under the existing `ui/tests` fixture. Final review corrections cover
native-close ordering, modal admission while a saved contact is loading, literal
ampersands in native menu text, and layout reset preserving the contact draft.

Automated verification: 19 menu tests and 167 desktop tests pass; two desktop
tests remain ignored. The desktop run explicitly excludes
`enrollment::tests::saved_native_credential_resumes_the_actual_client`: its
separate full-suite attempt failed with `verify two desktop sessions: channel
closed`, with a process sample showing a native Keychain read wait. This is not
a claim that the complete desktop suite passes. Formatting, strict all-target
Clippy and private-item documentation checks pass.

The exact production Windows menu module type-checks against the pinned
libraries. Full Windows application checking is blocked in existing cryptographic
C dependencies by the unavailable Windows SDK and C headers. Linux fallback has
host component coverage; no full Linux build was performed here.

Native macOS UI acceptance is assigned to the user at their request. Windows
runtime acceptance also remains outstanding. Neither source review nor successful
compilation substitutes for the native acceptance checklist above. No branch
integration, push or merge is part of this implementation.

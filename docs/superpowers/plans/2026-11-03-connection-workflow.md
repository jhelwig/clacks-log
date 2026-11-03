# Connection Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Make Connect complete connection setup without user-managed recovery.

**Architecture:** Consolidate intent handling in the existing enrollment worker.
Reuse Huskarl authorization/refresh, native credential storage, existing completion
state and session admission. Extract the existing population predicate for preflight
while retaining its transaction-time enforcement.

**Tech Stack:** Rust 1.99.1, egui/eframe, Tokio, SQLx, Huskarl 0.11.9 and the
existing native keyring adapter. No new dependencies or migrations.

**Spec:** [Connection workflow design](../specs/2026-11-03-connection-workflow-design.md)

## Global Constraints

- Recovery decisions remain internal; the UI submits Connect or Cancel.
- Preserve installed account/server boundaries, pending work and native secrets.
- Network/server/access failures do not imply invalid credentials.
- Keep the current workspace; do not add startup policy or background retry.
- Tests use temporary workspaces/disposable databases and production migrations.
- Before native validation follow the applicable user-owned development inventory
  and instructions; launch with explicit isolated workspace/configuration/log paths.
- Preserve unrelated changes. Before commits inspect a message file, hard-wrap
  prose at 72 columns and use `git commit --file`; do not merge or deploy.

## Review Focus

- A server address changes after refresh material was saved but before binding:
  Task 2 verifies guarded replacement without altering an installed connection.
- Native storage is inaccessible rather than absent: Task 2 verifies preservation
  and no browser fallback from access failure.
- Installation commits before cleanup fails: Task 2 verifies exact binding reuse.
- Data changes after preflight: Task 1 retains installation-time rejection.
- A cancelled worker has not exited: Task 3 verifies no duplicate admission.

## Task 1: Reject ineligible standalone logs before authorization

**Files:** `lib/clacks-client-store/src/enrollment/first.rs`, its existing enrollment
integration suite, `lib/clacks-desktop/src/enrollment/worker.rs`, and
`lib/clacks-desktop/src/enrollment/completion_tests.rs`.

**Interfaces:** Add `pub async fn check_eligible(store: &ClientStore) -> anyhow::Result<()>`
to `enrollment::first`. Share the current transaction-level population predicate
with `install`; do not duplicate its rules or weaken its atomic guard. Keep fixed
queries in their existing SQL files. The worker calls preflight only for first
connection, before native-key creation/browser work.

- [x] Extend `connect_preserves_populated_standalone_log` and existing store cases:
  independently authored contacts/customized definitions reject; unchanged bundled
  initialization passes; a write after successful preflight still rejects install.
- [x] Run the affected tests and confirm the preflight assertions fail before the
  implementation; retain real production-store operations for fixture setup.
- [x] Extract the predicate, wire preflight, and return a specific existing
  populated-workspace error before any authorization side effects.
- [x] Run `SQLX_OFFLINE=true cargo test -p clacks-client-store --all-targets --locked --offline`
  and the affected desktop completion tests; require passing results.
- [x] Commit the independently verified eligibility change with a reviewed message.

## Task 2: Consolidate connection and internal recovery

**Files:** `lib/clacks-desktop/src/enrollment.rs`, `enrollment/worker.rs`,
`enrollment/flow.rs`, `enrollment/completion.rs`, `session.rs`, and the existing
`enrollment/worker/tests.rs`, `enrollment/completion_tests.rs`, `enrollment/tests/`
suites. Remove the obsolete buttons and wire the primary action in this task together
with command/status changes so the crate and its tests remain coherent. Task 3
finishes presentation, dismissal and assembled UI validation.

**Interfaces:** Replace command variants with `Connect { server: String, label: String }`
and `Cancel`. Retain `worker::execute` and `flow::authorize`/`flow::resume` signatures.
Keep status presentation-oriented: Idle carries server/enrolled context; Authorizing
carries the existing nonlogged URL; Saving represents connection progress; Ready
signals local completion; Failed carries a safe summary and optional sanitized
technical details. Remove resume/discard availability flags from all producers.

- [x] Use the pinned Huskarl `core::Error::verdict()` and returned
  `OAuthError::code()` accessors through the preserved error chain. The library
  deliberately excludes HTTP 429/5xx from rejection verdicts even when their bodies
  contain OAuth errors. Use `OAuthErrorCode::InvalidGrant`
  for definite refresh rejection, native `NoEntry` for absence, and preserve source
  errors for other outcomes. Never inspect formatted error strings.
- [x] Extend existing real enrollment fixtures with assertions for: a retryable
  server failure preserving the key; saved refresh completion without fresh browser
  approval; rejected refresh causing one fresh browser flow; inaccessible native
  material not causing deletion/replacement; corrected unbound server retiring only
  its uninstalled material; installed server mismatch preserving the binding; a verified wrong-account
  saved credential entering one account-constrained browser flow, with a repeated
  wrong-account approval returning an error and retaining the installed binding.
  Use actual protocol outcomes/environmental failures supported by existing fixtures;
  do not add production test hooks or a mock credential-service abstraction.
- [x] Extend completion coverage to reenter after a committed binding with pending
  cleanup, asserting the same account/writer and no new authorization. Use existing
  persistence operations to establish that legitimate state, not broken tables.
- [x] Run the affected cases to establish failing Clacks behavior, then implement
  the spec's ordered handling in the worker. Validate labels on every Connect path.
  Keep server replacement guarded by actual binding and native entry inspection.
- [x] Preserve cancellation, session joining, service reloading and existing
  same-account/new-writer recovery rules. Distinguish committed connection from
  remaining cleanup failure in outcomes and messages.
- [x] Add bounded workflow outcome reporting using existing tracing/metrics patterns.
  Verify credential/URL exclusion and truthful committed outcomes in the existing
  telemetry test support; avoid testing subscriber formatting or library internals.
- [x] Run `SQLX_OFFLINE=true cargo test -p clacks-desktop --locked --offline enrollment`
  under the documented native test prerequisites. Record any platform constraint;
  do not claim native credential coverage from skipped scenarios.
- [x] Commit the verified workflow changes with a reviewed message.

## Task 3: Present one connection operation and verify the assembled UI

**Files:** `lib/clacks-desktop/src/ui/enrollment.rs`, `ui/enrollment/tests.rs`,
`session.rs` and its existing tests as needed; `lib/clacks-desktop/README.md`.
Use existing shared dialog, fields, validation and feedback components.

**Interfaces:** Consume Task 2's Connect command and presentation statuses.
A valid idle/failed form submits its current server/label regardless of pending
internal state. An installed binding alone fixes the server; busy state disables
input and duplicate submission. Ready closes the dialog and restores opener focus.

- [x] Default new connections to `https://clacks.w7hex.com` and “Desktop”; verify
  startup inspection does not clear that default and one click submits both values.
  Retain saved/installed server values and editable self-hosted input.
- [x] Update the existing form scenario to assert retry uses Connect with retained
  inputs, edited server is submitted before enrollment, and no recovery-management
  controls appear. Assert reauthorization retains its installed server.
- [x] Extend actual session/UI scenarios for cancellation followed by retry:
  Cancel/Escape/close dismiss, worker admission remains serialized, and terminal
  success closes the dialog without presenting a path or synchronization claim.
- [x] Implement the field helper, progress copy, browser fallback, sanitized
  expandable error details and completion behavior from the design.
- [x] Update the desktop README from its temporary implementation note to describe
  delivered behavior. Confirm the owning enrollment contract and design agree.
- [x] Run `SQLX_OFFLINE=true cargo test -p clacks-desktop --locked --offline`,
  `SQLX_OFFLINE=true cargo clippy -p clacks-desktop -p clacks-client-store --all-targets --locked --offline -- -D warnings`,
  `cargo fmt --all --check`, and `git diff --check`. Resolve meaningful failures.
- [x] Follow the development inventory and desktop UI completion gates: inspect
  the production application in all states listed in the spec, resize it, exercise
  keyboard dismissal/details, and record screenshots/platform/remaining Windows
  acceptance limitations. No default workspace or persistent development account
  is used for automated tests.
- [x] Review the final diff against the design and commit with a reviewed message.
  Report behavior and evidence; leave integration/merge to the user.

## Execution evidence

The owning enrollment contract, desktop README and browser approval copy describe
in-place enrollment and internal recovery. Fresh connections use the hosted server
and existing Desktop label. No new dependencies or migrations were introduced.

The client-store suite passed all 92 tests. The signed macOS desktop suite passed
205 tests, with two child-process fixture entry points ignored by the outer runner
and invoked through their owning tests. Native enrollment exercised real protected
storage, the actual issuer and disposable databases. Typed classification covers
native access denial; a live macOS credential-access denial was not forced.

Fresh review identified two startup/completion cases: late inspection overwriting
the saved server with the hosted default, and a committed same-account credential
replacement failing to reload services when cleanup failed. Both had failing
regression tests before their fixes and passed in the final full desktop suite.

Tasks 1–3 share one implementation commit because the command, session and UI
migration is coupled. Native Windows visual and credential acceptance remains
unverified on this macOS host. Native UI evidence is recorded with delivery.

The production macOS application was inspected in an explicitly isolated workspace
at 100% application scale, in light and dark appearance, at normal and maximized
window sizes. Hosted defaults, invalid input, unavailable-server failure, expanded
diagnostics, server correction, browser progress, Cancel/Escape dismissal, first
connection and same-account reauthorization were exercised. Both successful browser
round trips closed the dialog and showed Server connected. The installed server
rejected editing. Populated-workspace rejection and interrupted completion were
verified by automated production-store/session tests rather than forced in the UI.

Matching server/browser assets must be built with `cargo leptos build`; combining
an ordinary Cargo server build with cargo-leptos development assets produced a
hydration mismatch during inspection. Rebuilding both through the framework's
standard command resolved it without changing authorization policy.

# Connection Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

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

- [ ] Extend `connect_preserves_populated_standalone_log` and existing store cases:
  independently authored contacts/customized definitions reject; unchanged bundled
  initialization passes; a write after successful preflight still rejects install.
- [ ] Run the affected tests and confirm the preflight assertions fail before the
  implementation; retain real production-store operations for fixture setup.
- [ ] Extract the predicate, wire preflight, and return a specific existing
  populated-workspace error before any authorization side effects.
- [ ] Run `SQLX_OFFLINE=true cargo test -p clacks-client-store --all-targets --locked --offline`
  and the affected desktop completion tests; require passing results.
- [ ] Commit the independently verified eligibility change with a reviewed message.

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

- [ ] Use the pinned Huskarl `core::Error::verdict()` and returned
  `OAuthError::code()` accessors through the preserved error chain. The library
  deliberately excludes HTTP 429/5xx from rejection verdicts even when their bodies
  contain OAuth errors. Use `OAuthErrorCode::InvalidGrant`
  for definite refresh rejection, native `NoEntry` for absence, and preserve source
  errors for other outcomes. Never inspect formatted error strings.
- [ ] Extend existing real enrollment fixtures with assertions for: a retryable
  server failure preserving the key; saved refresh completion without fresh browser
  approval; rejected refresh causing one fresh browser flow; inaccessible native
  material not causing deletion/replacement; corrected unbound server retiring only
  its uninstalled material; installed server mismatch preserving the binding; a verified wrong-account
  saved credential entering one account-constrained browser flow, with a repeated
  wrong-account approval returning an error and retaining the installed binding.
  Use actual protocol outcomes/environmental failures supported by existing fixtures;
  do not add production test hooks or a mock credential-service abstraction.
- [ ] Extend completion coverage to reenter after a committed binding with pending
  cleanup, asserting the same account/writer and no new authorization. Use existing
  persistence operations to establish that legitimate state, not broken tables.
- [ ] Run the affected cases to establish failing Clacks behavior, then implement
  the spec's ordered handling in the worker. Validate labels on every Connect path.
  Keep server replacement guarded by actual binding and native entry inspection.
- [ ] Preserve cancellation, session joining, service reloading and existing
  same-account/new-writer recovery rules. Distinguish committed connection from
  remaining cleanup failure in outcomes and messages.
- [ ] Add bounded workflow outcome reporting using existing tracing/metrics patterns.
  Verify credential/URL exclusion and truthful committed outcomes in the existing
  telemetry test support; avoid testing subscriber formatting or library internals.
- [ ] Run `SQLX_OFFLINE=true cargo test -p clacks-desktop --locked --offline enrollment`
  under the documented native test prerequisites. Record any platform constraint;
  do not claim native credential coverage from skipped scenarios.
- [ ] Commit the verified workflow changes with a reviewed message.

## Task 3: Present one connection operation and verify the assembled UI

**Files:** `lib/clacks-desktop/src/ui/enrollment.rs`, `ui/enrollment/tests.rs`,
`session.rs` and its existing tests as needed; `lib/clacks-desktop/README.md`.
Use existing shared dialog, fields, validation and feedback components.

**Interfaces:** Consume Task 2's Connect command and presentation statuses.
A valid idle/failed form submits its current server/label regardless of pending
internal state. An installed binding alone fixes the server; busy state disables
input and duplicate submission. Ready closes the dialog and restores opener focus.

- [ ] Update the existing form scenario to assert retry uses Connect with retained
  inputs, edited server is submitted before enrollment, and no recovery-management
  controls appear. Assert reauthorization retains its installed server.
- [ ] Extend actual session/UI scenarios for cancellation followed by retry:
  Cancel/Escape/close dismiss, worker admission remains serialized, and terminal
  success closes the dialog without presenting a path or synchronization claim.
- [ ] Implement the field helper, progress copy, browser fallback, sanitized
  expandable error details and completion behavior from the design.
- [ ] Update the desktop README from its temporary implementation note to describe
  delivered behavior. Confirm the owning enrollment contract and design agree.
- [ ] Run `SQLX_OFFLINE=true cargo test -p clacks-desktop --locked --offline`,
  `SQLX_OFFLINE=true cargo clippy -p clacks-desktop -p clacks-client-store --all-targets --locked --offline -- -D warnings`,
  `cargo fmt --all --check`, and `git diff --check`. Resolve meaningful failures.
- [ ] Follow the development inventory and desktop UI completion gates: inspect
  the production application in all states listed in the spec, resize it, exercise
  keyboard dismissal/details, and record screenshots/platform/remaining Windows
  acceptance limitations. No default workspace or persistent development account
  is used for automated tests.
- [ ] Review the final diff against the design and commit with a reviewed message.
  Report behavior and evidence; leave integration/merge to the user.

## Documentation reconciliation already included with this plan

The owning enrollment contract now describes in-place first enrollment and internal
recovery. The desktop README distinguishes the required cleanup from the still-old
implementation until Task 3 lands. No application behavior is claimed as delivered
by this documentation change. This plan has not been executed.

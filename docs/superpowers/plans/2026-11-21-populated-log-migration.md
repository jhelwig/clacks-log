# Populated Log Migration Implementation Plan

> **For agentic workers:** Use `superpowers:executing-plans` or `superpowers:subagent-driven-development` when implementation is authorized. Steps use checkboxes for tracking.

**Goal:** Deliver the documented export, empty-workspace enrollment, and
re-import workflow for [#26](https://github.com/jhelwig/clacks-log/issues/26).

**Architecture:** The desktop sync worker publishes connectivity and exchange
progress on its existing status channel. The session forwards those values and
the footer renders them. The installed inbound checkpoint and the durable
pending-work view determine when the worker can report **Up to date**.

**Tech stack:** Rust 1.99.1, Tokio watch channels and exchange gate, egui footer,
existing local client store and sync protocol.

**Spec:** [Populated log migration and desktop sync status](../specs/2026-11-21-populated-log-migration-design.md).

## Constraints and review focus

- Keep sync status general across enrollment, reconnect, and steady-state use.
  Keep workspace lifecycle reporting with the session.
- Reuse the installed checkpoint, pause gate, pending-work read, and existing
  sync telemetry. Keep import, export, enrollment and matching behavior intact.
- Review state transitions in the implementation diff. Check fresh connection
  and reconnect, a follow-up exchange after a change, an import holding the
  gate, a settled exchange with unresolved work, and worker failure. The sole
  new executable check is the partial-receipt regression below.

## Task 1. Publish and display the worker's status

**Files**

- `lib/clacks-desktop/src/synchronization.rs` owns the status type, connection
  loop and pause gate.
- `lib/clacks-desktop/src/synchronization/exchange.rs` owns negotiation,
  exchange progress and the settled transition.
- `lib/clacks-desktop/src/session.rs` owns worker lifetime and status forwarding.
- `lib/clacks-desktop/src/ui/startup.rs` renders the footer.
- `lib/clacks-desktop/src/synchronization/exchange/tests.rs` houses the
  cross-platform regression, registered with `#[cfg(test)]` in `exchange.rs`.
  The existing native enrollment checks stay in their current module.

- [ ] **Add the single regression scenario.** Name it
  `status_stays_syncing_until_initial_receipt_completes`. Run the worker's
  production `exchange::run` with a temporary, empty enrolled `ClientStore`
  and the same `watch::Sender<Status>` used in production. Connect it to a
  loopback WebSocket peer so the test controls delivery without changing the
  production server or using the operating system's credential store. Use
  `AccountMetadata::initialize` for a valid matching `Hello` and the
  `Start`/`Page`/`End` sequence from
  `lib/clacks-client-store/tests/transfers/windows.rs` for a full receipt.
  The page carrying the baseline is sufficient. Handle the worker's directory
  configuration request as incidental to the exchange.

  Withhold `End` until the peer receives `PageAck`. At that barrier assert
  `Status::Syncing` and that `synchronization::state(&store).await?.checkpoint`
  is `None`. Release `End`, observe `Status::UpToDate` via the watch receiver,
  and assert that the stored checkpoint equals the one sent by the peer.
  Bound the scenario with a timeout and use the worker's cancellation token
  for shutdown. The protocol messages provide the barriers without sleeps.

- [ ] **Run the test against the existing behavior.** Wire the sender and new
  status variants as needed to compile the test, keeping the current behavior
  of publishing only connection progress until the exchange logic changes.
  Run the command below and confirm it fails because the held transfer does
  not report `Syncing`.

  ```sh
  cargo +1.99.1 test -p clacks-desktop --lib --locked --offline \
    synchronization::exchange::tests::status_stays_syncing_until_initial_receipt_completes -- --exact
  ```

- [ ] **Publish progress from the worker.** Pass `&watch::Sender<Status>` to
  `exchange::run` from the connection loop. Keep `Disconnected`, `Connecting`,
  and `Failed(String)` and replace the connection-only `Connected` state with
  `Syncing`, `Paused`, `UnsynchronizedChanges`, and `UpToDate`. Before
  negotiation and on each new exchange, publish `Syncing`. Give the existing
  `ExchangeGate::exchange` call an `on_wait: impl FnOnce()` callback at its
  existing contended acquisition branch. The worker supplies a callback that
  reports `Paused` and returns to `Syncing` when it obtains the guard. Adjust
  the existing pause test's call site for the parameter. Import continues to
  hold and release the existing shared guard.

- [ ] **Publish the settled state at the existing idle boundary.** After
  inbound completion and outgoing publication, take a fresh read of
  `synchronization::state(store).await?.pending` on the path to idle. Use the
  full pending-work view, including items that `pending::publish` skips. Check
  `dirty` and `changes.has_changed()` before entering idle and recheck local
  notifications after the pending read so an intervening save starts another
  exchange. Publish `UpToDate` for an empty pending set and
  `UnsynchronizedChanges` otherwise. A server notification or local change
  reenters the normal exchange path. A failed exchange returns to the
  connection loop's existing error handling and retry policy.

- [ ] **Wire the footer.** Keep the session's forwarding path and render the
  exact labels in the spec, including the existing failure details. The
  session initializes the display to disconnected before a worker starts.
  If a worker task terminates unexpectedly, preserve the session's task
  failure diagnostic as a lifecycle fallback and stop forwarding that task's
  last live status. Keep the live worker channel free of session or UI writes.
  Render workspace opening, standalone, failure and shutdown from session
  lifecycle state as before.

- [ ] **Rerun the same targeted test.** Confirm one passing test with the
  partial-receipt and committed-checkpoint assertions. Review the state
  mappings and existing tracing at the changed branches in the diff.

## Task 2. Document the migration entry points

**File:** `lib/clacks-desktop/README.md`, under **Server connection**.

- [ ] **Write the walkthrough.** Point to **File → Export ADI…**, **All contacts
  in this workspace**, and **Ordinary ADI**. Keep the original workspace and
  launch with `--workspace <directory>` pointing to a fresh directory. Use
  **Connect…** with the existing account and wait for **Up to date** before
  opening **File → Import ADI…**. Explain destination tag selection, reuse and
  explicit possible-match review. Describe the status and data limits from
  the spec, including preservation of supported claims without fabricating
  external associations. Keep reauthorization and ordinary offline return
  under the existing connection guidance.
- [ ] **Review the guide against the menu labels, launch option, and status
  transitions.** Run `git diff --check` on the implementation diff. Record
  the one regression result as the evidence for the new status behavior.

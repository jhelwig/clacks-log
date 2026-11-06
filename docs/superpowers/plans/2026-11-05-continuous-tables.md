# Continuous Tables Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task by task.

**Goal:** Deliver issue #88's continuous scrolling tables.

**Architecture:** Keep store paging internal. Let egui_extras virtualize all
matching rows, requesting only the visible 100-row pages through existing
runtime channels. Share row storage between Worked Before presentations while
retaining each presentation's viewport and keyboard target.

**Tech Stack:** Rust 1.99.1, egui/eframe/egui_extras 0.37, SQLx, egui_kittest.

**Spec:** ../specs/2026-11-05-continuous-tables-design.md

## Global Constraints

- Preserve native identity selection, applied-query semantics and offline use.
- No new dependency, schema, protocol or full-account in-memory query engine.
- Preserve Rust 1.99.1 and use isolated data paths for native inspection.
- Reuse store/runtime observability; no payloads in diagnostics.

## Review Focus

- Show selected has a different logical extent from the filtered match count.
- Arrow navigation must fetch, focus and activate a row across a read boundary.
- Far-apart Worked Before viewports must not cancel each other's useful reads.
- A failed read must not retry endlessly or erase usable rows.
- Mutations and changed query inputs must not mix stale pages into current rows.

### Task 1: Virtual rows and data ownership

**Files:** `lib/clacks-desktop/src/ui/contact_table.rs`, new
`contact_table/data.rs`, `contact_table/interaction.rs`, existing table tests.

**Interfaces:** `TableContent` consumes bounded row data and initial/loading
feedback. `TableAction` produces visible-page demand and an activated QSO ID.
Data supports indexed reads, identity lookup, bounded retention and page install.
Interaction retains a pending absolute keyboard target until data arrives.

- [x] Add a component scenario that scrolls beyond the first 100 rows and
  requests the corresponding offset, plus keyboard crossing and selected-only
  extent assertions. Run it and observe the missing behavior.
- [x] Implement full-result `TableBody::rows`, positioned placeholders and
  bounded page storage. Preserve shared headers, styling and column widths.
- [x] Verify table component tests, including the existing column/focus tests.

### Task 2: Integrate both query owners

**Files:** `ui/contacts.rs`, `contacts/responses.rs`, `contacts/membership.rs`,
`ui/worked_before.rs`, `worked_before/model.rs`, `worked_before/window.rs`,
existing contacts/Worked Before/workbench tests.

**Interfaces:** Query owners consume table demand; existing `Page` and
`WorkedBefore` session actions remain the bounded read boundary. Replies install
at their actual offset only for the current complete request inputs.

- [x] Adapt pagination-specific layout tests to fixed count/status controls;
  add request/reply scenarios for direct jumps, refresh, stale input rejection,
  errors/retry and simultaneous Worked Before viewport demand. Observe failures.
- [x] Remove pagination controls, connect viewport demand, and retain only useful
  pages. Preserve prior results while refreshing and reset scrolling for changed
  result scope/order. Keep selection counts accurate without stale row mixing.
- [x] Run the desktop suite, excluding only documented native credential tests
  that require the separately provisioned test host. Run fmt and clippy.

### Task 3: Contracts and acceptance

**Files:** `docs/history.md`, `docs/desktop-ui.md`, client-store/desktop READMEs.

- [x] Replace user-facing pagination wording with the issue #88 contract and
  document internal bounded reads, full-result scrollbar and independent views.
- [x] Build and inspect the production desktop with isolated explicit data paths;
  record exercised states, screenshots and unavailable platform checks.
- [x] Review the full diff and changed tests against the requirement/mistake
  criterion; request independent whole-branch review, resolve real findings and
  rerun affected checks. Leave the branch reviewable with evidence.

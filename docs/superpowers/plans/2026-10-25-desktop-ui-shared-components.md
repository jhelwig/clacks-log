# Desktop UI and Shared Components Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task using native execution. Steps use checkbox syntax for tracking. Read the specification and this entire plan before coding.

**Goal:** Apply the agreed desktop design to the working application, operating dialogs and reusable-record editors, with persistent appearance preferences, native font selection, debounced validation and compact operating controls.

**Architecture:** Extend the existing `clacks-desktop` presentation modules and runtime/session integration. Use egui's styling, theme, zoom, modal, input and font facilities; keep native validation and durable settings operations authoritative. The working operating/profile/equipment workflows consume every new presentation helper introduced here.

**Tech stack:** Rust 1.99.1; locked egui/eframe 0.37.0 with wgpu; Tokio; existing SQLite store; Serde and eframe persistence; fontique 0.11.1 already present transitively in the lockfile.

**Spec:** [Desktop UI and shared-component contract](../../desktop-ui.md), with [operating settings](../../operating-settings.md), [desktop foundation](../../desktop.md), and [observability](../../observability.md).

## Global constraints

- Keep libraries under `lib/<crate-name>` and applications under `bin/<application-name>`; keep `bin/clacks-log` thin. No new crate is needed for these presentation modules.
- Use Rust 1.99.1, workspace-inherited metadata/dependencies, the committed lockfile and Elastic License 2.0. Windows is the MVP desktop target; preserve cross-platform architecture.
- Project-owned tooling and non-trivial fixtures are Rust-only. No Python, xtask or generic maintenance dispatcher.
- No product code, scaffolding, dependency changes or database migrations in the planning commit. Create an isolated worktree at execution time.
- Preserve existing source loading/detachment, stale-save checks, reference repair, known-deletion cleanup and historical snapshots. UI changes must not reinterpret the domain model.
- Components do not perform store/network work in rendering callbacks. Reuse application-owned Tokio, channels, request responses and repaint requests.
- Keep focused modules; investigate any Rust file approaching 600 lines. No production `expect`, `unwrap`, unchecked collection indexing or test-only public APIs. No `mod.rs` outside tests.
- Use named fields for record-like data, owning-module imports and standalone rustdoc comments. Keep related code visually grouped.
- Use existing egui facilities, not a form engine, widget registry, event bus or telemetry abstraction. Instrument real runtime operations, not keystrokes or drawing helpers.
- Every implementation commit needs a meaningful subject and body explaining motivation/constraints. Commit concrete related changes; do not leave cleanup commits for mistakes introduced by the branch.
- Do not stage the unrelated existing change to `references/desktop-toolkit.md`.

## Delivery boundary and contract coverage

The specification deliberately distinguishes current production consumers from future capabilities. This execution delivers the **current desktop retrofit**, not the still-unimplemented tag/contact/history/provider workflows. Do not create dummy QSO panels, a separate UI demo application, unused contact widgets or a second draft model to appear to cover them.

| Contract requirement | Deliverable/owner |
| --- | --- |
| Defined sections, System/Light/Dark, unified scale and selectable radio font | Tasks 1–2; real application and current editors. |
| Edited/error/focus states, live validation and disabled invalid saves | Task 3; real operating/profile/equipment forms. |
| Operating dialog and independent reusable editors; dismissal/nesting | Task 4; production navigation and requests. |
| Compact summary, inline power and quick profile/equipment/operator changes | Tasks 5–6; real operating state and store commits. |
| Truthful local success/failure, retained edits and source/read status | Tasks 4–7; actual settings outcomes, not fabricated batch jobs. |
| Default workbench panes, resizable dividers, local dimensions/reset | Contact-entry/history integration under #9/#10, using these visual/preferences foundations. No empty panes in this delivery. |
| Shared in-progress QSO and additional QSO/Worked Before windows | #9/#10 integration; one production draft, egui viewports and existing application runtime. |
| Contact selection, logbook boundary, direct activation, configurable columns/sorting | #8–#10 integration; exact acceptance obligations below. No unused contact-table implementation in this branch. |
| Batch progress, cancellation and per-record reports | Real import/lookup/service capabilities; use current operation-feedback presentation and their existing domain policies. |

This is not completion evidence for #8–#10 or later service capabilities. Their plans must integrate this contract and extend the same production modules. The final coverage report must distinguish delivered retrofit behavior from these dependencies.

## Review focus

1. Input changed before a validation deadline or commit: the newest text must be checked; stale feedback must not enable a save or overwrite newer typing. Task 3.
2. Decimal watts `.5`, blank, overflow and excessive precision: preserve whole-milliwatt meaning and the previous committed value on rejection. Tasks 3/5.
3. A write succeeded but refreshing failed: show its acknowledged effect, mark the remaining view stale, and never retry the successful write automatically. Tasks 4/5.
4. A selected font disappears, a window shrinks or scale increases: retain usable controls and provide a working fallback without changing stored radio data. Tasks 1/2/7.
5. Escape/close during a nested or already-admitted operation: discard only unapplied UI edits, preserve completed saves and drain admitted work through the existing shutdown path. Tasks 4/6.

## Verified framework capabilities and engineering choices

These choices are based on the locked local sources, not an assumption about current online APIs:

- `egui::Context::set_theme` accepts `ThemePreference`; egui already follows native theme input for System. Install matching light/dark styles using framework styles; do not poll OS appearance yourself.
- `Context::set_zoom_factor` composes with native pixels-per-point. Do not multiply every font size/margin by a second scale or override OS display scaling.
- eframe's optional `persistence` feature supplies `CreationContext::storage`, `App::save`, `get_value`/`set_value` and native storage. Enable it. Set `NativeOptions::persistence_path` to `workspace/desktop-ui`; this scopes preferences to the local account workspace and prevents concurrent independent workspace processes from overwriting one file. All windows in that client share the same preferences.
- Persist one typed `AppearancePreferences` value at `clacks.appearance.v1`. Do not serialize `DesktopApp`, pending editors, errors, operating values, credentials or an entire UI session. Return `false` from `App::persist_egui_memory`; keep eframe's native-window persistence. Future pane/table state gets separate typed keys when its actual consumers arrive.
- The existing eframe `system_fonts` feature is fallback support, **not an installed-font picker**. Use fontique's actual `Collection::family_names`, `family_by_name`, `FamilyInfo::default_font`, `FontInfo::source/index` and `SourceCache::get` to enumerate/load a selected face off-thread. Register that face through `egui::FontData::from_blob` and the normal font definitions, retaining bundled fallback fonts.
- `egui::Modal` manages the modal layer. Its `should_close()` also treats a backdrop click as dismissal, which conflicts with the approved contract's default. Use its documented response fields and input consumption to accept explicit close/top-modal Escape while ignoring backdrop clicks; preserve the framework popup-before-modal Escape behavior.
- Initial preferences: System appearance, zoom `1.0`, bundled default radio monospace, ordinary UI typeface. Offer UI and radio-data font choices independently, including installed families; choosing a proportional radio face is allowed. Missing families retain the stored requested name, show a fallback notice and use the bundled role default until resolved or changed.
- Appearance editor uses Apply/Cancel with live visual preview; Cancel restores committed preferences. Apply changes only presentation and schedules framework persistence. Show invalid-scale errors and disable Apply. Offer 75%–200% in 5% steps initially, with a direct reset to 100%; font sizes and spacing remain logical points before framework zoom.
- Use the approved 300 ms debounce default. Validation is local/native-model work, not provider lookup; the separately agreed callsign-lookup debounce remains 500 ms.

## File and responsibility map

Existing UI owners remain `ui.rs`, `ui/operating.rs`, `ui/operating/{pending,responses}.rs`, `ui/profiles.rs` and `ui/equipment.rs`. Split coherent responsibilities as below, rather than introducing a generic rendering layer.

| New/changed file | Responsibility |
| --- | --- |
| `src/ui/preferences.rs` | Typed appearance preferences, framework storage key, validation and preference editor state. |
| `src/ui/theme.rs` | Role palette and egui light/dark styles; spacing/type tokens and style installation. |
| `src/ui/fonts.rs` | Native family enumeration/face loading and egui font installation; no custom fallback engine. |
| `src/ui/appearance.rs` | Actual Appearance dialog and its preview/Apply/Cancel behavior. |
| `src/ui/fields.rs` | Shared labeled text/unit/read-only/error/edited presentation, consumed by existing forms. |
| `src/ui/validation.rs` | Small per-editor debounce/deadline state; no form schema or parallel domain validator. |
| `src/ui/dialog.rs` | Shared modal title, action footer and dismissal/focus conventions. |
| `src/ui/feedback.rs` | Source/read notices and actual operation outcome presentation. |
| `src/ui/operating/summary.rs` | Real compact applied summary and its quick-control openers. |
| `src/ui/operating/quick.rs` | Quick editing requests and independent in-flight power/selection state. |
| `src/operating/power.rs` | Shared exact decimal-watts parsing/formatting used by full and inline editing. |
| Existing private test modules plus matching `ui/<module>/tests.rs` | Actual Clacks integration and targeted state/layout tests. |

All paths above are relative to `lib/clacks-desktop/`. Keep items crate-private or narrower; these are not a new public library API. Exact field layouts can follow idiomatic ownership, but preserve the interfaces and contracts below.

## Task 1: Apply the visual foundation and persistent appearance controls

**Files:** Modify root `Cargo.toml`, `Cargo.lock`, `lib/clacks-desktop/src/{application,ui}.rs`; create `ui/{preferences,theme,appearance}.rs` and focused private tests.

**Interfaces:**
- `AppearancePreferences { appearance: Appearance, zoom: f32, ui_font: FontChoice, radio_font: FontChoice }`, Serde/defaults; `Appearance::{System,Light,Dark}`; `FontChoice::{Default, Installed { family: String }}`.
- `AppearancePreferences::validate(&self) -> Result<(), Vec<FieldError>>` and `theme::install(context: &egui::Context, preferences: &AppearancePreferences)`.
- `AppearanceEditor::new(&AppearancePreferences)` owns pending/committed copies for this presentation dialog only; `appearance::show(...) -> Option<AppearanceAction>`, with named Apply/Cancel outcomes. Preserve egui response ownership rather than manufacturing a global action dispatcher.

- [ ] Write `appearance_apply_survives_app_save_and_reconstruction`, asserting that the real application save hook writes only committed preferences and a reconstructed app uses Light, `1.25` zoom and the chosen radio role; a cancelled preview restores System/`1.0`. Seed an unknown unrelated storage key and assert it remains untouched. Test Clacks hook/key/behavior, not a standalone Serde round trip.
- [ ] Add `appearance_preserves_native_scale_and_operating_values`: installing styles changes theme/zoom but leaves the real operating snapshot identical; each appearance has distinguishable text, selection and error roles. Add invalid/nonfinite/out-of-range scale cases and a missing/malformed saved-preference case that uses defaults with an actionable notice, without replacing account data.
- [ ] Run `cargo test -p clacks-desktop appearance_ --locked --offline`; confirm new expectations fail before implementation.
- [ ] Enable eframe persistence at workspace level. Use its native storage and save hook; create no parallel preference writer, migration-version registry or filesystem lock. Restore preferences during application creation and use the committed copy for saving. A missing config/storage uses defaults; a malformed preference value must not stop workspace startup.
- [ ] Add **Appearance…** to the real application. Use the existing GUI font/spacing hierarchy: 13 logical-point body/data, 12 labels/supporting text, 15 section/dialog title and 21 primary callsign entry when that consumer exists; 4/8/12/16 logical-point spacing steps, 30-point regular controls and 27-point compact rows. Treat these as centralized engineering defaults, not independent per-screen literals. Error, edited, focused and selected roles remain distinct.
- [ ] Run the targeted tests and `cargo clippy -p clacks-desktop --all-targets --locked --offline -- -D warnings`. Commit with motivation for shared styles and workspace-local framework persistence.

## Task 2: Provide a real installed-font picker with fallback

**Files:** Create `ui/fonts.rs`, `ui/fonts/tests.rs`; modify `ui/{appearance,preferences,theme}.rs`, `ui.rs`, `application.rs`, root/member manifests and lockfile.

**Interfaces:**
- `fonts::enumerate() -> anyhow::Result<Vec<String>>` returns installed family names; `fonts::load(family: &str) -> anyhow::Result<egui::FontData>` loads the family's default face and preserves its collection index.
- UI-owned `FontRequest { choice: FontChoice, reply: oneshot::Receiver<anyhow::Result<egui::FontData>> }` retains the requested choice beside its response. Send explicit font operations through the existing `SessionCommand` channel; the session tracks blocking jobs in a drained `JoinSet`, preserving shutdown admission and completion. No separate executor or generic job bus.

- [ ] Write `font_result_for_previous_choice_cannot_replace_current_preview`: choose family A, then B, complete A late; only B may install. Add `missing_selected_family_uses_default_without_erasing_choice` and `font_and_scale_changes_preserve_pending_operating_text` using real editor state.
- [ ] Add a rendering-boundary check with a small licensed, project-reproducible Rust test input or existing bundled font bytes: registering a chosen face changes the radio role used by an actual field while leaving its recorded text unchanged. Do not assert whatever fonts happen to be installed on CI or test fontique's discovery implementation.
- [ ] Run `cargo test -p clacks-desktop font_ --locked --offline` and confirm failures.
- [ ] Add workspace `fontique = "=0.11.1"` and member inheritance. Enumerate/load outside drawing; poll owned responses and request repaint. Keep the existing system fallback provider enabled. Install selected family data into named egui families while retaining bundled/glyph fallbacks; do not reinterpret `FontFamily::Name` as automatic OS-family loading.
- [ ] Populate actual UI/radio font selectors and preview using known available families. Display loading/failure/missing-selection state without replacing the pending choice. No bundled third-party font downloads or new font fixtures without reproducible provenance and notices.
- [ ] Run targeted tests and Clippy. Commit the working preference workflow, explaining why system-font fallback alone did not implement selection.

## Task 3: Integrate debounced validation and visible unapplied state

**Files:** Create `operating/power.rs`, `ui/{fields,validation}.rs` and tests; modify `operating/{editor,station,equipment}.rs`, `operating.rs`, `ui/operating/{fields,pending}.rs`, `ui/{profiles,equipment}.rs`.

**Interfaces:**
- `power::parse_watts(text: &str) -> Result<Option<Milliwatts>, String>` and `power::format_watts(value: Option<Milliwatts>) -> String`; full operating build and inline power share them.
- `ValidationState::edited(&mut self, now: Instant)`, `due(&self, now: Instant) -> bool`, `validated(&mut self, errors: Vec<FieldError>)`; state retains whether validation belongs to the current edit. A ready action has current successful validation and no in-flight request.
- `fields::text(ui: &mut egui::Ui, id: egui::Id, label: &str, value: &mut String, presentation: FieldPresentation<'_>) -> egui::Response`; `FieldPresentation` has named `edited`, `read_only` and matching `errors` fields. Unit/help presentation stays with its field. No fluent field-schema DSL.

- [ ] Add exact assertions: `parse_watts(".5").unwrap().unwrap().get() == 500`, `"0.001" == 1`, `"  " == None`; reject `"0"`, `"-1"`, `".0001"` and overflow without modifying the prior operating state. Keep full seconds/whole-hertz and other native rules unchanged.

  Representative test in `operating/power/tests.rs` (test-only unwraps):

  ```rust
  #[test]
  fn power_entry_accepts_leading_decimal_and_preserves_milliwatts() {
      let half_watt = parse_watts(".5").unwrap().unwrap();
      let one_milliwatt = parse_watts("0.001").unwrap().unwrap();

      assert_eq!(half_watt.get(), 500);
      assert_eq!(one_milliwatt.get(), 1);
      assert_eq!(parse_watts("  ").unwrap(), None);
      assert!(parse_watts("0").is_err());
      assert!(parse_watts(".0001").is_err());
      assert!(parse_watts("9223372036854776").is_err());
  }
  ```
- [ ] Add `validation_waits_300ms_then_checks_latest_form`: at 299 ms the new edit is pending; at 300 ms validation uses the latest text. A second edit restarts the deadline. Assert no settings request was sent by validation.
- [ ] Add real rendered-form tests: invalid power disables operating Apply with field and summary messages; correcting to `.5` enables it after validation without saving; profile Save remains independently valid while unrelated pending power is invalid. Cover both invalid-first and valid-then-invalid edits before a deadline.
- [ ] Add `edited_invalid_and_focused_states_coexist` and `semantic_normalization_does_not_create_false_edit_markers`. Compare against existing expected operating/shared-record values, not a second copy of a full domain model. New populated unsaved records are still unsaved; selected-source baselines are not dirty-marker baselines.
- [ ] Run `cargo test -p clacks-desktop validation_ --locked --offline` and `cargo test -p clacks-desktop power_ --locked --offline`; confirm new tests fail.
- [ ] Move exact power parsing out of `editor.rs` and accept a leading decimal point. Keep checked integer arithmetic, milliwatt precision and signed-store range. Reuse each editor's existing `build()` validation; avoid per-keystroke domain writes or duplicate validators.
- [ ] Apply the shared field presentation to all current station/location/reference, equipment, operator, power and profile-name fields. Invalidate cached validation on edits, definition changes and source loads. Use `request_repaint_after` for the nearest validation deadline, not a spinning frame loop or background network task. Commit paths rebuild the latest values even if called before the debounce completes.
- [ ] Disable affected Save/Apply while validation is pending/invalid or a request is in flight. Keep errors useful across tabs and provide a summary naming the relevant fields. Preserve source-detachment semantics; invalid text and its amber marker remain visible. Keep explicit read-only comparison values legible.
- [ ] Run targeted tests plus existing `operating`/UI suites and Clippy. Commit the shared field behavior and parser correction together with their actual consumers.

## Task 4: Move current editing workflows into consistent dialogs

**Files:** Create `ui/{dialog,feedback}.rs` and focused tests; modify `ui.rs`, `ui/{operating,profiles,equipment}.rs`, `ui/operating/{pending,responses}.rs`, existing refresh/stale tests.

**Interfaces:**
- `dialog::show<R>(context: &egui::Context, id: egui::Id, title: &str, body: impl FnOnce(&mut egui::Ui) -> R) -> DialogResponse<R>`; named response fields carry `inner` and `cancel_requested`. Use egui's modal response and focus facilities, with a narrowly owned opener ID.
- Keep existing `Screen`, `Frame`, `Navigation` and `SettingsAction/Response` domain boundaries. Distinguish persistent failure/read status from validation errors and normal success in `feedback` presentation; do not replace typed responses with strings.

- [ ] Write `escape_cancels_only_top_editor_and_restores_opener`, `backdrop_does_not_discard_editor`, and `menu_escape_does_not_cancel_outer_editor`, exercising actual egui input and production navigation. No custom modal stack or keyboard focus engine.
- [ ] Extend real-store workflows: inner equipment Save survives outer profile Cancel; changing operating views retains one pending context; X/Escape discards that form without a second prompt. Cancel never undoes known-deletion cleanup.
- [ ] Add `dismiss_after_request_admission_does_not_lose_completion`: a committed save is still accounted for if its originating dialog is gone; reopening reads its real durable result. Preserve pending operations separately from visible frames until they settle—do not drop a response receiver merely by popping a dialog. No retry on ambiguous acknowledgment loss.
- [ ] Add `committed_apply_refresh_failure_shows_acknowledged_state_once`: after the store accepted an Apply but the following read failed, show the acknowledged operating state and a stale-view warning; reopening must not apply the same edit again. Retain exact old form/current basis on `Stale` or failure. Use existing returned `OperatingState` and request ownership, not new generation counters.
- [ ] Run `cargo test -p clacks-desktop ui:: --locked --offline`; establish failing cases before modifying navigation.
- [ ] Render real operating/profile/equipment editors with the shared dialog header/footer. Use one operating Apply/Cancel across tabs and separate reusable-record Save/Cancel. Preserve unfinished callers and the explicit equipment-default association when entering/leaving inner editors.
- [ ] Do not dismiss by blindly calling `ModalResponse::should_close()`. Accept close/Escape only for the top editor after popups handle Escape; ignore backdrop clicks. While a write is already admitted, dismissal only hides/discards remaining UI edits: completion still flows through an owned response and real committed view. Application close still drains work through existing shutdown.
- [ ] Keep failed forms open when visible. Preserve acknowledgment outcomes even on failed refresh; current summary/next editor use the acknowledged operating basis until a subsequent fresh read supersedes it. Retain deletion knowledge and exclude deleted choices on failed refresh exactly as today. Give stale review explicit context; do not silently rebase or reload newer saved values.
- [ ] Run UI/store regression suites and Clippy. Commit the dialog conversion without domain/persistence rewrites.

## Task 5: Deliver the compact summary and exact inline power editing

**Files:** Create `ui/operating/{summary,quick}.rs` and focused tests; modify `ui/operating.rs`, `ui.rs`, `operating/power.rs`, private rendered-workflow tests.

**Interfaces:**
- `summary::show(ui: &mut egui::Ui, active: &OperatingState, quick: &mut QuickOperatingUi) -> SummaryAction` exposes named action variants for opening real editors/quick controls; its power presentation uses `power` parsing/formatting.
- `QuickOperatingUi` owns its power text, original committed value, current validation and optional in-flight `SettingsRequest` response. Keep it separate from the full pending `OperatingEditor`.
- Quick commits use existing `SettingsAction::Apply { expected, next }`; build `next` from the latest active state and replace only the intended power field. Retain copied station/equipment/operator values and selections.

- [ ] Write `inline_power_point_five_commits_500_milliwatts_and_reopens`, using production UI requests and a real store; Enter, Tab and ordinary focus departure commit valid input, while Escape restores the previous value without sending an Apply.
- [ ] Write `inline_power_invalid_or_failed_commit_preserves_previous_active_value`; zero/invalid/overflow input stays editable with local feedback, and failures do not claim success. Blank clears only optional power. Repeated Enter/blur for one admitted edit must not submit duplicate writes.
- [ ] Add `power_reply_cannot_replace_later_typing` and `inline_power_change_makes_open_full_editor_stale`. Retain submitted text beside its response and preserve later text on completion; use existing stale-save protection for the other editor, not an automatic merge.
- [ ] Add `quick_controls_without_active_station_do_not_create_incomplete_context`: before initial station setup, operator/power/equipment quick commits are unavailable with an **Operating settings…** route and reason. Loading a valid saved station profile can establish a complete context. Never persist an incomplete `CurrentOperatingValues` merely to save a scalar preference.
- [ ] Run `cargo test -p clacks-desktop inline_power --locked --offline` and confirm new failures.
- [ ] Replace the long applied-settings dump with a compact real summary: station/callsign/location, selected equipment with useful context, operator and power. Keep full optional facts in the dialog. Display changed-source/override/read status without confusing it with an unapplied-value marker. Put **Operating settings…** beside its controls.
- [ ] Use an actual inline `TextEdit` for power with a visible watts unit. Integrate live validation; explicit commit validates immediately. Defer blur commit until input handling knows whether Escape cancelled it. Preserve last committed value until acknowledgment, and preserve typed text on store error or stale response. Do not require a popup or change unrelated settings.
- [ ] Run the targeted tests, all current operating workflows and Clippy. Commit the real summary and power workflow, with no QSO/history placeholders below it.

## Task 6: Deliver quick source/operator controls without blurring ownership

**Files:** Extend `ui/operating/{summary,quick}.rs`; modify `ui/operating.rs` and current shared source selectors; add private UI/store integration coverage.

**Interfaces:**
- `QuickEdit::{Profile, Equipment, Operator}` carries a private pending `OperatingEditor`, source-read basis and owned response. Profile/equipment selection uses current typed account IDs and existing `stage_profile`/`stage_equipment` methods.
- Quick Apply uses the same validated `SettingsAction::Apply` as Task 5; full-editor state remains independent. Do not save reusable records from quick controls.

- [ ] Add real workflow tests: profile with default switches station and equipment but preserves operator/power; profile without default preserves equipment and its selection; equipment-only selection preserves the selected station profile and marks the override accurately.
- [ ] Add operator tests for independently optional callsign/name and normalized callsigns; operator changes detach neither source. Quick Cancel sends no write and preserves active state.
- [ ] Add `deleted_or_unavailable_default_does_not_partially_apply_quick_profile`, `quick_source_change_does_not_advance_captured_baseline`, and `quick_apply_racing_full_editor_retains_stale_review`. A failed read does not imply deletion; a known deletion does clear the affected selection even when the following refresh fails.
- [ ] Run `cargo test -p clacks-desktop quick_ --locked --offline`; confirm new cases fail.
- [ ] Add source/operator quick editors anchored to the summary with explicit Apply/Cancel. Preview the chosen profile's equipment consequence before applying. Display duplicate names with callsign/location or equipment context. Keep source-changed, no-source and missing-reference states truthful and visible.
- [ ] Reuse full-editor validation, loading and stale-save semantics; preserve independently completed reusable saves. Keep a request's result owned if its quick editor is dismissed, and never turn a source picker into a saved-record editor.
- [ ] Run full desktop tests and Clippy. Commit the complete quick-control workflows.

## Task 7: Verify production integration and hand off remaining capabilities accurately

**Files:** Modify `lib/clacks-desktop/README.md`, `docs/desktop-ui.md`, `docs/mvp-roadmap.md`, relevant UI tests and existing diagnostics only where needed. Do not add an implementation-review report or duplicate backlog.

- [ ] Add focused appearance/field-layout tests across Light/Dark, 100%/125%/150%/200%, and representative 680×440, 1024×768 and 1440×900 logical windows. Assert the Clacks-required values/errors/actions remain reachable without clipped editable frequency/time/power text; scrolling/reflow is acceptable. Avoid pixel-perfect tests of egui painting internals.
- [ ] Verify keyboard traversal and stable field IDs across tab changes, source refresh and validation. Test only actual current controls; contact-table keyboard/performance tests arrive with their real consumer.
- [ ] Preserve existing settings-operation tracing/events/metrics. Add `#[instrument(skip_all, ...)]` only to meaningful new font resolution/application boundaries, using default module targets and direct fields. A selected-font failure is handled once with UI feedback and a bounded diagnostic category; never emit font file paths, form contents or a timer/completion framework. No metric is required for style installation or every preference edit. Keep committed settings metrics unchanged.
- [ ] Run the complete verification commands below once after changes pass their targeted checks. Add regressions for new failures, rather than repeatedly broadening tests without cause.
- [ ] Update the desktop guide with Appearance controls, workspace-local preference location, native font fallback, full dialog navigation, power commit/cancel and real failure behavior. Update the roadmap at milestone level to reference the UI contract, not list functions/tests/tasks.
- [ ] Record a delivery matrix in the PR description: current retrofit delivered versus #8–#10/later workflow dependencies. Do not describe the full future workbench as implemented. Keep future component obligations below with the owning contract, not as new fake production types.
- [ ] Use the production Windows executable for human smoke checks: Appearance Apply/Cancel and normal reopen persistence, OS appearance change while System is selected, missing-font recovery, scaling/window resizing, nested Save/Cancel, power Enter/Tab/Escape, quick source/operator Apply/Cancel, and graceful close during a request. No ignored manual cargo-test harness or shipped delay flag. The user has a Windows computer but cannot provide a remote session: supply production-executable instructions and collect their observations, rather than requesting a session again. CI is not graphics/input evidence.
- [ ] Commit the integration documentation and any focused verification changes with a substantive message. Do not push, create a PR or claim native Windows acceptance without the relevant authorization/evidence.

## Downstream integration obligations—not unused code to build now

The contract's remaining capabilities require their owning specs/plans. Carry these concrete requirements into those plans; do not weaken or mark them complete in this branch:

- **Tags/current logbook (#8):** current-logbook selection remains client-local, supplies the new-contact default tag, and clears contact selection when switched. Cross-logbook selection uses an unscoped view. Implement real tag deletion choices/counts and confirmations through the shared dialog styling, preserving the existing domain operations.
- **Contact entry/editor (#9):** one nonpersistent client draft feeds entry in the workbench and additional entry windows. Primary/receive quantities, time locks, reference entry and validation use native contracts. Full editing is a dialog; successful saves alone reset times under the existing rules. Apply the shared radio typography and edited/error/action states to real fields.
- **History/Worked Before (#10):** query actual local working contacts, exclude pending deletions, show pending/conflict indicators and one row per contact. Use egui_extras' production table facilities with stable QSO IDs and real large-log queries. Independent local keys `history.v1`/`worked-before.v1` own visible/ordered columns, logical widths and sort; resets affect only their table. Sorting/filtering preserve selected IDs and expose hidden counts/Show selected. Double-click/Enter activates the focused QSO editor; table shortcuts do not consume input from field controls. No unrestricted bulk fact editor.
- **Workbench integration (#9/#10):** QSO entry and Worked Before above full-width history, plus the real compact summary. Use egui panels/resizing facilities for dividers; a local `workbench.v1` preference stores restored proportions bounded to the current usable window. Reset affects layout only. Additional egui viewports reuse the same draft/context and do not remove panels from the main workbench.
- **Lookup/import/provider workflows:** show actual operation counts and record-specific reports; cancellation/partial completion obey their specifications. Source snapshots stay separate from recorded facts. Extend the existing feedback presentation only when real operation data is available.

## Verification commands

Use the repository toolchain and lockfile. Dependency fetch during authorized execution may be needed before offline checks.

```sh
cargo fmt --all --check
cargo test -p clacks-desktop --all-targets --locked --offline
cargo test --workspace --all-targets --locked --offline
cargo test --workspace --doc --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --document-private-items --locked --offline
cargo run --bin clacks-log --locked --offline -- --help
git diff --check
```

No database migration/query change is expected: current operating writes use existing store operations and appearance uses framework-local persistence. If execution demonstrates a necessary store change, update the concrete plan first, preserve separate SQL query files and committed metadata, and run the existing SQLx CI verification. Do not introduce a preference database just to avoid using framework storage.

Linux and Windows CI must pass the existing Rust workflow after an authorized push/PR. Keep its current trigger boundaries and `rust-toolchain.toml` as the version source. Cross-target compilation supplements, but does not replace, native Windows human verification. Report any unavailable native check explicitly.

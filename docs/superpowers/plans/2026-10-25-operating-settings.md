# Operating Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for native execution, or superpowers:subagent-driven-development if the user chooses that approach. Implement task by task; checkboxes track execution, not product decisions. Do not start implementation until the user authorizes execution.

**Goal:** Deliver #7's offline operating-settings, reusable station-profile and equipment workflows in the production desktop, with independent source selections and correct historical copies.

**Architecture:** Extend the existing model, SQLite store and egui desktop. Persist independent optional source baselines, stage operating edits in window-owned forms, and use the existing Tokio runtime for guarded writes and snapshot refreshes. Route deletion cleanup through the common client-record persistence boundary so local and received effective deletions obey the same rule.

**Tech Stack:** Rust 1.99.1, egui/eframe 0.37.0 with wgpu, Tokio, SQLx 0.9.0/SQLite, existing tracing and metrics composition.

**Spec:** [Consolidated operating-settings specification](../../operating-settings.md), particularly its action boundaries, D285 detachment, D287 deletion cleanup and D183 missing-reference handling.

## Global constraints

- Use `rust-toolchain.toml` as the Rust version source; preserve Rust 1.99.1 and workspace-inherited dependencies/package metadata.
- Extend `lib/clacks-model`, `lib/clacks-client-store`, `lib/clacks-desktop` and the existing thin `bin/clacks-log`; deliver production components, not replaceable environments.
- Windows is the MVP target; keep the architecture cross-platform. No new external storage service, renderer, runtime or dependency is needed for this plan.
- Keep SQLite fully relational, SQLx fixed queries in `.sql` files, committed `.sqlx` metadata and forward-only startup migrations. Never edit migrations 0001–0008.
- Retain ordinary immediate write transactions and snapshot reads. No custom write queue, new generation counters, schema markers or generic multi-record framework.
- Keep station, equipment, operator and power scopes distinct; one Apply/Cancel pair governs operating edits. Separate reusable saves survive outer Cancel and never implicitly select/apply.
- Manual station/equipment changes detach only their own selection/baseline. Known deletions clear matching selections while preserving copied values. Missing data alone is not deletion.
- Do not implement contact entry/history, tags, live synchronization/enrollment, global conflict review, geographic autofill, inventory or contest exchange assistance here.
- Use Rust-only project tooling/fixtures; no Python, xtask, compiled TrustedQSL or test harness shipped as application behavior.
- Follow AGENTS.md for rustdoc, named fields, no production panic/indexing, module names without `mod.rs` outside tests, focused files and meaningful commit bodies.
- Instrument domain work with existing framework facilities and the observability policy; do not add telemetry-only wrappers or duplicate span lifecycle machinery.
- Preserve the existing unrelated `references/desktop-toolkit.md` changes. Planning/specification changes already in the working tree are input to this work, not disposable changes.
- This plan authorizes neither push/PR creation nor claims of Windows verification; obtain the user's execution/publication instruction and record actual evidence.

## Review focus

Each condition below has concrete coverage in its owning task:

1. A partially typed coordinate or callsign must not lose text, falsely retain a selected source, or turn repaint/normalization into a new edit — Task 4.
2. A source deleted while Apply is waiting must not be reattached; a genuinely intervening operating edit must still produce Stale — Tasks 2–3 and 5.
3. A new referring profile created after an equipment-deletion preview must abort the complete deletion/repair operation — Task 3.
4. A saved equipment default disappearing during Save Profile must leave a pending missing-reference edit, not lose the profile or bypass its own stale-save guard — Tasks 2 and 6.
5. A migration must compare station values rather than snapshot row IDs and must not fabricate an equipment selection from matching descriptions — Task 1.

## File structure and interfaces

Paths below are relative to the repository root. New Rust files use named module files; tests can use their established support modules. Split further by coherent responsibility if a file approaches 600 lines; do not reorganize unrelated code.

| Owning files | Responsibility |
| --- | --- |
| `lib/clacks-model/src/profile.rs`, `lib/clacks-model/tests/defaults_and_tags.rs` | Keep native reusable values/validation; remove obsolete combined-baseline APIs and copy wrappers whose production caller is replaced by staged editors. |
| `lib/clacks-client-store/src/operating.rs`, new `operating/{values,selections}.rs`, `rows/operating.rs` | Independent state, relational hydration and guarded Apply/known-deletion normalization. |
| New `migrations/0009_operating_selections.sql`, `sql/operating/{state_select,state_store,forget_source}.sql` | Forward-only metadata migration and selection references. |
| New `lib/clacks-client-store/src/settings.rs`, `settings/{read,save,deletion}.rs` | Production settings snapshot, reusable save feedback and reviewed equipment deletion. |
| New `sql/settings/{records,referring_profiles}.sql` | Account-scoped settings keys and complete live referring-profile set. |
| `metadata/client.rs`, `save.rs`, `read.rs`, `retention.rs`, retention SQL | Common effective-deletion cleanup, public-save routing, snapshot access and retained selection roots. |
| New `lib/clacks-desktop/src/operating.rs`, `operating/{station,position,equipment,editor}.rs` | Window-owned editable forms, native conversion, independent detachment and caller-preserving navigation. |
| New `lib/clacks-desktop/src/session/settings.rs`; `session.rs`, `application.rs`, `workspace.rs` | Domain requests/results alongside the existing lifecycle channel and runtime. |
| New `lib/clacks-desktop/src/ui/{operating,profiles,equipment}.rs`; `ui.rs`, `ui/startup.rs` | Actual egui views and active summary, using the same forms and store-backed results. |
| Existing/new store integration tests and desktop private tests | Durable behavior, native UI actions, shutdown and instrumentation evidence. |
| Crate READMEs, `docs/operating-settings.md`, related owning docs | Actual APIs, commands, validation and delivery limits. |

### Store API contract used by later tasks

Define these items in their owning modules; do not facade-re-export them:

```rust
// clacks_client_store::operating
pub struct OperatingState {
    pub current: Option<CurrentOperatingValues>,
    pub selected_profile: Option<SavedLocationProfile>,
    pub selected_equipment: Option<EquipmentConfiguration>,
}
// Retain Saved(OperatingState) and Stale(OperatingState); add Unchanged(OperatingState).
pub async fn load(store: &ClientStore) -> anyhow::Result<OperatingState>;
pub async fn save(store: &ClientStore, expected: &OperatingState,
                  next: &OperatingState) -> anyhow::Result<OperatingSaveOutcome>;

// clacks_client_store::settings
pub struct SettingsSnapshot {
    pub operating: OperatingState,
    pub profiles: Vec<ClientRecord>,
    pub equipment: Vec<ClientRecord>,
    pub groups: Vec<ClientRecord>,
}
pub struct ProfileSaveResult {
    pub outcome: SaveOutcome,
    pub missing_equipment: Option<EquipmentConfigurationId>,
}
pub struct EquipmentDeletionPreview {
    pub equipment: ClientRecord,
    pub referring_profiles: Vec<ClientRecord>,
    pub replacement_equipment: Vec<ClientRecord>,
}
pub struct ProfileEquipmentReplacement {
    pub profile: ProfileId,
    pub equipment: Option<EquipmentConfigurationId>,
}
pub enum EquipmentDeleteOutcome {
    Applied,
    Stale(EquipmentDeletionPreview),
    Unavailable { current: Option<ClientRecord> },
}
pub async fn snapshot(store: &ClientStore) -> anyhow::Result<SettingsSnapshot>;
pub async fn save_profile(store: &ClientStore, expected: Option<&ClientRecord>,
    next: &SavedLocationProfile) -> anyhow::Result<ProfileSaveResult>;
pub async fn save_equipment(store: &ClientStore, expected: Option<&ClientRecord>,
    next: &EquipmentConfiguration) -> anyhow::Result<SaveOutcome>;
pub async fn delete_profile(store: &ClientStore, expected: &ClientRecord)
    -> anyhow::Result<SaveOutcome>;
pub async fn preview_equipment_deletion(store: &ClientStore, id: EquipmentConfigurationId)
    -> anyhow::Result<EquipmentDeletionPreview>;
pub async fn delete_equipment(store: &ClientStore, expected: &EquipmentDeletionPreview,
    replacements: &[ProfileEquipmentReplacement]) -> anyhow::Result<EquipmentDeleteOutcome>;
```

`ClientRecord` is the existing account-qualified saved/accepted/pending record, not a new serialized representation. Preserve Deleted/unavailable records in snapshot results so absence is not mislabeled as deletion; render only live working records as selectable choices. Preview reads also capture all available live replacement configurations except the target, for explicit review; final deletion revalidates only those chosen. Refresh the preview before offering newly created replacements. `Stale` refreshes the review; `Unavailable` carries the actual current record, if any, without fabricating a live preview or treating absence as deletion. A missing-equipment issue accompanies only a Saved/Unchanged live profile outcome. Public functions check store account ownership; the store derives the account for ID-only calls.

## Task 1: Persist independent selections and migrate existing workspaces

**Files:** Modify model `src/profile.rs`, model tests `defaults_and_tags.rs`; store `src/operating.rs`, `src/rows/operating.rs`, `src/read.rs`; create store `src/operating/{values,selections}.rs`, migration 0009; modify operating/retention queries and store `tests/operating.rs`, `src/migration_tests.rs`. Update `.sqlx/` and affected model/store READMEs.

**Interfaces:** Produce the `OperatingState`, `OperatingSaveOutcome`, `load` and `save` interfaces above. Keep native `load_profile`/`load_equipment` temporarily until Task 4 ports their useful tests to actual staged workflows and removes those unused wrappers; remove model `LoadedProfileBaseline`, `ProfileDifference`, `compare_loaded_profile` now and update all callers/tests. Remove the store's immediate-writing `operating::load_profile`; staging belongs to the read/form flow, not a second Apply path.

- [x] **1. Write failing persistence and migration cases.** Add named tests:
  - `independent_sources_survive_reopen`: save Home plus selected Rig B, close/reopen; assert both source IDs/baseline values and operator/power exactly match, while Home's default is still Rig A.
  - `apply_detaches_only_the_edited_component`: next station changes and `selected_profile = None`; assert equipment selection survives; repeat for equipment.
  - `old_baseline_migrates_by_station_values`: construct a real version-8 database using the committed first eight migrations and Rust test support. Equal station values in different snapshot rows retain the profile; altered grid/reference/geographic values drop it. Operator/power/equipment-only changes do not drop an unchanged station selection.
  - `legacy_equipment_selection_is_not_guessed`: matching descriptions and Home's default Rig A still produce `selected_equipment == None` after upgrade.
  - `selection_roots_survive_collection`: collect unreferenced payloads, reopen and assert both selected baselines remain; detach one and assert unrelated live/pending/contact roots remain intact.

  Pin the reopened values, not just successful deserialization:
  ```rust
  assert_eq!(reopened.selected_profile.as_ref().map(|p| p.id), Some(home.id));
  assert_eq!(reopened.selected_equipment.as_ref().map(|e| e.id), Some(rig_b.id));
  assert_eq!(reopened.current, Some(applied.clone()));
  ```
- [x] **2. Verify the new cases fail against the old API/schema.** Run `SQLX_OFFLINE=true cargo test -p clacks-client-store --test operating --locked --offline` and the targeted migration tests. Missing-field/variant compile failures are an expected first red result; do not weaken cases to fit the old API.
- [x] **3. Implement the state and forward migration.** Rebuild `operating_state` in 0009 with existing `current_values`, `loaded_profile`/`profile_record`, and new `loaded_equipment`/`equipment_record` columns. Each optional pair is all-null or complete; selections require initialized current values. Use kind-checked, account-qualified foreign keys to the existing immutable `payload` roots. Remove `loaded_values`; retain `profile_kind` and add fixed `equipment_kind` checks.
  - Migration compares current station to the old `loaded_values` station semantically: callsign, location presence and every location column using null-safe equality, and both directions of reference-set difference using `EXCEPT`. Do not compare station row IDs. Exclude known working tombstones from retained profile selection. Start equipment selection null; never reload current records.
  - SQLx alone owns migration/history atomicity; no post-startup migration marker or Rust side migration. Preserve current value IDs and all underlying facts.
  - Hydrate independent native baseline copies using existing payload functions. Validate that selections imply current values and match their own copied station/equipment facts; operator/power and equipment override do not invalidate the station selection.
  - Move value hydration to `operating/values.rs`, selection handling to `operating/selections.rs`; keep transactional orchestration in `operating.rs`.
  - Retention protects both baseline payload roots. `operating_garbage.sql` no longer references `loaded_values`; other station/equipment garbage rules still protect live operating/contact/payload owners.
- [x] **4. Regenerate SQLx metadata and verify.** Use the query-check procedure below, then `cargo test -p clacks-model --test defaults_and_tags --locked --offline` and the store tests above. Update old tests asserting migration version 8 to the new real version 9, retaining their semantic checks.
- [x] **5. Commit the deliverable.** Suggested subject: `Persist independent operating source selections`. Body explains why coupled comparisons no longer fit D285, conservative legacy equipment treatment, and migration/reopen evidence. Stage explicit files only.

## Task 2: Read settings consistently and apply/save without conflating effects

**Files:** Create store `src/settings.rs`, `settings/{read,save}.rs`, `sql/settings/records.sql`; extend `read.rs`, `lib.rs`, `operating/selections.rs`, `operating.rs`; modify `save.rs` as needed for private transaction reuse. Add `tests/settings.rs`, extend `tests/operating.rs` and `.sqlx/`.

**Interfaces:** Consume Task 1. Produce `SettingsSnapshot`, `snapshot`, reusable save APIs and `ProfileSaveResult` above. Keep `save::save_in` crate-private and transaction-composable. Add `ReadSnapshot::settings(&mut self) -> anyhow::Result<SettingsSnapshot>`; `settings::snapshot` opens/closes that ordinary read snapshot.

- [x] **1. Write failing behavior cases.** Test `settings_snapshot_is_one_account_and_one_read_basis`, `profile_save_with_deleted_default_is_pending`, `profile_save_preserves_its_stale_guard`, `apply_keeps_loaded_values_after_saved_source_update`, and `unknown_source_is_not_deleted`.
  - Persist Home/default A, selected B and account group definitions; assert settings reads return their actual working records, retained tombstones and exact active values. Cross-account expected records must fail with no writes.
  - Read a new profile editor basis, delete A, then save that otherwise valid profile referencing A: assert `Saved`, `missing_equipment == Some(A)`, actual pending working profile still references A, and operating values do not change. Do not recreate A.
  - Change the profile itself before saving: assert `Stale`, unchanged saved state and no fabricated missing-reference success.
  - Change saved B's antenna from Vertical to End-fed after staging: Apply retains Vertical and the old B baseline.
  - An unknown source/failed read leaves source selection intact; validation failure writes neither current values nor selections.

  The deleted-default case must assert both the outcome and persisted intent:
  ```rust
  assert!(matches!(result.outcome, SaveOutcome::Saved(_)));
  assert_eq!(result.missing_equipment, Some(rig_a.id));
  assert_eq!(persisted_profile.default_equipment, Some(rig_a.id));
  assert_eq!(after.operating, before.operating);
  ```
- [x] **2. Run the named cases red.** `SQLX_OFFLINE=true cargo test -p clacks-client-store --test settings --locked --offline` and `--test operating`.
- [x] **3. Implement consistent reads and guarded saves.** Query account-qualified `client_record` keys for profile/equipment/group kinds and hydrate through the existing client loader inside one `ReadSnapshot`. Include working deletions and records without a usable working value for availability classification. Do not invent a serialized settings cache or public generic repository.
  - Profile/equipment saves use native validation plus normal pending/original/prepared roles; require nonblank display names at these acceptance boundaries. Return native saved rows and distinguish Unchanged/Stale.
  - Check a profile's equipment availability in the same write transaction. A known tombstone leaves the profile pending with the reported issue (D183); missing/unavailable is not silently classified Deleted. Keep the actual ID for explicit repair.
  - In operating Apply, read the working status of relevant IDs from expected/current/proposed selections. Normalize only explicitly known Deleted IDs out of each state's selections, never their values. Compare normalized expected with normalized current; a remaining difference returns Stale. Normalize proposed selections before saving. This admits only proven deletion cleanup, not an unrelated intervening value/selection edit.
  - After the normalized-basis guard, compare the cleaned desired state to the actual stored current state to decide Saved versus Unchanged. Never skip a needed metadata cleanup merely because two temporary normalized copies compare equal. An unrelated stale result reports actual current state without silently committing the caller's value edit. Shared-record name/value changes alone do not reject Apply or replace baselines.
- [x] **4. Instrument committed effects at their owning boundaries.** Keep default tracing targets and framework span lifecycle. Use separate `clacks_client_profile_mutations_total` and `clacks_client_equipment_mutations_total`, bounded `change=create|update|delete`, alongside the existing QSO metric. The public function owning the commit emits that mutation once: generic `save::save` and the settings-specific transaction paths each account for their own commits; private `save_in` emits no committed counter/event. Operating saves increment `clacks_client_operating_applications_total` only for changed committed contexts. Add a deliberate `info` apply event; retain explicit staging versus committed wording. Do not introduce a telemetry-only completion wrapper to share these few statements.
- [x] **5. Regenerate query metadata and run tests green.** Run Task 2 tests, `cargo clippy -p clacks-client-store --all-targets --locked --offline -- -D warnings` with `SQLX_OFFLINE=true`, and existing `tests/local_saves.rs` regression coverage.
- [x] **6. Commit.** Suggested subject: `Add production settings reads and independent save boundaries`. Body explains snapshot consistency, pending missing-reference feedback and why source freshness does not gate local Apply.

## Task 3: Delete reusable sources safely across local and received changes

**Files:** Create store `settings/deletion.rs`, `sql/settings/referring_profiles.sql`, `sql/operating/forget_source.sql`, `tests/settings_deletion.rs`; extend `metadata/client.rs`, `operating/selections.rs`, `settings.rs`, `save.rs`; extend `tests/transfers.rs` or its scenario modules and `tests/support/` as needed. Update `.sqlx/`.

**Interfaces:** Produce `EquipmentDeletionPreview`, `ProfileEquipmentReplacement`, `EquipmentDeleteOutcome`, preview/delete functions and `delete_profile` above. Add crate-private `operating::selections::forget_source_in(connection: &mut SqliteConnection, key: RecordKey) -> anyhow::Result<()>`; it removes only matching selection columns, preserving `current_values` and the other selection.

- [x] **1. Write failing transaction cases.** Add:
  - `delete_source_clears_only_matching_metadata`: deleting Home retains exact current facts/selected B; deleting B retains station/profile; close/reopen to prove the result.
  - `effective_received_deletion_forgets_source`: use real core envelopes/transfer installation and the production store. An effective working tombstone detaches; an accepted deletion with a retained live pending working value follows the core's actual working-state result instead of inventing a second conflict rule.
  - `equipment_delete_repairs_all_reviewed_profiles_atomically`: Home/A and Field/A become Home/B and Field/None with A Deleted; exact existing QSO/current equipment facts remain. Observe no intermediate committed state.
  - `new_referrer_makes_preview_stale`: create a third A-referring profile after preview; assert Stale and all original profile/equipment values unchanged.
  - `replacement_edit_or_deletion_aborts_review`: replacement B changes or is deleted after review; no partial repairs/deletion commit.
  - `failed_equipment_delete_rolls_back_repairs_and_cleanup`: Rust test-only database failure injection after a repair; reopen and assert original profiles, A and selections all remain. No public failure controls.
  - `apply_racing_delete_never_reattaches`: stale expected source metadata is removed only on actual tombstone proof; a separate intervening power/grid edit still returns Stale.

  For the new-referrer case, preserve all three records and operating state:
  ```rust
  assert!(matches!(outcome, EquipmentDeleteOutcome::Stale(_)));
  assert_eq!(after.profiles, before_attempt.profiles);
  assert_eq!(after.equipment, before_attempt.equipment);
  assert_eq!(after.operating, before_attempt.operating);
  ```
- [x] **2. Run red.** `SQLX_OFFLINE=true cargo test -p clacks-client-store --test settings_deletion --locked --offline`; run the targeted transfer regression as well.
- [x] **3. Add cleanup at the common persistence boundary.** After `metadata::client::store` has installed a client's new roles, inspect `value.working_state()`. For a Profile/Equipment Deleted working state, call the private metadata-only cleanup using that record key within the same transaction. Route its early unchanged return through this idempotent cleanup too, so retries cannot leave a selected tombstone. No recursion through profile save, implicit value reload, or treating `None`/failed loads as deletion.
  - This covers local saves, accepted responses, complete received batches and resolution installs that already use this boundary. Verify callers with `rg`, not a second list of special-case sync algorithms.
  - The hook may emit `debug` transaction-local detail; it cannot claim a committed deletion before the caller commits. Public save/delete boundaries retain their committed events. Runtime refresh in Task 5 reports observed committed selection cleanup without generic telemetry plumbing.
- [x] **4. Implement reviewed equipment deletion.** Snapshot equipment, the complete referring working-profile set and available replacement records together using `EquipmentDeletionPreview`. Require exactly one explicit replacement/removal choice per referring profile; reject duplicate/missing/extra profile choices and selecting the target itself. Each chosen replacement must exist in the preview; newly created candidates require refreshing it.
  - Re-read complete set, expected rows and replacements inside one immediate transaction. Stale set/value/replacement => no writes. Missing/non-live target => actionable no-effect outcome.
  - Compose normal `save_in` profile edits and equipment tombstone under that transaction; commit once. Do not allocate actors or alter prepared/original evidence.
  - After commit, report the actual repaired-profile count and deleted equipment ID; increment profile-update mutations by the committed repair count and equipment-delete mutations by one. A Stale/Unavailable/error path increments neither. No mutation metrics in `save_in` or the uncommitted cleanup hook.
  - Protect the generic public `save::save` deletion entry: equipment with live referring profiles requires this reviewed path. Profile deletion can use ordinary guarded save plus common cleanup. Do not put the local review restriction on the lower-level received-state loader.
- [x] **5. Verify green with reopen/transfer checks.** Regenerate query metadata, run settings deletion/transfers/operating tests, and the store clippy command. Confirm future server-side acceptance remains downstream; do not implement a fake server to test it.
- [x] **6. Commit.** Suggested subject: `Make source deletion repair references and forget selections atomically`. Body describes phantom-referrer checks, core working-state classification and preserved copied values.

## Task 4: Build transient operating and reusable-record editor state

**Files:** Create desktop `src/operating.rs`, `operating/{station,position,equipment,editor}.rs` and focused private tests; modify desktop `lib.rs` and workspace-inherited dependencies only when an existing model/sync dependency is missing. Complete the obsolete-helper cleanup in model `src/profile.rs`, `tests/defaults_and_tags.rs`, `tests/observability.rs` and its README after replacing the old immediate-load caller.

**Interfaces:** These types are crate-private and used by actual UI/runtime code:

```rust
pub(crate) struct FieldError { pub field: String, pub message: String }
pub(crate) struct OperatingEditor {
    pub expected: OperatingState,
    pub station: StationForm,
    pub equipment: EquipmentForm,
    pub operator_callsign: String,
    pub operator_name: String,
    pub power_watts: String,
    pub selected_profile: Option<SavedLocationProfile>,
    pub selected_equipment: Option<EquipmentConfiguration>,
}
pub(crate) struct ProfileEditor {
    pub id: ProfileId,
    pub expected: Option<ClientRecord>,
    pub name: String,
    pub station: StationForm,
    pub default_equipment: Option<EquipmentConfigurationId>,
}
pub(crate) struct EquipmentEditor {
    pub id: EquipmentConfigurationId,
    pub expected: Option<ClientRecord>,
    pub name: String,
    pub equipment: EquipmentForm,
}
// In operating/editor.rs:
impl OperatingEditor {
    pub fn new(state: &OperatingState) -> Self;
    pub fn stage_profile(&mut self, profile: &SavedLocationProfile,
        equipment: Option<&EquipmentConfiguration>) -> anyhow::Result<()>;
    pub fn stage_equipment(&mut self, equipment: &EquipmentConfiguration);
    pub fn build(&self, definitions: &DefinitionSet)
        -> Result<OperatingState, Vec<FieldError>>;
    pub fn refresh_sources(&mut self, snapshot: &SettingsSnapshot);
}
// StationForm::build(&self, definitions: &DefinitionSet) -> Result<StationSnapshot, Vec<FieldError>>
// EquipmentForm::build(&self) -> Result<Option<EquipmentValues>, Vec<FieldError>>
// ProfileEditor::build(&self, definitions: &DefinitionSet) -> Result<SavedLocationProfile, Vec<FieldError>>
// EquipmentEditor::build(&self) -> Result<EquipmentConfiguration, Vec<FieldError>>
```

Use named form fields and dedicated edit methods rather than anonymous tuples. `build` creates native values only at a save/apply boundary; the form owns raw incomplete text, not incomplete native records. Keep ordinary UI state private, not public APIs for tests.

- [x] **1. Write failing form cases.** In the owning modules, assert exact behavior for:
  - W7HEX/Home and Rig A: manually change grid CN87 to CN88 => profile selection None, equipment still A; change back => still None. Changing antenna Dipole to Loop clears only equipment. Operator W7ABC or power 25 clears neither.
  - Callsign ` w7hex/p ` builds W7HEX/P without dropping slash components. Merely normalizing equivalent text or repainting does not detach. Partial invalid manual text detaches its component and stays editable; Apply returns field errors, not a panic.
  - Switching views retains raw text and selections; stage Field with absent station fields clears them, no equipment default preserves current equipment, explicit B changes only equipment.
  - Power `0.001` W builds 1 mW; blank builds None; zero, negative, excess unsupported precision and overflow produce actionable field errors. Invalid power does not prevent building a valid ProfileEditor from station values.
  - Grid CN87 remains GridArea with retained length; two-character own entry is rejected; twelve-character entry survives. Editing explicit coordinates retains microdegrees and display grid length, and changing display length alone retains geographic corrections. An actual position change clears old-position corrections without autofilling new ones.
  - Unknown existing reference association survives unchanged; an explicitly edited known-group value uses `group::normalize_reference`, requiredness and syntax. Definition refresh alone changes no stored text or selection.

  After the actual station edit handler, verify the independent state:
  ```rust
  assert_eq!(editor.selected_profile, None);
  assert_eq!(editor.selected_equipment.as_ref().map(|e| e.id), Some(rig_a.id));
  assert_eq!(editor.expected, active_before_edit);
  ```
- [x] **2. Run red.** `SQLX_OFFLINE=true cargo test -p clacks-desktop operating:: --locked --offline`.
- [x] **3. Implement the forms.** Use `RecordedCallsign::new`, `Position`, `Locator`, `GridLength`, `Coordinates::from_decimal_degrees`, `Position::same_geographic_position`, native normalization and `group::normalize_reference`. Keep area/point intent explicit to avoid coordinate feedback. Parse power to checked integer milliwatts; no floating point or new decimal framework.
  - Detach on a user-originated value change, not on a `.changed()` caused by programmatic reload/derived display. For complete valid text compare native meaning; equivalent normalization is not a new value. Once detached, matching again does not reattach.
  - Remember which reference rows were explicitly edited so Save does not renormalize existing rows merely because definitions changed.
  - `refresh_sources` updates availability/source-change display only. Known working tombstones clear matching source copies and normalize the expected read basis only when all other active values/selections still match; otherwise retain the form and its ordinary stale-save condition. No reload or baseline advance on ordinary saved edits.
  - Creating/updating a reusable editor from form values copies only its owned fields. Existing profile target retains its saved default; new profile starts None. New equipment creation does not select it for the caller.
  - Stage a profile by validating its station and required configuration ID/values before changing any form fields, then copying owned station/equipment facts. Operator/power and equipment-with-no-default remain as their original editable text, even when incomplete. Do not construct a synthetic valid operating context merely to call an old copying helper. Remove native `load_profile`/`load_equipment` wrappers once their sole production store caller is replaced; the real `OperatingEditor::stage_profile`/`stage_equipment` methods and their tests own this interaction. Port useful old loading/copy assertions to those product paths and remove obsolete helper-span assertions without weakening reference-entry telemetry coverage.
- [x] **4. Run green and model validation regressions.** Run the form tests and `SQLX_OFFLINE=true cargo clippy -p clacks-desktop --all-targets --locked --offline -- -D warnings`. Test actual conversion outputs, not egui's text buffer implementation.
- [x] **5. Commit.** Suggested subject: `Add transient native operating and reusable-record editors`. Body explains independent form ownership, actual detachment and preservation of partial input without persisted unfinished records.

## Task 5: Integrate settings work with the production desktop runtime

**Files:** Create desktop `src/session/settings.rs` and private `session/settings/tests.rs`; modify `session.rs`, `application.rs`, `workspace.rs`, `ui.rs`; use the existing store/settings APIs. Do not change command-line/configuration semantics.

**Interfaces:** Keep `SessionCommand` lifecycle-only. Add separate crate-private `SettingsRequest`/`SettingsResponse` in `session/settings.rs` and pass their channels through actual application composition. Request variants are `Refresh`, `Apply`, `SaveProfile`, `SaveEquipment`, `DeleteProfile`, `PreviewEquipmentDeletion`, `DeleteEquipment`, with owned named fields matching store API arguments. A request carries a `oneshot::Sender<SettingsResponse>`; retain the receiver with its originating editor so late responses cannot update a different form. Use `SettingsResponse` variants for each typed store outcome plus actionable `Failed(String)`/closing rejection. Application composition creates a bounded request channel of eight and a settings watch channel; add its receiver/sender to `SessionResources` as `settings_requests` and `settings_updates`. The window receives `watch::Receiver<Option<Arc<SettingsSnapshot>>>`. This replaces the startup-only boolean as the source of operating presentation; retain lifecycle status for readiness/shutdown.

```rust
pub(crate) enum SettingsResponse {
    Refresh(SettingsSnapshot),
    Apply(OperatingSaveOutcome),
    SaveProfile(ProfileSaveResult),
    SaveEquipment(SaveOutcome),
    DeleteProfile(SaveOutcome),
    PreviewEquipmentDeletion(EquipmentDeletionPreview),
    DeleteEquipment(EquipmentDeleteOutcome),
    Failed(String),
    RefreshFailed { outcome: Box<SettingsResponse>, message: String },
    Closing,
}
```

- [x] **1. Write failing runtime integration cases.** Drive the real session with a temporary workspace:
  - Save profile/equipment => published snapshot contains actual stored pending records, active values unchanged.
  - Apply => published active values change only after successful transaction; failure/Stale preserves prior active state and caller form.
  - Complete a Save after its caller navigates elsewhere => saved record remains, unrelated form is not overwritten.
  - Start shutdown during an admitted settings write => finish or roll back that transaction before store close; reject later domain requests, retain responsive close handling and normal activation.
  - A known deletion refresh clears active/pending selections and permits Cancel without resurrecting them; an unavailable refresh preserves the prior snapshot and pending inputs.

  After the Save response and its committed snapshot publication:
  ```rust
  assert!(matches!(response, SettingsResponse::SaveEquipment(SaveOutcome::Saved(_))));
  assert_eq!(published.operating, active_before_save);
  assert_eq!(persisted_equipment.values, requested_equipment.values);
  ```
- [x] **2. Run red.** `SQLX_OFFLINE=true cargo test -p clacks-desktop session:: --locked --offline`.
- [x] **3. Extend session work without replacing the lifecycle design.** Share the established account store/pool and runtime. Open resources once, populate settings on Ready, admit requests only while ready, and publish a consistent refreshed snapshot after committed writes. Use bounded standard channels and standard JoinSet/task facilities; do not perform database work or block on results in the redraw callback.
  - Do not await a long database operation inside a select branch that prevents handling shutdown/activation. Track admitted work and drain it before `shutdown::finish` closes the store. Propagate span context with `tracing::Instrument` across spawned work.
  - Do not erase a known successful Save if its subsequent list refresh fails: report the committed outcome separately, keep the last usable snapshot and offer refresh. Never automatically repeat a mutation just to recover a failed refresh.
  - Source cleanup observed from committed snapshots produces an `info` domain event with account/source IDs and affected component, no copied values. Refreshes do not log keystrokes or emit generic completion events. Runtime handling boundaries report unexpected failures once, without dumping user forms/error payloads into logs.
- [x] **4. Run green with lifecycle regression coverage.** Run all session tests and existing `instance`/`ui` tests. Test shutdown through real Rust task/transaction coordination; no shipped delay flags or manual ignored tests.
- [x] **5. Commit.** Suggested subject: `Connect desktop settings requests to the existing workspace session`. Body explains lifecycle separation, committed-result publication and shutdown behavior.

## Task 6: Deliver the separated egui workflows and deletion review

**Files:** Create desktop `ui/{operating,profiles,equipment}.rs` and focused private tests under `ui/`; modify `ui.rs`, `ui/startup.rs`, desktop `operating/editor.rs` navigation as needed. Integrate Task 4 forms and Task 5 requests directly; no parallel demonstration UI.

**Interfaces:** Each view exposes a crate-private `show(ui: &mut egui::Ui, ...)` with named form/snapshot/request context owned by DesktopApp. Use a small navigation enum with explicit screens and stored caller forms; no generic router, callback registry or anonymous record tuples. Keep one OperatingEditor alive while visiting nested reusable editors. Track outstanding request receivers next to their originating form; UI rendering emits the real SettingsRequest variants.

- [x] **1. Write failing workflow tests using the production form/session paths.** Give egui controls stable IDs derived from record IDs. Exercise pointer/keyboard events or captured control rectangles from the actual rendered views, rather than a test-only action API:
  - Load Home/A, select B, Apply => visible active Home/B and saved Home/A; after restart it remains Home/B.
  - Edit grid => visible pending profile None and no profile reload/source-comparison control; active still Home. Switch equipment/station views, Cancel => active still Home/A.
  - Save Field from edited station while power is invalid => profile saved, Apply still invalid, no implicit selection; outer Cancel preserves Field.
  - Open Home profile, create Rig C, save it and cancel Home => C saved, Home default unchanged. Edit existing A inside Home and cancel Home => that separate A save remains.
  - Delete selected source with pending power edit => selection clears in active/pending forms, Cancel drops only pending power.
  - Profile save loses equipment default concurrently => visible saved-local/missing-reference feedback and explicit replacement/None choices; a stale profile basis is shown separately.
  - Duplicate display names remain separately selectable by native identity/context; missing source errors and failed writes keep typed text/focus usable.

  In the nested-save/outer-Cancel test, query the real store after driving the views:
  ```rust
  assert_eq!(persisted_home.default_equipment, Some(rig_a.id));
  assert_eq!(persisted_rig_c.values, entered_rig_c.values);
  assert_eq!(reopened.operating, active_before_edit);
  ```
- [x] **2. Run red.** `SQLX_OFFLINE=true cargo test -p clacks-desktop ui:: --locked --offline` and targeted settings session tests.
- [x] **3. Implement the real views.** Include an always-visible active source/value summary, one operating editor with Station/Equipment/Operator-power views and a single Apply/Cancel pair, separate profile/equipment lists and editors, explicit update-versus-new selection, visible profile/default-equipment association, and reviewed equipment deletion with one explicit choice per affected profile.
  - All geographic/manual correction fields and reference rows come from Task 4 forms, not a grid-only mockup. No geographic-data calls yet.
  - Only explicit loading reattaches a source. Hide source comparison/reload when detached. Saved-source updates remain indicators, not forced refreshes.
  - Save and Cancel target the currently shown reusable editor; return to its preserved caller. Operating Apply/Cancel closes that form after committed success/discard; closing a reusable editor does not apply operating values.
  - Show validation next to fields and request state at the owning action; disable duplicate submission while that request is pending, preserving navigation/activation/shutdown responsiveness. Use standard egui focus/navigation controls, not custom key handling for ordinary fields.
  - Deletion review reuses actual preview/outcome types. A stale affected set refreshes the review with user choices retained for still-matching IDs; new profiles require new explicit choices. No partial-success deletion claim.
- [x] **4. Verify green and telemetry decisions through real operations.** Extend store `tests/observability.rs` and desktop private tests to assert staging does not increment application/mutation counters; one committed create/update/delete increments the correct family; Unchanged/Stale/rollback does not. Deliberately distinctive callsign/location/equipment/reference strings must be absent from emitted telemetry; account/record IDs remain available at the intended enabled level. Reuse existing scoped test subscribers/recorders; do not retest library formatting or duplicate the full suite without telemetry.
- [x] **5. Commit.** Suggested subject: `Deliver operating settings and reusable station/equipment workflows`. Body explains Save/Apply/Cancel boundaries, explicit associations, actual detachment and Windows evidence still required if unavailable.

## Task 7: Complete durable acceptance, documentation and platform verification

**Files:** Update model/store/desktop READMEs, `docs/operating-settings.md` only for concrete clarified APIs/limits, related owning contract wording where needed, and tests owning any uncovered failures. Keep capability status in GitHub, not a new backlog/handoff/review narrative document.

**Interfaces:** Consume the shipped production executable and Tasks 1–6 APIs. No new application behavior or test-only public interface.

- [ ] **1. Run the complete persistence/workflow acceptance set.** Save real current settings and an existing QSO snapshot, edit/delete reusable records and Apply a new context, collect unreferenced rows, reopen and assert exact QSO facts unchanged. Add missing coverage to existing store operating/deletion tests. Verify older-schema upgrades, known-deletion cleanup after a received batch, nested saves and failure preservation as specified; do not count a helper-only clone comparison as durable evidence.
- [ ] **2. Verify query metadata against a fresh production schema.** Use a task-specific temporary directory and `DATABASE_URL` pointing to `sqlite://<absolute-file-path>`. Install the existing pinned tool if unavailable:

  ```sh
  cargo install sqlx-cli --version 0.9.0 --locked --no-default-features --features sqlite
  # Run the following with SQLX_OFFLINE=false and the temporary DATABASE_URL:
  cargo sqlx database create
  cargo sqlx migrate run --source lib/clacks-client-store/migrations
  cargo sqlx prepare --workspace -- --all-targets
  cargo sqlx prepare --check --workspace -- --all-targets
  ```

  Regenerate `.sqlx/` with each query-changing task, not only at the end. Do not point tooling at a user's real workspace or hand-edit generated metadata.
- [ ] **3. Run final repository checks.** Set `SQLX_OFFLINE=true` for compilation/test commands:

  ```sh
  cargo fmt --all --check
  cargo test --workspace --all-targets --locked --offline
  cargo test --workspace --doc --locked --offline
  cargo clippy --workspace --all-targets --locked --offline -- -D warnings
  # Set RUSTDOCFLAGS='-D warnings' for this command:
  cargo doc --workspace --no-deps --document-private-items --locked --offline
  cargo run --bin clacks-log --locked --offline -- --help
  git diff --check
  ```

  Passing means actual exit success for each applicable command. Fix failures in their owning task; do not weaken the specified behavior to preserve old tests.
- [ ] **4. Run native acceptance using the production executable.** On Windows use a separate disposable workspace directory with the normal `--workspace` flag. Exercise blank first use, profile/equipment create and explicit selection, keyboard traversal and position/reference entry, one Apply/Cancel pair, manual detachment, nested saves, equipment deletion review, close/reopen, and existing activation/shutdown. No test harness, artificial mutation-delay flag or human-dependent cargo test. Record unavailable Windows access as an outstanding acceptance step; Linux/macOS results do not substitute.
- [ ] **5. Update usage and contracts.** Document actual shipped paths/screens and save boundaries in the desktop README; document independent state/API and query regeneration in the store README; remove obsolete model comparison usage. Preserve distinction between delivered local operations and future transport/global conflict/geo autofill. Link the specification from relevant owning docs, not transient visualization paths.
- [ ] **6. Finish with reviewed commits and truthful evidence.** Each commit needs a subject plus motivation/constraints/verification body. If a review finds a defect introduced by an earlier task, fold it into that task's commit before publication without collapsing unrelated substantive tasks into one. Never stage unrelated research changes. Keep completed implementation plans only until merged, following the repository's existing cleanup practice.

## Dependency order, publication and handoff

Execute Tasks 1 → 2 → 3; Task 4 consumes their final interfaces, Task 5 connects those APIs to the existing runtime, Task 6 integrates the same forms/runtime into egui, and Task 7 verifies the whole production capability. The implementation plan's sequence is not a new roadmap or issue-status tracker.

Use the existing Git worktree workflow when implementation is authorized. The approved specification/notes/plan are currently uncommitted: transfer only those selected planning files into the isolated implementation checkout and commit a planning checkpoint before code, or use an already committed approved checkpoint if one exists by then. Do not lose them by creating a clean worktree from an older main or include unrelated research. Do not push or create a PR solely to run CI without the user's publication authorization. Once authorized, reuse the existing PR/main-only Rust workflow and monitor both native Windows and Linux jobs; do not enable all-branch push triggers. A green Windows job is automated evidence, not native graphics/input acceptance. Request human Windows smoke verification when access is unavailable.

Before calling #7 complete, perform the required whole-branch implementation review, resolve findings, and report delivered behavior, verification and remaining platform limitations. Do not close #7 for this plan or for a specification alone. This document is ready for user review; implementation has not started.

# Operating settings, station profiles and equipment

This specification defines the production desktop workflows for
[issue #7](https://github.com/jhelwig/clacks-log/issues/7), carrying forward
D21–D26, D68–D78, D183 and D280–D287. The [product contract](product.md),
[native model](native-model.md), [client store](client-store.md),
[desktop foundation](desktop.md) and [observability contract](observability.md)
retain their owning responsibilities. This contract owns operating-settings behavior; crate READMEs document the
production APIs and screen paths.

## 1. Outcome and boundaries

An operator can prepare one-off operating settings or reuse station profiles and
independent equipment configurations, entirely offline. Apply makes a complete
validated operating context active on this client. Reusable-record saves are
separate actions. Existing contacts retain their recorded historical snapshots.

Extend the real egui desktop application from #46 and SQLite store from #6. Keep
Rust 1.99.1, the wgpu renderer, the application-owned Tokio runtime, and the thin
`bin/clacks-log` wrapper around `lib/clacks-desktop`. No mock store, replacement
application shell, demonstration editor or test-only runtime is a deliverable.

This capability includes station/profile and equipment creation, editing,
selection, explicit reload, deletion, manual geographic details and reference
entry. It does not deliver contact entry/history (#9/#10), tags (#8), live
synchronization/enrollment, the account-wide conflict-review interface, geographic
autofill (Phase 4), equipment inventory, a dedicated amplifier field, contest
exchanges or serial assistance. Later work integrates the same components.

A profile, server connection, enrollment and an operating-session lifecycle are
not prerequisites. The desktop can remain open between intermittent operating.
Windows is the MVP target; keep application and domain boundaries cross-platform.

## 2. Selected interaction model

Separate reusable-record management from current operation:

- **Operating settings:** station, equipment and operator/power views edit one
  transient pending context. One Apply/Cancel pair governs that whole context.
  Switching views preserves edits and introduces no additional confirmation.
- **Station-profile editor:** Save Profile writes a reusable station snapshot and
  its explicitly selected optional equipment default. Equipment values are shown
  only as context for the association, not edited as profile-owned fields.
- **Equipment-configuration editor:** Save Configuration writes a reusable name,
  equipment description, antenna description and notes. Its editor can show which
  saved profiles reference it, but editing it does not reassign those profiles.

The [desktop UI contract](desktop-ui.md) defines the compact workbench, additional
views and operating-summary controls. Full operating editing uses a dialog with
the Apply/Cancel boundaries above. Quick profile, equipment and operator edits
are available from the summary; power uses inline commit/cancel behavior rather
than a popup or the full-dialog Apply button. These shortcuts preserve the value,
selection and reusable-record semantics in this specification. The active summary
distinguishes selected saved sources from unassociated values and reports source
changes for selections that remain. Pending dialog edits are visually distinct
from that active summary.

The combined form mixing reusable records, their association and operating copies
was rejected because the effects of saves and application were unclear. Nested
operating editors with “Return with changes” were rejected because they added a
second acceptance step. Retaining a “based on” source after manual adjustment was
rejected: D285 requires actual detachment, not a different label.

## 3. Values and ownership

### 3.1. Reusable account records

Use the existing native values and account-qualified identities:

| Record | Owned values |
| --- | --- |
| `SavedLocationProfile` | Profile ID, display name, complete `StationSnapshot`, optional `EquipmentConfigurationId` default. |
| `EquipmentConfiguration` | Configuration ID, display name, `EquipmentValues`. |
| `EquipmentValues` | Optional equipment description, antenna description and notes. |

Profiles and equipment are ordinary account-shared records, including offline
pending edits and retained deletions. Names are display labels, not lookup or
ownership keys. Use stable IDs in selections and associations. Do not add a
name-based merge or a uniqueness constraint absent from the native contract;
provide callsign/location or equipment details to distinguish like-named entries.

Equipment is reusable independently of any profile. A profile references one
optional saved configuration; it does not embed a private equipment override.
Operator information and transmit power belong to neither reusable record.

### 3.2. Active state and independent selections

The logical client-local state consists of:

```text
current: optional CurrentOperatingValues
selected_profile: optional exact last-loaded SavedLocationProfile
selected_equipment: optional exact last-loaded EquipmentConfiguration
```

These are logical fields, not prescribed Rust type declarations. Each selected
record copy already contains its native ID and comparison baseline. Avoid
redundant copies of an entire operating context solely to compare a selected
station or equipment source. A baseline is the exact saved value loaded, not
merely its ID and not a version-history graph.

Selections are independent. Selected Home can coexist with selected Rig B even
when Home defaults to Rig A. Show that equipment override without editing Home.
A profile with no equipment default leaves the current equipment values and
optional equipment selection unchanged.

When a source remains selected, compare its latest local working record with the
captured baseline to show saved-source changes. Do not automatically replace
operating values or advance that baseline. Offline, “latest” means locally
available, not an assertion about unseen server state.

Manual station changes clear `selected_profile`; manual equipment changes clear
`selected_equipment`. Retain their values without the affected source identity,
baseline, “based on” label, source comparison, reload action or source-change
indicator. Changing back to matching values does not reattach them. Operator or
power edits detach neither. Programmatically displaying derived coordinates or
repainting fields is not a second user edit that detaches the other component.

Before initial setup, `current` is absent and both selections are absent. The
editor can contain incomplete text, but Apply must construct valid native values;
it must not persist a fake callsign or an incomplete operating record. Reusable
equipment/profile management remains accessible before applying a first context.

### 3.3. Transient editor state

The pending operating context holds editable values, independent optional
selections/baselines and the expected active-state read basis. It is not a saved
record, does not synchronize and is not restored as an unfinished form on restart.
Keep its fields through tab changes, nested editor visits and failed operations.

Reusable-record editors hold their own form and expected shared-record basis.
Opening an inner editor preserves its caller's pending form. Cancelling it
returns without saving that editor's changes. Returning after Save preserves the
caller's form rather than loading the saved result into it.

## 4. Navigation and action effects

### 4.1. Prepare operating settings

Open Operating settings from the active summary. Start with the active context,
or blank entry fields if none has been initialized. Switching station/equipment/
operator views edits the same pending context.

Explicit profile loading stages the whole saved station snapshot, including
clearing absent fields and replacing references. A default equipment reference
also stages that configuration's copied values and baseline. Without a default,
equipment and its selection remain untouched. Operator and power remain unchanged.
Explicit equipment loading changes only equipment values and its selection.

A load reads a consistent source snapshot. If a required default configuration is
unavailable or already deleted, report the problem and preserve the pending form;
do not partially load the station or silently drop the profile's saved default.
A later source edit does not invalidate an otherwise valid local Apply (D283).

Apply validates and atomically saves the complete pending context, including
selection removals. Only after successful commit does the active summary change.
Cancel discards unapplied changes and closes the operating editor. Both leave
saved reusable records and historical contacts unchanged. Known-deletion cleanup
has the exception described in section 6; Cancel cannot restore deleted selections.

### 4.2. Manage reusable station profiles

A profile list shows each profile's station context and associated equipment
configuration, or that no default is specified. Create/edit opens a separate
profile editor. Require its name and station callsign, validate the native
station snapshot and preserve explicit absent fields.

The profile editor explicitly chooses an existing saved equipment default or no
default. It can open the selected equipment's editor or create a configuration,
then return with its unfinished profile form preserved. Creating equipment does
not silently select it as the profile's default; choose the resulting record
explicitly. Editing existing equipment changes that record, not the profile ID
reference. Cancel Profile does not undo a completed equipment save.

Save Profile from operating settings opens this same editor with the edited
station values, without requiring Apply. Explicitly choose whether to update a
saved profile or create a new one. When updating one, start its equipment-default
choice from that saved record; when creating one, start with no default. The
choice remains visible and editable. Do not substitute the pending operating
equipment selection for that default implicitly.

Save writes only the station profile, including the chosen default reference.
It excludes operator, power and edited equipment descriptions. It does not select
or apply the saved profile, including when its values match the pending station.
Cancelling the caller does not undo the completed profile save.

If its chosen equipment is concurrently deleted, distinguish that reference issue
from a stale profile save. When the expected profile basis still matches, retain
the edited profile as a pending local working record with a missing-reference
issue under D183. Report “saved locally; equipment reference needs repair,” and
offer explicit replacement or removal of that default. Do not discard the edit,
recreate equipment, silently clear the reference or claim it is ready for shared
acceptance. If the profile itself also changed, ordinary stale-save protection
still prevents overwriting it and preserves the editor's values for re-evaluation.

### 4.3. Manage reusable equipment configurations

An equipment list supports create/edit and deletion. The editor exposes name,
equipment description, antenna description and notes; no equipment inventory or
power ownership is introduced.

Save equipment from pending operating values opens this same editor with those
equipment facts. Explicitly choose an existing target or a new configuration.
The save does not alter the operating selection, reattach detached equipment,
change any profile's default reference or apply the context. Explicitly load the
saved result if it is to become the selected operating configuration.

## 5. Entry and validation

Reuse native validation and transformations rather than introducing a parallel
UI data model. Forms may retain temporarily invalid text; display useful errors
and preserve the form when Save/Apply cannot complete. Validate each saved record
independently; an invalid pending operator/power field does not prevent saving an
otherwise valid station profile or equipment configuration.

- Trim surrounding callsign whitespace and uppercase recorded callsigns,
  preserving prefixes, suffixes and slash components. A profile and an applied
  station context require a station callsign. Operator callsign and recorded
  operator name remain independently optional; never infer either from station.
- Use the native grid-area/explicit-point position model. Own-location entry
  requires at least four Maidenhead characters and supports through twelve.
  Grid entry derives center coordinates without claiming an exact point;
  coordinate entry preserves the explicit point and derives the displayed grid.
  Preserve chosen grid length, initially six for coordinate entry without one.
  Use native microdegree coordinate resolution. Location can remain absent.
- Expose optional geographic fields and manual corrections. A profile load copies
  them as part of the whole station snapshot. Preserve corrections at an unchanged
  position; do not carry old-position corrections to a newly entered position.
  Geographic autofill integration is later work, not an implicit #7 dependency.
- Record own-station participation/reference associations through the existing
  account group definitions: group ID plus optional identifier, unordered and
  deduplicated. Apply declared normalization/syntax rules to entry and explicit
  edits. Preserve unfamiliar existing associations without guessing properties
  or rewriting them on display. No group-management or contest-exchange editor.
- Treat blank optional text as absent; otherwise retain native text semantics.
  Transmit power is separate, optional, positive and uses native milliwatt
  resolution. Equipment fields remain optional under native validation.

Presentation-only changes, such as switching views, do not detach selections.
A manual value change detaches the affected component; correcting it back does
not implicitly reselect a source. Native normalization and recognition changes
alone are not operating edits.

## 6. Deletion and source availability

### 6.1. Selected source deletion

A known source deletion clears the matching active selection and baseline while
preserving all copied operating values. The other selection remains intact.
Clear matching selections in an open pending form as well, preserving its values
and manual edits. Cancel does not undo the deletion or restore those selections.
Apply must not resurrect an association to a source already known deleted.
Retain a confirmed deletion when the following view refresh fails. Exclude that
source from cached load/target choices and clear its selection in newly opened
operating forms; a later successful read of a live working source can establish
availability again.

This applies to a locally committed deletion and to a deletion becoming effective
through the existing synchronization transitions. Classify the current local
working state using those transitions; do not invent a second accepted-versus-
pending conflict policy. A failed read, unknown state or missing row is not proof
of deletion. Display a refresh/unavailable error without silently detaching or
creating a replacement record.

Saving/resolving a reusable record later does not automatically reattach operating
values. Loading it explicitly creates a new selection under the normal rules.
Restoring or cloning deleted reusable records is not part of this capability.

### 6.2. Referenced-equipment deletion

Before deleting equipment, read every locally visible working profile referencing
it. Present those profiles and require explicit removal or reassignment of each
default. Verify replacement configurations and show the consequences before commit.
Do not cascade-delete profiles or contacts, or erase copied equipment values.

Revalidate the equipment, replacement records and complete affected-profile set
inside the same immediate transaction that writes profile changes, the equipment
tombstone and active-selection cleanup. If any reviewed basis changed, including
a newly added referring profile, commit none of those effects and refresh the
review. Keep the user's form/choices available for re-evaluation. Do not hold a
transaction while asking for decisions.

These writes create normal local shared-record changes with their real original/
pending evidence. Local atomicity does not invent a network-wide transaction or
server conflict policy. Future synchronization must preserve D73/D183: a profile
edit targeting already deleted equipment remains pending for explicit repair;
a reference accepted before deletion requires refreshing affected profiles before
accepting the deletion. #7 delivers reusable local operations and integration
coverage; transport/server and the general conflict-review UI remain downstream.

## 7. Production integration and persistence

### 7.1. Responsibilities

`clacks-model` owns existing station/equipment values, validation and explicit
copying operations. Adapt its combined profile comparison to the independent
selection contract; do not preserve obsolete comparison behavior just to satisfy
existing tests. Client-local selection metadata is not part of synchronized
profile/equipment payloads or QSO snapshots.

`clacks-client-store` owns relational selection/baseline persistence, consistent
record reads, guarded shared saves, referenced-deletion transactions and
known-deletion cleanup. Extend existing production operations and private
transaction composition. Do not add a custom write queue, generation-counter
scheme, duplicate migration metadata or generic multi-record framework.

`clacks-desktop` owns transient forms, navigation, validation feedback and requests
through the existing runtime/session integration. Keep database operations off UI
callbacks. Publish committed results and refresh dependent lists/source indicators
without replacing pending forms. Use the existing native event-loop repaint and
Tokio communication facilities, not a custom reactive/invalidation framework.
Closing uses the existing graceful shutdown; transient unsaved forms do not become
background writes. In-flight writes complete or roll back at real transaction
boundaries, and the production application remains the manual smoke-test target.

### 7.2. Transactions and stale saves

| Operation | Boundary and effect |
| --- | --- |
| Read for selection | One ordinary snapshot reads the working profile and required equipment consistently; stage copies in memory only. |
| Apply operating context | One immediate transaction checks the expected active basis, validates values, clears known-deleted proposed selections, and writes values plus remaining selections/baselines together. Shared-source freshness alone does not block Apply. |
| Save reusable record | Existing guarded shared save; preserve original/prepared evidence and report unchanged/stale without claiming a mutation. A concurrently deleted equipment reference leaves the otherwise savable profile pending with an explicit missing-reference issue (D183), not a discarded edit. No operating-value write. |
| Delete profile | Guarded tombstone and matching active profile-selection cleanup in one transaction. No equipment-selection or operating-value changes. |
| Delete equipment | Complete reviewed profile-reference repairs, equipment tombstone and matching active equipment-selection cleanup in one transaction. |
| Install effective deletion | Existing store transition also clears matching active source metadata before publishing its committed result. |

Retain existing active-state stale-save protection. An intervening operating-value
change must not be overwritten by a pending form. Metadata-only cleanup of a
known deletion is not permission to discard that form: preserve edits, clear its
matching selection, refresh its read basis only after confirming the intervening
change is solely that cleanup, then revalidate. A storage error or stale result
must not be shown as a successful Apply or Save.

### 7.3. Schema evolution and historical snapshots

Use forward-only SQLx startup migrations, fixed queries in separate `.sql` files
with committed offline checking metadata, and the existing fully relational
payload/reference organization. Store immutable loaded profile/equipment values
as explicit local baseline roles; no opaque serialized operating-state column.
Allow each selection to be absent independently and clear references on
manual detachment/deletion without deleting data still rooted elsewhere.

Migrate existing combined loaded metadata conservatively. Preserve current values,
operator and power. Retain a profile selection only if its station still matches
the captured loaded station. Do not infer an equipment selection from equality
of descriptions or a profile's current default: old metadata lacks an independent
equipment-selection history. Start that selection absent unless existing stored
evidence actually establishes it. Never reload newer saved values during migration.
Exact columns, migration scripts and module/type names belong to the plan.

Future contact entry copies the applied native station/operator/power/equipment
facts into each QSO. It does not copy current selections or retain configuration
names as required historical facts. Profile/configuration edits and deletion,
Apply, Cancel and conflict resolution do not rewrite any existing QSO payload.

## 8. Worked workflows

1. **Equipment override:** Home defaults to Rig A. Load Rig B in the equipment
   view. Home remains selected; Rig B is selected independently. Apply activates
   that combination; Home still defaults to Rig A. Cancel retains prior operation.
2. **Manual adjustment:** change Home's pending grid. Its profile selection and
   baseline disappear immediately in the form; equipment stays selected. Switch
   views and back without losing the grid. Apply commits the new grid without a
   profile association. Cancel preserves the prior active Home selection.
3. **Save before Apply:** enter one-off station values and Save Profile as Field,
   explicitly selecting its equipment default. Return to the pending operating
   form. It remains unassociated. Cancel leaves active operation unchanged and
   Field saved. Loading Field explicitly stages its values and selection.
4. **Nested equipment save:** edit Home's profile and create Rig C from its
   equipment section. Save Rig C, return to the unfinished Home form, then Cancel
   Home. Rig C remains saved; Home's default association remains unchanged.
5. **Saved source changes:** Rig B is selected and loaded as Vertical. Its saved
   antenna becomes End-fed while the operating form is open. Show the change,
   preserving Vertical and the captured baseline. Apply keeps Vertical. Explicit
   reload takes End-fed. A manual antenna edit instead clears Rig B's selection.
6. **Deletion during editing:** active and pending station both select Home;
   the pending power is edited. Delete Home through profile management. Both
   profile selections clear, with station values and pending power preserved.
   Cancel discards the pending power edit but cannot restore the Home selection.
7. **Referenced equipment:** Home and Field reference Rig A. Choose Rig B for
   Home and no default for Field, then delete Rig A. All local changes commit
   together; active equipment values remain, but selection of Rig A clears.
   If another referring profile appeared after preview, refresh with no deletion.

## 9. Observability

Use [the existing policy](observability.md) without new telemetry abstractions.
The concrete diagnostic questions are whether a requested Apply/Save committed,
why it was rejected or failed, and which referenced-deletion review became stale.

Instrument meaningful runtime/store operations with normal module targets and
`#[tracing::instrument(skip_all, ...)]`. Carry explicit account/record IDs where
needed for correlation, never names, callsigns, coordinates, reference strings,
operator information, equipment descriptions or whole forms. Preserve async span
context through existing framework facilities. Do not log every keystroke, tab
switch or scalar validation; log failures once at their handling boundary.

Successful Apply, reusable-record save/delete and deletion-driven source cleanup
are `info` events with explicit committed effects and affected counts. Expected
validation/stale outcomes are useful diagnostic events, not automatic warnings
or failures. Load selection is staging, not an applied-context success. Unexpected
persistence failures are handled once with actionable UI feedback and an `error`
event; ordinary source changes or detachment are not anomalies.

Metrics count actual committed changes: operating-context applications, station-
profile mutations and equipment-configuration mutations are separate quantities.
Use bounded change labels where useful; record/account IDs and user values are
not metric labels. Unchanged saves, retries and failed transactions do not inflate
mutation counters. Selection staging requires no extra metric. Use framework
span timing for investigation; do not add generic lifecycle wrappers, manual
completion machinery or a timer for every helper.

## 10. Acceptance and implementation-plan boundary

Verify Clacks behavior through native operations, real store transactions and
noninteractive desktop workflow tests. Do not test dependency internals or use
manual/ignored cargo tests as application harnesses.

Required coverage includes:

- first one-off context without a profile/server; valid values survive reopen;
- whole-station loading with absent fields, a default configuration, and no
  default; operator/power preservation and account isolation;
- independent detachment, no implicit reattachment on matching values, no source
  indicators after detachment, shared pending views and Apply/Cancel boundaries;
- Save Profile/Configuration before Apply, explicit target/default selection,
  nested Save followed by outer Cancel, and no implicit load after Save;
- saved-source updates while editing, unchanged copied values and baseline until
  reload, and normalization/recognition changes that do not create edits;
- known deletion cleanup in active/open forms, Cancel after cleanup, Apply racing
  deletion, and unavailable/failed reads that do not trigger false deletion;
- referenced-equipment deletion with explicit repairs, full stale-set detection,
  rollback on failure, and preservation of current/historical copies;
- Save Profile racing equipment deletion: retain the profile edit and report its
  missing reference when its own basis matches; keep ordinary stale protection
  and the form when the profile record also changed;
- stale ordinary saves and failed writes preserving forms and durable state;
- real migration/reopen evidence for independent optional selections, including
  an older combined baseline with manually adjusted current values;
- telemetry distinguishing staging, unchanged/stale outcomes and committed
  effects, retaining correlation at configured levels and excluding user payloads.

Use production integration tests with existing synchronization transitions for
relevant pending/deletion cases; no fake transport/server replaces future work.
Windows continuous integration (CI) must run the applicable workspace checks. Native Windows human smoke
checks exercise the shipped executable's navigation, keyboard/focus, selection,
Save/Apply/Cancel, deletion review and reopen behavior. Linux/macOS checks support
cross-platform architecture without being Windows acceptance evidence.

The implementation plan defines focused modules, public production APIs, schema
migration/query changes, concrete UI controls, task sequencing and commands. It
must preserve the contracts above and scope later integrations honestly. No
remaining product choice is intentionally deferred by this specification.

# Bundled offline station-location assistance

Engineering design for [issue #23](https://github.com/jhelwig/clacks-log/issues/23).
Geographic assistance is
shared native functionality consumed by UI and non-UI workflows; forms do not
own its rules.

## Outcome and authority

Given an entered operating position, the desktop uses installed reference data
to populate independently unambiguous geographic fields, offers meaningful
choices where coverage is ambiguous or incomplete, and preserves manual
corrections at an unchanged position. Newly captured contacts use the same
functionality with their supplied-field/default precedence. It works without
enrollment or network access. Each workflow's existing save operation commits
the resulting native snapshot.

DX Century Club is abbreviated DXCC; International Telecommunication Union is
ITU. CQ refers to CQ magazine's operating zones. Secure Hash Algorithm with a
256-bit digest is SHA-256. Geographic JavaScript Object Notation is GeoJSON.
Continuous integration is CI; Windows Installer packages are MSI files; macOS
disk images are DMG files. These terms name existing formats and facilities.

| Requirement | Owning source |
| --- | --- |
| Offline DXCC, state, county, CQ and ITU assistance with corrections | D77–D78, D132–D134; issue #23 |
| Required U.S. coverage; desired worldwide radio coverage with documented limits | D133, D239; [source research](../../../references/location-lookup.md) |
| Grid-area versus explicit-point lookup; independent field resolution | D184, D260, D266–D267; [native model](../../native-model.md#6-positions-and-geographic-details) |
| Existing production producer, reader, sources and schema | D257–D269; [geographic pipeline](../../geographic-pipeline.md) |
| Snapshot preservation, separate Apply and profile-save boundaries | [Operating settings](../../operating-settings.md), [manual logging](../../manual-logging.md) |
| Separate directory facts and whole-location source selection | [Integrations](../../integrations.md), native-model source selection |
| One cacheable geographic build artifact shared by native packages; generated data stays outside Git, including Git LFS | User direction in this design discussion |
| Independently replaceable/versioned data bundled with application releases | D134; issue #23; no separate publication prerequisite |
| Server-provided updates remain future work | User direction in this design discussion; D134 does not require an updater |
| Shared geographic functionality with UI and non-UI consumers | User correction and direction in this design discussion; D80 and the existing capture design retain their workflow rules |
| Meaningful diagnostics and proportional verification | [Observability](../../observability.md), repository and user working agreements |

This is one capability. Data preparation, editor integration and packaging are
implementation responsibilities, not separate delivery milestones. This design
does not authorize implementation, publication or closing #23.

## Existing components and approach

- `clacks-geography` supplies production validation and runtime integrity loading, retains
  file handles, and queries indexed FlatGeobuf layers. Results include a release
  ID and independently scoped candidates and coverage for every field. Values,
  display labels and source identities are already separate.
- `clacks-geography-builder` acquires pinned sources, transforms and compiles
  reviewed geometry/mappings, validates through the production reader and runs
  recipe acceptance cases. It rejects different bytes under an existing release
  ID. `clacks-geography-build` already exposes fetch, inspect, build and verify.
- `Position::lookup_geometry` preserves the entire locator area or explicit
  point. `same_geographic_position` ignores explicit-point display precision.
- `StationForm` is shared by operating settings, profiles and both contact
  sides. The quick DX-grid field reaches its edit method too. `ContactForm`
  reopens recorded facts without applying current defaults.
- Existing guarded settings/profile/contact saves persist native geographic
  fields and position evidence. No geographic persistence table is missing.
- Desktop startup owns a Tokio runtime, configuration composition and shared
  editor/capture geographic consumers. Existing geography metrics are included
  in desktop telemetry configuration.
- The native packaging workflow produces per-user Windows installers and
  signed/notarized macOS application bundles and includes geographic resources.

Extend these components. No alternative geometry engine, embedded spatial
database, source-ingestion dependency in the desktop, custom write queue or
geographic synchronization record is required.

### Approaches considered

Geography is a normal cacheable build dependency. Restore its validated output
when the generation inputs match; otherwise generate it from the current source
checkout. Both native packaging jobs consume one common output. A generator
change and its resulting data therefore travel in the same PR and build pipeline.
Separate geographic releases, registry uploads and hand-maintained artifact pins
are not prerequisites. A cache miss is ordinary work, not a release-process error.
Independently rebuilding on each native platform would duplicate work without
establishing common bytes. Copying resources into each workspace adds lifecycle
work with no demonstrated need.

For assistance there is one supported production query interface and one native
recorded-location representation. Extend them with shared value-level behavior;
the existing station form and automatic capture are consumers. Keeping rules in
`StationForm` would exclude capture, while automatically deriving inside every
native constructor or storage save would change reload, synchronization and
explicit field-resolution semantics. Neither boundary matches the requirements.

## Shared functionality and production consumers

### Responsibility boundaries

Proposed engineering ownership uses the existing crate direction without another
crate or plugin/service abstraction:

| Owner | Responsibility |
| --- | --- |
| `clacks-model::position` and `location` | Native positions/fields, geographic-position equality and ordinary validation; construction/deserialization never triggers assistance |
| `clacks-geography::lookup` | Existing point/whole-grid queries, field candidates, coverage and release provenance |
| `clacks-geography::assistance` (new focused module) | Pure native-value rules for eligible automatic population and position-bound correction lifetime; no UI, runtime, database, download or provider parsing dependency |
| Desktop application/runtime composition | Select and retain the reader, run blocking queries off the UI/receiver loops, and deliver results to the current operation |
| Editor adapters | Incomplete text, validation presentation, candidate selection and translating user intent into native operations; no independent geographic rules |
| Live-capture adapter | Existing source parsing, optional-field rejection, own-default precedence, format pairing and contact submission; calls the same assistance operations without constructing a form |
| Existing store/synchronization operations | Persist or reconcile the chosen snapshot, without implicit enrichment |

The concrete gap is application of the reader's existing results to native
values, shared by current consumers. Extend existing result/native types rather
than wrapping them in a generic context, provenance framework or alternate
geographic model. The shared rules receive a valid native position, geographic
fields and explicit per-field eligibility/protection from the caller. They
return proposed native values plus the existing candidates/coverage needed by
interactive consumers. Reuse `same_geographic_position` and `lookup_geometry`.

Nonempty supplied values, preserved snapshot values and explicit manual choices
are protected. Manual clearing is protected absence; it is not equivalent to an
unfilled field. Invalid supplied optional fields remain rejected under their
existing parser policy, rather than being silently replaced by geographic
inference. These distinctions are transient operation input, not new persisted
origin metadata. Correction state follows the native position, not a widget ID.

An adapter does not need a valid callsign or a complete valid station record to
request assistance: a valid position is sufficient. An invalid manual field
remains editable/protected without preventing independent fields from resolving.
The geographic layer never rewrites the position or treats provider fallback
display coordinates as newly entered recorded evidence.

### Consumer trace and integration disposition

| Existing consumer/path | Geographic behavior in #23 |
| --- | --- |
| `ui/operating/pending.rs` -> `OperatingEditor` -> session `SettingsAction::Apply` | Native assistance updates pending operating values; existing Apply/Cancel commits/discards them |
| `ui/profiles.rs` -> `ProfileEditor` -> `settings::save_profile` | Same assistance for deliberate location edits; opening/loading a profile copies its snapshot without recalculation |
| `ui/logging/entry.rs`, `details.rs` -> `ContactForm`/`NewEntry` -> contact save | Quick DX-grid and both station detail panes consume shared assistance; incomplete text stays in the adapter |
| `logging/defaults.rs` -> `OperatingEditor::new`/`StationForm::from_snapshot` | Copy coherent native defaults and preserve position-bound explicit corrections; copying is not a fresh geographic query |
| `ui/contacts/editor.rs` -> `ContactForm::from_qso` -> guarded contact save | Opening preserves history; deliberate position edits consume shared assistance before normal submission |
| `ws_suite/contact.rs` -> `pairing.rs` -> `runtime::save` | Validate/project and choose the representation first; assist both native station sides before the existing contact transaction, preserving supplied values and own-default rules |
| `review/fields/station.rs::{copy,edit}` and `ui/review/editors.rs` | Preserve explicit per-field resolution. Copying/editing a position must not clear or fill other independently chosen resolution fields |
| `directory/providers/mapping.rs`, directory persistence and `ui/logging/directory.rs` | Preserve attributed provider facts and existing whole-source fallback. Do not rewrite provider snapshots or turn displayed fallback into recorded input |
| `clacks-model::effective` and service applicability/projection | Read/project selected source values without implicit geographic completion or recorded-value mutation |
| Client/server `payload/contact.rs`, `payload/directory.rs`, service payload codecs and native deserialization | Decode exact recorded/provider values; synchronization, reload and restore do not invoke assistance |
| Geographic builder compilation/verification and CLI | Produce and query immutable data using the existing pipeline; no contact/UI policy or new application consumer |

The capture source adapter constructs native values directly; its lack of a
`StationForm` is not a scope decision. Conversely, a shared constructor/save
hook is too broad: conflict-resolution copy/edit explicitly promises to change
only the selected member, and source codecs preserve recorded facts. This is
an operation boundary, not a distinction between visible and invisible callers.

Historical import is not currently a contact-ingestion consumer in this source
baseline. D38/D42 require historical station context and selected field updates,
not current-default fallback. #23 does not implement an import workflow or
implicitly enrich historical missing own-station fields. A future authorized
assistance operation can use the same native functions without borrowing a UI.

## Data maintenance and build distribution

### Inputs, cache and generation

Maintain source pins, factual mappings, GeoJSON corrections, exclusions, notices,
coverage limits and independently justified acceptance cases under
`references/geography/`. Keep generated bundles and acquired archives outside
Git, including Git Large File Storage (LFS). The supported authoring command
updates reviewed inputs deliberately; ordinary builds consume those inputs and
never rerun authoring or discover newer upstream versions.

The build dependency is a complete validated bundle directory, not a separately
published geographic release. One Linux preparation job in the existing native
packaging pipeline performs these operations automatically:

1. Fingerprint generation inputs using standard build/workflow hashing.
2. Restore an exact matching successful output from the ordinary build cache.
3. On a miss, build the producer from the current checkout with Rust 1.99.1 and
   locked dependencies, acquire digest-pinned originals through the existing
   verified source cache, then compile and run production acceptance.
4. Save only successfully validated output to the cache, and transfer that one
   directory to downstream jobs using existing GitHub Actions artifacts.

The input fingerprint covers the shipping recipe and every referenced notice,
correction and other input; the producer's local dependency sources and embedded
catalogs (`clacks-geography-builder`, `clacks-geography`, `clacks-model` and the
build CLI); their manifests; root Cargo manifest/lockfile, toolchain and relevant
Cargo configuration; and the producer build invocation/settings. It excludes
application source outside that dependency set, Git commit IDs, wall-clock time,
run IDs and documentation unrelated to generation. Whole root manifest/lockfile
hashing is a conservative invalidation choice: an unrelated dependency edit may
cause a miss. No custom Cargo dependency-graph cache system is justified to avoid
that occasional rebuild. Input additions must update the declared build inputs.

Use exact cache matches only, with no prefix fallback to older compiled data.
Cache expiry, eviction, branch scoping and concurrent misses may cause another
build; correctness never depends on a cache entry existing. A failed acquisition,
compilation or acceptance fails packaging rather than substituting another
bundle. Restoring validated output does not rerun polygon, feature, index or
inventory acceptance. Ordinary tests stay source-independent; the data build
runs when packaging needs it, not on every workspace test invocation.

The cache and its recorded digest are trusted build outputs under the existing
workflow permissions and cache scoping. Digest checks detect damage or mixing;
they do not independently authenticate a maliciously replaced bundle/digest pair.
Source-cache entries retain their existing acquisition-time digest checks; a
corrupt entry fails rather than silently acquiring different source bytes.

Use the input fingerprint as generated data identity, independent of application
version. Remove manual release IDs and producer commit pins from the shipping
recipe. The same fingerprint covers producer content and locked build inputs,
replacing the separate manually declared producer revision. A current checkout's
Git SHA is not required to match the provenance of a reusable cache entry.
Manifest source pins, payload digests, notices, transformations and coverage
retain their existing ownership. The manifest digest identifies the actual
output bytes; the input fingerprint identifies the requested build. Neither
requires a Git tag or a schema-version bump for this unshipped change.

### Packaging and artifact identity

After generation, compute the manifest digest and preserve it alongside the
validated directory as build output, outside that directory. Restore and transfer
that identity with the cached output. It is supplied by the build, not accepted
from an installed manifest's self-declaration. Native compilation embeds it via
`CLACKS_GEOGRAPHY_MANIFEST_SHA256`; existing packaging inspection checks the
payload against that same expected digest and the reader's ordinary integrity
checks. A local build without packaged data remains usable for manual work and
explicit external selection; release packaging requires the generated identity.

Both platform jobs download the same common directory and expected identity.
No checked-in `deploy/desktop/geography.json`, repository/tag/asset selection,
manual digest update, standalone geography workflow, draft or geographic tag is
required. Actions already transfers directory artifacts; a custom standalone ZIP
writer and its command/tests are unnecessary solely for that transfer. Keep
existing bundle-writing/validation facilities; do not replace them with a new
publisher, registry client, queue or cache manager.

| Platform | Installed default bundle | Lifecycle |
| --- | --- | --- |
| Windows | `geography` beneath the application installation directory | Ordinary installer-managed files, replaced/removed through upgrade/uninstall |
| macOS | `Contents/Resources/geography` in `Clacks Log.app` | Included before signing/notarization and replaced with the application |

Use the existing MSI builder's directory support and ordinary resource copying.
Read installed data in place without copying into workspaces. The existing
application package and package evidence retain the shipped bundle and its
identity; cache retention is not release retention. No separate geography release
asset, evidence JSON asset or measurements JSON asset is needed for #23.

Standard facilities:
[GitHub Actions cache behavior and scoping](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching),
[workflow artifact transfer](https://docs.github.com/en/actions/how-tos/write-workflows/choose-what-workflows-do/store-and-share-data),
[MSI builder directory support](https://github.com/jkroepke/msi-builder/blob/v1.2.0/pkg/config/schema.json),
[Apple signed resources](https://developer.apple.com/library/archive/technotes/tn2206/_index.html).

### Desktop selection and future server provision

Proposed engineering default: add one optional bundle-directory override to the
existing startup configuration, resolved by its normal path/precedence rules.
Without an override, use the installed default. An explicit override selects
that exact release directory; invalid selection is reported, not silently
replaced with another dataset. Missing or invalid data disables assistance, not
ordinary workspace opening or otherwise valid manual saves. Development uses
this same override with an
explicit data path. No personal development configuration enters the repository.

Open and retain the selected bundle once per owning desktop process; do not
reopen for each form or frame. Selection takes effect at startup. No hot reload,
archive importer, automatic version discovery or replacement command is added
by this design. A complete externally prepared directory can be selected without
changing the installed application or recorded account data.

All artifact semantics, feature/index/inventory checks and polygon validity are
established during production and explicit producer verification. Selection
supplies an independent expected manifest digest: the generated build identity for
bundled data, or explicit external configuration for another prepared release.
The directory override defaults to the packaged identity for that same release
elsewhere; another identity uses `--geography-manifest-sha256` or
`[geography] manifest_sha256`. Runtime opening checks that manifest identity
before interpreting it, schema compatibility and declared payload integrity;
it retains file handles without repeating production acceptance. Safe decoding
and I/O failure handling remain at actual query boundaries.

Future server distribution can consume a validated bundle produced by this same
build dependency, independently of an application release. A directory build
output does not prescribe the server's eventual download transport or require a
registry, bucket or public release service now. Independently updated client data
must live outside signed application resources and workspace/account records.
Server endpoints, update discovery, downloading, selection, retention and trust
policy remain future work. No synchronization message or updater is added by
#23, and changing reference data never rewrites saved facts or corrections.

## Shipping coverage and capacity

Extend the selected Census inputs to required U.S. state/county coverage,
retaining actual Connecticut counties and the prescribed regional coordinate
operations, onshore masks and disclosed no-shift assignments. Extend DXCC and
zone mappings/corrections to provide the required U.S. fields. Review the complete
selected feature inventory; representative cases alone do not prove coverage.
Do not excuse a missing required U.S. region as an optional worldwide omission.

Pursue worldwide DXCC/CQ/ITU coverage using the accepted Natural Earth and HA8TKS
sources. Record unsupported entities, incomplete geometry, exclusions and
accuracy limits. No complete-world fidelity gate or worldwide administrative
subdivision project is introduced. Preserve D260's ambiguity/coverage rules and
D269's exact-operation seams; do not simplify, snap, buffer or pick the largest
overlap merely to produce apparently complete results.

Historical upstream archive sizes are not installed-size or latency estimates.
Measure compiled size, final native package size, release build time/peak memory,
cold/warm opening and representative point/grid query latency when shipping data
is prepared. Keep full geometry and
one coherent bundle unless measurements establish a concrete need to revisit
that choice.

## Position edits, automatic results and correction lifetime

The shared native operations implement the following rules. Editors consume
them through valid position events; source capture supplies already validated
native input and field eligibility. Incomplete text and drawing remain adapter
responsibilities. These are proposed engineering integration of the established
snapshot and correction contracts.

| Input event | Result |
| --- | --- |
| Deliberate newly entered or geographically changed operating/contact position | Expire old-position geographic values and choices; resolve eligible fields for the new native geometry |
| Incomplete/invalid position text | Preserve input and last-valid-position corrections; do not query invalid geometry or apply results while input is invalid |
| Valid removal of position | Clear old-position geographic values and suggestions; allow manual geographic entry without a position |
| Same grid area or same explicit coordinates | Preserve corrections; explicit-point display-grid precision does not count as movement |
| Load/reload a profile, open a contact, restore applied settings | Copy its complete recorded snapshot without recalculation, including absent fields |
| Install/select another geographic release | Use it for subsequent assistance; do not scan or rewrite existing values |

Editor adapters use `build_recorded` semantics when recognizing contacted-station
positions; the current form change handler rejects two-character contacted
locators through own-station validation. Own operating-entry minimum precision
remains unchanged. Transfer position/correction bookkeeping to shared native
operations instead of leaving a second implementation in `StationForm`. Keep
the position basis synchronized when default refresh copies values; current
direct assignments bypass `last_position`.

Separate automatic changes from explicit operator edits. Unique candidates with
complete coverage populate their field; partial/multiple/unknown results do not
autofill it. Manual text edits, explicit clearing and candidate selection take
precedence for that position. Never infer recordable values from labels.

`NewEntry::capture_corrections` must not classify generated values as operator
corrections. Its geographic correction flags must expire with their position.
An explicitly corrected contact position keeps its own geographic context;
later default refresh must not mix in values for the active default's different
position. When the entry follows a new active position, drop old-position
corrections and copy the new default snapshot coherently. Unrelated own fields
retain their existing default-following behavior. Do not add persistent manual
origin flags: reopened snapshots are preserved as a whole.

Run blocking opening/query work outside the UI thread and packet receiver loop
using the existing Tokio runtime and standard task/synchronization facilities.
Both editor and capture adapters use this runtime-owned reader, not independent
engines/readers per form or workflow. Keep result application bound to the live
operation, native geographic position and selected release. Discard results for
a closed, replaced, invalid or moved operation; preserve intervening operator
choices. A move away and back must not resurrect abandoned choices. Concrete
task/channel composition belongs to the implementation plan, not a new service
or durable job system. Pure assistance rules remain independently callable with
native values and completed reader results.

### Automatic live capture

Extend the production `ws_suite` projection/save composition, not the editor.
Keep the existing format selection, 100 ms pairing interval, source identity,
stable QSO ID, duplicate suppression and captured active-context basis.
Geographic querying occurs after selecting the usable representation and before
its existing save transaction; do not query both representations merely because
they were paired. Existing async capture work can await local assistance without
blocking packet reception. Do not hold a database transaction while querying.

Apply D80 first: valid nonempty supplied values take precedence; omitted/empty
own fields can use the captured active defaults. Protect all values selected by
that existing projection. Shared assistance then fills only still-eligible absent
fields against the resulting station's native position. Do not erase supplied
or defaulted values merely because they disagree with the bundle. No defaults
are borrowed for the contacted station. A nonempty rejected field retains its
warning/protected absence rather than being treated as omitted input. An invalid
supplied grid does not authorize substituting a default grid or querying it.

One complete candidate can fill each eligible field independently. Ambiguous,
partial or unknown fields remain absent for a noninteractive caller; candidates
are not arbitrarily selected and no modal choice interrupts capture. Unavailable
data or a query error leaves the projected contact usable, with appropriate
diagnostics, rather than failing otherwise valid capture. Once geographic work
has produced the submitted native snapshot, keep that snapshot stable through
its save outcome; no late enrichment changes a committed contact. Corrections
to captured contacts use the ordinary contact-editing workflow and shared rules.
Incoming facts and assistance never modify active settings or saved profiles.

## Presentation, save boundaries and other sources

Extend existing field controls rather than creating another location editor.
Show understandable candidate labels and independent field coverage. Complete
coverage with several candidates, partial coverage, no coverage and actual
reader failures are distinct states. Operators may choose supported candidates
or enter their own values; partial coverage is not presented as confidence.
Keep parent-scope context visible so equal codes remain distinguishable.

The quick DX-grid control and Contact details share the same result state. Our
settings, operating settings and profile editing use the same transition rules.
No extra geographic Apply/Save action is introduced. Existing parent actions
commit the pending fields through the current guarded storage operations.

Assistance is optional to an otherwise valid save: pending or unavailable lookup
does not block Apply, Save Profile or Save QSO. Save commits the values validated
at submission. Freeze that submitted form while its save is pending; results
cannot alter a submitted value or silently enrich the saved record afterward.
On success, retire that form's interest; on failure, retain input for correction
or retry through the existing workflow. Cancel never commits assistance results.

Preserve directory hints as display-only separate source facts. Query the
native position selected by the calling operation, not a displayed provider
fallback or a grid center mistaken for an explicit point. Shared assistance
proposes recorded values for the current operation; it does not complete
lower-priority provider snapshots or alter whole-location fallback. A common
persistence hook must not run assistance during synchronization, restore,
profile loading, conflict resolution or output projection. These operations
preserve explicitly recorded/selected values, even when they construct the same
native types as assisted workflows.

## Diagnostics and verification

Reuse reader lookup duration and field-coverage metrics and producer release
instrumentation. Add meaningful desktop handling context for bundle selection,
validation success/failure, query failures and rejected obsolete results.
Successful selection/recovery is visible at `info`; failures are recorded at
the handling boundary. Per-edit detail belongs at `debug`, without per-frame
or per-feature logging. Correlate assistance requests with their editing or
capture operation and retain release
identity in diagnostics, not coordinates, locator text, callsigns, payload dumps
or arbitrary paths. Metric dimensions remain bounded; do not label metrics by
release IDs or form/request IDs. Do not count suggestions as committed edits.

Verification targets Clacks-owned integration mistakes rather than duplicating
geometry-library checks:

- Extend recipe acceptance with independently justified U.S. region/field and
  boundary cases, reviewed inventory/omission checks and relevant worldwide
  cases. Run original-source acceptance on data-build cache misses,
  not on every application build.
- Exercise shared native rules directly for field eligibility, supplied/manual
  preservation, correction lifetime, independent complete/ambiguous/partial
  fields and position identity. Do not repeat existing geometry predicates.
- Extend existing UI scenarios only for adapter risks: valid-position requests
  from quick/details/profile/operating consumers, two-character contacted grids,
  manual choice/clearing, default refresh and stale results. Use semantic
  egui_kittest interactions and actual requests, without replaying the native
  rule suite through each form.
- Extend existing production capture scenarios to prove both native station
  sides receive eligible geography with supplied/default precedence, invalid
  optional-field protection, nonfatal unavailable/ambiguous results and ordinary
  duplicate/save behavior. Use independently expected geographic outputs and
  real reader inputs; do not mock commands or add an alternative capture engine.
- Use existing save/reload operations to establish snapshot preservation,
  submission immutability and ordinary stale-save behavior. Changing the
  selected data release must not rewrite existing saved profiles/contacts.
- Packaging verifies expected manifest and declared payload identities, with
  identical geographic bytes on both platforms. Full inventory acceptance
  remains in production. Establish that normal application composition discovers
  bundled data and makes assistance available offline. Reuse existing native,
  reader, editor and capture evidence; perform a focused packaged-app startup
  check only for an uncovered resource/identity handoff, using isolated
  workspaces/configuration. Explicit external selection and snapshot/correction
  preservation need their existing functional evidence, not an installer suite.
  Clean-machine installation, upgrade, uninstall and distribution-trust
  acceptance belong to #35/#70; they are not completion gates for #23.
- Verify Clacks's reporting decisions for real selection/query failures and
  obsolete-result handling without testing telemetry-library formatting or
  duplicating the functional suite under every logging configuration.

Keep ordinary source-independent regression checks in existing relevant suites.
Justify each new CI check by its concrete integration failure and cost; data
preparation/performance measurements are implementation evidence, not CI gates. Document
Windows/macOS evidence and any unmeasured capacity honestly in the issue/PR.

## Engineering ownership and scope

Shipping selections and mappings belong to the maintained recipe. Build inputs,
resource discovery, configuration, shared native interfaces,
runtime ownership and controls belong to their implementing modules and owning
contracts.

Automatic live capture consumes the shared functionality through the
[capture design](2026-11-03-ws-suite-capture-design.md#6-native-values-and-live-fallback).
Existing D80 precedence and conflict-resolution/source-preservation contracts
govern its application behavior. Server-provided updates remain outside this
capability.

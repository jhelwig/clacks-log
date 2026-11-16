# Early Windows Installer Implementation Plan

**Goal:** Deliver repeatable Windows 11 x64 development MSIs through Actions
artifacts and versioned MSIs through draft GitHub Releases. Native acceptance
must establish installation behavior and preservation of user state.

**Spec:** [Installer design](../specs/2026-11-04-windows-installer-design.md).

**Architecture:** Build the production desktop with Windows resources. Explicit
MSI YAML file entries name the executable and notices directly. Pinned
msi-builder owns package validation, construction and default payload read-back;
GitHub Actions owns ordering and artifact transport; GitHub CLI owns draft
creation and uploads. A focused Rust helper owns Cargo/tag identity and the
release eligibility decision using existing GitHub results. No staging manifest,
MSI parser, inspection checkpoint or release reconciliation protocol is needed.

**Tools:** Rust 1.99.1, native `x86_64-pc-windows-msvc`, msi-builder 1.2.0,
winresource 0.1.34, cargo-about 0.9.3, GitHub Actions and GitHub CLI.

Implementation is on `feat/windows-installer`. Native Windows construction,
actual delivery and the specification's native acceptance matrix remain pending.
Issue #45 remains open. Local portable verification does not establish those
outcomes or authorize publication.

## Constraints

- Windows 11 x64, per-user installation in
  `%LOCALAPPDATA%\Programs\Clacks Log`, current-user Start menu shortcut.
- Product: Clacks Log; owning entity: Technosorcery, LLC; executable:
  `clacks-log.exe`. Preserve existing application and storage identifiers.
- Source-owned Cargo version, stable upgrade GUID, downgrade rejection,
  standard upgrades and user-controlled files-in-use handling.
- Preserve workspaces, configuration, pending work, UI state and credentials.
- Manual development builds run Windows build/packaging checks only and deliver
  Actions artifacts. No Windows PR, ordinary push, merge or scheduled builds.
- Version tags reuse existing Linux/web results for the exact tagged commit.
  Select the newest applicable run and its latest attempt, including pending or
  unsuccessful results; never substitute an older pass or rerun those checks.
- Successful release builds create an unpublished draft GitHub Release. The
  user tests and publishes those same bytes. Website delivery is out of scope.
- Initial artifacts are unsigned and require no signing credentials. Preserve
  executable signing before MSI construction and MSI signing before final
  verification, checksumming and upload for future integration.
- Project-owned executable tooling is Rust. Use standard tool and platform
  commands for resource inspection, hashing, logged evidence and delivery.
- Preserve unrelated state and use isolated application paths for native tests.
- Review commit message files and physically wrap prose at 72 columns.

## Implementation

### 1. Application identity and source/release eligibility

- [x] Retain the user-selected pale blue CL artwork on the desktop palette.
  Preserve the original and conversion provenance under the spec assets and
  `lib/clacks-desktop/assets`. Use its PNG/ICO for the window and Windows PE
  resources through existing framework facilities and winresource.
- [x] Set application version resources from Cargo metadata and publisher fields
  to Technosorcery, LLC without changing storage or application identifiers.
- [x] Keep `bin/clacks-desktop-package` thin; behavior lives in
  `lib/clacks-desktop-packaging` with `prepare` and `gate` operations.
- [x] Read the `clacks-log` Cargo version. Accept `vVERSION` for releases and
  `vVERSION-rc.N` for prereleases, with numeric `N`. Pass the classification as
  the release step's input; both forms remain drafts. RC numbers do not change
  the numeric MSI version. Cargo and msi-builder own version-format validation.
- [x] Delegate commit/workflow filtering and latest result retrieval to
  `gh run list` and `gh run view`. Require completed, successful results for
  `verify (ubuntu-latest)` and `verify-web`; record commit, run, attempt and links.
- [x] Test tag/version disagreement and Clacks's decision to reject pending,
  unsuccessful or missing required verification. Do not test API filtering,
  serialization, fixed configuration values or upstream version validation.

### 2. Runtime notices and explicit MSI inputs

- [x] Pin cargo-about 0.9.3 with its CLI feature. Generate plain-text informational
  notices from the actual Windows desktop runtime graph using `about.toml` and
  `notices.hbs`; unresolved requirements fail harvesting.
- [x] Include project LICENSE and ownership NOTICE, generated dependency notices,
  and the unchanged pinned TrustedQSL license for the embedded catalog data.
  These installed notices do not create a license acceptance page.
- [x] Declare inputs and installed filenames directly in `msi.yaml`. Do not
  duplicate its file list in a staging manifest, inventory or custom copier.

### 3. MSI authoring

- [x] Author Windows x64 per-user installation with stable upgrade code,
  install location, application icon and Start menu shortcut.
- [x] Use standard upgrade handling with old-product removal after install
  initialization, reject downgrades and surface removal failures.
- [x] Configure `MSIRESTARTMANAGERCONTROL=DisableShutdown` and
  `REBOOT=ReallySuppress`; add no process-kill, automatic launch or user-data
  removal custom actions. These settings do not prove native behavior.
- [x] Verify the pinned packager archive digest before execution. Run its
  validator and build with default read-back enabled. Retain raw inspection
  output for diagnosis rather than parsing it into a second MSI validator.
- [ ] Build an actual native Windows installer and establish installation,
  upgrade, cancellation and preservation behavior using the spec's matrix.

### 4. Workflow and delivery

- [x] Add only manual dispatch and version-tag triggers. Run release eligibility
  before allocating Windows; manual builds do not require Linux/web evidence.
- [x] Build and check the helper, build the production Windows desktop with
  static CRT selection, smoke-check help/version and harvest notices.
- [x] Compare actual executable version resources with the Cargo version. Retain
  standard SDK dependency listing and packager inspection output. Native Windows
  testing must establish actual runtime availability.
- [x] Preserve future EXE/MSI signing insertion points. After final verification,
  use standard SHA-256 hashing and log/summary commands to record the exact
  artifact, source/build identity and reused verification links. No checkpoint
  file or additional artifact immutability protocol is introduced.
- [x] Upload only the MSI with producer-owned artifact names
  including run and attempt. Consumers use producer outputs so partial job
  reruns retrieve the correct artifacts. Propagate each command's failure.
- [x] Give contents-write permission only to release delivery. Use
  `gh release create --verify-tag --draft` with only the final MSI. GitHub-reported conflicts
  or partial uploads fail visibly; do not implement automatic asset reconciliation,
  deletion or replacement. Publication remains manual.
- [ ] Exercise manual and tag workflows once available on the default branch;
  verify actual artifacts and draft delivery without publishing.

## Verification and observability

Run focused Rust tests, Clippy, formatting, documentation, workflow validation
and pinned MSI authoring validation. Build/read back with disposable stand-in
files only to verify authoring portability; label it separately from production
Windows packaging. No fake MSI-table fixtures or tests of framework applying
supplied values are retained.

Use meaningful helper spans for metadata and eligibility decisions and normal
workflow step logs for build, notices, MSI construction, inspection and delivery.
Record revision/run/attempt and result links without tokens or response bodies.
No packaging metrics service or synthetic per-file metrics is required.

Execute the specification's native acceptance matrix on clean Windows 11 x64
using a disposable ordinary-user account and isolated app paths. Record exact
artifact hash/version/revision, Windows environment, signing status and observed
results. Native acceptance, signing/trust acceptance and unperformed checks stay
explicitly unresolved; do not infer them from portable tests or CI success.

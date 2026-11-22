# Windows desktop installers

The production application is packaged as a per-user Windows Installer (MSI)
package for Windows 11 x64. Clacks Log is owned and published by
**Technosorcery, LLC**. This installer does not establish completion of
all MVP workflows or signed public-distribution acceptance. The initial pipeline
is unsigned and needs no signing credentials. The [macOS guide](../macos/README.md)
covers the signed Apple Silicon DMG in the shared pipeline. A website mirror,
Store distribution and an updater are separate work.

## Development downloads

After this workflow is present on the default branch, open GitHub Actions →
**Desktop packages** → **Run workflow**, select the desired branch/tag, leave
platform set to **windows** and start it. The shared geographic preparation and
Windows packaging checks run. Select **both** to request macOS too, with its
signing prerequisites.
No PR, ordinary push,
merge or schedule starts a Windows build. Download the run's
`windows-installer-SHA-RUN-ATTEMPT` artifact and extract it before installation.
The ZIP contains only the MSI. The workflow summary records its SHA-256, version,
source commit, signing status, runner name and Windows version. Installer tables, executable
metadata, native imports and reused verification results remain in the job logs.

Development filenames include the unchanged Cargo version, revision and run
identity. Rebuilding the same version does not make it a newer MSI release.
Do not use development rebuilds as evidence of ordered version upgrades.
GitHub's normal artifact availability applies; no archival/retention service is
introduced. Preserve the candidate needed for an actual native comparison.

## Versioned releases

1. Commit a source version with three numeric fields within MSI bounds
   (major/minor at most 255, build at most 65535). Every distinct delivered
   release advances one of those fields. Packaging never edits Cargo versions.
2. Use a commit whose newest existing **Rust** push workflow and latest attempt
   passed both
   `verify (ubuntu-latest)` and `verify-web`. Tag that exact commit as `vVERSION`
   for a release or `vVERSION-rc.N` for a prerelease, where `N` is numeric,
   through the normal authorized release process.
3. The tag workflow reads those existing results before scheduling Windows.
   It does not rerun Linux/web checks. PR checks test a merge revision and are
   not substituted for an exact commit's push result. Missing, pending, failed,
   skipped or cancelled results fail eligibility with a diagnostic. Wait for
   existing checks to finish, then rerun this workflow; it never polls or starts
   them. Documentation-only commits excluded by existing path filters do not
   inherit an ancestor's pass.
4. Each platform uploads only its final package as an Actions artifact. After
   both Windows and macOS packaging succeed, one delivery job creates a
   **draft release** with that same MSI and the final Mac DMG attached. A
   failed Mac build retains the successful Windows artifact but prevents a
   combined draft. Only delivery has contents
   write permission; build and eligibility jobs cannot publish a release.
5. Download the draft's exact MSI, compare its hash with the workflow summary
   linked from the release notes, and perform the [native acceptance scenarios](#preservation-trust-and-acceptance).
   Record results against its hash and source revision in issue #45/PR.
   Publish the draft manually after testing;
   publication does not rebuild, re-sign or otherwise change the tested bytes.
   Apply the macOS guide's native acceptance to its exact DMG as well.

The release filename includes the complete tag without its leading `v`, for
example `clacks-log-0.1.0-rc.1-windows-x64.msi`. The validated tag supplies the
release step's prerelease input: false for `vVERSION`, true for `vVERSION-rc.N`.
Both forms create drafts for manual testing and publication. The MSI keeps the
numeric Cargo version; RC numbers do not establish newer MSI upgrade versions.
Standard GitHub CLI creates one draft with the MSI and DMG. Release notes
identify Windows unsigned status separately from macOS signing/notarization.
GitHub-reported conflicts or partial upload failures fail delivery; no automatic asset reconciliation,
replacement or deletion is performed. A partial upload remains unpublished.
Inspect any partial draft before retrying delivery. Rebuilt bytes need fresh
acceptance. Tags must not move.

Building this branch locally does not start CI, push a tag or create a release.
A new workflow cannot be manually dispatched until GitHub knows it on the
repository's default branch. No temporary Windows PR trigger is added to work
around that platform rule.

## Package policy and local build

The Windows job uses the dedicated `clacks-windows` runner. Cargo's standard
`CARGO_TARGET_DIR` points to a `target` directory beside `GITHUB_WORKSPACE`, outside
the source checkout.
Cargo retains compiled dependencies and tracks freshness across builds; normal
checkout cleaning leaves this directory intact. Cargo downloads remain in the
runner's managed `CARGO_HOME`. No cloud cache upload or download is involved.

Packaging scratch files and delivery outputs stay under the checkout's `target`
directory and are recreated for each run. The MSI compiler receives the compiled
EXE's absolute path through its standard `--var EXECUTABLE=...` option. For a
local build, supply that variable to both `validate` and `build`, resolving the
EXE under the target directory selected by Cargo (`cargo metadata` reports
`target_directory`). Keeping compiled artifacts does not establish installer or
application acceptance.

The application installs to `%LOCALAPPDATA%\Programs\Clacks Log` and adds one
current-user Start menu shortcut. No desktop shortcut, startup-at-login or
post-install launch is configured. The selected original icon and provenance
live in the spec's assets and `lib/clacks-desktop/assets/README.md`.

Use the Windows job in `.github/workflows/desktop-release.yml` as the executable build recipe on
native Windows: pinned Rust 1.99.1 and lockfile, release production EXE, pinned
notice generator, digest-verified msi-builder 1.2.0, default package read-back,
executable version comparison, native import listing, final checksums, then upload.
The focused helper's `--help` documents its arguments. Declarative MSI authoring
and its explicit file inputs are in `msi.yaml`. Source files are consumed directly;
the geographic build output is reader-checked separately below. MSI table
output is retained
for diagnosis without a custom table parser or duplicate policy checker.

The build selects static MSVC CRT linkage to reduce dependencies on separately
installed VC runtimes. The SDK import listing and clean Windows tests must
still establish actual runtime dependencies. If a permitted runtime DLL is
needed, add its explicit installer component and applicable
redistribution notices; do not install an elevated prerequisite package or
claim a developer/Server runner establishes Windows 11 availability.

The MSI compiler archive is `msi-builder_1.2.0_windows_amd64.zip` from upstream
v1.2.0, verified before execution against SHA-256
`d528277ae68218335318e8f335c01bdbfaa89882ffa53625251ad3e70c7735a7`.
`cargo-about` is pinned to 0.9.3 with its `cli` feature. Its config selects the
actual desktop Windows runtime graph, including conjunctive native/font/data
notices, rather than all workspace/development dependencies. The payload contains
project LICENSE and ownership NOTICE, generated full third-party license text and the pinned
TrustedQSL notice for the embedded LoTW model catalog, copied unchanged as
`THIRD-PARTY-DATA-NOTICES.txt`. Its applicability is recorded in
`lib/clacks-model/data/README.md`; this does not bundle or execute TrustedQSL
source code. Regeneration fails
on unresolved license requirements; review new requirements rather than hiding
them with exclusions. Plain-text templates preserve unescaped license text.

## Geographic build artifact

Both native jobs consume the same `desktop-geography-RUN-ATTEMPT` directory
artifact, including single-platform manual builds. The common Linux job restores
an exact cached output or automatically generates/validates data from the current
checkout on a miss. No geographic release or manual selection is required; see
[build inputs and cache behavior](../../../references/geography/BUILDING.md).
The generated `manifest-sha256.txt` travels outside the bundle and supplies
`CLACKS_GEOGRAPHY_MANIFEST_SHA256` before native compilation. Packaging checks
that expected identity, schema and payload integrity without repeating polygon,
index or inventory production acceptance.

The MSI's recursive directory source installs the common tree beneath `geography`
next to the executable, retaining declared notice paths. The pinned compiler
creates a standard component per file and re-extracts cabinets to compare each
file's SHA-256 to its source; normal `inspect` output remains in job logs.
Existing package evidence records geographic identity, manifest digest and
payload length. Geographic notices travel inside the bundle, including the
notice for catalog-derived county labels; the separate model catalog notice
remains `THIRD-PARTY-DATA-NOTICES.txt`.

To inspect an existing build output locally:

```sh
cargo run -p clacks-desktop-package --locked --offline -- geography-inspect \
  --manifest-sha256 "$(cat target/geography-build/manifest-sha256.txt)" \
  --bundle target/geography-build/geography --output target/geography-build/geography-evidence.json
```

## Preservation, trust and acceptance

Install/upgrade/repair/uninstall never open or migrate an application workspace.
Uninstall removes installer-owned files, shortcuts and registration while
retaining workspace data, configuration, UI state, pending work and protected
credentials. Reinstall as the same user reuses that retained state. The desktop
continues to own migrations, account workflows and graceful save draining.

Running-app handling is user-controlled: close normally, retry or cancel.
`MSIRESTARTMANAGERCONTROL=DisableShutdown` prevents automatic Restart Manager
shutdown; no timed kill custom action is included. `REBOOT=ReallySuppress`
prevents forced reboot. Record actual return codes, cancellation and reboot
requirements separately; these settings do not prove locked replacements never
require a reboot or that any particular cancellation stage exercised rollback.

Record download/install/launch warnings or blocks under default protections;
do not prescribe bypasses. Unsigned delivery does not establish the later signed,
prompt-free distribution gate. Windows Server CI/package checks do not establish
clean Windows 11 runtime or desktop acceptance.

Native testing uses a disposable ordinary-user account and isolated application
workspace/config/log paths, never personal state. Cover first launch/offline
CRUD/reopen; available connected enrollment/sync/credentials; running-app
install/upgrade/removal with admitted saves and full/quiet modes; two genuinely
increasing source-versioned installers; meaningful cancellation/environmental
failure; repeat maintenance/downgrade rejection; uninstall/reinstall preservation.
Confirm the installation scope, installed-program metadata, icon and Start
menu entry. After upgrade, confirm one product registration, updated files and
usable credentials. After cancellation or failure, check that the prior
installation and data remain usable and record where the operation stopped.
Record Windows edition/build, architecture, graphics/runtime environment, exact
MSI hash/version/revision/signing mode and actual scenario outcomes. Redact native
verbose logs before sharing. Keep #45 open until actual artifacts and required
native evidence exist; report blocked/unavailable checks distinctly.

## Future signing

Sign and verify the built EXE before the packager consumes it. Sign the built
MSI before package inspection, final verification and hashing. Add the eventual
provider through supported msi-builder or SignTool facilities at those stages;
no exportable-key requirement or provider choice is imposed now. Extend evidence
to record the verified signed mode when that integration is added.
Never silently downgrade a requested signed operation or run the signed-only
`msi-builder verify` command against an unsigned package. No signing service,
credentials, purchase or trust acceptance is established by this pipeline.

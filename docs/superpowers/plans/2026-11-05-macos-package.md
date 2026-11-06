# Early macOS Package Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. The user selects the execution method before implementation.

**Goal:** Deliver the current production desktop as a signed, notarized DMG
for Apple Silicon Macs running macOS 26 Tahoe and newer, through the existing
development-download and draft-release process.

**Architecture:** Extend the focused Rust packaging helper for truthful
platform metadata. Apple's tools assemble, sign, notarize and verify the app
and DMG; one GitHub Actions workflow owns independent platform builds and a
single combined draft delivery. The desktop retains ownership of application
state, credentials and migrations.

**Tech Stack:** Rust 1.99.1, egui/eframe production desktop,
`apple-native-keyring-store` 1.0.2 protected backend, native Apple tools,
Xcode 26.6 on `macos-26`, cargo-about 0.9.3, GitHub Actions and GitHub CLI.

**Spec:** [Approved macOS package design](../specs/2026-11-05-macos-package-design.md).

Status: revised plan approved by the user on November 5, 2026. Inline
implementation with an independent final review is authorized.

## Global Constraints

- Support macOS **26 Tahoe and newer on Apple Silicon only**; no continuing
  rolling-version support policy is selected.
- Target `aarch64-apple-darwin`; set `MACOSX_DEPLOYMENT_TARGET=26.0` and
  `LSMinimumSystemVersion` to `26.0`.
- Preserve `com.w7hex.clacks-log`, **Clacks Log.app**, `clacks-log`, existing
  credential services and application data paths; owner Technosorcery, LLC.
- A Developer ID Application certificate, matching profile and notarization
  authentication are prerequisites.
- Both development and tagged macOS deliveries require Developer ID signing,
  hardened runtime, notarization and a stapled DMG; no weaker fallback.
- Manual dispatch selects Windows, macOS or both, default Windows. Tags build
  both; ordinary pushes, merges, pull requests and schedules build neither.
- Immutable tags `vVERSION` / `vVERSION-rc.N` must match committed Cargo version
  and successful exact-commit existing Linux/web push checks; no new verifier run.
- One unpublished draft receives the exact final MSI and DMG. Only the user
  publishes after native testing; no byte mutation after final checksumming.
- Use native tools and existing Rust helpers; no generic dispatcher, installer
  service, profile parser, custom signer/notarization client or recovery queue.
- Every added test/assertion must identify a Clacks-owned decision or wiring
  mistake it detects. A custom name, ID, value or configuration passed to a
  dependency does not justify testing that dependency's documented behavior.
  Review fixed wiring directly when there is no application decision to test.
- Preserve data, credentials, pending work and configuration through replacement
  and removal; no package operation launches or migrates a workspace.
- Follow the user-owned development inventory before desktop/server execution.
  All acceptance uses explicit isolated workspace/config/log paths, disposable
  accounts and disposable test databases. Preserve unrelated state.
- Use worktree-placement before creating implementation isolation. Review
  actual commit-message files and hard-wrap ordinary prose at 72 columns;
  use `git commit --file`, preserving literal identifiers and URLs.
- This plan authorizes no account changes, credential creation, purchases,
  GitHub configuration changes, workflow dispatch, tag push or publication.
  Obtain concrete authorization for those external operations when ready.

## Review Focus

1. Platform mix-up: Mac metadata must never report Windows target/compiler or
   an unverified artifact as signed. Pin assertions in Task 1.
2. Distribution wiring: the real delivered app must use the selected identity,
   profile and native verification gate. Task 3 inspects those inputs/results;
   Task 4 proves installed operation, not Apple's rejection of bad fixtures.
3. Missing/slow signing services: review Clacks's input guard, native command
   invocation, failure propagation, upload ordering and cleanup in Task 3.
   Do not create tests that prove a native tool honors its timeout or rejects
   invalid authentication.
4. Release graph mistakes: a skipped/failed platform must prevent a tagged
   combined draft; a manual single-platform build must remain independent.
   Task 3 validates syntax and exercises available authorized dispatch cases.
5. User state and trust: an installed exact artifact must work with quarantine
   and protected credentials and preserve pending work across replacement.
   Task 4 tests the installed product; metadata checks cannot substitute.

## Verification responsibility boundaries

| Check | Clacks responsibility and concrete mistake it detects |
| --- | --- |
| Platform metadata unit scenario, Task 1 | Clacks selects a different set of metadata for each supported platform. Passing the Mac variant but executing the Windows branch, or claiming verified signing during preparation, is a Clacks mistake. Test that selection; do not test Serde serialization or copy-through fields. |
| Existing tag/eligibility unit scenarios, Task 1 | Clacks compares source versions/tags and decides which job outcomes qualify. Retain that decision coverage; do not duplicate it per platform or test GitHub filtering/order or Clap parsing. |
| Resource lint/build and assembled-artifact inspection, Task 2 | Check Clacks's authored inputs and complete payload for wrong source files, stale templates, omitted resources or unsupported dependencies. Do not unit-test native tools applying their documented options, run an icon conversion round trip, or assert framework constants. |
| Workflow review and actual authorized dispatch, Task 3 | Check Clacks's event/input/job conditions, selected artifact paths, permission placement, native command invocation and delivery order. Do not add tests of Actions dependency scheduling, environment-ref enforcement, GitHub CLI release semantics or shell fail-fast behavior. |
| Native signing/notary gates on real candidates, Task 3 | These remain production verification steps for what Clacks actually assembled and signed. No deliberately invalid profiles, development-signed fixtures or broken authentication/network are needed to prove Apple's documented rejection or timeout behavior. |
| Exact downloaded package and installed app, Task 4 | Prove the Clacks pipeline delivered the inspected bytes and the installed Clacks app uses its resources, state and credentials correctly. Do not add generic hashing, Keychain create/read/update/delete, notarization/Gatekeeper or Finder test suites. |

Source review establishes invocation/configuration choices; native acceptance
establishes the installed application's behavior. Neither is a dependency
conformance test. Removing a proposed dependency test does not leave an
unfulfilled acceptance gate for that test. Preserve the spec's real-artifact
verification and observations without manufacturing new scenarios to replace it.

## File map and task interfaces

| Files | Responsibility / task |
| --- | --- |
| `lib/clacks-desktop-packaging/src/metadata.rs`, `metadata/tests.rs`, `cli.rs` | Explicit platform metadata and CLI selection; Task 1. Existing `ci.rs`/tests remain the release eligibility owner. |
| `bin/clacks-log/macos/Info.plist` | Stable bundle identity, minimum version and icon resource name; Task 2. Version values are supplied to a build copy. |
| `lib/clacks-desktop/assets/clacks-log.icns`, `assets/README.md` | Derived selected artwork and reproducible native conversion recipe; Task 2. |
| `deploy/desktop/macos/about.toml`, `entitlements.plist`, `README.md` | macOS runtime notice graph, entitlement template and operator build/install/acceptance instructions; Tasks 2–4. Reuse existing notice text template. |
| `.github/workflows/windows-installer.yml` → `desktop-release.yml` | Rename and extend the existing pipeline, retain the Windows recipe, add native macOS and shared draft delivery; Task 3. |
| `deploy/desktop/windows/README.md`, `docs/desktop.md`, `lib/clacks-desktop/README.md` | Update workflow instructions and describe macOS packaging versus local development host; Tasks 3–4. |

Task 1's `prepare --platform windows-x64|macos-arm64` emits metadata consumed
by both jobs. Task 2's files are the explicit macOS bundle inputs. Task 3
produces final artifact identities/hashes and draft downloads. Task 4 consumes
those exact downloads without rebuilding them. No task adds application schema
or credential-backend changes.

### Task 1: Make source metadata explicitly platform-aware

**Files:** Modify `lib/clacks-desktop-packaging/src/metadata.rs`,
`lib/clacks-desktop-packaging/src/metadata/tests.rs`,
`lib/clacks-desktop-packaging/src/cli.rs` and the two existing workflow
`prepare` invocations; preserve `ci.rs` and its existing decision tests.

**Interfaces:** Add crate-private `PackagePlatform::{WindowsX64, MacosArm64}`
with Clap value names `windows-x64` / `macos-arm64`. Change
`prepare(input: &serde_json::Value, revision: &str, tag: Option<&str>,
platform: PackagePlatform) -> anyhow::Result<BuildMetadata>`. CLI `prepare`
requires `--platform`. Retain the output field names; change `tool_version`
to `Option<String>` because Apple tools have multiple separately recorded
versions. Windows emits `Some("1.2.0")`, target `x86_64-pc-windows-msvc`,
architecture `x64`, signing mode `unsigned`; Mac emits `None`, target
`aarch64-apple-darwin`, architecture `arm64`, signing mode `not-established`.
The final Mac job records verified signing/notarization separately.

- [ ] Add `platform_metadata_does_not_claim_distribution_before_verification`
  beside existing metadata tests. Exercise both branches in one scenario and
  assert their selected target, architecture, tool-version and preparation
  signing mode. This detects Clacks selecting the wrong platform metadata.
  Do not add version/revision copy-through or Serde `None`/null assertions.
  This justification depends on Clacks owning that selection. If implementation
  instead delegates it to a library's documented platform mapping, reassess
  the test; Clacks-specific output strings alone do not justify retaining it.

  ```rust
  let mac = prepare(&input, &revision, None, PackagePlatform::MacosArm64).unwrap();
  assert_eq!(mac.target, "aarch64-apple-darwin");
  assert_eq!(mac.architecture, "arm64");
  assert_eq!(mac.signing_mode, "not-established");
  assert_eq!(mac.tool_version, None);
  ```

- [ ] Update existing release/RC tests only to pass the new platform argument;
  keep their current source/tag comparison and prerelease-decision assertions.
  That logic remains shared, so do not multiply it across platform variants.
  Keep existing eligibility tests unchanged; do not add Clap/Serde tests.
- [ ] Run `cargo test -p clacks-desktop-packaging --locked --offline` and
  confirm the new calls/assertions fail against the old platformless API.
- [ ] Implement the interfaces and required CLI argument. Keep common version,
  revision and tag validation in `prepare`; match platform only where choosing
  platform fields. Add platform to the existing operation span/info outcome,
  without payload dumps or another subscriber. Update existing workflow calls
  with `--platform windows-x64`, preserving current operation until Task 3.
- [ ] Run the same tests, then
  `cargo clippy -p clacks-desktop-packaging -p clacks-desktop-package --all-targets --locked --offline -- -D warnings`
  and `cargo fmt --all --check`; require successful exits. Existing eligibility
  tests must still reject missing/pending/unsuccessful exact-commit results.
- [ ] Review the focused diff and commit with a reviewed, wrapped message file.
  Do not claim a signed artifact from source metadata or these portable tests.

### Task 2: Define the native bundle payload and resources

**Files:** Modify `bin/clacks-log/macos/Info.plist` and
`lib/clacks-desktop/assets/README.md`; create
`lib/clacks-desktop/assets/clacks-log.icns`,
`deploy/desktop/macos/about.toml`, `entitlements.plist`, `README.md`.

**Interfaces:** The bundle input template keeps its existing identifier/name/
executable/ownership and adds minimum version `26.0` plus
`CFBundleIconFile = clacks-log.icns`. Do not commit release-specific versions.
The entitlement template has string placeholders for
`com.apple.application-identifier`, `com.apple.developer.team-identifier`,
and a single `keychain-access-groups` string; populate a temporary copy using
PlistBuddy. No signing identity/private key/profile is committed.

- [ ] Define an explicit bundle file list in the operator recipe: production
  executable → `Contents/MacOS/clacks-log`; populated plist →
  `Contents/Info.plist`; icon/LICENSE/NOTICE/generated third-party notices/
  existing data notices → named files in `Contents/Resources`; authorized
  profile → `Contents/embedded.provisionprofile` only in the signed build.
- [ ] Set the minimum version/icon fields. Derive ICNS from the existing original
  PNG with `sips` and `iconutil`, using a temporary standard `.iconset` with
  16/32/128/256/512 and corresponding doubled-resolution PNGs. Record exact
  conversion commands in the artwork guide; preserve original provenance.
  Do not add an image generator or release-time conversion dependency.
- [ ] Add macOS cargo-about configuration selecting only
  `aarch64-apple-darwin` runtime dependencies. Start with existing license
  choices; retain actual required native/font/data notices and fail unresolved
  requirements. Reuse `deploy/desktop/windows/notices.hbs` and unchanged
  `lib/clacks-model/data/README.md` notice applicability rather than copy text.
- [ ] Write the local build/assembly recipe using standard native commands,
  `ditto`, PlistBuddy and explicit copy inputs. Use a build directory under
  `target/macos-package`; application versions come from Task 1 metadata.
  Document the complete release recipe, not a second project shell runner.
- [ ] Validate authored plists with `plutil -lint` and inspect the resulting
  icon artwork for accidental source/authoring changes; inspect its actual
  native appearance in Task 4. Do not add a converter round-trip test or an
  assertion that `sips`/`iconutil` uses the dimensions it was passed.
  Run pinned cargo-about 0.9.3 against the Mac target configuration and require
  successful notice generation. These are resource/integration checks, not
  unit tests asserting fixed configuration values.
- [ ] Build with
  `MACOSX_DEPLOYMENT_TARGET=26.0 SQLX_OFFLINE=true LEPTOS_OUTPUT_NAME=clacks-web cargo build -p clacks-log --release --target aarch64-apple-darwin --locked --offline`;
  run executable `--help` / `--version` only. Inspect a temporary assembled
  bundle using `plutil`, `file`, `xcrun vtool -show-build` and `otool -L`.
  Require Arm64, source version, executable minimum `26.0`, no bundled library
  requiring a newer system, expected resources, no
  developer/Homebrew dependency paths; resolve any actual non-system dependency
  with permitted explicit bundling and applicable notices. Never open a workspace.
- [ ] Record unavailable native/tool checks honestly if host/tool inputs are
  missing; no unsigned bundle is a deliverable. Commit the reviewed resource
  and recipe changes with a wrapped message file.

### Task 3: Integrate signing, notarization and shared release delivery

**Files:** Rename `.github/workflows/windows-installer.yml` to
`.github/workflows/desktop-release.yml`; update both platform guides,
`docs/desktop.md` packaging links and `lib/clacks-desktop/README.md`.

**Interfaces:** Workflow display name **Desktop packages**; dispatch input
`platform` is a required choice with `windows`, `macos`, `both`, default
`windows`. Jobs: `eligibility`, `windows`, `macos`, `draft`. Each platform job
outputs `sha`, `prerelease`, `package`, `artifact`; final job attaches both
named packages. Keep local outputs under `target/windows-package` and
`target/macos-package`. Artifact names include platform, exact revision,
run and attempt. Use environment `macos-signing` only for the Mac job.

- [ ] Rename/extend in one change: retain `workflow_dispatch` and pushed `v*`
  tags only, existing pinned action revisions, default read permissions and
  the Windows recipe/signing insertion points. Update Windows `msi` output
  references to shared `package` without changing MSI identity or filename.
- [ ] Keep eligibility tag-only and shared. Preserve `prepare` source/tag
  validation (preflight can select `windows-x64`); produce source-validation
  metadata, not Mac signing evidence. Keep exact-commit `gate` unchanged.
  Both platform jobs use `needs: eligibility` with `always()` and an explicit
  event/input/result condition so skipped eligibility permits manual builds,
  while tags require its success. Each job resolves/checks its own source SHA.
- [ ] Configure `macos` on `macos-26` with
  `DEVELOPER_DIR=/Applications/Xcode_26.6.app/Contents/Developer`,
  `MACOSX_DEPLOYMENT_TARGET=26.0`, `SQLX_OFFLINE=true`,
  `LEPTOS_OUTPUT_NAME=clacks-web`. Fail if the selected Xcode path is absent;
  record `xcodebuild -version`, SDK and image identity. Key caches by target,
  deployment baseline and selected tool version. Fetch locked dependencies,
  then build/test/lint the focused helper and release application offline.
- [ ] Wire Task 2's recipe into native workflow steps. Generate notices with the
  existing pinned cargo-about installation pattern; smoke-check help/version;
  populate the copied plist from `prepare --platform macos-arm64` output.
  Do not download an unrelated packaging framework.
- [ ] Define required environment inputs:
  `APPLE_SIGNING_P12_BASE64`, `APPLE_SIGNING_P12_PASSWORD`,
  `APPLE_PROVISION_PROFILE_BASE64`, `APPLE_ID`, `APPLE_TEAM_ID`,
  `APPLE_APP_SPECIFIC_PASSWORD`; use secrets for sensitive values. A fresh
  ephemeral keychain password is generated/masked per run. The proposed
  notarization authentication is Apple ID + app-specific password for the
  configured signing account; no additional authentication service is required.
  Document provisioning/setup for the owner; do not change external settings.
- [ ] Check all required inputs before import; empty inputs fail with field
  names only. Import into an unlocked temporary keychain following GitHub's
  native pattern. Decode the profile with `security cms`, extract prefix/team
  with `plutil`, and populate the entitlement copy. Require the expected team
  and derived app identity, preserve the default access group, and use native
  distribution validation for profile entitlement authorization. Do not add
  a wildcard/profile parser. Private material lives only under runner temp.
- [ ] Sign nested code, if any, then the app using the selected Developer ID
  identity, `--timestamp` and `--options runtime`. Verify strict signatures;
  inspect signed entitlements and profile. Create a read-only UDZO image with
  the finished app and Applications symlink. Sign the image using stable ID
  `com.w7hex.clacks-log.dmg` and Developer ID Application, not Installer.
- [ ] Store notarization authentication in the temporary keychain with
  `xcrun notarytool store-credentials`; call `submit --wait --timeout 30m`
  with JSON output redirected to a temporary file. Require process success
  and `status == Accepted`; record submission ID and retrieve/redact logs.
  A timeout/service failure is a failed delivery even if Apple continues
  processing. No unattended resubmission loop or completion watcher.
- [ ] Staple/validate the DMG, verify its integrity and signature, mount read-only
  without opening Finder and verify the inner app. Assess the outer image with
  `spctl -a -t open --context context:primary-signature -v` and the mounted app
  with `spctl -a -t exec -vv`. Run that final mounted executable's `--help` and
  `--version` using explicit temporary data paths; signed launch also exercises
  restricted-entitlement authorization before any workspace is opened. Actual
  protected credential use remains Task 4. Do not gate the inner app with
  `syspolicy_check distribution`: it incorrectly reports a fatal missing app
  ticket when only the outer DMG is stapled. Unmount via unconditional cleanup; do not
  mutate the inner app or add a second notarization round. Fail before upload
  if any required check fails.
- [ ] Hash final bytes only now. Record truthful final signing/notary status,
  source/version, profile identity/expiry, submission ID, tools/image/run/attempt
  and SHA-256 in summary/logs. Upload only the final DMG. Add unconditional
  deletion of the imported keychain, key export, profile and authentication
  temporary files on success, failure and cancellation. No secret-bearing cache.
- [ ] Make `draft` tag-only with explicit successful `windows` and `macos`
  dependencies, contents write permission only there, and per-tag concurrency
  without cancellation. Download the exact named artifacts and call
  `gh release create --verify-tag --draft --latest=false` with both files and
  validated prerelease status. Notes distinguish unsigned Windows from signed/
  notarized macOS and link checksums/evidence. Existing releases/partial upload
  conflicts fail; no clobber/reconciliation/publication. Manual dispatch never
  reaches this job.
- [ ] Run `actionlint .github/workflows/desktop-release.yml` when available;
  require zero diagnostics, and report unavailable tool checks distinctly.
  Re-run Task 1's focused tests/Clippy and formatting checks if helper code changed.
  Review the trigger/permission/needs conditions directly; do not create YAML
  substring tests or a fake release coordinator to test Actions semantics.
- [ ] With separate authorization and trusted environment configuration,
  exercise manual Windows-only, Mac-only and both dispatches. Require only the
  selected platform jobs, no draft, and final package/summary agreement. This
  checks Clacks's event/input conditions and artifact wiring. Review trusted-ref
  environment settings and job access directly; do not test that GitHub honors
  an environment's configured restrictions or create a test-only environment.
- [ ] Review the missing-input guard and command-failure/upload/cleanup paths.
  If a real run encounters missing inputs or an external failure, record
  Clacks's resulting diagnostic, stopped delivery and cleanup. Do not create
  invalid signed bundles, bad authentication or a network outage to prove a
  native tool rejects them; do not wait out `notarytool` to test its timeout.
  Do not build command stubs or additional production failure-handling machinery
  merely to manufacture coverage of these standard integrations.
- [ ] For an authorized future candidate tag, verify failed/skipped platform
  results prevent the combined draft and successful results create one draft
  with both exact files. If a real failure case is unavailable, source review
  establishes Clacks's job condition, not runtime evidence of that case. Do not
  manufacture extra tags/failures to test Actions enforcing `needs`. Never push
  a disposable release tag or publish software without authorization.
- [ ] Finish guides with exact names/inputs, trusted-ref environment setup,
  local reproduction, missing-credential diagnostics, timeout/manual retry,
  partial-draft handling, mixed signing status and unchanged publication rules.
  Remove obsolete Windows-only workflow references, preserving Windows policy.
  Review and commit with a wrapped message file.

### Task 4: Verify exact installed artifacts and preservation

**Files:** Finish `deploy/desktop/macos/README.md` and relevant desktop guide
links. Record artifact-specific evidence in #80/its pull request when authorized;
do not add a second repository delivery-status ledger.

**Interfaces:** Consume Task 3's exact downloaded DMG and hash/version/revision.
Use two genuinely increasing source-version artifacts for upgrades, and the
spec's complete native acceptance matrix. No rebuild or test harness can stand
in for the production artifact under test.

- [ ] Document ordinary Finder installation, normal quit before replacement,
  bundle removal/reinstall, retained data/credentials and unsupported downgrade
  rollback. Include current pre-alpha limitations and absence of an updater.
  Do not describe macOS replacement as MSI transactional rollback.
- [ ] Arrange clean disposable ordinary-user environments on Apple Silicon
  macOS 26.0 and the current newer major release. If unavailable, identify
  exact evidence gaps. Use user-owned machine-local instructions before server/
  desktop execution, without copying their configuration/secrets into guides.
- [ ] Download the actual Actions/draft DMG through the intended browser path,
  compare final hash, retain normal quarantine and perform first mount/copy/
  eject/launch. Use disposable user state and explicit isolated data paths;
  verify the running executable/arguments before UI automation attaches.
- [ ] Repeat first-install trust with networking unavailable from a clean
  environment. Record ordinary consent separately from blocked/untrusted
  results. Do not disable Gatekeeper or remove quarantine to pass.
- [ ] Exercise offline create/edit/delete, save/reopen, native menus and orderly
  shutdown, input/scaling and icon display. Exercise available enrollment/
  browser-return/sync and directory workflows with disposable accounts; prove
  protected secrets survive restart and neither fallback nor logs expose them.
- [ ] Close normally with saved data, pending offline edits and credentials,
  replace with the genuinely newer artifact and reopen. Verify retained facts,
  configuration, pending work and protected credential access/sync. Observe
  Finder cancellation/permission/running-app conflict using real conditions;
  record observations required by the spec without asserting undocumented
  Finder rollback or testing its documented copy/permission semantics. No force
  kill, schema downgrade or manufactured application failpoint.
- [ ] Remove and reinstall the bundle as the same user/signing identity; verify
  the reinstalled Clacks app reopens the retained workspace, resumes pending
  work and accesses its existing credentials. These catch changed application
  identity, access-group or startup wiring; do not assert that Finder deleting
  a bundle leaves unrelated directories intact. Reinspect installed version,
  profile/entitlements/signatures/resources against the downloaded artifact.
- [ ] If running existing credential integration tests as additional evidence,
  build the actual desktop library test executable, copy it into an equivalent
  signed/profile-authorized test bundle and run bounded native filters with
  disposable databases per the existing guide. Never ship it or treat it as
  installed-production acceptance; preserve bundle while the process runs.
  Select existing application scenarios such as
  `enrollment::tests::saved_native_credential_resumes_the_actual_client`,
  which exercises Clacks's enrollment/persistence/resumption wiring. Add no
  generic Keychain set/get round trips or assertions of backend defaults that
  are supplied directly by the library.
- [ ] Record each scenario as passed, failed, blocked or untested with exact
  native environment, artifacts and observed trust behavior. Redact secrets;
  ordinary GitHub artifacts/notes suffice. Keep #80 open until its installed
  artifact evidence is complete; leave #70/#37/#38's final workflow acceptance
  with those issues. Do not modify closed #79 or publish a release automatically.
- [ ] Run documentation link/whitespace checks and review the complete change
  against the approved spec, including telemetry fields and secret cleanup.
  Commit only reviewed changes using a wrapped message file. Report native
  acceptance limits independently from passing source/build checks.

## Plan self-review and handoff

Coverage: platform/identity/metadata and helper diagnostics → Task 1;
payload/resources/notices/minimum-version inspection → Task 2;
signing/profile/notarization/secrets, byte ordering, release graph and workflow
diagnostics → Task 3; installation/trust/current workflows/preservation and
exact native evidence → Task 4. Each Review Focus condition has application
decision coverage, targeted source review or real-artifact acceptance as
specified in the responsibility table; no condition mandates a dependency
conformance test. Routine operator-guide updates travel with the feature they
document.

Existing portable eligibility tests remain authoritative for Clacks's release
decision; native tool checks and real dispatches establish their distinct
integration claims. No checks in this plan are reported as executed. Missing
credentials or clean environments block the relevant artifact/evidence steps,
not unrelated implementation. Spec approval does not mean release acceptance.

After plan review, choose inline implementation or subagent-driven execution.
Inline is recommended because these four tasks share a small helper and one
workflow; finish with a fresh whole-change review before integration. Creating
an isolated implementation worktree, committing/pushing changes, configuring
external signing inputs and dispatching builds must follow the authorization
and worktree-placement rules in force at execution time.

References: [Apple packaging](https://developer.apple.com/documentation/xcode/packaging-mac-software-for-distribution),
[Apple notarization](https://developer.apple.com/documentation/security/customizing-the-notarization-workflow),
[GitHub signing](https://docs.github.com/en/actions/how-tos/deploy/deploy-to-third-party-platforms/sign-xcode-applications),
[runner tool inventory](https://github.com/actions/runner-images/blob/main/images/macos/macos-26-arm64-Readme.md).
Apple's [launch diagnostics](https://developer.apple.com/forums/thread/706442)
document both restricted-entitlement execution failures and the inner-app
missing-ticket diagnostic. `notarytool submit --help` documents
timeout/authentication behavior. Verify the
selected versions remain available when implementing; do not silently switch
to `macos-latest` or another Xcode version.

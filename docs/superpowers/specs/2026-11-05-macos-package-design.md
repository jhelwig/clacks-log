# Early macOS desktop package

Status: approved by the user on November 5, 2026. This specification records
the discussion of [issue #46][issue-80]. Prepare and review its implementation
plan before coding. Approval does not authorize Apple account changes,
credential creation, purchases or publication. No application artifact or
native acceptance is established by this document.

## Intent and authority

Deliver the production Clacks Log functionality available at packaging time
as a native macOS application in a downloadable disk image (DMG). Extend the
Windows release pipeline established for [#45][issue-79], preserving its
distribution rules and exact-artifact acceptance. Do not wait for complete
MVP workflow readiness. GitHub owns delivery status and completion.

The issue requires stable application identity, runtime assets and notices,
the existing protected credential integration, repeatable builds and native
verification of installed artifacts. D391 owns the Data Protection Keychain
choice; D392 owns early desktop release sequencing. [#43][issue-70] retains
complete integrated MVP and final macOS release acceptance. Completing signing
and notarization here does not establish that later workflow acceptance.

### Decisions from the user

- Support macOS **26 Tahoe and newer on Apple Silicon only**. This is the
  initial baseline, not a rolling support policy. How many older versions to
  retain as macOS evolves remains undecided.
- Distribute a DMG containing **Clacks Log.app**, copied into Applications.
- Apply the Windows distribution rules: manually requested development
  downloads through GitHub Actions; eligible immutable version tags create
  draft GitHub Releases; native testing precedes manual publication; publish
  the tested bytes without rebuilding them.
- Developer ID signing and notarization require an Apple Developer Program
  membership and a distribution identity. Account changes require separate
  authorization. Technosorcery, LLC remains the application owner in existing
  notices and application metadata.

### Engineering choices approved with this specification

Use Apple's native bundle, signing and disk-image tools, Developer ID
Application signing, hardened runtime, notarization and stapling. Apply the
same complete macOS packaging stages to development and versioned downloads;
do not deliver a credential-disabled or unsigned substitute when inputs are
missing. The user's acceptance of the signing name is not evidence that these
signing inputs exist.

Keep one release workflow with independent Windows and macOS build jobs and
one draft-release delivery job. Reuse the focused Rust packaging helper for
Clacks-specific metadata and eligibility; extend its Windows-specific fields
without creating another release framework. These are implementation choices
selected by this approved design, not independent product requirements.

## Approaches considered

| Approach | Fit and tradeoff | Selection |
| --- | --- | --- |
| Native application inside a DMG | Explicit open-and-copy installation; Apple supplies container creation, signing and notarization facilities. Requires macOS signing inputs and native validation. | User selected. |
| Native application inside a ZIP archive | Supported direct-distribution container with fewer presentation steps; leaves extraction and placement to the user. | Discussed; user preferred DMG. |

Apple documents `hdiutil` for disk images and `codesign` for their signatures.
The current application is one bundle; there is no demonstrated need for an
installer package, privileged helper or third-party packaging framework.
Workflow configuration invokes the supported tools. Any substantive
project-owned executable logic belongs in Rust. See [Apple packaging][packaging].

No updater, tray behavior, Linux desktop release, Intel/universal artifact,
Mac App Store submission, website mirror or hosted launch is added. This work
does not alter synchronization, enrollment, credential backends or storage
schemas. Packaging must not open or migrate an application workspace.

## Existing components and integration gap

The production executable is `bin/clacks-log`; application behavior lives in
`lib/clacks-desktop`. The existing
[`Info.plist`](../../../bin/clacks-log/macos/Info.plist) defines
`com.w7hex.clacks-log`, the display name and executable name. The desktop guide
already describes a signed/profile-authorized local development bundle; that
host is not a distribution pipeline.

`credentials::entry` uses `apple-native-keyring-store` 1.0.2's `protected`
backend. Its existing defaults supply app-scoped local storage, access while
unlocked, no user-presence requirement and no cloud synchronization. Packaging
must authorize that existing behavior rather than introduce another store or
secret-migration mechanism. See the [desktop guide][desktop-guide] and
[Apple keychain guidance][keychains].

The existing Windows workflow owns tag/development triggers, exact-commit
Linux/web eligibility, native packaging and draft release creation. Its
`clacks-desktop-package` helper currently emits Windows target, architecture,
tool version and unsigned status. Reuse its source version/tag and eligibility
logic while making platform metadata truthful for macOS. The actual gaps are
macOS bundle assembly, platform metadata, signing/profile integration, DMG
delivery and native evidence. No new application state, queue, inventory
database or release-recovery service is required.

## Application identity and payload

- Keep `CFBundleIdentifier = com.w7hex.clacks-log`, bundle/display name
  **Clacks Log**, executable `Contents/MacOS/clacks-log`, credential service
  namespaces and existing workspace discovery unchanged.
- Set `LSMinimumSystemVersion` to `26.0`. Build the production executable for
  `aarch64-apple-darwin`, with `MACOSX_DEPLOYMENT_TARGET=26.0` applied to Rust
  and native dependency builds. Inspect the resulting Mach-O architecture,
  deployment information and dependencies; metadata alone does not prove
  runtime support. Do not use an Intel executable through Rosetta.
- Obtain the numeric application version from committed Cargo metadata.
  Populate `CFBundleShortVersionString` and `CFBundleVersion` consistently
  from it. Retain the existing shared release tag forms `vVERSION` and
  `vVERSION-rc.N`, where `N` is numeric. Packaging never rewrites Cargo
  versions or introduces an independent version counter. Distinct delivered
  releases advance the source version under the Windows rules.
- Reuse the selected original icon and its provenance in
  [the artwork guide][artwork]. Derive an `.icns` resource with supported
  macOS image/icon tools; set the bundle icon metadata and inspect native
  small-size rendering. Do not redraw or generate replacement artwork.
- Include project LICENSE, ownership NOTICE, full applicable third-party
  notices and embedded-data notices under `Contents/Resources`. Reuse the
  pinned notice generator and templates, selecting the actual macOS desktop
  runtime graph rather than copying the Windows target dependency list.
  Unresolved license requirements fail packaging.
- Include only production runtime resources and permitted non-system dynamic
  libraries actually needed by the delivered build. Inspect native imports
  and verify on a clean supported Mac. No developer-home/Homebrew paths,
  debug symbols, test executables, development configuration, private signing
  keys or reference source trees belong in the delivered payload. The embedded
  provisioning profile is an intentional public bundle resource.

The DMG contains the finished application and a conventional Applications
shortcut. Keep its presentation simple, without custom Finder-layout automation
or a new background illustration. Document installation into `/Applications`
or the user's `~/Applications` folder using ordinary Finder permissions. No
privileged installer, automatic application launch or startup-at-login action
is introduced. The implementation owns the explicit source-file list under
`deploy/desktop/macos/`; do not package broad repository or home-directory globs.

## Signing, profile authorization and prerequisites

Development signing, entitlement authorization and direct-distribution trust
are distinct. An Apple Development certificate alone does not establish the
intended downloadable distribution.

Use a **Developer ID Application** certificate and matching private key for
the app and disk image. An Apple Distribution certificate is a different
certificate type; Developer ID Installer applies to installer packages, not
this DMG.
See [Apple certificate types][certificates] and [Developer ID setup][developer-id].

Embed a Developer ID provisioning profile authorizing this application's
identifier and default Keychain access group. Derive the application identifier
prefix and team identifier from the actual profile; do not assume the prefix
equals the team ID. Verify that the profile authorizes the requested signed
entitlements, including `com.apple.application-identifier`,
`com.apple.developer.team-identifier` and `keychain-access-groups`. Keep the
default app-specific group stable across delivered upgrades. Do not enable
iCloud, cross-app credential sharing, sandboxing or unrelated capabilities.
See [Apple provisioning profiles][profiles].

The owner must supply or authorize creation of the Developer ID identity,
matching profile and notarization authentication before signed artifacts can
be produced. Verify actual account capabilities, profile contents, certificate
validity and private-key availability during setup. Do not infer readiness
from paid membership or the development certificate. These prerequisites do
not prevent implementation planning, but missing inputs must stop artifact
delivery clearly. Never fall back to ad hoc or development signing.

For hosted automation, use GitHub's documented temporary-keychain pattern:
encrypted signing inputs, narrowly exposed to the macOS signing job, imported
into an ephemeral keychain and removed with temporary profile/authentication
files in unconditional cleanup. A password-protected signing identity export
is the proposed standard integration, not a requirement to change the owner's
membership. Select supported notarization authentication during setup; do not
create a custom credential broker. See [GitHub signing guidance][github-signing].

Only trusted release/development refs may access signing secrets. Use a
protected GitHub environment and appropriate ref restrictions. Pull requests
must not receive signing secrets, and manual branch selection must not bypass
those restrictions. Do not cache or upload keys, password files or keychains;
do not echo secrets or enable shell tracing around them. Account/environment
configuration changes require separate authorization when performed.

## Build, notarization and final artifact order

Use the pinned Rust 1.99.1 toolchain and lockfile. Fetch locked dependencies,
then build offline in release mode with `SQLX_OFFLINE=true` and the existing
`LEPTOS_OUTPUT_NAME=clacks-web` setting. Use a versioned native Apple Silicon
runner, initially `macos-26`, and explicitly select/record its Xcode version.
Record the runner image version because a versioned label is not an immutable
image. GitHub documents that [the standard macos-26 runner is Arm64][runner].

The ordered stages are:

1. Resolve the exact source revision, Cargo version and requested platform.
   For version tags, reuse successful existing Linux/web results as below.
2. Build the production desktop and focused packaging helper. Run targeted
   helper checks and desktop command-line smoke checks that do not open a
   workspace. Generate notices and assemble the complete bundle.
3. Inspect bundle metadata, architecture, deployment information and native
   dependencies. Embed the verified profile and entitlements. Sign any bundled
   nested code before the outer application, with Developer ID, a secure
   timestamp and hardened runtime. Do not use `codesign --deep` as an assembly
   or signing substitute, or add broad runtime exceptions without evidence.
4. Verify application signatures and signed entitlement/profile agreement.
   Create a read-only compressed disk image with `hdiutil`, preserving bundle
   symlinks with Apple's supported copy facilities. Sign it with Developer ID
   Application and a distinct stable identifier, proposed as
   `com.w7hex.clacks-log.dmg`.
5. Submit the signed DMG through `xcrun notarytool`. Require an accepted result;
   retain its submission identifier and inspect diagnostics, including warnings.
   Invalid, unavailable or timed-out results stop delivery. Use supported
   bounded waiting; no background completion watcher or custom retry queue.
6. Staple and validate the ticket on the DMG with `xcrun stapler`. Verify image
   integrity, its signature and the mounted application's signature and
   identity after this final mutation. Keep the submitted image's payload
   unchanged. Do not require a separately stapled inner app in this one-pass
   DMG path; verify offline trust for the actual mounted/copied distribution.
7. Compute SHA-256 only after signing and stapling finish, then upload the exact
   DMG. Signing/notarization/packaging failures must not emit a successful
   deliverable or silently produce a weaker signing mode.

Apple supplies these facilities; the application must wire them together and
validate Clacks's identity and payload. Do not implement a signer, provisioning
profile generator or notarization client. See [notarization requirements][notarize]
and [the supported custom workflow][notary-workflow]. The resulting final bytes
are repeatable build outputs, not a claim of bit-for-bit reproducibility across
signing timestamps, runner updates or notarization submissions.

## Shared release and development delivery

Extend the existing workflow into one desktop release workflow. The concrete
workflow filename/display-name transition belongs to implementation; update
the operator guide and references together and remove the old tag producer so
two workflows cannot create the same release. Preserve the Windows package
recipe and its independent signing-ready stages.

| Trigger | Build and delivery |
| --- | --- |
| Manual dispatch | Select Windows, macOS or both; default Windows preserves the existing single-platform action. Build only selected platforms, upload their final packages as Actions artifacts, and create no release. macOS still requires its complete signing inputs. |
| Push of `vVERSION` or `vVERSION-rc.N` | Run one shared eligibility job, then both platform package jobs. One delivery job collects their exact outputs and creates one unpublished draft with both the MSI and DMG. |
| Pull request, ordinary push/merge or schedule | No desktop packaging jobs or signing access. Existing ordinary verification remains in its own workflow. |

The dispatch platform selector and shared delivery job are engineering choices
to preserve independent development builds while using ordinary workflow
dependencies for one tagged release. Platform jobs may run concurrently after
eligibility; the delivery job requires both to succeed. A failed macOS build
must not erase a successful Windows Actions artifact, but it prevents creation
of a complete combined release draft.

Reuse the newest existing **Rust** push workflow and its latest attempt for
the exact commit, requiring `verify (ubuntu-latest)` and `verify-web` success.
Do not rerun, dispatch or poll those jobs, accept a merge-revision PR result,
inherit an ancestor's pass or search backwards for an older successful
attempt. Missing/pending/failed/skipped/cancelled results fail eligibility.
Manual development dispatch retains the Windows policy of running only the
requested platform checks without this release eligibility gate.

Release DMG filenames are `clacks-log-VERSION-macos-arm64.dmg`, with the full
validated tag minus its leading `v` supplying VERSION, including `-rc.N` when
present. Development filenames follow the Windows identity pattern with
Cargo version, short revision, run and attempt before `-macos-arm64.dmg`.
Use platform-specific Actions artifact names; upload only the final package
for each platform. Put hashes, tool/signing identity, source revision and
diagnostics in the workflow summary/logs, linked from the draft's notes.

Use ordinary GitHub CLI draft creation, with release/prerelease status derived
from the validated tag. Only delivery needs repository contents write access;
eligibility and package jobs do not publish. An existing release or conflicting
asset is a diagnostic failure: do not overwrite assets, move tags, delete a
partial release or introduce automatic reconciliation. Inspect partial drafts
before retrying. Rebuilt bytes require fresh acceptance. No custom retention
system is added. See [GitHub release creation][gh-release].

Publication remains manual after native testing of the draft's exact downloads.
It does not rebuild, re-sign, re-notarize or otherwise mutate the packages.
The draft identifies platform-specific signing status truthfully: Windows may
remain unsigned while macOS is Developer ID signed and notarized. Do not label
the combined release as wholly signed or wholly unsigned.

## Installation, replacement and preservation

Installation is a Finder copy of the complete bundle. Upgrade means quit
normally, then replace the installed bundle with the new one. Do not replace a
running application, force-quit it, force a reboot or add an application-owned
replacement coordinator. The guide must require normal close before replacement;
native checks record Finder's actual handling of conflicts, cancellation and
permissions rather than promising Windows Installer transactions on macOS.

Removal means removing the application bundle. Installing, replacing or removing
the bundle never deletes workspaces, configuration, UI preferences, protected
credentials or pending work. Reinstall as the same user and signing identity
reuses retained state. The application continues to own schema migrations,
save draining and account/session behavior. Do not add package-driven credential
cleanup or migration. Document what is retained and the existing explicit
removal operations; do not invent a new destructive cleanup feature.

A DMG/Finder copy does not enforce ordered version installation. Preserve the
source version policy and test a genuine increasing-version upgrade; do not
claim automatic downgrade rejection. Document that an older application may be
incompatible with a forward-migrated workspace and is not a supported rollback
method. Failed/cancelled copying must not be reported as a verified upgrade.

## Diagnostics and observability

Keep the desktop's existing tracing, logs and metrics; packaging does not install
another subscriber or alter runtime diagnostics. Extend the helper's existing
operation spans and structured events for platform/source validation and any
Clacks-owned decision logic. Significant stage outcomes appear at `info`;
attention-worthy warnings and handling-boundary failures retain useful causes.
Use workflow job/step timing for external-tool stages rather than synthetic
duplicate timers, metrics exporters or a packaging telemetry service.

Correlate evidence with source revision, platform, workflow run/attempt,
tool versions, notarization submission ID and final hash. Record accepted versus
rejected signing/notarization outcomes accurately. Never log secrets, private
keys, notarization authentication or credential contents. Review native and
notary diagnostics before sharing; exclude unnecessary account/device details.
Verification must cover any changed Clacks reporting decisions, without testing
Apple/GitHub formatting or repeating domain scenarios just for telemetry.

## Verification and completion evidence

Implementation checks must target actual Clacks mistakes: wrong platform
metadata, version/tag mismatches, wrong minimum version or architecture,
wrong bundle identity/profile entitlements, missing resources/notices,
incorrect shared release gating and accidental byte mutation after hashing.
Reuse helper tests and platform tools; do not build parallel parsers for
standard tool output merely to duplicate their validation. The implementation
plan must name precise checks and report unavailable checks separately.

Native acceptance uses the production app copied from the exact downloaded DMG,
on clean Apple Silicon environments for macOS 26.0 and the current newer major
release available at validation time. Record exact tested versions; do not claim
all future macOS releases are tested or adopt a rolling-version support rule.
If minimum-version or newer-version evidence cannot be obtained, leave that
acceptance gap explicit rather than silently changing the selected baseline.
A hosted runner build or an updated developer Mac is not clean-machine evidence.

| Scenario | Required observation |
| --- | --- |
| Download and first installation | Browser download with normal quarantine; compare SHA-256; mount/copy/eject; ordinary-user launch from installed location with Gatekeeper intact. Record ordinary consent separately from warnings or trust failures. No security bypass instructions. |
| Offline trust | Exercise first installation/launch from the stapled downloaded DMG with network unavailable on a clean environment; record actual ticket/trust behavior. Command-line signature checks alone do not prove this. |
| Offline application | Create isolated state, log/edit/delete a contact, quit normally and reopen. Verify durable data and usable rendering, native menus, keyboard input and scaling. |
| Protected credentials and connected functionality | Use disposable accounts to verify the installed app can write/read protected credentials, perform available enrollment/browser-return and synchronization workflows, and reconnect after restart. Verify directory credentials where available. No plaintext/backend fallback or secrets in logs. |
| Upgrade | Install two genuinely increasing source-versioned artifacts with the same bundle/team/access-group identity. Retain saved data, credentials, pending offline edits and configuration; verify continued credential access and sync after normal close/replacement/reopen. |
| Running app and interrupted replacement | With an admitted save and secondary window, follow normal-close guidance and observe Finder refusal/cancel/error behavior. Preserve original application/state when replacement is not completed; record limits instead of claiming transactional rollback. |
| Uninstall/reinstall | Remove the bundle and reinstall the exact identified artifact as the same user. Confirm retained data, credentials and pending work. Verify the guide's retention/removal statements. |
| Artifact contents | Inspect installed bundle identity, icon, version, notices, architecture, deployment information, runtime dependencies, signed entitlements/profile, signatures and stapled DMG. Exclude developer-only assets and private signing material. |

Use disposable ordinary-user accounts and explicit isolated workspace,
configuration and log paths for all native exercises. Automated tests use
temporary workspaces and disposable databases. Follow the user-owned local
development inventory before running the desktop/server; keep its paths and
environment details out of the repository. Verify an explicitly launched
isolated process before attaching native UI automation. Never use, repair,
downgrade or delete the owner's normal application state for acceptance.

Record DMG hash, version/tag, exact source revision, signing identity/team and
profile identity/expiry, notarization submission/result, runner/tool versions,
native macOS/hardware/graphics environment, observed trust prompts and each
scenario's result in issue #46 or its pull request. Ordinary GitHub artifacts
and verification notes suffice. No credentials belong in that evidence.

Keep #46 open until actual installed-artifact evidence satisfies its scope.
Documenting this design, successful cross-compilation, metadata validation or
signing checks alone cannot complete it. Leave #43's complete-workflow release
acceptance and #37/#38's final integrated/recovery coverage with those issues.

## Work left to implementation planning

After user approval of this specification, prepare the implementation plan.
It must resolve the focused helper/platform parameter changes, exact workflow
job graph and protected-environment wiring, selected Xcode/tool versions,
bundle resource layout, icon conversion, entitlement/profile verification,
notarization timeout/error handling, operator guide updates and precise native
acceptance procedure through supported facilities. These routine details do
not reopen the selected platform or distribution rules.

The continuing macOS version-support policy is explicitly deferred to the user.
Certificate/profile/notarization access is an unfulfilled delivery prerequisite,
not a reason to substitute development signing. Account conversion is not a
prerequisite and no new developer subscription is assumed.

## Sources

[issue-80]: https://github.com/jhelwig/clacks-log/issues/46
[issue-79]: https://github.com/jhelwig/clacks-log/issues/45
[issue-70]: https://github.com/jhelwig/clacks-log/issues/43
[desktop-guide]: ../../../lib/clacks-desktop/README.md#native-credential-integration
[artwork]: ../../../lib/clacks-desktop/assets/README.md
[packaging]: https://developer.apple.com/documentation/xcode/packaging-mac-software-for-distribution
[certificates]: https://developer.apple.com/help/account/certificates/certificates-overview
[developer-id]: https://developer.apple.com/help/account/certificates/create-developer-id-certificates
[keychains]: https://developer.apple.com/documentation/technotes/tn3137-on-mac-keychains
[profiles]: https://developer.apple.com/documentation/technotes/tn3125-inside-code-signing-provisioning-profiles
[notarize]: https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution
[notary-workflow]: https://developer.apple.com/documentation/security/customizing-the-notarization-workflow
[github-signing]: https://docs.github.com/en/actions/how-tos/deploy/deploy-to-third-party-platforms/sign-xcode-applications
[runner]: https://github.blog/changelog/2026-02-26-macos-26-is-now-generally-available-for-github-hosted-runners/
[gh-release]: https://cli.github.com/manual/gh_release_create

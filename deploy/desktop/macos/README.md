# macOS desktop disk images

Clacks Log's early macOS package targets **macOS 26 Tahoe and newer on Apple
Silicon** (`aarch64-apple-darwin`). This baseline does not select a continuing
policy for how many older macOS releases to support. The application is owned
by Technosorcery, LLC. Developer ID signing and notarization are required for
macOS distribution. Current functionality is pre-alpha.

## Downloads and release rules

The **Desktop packages** workflow is the executable recipe for both platforms.
After it is on the default branch, manually dispatch it with platform `macos`
or `both` for development downloads, leaving **release** disabled. The default
remains `windows`. Ordinary PRs and pushes do not start packaging; merged
release-preparation PRs continue the requested release automatically.
The `Clacks-Log-VERSION-Development-macOS-Apple-Silicon` artifact contains only the DMG.
The summary/logs record its final SHA-256, source revision, version, signing and
notarization evidence, tools and runner image. Normal GitHub availability applies.

Release preparation creates `desktop-vYY.M.N`, explicitly dispatches both
platforms, and publishes one prerelease after both exact-source installers
are uploaded successfully. Follow the
[shared release steps](../windows/README.md#versioned-releases) for exact-commit
verification, automated publication, and promoting the same files to Latest.

The Mac release filename is `Clacks-Log-VERSION-macOS-Apple-Silicon.dmg`;
development uses `Clacks-Log-VERSION-Development-macOS-Apple-Silicon.dmg`.
Actions artifact names are the filename stems. Revision, run and attempt remain
in ordinary provenance output.
Both About and `--version` expose the numeric application version and available
build provenance. To supply provenance to a local build from a Git working
tree:

```sh
CLACKS_BUILD_REVISION="$(git rev-parse HEAD)" \
CLACKS_BUILD_DIRTY="$(if [ -n "$(git status --porcelain)" ]; then echo true; else echo false; fi)" \
  cargo build -p clacks-log --locked
```

## Geographic build artifact

The common Linux job restores the validated data cache or generates data on a
miss, then transfers the same directory and expected manifest digest to both
native jobs. It also runs for a macOS-only manual build. See the
[common build procedure](../../../references/geography/BUILDING.md) and
[Windows payload instructions](../windows/README.md#geographic-build-artifact).
Native compilation embeds `CLACKS_GEOGRAPHY_MANIFEST_SHA256` from that output.

Assembly copies the tree to `Contents/Resources/geography`, retaining declared
notices, and checks it against the common expected digest before Developer ID
signing. Mounted disk-image inspection verifies runtime identity/integrity and
the final application signature with data already included. It does not repeat
exhaustive production acceptance. Existing build evidence records geographic
identity and size; no standalone geographic release is needed.

## Signing setup

Both development downloads and versioned downloads require:

- A Developer ID Application certificate and its private key. Apple Development
  alone is not a distribution identity. Apple
  Distribution and Developer ID Installer are not the certificate types used
  for this application-in-DMG path.
- A Developer ID provisioning profile authorizing `com.w7hex.clacks-log` and
  its default Keychain access group. Derive the prefix and team from that
  actual profile; the prefix is not assumed to equal the team ID.
- Working notarization authentication: Apple ID, team ID and an app-specific
  password with Apple's `notarytool`.

The owner must separately authorize account/credential and GitHub environment
setup, purchases and software publication.

Configure environment **macos-signing** with deployment ref restrictions
allowing only trusted source code. For tagged desktop releases, update the
allowed tag pattern from `v*` to `desktop-v*`. A manual dispatch cannot bypass those rules;
the selected branch executes workflow code and must be trusted. Provide these
inputs as encrypted environment secrets (the team ID may also be a variable
only if the workflow is updated to read it there):

| Secret | Contents |
| --- | --- |
| `APPLE_SIGNING_P12_BASE64` | Base64 password-protected Developer ID signing identity export, including the private key. |
| `APPLE_SIGNING_P12_PASSWORD` | Password for that export. |
| `APPLE_PROVISION_PROFILE_BASE64` | Base64 Developer ID profile for the application. |
| `APPLE_ID` | Developer account Apple ID used for notarization. |
| `APPLE_TEAM_ID` | Expected signing team ID. |
| `APPLE_APP_SPECIFIC_PASSWORD` | App-specific password for notarization. |

Missing inputs fail by field name without printing values. The native job
imports the identity into an ephemeral keychain, selects its Developer ID
Application identity, embeds the profile and populates a temporary entitlement
copy. Keychain access stays local, app-specific and When Unlocked, without
cloud synchronization or a different credential backend. No sandbox or
unrelated entitlement is added.

Do not put keys, profiles, authentication files or keychains in source control,
caches or delivery artifacts. The embedded profile is intentionally inside the
signed app; the private key and authentication material are not. Cleanup runs
even on failure/cancellation; hosted runner disposal is a further boundary.
Use the profile's existing team/prefix/access-group identity across upgrades.

## Native build and bundle inputs

The workflow selects `macos-26` and Xcode 26.6 explicitly, pins Rust 1.99.1
through `rust-toolchain.toml`, uses locked dependencies and builds offline
after fetching them. Record actual image/tool versions: runner images can
change while their label stays the same. Local builds with another Xcode are
not evidence of reproducing the selected hosted recipe.

From the repository root, the basic build and notice commands are:

```sh
cargo fetch --locked
MACOSX_DEPLOYMENT_TARGET=26.0 SQLX_OFFLINE=true \
LEPTOS_OUTPUT_NAME=clacks-web \
  cargo build -p clacks-log --release --target aarch64-apple-darwin \
  --locked --offline
cargo build -p clacks-desktop-package --locked --offline
mkdir -p target/macos-package
cargo metadata --no-deps --format-version 1 --locked --offline \
  > target/macos-package/cargo-metadata.json
target/debug/clacks-desktop-package prepare --platform macos-arm64 \
  --metadata target/macos-package/cargo-metadata.json \
  --revision "$(git rev-parse HEAD)" --output target/macos-package/metadata.json
cargo install cargo-about --version 0.9.3 --features cli --locked \
  --root target/notice-tool
target/notice-tool/bin/cargo-about generate --locked --fail \
  --manifest-path bin/clacks-log/Cargo.toml \
  --config deploy/desktop/macos/about.toml \
  --output-file target/macos-package/THIRD-PARTY-NOTICES.txt \
  deploy/desktop/windows/notices.hbs
```

Assemble `target/macos-package/image/Clacks Log.app` with these explicit inputs:

| Source | Bundle destination |
| --- | --- |
| `target/aarch64-apple-darwin/release/clacks-log` | `Contents/MacOS/clacks-log` |
| `bin/clacks-log/macos/Info.plist`, with versions supplied from prepared metadata | `Contents/Info.plist` |
| `lib/clacks-desktop/assets/clacks-log.icns` | `Contents/Resources/clacks-log.icns` |
| `LICENSE`, `NOTICE` | `Contents/Resources/LICENSE.txt`, `NOTICE.txt` |
| Generated `THIRD-PARTY-NOTICES.txt` | `Contents/Resources/THIRD-PARTY-NOTICES.txt` |
| `references/trustedqsl/upstream/LICENSE.txt`, unchanged for embedded model data | `Contents/Resources/THIRD-PARTY-DATA-NOTICES.txt` |
| Authorized external provisioning profile | `Contents/embedded.provisionprofile` |

Use `ditto` for bundle contents, preserving symlinks; use PlistBuddy to set the
copied `CFBundleVersion` and `CFBundleShortVersionString` from Cargo metadata.
The source plist supplies minimum system version `26.0`. Populate a temporary
copy of `entitlements.plist` from the profile, with the application identifier
and sole default access group `PREFIX.com.w7hex.clacks-log` and its team ID.
Keep credential service identifiers unchanged. The original icon/provenance
lives in [the artwork guide](../../../lib/clacks-desktop/assets/README.md).

Geographic files ship in `Contents/Resources/geography`; no separate installation
is required. There are no presently required non-system dynamic
libraries in this payload. Inspect `file`, `vtool -show-build` and `otool -L`
against the actual build; add explicitly required permitted runtime files and
their notices if dependencies change. Do not ship developer/Homebrew paths,
test executables, development settings, reference source trees or debug symbols.
The [data notice guide](../../../lib/clacks-model/data/README.md) explains
applicability; copying its notice does not execute TrustedQSL source.

The complete signing recipe is in
[desktop-release.yml](../../../.github/workflows/desktop-release.yml): sign
nested code before the outer app, use Developer ID with a secure timestamp and
hardened runtime, inspect signatures/entitlements/profile, create a read-only
compressed DMG with `hdiutil` and an Applications symlink, and sign that DMG
with ID `com.w7hex.clacks-log.dmg`. Apple's tools own signing and profile
authorization checks; no custom profile parser or wildcard matcher is used.

Submit the signed DMG with `notarytool --wait --timeout 30m`; require accepted
notarization, inspect diagnostics, staple and validate the DMG, then inspect
the mounted application's signature and distribution policy. Do not mutate
its inner payload or require another notarization cycle. Compute the final
hash after all signing/stapling, and upload that same file. A failed or timed-out
operation stops delivery; Apple may continue processing a timed-out submission.
Inspect its recorded submission ID before manually deciding whether to rerun.
There is no unattended resubmission/waiting queue or weaker-signing fallback.

Use `spctl` to assess the signed/stapled DMG and its mounted app, then run the
final mounted executable's `--help`/`--version` with isolated data paths. Direct
execution checks launch authorization for its restricted entitlements without
opening a workspace; actual protected credential use still needs native
acceptance. Do not gate this outer-stapled arrangement on
`syspolicy_check distribution` of the inner app: Apple documents its false
fatal missing-ticket result for an unstapled app. See
[Apple launch diagnostics](https://developer.apple.com/forums/thread/706442)
and [native container assessment](https://developer.apple.com/library/archive/technotes/tn2206/).

## Installation, upgrade and removal

Open the downloaded DMG and copy **Clacks Log.app** to Applications using Finder.
Use `/Applications` or your user's `~/Applications` according to ordinary folder
permissions. Eject the image and launch the installed app. There is no privileged
installer, automatic post-install launch, login item or separate updater.

Quit Clacks Log normally before replacing its bundle. Let admitted saves finish;
do not replace a running app or force-quit it for an upgrade. Finder owns copying,
permissions and cancellation. Observe actual errors; this is not a transactional
Windows Installer upgrade or guaranteed rollback. Older builds may not understand
a workspace migrated by a newer build; replacing the app with an older copy is
not a supported data rollback method. A DMG does not enforce downgrade rejection.

Removing the application bundle retains user workspaces, pending offline work,
configuration, UI preferences and protected credentials. Reinstalling as the same
user with the same signing/access-group identity reopens retained state. Only
the application opens/migrates workspaces or manages credentials; mounting,
copying and removing the bundle never run those operations. Intentional removal
of retained state is separate from removing the app; no destructive cleanup
command is added. Existing account/enrollment operations keep their documented
credential lifecycle.

## Native acceptance

Build, signature and policy checks are not installed-product acceptance. Keep
[#46](https://github.com/jhelwig/clacks-log/issues/46) open until exact installed
artifacts satisfy the scenarios below.
[#43](https://github.com/jhelwig/clacks-log/issues/43) retains complete integrated
MVP release acceptance; signing this early package does not complete it.

Use clean Apple Silicon environments on macOS 26.0 and the newer current major
version. Record exact versions/hardware; an upgraded developer Mac or hosted
runner is not clean-machine evidence. Download through the intended browser,
preserve quarantine and normal Gatekeeper checks, and compare the final hash.
Mount the image, copy the app, eject the image and launch from the installed
location. Distinguish ordinary consent from trust failures. Do not prescribe
security bypasses. Exercise first installation and launch offline and online.

Use disposable ordinary-user accounts and explicit isolated `--workspace`,
`--config` and `--log-directory` paths. Follow the user-owned local development
inventory before desktop/server execution and verify the actual process before
UI automation attaches. Never discover/launch a development app into default
personal state. Tests use temporary workspaces and disposable databases.

Exercise offline contact editing/save/reopen; native menus/input/scaling/icons;
available enrollment/browser return/sync/directory credentials; normal shutdown;
an upgrade between two genuinely increasing source-versioned artifacts with
the same bundle, team and access-group identity, retaining pending work and
credential access; interrupted or rejected copy with a running app and an
admitted save; and same-user reinstall with retained state. Inspect the
installed bundle identity, icon, version, notices, architecture, deployment
target, runtime dependencies, signed entitlements/profile, signatures and
stapled image. Test Clacks's installed behavior, not generic Finder, Keychain
or Gatekeeper conformance. Existing signed-host integration tests are additional
application evidence, not a substitute for this production artifact.

Record DMG hash/version/tag/revision, signing team/profile identity and expiry,
notarization submission/result, tools/image, native environment, observations and
per-scenario passed/failed/blocked/untested status in #46 or its pull request.
Redact secrets and unnecessary personal/device information. Normal GitHub
artifacts and notes suffice; no separate archival/retention service is required.

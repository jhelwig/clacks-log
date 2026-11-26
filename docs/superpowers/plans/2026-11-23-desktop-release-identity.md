# Desktop release identity implementation plan

**Goal**
Give the desktop executable and its Windows and macOS packages one release
identity that distinguishes packaged builds at the same version. Local builds
report known provenance.

**Architecture**
The `clacks-log` executable owns the version in Cargo. The desktop library
compiles that version and the available provenance into a single build identity.
The existing packaging helper reads the same package version from Cargo
metadata, validates the tag and supplies the numeric version to both native
jobs. GitHub release state expresses whether the tagged build is a draft, a
public prerelease or the recommended final.

**References**
The [release lifecycle design] supplies the product behavior. [Issue #71]
tracks delivery. The later download page, compatibility declaration and update
checks have their own issues.

## Scope and review focus

- Keep Rust 1.99.1 and the existing Windows 11 x64 and Apple Silicon macOS 26
  packaging targets. Keep the existing exact-commit eligibility, signing,
  geographic payload and final verification steps.
- Use `YY.M.N` and `desktop-vYY.M.N` for a release build. Keep the current
  numeric version when introducing the plumbing. Assign the first release
  number when cutting a candidate.
- Check the identity at its real consumers. In particular, a library version
  can differ from the app, two app builds can share a numeric version, and the
  installer names can accidentally include the Git tag prefix.
- Same-version Windows Installer (MSI) replacement keeps its current package
  configuration. This plan adds coverage for provenance and release packaging.

## 1. Carry the desktop build identity into the application

This change delivers `--version` and About before changing the release tag
workflow. Use `bin/clacks-log/Cargo.toml` and `Cargo.lock` for package ownership
and `lib/clacks-desktop/src/build_identity.rs` for the compiled identity and
its display. Keep the existing `clacks_desktop::run()` entry point.

- [ ] Move the current desktop version to an explicit `version` in
  `bin/clacks-log/Cargo.toml`. Update the workspace lockfile if Cargo requires
  it. Keep workspace inheritance for the server and other packages.
- [ ] Add `lib/clacks-desktop/build.rs` to read `package.version` from
  `bin/clacks-log/Cargo.toml` with the workspace's locked `toml` parser. Resolve
  the path relative to `CARGO_MANIFEST_DIR`, declare the manifest as a rebuild
  input, and export the value as `CLACKS_DESKTOP_VERSION` at compile time.
  Fail the build with a manifest-specific diagnostic if the field is missing.
  This lets the library use the same version as packaging while the Cargo
  package version visible to the library still belongs to the library.
- [ ] In `build_identity.rs`, define a crate-private `const BUILD_IDENTITY`
  from `env!("CLACKS_DESKTOP_VERSION")` and the optional compile-time
  provenance inputs. Keep the numeric version available on its own and share
  provenance formatting between About and `--version`.
- [ ] In the existing Windows and macOS workflow identity steps, export
  `CLACKS_BUILD_REVISION` from the checked-out commit already used to verify
  package identity. Export `CLACKS_BUILD_RUN_ID` and
  `CLACKS_BUILD_RUN_ATTEMPT` from the build job. Support optional
  `CLACKS_BUILD_DIRTY` input with literal `true` or `false` for known local
  status. Document a local build invocation that captures `git rev-parse HEAD`
  and whether `git status --porcelain` reports changes before calling Cargo.
  Plain Cargo invocations remain usable with unavailable provenance omitted.
  The compile-time inputs let repeated builds use the same Cargo cache.
- [ ] In `lib/clacks-desktop/src/lib.rs` and `config.rs`, configure the existing
  Clap command with the executable version and provenance. Use Clap's owned
  string support for `long_version` and keep argument handling before workspace
  discovery. Reuse the pinned native menu library's built-in About panel on
  both platforms. Feed the application version and provenance into the macOS
  application menu and add the Windows entry to the existing Help submenu in
  `menus/native.rs`. Use the existing platform adapters and platform-supported
  About fields.
- [ ] Audit the other desktop `CARGO_PKG_VERSION` reads when splitting the
  manifest version. Existing application version outputs are the desktop
  telemetry `service.version`, the provider request identifiers, and the
  Amateur Data Interchange Format (ADIF) export `PROGRAMVERSION`. Read the
  numeric version from the compiled identity at those existing sites. Provider
  requests and exports carry only the numeric version. Keep metric dimensions
  unchanged.
- [ ] Extend the normal package job's `--version` smoke check to compare
  reported version, commit, workflow run and attempt with its packaging inputs.
  Capture stdout explicitly and wait for exit on Windows. Check the executable
  in the mounted final disk image on macOS. Confirm About shows the same
  information during native packaging acceptance. These checks cover the Clacks
  wiring from build inputs to the executable and its presentation.

## 2. Drive tagged packaging and candidate publication from the desktop version

This change delivers the `desktop-v*` workflow. The existing
`clacks-desktop-package prepare` command remains the boundary for validating
the app's Cargo metadata and optional release tag.

- [ ] In `lib/clacks-desktop-packaging/src/metadata.rs`, use the existing
  `prepare` signature. For tagged builds, validate three canonical numeric
  components with a two-digit year, a month from 1 through 12 and the
  existing native version bounds.
  Require an exact match to `desktop-v{version}`. Allow development builds
  without a tag to keep their source version. Keep the validated numeric
  `version` in `BuildMetadata` and remove the tag-derived `prerelease` field.
- [ ] Replace the old `v*` trigger, tag stripping and job-level prerelease
  outputs in `.github/workflows/desktop-release.yml`. Take installer names from
  the validated version on each platform. Feed that value into the Windows
  Installer and the macOS bundle version fields through their existing paths.
  Keep the development filenames and platform selection. The combined delivery
  job creates a draft marked as a prerelease with `--latest=false`. The
  maintainer publishes that draft and later promotes the same GitHub Release
  and both installers by clearing the flag and selecting Latest.
- [ ] Update the versioned release instructions in both platform packaging
  guides and the corresponding links or summaries in `docs/desktop.md` where
  they describe this workflow. Document the new tag, development provenance
  inputs, draft publication and same-artifact promotion.
- [ ] Replace the old release-tag cases in
  `lib/clacks-desktop-packaging/src/metadata/tests.rs` with cases showing that
  `desktop-v26.11.1` selects the `clacks-log` manifest version even when another
  package has a different version, and that a mismatched tag, a server tag or
  an old `-rc.N` tag fails validation. Include a tagged invalid calendar month
  and a development build with its unchanged source version. Run
  `cargo test --locked -p clacks-desktop-packaging` and
  `SQLX_OFFLINE=true cargo check --locked -p clacks-log`.
- [ ] Check the workflow's package names and native version outputs against
  the validated value when packaging on the existing native runners. Use the
  existing Windows executable resource check and read `ProductVersion` in
  the generated MSI. Compare both macOS bundle version fields where the final
  image is already inspected. Record native evidence from an available
  packaging run. For the first candidate, merge the CalVer to `main`, wait
  for the existing push checks on that exact revision, then tag it. Use this
  candidate to verify the displayed version when the app and library versions
  differ.

[release lifecycle design]: ../specs/2026-11-23-desktop-release-lifecycle-design.md
[Issue #71]: https://github.com/jhelwig/clacks-log/issues/71

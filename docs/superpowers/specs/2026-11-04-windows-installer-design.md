# Early Windows desktop installer

Status: the installer design and updated release pipeline were approved on
November 4, 2026. The user selected
Windows 11 x64, current-user installation, MSI (Windows Installer package),
and `msi-builder`. Pipeline trigger and delivery decisions below were explicitly
agreed with the user. No installer or native acceptance is delivered by this
document.

## Intent, requirements and boundaries

Deliver the production desktop functionality available now through repeatable
release-mode builds and a usable installer. Issue [#79] is authoritative for
scope and completion; D392 establishes early Windows delivery before complete
MVP readiness. Do not wait for future logging workflows or geographic assets.

The issue requires stable identity, icons, version metadata, shortcuts,
runtime resources and redistribution notices; preservation of data, protected
credentials and pending work; exact-artifact native acceptance; and a single
pipeline that permits executable signing before packaging and MSI signing
afterward. Unsigned builds must need no signing credentials.

The user's evaluation preference assigns significant negative weight to
additional mandatory payment agreements for tools, rather than automatically
excluding them. Standard integration is preferred over project-owned installer,
updater, signing or recovery machinery. Project-owned executable tooling stays
in Rust. Declarative packaging and workflow configuration are not another
application implementation language.

This delivery adds no automatic updater, Microsoft Store delivery, Linux
desktop distribution, macOS package, hosted launch or new server workflow.
Website download delivery is deferred. Automation creates draft GitHub Releases;
the user publishes them after testing. This design does not authorize publishing
a release now or purchasing signing services.
The existing application owns workspaces, database migrations, enrollment and
credential access; the installer owns application files and installation
registration. No installer action opens or migrates a workspace.

## Approaches considered and selection

| Approach | Demonstrated fit and tradeoff | Disposition |
| --- | --- | --- |
| `msi-builder` | Independent YAML authoring, current-user packages, upgrade sequencing, standard installer UI, resources and optional signing; exact native integration still needs acceptance. Apache 2.0 source license. | Selected by the user. |
| GNOME `wixl` | Independent XML compiler with current-user installation and explicit action sequencing. Released v0.106's high-level upgrade default removes the old version outside the replacement transaction; ordinary sequence authoring needs verification. | Viable alternative, not selected. |
| Advanced Installer | Freeware includes current-user installation, upgrades and command-line builds. Integrated signing and command-line editing are paid features; external signing does not itself require those features. Paid subscriptions and unattended-agent licensing impose continuing cost. | Not selected; no required capability established a reason to purchase. |
| Direct WiX and its wrappers | MSI authoring facilities fit, but current vendor binaries introduce an additional maintenance-fee agreement and explicit acceptance. Wrappers do not remove that consideration. | Significant negative under the user's preference; not selected. |
| NSIS and Inno Setup | Produce executable installers rather than the subsequently selected MSI format. cargo-packager's stock NSIS process handling can terminate by executable name. | Earlier alternatives, not selected. |

Comparison sources: [msi-builder release][tool-release],
[wixl v0.106 source][wixl-source], [Advanced Installer features][ai-features]
and [licensing][ai-licenses], [WiX binary agreement][wix-agreement], and
[cargo-packager NSIS template][nsis-template]. These explain the selection;
they do not create additional Clacks requirements.

## Selected tool and evidence boundary

Use `msi-builder` **v1.2.0**, tag commit
`d47749fee3d4a274d5728a72286b986eb84e7b07`, released November 1, 2026.
The selected Windows build-host archive is
`msi-builder_1.2.0_windows_amd64.zip`, with SHA-256:

```text
d528277ae68218335318e8f335c01bdbfaa89882ffa53625251ad3e70c7735a7
```

The version, archive and digest come from the [release metadata][tool-release].
Pin them in the implementation; verify the archive before extraction and
execution. Do not resolve `latest` during a build. Tool updates require a
reviewed version/digest change and relevant package checks.

The tagged [schema][tool-schema] and [close implementation][tool-close] supply
the relevant authoring facilities. The tagged [build command][tool-build]
reads back the package and compares its contents with the sources by default;
do not disable that check. `validate` checks configuration without reading
payload files. `inspect` reads package tables. `verify` checks a signed
package's content signature and timestamp, returns failure for an unsigned
package, and does not establish certificate trust or revocation status.
Treat these as distinct checks, not interchangeable verification commands.

The release API and tagged README establish v1.2.0. Upstream
[Windows test results][tool-results]
are supporting evidence about that tool, not native acceptance of Clacks on
Windows 11. No local execution of the tool is claimed here.

## Package identity, layout and resources

- Application and installed-program display name: **Clacks Log**. Keep the
  identity used for upgrade detection stable. The owning entity and
  publisher/company metadata are **Technosorcery, LLC**, as explicitly supplied by the user
  (D393). Preserve third-party ownership and notices.
- Reuse `com.w7hex.clacks-log` as the application identity where Windows
  supports it. It already exists in the production macOS bundle definition.
  Keep the executable name `clacks-log.exe` and protected-credential service
  names unchanged.
- Generate one random upgrade GUID (globally unique identifier) at initial
  implementation, commit it in the package definition, and retain it for this
  product. Use the tool's generated product/package identities rather than
  inventing an identity registry or maintaining component GUIDs by hand.
- Select platform `x64` and scope `perUser`. Default installed files to
  `%LOCALAPPDATA%\Programs\Clacks Log`. Do not offer machine-wide scope or
  write machine-wide registration. Windows 11 x64 is the supported target;
  this does not claim support for Windows 10, Windows Server or Arm64 emulation.
- Install a current-user Start menu shortcut. Propose no desktop shortcut by
  default. Launch the executable directly without a shell or embedded server
  configuration. Do not add startup-at-login, file associations or protocols.
- Supply the selected application icon in the executable, installed-program
  registration and shortcut/window contexts. The user selected generated
  option 2: pale blue CL letters on a dark rounded-square background, using
  the desktop palette (`#ADD6E7` accent and `#1D2228` workspace). Preserve the
  [selected original PNG](assets/2026-11-04-clacks-log-icon.png) as the artwork
  source and record its generation provenance. It was generated with the
  built-in image generation tool, requesting ordinary bold sans-serif CL
  letters, generous margins, flat palette fills and no additional symbols.
  No editable SVG was supplied or selected; do not invent or redraw one.
  Derive production PNG/ICO sizes from this artwork and inspect small-size
  legibility, transparency edges and visible generation artifacts before use.
- Use a standard setup wizard with install/progress/completion and ordinary
  cancellation/error handling. Include an application description and
  installed license/notices. No custom setup screens collect server addresses,
  credentials or workspace choices. Do not automatically launch the desktop
  from installation, repair or removal.

Keep a focused definition under `deploy/desktop/windows/`. YAML lists
explicit MSI file inputs, not a glob of the repository, target directory or
developer home. Package the executable, necessary runtime dependencies/assets,
project license and dependency/data notices. Embedded assets still require
applicable notices. Derive the actual payload from the release binary and
available production features; do not ship unused geography examples, reference
source trees, development configuration, secrets or debug symbols in the MSI.
The implementation checks native library dependencies on a clean Windows host;
it may bundle permitted runtime libraries rather than adding an elevated
prerequisite installer. Do not presume that a developer machine proves a
dependency is supplied by Windows.

## Versioning and executable metadata

Use the Cargo workspace package version as the application/package release
version. Start from the actual source version, currently `0.1.0`, and obtain it
through Cargo metadata or its build environment. Pass it through the tool's
supported YAML variables. Do not hand-edit source during packaging, invent a
parallel installer version counter or treat a fourth MSI version field as an
ordered release number. Every distinct delivered release advances one of the
first three numeric fields. Rebuilding a source version is allowed for
verification, but is not advertised as a newer release.

The Rust binary build supplies Windows executable icon and version resources.
Use an established Rust resource helper, with its compatible release locked;
[`winresource`][resource-helper] documents Cargo build-script integration, icons, version fields
and the standard Windows resource compiler. No project-owned resource compiler
or executable patcher is justified. Validate executable file/product version
against the MSI version before packaging. Keep source revision and signing
status in artifact evidence rather than overloading MSI comparison fields.
Do not change workspace or credential identity to encode the release version.

## Upgrade, shutdown and removal

Use the tool's major-upgrade facilities, explicitly selecting
`removeOld: afterInstallInitialize`, rejecting lower-version installs and not
ignoring old-version removal failures. Keep one upgrade identity. This provides
Windows Installer's transaction around removal and replacement. Verify the
generated Upgrade and InstallExecuteSequence tables, and verify failure and
cancellation behavior natively. An upgrade failure must not be reported as a
successful replacement. Do not add a Clacks rollback daemon or installer queue.

The proposed initial policy is **user-controlled normal application close**.
Do not configure `close: true`, a timed termination fallback or another
process-killing action. Put `MSIRESTARTMANAGERCONTROL=DisableShutdown` in the
package's Property table so Windows can detect files in use without requesting
automatic application shutdown; use standard files-in-use retry/cancel handling.
Put `REBOOT=ReallySuppress` in that table and add no reboot actions. These are
standard [shutdown][restart-control] and [reboot][reboot-control] controls, not a promise that
replacement of locked files never needs a reboot. Record reboot-required
outcomes distinctly; never force a reboot. Verify full UI and quiet execution
with the desktop running, including all windows and a save already admitted.
If the standard integration does not deliver this behavior, establish the
specific gap before proposing application hooks or other machinery.

The user closes Clacks through its existing graceful-close flow, which drains
admitted work without waiting for network synchronization. MSI files-in-use
handling is not evidence that this flow was exercised: observe saved data and
pending work after restart. Normal shortcuts do not launch an extra background
process. No automatic restart registration is required for this delivery.

User state is outside the installed payload:

- Workspace: `%LOCALAPPDATA%\Clacks Log\data\workspace`, including database,
  local UI state, pending changes and logs.
- Configuration: `%APPDATA%\Clacks Log\config\config.toml`.
- Enrollment/directory secrets: existing Windows protected storage, with local
  persistence and unchanged service/reference identities.

Upgrade, repair, uninstall and reinstall neither initialize nor delete this
state, including user-selected paths. Omit purge/remove-folder actions for
application data and all credential deletion actions. Uninstall removes only
installer-owned files, shortcuts and product registration. Explicitly document
that uninstall retains user data and credentials; do not add a data-removal
checkbox or a reset/recovery feature to satisfy an imagined need. A reinstall
under the same Windows user can reopen its existing workspace and credentials.

## Release automation and signing

Add a focused Windows packaging workflow, preserving every existing Linux and
web verification check. Use a pinned Windows runner family, native
`x86_64-pc-windows-msvc` Rust build, the pinned `rust-toolchain.toml` and committed
Cargo lockfile. Fetch locked dependencies before offline build/check steps.
Use the existing test suites and production binary; do not create a packaging
demo application. Development and release builds use the same packaging stages.

### Triggers and delivery

The initial audience is the user installing on their own Windows machines.
The agreed entry points are:

| Entry point | Output destination | Publication behavior |
| --- | --- | --- |
| Manual development build (`workflow_dispatch`) on a selected branch or tag | GitHub Actions artifacts attached to that run | Does not create or publish a GitHub Release. |
| Push of a version tag, such as `v0.1.0` or `v0.1.0-rc.1` | Draft GitHub Release associated with that exact tag | The user tests the attached installer and publishes the draft manually. |

Do not run Windows installer builds for pull requests or ordinary branch pushes,
including merges to `main`. Windows PR builds remain disabled because of runner
minute consumption. Preserve the existing Linux/web PR and main-branch checks.
Do not add scheduled Windows builds. GitHub Actions artifacts are sufficient for
development delivery; versioned releases use GitHub Releases. Later website
availability is outside this specification.

Resolve the requested ref to an exact source revision and record it with the
outputs. Release tags use `vVERSION`; prerelease tags use `vVERSION-rc.N`,
where `VERSION` matches the numeric committed Cargo version and `N` is numeric.
The validated tag supplies the release step's prerelease input. Both forms create
unpublished drafts. Reject mismatches rather than rewriting source versions.
RC numbers distinguish release candidates without changing MSI version ordering. Development output identity
includes its revision and workflow run so repeated builds of the same Cargo
version can be distinguished without claiming a newer MSI version.

The installer download contains only the final MSI. Record its SHA-256 and
build identity in the workflow summary, and keep diagnostics and verification
results in the job logs. Release notes link to the build run. Identify signing
status, source revision, version, architecture and workflow run. Draft creation
is not publication or proof of native acceptance. The user tests the exact
attached installer; publishing the draft must not rebuild, re-sign or otherwise
replace its bytes. A changed installer requires fresh checksum identification
and native acceptance.

Keep development jobs limited to source read access and ordinary artifact
upload. Grant repository contents write access only to the tag-triggered draft
release delivery job; PR and manual development builds do not get release write
privileges. A failed build or verification must not produce a draft presented as
ready for testing. Failed draft delivery is reported separately from successful
packaging; do not silently fall back to a different delivery destination.

### Check scope and release eligibility

Manual development builds run only the Windows build and packaging checks
described below. They do not run, rerun or require the Linux/web verification
jobs. Their notes must not imply that the complete workspace suite passed.

Release builds run the same Windows checks and reuse the existing Linux/web
results for the exact tagged commit. They do not rerun those checks or dispatch
the Rust workflow. Before starting the Windows runner, resolve the tag to its
commit and inspect the existing `.github/workflows/rust.yml` results through
GitHub's supported read APIs. Require successful completion of both `verify`
(currently Ubuntu workspace verification) and `verify-web` for that commit.
Use the actual tested revision: a PR merge commit or a successful ancestor is
not evidence for a different tagged commit. Match the owning repository and
workflow, not arbitrary check names from another workflow.

Missing, pending, failed, cancelled or skipped required results are not a pass.
Report the specific absent or unsuccessful result and its run link when
available; do not start Windows packaging or create a draft. If checks are
still running, the user can rerun the release workflow after they finish.
Do not add a polling service or automatic Linux/web reruns. Path-filtered
commits with no results remain ineligible; do not silently substitute results
from an earlier revision. Such a failure needs an explicit resolution rather
than a new verification trigger introduced by the packaging workflow.

Use the newest applicable existing run for the commit and its latest attempt.
A pending or unsuccessful latest result must never fall back to an older pass.
Record the checked commit, run ID/attempt,
job conclusions and links in release build notes. Keep reused Linux/web results
distinct from Windows checks performed by the packaging run. Grant only the
read permission needed to inspect existing Actions results. GitHub documents
commit-filtered workflow queries and run/job inspection in its
[workflow-run API][workflow-runs] and [workflow-job API][workflow-jobs].

### Build and signing stages

The sequence is:

1. For a release, establish eligibility from the existing Linux/web results
   before starting Windows packaging. Check out the exact requested revision.
   Validate the packaging configuration and release metadata. Run only the
   applicable Windows build/packaging checks here; do not rerun Linux/web
   checks. Record newly run and reused checks separately in the build notes.
2. Build the production desktop in release mode, smoke-check its command-line
   startup, and generate the reviewed notices. MSI authoring names its inputs
   directly.
3. In unsigned mode, leave the executable unsigned. In a deliberately enabled
   future signed mode, sign and verify the built executable before packaging.
4. Build the MSI with pinned `msi-builder` and its read-back check enabled.
   Retain its supported inspection output in the job logs for diagnosis; use
   the packager's validation and default read-back rather than a custom MSI table checker.
5. Optionally sign the MSI through supported `msi-builder sign` facilities or
   Microsoft's standard SignTool when the eventual signing provider calls for
   it. Do not require exportable private keys or select a provider now. A
   requested signed build with absent credentials or failed signing fails;
   it never silently becomes unsigned. Ordinary unsigned builds have no secret
   prerequisite and do not invoke signature verification that rejects them.
6. Verify final artifacts as appropriate for the mode, then compute SHA-256
   checksums for the workflow summary. Upload only the MSI; retain build
   evidence in the summary and logs. No artifact mutation follows its final
   checksum. Manual development builds upload workflow artifacts; successful
   tag builds attach the same MSI to a draft GitHub Release. Automation never publishes that draft.

Use workflow composition and the packager's existing facilities. If nontrivial
project-owned executable preparation is genuinely necessary, keep it in Rust
and identify its responsibility during planning; do not add an xtask dispatcher,
custom MSI generator, credential broker or signing abstraction by default.

Signing credentials/services and certificate trust acceptance remain future
integration. Tests of conditional stage ordering do not establish actual
signing. Signing-ready here means the same pipeline has the correct mutation
order and explicit modes, not that an unsigned artifact passed trust acceptance.

## Diagnostics and observability

Follow the existing observability contract at changed production boundaries.
Packaging uses normal workflow step logs, packager stderr and exit status, its
read-back/inspection output, and native Windows Installer verbose logs. Correlate
them with workflow run, source revision, application/tool version, architecture,
signing mode and final artifact checksum. Keep build/packaging/signing/upload
outcomes distinct; an upload is not native installation success. Record actual
native return codes and pending-reboot outcomes rather than flattening them.

Do not add runtime tracing exporters, installer telemetry collectors or a new
metrics service for packaging. Existing desktop spans/events/metrics continue
to own startup, storage, credentials, synchronization and shutdown. Verify those
remain usable after installation. Any Rust production helper introduced later
gets meaningful operation spans, structured stage/outcome/count fields and
appropriate measurements under that contract, not per-file boilerplate metrics.
Exclude keys, tokens, provider replies, contact payloads and secret-bearing
command lines from evidence. Treat raw native logs as potentially sensitive;
review/redact them before sharing. Signing mode is safe to record; signing
secrets are not.

## Verification and delivery evidence

Automation validates configuration and builds the production executable and MSI.
The packager owns MSI validation and default payload read-back. Standard Windows
resource and dependency inspection records the executable's identity and imports;
compare its version with the Cargo version supplied to MSI authoring. Final
checksums follow every artifact mutation. Tests cover Clacks's release tag/version
relationship and eligibility decisions, rather than library behavior, invented
MSI tables, or policies introduced by duplicate packaging machinery. Native
acceptance establishes actual installation and preservation behavior.

Native acceptance uses a clean Windows 11 x64 environment and a disposable
ordinary-user account/workspace, never an existing personal log. Report exact
Windows edition/build, architecture, graphics/runtime environment, source
revision, artifact version, checksum and actual signing status. A Windows Server
CI smoke check is not Windows 11 desktop acceptance.

| Scenario | Required observation |
| --- | --- |
| Download and install | Record actual browser/download, installer and launch warnings or blocks. Observe scope, installed-program metadata, icon, Start menu entry and ordinary-user access. Do not prescribe security bypasses; record blocked checks as blocked. |
| First launch and offline use | Create/reopen the isolated workspace, perform available logbook/station setup and offline contact entry/edit/delete, exit normally and confirm persisted values. |
| Available connected features | Use disposable test accounts/server data for enrollment, synchronization/reconnection, and protected enrollment/directory credentials. Record prerequisites and actual coverage; do not label unavailable checks passed. |
| Running application | Exercise install/upgrade/removal while Clacks is open and while admitted saves are completing; observe files-in-use handling and normal close without timed killing. |
| Upgrade | Install an identified earlier numbered artifact, create offline pending work and protected credentials, then install the exact candidate. Confirm one product registration, updated files, preserved account/UI/configuration/pending work, and usable credentials. |
| Failed/cancelled upgrade | Exercise ordinary cancellation or a real environmental failure at a meaningful installation point; verify the old installation/data remain usable and record where cancellation occurred. Do not manufacture application database failures to test the installer. |
| Uninstall/reinstall | Remove the application; confirm installer-owned files/shortcuts are gone and data/credential entries remain. Reinstall as the same user and verify recovery of saved work and credential access. |
| Repeat/no-op | Re-run the same exact artifact and verify its standard maintenance behavior without duplicate entries or user-data changes; reject an older numbered release under the selected policy. |

Prepare two real, identified application artifacts with increasing source-owned
versions for upgrade acceptance. The first package establishes the baseline;
the final candidate is the exact artifact assessed. Do not fake an upgrade by
editing an installed executable or rewriting its database. When any required
environment or trust setting blocks a scenario, retain the concrete limitation
and leave issue #79 open. Native failures are resolved in scope or brought back
with evidence; cross-compilation, package construction and plans alone do not
complete the issue.

Evidence consists of ordinary workflow artifacts and concise verification notes
attached to the issue/PR. No archival system, retention policy, delivery-tracking
copy or permanent test-report collection is introduced. Future functionality
extends the same payload and pipeline when delivered.

## Implementation discretion and next artifact

The builder chooses coherent source/module placement, exact compatible resource
helper and notice tooling, initial upgrade GUID, icon artwork details, runner
image/action pins and focused verification commands within these boundaries.
These are routine integration choices, not a product approval queue. No new
storage model, synchronization rule or server deployment is authorized.

After this specification is approved, prepare the concrete implementation plan
before coding. The specification and plan do not close #79. Preserve unrelated
working-tree changes and obtain authorization appropriate to any subsequent
public publication or signing purchase.

[#79]: https://github.com/jhelwig/clacks-log/issues/79
[tool-release]: https://github.com/jkroepke/msi-builder/releases/tag/v1.2.0
[tool-schema]: https://github.com/jkroepke/msi-builder/blob/v1.2.0/docs/design/yaml-schema.md
[tool-close]: https://github.com/jkroepke/msi-builder/blob/v1.2.0/internal/builder/close.go
[tool-build]: https://github.com/jkroepke/msi-builder/blob/v1.2.0/docs/cli/msi-builder_build.md
[tool-results]: https://github.com/jkroepke/msi-builder/blob/v1.2.0/docs/spec/MSISPEC-RESULTS.md
[wixl-source]: https://github.com/GNOME/msitools/blob/v0.106/tools/wixl/builder.vala
[ai-features]: https://www.advancedinstaller.com/feats-list.html
[ai-licenses]: https://www.advancedinstaller.com/licenses.html
[wix-agreement]: https://github.com/wixtoolset/wix/blob/v7.0.1/OSMFEULA.txt
[nsis-template]: https://github.com/crabnebula-dev/cargo-packager/blob/main/crates/packager/src/package/nsis/installer.nsi
[restart-control]: https://learn.microsoft.com/en-us/windows/win32/msi/msirestartmanagercontrol
[reboot-control]: https://learn.microsoft.com/en-us/windows/win32/msi/reboot
[resource-helper]: https://docs.rs/winresource/0.1.34/winresource/
[workflow-runs]: https://docs.github.com/en/rest/actions/workflow-runs
[workflow-jobs]: https://docs.github.com/en/rest/actions/workflow-jobs

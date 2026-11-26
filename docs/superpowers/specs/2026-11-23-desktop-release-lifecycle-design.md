# Desktop release lifecycle

This design covers desktop release identity, publication and update notices for
Clacks Log. It builds on the existing [desktop packaging workflow] and the
[synchronization model].

## Release identity and build provenance

The desktop uses calendar versioning (`YY.M.N`). `YY` is the two-digit year,
`M` is the unpadded month, and `N` orders distinct release builds within that
month. Commit the numeric version before cutting a candidate and tag the
matching revision as `desktop-vYY.M.N`. A changed candidate gets a new version.
Promotion keeps the candidate's version, tag and installer bytes, including
when promotion falls in a later month. Both installer filenames use the
validated numeric version.

Development and one-off builds use the version in their source revision and
keep their existing GitHub Actions artifact delivery. The desktop exposes the
package version, source commit and available build provenance in About and
`--version`. Include the workflow run and attempt when available and the local
dirty-tree status when known. On Windows, a second development installer at the
same numeric version can replace the first in place while preserving local
workspaces, settings and protected credentials. A build's provenance identifies
which executable actually installed.

Desktop release tags and the server's `sync-server-v…` snapshot tags have
independent version sequences. Self-hosted snapshots use a matching server
Cargo package version. The hosted service continues deploying from `main` and
reports its actual revision in build provenance. Server snapshot distribution
is outside this design.

## Package, publish and promote

GitHub Releases are for the desktop application. Use the existing combined
packaging job and exact-commit checks, with `desktop-v*` as the release trigger.
A draft contains both native installers from the same source revision. The
initial targets follow the current packaging pipeline.

| Platform | Installer |
| --- | --- |
| Windows 11 x64 | Current-user Windows Installer (MSI) package. The initial release can use the unsigned pipeline. Add signing at the existing point before final inspection and hashing when signing is available. |
| macOS 26 Tahoe and newer on Apple Silicon | Developer ID signed, notarized and stapled disk image (DMG). Preserve the existing protected-credential provisioning. |

The maintainer publishes the draft as a GitHub prerelease for testers. To
promote it, clear the prerelease flag and explicitly mark that same release
Latest. The website's public download page shows the recommended final release
identified by GitHub Latest, with both installers and its release notes. The
website provides the user-facing download URL. GitHub can serve the installer
files.

To withdraw a recommendation, explicitly mark the previous usable final Latest
and add a withdrawal notice to the affected release. If this is the first final
release, publish a usable replacement for Latest. Keep the withdrawn release
and its installers accessible in release history. Desktops compare versions in
numeric order and offer upgrades only.

## Sync compatibility

The live handshake and the updater use the same definition of compatibility.
The desktop declares the sync contracts it implements and the deployment
declares the contracts it currently accepts. A shared contract permits a sync
session under that contract. Without a shared contract, sync requires a
compatible upgrade and local work remains available. Release versions identify
installers and deployment snapshots. The sync declarations carry compatibility.

Aim for at least 90 days of overlap on a best-effort basis so older desktops
can continue syncing with upgraded servers. Count the window from the point
when the replacement desktop is available as a final release on both platforms
and the server release that provides the overlap is available. A later server
deployment retires the old contract across that deployment. The server owns
migration of accepted shared data. A desktop upgrade preserves its workspace
and pending local edits for reconciliation when sync resumes.

### Proposed integration

Publish a `desktop-release.json` asset containing `sync_contracts`, generated
from the desktop's actual contract declarations. Use the validated numeric
release tag for version ordering and retrieve the metadata asset from that same
release. Have the running server expose its accepted contracts through a
read-only HTTPS lookup, backed by the same declarations the live handshake
uses. The handshake can select a common contract using WebSocket subprotocol
negotiation before the existing sync exchange. The current protocol is the
initial contract for both sides.

## Desktop update experience

Check GitHub Latest on **Check for Updates** and when initiating a sync server
connection. Run the release lookup independently of sync so a slow or
unavailable release source does not hold up synchronization. Automatic and
manual checks consider final releases only.

| Check result | Desktop behavior |
| --- | --- |
| Final release is newer and the workspace has no configured sync server | Offer the release. |
| Final release is newer and shares a contract with the configured server | Offer the release. |
| Final release is newer and has no common contract with the configured server | Explain that the sync server needs updating before the user can install the latest desktop release. |
| Final release is newer and compatibility could not be verified | Explain the uncertainty on a manual check. Withhold the automatic recommendation. |
| Installed version is equal to or newer than the recommended final | A manual check reports that no newer release is recommended. |
| Latest release information could not be obtained or validated | A manual check reports that the update check could not complete. |

The download action opens the project's website for the **exact release the
app checked**. The page identifies that release and links to its installers.
If it has been withdrawn, show the withdrawal notice and direct the user back
to **Check for Updates**. Its installers remain accessible through release
history. A newer release becoming Latest while the browser opens leaves the
link pointed at the checked release.

For MVP, dismissing an automatic notice suppresses automatic notices for that
release on the installation, including after restart. A subsequent newer
release is eligible for an automatic notice. A manual check always reports
the current result, including for a dismissed release.

## Implementation boundaries and verification

Keep the desktop worker and sync negotiation observable through the existing
[observability contract]. Record the check trigger and outcome, correlate one
check across release lookup and compatibility lookup, and record the selected
contract or mismatch at the handshake boundary. Measure update-check attempts
and duration with bounded trigger and outcome labels. Keep server URLs, account
details, credentials and release-note content out of these events and metrics.
Emit at operation boundaries so reconnects and request failures remain
diagnosable without per-message instrumentation.

Verify candidate promotion preserves both installer assets and that changing
Latest changes the website's general recommendation. Exercise the update
decision table, dismissal across restart, and the exact-release link if Latest
changes or the release is withdrawn. Exercise a shared and a disjoint sync
contract at the live handshake and through the pre-install check. Verify that
the packaged application exposes its intended version and build provenance.

[desktop packaging workflow]: ../../../.github/workflows/desktop-release.yml
[synchronization model]: ../../synchronization-model.md
[observability contract]: ../../observability.md

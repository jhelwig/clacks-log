# Desktop sync compatibility and automated releases

This draft addresses [issue #73] and updates the compatibility and release
process in the [desktop release lifecycle design]. It covers public deployment
capabilities, live contract selection, generated desktop metadata, the download
page's compatibility check and automated desktop releases.

## Intent and scope

Before installing a prospective desktop release, a client can determine whether
that release shares a sync contract with its configured deployment. Live sync
uses the same implemented desktop support and accepted server contracts.
Application versions identify releases; sync contracts establish compatibility.
Local work remains available when sync cannot connect.

Deployment capabilities are public and contain no account information. The
[public download design] assigns compatibility with the serving deployment to
this issue and keeps downloads available regardless of the result.

This issue also automates release preparation, tagging, native packaging,
prerelease publication and final promotion. In-app update notices, installation
and dismissal behavior remain separate desktop update work.

## Existing implementation and chosen approach

`clacks-sync::protocol::messages::VERSION` is currently `1`. The desktop sends
that number in `ClientMessage::Hello`; the server checks it after decoding the
message and returns it in `ServerMessage::Hello`. The server authorizes
`/api/sync` before upgrading to WebSocket.

The alternatives considered were extending the opening sync message or selecting
a contract through WebSocket subprotocol negotiation. The user selected the
latter: it selects the contract before decoding contract-specific sync messages
and uses existing transport facilities. [Axum 0.8.9] supplies subprotocol
selection. The desktop already uses tokio-tungstenite for the connection.

The existing packaging workflow builds both native installers from one revision
and creates a draft release. It supplies source/package identity checks and
platform packaging operations that this work can adapt.

## Contract identity and ownership

Use `clacks-sync-v1` for the current native sync representation and interpretation,
including synchronized values and recovery rules. Keep its identity with the
implementation in `clacks-sync::protocol`.

The `SyncContract` enum determines implemented support. The desktop enumerates
its variants for its offers and generated metadata; the server enumerates them
for public capabilities and live selection. Both endpoints exhaustively dispatch
the selected variant to its implementation. There is no separately maintained
contract list on either endpoint. The server uses enumeration order when
selecting among client offers, so desktop offer order does not determine
selection. This issue implements the current contract using the existing sync
code. Future protocol implementations and transitions are later work.

## Public deployment discovery

Provide `GET /api/sync/capabilities` returning:

```json
{"sync_contracts":["clacks-sync-v1"]}
```

The array contains the deployment's accepted contracts in server preference
order. Match identifiers exactly. The handler performs no account lookup and
returns deployment capabilities without requiring authentication or including
account data or secrets.

Return `Cache-Control: no-store` to prohibit private and shared HTTP caches from
storing or reusing the response, including automatic browser and intermediary
caching ([RFC 9111 section 5.2.2.5]). Each new discovery check contacts the
deployment; Clacks does not satisfy it from an application cache.

Clients start at the capabilities path on their configured server, omit
credentials and use normal bounded HTTPS redirects with certificate validation
throughout. A redirect can change the lookup origin without changing the
configured sync destination. Lookup failures mean compatibility is unknown.

A shared contract means protocol compatibility with the observed deployment.
It does not establish account access or guarantee that the deployment will
remain unchanged.

## Live selection and exchange

1. The desktop sends its authenticated `/api/sync` upgrade request, offering
   its implemented contracts in `Sec-WebSocket-Protocol`.
2. The server performs its existing credential and account authorization.
3. Using Axum's subprotocol facilities, the server selects the first contract
   in its accepted preference order that appears in the desktop's offer. It
   returns that value in the upgrade response's `Sec-WebSocket-Protocol` header.
   With no match, including a missing offer, it refuses the upgrade using the
   existing HTTP error-response conventions.
4. The desktop retains the selected contract and its implementation for the
   connection before exchanging sync messages. Failure to establish that binding
   is an ordinary setup error. Carry the successful binding into the exchange
   using the WebSocket client's results and existing protocol facilities.
   Preserve existing authorization response processing, including
   proof-of-possession nonce handling.
5. Both sides use the selected contract for the sync exchange. Initial messages
   still introduce authenticated identity, coverage and recovery information.
   Remove their duplicate numeric `version` fields and version-equality checks.

Selection follows [RFC 6455 section 4.2.2]. Axum allows upgrading without a
selected subprotocol, so Clacks must refuse unmatched offers explicitly.
Connection failure preserves enrollment, local values, pending work and
comparison information. Ordinary offline work remains available.

## Generated desktop metadata

Generate `deploy/desktop/sync-contracts.json` from the same implemented-contract
definitions used by desktop negotiation:

```json
{"sync_contracts":["clacks-sync-v1"]}
```

The file is published output. It does not determine executable support.
The packaging helper deterministically generates bytes by enumerating
`SyncContract` and serializing its identifiers. Its regeneration command writes
those bytes; the normal freshness test reads the same in-memory output.

Commit generated changes with the owning code change. Ordinary continuous
integration (CI) runs the same normal packaging test as local development:
compare shared in-memory generation with the on-disk file and fail if it is
missing or differs. Verification does not modify the file. Release preparation
uses that normal CI result and fails on stale metadata instead of repairing it.

The release tag identifies the committed metadata for a prospective release.
Existing exact-source and package identity checks connect that source to the
native installers; no additional executable-report command is required.

## Automated release process

Build on GitHub Actions, pull requests, tags and releases. Workflow structure,
job boundaries and packaging actions can change to implement this process.
Keep the existing geographic payload, platform prerequisites and signing policy,
including macOS notarization/stapling and the ability to add Windows signing.

### 1. Prepare a candidate

The maintainer starts a release workflow with a source revision and desktop
version `YY.M.N`. The workflow prepares the version change in
`bin/clacks-log/Cargo.toml` through the normal review and merge process. If the
committed version is already correct, no empty change is needed. A version-only
change leaves sync-contract metadata unchanged.

Starting this workflow requests preparation and prerelease publication. Final
promotion is a later maintainer action.

Desktop release preparation is a manual-only workflow. A separate Desktop
release continuation workflow handles successful main-push verification. It
looks up the merged release request before source checkout or a Rust tooling
build. Ordinary merges skip the release job.

### 2. Tag the release source

After preparation lands on `main`, automation uses the resulting commit as the
release source. It reuses the existing Linux/web checks for that exact commit,
including the normal metadata-generation check, validates the release version
and creates `desktop-vYY.M.N`. An existing tag for different source is an error.

The continuation workflow passes the selected request branch and verified
commit to the helper. The helper reads that checkout's application version and
compares the request branch with the branch derived from that version before
tagging. The workflow owns merged-PR selection; the helper does not repeat the
lookup.

Automation then starts native packaging for that tag. Account for [GitHub token
event behavior]: a tag created with `GITHUB_TOKEN` does not itself trigger the
existing tag-push workflow. Use supported workflow composition or explicit
dispatch so the process continues without manual tagging or job invocation.

### 3. Build the native installers

Build Windows and macOS from that tagged revision using the existing source,
package identity, signing and platform checks. Reuse normal package hashes and
build provenance.

Use the following installer and user-facing Actions artifact names. The artifact
name is the installer filename without its extension. For version `26.11.1`:

| Build | Installer filename | Actions artifact name |
| --- | --- | --- |
| Windows release candidate/final | `Clacks-Log-26.11.1-Windows-x64.msi` | `Clacks-Log-26.11.1-Windows-x64` |
| macOS release candidate/final | `Clacks-Log-26.11.1-macOS-Apple-Silicon.dmg` | `Clacks-Log-26.11.1-macOS-Apple-Silicon` |
| Windows development | `Clacks-Log-26.11.1-Development-Windows-x64.msi` | `Clacks-Log-26.11.1-Development-Windows-x64` |
| macOS development | `Clacks-Log-26.11.1-Development-macOS-Apple-Silicon.dmg` | `Clacks-Log-26.11.1-Development-macOS-Apple-Silicon` |

Release files use the committed candidate version; development files use their
source version. Source revisions, run IDs and attempts remain in About,
`--version` and normal workflow output. Apply naming changes to package output,
artifact upload/download references, release assembly and download-page selection.
Internal verification/geography artifacts remain workflow details.

### 4. Publish the candidate

Prepare release notes from the candidate's changes and assemble both installers
in one draft GitHub release. Publish it automatically as a prerelease, with
Latest disabled, after both native builds and uploads succeed. A failed or
incomplete build/upload leaves it unpublished. Use the files produced for this
candidate; existing package identity checks apply.

### 5. Test the candidate

Test the distributed installers using the existing Windows and macOS native
checks relevant to the release. Automate checks that can run unattended and
perform installation or operating-system interaction where needed. Normal test
results and release notes suffice; this process adds no acceptance record format
or evidence-submission requirement.

If a correction changes the candidate, prepare a new version. Keep the candidate
being tested and its installer files intact.

### 6. Promote to final

When satisfied with the candidate, the maintainer invokes its promotion workflow.
This is the maintainer's confirmation of acceptance and decision to release.
Automation checks the selected candidate and installer identities using existing
release/build information and hashes, then marks that same release final and
Latest. A mismatched or incomplete candidate fails promotion.

Promotion preserves the tag, filenames and installer bytes. It does not rebuild
or replace files. The download page then discovers the new final through GitHub
Latest and evaluates compatibility using that release's metadata.

The implementation plan owns workflow inputs, continuation after review/merge,
permissions and release-note generation. Update platform release documentation
to describe the workflow entry points and useful failure diagnostics.

## Download-page compatibility check

The public `/download` page compares its displayed release's contracts with the
accepted contracts of the deployment serving the page. Fetch
`/api/sync/capabilities` from that origin with credentials omitted.

Keep GitHub reads in the browser. Retrieve the metadata for the exact release tag
already selected for the version, installers and notes through the Repository
Contents API, with `Accept: application/vnd.github.raw+json`:

`https://api.github.com/repos/jhelwig/clacks-log/contents/deploy/desktop/sync-contracts.json?ref=desktop-vYY.M.N`

Use the selected tag for this request, without another Latest lookup. Omit
credentials and parse the returned declaration. This uses the browser-readable
repository route established during #72 instead of a release-asset download or
server relay.

Show the release and installer links independently of the compatibility lookup.
Place a concise status beside the release information:

| Result | Page behavior |
| --- | --- |
| Lookup pending | Indicate that compatibility with this server is being checked. |
| Shared contract | State that this release is sync-compatible with this server. |
| Valid declarations with no shared contract | State that this release cannot sync with this server; local offline use remains available. |
| Missing, invalid or unavailable declaration | State that compatibility could not be verified. |

Keep available downloads and release history accessible in every state. Preserve
the page's existing handling of no final release, invalid release information and
missing individual installers. A compatibility result is associated with the
displayed release, even if GitHub Latest subsequently changes.

## Future transition and retirement

Keep the best-effort target of at least 90 days of overlap from the lifecycle
design, counted once the replacement desktop is final on both platforms and the
server release providing overlap is available. This is release guidance, not a
runtime timer. Retire an older contract through a server deployment, changing
public capabilities and live acceptance together. Existing server migration
and desktop workspace/pending-edit preservation rules continue to apply.

## Diagnostics

Follow the [observability contract]. Extend existing connection diagnostics to
report the selected contract or incompatibility and reuse existing request
correlation, duration metrics and failure handling. Keep secrets out of logs and
arbitrary client offers out of metric labels. Public discovery uses normal HTTP
diagnostics without account enrichment.

The download page shows pending and failure outcomes directly, as it does for
release lookup. Release workflows use ordinary Actions logs and useful error
messages identifying the candidate and failed operation.

## Verification

Keep checks that detect mistakes in Clacks's code or workflow wiring. Use
existing suites, normal CI and platform packaging checks where they cover the
failure. Do not add parallel validators or tests of documented dependency behavior.

### Automated checks

- Exercise public capabilities through the routed server and use an authorized
  sync connection to establish that the advertised contract works and an
  unsupported or missing offer is refused before sync messages are exchanged.
- Exercise the desktop's real offer and use of the selected implementation in
  existing sync scenarios. Retain checks for local-work preservation where the
  changed connection handling could affect it.
- Run the normal packaging freshness test comparing shared in-memory generation
  with the on-disk metadata without modifying it. Use the same contract
  definitions for live offers and generation.
- Update existing package identity and installer-selection checks for the new
  names, including selection of the correct platform and release version.
- Exercise the page's shared/disjoint/unknown comparison and association with the
  displayed release. Failed metadata lookup must leave downloads available.
- Check meaningful selected-contract and failure diagnostics through those same
  operations, following the existing observability policy.

Release checks reuse exact-source CI and package identity results. The new
workflow wiring must start packaging after tagging, wait for both installers
before publication and preserve the candidate on promotion. Exercise those paths
with genuine releases; do not create dummy releases or add mocked-command tests
that only restate GitHub's behavior. Report hosted steps not yet exercised plainly.

### Browser and native checks

During download-page integration, use a real browser to verify the repository
path/tag request, cross-origin metadata read, credential omission and displayed
status. Use normal browser redirect handling. This is feature verification,
not a recurring release gate.

Test the relevant installed-product behavior on Windows and macOS using the
candidate files. Passing source tests or packaging checks does not replace
installation and operating-system checks that need the native environment.
The maintainer decides when the candidate is ready to promote.

## Owning documentation

Update the sync integration, lifecycle and platform guides where they describe
the old opening-message version checks, release-asset metadata, manual release
steps or old installer names. The roadmap and GitHub issues continue to own
scope and delivery tracking.

[issue #73]: https://github.com/jhelwig/clacks-log/issues/73
[desktop release lifecycle design]: 2026-11-23-desktop-release-lifecycle-design.md
[public download design]: 2026-11-26-public-landing-and-download-design.md
[RFC 6455 section 4.2.2]: https://www.rfc-editor.org/rfc/rfc6455.html#section-4.2.2
[Axum 0.8.9]: https://docs.rs/axum/0.8.9/axum/extract/ws/struct.WebSocketUpgrade.html#method.protocols
[GitHub token event behavior]: https://docs.github.com/en/actions/concepts/security/github_token#when-github_token-triggers-workflow-runs
[RFC 9111 section 5.2.2.5]: https://www.rfc-editor.org/rfc/rfc9111.html#section-5.2.2.5
[observability contract]: ../../observability.md

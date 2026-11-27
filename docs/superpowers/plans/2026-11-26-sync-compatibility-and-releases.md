# Sync Compatibility and Automated Desktop Releases Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver #73's public compatibility checks and complete automated desktop release flow.

**Architecture:** Shared Rust contract definitions drive desktop offers and normal metadata generation; server acceptance remains independently owned. Axum selects a WebSocket subprotocol, while the browser compares deployment capabilities with metadata from the displayed release tag. Extend existing packaging tooling and GitHub workflows for version preparation, publication and promotion.

**Tech Stack:** Rust 1.99.1, Axum 0.8.9, tokio-tungstenite 0.29.0, Serde, Leptos, gloo-net 0.6, GitHub Actions and GitHub CLI.

**Spec:** [Desktop sync compatibility and automated releases](../specs/2026-11-26-sync-compatibility-design.md). Read it with this plan.

## Global constraints

- Initial contract: `clacks-sync-v1`. Use the existing native protocol implementation; add no historical codecs, registry or generic protocol framework.
- Generated output: `deploy/desktop/sync-contracts.json`. The command and normal packaging test share in-memory generation. The test compares those bytes with the on-disk file without modifying it; ordinary CI runs the same test against its checkout. Release preparation reuses those results.
- Desktop versions: `YY.M.N`; tags: `desktop-vYY.M.N`. Development builds keep their source version. Promote the same candidate and installer files.
- Keep existing source/package identity, geography and signing behavior. Packaging actions and job structure may change. Use the spec's exact friendly release/development filenames and Actions artifact names.
- No desktop capability-report command, per-platform support comparison, metadata relay, per-release retrieval check, credential-state discovery tests, separate redirect test or acceptance-evidence inputs.
- Use existing Rust tooling, suites and workflow facilities. Add a test only for a concrete Clacks decision not already covered. No test of Git, JSON serialization, browser redirects or GitHub's documented behavior.
- Follow existing observability and isolated-development guidance. Database tests use disposable databases. Do not launch development desktop builds against normal app data.
- Treat tasks as coherent logical changes. If committing is authorized, use reviewed message files, substantive bodies and physical 72-column prose wrapping; fold corrections into their owning commits.

## Review focus

1. Missing or stale metadata must fail the normal packaging test comparing in-memory generation with the on-disk file; publishing cannot repair it (Task 1, local tests and existing CI).
2. Advertised acceptance must agree with the authenticated upgrade, including absent/disjoint offers (Task 2, existing production HTTP scenario).
3. A failed metadata request or a changed Latest must not hide downloads or associate the wrong contracts with displayed installers (Task 3, release-mapping scenario and browser integration).
4. Automated tagging must actually start release-mode packaging of the merged source, rather than development-mode packaging or the workflow's default-branch revision (Task 4, genuine workflow run).
5. Incomplete delivery must stay unpublished, and promotion must retain the selected candidate's files (Tasks 4–5, actual candidate publication/promotion).

## Current integration points

The workspace already builds `clacks-desktop-package` in ordinary Linux CI.
`lib/clacks-desktop-packaging` owns Cargo/tag validation and reuse of exact-commit
Linux/web results. `lib/clacks-web/tests/pages.rs` already creates a real server,
provider, account and proof-bound access token; extend it rather than duplicating
that setup. Native desktop sync tests also exist under enrollment and exchange.

The generator will be a focused subcommand of the existing packaging tool,
using the same `clacks-sync` definitions as live offers. This is normal Rust
maintenance tooling, not a command on the desktop app or a native-build report.
Document its regeneration command with ordinary developer checks. The normal
packaging test checks freshness locally and in existing CI without writing output.

GitHub's repository option **Allow GitHub Actions to create and approve pull
requests** is enabled, verified on 2026-11-26
(`can_approve_pull_request_reviews=true`). Default workflow permissions remain
read-only. The built-in token can create the version PR when the workflow
requests its write scopes. The workflow does not approve/merge PRs or use a new
personal token. Normal branch review/CI rules remain in place.
Bot-created PR runs may require GitHub's workflow-run approval; do not silently
work around that platform behavior or claim it has been exercised.

---

### Task 1: Implemented contracts and normal metadata generation

**Files**

- Create `lib/clacks-sync/src/protocol/contracts.rs` and `deploy/desktop/sync-contracts.json`.
- Modify `lib/clacks-sync/src/protocol.rs`, its `Cargo.toml` and the root workspace dependencies.
- Create `lib/clacks-desktop-packaging/src/sync_contracts.rs`; modify its `src/lib.rs`, `src/cli.rs` and `Cargo.toml`.
- Modify `README.md` and `lib/clacks-sync/README.md`. Existing CI already runs packaging tests; no workflow generation step is needed.

**Interfaces**

- `contracts::SyncContract`: a documented enum representing implemented contracts, initially just `V1`, deriving `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq` and `strum::EnumIter`. `SyncContract::iter()` enumerates the variants in declaration order through `strum::IntoEnumIterator`, providing deterministic enumeration for desktop offers and generated metadata. Desktop offer order does not determine selection. There is no separately maintained implemented-contract list.
- `SyncContract::id(self) -> &'static str` uses an exhaustive match, with `V1` mapped to `clacks-sync-v1`. `SyncContract::from_id(id: &str) -> Option<Self>` finds the matching value through `Self::iter()` and `id()`, so the identifier mapping is defined once.
- `sync_contracts::generate() -> anyhow::Result<Vec<u8>>` constructs a private Serde-serializable declaration with `sync_contracts: Vec<&'static str>`, populated by `SyncContract::iter().map(SyncContract::id).collect()`. Serialize that value using `serde_json::to_vec_pretty`, then append a newline. The initial output is `{"sync_contracts":["clacks-sync-v1"]}` in pretty JSON; this is the expected result, not a JSON literal to write or a second source of contract identifiers. This operation performs no filesystem I/O.
- `sync_contracts::write(output: &Path) -> anyhow::Result<()>` writes the bytes returned by `generate()` to the requested path. It contains no separate declaration construction or serialization path.
- CLI: `clacks-desktop-package sync-contracts --output PATH`.

- [ ] Add the contract definitions to the owning sync module. Use workspace dependency `strum` 0.28.0 with its `derive` feature, already present in the lockfile, for enum iteration. Associate `V1` with the current implementation; do not introduce handlers for hypothetical contracts.
- [ ] Add `clacks-sync` as a workspace-inherited dependency of the packaging library and implement the generator subcommand with the enum-derived declaration and serialization path above. Desktop offers in Task 2 use the same `SyncContract::iter().map(SyncContract::id)` path. Contract identifiers belong only in the enum's `id()` mapping; the generator contains no contract-specific strings or manually maintained list.
- [ ] Generate and commit the output with this logical change. Normal developer regeneration is:

  ```sh
  cargo run --bin clacks-desktop-package --locked --offline -- sync-contracts --output deploy/desktop/sync-contracts.json
  ```

- [ ] Add normal packaging test `sync_contracts::tests::committed_metadata_matches_implemented_contracts`. Read `deploy/desktop/sync-contracts.json` relative to `env!("CARGO_MANIFEST_DIR")` and compare its bytes directly with `generate()` output. A missing file or differing bytes must fail with the regeneration command above. This tests freshness of Clacks's published declaration, not Serde behavior. The test neither writes the file nor uses a temporary output, Git status or a duplicated expected contract list. An up-to-date uncommitted file passes locally and remains an ordinary Git change; CI reads the committed checkout. Existing packaging-test execution covers CI without generation/diff or tracked-file checks.
- [ ] Document regeneration in the root development checks. The standalone command also makes regeneration reliable with persistent Cargo build caches; no build-script self-triggering or forced native rebuild is needed.
- [ ] Run `cargo test -p clacks-desktop-packaging --locked --offline`; the freshness test and existing packaging scenarios must pass. Keep dependency tables sorted with the pinned `cargo sort` command.

**Done when:** live offers can use an implemented contract value, and normal packaging tests detect missing or stale publication metadata locally and in CI without modifying it or executing the desktop app.

### Task 2: Public capabilities and live subprotocol selection

**Files**

- Modify `lib/clacks-server/src/synchronization.rs`; create `lib/clacks-server/src/synchronization/contracts.rs`.
- Modify `lib/clacks-server/src/synchronization/connection.rs` and `lib/clacks-sync/src/protocol/messages.rs`.
- Modify `lib/clacks-desktop/src/synchronization.rs` and `synchronization/exchange.rs`.
- Update existing `synchronization/tests.rs`, `synchronization/exchange/tests.rs` and native enrollment sync helpers for the changed Hello messages.
- Extend `lib/clacks-web/tests/pages.rs` and its existing Cargo dev-dependencies for the real WebSocket client.
- Update `docs/synchronization-integration-proposal.md`, relevant `docs/synchronization.md` wording, and sync/server/desktop crate guides.

**Interfaces**

- Server discovery and Axum selection both enumerate `SyncContract::iter().map(SyncContract::id)`, using the same implementation source as desktop offers and generated metadata. There is no separately maintained server contract list. The server selects the first contract in its enumeration order that the desktop offers, regardless of desktop offer order.
- Public handler: `GET /api/sync/capabilities`, JSON field `sync_contracts`, response `Cache-Control: no-store`; no account-state extractor or authorization call.
- Private desktop `Connected { socket: Socket, contract: SyncContract }`; `connect(...) -> anyhow::Result<Connected>` keeps its existing arguments.
- On both server and desktop, the connection/exchange boundary exhaustively matches the retained `SyncContract`, with `V1` invoking the existing exchange implementation. The selected value determines the call; there is no wildcard arm or unconditional exchange call after merely recording the selection. No dynamic dispatch framework is needed.

- [ ] Extend the existing production HTTP scenario with capabilities retrieval and authenticated upgrade cases. Reuse its actual issued token and fresh proof-of-possession proof for each upgrade, using the HTTPS proof target representation already used by `/api/client`. Use a temporary workspace for any local state.
- [ ] Assert the initial public JSON is `{"sync_contracts":["clacks-sync-v1"]}`. For an authorized upgrade, offer `clacks-sync-v1`, assert that selection, then send the ordinary initial Hello and receive its normal reply. A disjoint offer such as `clacks-sync-unsupported`, and an absent offer, must be refused before sync messages. Do not create discovery requests with credentials.
- [ ] Run that scenario before implementation: the missing route/selection should fail the new assertions. Its existing login and database setup already covers protected-route authorization.
- [ ] Mount capabilities through the existing native sync router, which is merged outside cookie/session route layers in `clacks-server/src/http.rs`. Serialize IDs from `SyncContract::iter().map(SyncContract::id)`. Add the workspace-inherited `strum` dependency to the server and import `strum::IntoEnumIterator`. Use standard Axum response headers for `no-store`.
- [ ] In `upgrade`, retain existing authorization, configure `WebSocketUpgrade::protocols(SyncContract::iter().map(SyncContract::id))`, and consume `selected_protocol()` as an implemented `SyncContract`. Refuse an absent match with HTTP 400 and the existing JSON error shape (`error: "sync_contract_incompatible"` plus a safe description). Use ordinary response branching; no new authorization-error variant is needed for incompatibility.
- [ ] Pass the selected binding into the server socket handler and dispatch through the exhaustive contract match above. Add selection/mismatch fields to the existing connect/upgrade diagnostics; reuse existing request metrics and redaction.
- [ ] Add the desktop offer to the existing authenticated request using `SyncContract::iter().map(SyncContract::id)` from Task 1. Consume the successful response's selected header into `Connected`; propagate any inability to resolve the binding through the ordinary connection error. Preserve authorizer response/nonce processing. Do not assert that handshake success makes later header access infallible.
- [ ] Consume `Connected` in the desktop worker, retaining the socket and selected implementation together through setup, and dispatch through the same exhaustive-match rule. Reuse tungstenite's normal handshake results; no second validation pass or library-validation tests.
- [ ] Remove `messages::VERSION`, both Hello `version` fields and their equality checks. Remove the old wire-level incompatible failure category if it has no remaining consumer after upgrade refusal takes over. Preserve account/actor validation, checkpoints, lineage, recovery and all pending-work handling. Replace the old unsupported-Hello-version scenario with the real upgrade refusal already covered above.
- [ ] Run targeted checks with the existing disposable PostgreSQL role/schema setup:

  ```sh
  SQLX_OFFLINE=true LEPTOS_OUTPUT_NAME=clacks-web cargo test -p clacks-web --features ssr --test integration pages::account_page_and_direct_server_function_require_the_same_login --locked --offline
  SQLX_OFFLINE=true cargo test -p clacks-desktop --lib synchronization::exchange::tests:: --locked --offline
  ```

  Expected: the real public/upgrade assertions and existing exchange scenarios pass. Native enrollment tests require their existing platform/key-store prerequisites; report that separately if unavailable. Do not launch a default desktop workspace.
- [ ] Review that existing local-work/reconnect scenarios still exercise the changed setup path. Add a new scenario only if those checks leave a specific Clacks state-loss risk uncovered. Update owning docs to describe subprotocol selection and the unchanged recovery exchange.

**Done when:** public acceptance agrees with actual upgrade admission, and desktop sync carries a usable selected contract into the existing exchange.

### Task 3: Friendly package names and download-page compatibility

**Files**

- Create `lib/clacks-desktop-release/Cargo.toml` and `src/lib.rs` for pure release-version and package-naming operations shared by packaging and the browser.
- Modify `lib/clacks-desktop-packaging/src/metadata.rs`, `metadata/tests.rs` and `.github/workflows/desktop-release.yml`.
- Modify packaging `src/cli.rs`/`Cargo.toml` to use the shared platform type and rules.
- Modify `lib/clacks-web/src/pages/download.rs`, `download/release.rs`, `lib/clacks-web/Cargo.toml`, root `Cargo.toml`/lockfile and `lib/clacks-web/style/account.css` as needed.
- Create `lib/clacks-web/src/pages/download/compatibility.rs`.
- Update `docs/superpowers/specs/2026-11-26-public-landing-and-download-design.md` and the web guide where filenames/source handling change.

**Interfaces**

- Pure `clacks-desktop-release` owns `ReleaseVersion`, `PackagePlatform`, and the filename templates. It has no command execution, filesystem, network or packaging-tool dependencies. Both packaging and web depend on it through workspace dependencies; release identity does not belong in the recorded-contact model or sync-contract module.
- `ReleaseVersion::parse(value: &str) -> anyhow::Result<Self>` validates and retains the canonical version spelling: exactly two ASCII year digits, unpadded month 1–12, and an unpadded decimal build number 0–65535. `as_str(&self) -> &str` exposes that spelling; `tag(&self) -> String` adds `desktop-v`; `from_tag(tag: &str) -> anyhow::Result<Self>` removes that exact prefix and calls `parse`. Preparation, packaging, browser selection and promotion all use these operations; remove their separate version/tag validators.
- Move existing `PackagePlatform { WindowsX64, MacosArm64 }` into that library and derive `strum::EnumIter`. Its variants enumerate distributed installer platforms; promotion uses this enumeration rather than a second platform list or fixed-size asset array. Its `installer_name(self, version: &str, development: bool) -> String` owns both platform suffixes and insertion of `Development`, producing the spec's exact filenames. The version argument comes from the application's Cargo package during packaging or `ReleaseVersion::as_str()` during release selection. It permits development's existing source version, including `0.1.0`, without imposing release-calendar validation on development builds. Implement standard `FromStr` for the existing CLI spellings `windows-x64` and `macos-arm64`; use Clap's normal type parser instead of a duplicate CLI enum or `ValueEnum` dependency in this library.
- Extend existing `metadata::BuildMetadata` with `installer_name: String` and `artifact_name: String`. Existing `prepare(...)` reads the application version once, validates any supplied tag through `ReleaseVersion`, and calls the shared platform naming operation with development determined by tag absence; keep its signature. Derive `artifact_name` from that resulting filename's stem, rather than another naming template.
- Extract `metadata::application_version(input: &serde_json::Value) -> anyhow::Result<&str>` from existing `prepare`: select Cargo package `clacks-log` and return its version. Both package preparation and Task 4 candidate resolution consume this operation.
- `DesktopRelease` retains one `version: ReleaseVersion` parsed from the single selected Latest response. Its canonical spelling preserves the exact selected tag through `version.tag()` and supplies the display version through `version.as_str()`; do not store independently editable tag/version strings.
- Private browser input `ContractDeclaration { sync_contracts: Vec<String> }` reads public IDs as opaque strings; an older web build must still compare a future contract ID it does not implement.
- Private `Compatibility { Compatible, Incompatible, Unknown }` and `compare(release: Option<&ContractDeclaration>, deployment: Option<&ContractDeclaration>) -> Compatibility`. Missing decoded input means `Unknown`; two valid declarations determine shared/disjoint status.
- Hydration-only `load_compatibility(tag: &str) -> Compatibility` fetches the serving deployment and that tag's committed metadata; request/HTTP/decode errors return `Unknown`.

- [ ] Move version/tag and filename rules into the shared library and wire existing packaging and browser consumers to them. Extend the existing package-metadata and release-mapping scenarios with the spec's exact names and the padded-month mismatch identified in review: `26.01.1`/`desktop-v26.01.1` are rejected by both consumers. Retain the distinct application/library versions in the fixture so these checks detect selecting the wrong Cargo package. Do not add standalone string-concatenation tests.
- [ ] Have the native workflow use helper-produced installer/artifact names for both release and development downloads. Remove SHA/run suffixes from user-facing names while preserving existing About/`--version` provenance. Update every matching upload/download/assembly reference.
- [ ] Update the existing web installer-selection test and mapping to parse the response tag through `ReleaseVersion::from_tag` and compute each expected filename through `PackagePlatform::installer_name` in release mode. Keep exact tag/version association and partial-installer behavior. Use the original selected response tag for asset URLs and metadata retrieval, and derive the displayed version from its parsed value; do not reimplement templates or parse version components locally.
- [ ] Preserve the selected release identity in `DesktopRelease`. Build the Contents API request from its exact canonical tag and `deploy/desktop/sync-contracts.json`, using `Accept: application/vnd.github.raw+json`. Do not fetch Latest again. Register the compatibility module for hydration and tests, keeping its fetch function hydration-only like `load_latest`.
- [ ] Add optional hydration-only `web-sys` 0.3.106 (already locked) access for `RequestCredentials` and `RequestCache` through workspace dependencies. Use `gloo-net` with credentials omitted for both requests and `RequestCache::NoStore` for deployment discovery. Keep normal Fetch redirect handling. Add no server-side GitHub fetch, custom redirect policy or native update-check consumer.
- [ ] In `ReleaseDetails`, start compatibility in its own Leptos `LocalResource`; render existing installer links and release notes independently. Show the spec's pending/compatible/incompatible/unknown meanings beside release information, using existing styling and status semantics. No new page layout or download blocking.
- [ ] Extend the existing web selection/presentation checks for shared, disjoint and unknown results and the fixed selected-tag request. If one small new scenario is needed, keep it in the existing web test target. Do not test JSON or set-intersection mechanics in isolation.
- [ ] Run:

  ```sh
  cargo test -p clacks-desktop-packaging --locked --offline
  cargo test -p clacks-web --lib --locked --offline
  SQLX_OFFLINE=true LEPTOS_OUTPUT_NAME=clacks-web cargo leptos build --release
  ```

  Expected: updated package/release-selection assertions pass and server rendering plus WASM hydration compile together.
- [ ] During browser integration, inspect actual request path/tag and credential omission, cross-origin retrieval, result presentation and continued download access. Include a failed retrieval and a changed-Latest association using representative inputs where no real final exists. Use existing browser tooling; build no new comparison runner or redirect harness. This is feature verification, not a release gate.

**Done when:** every download uses the agreed naming, and the page compares the displayed candidate's metadata with the serving deployment without delaying or hiding downloads.

### Task 4: Automated version preparation, tagging and prerelease publication

**Files**

- Create `.github/workflows/desktop-release-prepare.yml`.
- Extend `.github/workflows/desktop-release.yml` for continuation and explicit release-mode dispatch.
- Create `lib/clacks-desktop-packaging/src/release.rs` for version preparation and candidate lookup; extend `src/cli.rs`/`src/lib.rs`.
- Modify packaging/root Cargo manifests and lockfile for the existing locked `toml_edit` 0.25.13 family.

**Interfaces and workflow entry points**

- Use Task 3's `ReleaseVersion` for requested release versions and tags, and `metadata::application_version` for the committed application version. Do not add another release validator.
- `release::set_desktop_version(manifest: &Path, version: &str) -> anyhow::Result<VersionPreparation>` updates only the application package version. Its serialized result contains `changed: bool` and the request `branch: String`, derived from the validated `ReleaseVersion::tag()` within the release module. The workflow consumes that result rather than constructing its own tag/branch spelling.
- CLI: `clacks-desktop-package release-version --manifest PATH --version YY.M.N`.
- `release::Candidate { revision: String, version: ReleaseVersion }`; compute its tag with `version.tag()` instead of storing independently editable version/tag strings. The revision is an exact commit, not a branch name.
- `release::candidate_for_push(repository: &str, revision: &str) -> anyhow::Result<Option<Candidate>>` identifies a merged release-preparation PR associated with that exact main push. The caller checks out that revision before invoking it. Use GitHub's commit-associated-PR API to identify the release request, then normal Cargo metadata from that checkout and `metadata::application_version` to obtain the actual application version. Parse it with `ReleaseVersion`; the PR branch identifies the request and must agree with the derived tag, but supplies neither executable version nor release source. No custom persisted request table.
- Preparation dispatch inputs: `source` (default `main`) and `version` (required). Generated branch: `release/desktop-v{version}`; base: `main`.
- Packaging continuation: successful `workflow_run` for the existing `Rust` push workflow on this repository's `main`. Explicit dispatch additionally has boolean `release` (default false); existing `platform` remains for development downloads.

- [ ] Add focused version preparation to the existing Rust helper, using `toml_edit::DocumentMut` with checked access to `package.version`. Do not rewrite dependency tables. Run normal Cargo metadata to update the local package version in `Cargo.lock`; no dependency refresh is intended.
- [ ] Reuse existing version/tag tests. Add only a project-owned preparation assertion if needed: the selected application version changes while unrelated manifest fields and workspace/library versions remain intact. Use actual TOML files in a temporary directory; do not test toml_edit's formatting facilities independently.
- [ ] Implement the preparation workflow with the built-in token and job-local `contents: write`, `pull-requests: write`, `actions: write`. Use `source` through action inputs/environment and literal command arguments, not shell interpolation of expression text. Prepare the version branch/commit and create its PR with `gh pr create --body-file`. The commit message uses a file and the project's wrapping convention. Let normal review and merge take place.
- [ ] Resolve the requested source to an exact commit before reading its Cargo metadata or editing the version. If that committed application's version already matches the parsed request, `release-start` constructs `Candidate` from that commit and committed version, waits via standard `gh run watch --exit-status`, then applies the existing eligibility helper. Waiting and eligibility share the helper's exact-source CI-run lookup; the workflow does not repeat its query policy. Derive the tag from `Candidate.version` and explicitly dispatch release-mode packaging on it. Do not create an empty PR or a waiting daemon.
- [ ] For a merged version PR, use `workflow_run` continuation after successful main push CI. Filter to this repository's `push` event on `main` before write operations; PR/fork workflow results are not release sources. Find the merged same-repository PR through the standard API and generated branch name. No matching release request means no native packaging.
- [ ] For continuation, take the exact source from the triggering Rust run's `head_sha`, check it out, and derive `Candidate.version` from that checkout's application metadata. Run the existing exact-commit eligibility check for `Candidate.revision` and create `Candidate.version.tag()` if absent. An existing tag for the same source is reusable; a conflicting tag fails clearly. Do not use the current main tip, PR title or branch suffix as the source/version, and do not rewrite tags.
- [ ] Normalize one `release`/`source`/`tag` decision in the existing eligibility job's outputs. Release mode resolves the candidate tag to its exact commit and validates its application version; development mode resolves the dispatch ref to its exact commit and has no release tag. These outputs are transport for that decision, not independently supplied identities. Geography input hashing/building, helper/package preparation, both native builds, source checks and release assembly all consume that same resolved source. Every source-dependent checkout uses it, including geography's currently implicit checkout. Native jobs pass the resolved release tag to `prepare`, so naming and release assembly use the same mode. Replace assumptions that only `github.event_name == 'push'` means release, and that `GITHUB_SHA`/`GITHUB_REF_NAME` necessarily identify the candidate. Development dispatch still selects its requested platform and source-version naming.
- [ ] Keep existing signing environment, geography job and exact-source checks while adapting job conditions and checkout refs. Use Actions/CLI dispatch to continue after bot-created tags rather than relying on a suppressed tag-push event.
- [ ] Rename the draft assembly step as needed: use both resolved package outputs, including the source-owned version for the release title rather than parsing the tag in shell. Prepare notes through GitHub's release-note facilities plus existing build information, create one unpublished draft, upload both files, and publish that release as a prerelease with Latest disabled only after success. A partial upload fails and stays unpublished. Do not add a release ledger or generalized draft-recovery mechanism.
- [ ] Verify helper logic with existing packaging tests and inspect workflow conditions/outputs against the real payload schemas. Exercise hosted continuation and actual publication with the next genuine candidate. Do not mock GitHub commands to claim the deployment works or create test tags just to obtain a green result.

**Done when:** one release request prepares the normal version PR and automatically continues from its merged, checked source through both installers to a public prerelease.

### Task 5: Promotion and owning documentation

**Files**

- Create `.github/workflows/desktop-release-promote.yml`.
- Extend `lib/clacks-desktop-packaging/src/release.rs` with promotion behavior and expose it in the existing CLI.
- Update `deploy/desktop/windows/README.md`, `deploy/desktop/macos/README.md`, the lifecycle design, root README and affected crate guides.

**Interfaces**

- Promotion dispatch input: `tag`, selecting the actual candidate. No acceptance-record or evidence-reference inputs.
- CLI: `clacks-desktop-package promote --repository OWNER/REPO --tag desktop-vYY.M.N`.
- `release::promote(repository: &str, tag: &str) -> anyhow::Result<()>` operates on that existing GitHub release.

- [ ] Resolve the existing release selected by the input tag using GitHub/CLI APIs. Parse that tag with Task 3's `ReleaseVersion::from_tag`, require the returned release to have that tag and be a published prerelease, and select the expected files through `PackagePlatform::iter()` and `PackagePlatform::installer_name` in release mode. These release/asset API values identify the candidate promotion operates on. Do not create or retarget a release during promotion.
- [ ] Rely on Task 4 publication for exact-source CI and executable/package identity checks. Promotion does not repeat those checks or infer build provenance from filenames, release-note text or digests. Existing build logs/summaries remain useful diagnostics, not machine-readable promotion inputs.
- [ ] Retain the selected release ID, tag, its resolved source commit and the two asset identities (IDs, names and digests where supplied by GitHub) within the operation. Update only that release's prerelease/Latest metadata using the supported release-edit API. Read back the same release and confirm its tag/source and file identities remain unchanged. This checks preservation across promotion; it does not claim to re-establish build provenance. There is no rebuild, upload, asset replacement, new attestation, checksum report or external evidence lookup.
- [ ] Treat invoking the workflow as the maintainer's acceptance decision. Native candidate testing remains ordinary maintainer work using the distributed installers and existing platform guidance.
- [ ] Use normal Actions output to identify a failed candidate lookup, missing/mismatched file or release edit. Do not create additional reporting artifacts or require acceptance links.
- [ ] Test any new Clacks selection/mapping decision with one focused fixture containing the expected two files and an unrelated asset, then one missing expected installer. Use Task 3's shared version/tag and platform naming operations. Do not test GitHub honoring its API fields or mock the edit command as proof of promotion.
- [ ] With the first accepted genuine candidate, invoke promotion and confirm the same tag, filenames and installer hashes are final/Latest. Until then, describe the hosted promotion path as unexercised. No dummy release is needed.
- [ ] Update owning documents from the final implementation: generated repository metadata, WebSocket selection, friendly names, workflow entry points and automated prerelease publication/promotion. Remove conflicting old asset-download/manual-tagging/manual-publication instructions. Keep the 90-day best-effort overlap policy as release guidance, not runtime machinery.

**Done when:** the maintainer can promote the tested candidate through one workflow action, preserving its files, and the normal download page discovers it as Latest.

## Final verification and self-review

- [ ] Confirm every spec section maps to the tasks above. Keep public discovery separate from account access, server acceptance separate from desktop implementation, and repository metadata separate from executable input.
- [ ] Run the focused commands already listed, then the repository-required format/manifest checks and ordinary CI checks for the final diff. Use the existing disposable database setup for integration tests. No unrelated environment or native acceptance matrix is added.
- [ ] Run `cargo fmt --all --check` and `cargo sort --workspace --check --check-format`; the normal packaging freshness test must pass against the on-disk metadata. Regeneration is an explicit developer command when updating the file, not a verification side effect.
- [ ] Verify browser integration and relevant Windows/macOS installed behavior separately. Report what was actually checked, plus hosted release steps not yet exercised. Do not equate packaging success with native installation acceptance.
- [ ] Inspect the final diff and documentation links. Keep fixes with their owning logical changes.

### Sources for implementation choices

- [GitHub workflow events and continuation](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#workflow_run)
- [Built-in token event behavior](https://docs.github.com/en/actions/concepts/security/github_token#when-github_token-triggers-workflow-runs)
- [Repository setting for Actions-created PRs](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/enabling-features-for-your-repository/managing-github-actions-settings-for-a-repository)
- [Release editing API](https://docs.github.com/en/rest/releases/releases#update-a-release)
- [toml_edit comment/order-preserving editing](https://docs.rs/toml_edit/0.25.13+spec-1.1.0/toml_edit/)

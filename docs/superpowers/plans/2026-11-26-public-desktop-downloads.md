# Public desktop downloads implementation plan

**Goal** Deliver the public Clacks Log landing and download pages for [#72].

**Architecture** Add routes to the existing Leptos application and render their
shell on the server. Only the hydrated download page reads GitHub. GitHub serves
the installer files directly.

**Spec** [Public landing page and desktop downloads][spec]. That design records
the scope agreed after the original issue description, including one download
page and exact GitHub release links from the desktop update flow.

## 1. Public pages and navigation

- [ ] In `lib/clacks-web/src/lib.rs`, remove the Axum redirect for `/` and
  register `/` and `/download` in the existing Leptos router. Let the generated
  Axum route list mount both. Use a public header on these routes, with the
  product name linking home and links to `/download` and `/sign-in`. Preserve the
  existing headers, routes, session and enrollment behavior for account pages.
- [ ] Add the landing and download views under `lib/clacks-web/src/pages/` and
  register them in `pages.rs`. Apply the spec's copy and layout using public-page
  styles in `lib/clacks-web/style/account.css`. Reuse existing color variables,
  including their light and dark values. The download page's initial response
  includes its heading and the GitHub release history link.
- [ ] Extend the existing production HTTP scenario in
  `lib/clacks-web/tests/pages.rs` to request `/` and `/download` without a
  cookie and with redirects disabled. Assert both return 200, the landing
  response links to Download and Sign in, and the download response links to
  release history. This catches the old root redirect or an accidental
  authentication requirement using the existing application harness.

## 2. Load and present the current desktop release

- [ ] Add the browser request in `lib/clacks-web/src/pages/download.rs` with a
  Leptos `LocalResource`. Use the already locked `gloo-net` 0.6 HTTP client as
  an optional hydration-only dependency in `lib/clacks-web/Cargo.toml`. The
  browser makes an unauthenticated GET to
  `https://api.github.com/repos/jhelwig/clacks-log/releases/latest`. Keep the
  fetch out of the server-rendering build. Check status before decoding. Map
  404 to no final release and other request or decode failures to the error
  state. Keep the public release-history link outside the async content.
- [ ] Decode `tag_name`, `draft`, `prerelease`, `body`, and asset names and
  download URLs. Use only a final release with a valid `desktop-vYY.M.N` tag.
  Use the version suffix of the tag to select the exact packaging filenames
  `clacks-log-YY.M.N-windows-x64.msi` and
  `clacks-log-YY.M.N-macos-arm64.dmg`. Link to the matching assets' HTTPS
  GitHub download URLs. Keep a matching installer visible when the other is
  absent, with a clear unavailable label for the missing platform.
- [ ] Render the nonempty release body as Markdown below the installers with a
  browser-compatible Markdown renderer whose safe defaults exclude raw HTML
  and unsafe links. Comrak without its default CLI and syntax-highlighting
  features is the proposed renderer. Keep its unsafe mode disabled.
- [ ] Cover the app-owned release selection in one small fixture-based test
  near the release mapping. Give it the two exact assets, an unrelated asset,
  and then a release missing one installer. Check that the links match the
  exact tag and the remaining installer stays available. Exercise normal, 404
  and failed lookups in a browser smoke check with representative responses,
  since Clacks has no final desktop release available for a live success check.
  Confirm the page runs the request from the browser and stacks the platform
  choices at a narrow width. Check landing and download light and dark
  appearances and read a formatted release-note sample.

## Build and handoff

Run the relevant `clacks-web` HTTP scenario against the project's disposable
runtime database, `cargo fmt --all --check`, and the existing
`SQLX_OFFLINE=true cargo leptos build --release` web build. The latter verifies
server rendering and browser hydration compile together. Keep GitHub lookup
and compatibility work separate in delivery as described in the spec. The PR
should describe the agreed scope in place of the issue's earlier exact-release
website requirements.

[#72]: https://github.com/jhelwig/clacks-log/issues/72
[spec]: ../specs/2026-11-26-public-landing-and-download-design.md

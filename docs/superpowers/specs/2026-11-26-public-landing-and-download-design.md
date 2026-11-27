# Public landing page and desktop downloads

[Issue #72] gives someone arriving at a Clacks Log server a public introduction
to the desktop app and a direct path to its installers. The same web application
serves the pages on the hosted deployment and on self-hosted servers.

## Landing page

Serve a public landing page at `/`. Use the existing Clacks Log palette, with a
text-led layout and no desktop imagery. The header provides **Download** and
**Sign in**. Sign-in begins when the visitor follows the sign-in link.

Use **One amateur radio logbook across your computers.** as the headline.
Explain it with the example from the reviewed sketch:

> Log contacts on your shack computer and your field laptop. Keep logging on
> either one when you’re disconnected. When they reconnect, both computers sync
> to the same logbook.

The primary **Download Clacks Log** action opens `/download`. Keep the setup
instruction to one line. **Settings → Sync → Connect → Sign in**.

## Download page

Use a single public `/download` page. Show the final desktop version recommended
by GitHub Latest, with equal prominence for both installers.

| Download | Requirements | Format |
| --- | --- | --- |
| Windows | Windows 11 · x64 | MSI installer (.msi) |
| macOS | macOS 26 Tahoe or newer · Apple Silicon | Disk image (.dmg) |

Place the requirements beside the corresponding action, stack the choices on
narrow screens, and omit installer signing labels. Place release notes below
the installers. Link to GitHub release history from the page in every state.

| Result | Page behavior |
| --- | --- |
| Lookup pending | Show that release information is loading. |
| Latest has a final desktop release | Show the version, available installer links, and notes from that release. If an installer is missing, say which one and keep the other available. |
| Latest returns no final release | Say “No final desktop release is available yet.” |
| Lookup fails or returns invalid release data | Say “Couldn’t load release information.” |

## Release source and delivery

The existing Clacks web app owns both pages. The browser reads GitHub Latest
directly to avoid sharing the Clacks server's GitHub request allowance.
Downloads link directly to the release assets on GitHub. GitHub release history
preserves access to older releases. In-app updates use the exact GitHub release
the desktop checked.

[Issue #73] adds compatibility with the server serving the page. Fetch its
public capabilities without credentials and compare them with generated
`deploy/desktop/sync-contracts.json` through the raw Repository Contents API
at the displayed release's exact tag. The server's accepted-contract order
governs live selection; the page only determines whether support overlaps.
Downloads remain available while checking and for incompatible or unknown
results. Shared release identity/naming operations supply the friendly installer
names and version parsing used by both packaging and this page.

Existing web request tracing covers serving the public routes. The browser's
GitHub request uses visible loading and failure feedback without reporting the
release body or browser request details to the Clacks server.

[Issue #72]: https://github.com/jhelwig/clacks-log/issues/72
[Issue #73]: https://github.com/jhelwig/clacks-log/issues/73

# Clacks web interface

The Leptos application provides sign-in and linked-identity management. Axum
mounts its rendered pages, server functions and generated assets inside the
server's session, mutation-origin and request-observability policies. Account
operations live in `clacks-server`; only explicit display values cross into
browser hydration. A browser-supplied account ID never authorizes an operation.

The public landing page at `/` links to sign-in and to `/download`. The
download page reads GitHub Latest in the browser and links to the Windows and
macOS installers on GitHub. Its release-history link is available in the
initial HTML, including when the browser cannot load the latest release.

For local PostgreSQL, OIDC credentials and the cargo-leptos watch loop, use the
[server development workflow](../../docs/server-development.md).

The `ssr` feature means server-side rendering. It enables the server dependencies.
The `hydrate` feature builds the Rust WebAssembly browser runtime without them.
Use these features in separate builds, as cargo-leptos does:

```sh
cargo install cargo-leptos --version 0.3.11 --locked
rustup target add wasm32-unknown-unknown
SQLX_OFFLINE=true cargo leptos build --release
SQLX_OFFLINE=true cargo check -p clacks-web --no-default-features --features hydrate --target wasm32-unknown-unknown --locked
```

Run the generated `clacks-server` binary with `LEPTOS_SITE_ROOT` pointing at
`target/site` (or the packaged site directory). `LEPTOS_OUTPUT_NAME=clacks-web`
is needed for ordinary Cargo test/run commands that bypass cargo-leptos.
The stylesheet follows the defined-sections palette and system appearance.
Standard Leptos actions own pending/error states and browser redirects.

**Account → Directory** (`/account/directory`) selects QRZ, HamQTH or disabled
and edits each provider's login independently. Saved usernames are prefilled;
a fixed password placeholder indicates an existing password without returning
its value or length. Save credentials adds or updates the login, retaining the
saved password when that input is blank. Remove credentials clears the login.
Saving credentials preserves lookup selection, and saving selection preserves
both providers' logins. Failed submissions retain entered values.
The forms use ordinary same-origin authorized
actions and an expected configuration revision; saving configuration does not
test the provider login. Authorized enrolled desktops receive selected material
through their existing connection for protected local use.

The account HTTP scenario uses the production rendered and server-function
boundaries with a synthetic identity provider and the restricted runtime role:

```sh
LEPTOS_OUTPUT_NAME=clacks-web SQLX_OFFLINE=true cargo test -p clacks-web --features ssr --test integration --locked
```

Supply `CLACKS_TEST_RUNTIME_DATABASE_URL` for the disposable, migrated PostgreSQL
database described in the server README. These scenarios establish Clacks's
request boundary and deliberate secret exclusion; it does not establish live
provider interoperability or browser visual acceptance.

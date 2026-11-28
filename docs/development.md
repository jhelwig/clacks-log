# Maintainer development

The [workspace build and verification instructions](../README.md#development)
cover the Rust toolchain and ordinary checks. The
[server development guide](server-development.md) covers the local server and
web environment.

## Dependency checks

Cargo-deny 0.22.2 checks the resolved dependency graph for RustSec security and
maintenance advisories and for licenses outside the allowlist in `deny.toml`.
The checks include all features and target-specific dependencies, including
build and development dependencies. License approval does not replace required
third-party notices or source-distribution obligations.

Pull requests block on newly introduced **direct** dependencies already reported
as unmaintained. Promoting a transitive dependency to direct use also blocks.
Existing direct dependencies and every transitive maintenance finding warn.
Direct use means use by any workspace member; identity is the actual package
name and Cargo source, independent of aliases, versions and checkout paths.
Directness applies to the affected package version in the proposed graph.
Existing direct use on the base remains a warning across version updates or
when another workspace member starts using the same dependency directly.

In the PR's Checks tab, select **Dependency maintenance report** to read the
findings without opening the Actions run. Its name stays fixed; the report title
shows the current counts. Clean maintenance results succeed, non-blocking
warnings have a neutral conclusion, and newly introduced unmaintained direct
dependencies fail. An incomplete evaluation reports failure rather than a clean
scan. The required verification gate separately enforces vulnerabilities,
licenses and maintenance blockers.

The custom report requires a token with `checks: write`. Same-repository PRs,
main pushes, and scheduled/manual runs publish it. Fork PRs retain warning
annotations and Actions summaries with their read-only token.

The workflow scans the proposed graph and compares direct identities with the
base branch's resolved metadata. The classifier reads all audit JSON records,
deduplicates repeated findings and rejects empty, malformed or inconsistent
reports. Main-branch, manual and daily runs report all maintenance findings as
warnings. Vulnerabilities, unapproved licenses and scan failures
still fail dependency verification. The daily schedule uses GitHub Actions;
delivery follows the repository's Actions notification settings.

For a local scan from the repository root:

```sh
cargo install cargo-deny --version 0.22.2 --locked
(
set -e
cargo metadata --locked --all-features --format-version 1 > head-metadata.json
cargo deny fetch db
cargo deny --locked --all-features --offline --format json \
  --metadata-path head-metadata.json check \
  -W unmaintained --audit-compatible-output advisories > advisories.json
cargo deny --locked --all-features \
  --metadata-path head-metadata.json check licenses
cargo run --manifest-path tools/dependency-policy/Cargo.toml \
  -p dependency-policy-cli --locked -- head-metadata.json advisories.json
)
```

To apply the PR maintenance gate locally, generate `base-metadata.json` with
the same Cargo metadata command in a checkout of the comparison base, then
append that file as the final argument to the policy command. Without a base,
the command provides maintenance monitoring only. Keep generated JSON outside
version control. Do not treat a failed cargo-deny invocation as a clean scan.

The isolated Rust workspace under `tools/dependency-policy` contains only
the maintenance classifier and its command-line wrapper. Its regression tests
run in dependency CI because an incorrect classification could block unrelated
PRs or admit an unmaintained direct dependency. The gate covers the project
dependency graph.

### Remaining RSA advisory

The locked graph retains RSA 0.9.10 through `openidconnect` 4.1.0, whose
manifest requires RSA 0.9. [RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071.html)
concerns private-key recovery through timing leakage; RSA 0.10.0 is patched.
Clacks' direct RSA signing and Huskarl's crypto implementation use 0.10.0.
`deny.toml` contains an exception for this advisory, limited to the direct
dependent `openidconnect@4.1.0` using cargo-deny's native `allow` field.
Affected RSA introduced through another direct dependent or another OpenID
Connect version falls outside the exception and fails the advisory check.

The production OpenID Connect
[callback](../lib/clacks-server/src/auth/callbacks.rs) verifies provider ID tokens
using `claims` with `id_token_verifier`. In the released `openidconnect` source,
`core::crypto::verify_rsa_signature` constructs an RSA public key from the
provider's public modulus/exponent and calls `verify`. This path has no RSA
private key to expose. [Client authentication](../lib/clacks-server/src/auth/oidc.rs)
accepts only `client_secret_basic`, `client_secret_post` or `none`; it does not
configure RSA private client assertions. Production OpenID Connect uses no RSA
private-key operations affected by this advisory.

The [synthetic test provider](../lib/clacks-web/tests/support/provider.rs) does
use `CoreRsaPrivateSigningKey`, which imports and signs with RSA 0.9 internally.
Its listener binds to loopback and its keys are generated for each test provider,
with no production credentials. This test-only signing uses an affected
private-key operation. Reassess the exception when OpenID Connect usage or its
implementation changes, especially if private signing or decryption is introduced.
RSA 0.9 remains vulnerable. Remove the exception when the affected RSA
dependency is eliminated; cargo-deny reports unused advisory exceptions. See the
[cargo-deny exception configuration](https://embarkstudios.github.io/cargo-deny/checks/advisories/cfg.html#allow).

## Continuous integration

The `Rust` workflow reports a `CI result` check on every pull request. Changes
limited to root policy/license documents, `README.md` and `AGENTS.md` files,
or Markdown under `docs/` and `references/` skip workspace and web verification.
Dependency checks run for those pull requests as well. Other changes require
both verification jobs to succeed. Classification errors
and unexpected verification failures or skips fail `CI result`. Pushes to `main`
always run both verification jobs and dependency checks before the workflow can
succeed and trigger production deployment. Application pull-request jobs require
a branch in this repository; code changes from forks cannot satisfy `CI result`.
Fork dependency scans run on hosted runners.

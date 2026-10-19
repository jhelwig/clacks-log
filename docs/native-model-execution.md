# Native-model execution evidence

Checkpoint: 18 October 2026. The user authorized native execution of the
[issue #1 implementation plan](superpowers/plans/2026-10-18-native-model.md),
against the [native-model specification](superpowers/specs/2026-10-18-native-model-design.md).
Work is on branch `native-model` in an isolated Git worktree. The original main
checkout is unchanged. Independent whole-branch review precedes integration;
this checkpoint does not close the GitHub capability issue or complete a roadmap phase.

## Delivered boundary

The Rust 1.99.1 library in `lib/clacks-model` implements scalar/identity validation,
exact grid-area/point calculations, preserved mode/band identity, contact records,
copied profiles/equipment and baselines, tag policy/deletion classification,
data-driven groups/catalogs, eligible-source selection, bounded mapping validation,
context selection and in-memory reference output planning. A radio contact is a QSO.

No database, client/server application, service-observation store, causal sync
protocol, catalog downloader or full interchange file adapter is delivered.
A planned partition is not a published file. Program-specific full submission
validation, geographic datasets and import-side `CONTEST_ID` ownership stay in
their downstream capabilities. Existing copied facts remain independent of
provider and catalog metadata. Account authorization and durability are adapter obligations.

## Verification

All checks used the pinned compiler and committed dependency lockfile:

- 45 integration tests passed with `cargo +1.99.1 test --workspace --all-targets --locked`.
- One compile-fail documentation test passed with `cargo +1.99.1 test --doc --workspace --locked`.
- The same 45 tests passed offline after dependencies were acquired.
- Formatting and Clippy (`--all-targets --locked -- -D warnings`) passed.
- `cargo +1.99.1 check --workspace --target x86_64-pc-windows-msvc --locked` passed. This checks compilation, not a Windows runtime or installer.
- Reviewed the dependency tree: regex 1.13.1, Serde 1.0.229, serde_json 1.0.154 and ulid 3.0.0 with their Rust/platform-randomness dependencies. No OpenSSL or application framework.
- Checked named source modules, item separation, local documentation links and whitespace.

The five new cross-component tests passed on their first run because earlier
implementation tasks already supplied those contracts; no artificial failure
was introduced. Tasks 1–10 each first ran their new behavioral tests against
missing implementations and then passed them and the workspace suite.

## Execution rulings

- Native worktree creation could not identify the repository from the calling
  chat. Used a standard sibling Git worktree to protect main. Git manages it;
  the app could not attach the unmanaged checkout.
- The official Amateur Data Interchange Format (ADIF) 3.1.8 Mode table explicitly
  lists ten phase-shift keying (PSK) specializations absent from its Submode table. Included those
  declarations with source markers; the missing comma between two multiple-frequency shift keying (MFSK) names
  is resolved by their separate Submode rows. Cost if wrong: recognizing extra
  documented labels; recorded identity still does not change.
- `CurrentIdentifier` requires one association's context; export mappings use
  explicit collections/bindings. Cost if wrong: a definition must use those
  explicit forms rather than relying on an arbitrary member.
- Expressions are limited to 32 nested operations. Cost if wrong: unusually
  deep definitions must be flattened.
- Unknown associated groups produce explicit unsupported-group warnings for
  adapters. Cost if wrong: an additive diagnostic variant needs display handling.
- Empty expansions without the retain-one option report output NotReady instead
  of silently dropping contacts. Cost if wrong: deliberate zero-entry mappings
  would need a separate explicit contract.

## Review status

Independent whole-branch review pending. Any important fixes and their regression
checks will be recorded here before integration.

# Native-model execution evidence

Checkpoint: 18 October 2026. The user authorized native execution of the
[issue #1 implementation plan](superpowers/plans/2026-10-18-native-model.md),
against the [native-model specification](superpowers/specs/2026-10-18-native-model-design.md).
Work is on branch `native-model` in an isolated Git worktree. The user selected
publication as [PR #39](https://github.com/jhelwig/clacks-log/pull/39), open against
`main`; the branch is not merged. The adversarial panel reviewed `afb001d`; the user reviewed its results and
authorized fixes for all three P2 findings, now recorded as resolved in the
[panel review](architecture-review.md#relation-to-current-implementation-review). This
checkpoint does not close issue #1 or complete a roadmap phase.

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

Current code includes the user-authorized catalog-collision fix after `86baeb6`
(data-driven recognition). All checks used
the pinned compiler and committed dependency lockfile:

- 51 integration tests passed with `cargo +1.99.1 test --workspace --all-targets --locked`.
- One compile-fail documentation test passed with `cargo +1.99.1 test --doc --workspace --locked`.
- All 51 integration tests and the documentation test passed offline after dependencies were acquired.
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

## Independent review and fixes

A fresh reviewer inspected the entire branch at `9ad572b` and reproduced three
Important findings using a temporary harness without changing the checkout.
Each received a regression test that failed before the fix and passed afterward:

- `empty_aggregates_require_normalization_before_saving`: validation now rejects
  noncanonical empty location/operator structures until explicit normalization.
  Cost of this boundary choice: saving adapters must normalize before validation;
  validation does not mutate input or replace ordinary whole-record equality.
- `empty_receive_override_normalizes_to_primary_fallback`: normalization now
  removes an empty receive pair so receive inherits the primary pair.
- `unrelated_invalid_group_does_not_block_targeted_export`: targeted output now
  excludes unrelated groups before compiling definitions, preventing an unrelated
  malformed configuration from blocking valid output.

At `584d99c`, the post-review-fix suite passed: 48 integration tests and one
compile-fail documentation test. D256 subsequently added two regression tests,
bringing the total at `86baeb6` to 50 integration tests plus the
documentation test. Formatting, Clippy, offline verification and Windows
cross-compilation were repeated after fixes. That original fix pass was verified by regression tests
and the complete suite, without a second reviewer. It does not independently
review the later D256 refactor. The later adversarial panel is a separate review of `afb001d`; its
[user-authorized fixes](architecture-review.md#relation-to-current-implementation-review) do not imply a second
independent review of the fixes themselves.

The review left these planned component boundaries in place:

| Boundary | Why it remains downstream / practical limit |
| --- | --- |
| Provider eligibility, portable-call keys, request arbitration, cancellation and late results | Selectors consume eligible observations; service adapters must establish eligibility. |
| Durability, account authorization, causal reconciliation, deletion transactions and reference resolution | Pure records/classifications provide no durable or account-isolation guarantees on their own. |
| Geographic datasets and applying refreshed/manual corrections | Geometry/position guards exist; actual geographic autofill is not delivered. |
| Import parsing, omitted-versus-cleared fields, historical assignment and canonicalization warnings | Native values are not import instructions; no file importer is delivered. |
| Complete output mappings, effective provider fields, qualified submission validation, activation-location partitions and capture readiness | Generic reference mechanics do not certify program submission acceptance. |
| Consistent export reads, decision-gated publication, destination replacement and cancellation | A storage-backed file workflow is still needed before exposing real export. |
| Production group seeding and catalog acquisition/refresh | Application provisioning must supply the data through these interfaces. |
| Entry workflows, persistent current defaults, Windows runtime and large-log performance | Application readiness and scale are not certified by model tests or cross-compilation. |

The review's Minor packaging follow-up is resolved: the repository now includes
the unmodified [Elastic License 2.0 text](../LICENSE), copied byte-for-byte from
[Elastic's published text](https://raw.githubusercontent.com/elastic/elasticsearch/main/licenses/ELASTIC-LICENSE-2.0.txt).
Cargo metadata and README identify the selected license. The contributor
agreement remains separate lawyer-led work.

## Data-driven recognition correction

The user's D256 correction replaces concrete known-family/submode/band enums
with generic recorded values and catalog-derived interpretations. Removed
`recognition/known.rs`; no generated code or second catalog remains. The mode
parent/specialization structure, band-label serialization, equality and ordering
stay unchanged. Recorded-value construction no longer accepts a catalog;
interpretation takes the desired catalog and borrows its matching rows.

Two regression tests failed against the type-encoded implementation: supplied
new mode/submode rows and supplied new band rows could not be recognized. Both
now pass with catalog-only additions. Baseline tests inspect actual matched rows
and metadata, and old/new recognition views preserve recorded identity/context.
The D256 checkpoint had 50 integration tests and one compile-fail documentation
test. The panel regression adds one test, bringing current verification to
51 integration tests and the documentation test. The catalog-row checks use committed extracted data; they
do not independently re-download or certify the upstream specification.

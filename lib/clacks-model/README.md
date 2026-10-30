# Native-model library

This Rust 1.99.1 library implements the [native-model specification](../../docs/native-model.md).
It provides pure values, records, validation and selection operations for the
desktop and server. It performs no network access, persistence or file publication.

Public items are accessed through their owning modules, such as
`clacks_model::contact::Qso`, `clacks_model::recognition::mode::Mode` and
`clacks_model::export::evaluate::plan_reference_export`. The crate root does
not flatten module contents through re-exports.

## Usage

Prepare entry/import values outside saved records, then construct a contact with
`Qso::try_from(contact::input::QsoFields)`. Construction and deserialization reject
invalid saved input. Use immutable accessors for reading, and `into_fields()` for
a complete edit followed by checked reconstruction. Prepare optional text with
`scalar::optional_text`; location, operator, equipment and reusable-profile input
types have their own checked conversions. Simple aggregates whose component types
already establish validity retain ordinary fields.

`profile::SavedLocationProfile` owns a station snapshot
and an optional equipment configuration ID; `EquipmentConfiguration` owns reusable
equipment values. `CurrentOperatingValues` owns copied facts plus operator/power.
Desktop transient forms own loading and independent source selection; the store
owns their local baselines. None of these reusable values is a live link into
historical QSOs. See the [operating-settings contract](../../docs/operating-settings.md).

Group definitions are account data, not a hardcoded registry. Test fixtures are
not production account seeds. `plan_reference_export` returns fields, partitions,
decisions and warnings without changing inputs or writing interchange files.
Outstanding decisions block publication in the eventual export workflow.

## Responsibilities

- Scalar validation, typed identities, exact grid-area/point calculations and preserved mode/band identities.
- Contact records, copied profiles/equipment, tag policy and tag-deletion classification.
- Data-driven reference groups/catalogs, eligible-source selection, bounded mapping validation, export-context selection and in-memory reference-output planning.

Recorded facts remain independent of provider and catalog metadata. Mode and
band interpretation borrows supplied catalog rows; recognition does not rewrite
recorded identity, serialization, equality or ordering. Catalog labels must be
unique under ASCII case-insensitive lookup within bands, modes and submodes.
Accepted labels retain their supplied spelling.

Entry/import preparation collapses absent optional location/operator input before
construction. Saved location/operator values cannot be empty, and a present
band/frequency pair must contain at least one component. An absent receive
override inherits the primary pair. Targeted export excludes unrelated
groups before compiling definitions, so an unrelated malformed definition cannot
block the selected context.

Concrete validation and configuration errors use `thiserror` for their display
messages and standard error implementations. Operations combining record,
profile or export failures return `anyhow::Result` and propagate original
errors without forwarding-only enum variants. Callers can downcast to the
concrete error type when they need structured handling. Saved-record construction
is separate from export/service readiness and current account-reference checks.

Catalog lookup returns normal availability/listing outcomes separately from
configuration failures; failures retain their concrete errors through
`anyhow::Result`. The bundled recognition loader returns a fallible shared
catalog reference. Production library code propagates failures without
`expect()` or `unwrap()`.

## Mapping rules and catalog data

`CurrentIdentifier` requires one association's context; collection mappings use
explicit collections and bindings. Expressions are limited to 32 nested
operations. Unknown associated groups produce unsupported-group diagnostics.
An empty expansion without the retain-one option reports `NotReady` rather than
silently dropping the contact.

The bundled Amateur Data Interchange Format (ADIF) 3.1.8 recognition catalog
contains 33 bands, 91 mode labels and 198 submode declarations.
[Catalog provenance](data/README.md) explains source-table
discrepancies and extraction. Tests cover the committed rows; they do not
independently certify upstream source tables. Builds do not fetch catalogs.

Compiled group definitions borrow their source data rather than recursively
cloning it. Excessive expression depth is rejected during bounded validation.
Reference-export planning prepares each relevant definition once per operation
and reuses it for selection and evaluation across contacts. Preparation remains
limited to participating groups allowed by the export context.

## Adapter boundaries

| Responsibility | Boundary |
| --- | --- |
| Provider eligibility, portable-call lookup keys, request arbitration, cancellation and late results | Selectors consume eligible observations; service adapters establish eligibility. |
| Durability, account authorization, causal reconciliation, deletion transactions and reference resolution | Pure records/classifications provide no durable or account-isolation guarantees. |
| Geographic datasets and refreshed/manual corrections | Geometry and position guards do not supply geographic autofill. |
| Import parsing, omitted-versus-cleared fields, historical assignment and canonicalization warnings | Native values are not import instructions. |
| Complete output mappings, effective provider fields, submission validation, activation-location partitions and capture readiness | Generic reference planning does not certify program submission acceptance. |
| Consistent export reads, destination replacement and cancellation | An in-memory partition is not a published file; storage-backed publication must enforce export safeguards. |
| Production group seeding and catalog acquisition/refresh | Account provisioning installs `defaults::account_definitions()`; catalog acquisition/refresh remains with data-distribution adapters. |
| Entry workflows, persistent current defaults, Windows runtime and large-log performance | Model tests and cross-compilation do not establish application readiness or scale. |

Import-side station assignment of `CONTEST_ID` remains an import-mapping concern.
The library does not include a database, client/server application,
service-observation store, causal synchronization protocol, catalog downloader
or complete interchange file adapter.

## Validation

Use the [workspace verification commands](../../README.md#development) with the
pinned compiler and committed lockfile.

Offline checks require dependencies to be available locally; cross-compilation
also requires the target standard library. Windows compilation does not test
the runtime or installer. Tests cover preservation across differing recognition
views, case-colliding catalog rejection, coordinate boundaries, copied profiles,
whole-record equality and context-scoped reference expansion.

## Service projections and retained evidence

`service::destination` separates remote LoTW accounts and QRZ logbooks from live
certificates/API keys. `service::evidence` represents imported claims, frozen
attempts, scoped outcomes, observations, associations, dispositions, deletion
suppression and download coordination. Imported positive upload and confirmation
claims have independent baselines; missing upload status remains pending.

`service::adif` performs exact whole-hertz/milliwatt, UTC-second and coordinate
conversions and best-effort text transliteration with warnings. `service::lotw`
uses the pinned TrustedQSL configuration for normalization; `LotwSubmission` is
the common input for coverage comparisons and future signing. It retains
`LotwMappingProvenance` (configuration release and source revision).
`SubmissionProjection::same_payload` compares submitted values without that
metadata; structural equality includes provenance for immutable-evidence checks.
The signer consumes those frozen values rather than mapping modes a second time. It does
not depend on the synchronization crate. `service::qrz` prepares its ordinary
ADIF field set using eligible source fallback and existing context mappings.
`service::applicability` evaluates publication coverage and conservative
confirmation applicability separately.

These operations do not fetch remote records, issue QRZ replacement requests,
import private keys or prove LoTW acceptance. Actual adapters must validate their
wire responses, authenticate namespaces and preserve the public/frozen inputs
needed by [the service contract](../../docs/service-evidence.md).

Publication coverage accepts service, optional destination and optional prepared
submission separately. Trusted unchanged imported claims do not require routing,
credentials or signing readiness. Source baselines retain exact native times even
when interchange dates cannot be emitted. Uninterpreted band-only labels remain
comparison evidence alongside exact frequency fallbacks. LoTW source-only location
baselines compare the emitted locator and recognized subdivision values, retaining
uninterpretable supplied values without borrowing certificate defaults. The native
QSO retains its original position and location information.

A returned remote ID has member-level meaning only when its result scope identifies
one attempt or `SubmissionResult.external_attempt` explicitly correlates a batch
member. A batch success alone cannot attach its returned ID to an arbitrary QSO.
LoTW station normalization applies source-defined entity/subdivision dependencies,
field input flags and CQ/ITU zone-pair restrictions before sharing values with signing.

## Observability

The [production observability contract](../../docs/observability.md) applies to
this library. Meaningful operations use `#[tracing::instrument(skip_all, ...)]`
with explicit safe input fields and default module-path targets. Standard
subscribers/exporters supplied by application composition own lifecycle
presentation, context and span timing. Instrumentation preserves normal function
bodies, early returns and error propagation.

`service::evidence::ServiceRecordId` implements `Display` using the contained native ID's canonical text.
No telemetry-specific wrapper is needed. Annotation fields use constants or
simple input/field access; derived context is reported where domain code resolves it.

Local events and metrics report domain decisions and actual produced plans:
readiness, proposed/staged work, retry/review reasons and relevant quantities.
They do not report generic function completion or duplicate span timing. Metric
families have their own coherent bounded dimensions; IDs remain trace fields. The
model counts projection-readiness decisions; sync counts acceptance decisions
and ready/staged transfer plans. These include repeated or tentative calculations.
Shared export planning, compaction and coupled service planning report useful
quantities through events, without successful-call-only counters.
A propagated error produces no fabricated plan/proposal result. Error handling
and overall workflow success/error/latency measurements belong to their owning
runtime boundary; span timing does not automatically populate metric histograms.

Filtering cannot gate domain work or metrics, and domain events do not mutate
caller spans. The library installs no subscriber, recorder or exporter. Returned
values, concrete errors and synchronized records are independent of configuration.

## Initial account data

`defaults::account_definitions()` loads the validated production reference groups
and output profiles described in [the data notes](data/README.md#initial-account-definitions).
Account provisioning owns their installation; reopening an account must preserve
its saved definitions. The loader performs no account creation or networking.

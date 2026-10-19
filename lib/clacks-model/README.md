# Native-model library

This Rust 1.99.1 library implements the [native-model specification](../../docs/native-model.md).
It provides pure values, records, validation and selection operations for the
desktop and server. It performs no network access, persistence or file publication.

Public items are accessed through their owning modules, such as
`clacks_model::contact::Qso`, `clacks_model::recognition::mode::Mode` and
`clacks_model::export::evaluate::plan_reference_export`. The crate root does
not flatten module contents through re-exports.

## Usage

Normalize optional aggregates with `Qso::normalize_optional_aggregates`, then call
`Qso::validate` before saving. Incomplete entry and import preparation remain
outside saved records. Profile loading and equipment selection copy values.

Group definitions are account data, not a hardcoded registry. Test fixtures are
not production account seeds. `plan_reference_export` returns fields, partitions,
decisions and warnings without changing inputs or writing interchange files.
Outstanding decisions block publication in the eventual export workflow.

## Responsibilities

- Scalar validation, typed identities, exact grid-area/point calculations and preserved mode/band identities.
- Contact records, copied profiles/equipment, local comparison baselines, tag policy and tag-deletion classification.
- Data-driven reference groups/catalogs, eligible-source selection, bounded mapping validation, export-context selection and in-memory reference-output planning.

Recorded facts remain independent of provider and catalog metadata. Mode and
band interpretation borrows supplied catalog rows; recognition does not rewrite
recorded identity, serialization, equality or ordering. Catalog labels must be
unique under ASCII case-insensitive lookup within bands, modes and submodes.
Accepted labels retain their supplied spelling.

Saving adapters normalize optional aggregates before validation. Validation
rejects empty location/operator structures; an empty receive override normalizes
to absence and inherits the primary pair. Targeted export excludes unrelated
groups before compiling definitions, so an unrelated malformed definition cannot
block the selected context.

Concrete validation and configuration errors use `thiserror` for their display
messages and standard error implementations. Operations combining record,
profile or export failures return `anyhow::Result` and propagate original
errors without forwarding-only enum variants. Callers can downcast to the
concrete error type when they need structured handling. Export-readiness
context retains the underlying validation error in the diagnostic chain.

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
| Production group seeding and catalog acquisition/refresh | Account provisioning and data-distribution adapters supply these inputs. |
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

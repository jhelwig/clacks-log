# clacks-geography

Read-only geographic lookup and deterministic FlatGeobuf encoding. The reader depends on the native model and Rust geometry/format libraries. It has no source downloads, archive decoding, datum transformation or recorded-position mutation.

Use `artifact::write::write_bundle` to encode compiled contents into a private empty directory. All five schema-1 roles are required, including empty layers. The writer generates hashes, bounds and feature counts; source/rule provenance and notices travel with the bundle. Publication belongs to the builder.

Use `artifact::read::open_bundle` to validate a release, then `Bundle::lookup` with `clacks_model::position::LookupGeometry`. Open checks the schema before interpreting its metadata, every declared file and feature, scoped values/provenance, finite valid topology, exact columns, bounds/counts and the canonical index. Frame sizes are checked against file length before dependency feature allocation. Metadata uses the Open Geospatial Consortium's CRS84 coordinate reference system: longitude first in World Geodetic System 1984 (WGS84). Nonempty indexes use 16-way branching. Hashes establish coherence relative to the manifest, not publisher authentication.

A bundle pins validated file handles. Keep that immutable release unchanged while readers use it. Another release can be selected without changing the handle's data. There is no shared global reader/cache or hidden network access. `geometry::validate_topology` is also available to the producer for longitude-unwrapped geometry; coordinate-reference/domain checks belong to its caller. It combines `geo` sweep intersections, ring-contact connectivity and prepared topology relationships without repairing geometry.

Each field returns distinct scoped candidates and complete/partial/none coverage. Boundary points include every covering classification. A grid uses its entire rational area, excludes corner/edge-only contacts and retains every positive-area alternative. One candidate with complete coverage is eligible for application autofill. Partial coverage and ambiguity require operator judgment. Missing coverage is an ordinary result; malformed data, failed reads and unresolved numerical decisions are errors.

`geo` supplies membership and topology predicates. Area coverage uses `clipper2-rust` 1.2.0's Rust-only integer engine with nonzero winding on a shared binary lattice, using the largest power-of-two scale up to `2^48` units per degree that keeps every coordinate within `1e15` integer units. Coordinate conversion is relative to the query window and checked for finite/range errors. Relevant source vertices, intersection nodes and local boundary turns that cannot be distinguished at the engine's resolution return `NumericalFailure`; they are not treated as missing small slivers. This is a computational limit, not source accuracy, recorded-coordinate precision or a proof of arbitrary floating-point topology. Tests cover 2–12 character locators, shared non-binary borders, overlaps, holes, global features, narrow gaps and clipped triangular gaps down to the computational limit.

The producer's `geo`/`i_overlay` operations have a separate extent-dependent integer precision, approximately `2^-22` degrees at a 360-degree extent. That affects compiled boundaries independently of the reader's coverage lattice. The builder documents source/operation accuracy and computational limits in each release.

`assistance::populate` applies unique, fully covered values only to eligible
absent native fields. Existing values and protected absence remain authoritative.
`assistance::change_position` expires geographic facts and their transient
protection only when native geographic evidence changes; explicit-point display
precision alone preserves them. Neither operation performs I/O or persists data.
Editors and live capture use the same native rules with their owning precedence.

Run `cargo +1.99.1 test -p clacks-geography --locked --offline` and the corresponding
Clippy checks. Fixtures are authored or reproduced in Rust and use the production
writer/reader. Shipping data and package selection follow the independently
maintained [release procedure](../../references/geography/RELEASING.md).

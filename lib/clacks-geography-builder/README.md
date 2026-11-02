# clacks-geography-builder

Pinned-source acquisition, geographic compilation, acceptance checking and immutable publication. `bin/clacks-geography-build` is a thin entry point; startup and workflows live in this library. It uses `clacks-geography`'s production writer, reader and query contract.

Run from the workspace root with Rust 1.99.1:

```sh
cargo run -p clacks-geography-build --release --locked --offline -- --help
cargo run -p clacks-geography-build --release --locked --offline -- fetch --recipe references/geography/representative.json --cache <cache>
cargo run -p clacks-geography-build --release --locked --offline -- inspect --recipe references/geography/representative.json --cache <cache>
cargo run -p clacks-geography-build --release --locked --offline -- build --recipe references/geography/representative.json --cache <cache> --output <output>
cargo run -p clacks-geography-build --release --locked --offline -- verify --bundle <output>/releases/representative-2026-10-23 --recipe references/geography/representative.json
```

Cargo stays offline; only explicit `fetch` downloads inputs. Acquisition uses Rustls, bounded responses/timeouts and digest-keyed atomic cache installation. Every reused entry is verified. Interrupted, oversized and checksum-mismatched downloads are not installed; a corrupt existing entry fails. Build and verify have no network requests. Ctrl-C sets the workflow cancellation flag. `inspect` emits selected original identities, attributes and semantic guards as JSON, using the same decoder as compilation.

Archive inventory, expansion/count limits, regular paths, shape/attribute correspondence, character encoding and coordinate declarations are checked. Recipe typos/unknown fields fail rather than being ignored. Embedded HA8TKS collections are parsed without executing JavaScript; only trailing separators outside strings are removed before strict JSON decoding. Original source-byte hashes remain authoritative input guards.

Coordinate operations are explicit recipe data. The producer uses `proj4rs` with North American Datum 1983 (NAD83)/WGS84 ellipsoids, zero input ellipsoidal height, the prescribed geocentric translations, and a required pinned Caribbean grid. The grid's High Accuracy Reference Network target is approximated as WGS84. Missing/out-of-area grids and failed operations are errors. D259 no-shift assignments preserve angular coordinates and make no accuracy claim. Computation adaptively checks quarter/midpoint deviation to 0.01 m, bounded by depth and a shared feature vertex budget. This tolerance does not improve source accuracy.

The representative recipe covers selected complete administrative features, not clipped substitutes for county borders: Washington/Oregon samples, Connecticut's eight 2021 counties alongside its 2026 state, Alaska/Hawaii samples, Caribbean county footprints and Pacific territory states. Selected Census land faces define reviewed onshore masks: 534 Aleutians West, 5,963 San Juan and 1,352 St. Thomas. Source `LWFLAG` values G/I/L/P/S are checked; only L (Land) supports those masks. These faces are not output administrative subdivisions. Full Caribbean state coverage needs additional onshore masks; this release uses the complete selected county footprints for primary suggestions there.

World CQ zones, selected ITU zones and six Natural Earth subunits exercise worldwide and date-line mechanisms. ITU means International Telecommunication Union; DX Century Club (DXCC) values come from explicit authored mappings. Unsupported minor-island/ITU geography remains a reviewed omission. The Athos rectangle is a deliberately broad unclassified review area, not an entity boundary. Full replacements have source/feature/file guards. Census-derived CQ overrides and other exceptions are compiled into geometry; readers run no precedence rules. See [the recipe notes](../../references/geography/README.md) for provenance and the exact scope.

Before projection, shared source edges within each operation are noded at the same exact collinear endpoints across selected features. Subdivision is deterministic and direction-independent even when adjacent features originally segment their common boundary differently. Added nodes share the existing per-feature vertex budget.

Different operations retain their exact outputs under D269; sampled seam displacement and possible gaps/overlaps are reported in release metadata. No boundary blending or snapping forces agreement. Longitude unwrapping preserves source continuity; world-strip clipping preserves holes and restores its exact rectangle constraints after overlay quantization.

Assignment intersections/differences and mapping subtractions use nonzero-fill Clipper64 overlays on a binary lattice capped at 2^48 units/degree and 10^17 coordinate magnitude. Original vertices and source intersections are restored after clipping, with crossings inserted into both input paths before clipping. Unresolved distinct nodes, changed boundary turns, ambiguous intersections and invalid restored topology fail compilation. This preserves narrow supported exclusions and shared partition coordinates; no small-area threshold is applied. These numerical limits are separate from source accuracy and the reader's coverage precision.

`build_release` holds an OS file lock on the persistent `.publish.lock`, builds in private same-filesystem staging, flushes files, reopens/validates through the production reader and runs independent recipe cases before installing a release. Releases are immutable; different bytes under an existing ID are rejected. A same-directory atomic `current.json` replacement commits selection. Earlier failure/cancellation preserves the old pointer; a crash may leave an unreferenced release that a normal retry verifies/reuses. Existing releases are not automatically removed. Post-commit durability/cleanup warnings do not undo selection or report a failed publication. Directory entries are flushed on Unix where supported; other platforms retain file flushes and atomic replacement without a portable directory-flush guarantee. Power-loss guarantees depend on the filesystem/platform.

The CLI configures structured diagnostics on stderr (`RUST_LOG`, default `info`), and writes explicit command results to stdout. Domain operations install no subscriber or metrics recorder. Build counters/duration distinguish published, unchanged, cancelled and failed attempts; staged work does not count as publication. Lookup metrics belong to the reader. No telemetry abstraction or generic execution metric is introduced.

Ordinary tests need no public network; loopback acquisition tests need TCP permission. Run the real-source suite separately after explicit acquisition:

```sh
CLACKS_GEOGRAPHY_CACHE=<cache> cargo +1.99.1 test -p clacks-geography-builder --test real_sources --release --locked --offline -- --ignored
```

That suite checks the compiled artifact through production queries and independently checks original source facts. Geometry fixtures can be reproduced through its `reviewed_recipe_inputs_match_original_sources` test with `CLACKS_REFRESH_GEOGRAPHY_FIXTURES=1`; it does not author or rewrite the recipe's policies/mappings. No Python, native geographic executable or TrustedQSL execution is used. Source-data notices are separate from Clacks's Elastic License 2.0. Application autofill and expanded national/world coverage remain the integration scope of issue #23.

Use `--otlp` to opt into standard HTTP/protobuf trace, metric and log export.
The usual `OTEL_EXPORTER_OTLP_*` environment settings select the collector; the
service is `clacks-geography-build`. The executable owns the recorder and provider
lifetimes, including bounded shutdown after failures and cancellation. Without
export, command results and structured stderr remain available. A publication
counter describes actual publication outcome, including unchanged retries.

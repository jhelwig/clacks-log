# clacks-geography-builder

Rust source acquisition and preparation for Clacks's immutable geographic bundles. The application is `bin/clacks-geography-build`; startup, argument parsing and behavior live in this library. Source decoding uses the same production path for ordinary builds and recipe maintenance.

Run commands from the workspace root with Rust 1.99.1:

```sh
cargo run -p clacks-geography-build --locked --offline -- --help
cargo run -p clacks-geography-build --locked --offline -- fetch --recipe references/geography/representative.json --cache <cache>
cargo run -p clacks-geography-build --locked --offline -- inspect --recipe references/geography/representative.json --cache <cache>
```

Cargo remains offline in these examples; only the explicitly selected `fetch` command downloads data. `inspect` verifies and decodes existing sources and writes selected identities, attributes and canonical original feature guards as JSON to stdout. It does not query a separate reader, invent source identities from row positions, or execute JavaScript. This supports subsequent source releases and reviewed correction/mapping authorship.

Acquisition uses Rustls HTTPS, bounded response bodies/timeouts and digest-named cache entries. Interrupted, oversized or checksum-mismatched responses are not installed. A corrupt existing entry fails rather than being silently overwritten. Cache presence never bypasses integrity validation. Ctrl-C cancellation belongs to application composition.

Archive expansion uses a Rust Deflate backend, a complete pinned inventory and actual expansion/count limits. Nonregular paths, traversal, undeclared/duplicate members and missing members fail. Shapefile/attribute correspondence is checked, including deleted/incomplete rows that otherwise could shift the paired iterator. All currently selected source inputs declare UTF-8, decoded strictly. Coordinate declarations are individually pinned rather than normalized into a misleading common datum label.

Embedded HA8TKS source collections contain JavaScript trailing separators. A bounded string-aware conversion removes only those separators outside strings, followed by strict JSON decoding. The original source-byte hashes are retained. No expressions, comments or wrapper code are executed. Source-defined counts, identities and explicitly known open rings are checked.

The representative recipe pins source archives, notices and exact-count selections. Census topological-face metadata defines `LWFLAG` as G (Glacier), I (Intermittent Water), L (Land), P (Perennial Water), S (Marsh/Swamp). All codes are domain-checked; selected L faces support conservative source-coordinate onshore masks. The 2026 selected L counts are 534 for Aleutians West, 5,963 for San Juan and 1,352 for St. Thomas. These masks are source support data, not new output administrative subdivisions or a proof of survey accuracy.

Tests create small shapefiles/archives in Rust and exercise the public source entry points. Loopback HTTP cases require local TCP permission, but no public network. Run `cargo +1.99.1 test -p clacks-geography-builder --locked --offline` and the corresponding Clippy checks. The specification and implementation plan define coordinate compilation, artifact publication and acceptance obligations; source-decoding success alone does not establish a shipping dataset.

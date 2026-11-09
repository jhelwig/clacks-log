# Geographic build artifact

Geography is a cached dependency of the native packaging workflow. Dispatch
**Desktop packages** for the branch being built; no geographic release, upload,
tag or package-selection edit is required. Generator and input changes can be
reviewed and consumed in the same PR. Generated data and acquired originals stay
outside Git, including Git Large File Storage (LFS).

The common Linux job hashes producer sources, transitive local dependencies and
embedded catalogs, Cargo manifests/lockfile, toolchain/configuration and the
shipping recipe with its referenced correction files. The cache key also names
the producer platform and build settings. Changes to those settings or input
paths must update the key. Whole root manifest/lockfile hashing conservatively
invalidates data for dependency edits, including unrelated ones. Unrelated
application source and documentation do not invalidate the data cache.

An exact cache hit restores a validated bundle and its expected manifest digest.
A miss builds the current checkout, fetches checksum-pinned originals through the
existing source cache, compiles offline and runs production validation/recipe
acceptance. Only successful output is cached. Eviction or branch scoping can
cause another build; a cache entry is never required for correctness. A corrupt
source-cache entry fails its existing digest check. Ordinary workspace tests do
not build full geographic data.

Both native jobs receive the same bundle directory and generated expected digest
through a normal workflow artifact. They supply
`CLACKS_GEOGRAPHY_MANIFEST_SHA256` to native compilation and check the packaged
payload against that identity. Runtime checks identity, supported schema and
payload integrity; polygon/index/inventory acceptance remains in production.
The cache and expected digest are trusted build outputs under the workflow's
existing permissions, not independent attestations against a malicious writer.

Use `author-shipping` only when maintaining reviewed inputs; see
[shipping maintenance](shipping/README.md#maintenance-and-verification).
For explicit local production with Rust 1.99.1:

```sh
cargo run -p clacks-geography-build --release --locked --offline -- fetch --recipe references/geography/shipping.json --cache "$CLACKS_GEOGRAPHY_CACHE"
cargo run -p clacks-geography-build --release --locked --offline -- build --recipe references/geography/shipping.json --build-id "$CLACKS_GEOGRAPHY_BUILD_ID" --cache "$CLACKS_GEOGRAPHY_CACHE" --output "$CLACKS_GEOGRAPHY_OUTPUT"
cargo run -p clacks-geography-build --release --locked --offline -- verify --recipe references/geography/shipping.json --bundle "$CLACKS_GEOGRAPHY_OUTPUT/releases/$CLACKS_GEOGRAPHY_BUILD_ID"
```

`CLACKS_GEOGRAPHY_BUILD_ID` is the input fingerprint supplied automatically by the
packaging job; local explicit builds can supply their own safe directory identity.
The build store's `current.json` records the validated manifest digest. The
bundle contains `manifest.json`, five FlatGeobuf layers and declared notices;
the pointer, source cache and build diagnostics are not installed data. No custom
ZIP command or separate evidence/measurements release assets are produced.

Application packages retain their shipped geographic bytes and existing package
evidence records identity. Cache retention is not release retention. Server
update endpoints, transport and selection remain future work and can consume
output from this same producer independently of application versions. A change
in reference data never rewrites recorded contact/profile snapshots.

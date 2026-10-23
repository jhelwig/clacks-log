# LoTW reference-data and fixture tools

This library maintains the pinned Logbook of The World (LoTW) reference data.
`bin/clacks-lotw-data` is its thin executable entry point. Run from the checkout:

```sh
cargo +1.99.1 run --bin clacks-lotw-data --locked --offline -- --help
cargo +1.99.1 run --bin clacks-lotw-data --locked --offline -- --check
cargo +1.99.1 run --bin clacks-lotw-data --locked --offline -- --write
```

No arguments or `--help` displays usage without changing files. `--check` verifies
both committed normalization and signature files; only `--write` regenerates
them. The pinned configuration XML and
primary-subdivision table in the reference source are read as data only. No
reference source is compiled, linked, executed or wrapped. Missing sections,
malformed numeric values, lossy band conversions and stale derived files fail
with context. Decimal bounds use integer arithmetic, without binary floating
point. JSON output preserves existing bytes and source-defined signing order.

## Reproducible test fixtures

`bin/clacks-lotw-fixtures` enables this library's optional `fixtures` feature.
It constructs all 66 certificate, archive, signature and wire fixtures with
fixed public test inputs, independently of the production importer, signer and
serializer. Its cryptographic dependencies are confined to that feature.

```sh
cargo +1.99.1 run --release --bin clacks-lotw-fixtures --locked --offline -- --help
cargo +1.99.1 run --release --bin clacks-lotw-fixtures --locked --offline -- --check
cargo +1.99.1 run --release --bin clacks-lotw-fixtures --locked --offline -- --write
```

No arguments or `--help` displays usage. `--check` reconstructs every fixture and
compares exact bytes, rejecting missing, changed or unowned data files. Only
`--write` replaces files. Neither construction nor verification invokes external
programs. See the [fixture recipes](../clacks-lotw/tests/fixtures/README.md) for
inputs and independence limits. Use the optimized build for RSA key generation.
All executable project maintenance and non-trivial fixture generation is Rust.

One debug span covers the maintenance command. Its completion event reports only
check/write mode and actual file count. The tool does not install telemetry
subscribers, define workflow metrics, or emit source/data payloads.

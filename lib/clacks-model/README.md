# Clacks native model

This library represents recorded amateur-radio contacts (QSOs), station positions,
band/frequency pairs, modes, profiles, copied equipment, logbook tags and
participation/reference associations. It includes supplied group/catalog schemas,
whole-source fallback selection, definition validation and an in-memory
reference-export planner. It has no database, network, desktop or server runtime.

Use the public re-exports from `clacks_model`. Constrained scalar values validate
through constructors and ordinary Serde decoding. Records are ordinary structs:
normalize optional aggregates with `Qso::normalize_optional_aggregates`, then
call `Qso::validate` before saving; incomplete entry and import preparation stay
outside saved records. `normalize_optional_aggregates` explicitly removes empty
optional aggregates. Current recognition/metadata never participates in recorded
value equality. Profile loading and equipment selection copy values.

Group definitions are supplied account data, not a hardcoded registry. The test
fixtures demonstrate generic mappings; they are not the production account seed.
Catalog metadata remains separate from recorded station facts. Effective-value
selection expects already eligible provider observations; provider storage,
lookup request arbitration and account authorization belong to adapters.

`plan_reference_export` returns reference fields, partitions, decisions and
warnings without changing input records. Outstanding decisions block publication
in the eventual export workflow. It does not write Amateur Data Interchange
Format (ADIF) files or claim provider/submission acceptance. Complete common-field
mappings, qualified-reference submission rules, destination replacement and
point-in-time storage reads remain interchange work. Import-side `CONTEST_ID`
station assignment remains unresolved in that adapter, not guessed by this model.

Run `cargo test --workspace` from the repository root. Rust 1.99.1 is pinned.
The [recognition data provenance](data/README.md) identifies the complete selected
baseline and its source-table discrepancy. The [execution evidence](../../docs/native-model-execution.md)
records scope and verification.

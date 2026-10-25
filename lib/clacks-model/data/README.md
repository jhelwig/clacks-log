# Recognition data provenance

Source: [Amateur Data Interchange Format (ADIF) 3.1.8](https://www.adif.org/318/ADIF_318.htm), obtained 18 October 2026.

Extracted the Band, Mode and Submode enumeration tables from that pinned HTML document. Band megahertz values were converted exactly with decimal arithmetic to whole hertz. The source supplies finite bounds for every band, including submm; the representation also supports a future open upper bound. Deprecated import-only mode labels are retained and marked.

The Mode table lists additional phase-shift keying (PSK) specializations absent from the Submode table. Retain those explicit parent-list declarations too, labeled `source: mode-table-list`; do not infer aliases. The missing comma between the multiple-frequency shift keying (MFSK) labels MFSK128 and MFSK128L in the Mode table is resolved by their separate explicit Submode rows. Descriptive prose was not copied. These are factual recognition labels/relationships, not geographic reference data or reference-group seeds.

Counts and parity are checked by recognition tests. Builds and tests use only the committed data and never download recognition tables. Changes to these facts must cite the selected release and preserve existing recorded identities.

Extracted counts: 33 bands, 91 mode labels (including import-only), 188 explicit Submode table rows plus 10 PSK specializations explicitly listed only in the Mode table: 198 specialization declarations in total.

Recognition reads these rows directly. No Rust enum or generated code enumerates
concrete modes, submodes or bands. Supplied catalogs can add labels and parent
relationships without recompiling the recognition implementation; those changes
never rewrite recorded values.

## LoTW normalization

`lotw-11.34.json` is derived from the pinned TrustedQSL 2.8.6 configuration
at commit `6ce824363e27da455c6cf6fd440b9c401945bfcb`. It retains complete
mode mapping/group, band, station enumeration/dependency and signing-field data.
The upstream [notices](../../../references/trustedqsl/upstream/LICENSE.txt)
apply to this extracted data. Regenerate with `cargo run --bin clacks-lotw-data -- --write`;
`--check` verifies deterministic correspondence without changing upstream files.

LoTW extraction also retains field input flags, entity/subdivision zone pairs and
the primary-subdivision field list from `sasMapping` in the pinned `location.cpp`.
Secondary administrative bindings are the service contract's `US_COUNTY` and
`JA_CITY_GUN_KU`; park-selection fields are not administrative subdivisions.
Input uppercase flags and entity/subdivision zone constraints apply before
shared signing values are emitted. The extraction check covers these derived rows.

## Initial account definitions

`account-definitions.json` contains production seed data for new accounts, not
recognition types or test fixtures. It includes POTA/SOTA/IOTA reference groups,
the factual ADIF 3.1.8 Contest ID labels (including import-only identifiers),
and ordinary/POTA-compatible output profiles. Stable IDs are recorded once and
preserved by the Rust maintenance tool. Existing accounts are not overwritten
when bundled data changes.

Source: [ADIF 3.1.8](https://www.adif.org/318/ADIF_318.htm), obtained 24 October 2026,
Contest ID and reference-field/type tables. Only factual identifiers and authored
behavioral mappings are retained, not copied contest descriptions. POTA's
separate submission conventions follow its [technical reference](https://docs.pota.app/docs/activator_reference/ADIF_for_POTA_reference.html)
and the [existing interchange findings](../../../references/adif-matching.md).
Catalog membership is not asserted by syntax validation. SOTA/IOTA scalar ADIF
mappings remain export-adapter work; the seed preserves their entry/location rules.

Supply the released HTML document to the dedicated Rust maintenance command:

```sh
cargo run --bin clacks-account-data -- --help
cargo run --bin clacks-account-data -- --adif /path/to/ADIF_318.htm --check
cargo run --bin clacks-account-data -- --adif /path/to/ADIF_318.htm --write
```

The normal build/test path needs no downloaded document. The tool preserves
existing IDs by seed identity, validates every generated definition, and can
verify the committed data without writing. Changes to the initial dataset should
be reviewed as account provisioning data; they do not silently migrate accounts.

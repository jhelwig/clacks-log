# Native Model Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

Terms: radio contact (QSO); minimum viable product (MVP); Amateur Data Interchange Format (ADIF), whose text format is ADI; Coordinated Universal Time (UTC); Parks on the Air (POTA); Logbook of The World (LoTW); Universally Unique Lexicographically Sortable Identifier (ULID). DX Century Club is DXCC. CQ magazine and the International Telecommunication Union (ITU) define the two zone systems. A graphical user interface is a GUI. A regular expression is called a regex. API means application programming interface. JSON is JavaScript Object Notation, used for fixtures and Serde verification here, not selected as the synchronization transport by this plan.

**Goal:** Deliver issue #1's tested native contact/shared-data library, preserving recorded meaning across recognition, profile, provider and catalog changes.

**Architecture:** One Rust library, `lib/clacks-model`, owns native values, records and pure reference/selection operations. Account-scoped storage and application adapters consume it later; it does not perform network access, persistence or file publication. Definition evaluation produces an in-memory reference-export plan, not an ADIF file writer.

**Tech Stack:** Rust **1.99.1**, edition 2024; standard library collections; Serde 1 with derive, serde_json 1, regex 1 and ulid 3. Dependencies use compatible major-version ranges and a committed Cargo.lock resolved with the pinned compiler. No application or database crate is needed for this issue.

**Spec:** [Native contact and shared-data model](../specs/2026-10-18-native-model-design.md). The user's instruction to write this plan advances the written specification into implementation planning; it does not authorize execution. The four engineering choices in spec section 2 are the basis of this plan and remain visible in that section.

**Status:** Plan written for user review, 18 October 2026. No task has been executed. Checkboxes describe future work; no build or test result below is a current success claim.

## Global Constraints

- Rust 1.99.1, standard Serde serialization, libraries in `lib/<crate-name>` and thin applications in `bin/<application-name>` remain selected.
- A saved contact needs **frequency or band**, not an independently populated band.
- Incomplete entry/import preparation does not create a saved QSO. Validate before saving.
- Native modeling is independent of ADIF's field layout.
- There is one account synchronization scope. Tags organize account-owned QSOs; they do not own child contact records.
- Dedicated contest exchanges and serial-number assistance remain separate from participation/reference associations and remain post-MVP.
- Recognition changes alone affect interpretation, not underlying saved values or edits.
- An absent receive override means the primary pair is used in both directions.
- A band-only receive override does not borrow primary frequency, even for the same band.
- Current operating defaults are client-local and persistent; saved profiles and equipment synchronize.
- No OpenSSL, direct native foreign-function interface, storage queue, database/network framework, GUI or application scaffold belongs in this library.
- Preserve blank lines between Rust items, including between macro definitions and invocations. Use conventional formatting plus the explicit visual separation requested by the user.
- Use named module files (`recognition.rs` with children under `recognition/`, for example). Do not use `mod.rs` outside tests; `tests/support/mod.rs` is permitted.
- Commit messages need a subject and a substantive body explaining motivation, constraints or tradeoffs that the diff cannot convey. The task messages below provide that context; update them to reflect actual implementation decisions and evidence rather than copying an inaccurate template.
- Account authorization, durable saving, deletion retention and synchronization concurrency belong to their downstream capabilities. Pure functions here must not impersonate those guarantees.

## Review Focus

These five failure classes receive explicit tests in the owning tasks:

1. A newer client recognizes a stored mode/band that an older view did not: unrelated edits must preserve its identity and serialization (Tasks 3 and 11).
2. A coordinate just outside the geographic range rounds onto the boundary: reject before rounding; valid poles/date-line points must calculate a locator without overflow or moving the point (Task 2).
3. A profile lacks county/references/equipment defaults that the preceding setup had: clear station values but preserve equipment only in the no-default case (Task 5).
4. Catalog metadata or an old-key directory result arrives after a callsign/location change: never overwrite recorded facts or supply old-station fallback (Tasks 6 and 7).
5. Several contest associations or multi-park references reach ordinary/targeted export: no arbitrary contest choice, unrelated Cartesian product or silent contact filtering (Tasks 9 and 10).

---

## Delivery boundary and implementation decisions

The tasks form one cohesive model capability with shared interfaces, not independent server/client projects. Keep one crate initially; modules below separate responsibilities without imposing a crate per type. Later implementations may reuse these APIs without adopting a second persistent representation of the same facts.

The plan supplies these engineering decisions for review:

- Use `NonZeroU64` internally for hertz/milliwatts, `i32` for coordinate millionths and `i64` Unix seconds for contact times. Operational source timestamps use seconds plus nanoseconds, with nanoseconds less than 1,000,000,000. These are representations, not claims about clock accuracy or timestamp precision on the air.
- Use typed ULID wrappers with string Serde representations and `BTreeSet` for tags/references. Do not use ID timestamps as revision order. Names do not merge entities.
- Keep record structs ordinary data plus explicit `validate` methods. Nested value constructors enforce their own ranges. Storage/import adapters must validate before saving; do not create a draft, import-session or valid/invalid persistent-record hierarchy.
- Use exact decimal-string rounding at the coordinate boundary. Accept signed fixed-point decimal notation, including `.5`; adapters for other textual formats normalize before calling it. Keep the first six fractional digits and round magnitude away from zero when the seventh digit is at least five; inspect all digits for source-range validity before rounding. Do not route this operation through binary floating point.
- For locator calculation only, treat longitude +180 as the -180 meridian and latitude +90 as the upper edge of the final latitude cell. Preserve the supplied point integers. Cell arithmetic uses integer ratios, not incremental floating-point conversions.
- Compile regexes with the Rust `regex` crate and full-match semantics. Keep compiled state outside serialized/equality-bearing definitions. Use typed enum variants for expression operations and QSO field paths, not arbitrary script evaluation or reflection.
- Ship the complete mode/band recognition facts with the library. Reference-group/catalog definitions are supplied data: representative POTA/contest fixtures demonstrate them, but a client must never silently merge those fixtures into synchronized account definitions.

The library does not fetch canonical park catalogs, select a production default-group inventory, generate contest serials, write ADIF files, or implement service evidence/causal state. Account provisioning later seeds actual default definitions through the same record representation. Complete program mappings, qualified-reference submission details and import-side `CONTEST_ID` ownership remain integration work; do not fill those gaps with hidden program-name branches.

### Dependency and source checkpoints

Use [Serde's ordinary conversion/validation attributes](https://serde.rs/container-attrs.html), the [ulid 3 API](https://docs.rs/ulid/3.0.0/ulid/struct.Ulid.html), and the [regex API](https://docs.rs/regex/latest/regex/). ULID generation in version 3 is `Ulid::generate()`; avoid copying the older `Ulid::new()` call. The wrapper's string conversions can use the standard Serde `try_from`/`into` pattern, without depending on the underlying crate's serialized layout.

The [ADIF 3.1.8 resources section](https://www.adif.org/318/ADIF_318.htm) identifies machine-readable table exports at `https://adif.org.uk/318/resources`. If that download is unavailable, extract the three tables from the pinned 3.1.8 specification itself. Record the actual source URL and retrieval date. A complete table is required; an unavailable download is not permission to substitute a sparse catalog. Exact resolved dependency patch versions and source-table counts are measured at execution, recorded with the lockfile/data provenance, and verified rather than invented in this plan.

## File structure

The file tree uses repository-relative paths. In task Files blocks, `src/`, `tests/` and `data/` are relative to `lib/clacks-model/`; workspace files are at the repository root. Source files do not exist yet. Create them only during authorized execution.

```text
Cargo.toml                         workspace, one library member, resolver 3
Cargo.lock                         resolved dependency graph
rust-toolchain.toml                channel 1.99.1, minimal profile, rustfmt/clippy
.gitignore                        /target/ plus existing entries preserved
lib/clacks-model/Cargo.toml        package/dependency metadata, publish=false
lib/clacks-model/src/lib.rs        explicit module declarations and re-exports
lib/clacks-model/src/error.rs      ValueError and RecordError, no catch-all string errors
lib/clacks-model/src/ids.rs        typed entity IDs and string conversions
lib/clacks-model/src/scalar.rs     callsign, optional text, units, timestamps
lib/clacks-model/src/position.rs   locators, coordinates, area/point calculations
lib/clacks-model/src/location.rs   recorded geographic details and presence
lib/clacks-model/src/recognition.rs          catalog loading and lookups
lib/clacks-model/src/recognition/band.rs     band identity, interpretation and ranges
lib/clacks-model/src/recognition/mode.rs     hierarchical Mode and preservation adapter
lib/clacks-model/src/recognition/known.rs    complete known families/specializations
lib/clacks-model/src/reference.rs           ReferenceAssociation and station side
lib/clacks-model/src/contact.rs             Qso, station/operator/context records
lib/clacks-model/src/equipment.rs           reusable values/configurations
lib/clacks-model/src/profile.rs             saved profiles and pure load operation
lib/clacks-model/src/tag.rs                 policies and deletion classification
lib/clacks-model/src/catalog.rs             catalog/provenance/display bindings
lib/clacks-model/src/group.rs               definitions and reference normalization
lib/clacks-model/src/effective.rs           whole-location/name source selection
lib/clacks-model/src/export.rs              contexts, plans and diagnostics
lib/clacks-model/src/export/rules.rs        typed expressions/bindings/compilation
lib/clacks-model/src/export/select.rs       mapping selection and context decisions
lib/clacks-model/src/export/evaluate.rs     partitions, expansions and field evaluation
lib/clacks-model/data/adif-3.1.8.json        complete band/mode/submode recognition facts
lib/clacks-model/data/README.md             source provenance and data schema
lib/clacks-model/tests/support/mod.rs       fixed IDs and reusable contact fixtures
lib/clacks-model/tests/values.rs
lib/clacks-model/tests/positions.rs
lib/clacks-model/tests/recognition.rs
lib/clacks-model/tests/contacts.rs
lib/clacks-model/tests/defaults_and_tags.rs
lib/clacks-model/tests/groups_and_catalogs.rs
lib/clacks-model/tests/effective_values.rs
lib/clacks-model/tests/export_rules.rs
lib/clacks-model/tests/export_context.rs
lib/clacks-model/tests/reference_export.rs
lib/clacks-model/tests/compatibility.rs
lib/clacks-model/tests/fixtures/definitions.json  illustrative supplied group/profile data
lib/clacks-model/tests/fixtures/recognition-old.json  deliberately reduced recognition view
lib/clacks-model/tests/fixtures/recorded-values.json  preserved known/unfamiliar values
```

Do not add build-time network requests or a code generator framework. `known.rs` contains the full typed enum/mapping implementation, checked against the complete data artifact. Mechanical enum variants not named in a test use idiomatic Rust names with an explicit canonical code mapping. No match may silently funnel recognized catalog rows into a generic placeholder.

## Task 1: Establish the library through tested scalar and identity values

**Files:** Create the workspace/toolchain files, library manifest, `src/{lib,error,ids,scalar}.rs`, `tests/values.rs` and `tests/support/mod.rs`.

**Interfaces:** Consumes the pinned compiler and the four dependencies above; produces:

- `QsoId`, `TagId`, `ProfileId`, `EquipmentConfigurationId`, `GroupId`, `CatalogId`, `OutputProfileId`: `new() -> Self`, `from_u128(u128) -> Self`, `FromStr<Err = ValueError>`, `Display`, clone/copy/equality/order/hash and string Serde. Generation uses the verified ULID API; fixed IDs in tests use `from_u128`.
- `RecordedCallsign::new(&str) -> Result<Self, ValueError>` and `as_str(&self) -> &str`.
- `optional_text(&str) -> Option<String>`; `Hertz::new(u64)` and `Milliwatts::new(u64)` return `Result<Self, ValueError>`; both expose `get() -> u64`.
- `UtcSecond::from_unix_seconds(i64) -> Self`, `unix_seconds() -> i64`; `UtcTimestamp::new(i64, u32) -> Result<Self, ValueError>`. The latter is for operational provenance, not additional QSO precision.
- `ValueError` starts with `EmptyText { field: String }`, `ZeroQuantity { unit: String }`, `InvalidId { value: String }` and `InvalidTimestamp { nanoseconds: u32 }`. Later tasks add the position/catalog variants specified below. `RecordError` gains record-specific variants in its owning tasks. All error enums implement Display and std::error::Error; tests can return `Result<(), Box<dyn std::error::Error>>` to combine assertion setup and serialization errors.

- [x] **Step 1:** Create the minimal workspace and empty library entry point, then add `tests/values.rs` with these named assertions (imports resolve from `clacks_model` once implemented):

```rust
// callsign_and_text_preserve_recorded_meaning
assert_eq!(RecordedCallsign::new(" ok2cqr/p ")?.as_str(), "OK2CQR/P");
assert!(RecordedCallsign::new("  ").is_err());
assert_eq!(optional_text("  Zoë  "), Some("  Zoë  ".to_owned()));
assert_eq!(optional_text(" \t\n"), None);
// quantities_and_operational_time_are_validated
assert_eq!(Milliwatts::new(1)?.get(), 1);
assert!(Hertz::new(0).is_err());
assert!(UtcTimestamp::new(42, 1_000_000_000).is_err());
```

Also test ID string round trips, type separation with a compile-fail documentation example passing TagId where QsoId is required, negative Unix seconds, and JSON deserialization rejecting zero quantities. No callsign grammar beyond the selected normalization/nonblank requirement is introduced.

- [x] **Step 2:** Run `cargo +1.99.1 test -p clacks-model --test values`; expect unresolved native values before implementation, not an unrelated manifest/toolchain failure.
- [x] **Step 3:** Implement the interfaces. Use `serde = { version = "1", features = ["derive"] }`, `serde_json = "1"`, `regex = "1"`, `ulid = "3"`; package `edition = "2024"`, `rust-version = "1.99.1"`, `license = "Elastic-2.0"`, `publish = false`. Root membership is only `lib/clacks-model`. Use ordinary Serde validation/conversion for constrained wrappers; never deserialize an invalid numeric wrapper by bypassing its constructor. Keep any ID macro definition visually separated from invocations.
- [x] **Step 4:** Re-run the test and `cargo +1.99.1 fmt --all -- --check`; expect success. Commit Cargo.lock and preserve existing ignore entries. No database, network, GUI or executable crate is added.
- [x] **Step 5:** Commit with this subject and rationale, adjusted for the actual delivered change:

```text
feat(model): establish native scalar and identity values

Clients must preserve the same recorded meaning while operating offline and
creating records independently. Typed identities and constrained scalar values
provide that shared foundation without imposing a storage or sync design.

Keep the first workspace member under lib/ and use the selected Rust 1.99.1
toolchain so later applications can remain thin wrappers.
```

## Task 2: Implement exact positions and locator calculations

**Files:** Create `src/{position,location}.rs`, `tests/positions.rs`; modify `src/lib.rs` and `src/error.rs`.

**Interfaces:**

- `GridLength::new(u8) -> Result<Self, ValueError>` accepts only 2/4/6/8/10/12.
- `Locator::parse(&str) -> Result<Self, ValueError>`, `length() -> GridLength`, `center() -> Coordinates`, `bounds() -> GridBounds`. Parse case-insensitively; render uppercase while retaining length. `GridBounds { south: RationalDegree, north: RationalDegree, west: RationalDegree, east: RationalDegree }` uses `RationalDegree { numerator: i64, denominator: u64 }` in degrees with a positive denominator, retaining exact cell boundaries.
- `Coordinates::from_microdegrees(i32, i32) -> Result<Self, ValueError>`, `from_decimal_degrees(&str, &str) -> Result<Self, ValueError>`, latitude/longitude integer getters and `locator(GridLength) -> Locator`.
- `Position::{GridArea(Locator), ExplicitPoint { coordinates: Coordinates, grid_length: GridLength }}`; `display_coordinates() -> Coordinates`, `display_locator() -> Locator`, `validate_operating_entry() -> Result<(), ValueError>` (four-character minimum), `lookup_geometry() -> LookupGeometry` where `LookupGeometry::{Area(GridBounds), Point(Coordinates)}` distinguishes lookup inputs. `Position::same_geographic_position(&self, other: &Self) -> bool` compares area identity or explicit coordinates, ignoring only the point form's display grid length.
- `RecordedLocation` has the spec's optional position/DXCC/primary subdivision/secondary subdivision/CQ/ITU values. `RecordedLocation` fields are `position: Option<Position>`, `dxcc: Option<String>`, `primary_subdivision: Option<String>`, `secondary_subdivision: Option<String>`, `cq_zone: Option<CqZone>` and `itu_zone: Option<ItuZone>`. Geographic identifiers are nonblank preserved text, not display-name IDs or prematurely hardcoded datasets. `CqZone::new(u8)` and `ItuZone::new(u8)` return `Result<Self, ValueError>` and enforce 1–40 and 1–90 respectively. `has_values() -> bool` treats an all-absent structure as empty. Add `ValueError::{InvalidCoordinate { axis: String, value: String }, InvalidLocator { value: String }, InvalidGridLength { length: u8 }, MalformedDecimal { value: String }, InvalidZone { system: String, value: u8 }}`.

- [x] **Step 1:** Add these tests and table-driven checks for all six locator lengths:

```rust
// preserve_area_point_and_supplied_length
let grid = Locator::parse("fn31pr")?;
let area = Position::GridArea(grid.clone());
let point = Position::ExplicitPoint {
    coordinates: grid.center(), grid_length: GridLength::new(6)?,
};
assert_ne!(area, point);
assert_eq!(point.display_locator(), grid);
// operating_entry_does_not_reject_historical_areas
let coarse = Position::GridArea(Locator::parse("FN")?);
assert!(coarse.validate_operating_entry().is_err());
assert_eq!(coarse.display_locator().to_string(), "FN");
// reject_before_rounding_and_round_half_away_from_zero
assert!(Coordinates::from_decimal_degrees("90.0000001", "0").is_err());
assert_eq!(Coordinates::from_decimal_degrees("-0.0000005", "0")?.latitude_microdegrees(), -1);
```

Test no precision padding, invalid cell letters/digits, negative zero, signed `.5`, long fractional tails, ±90/±180, equal centers with different area/point meaning, and explicit point retention through repeated display conversion. At twelve characters, derive from the integers without floating-point accumulation. In `display_precision_does_not_change_point_identity`, changing only an explicit point's grid length leaves `same_geographic_position` true; changing coordinates or an area cell makes it false. Geographic lookup later uses this guard to retain corrections; this library does not run lookup or replace geographic values.

- [x] **Step 2:** Run `cargo +1.99.1 test -p clacks-model --test positions`; expect missing position implementation.
- [x] **Step 3:** Implement the interfaces. Decimal parsing validates every source digit/range before rounding. For locator indexing use absolute offsets, pair radices 18/10/24/10/24/10 and integer division; calculate bounds/centers as rational cell offsets before final microdegree rounding. Longitude +180 wraps only for indexing; latitude +90 clamps only the cell index to the last row. Grid-area lookup returns bounds, not its center as a point. No geographic dataset lookup is implemented here.
- [x] **Step 4:** Run the positions test, then the values test; expect success with unchanged point integers and bounded indices for the extrema.
- [x] **Step 5:** Commit with this subject and rationale, adjusted for the actual delivered change:

```text
feat(model): preserve grid-area and explicit-point positions

A supplied grid identifies an area, whereas coordinates identify a point.
Collapsing those forms would falsely resolve geographic ambiguity and lose
what historical imports actually recorded.

Use exact boundary arithmetic and six-decimal coordinate resolution while
retaining locator length, including coarse historical areas.
```

## Task 3: Deliver complete recognition and stable mode/band identity

**Files:** Create `src/recognition.rs`, `src/recognition/{band,mode,known}.rs`, `data/{adif-3.1.8.json,README.md}`, `tests/recognition.rs` and the old-recognition/recorded-values fixtures; modify `src/lib.rs`.

**Interfaces:**

- `RecognitionCatalog::adif_318() -> &'static Self`, `from_json(&str) -> Result<Self, ValueError>`, `without_modes_and_submodes(&[&str]) -> Self` for representative old-view verification.
- Add `ValueError::InvalidRecognitionCatalog { reason: String }` for malformed/inconsistent table data. The data schema contains `release`, complete `bands` (`code`, inclusive `lower_hz`, optional inclusive `upper_hz`), `modes` and `submodes` (`code`, `parent`, `deprecated`). Preserve deprecated labels and explicit aliases from the source, not inferred aliases.
- `Band::{Known(KnownBand), Other(String)}`; `from_recorded_label(&str, &RecognitionCatalog) -> Result<Self, ValueError>`, `label() -> &str`, `interpret(&RecognitionCatalog) -> Option<KnownBand>` and `for_frequency(Hertz, &RecognitionCatalog) -> Option<Self>`.
- `Mode` is the spec's partially hierarchical enum with family-specific child enums and structured `OtherMode`. Public operations: `from_recorded_identity(ModeIdentity, &RecognitionCatalog) -> Result<Self, ValueError>`, `identity() -> ModeIdentity`, `interpret(&RecognitionCatalog) -> ModeInterpretation`.
- `ModeIdentity::{Parent { label: String }, Specialization { label: String, parent: Option<String> }}` is the ordinary Serde representation helper, not a second QSO field. `OtherMode` preserves those two fallback forms. `ModeInterpretation::{Known(KnownMode), Other(OtherMode)}` is a read result. `KnownMode::Mfsk(Option<MfskSubmode>)` and `MfskSubmode::Ft4` are exact names used by tests; every other baseline family/specialization must also be typed.

- [x] **Step 1:** Obtain complete 3.1.8 table facts into the data artifact, recording source/date and observed row counts. Add tests independently comparing every source row with typed lookup/encoding coverage, plus these exact preservation checks:

```rust
// older_recognition_cannot_rewrite_a_specialization
let id = ModeIdentity::Specialization { label: "FT4".into(), parent: Some("MFSK".into()) };
let full = RecognitionCatalog::adif_318();
let old = full.without_modes_and_submodes(&["FT4"]);
let a = Mode::from_recorded_identity(id.clone(), &old)?;
let b = Mode::from_recorded_identity(id.clone(), full)?;
assert_eq!(a, b);
assert_eq!(serde_json::to_value(&a)?, serde_json::to_value(&b)?);
assert_eq!(a.identity(), id);
// exact_frequency_and_unknown_band_labels_are_retained
assert_eq!(Band::for_frequency(Hertz::new(14_074_000)?, full).expect("recognized 20m").label(), "20m");
assert!(Band::for_frequency(Hertz::new(1)?, full).is_none());
```

Also test parent-only MFSK, unknown submode with no parent, unknown parent context, deprecated catalog labels, maximum/open-ended band ranges and every recorded range boundary. The old fixture must omit a real specialization rather than merely round-trip on one current catalog.

- [x] **Step 2:** Run `cargo +1.99.1 test -p clacks-model --test recognition`; expect missing recognition types/lookup behavior.
- [x] **Step 3:** Implement full typed coverage. Band ranges convert source megahertz (MHz) to integer hertz exactly; open-ended upper bounds stay open. Serialize mode/band through preserved semantic labels, not Rust variant ordinals. Deserialize into a typed known form only if serializing that form reproduces the preserved identity exactly; otherwise retain `OtherMode`/`Other` and expose richer interpretation separately. This prevents a newly recognized specialization with different supplied context from silently acquiring a new parent or spelling. Eq/Ord compare preserved identity, so ordering is consistent with equality. Canonicalization on explicit entry/import is a separate adapter operation, not a deserialization side effect. Standard Serde helpers suffice; do not build a second serialization protocol.
- [x] **Step 4:** Run recognition, positions and values tests; inspect the baseline/source parity report and confirm there is no placeholder coverage. Tests must run offline from committed data; builds never fetch catalogs.
- [x] **Step 5:** Commit with this subject and rationale, adjusted for the actual delivered change:

```text
feat(model): add complete ADIF recognition with preserved value identity

The rejected implementation used a sparse catalog, and clients may recognize
different sets of labels as they upgrade. Recognition must therefore enrich
interpretation without changing saved meaning or creating synchronization
edits.

Cover the complete selected baseline and preserve unfamiliar labels and parent
context through ordinary serialization.
```

## Task 4: Establish contact records and whole-record equality

**Files:** Create `src/{reference,contact,equipment}.rs`, `tests/contacts.rs`; expand `tests/support/mod.rs` and `src/lib.rs`.

**Interfaces:**

- `ReferenceAssociation { group_id: GroupId, identifier: Option<String> }`, `StationSide::{Own, Contacted}`; associations derive equality/order on group plus optional identifier without consulting definitions.
- `BandFrequency::new(Option<Hertz>, Option<Band>, &RecognitionCatalog) -> Result<Self, RecordError>`, frequency/band getters, `set_frequency(Hertz, &RecognitionCatalog)`, `set_band(Band)` and `effective_receive<'a>(&'a self, override_pair: Option<&'a BandFrequency>) -> &'a BandFrequency`. Construction with a frequency derives its recognized band; if unclassified it retains the frequency without a derived band. Import adapters compare supplied versus derived band for correction warnings; deserialization preserves stored labels rather than re-running current recognition.
- `StationSnapshot`, `OperatorInformation`, `OwnOperatingContext`, `EquipmentValues` and `Qso` use the exact logical fields in spec sections 4/11. `Qso::validate(&self) -> Result<(), RecordError>` checks end time/order and nested constraints. `Qso` is ordinary data, not an entry draft or database operation. Add `RecordError::{MissingBandAndFrequency, EndBeforeStart { start: UtcSecond, end: UtcSecond }, InvalidOptionalText { field: String }}`; constrained callsign/mode/unit constructors prevent missing required scalar values. `OperatorInformation::normalized(self) -> Option<Self>` and `RecordedLocation::normalized(self) -> Option<Self>` collapse all-absent aggregates; `Qso::normalize_optional_aggregates(&mut self)` applies these without consulting providers.
- `Qso::same_recorded_values(&self, &Self) -> bool` compares whole recorded contents for the same identity, including sets; cross-ID calls return false. `Qso` equality remains ordinary whole-record value equality. No automatic merge function is provided.
- Test support supplies `qso(id: QsoId) -> Qso` with start 1000, callsigns W7HEX/K1ABC, primary 14_074_000 Hz, FT4 mode and empty optional values/sets; `reference(group: GroupId, identifier: Option<&str>) -> ReferenceAssociation`.

- [x] **Step 1:** Add tests covering frequency-only and band-only validity, both absent rejection, end 1000 valid/end 999 invalid, start/end spanning 86_400, both station callsigns, independently optional operator name/call and absent reports/equipment. Assert:

```rust
// a_present_receive_pair_is_a_whole_override
let primary = BandFrequency::new(Some(Hertz::new(14_074_000)?), Some(Band::Known(KnownBand::Meters20)), RecognitionCatalog::adif_318())?;
let receive = BandFrequency::new(None, Some(Band::Known(KnownBand::Meters20)), RecognitionCatalog::adif_318())?;
assert_eq!(primary.effective_receive(None), &primary);
assert!(primary.effective_receive(Some(&receive)).frequency().is_none());
```

Use `KnownBand::Meters20` as the exact Rust name for `20m`. Test that tag/reference reordering and duplicate insertion do not change a contact, while a different tag or comment does; same facts with another QsoId are distinct contacts. Test operator-name-only information without inventing an operator call. Serialize a valid contact and verify no start/end precision fields appear.

- [x] **Step 2:** Run `cargo +1.99.1 test -p clacks-model --test contacts`; expect missing record/value operations.
- [x] **Step 3:** Implement the interfaces and fixtures. Sets are `BTreeSet`; equipment descriptions/antenna/notes are optional preserved text. Empty operator/location aggregates normalize to absence through explicit normalization methods, not automatic provider filling. Frequency edits calculate band if possible; band edits clear frequency. Native record equality never reads current recognition or catalog state. Do not add persistence, import patches, a public QsoDraft/QsoParts hierarchy or revision fields. Saving adapters later call validation; in-memory test values can exercise invalid combinations without creating saved records.
- [x] **Step 4:** Run contacts and prior test binaries. Verify changing an unrelated comment leaves serialized unknown mode/band identities untouched.
- [x] **Step 5:** Commit with this subject and rationale, adjusted for the actual delivered change:

```text
feat(model): define contact snapshots and whole-record comparisons

Historical contacts must retain the facts recorded on the air rather than
follow later operating defaults or provider metadata. Keep copied station
context and the full tag set within the same contact value.

Whole-record comparison deliberately preserves the simple MVP conflict model;
disjoint tag and fact edits do not receive a special automatic merge.
```

## Task 5: Implement copied defaults, equipment relationships and tag policy

**Files:** Create `src/{profile,tag}.rs`, `tests/defaults_and_tags.rs`; modify equipment definitions, test support and exports.

**Interfaces:**

- `SavedLocationProfile`, `EquipmentConfiguration` and `LogbookTag` have the spec's fields. `CurrentOperatingValues { station: StationSnapshot, operator: Option<OperatorInformation>, transmit_power: Option<Milliwatts>, equipment: Option<EquipmentValues> }` describes configured values for a pure copy operation, not incomplete entry or a persistent draft.
- `load_profile(current: &CurrentOperatingValues, profile: &SavedLocationProfile, equipment: Option<&EquipmentConfiguration>) -> Result<CurrentOperatingValues, RecordError>`; `load_equipment(current: &CurrentOperatingValues, equipment: &EquipmentConfiguration) -> CurrentOperatingValues` replaces only equipment with copied values. A referenced configuration must match the profile's requested ID; an extra supplied configuration when the profile has no default is ignored, preserving current equipment; missing/deleted/mismatched input returns a reference error, not silent preservation.
- `LoadedProfileBaseline { profile: SavedLocationProfile, operating_values: CurrentOperatingValues }` retains copied values from the last explicit load. `compare_loaded_profile(current: &CurrentOperatingValues, latest: &SavedLocationProfile, baseline: &LoadedProfileBaseline) -> ProfileDifference`, where `ProfileDifference { local_station_or_equipment_changed: bool, saved_profile_changed: bool }`, compares current station/equipment against copied defaults and latest profile against the saved baseline; operator/power are excluded. The client storage/synchronization layer later associates its accepted revision with this baseline; the model invents no revision scheme.
- `AutomaticUploadPolicy::{Enable, Forbid, Neither}`, `automatic_upload_allowed(impl IntoIterator<Item = AutomaticUploadPolicy>) -> bool`.
- `TagDeletionChoice::{LeaveAll, DeleteAll, DeleteUnique}` and `plan_tag_deletion(tag: TagId, choice: TagDeletionChoice, qsos: &[Qso]) -> TagDeletionPlan`, where `TagDeletionPlan { affected: BTreeSet<QsoId>, mark_deleted: BTreeSet<QsoId>, remove_membership: BTreeSet<QsoId>, also_tagged_elsewhere: usize }`. The result is a pure classification, not durable deletion or concurrent acceptance.

- Add `RecordError::{MissingEquipment { id: EquipmentConfigurationId }, EquipmentIdMismatch { expected: EquipmentConfigurationId, supplied: EquipmentConfigurationId }}` for load failures.

- [x] **Step 1:** Add `profile_load_clears_old_station_and_preserves_nonprofile_values`, `equipment_copies_survive_configuration_edit`, `upload_policy_truth_table`, `tag_deletion_counts_identities` and `duplicate_tag_names_preserve_ids`. Assert station county/references missing from the loaded profile are cleared; operator/power unchanged; no equipment default preserves current equipment; a selected default replaces copied values and missing/mismatched default errors.

```rust
// upload_policy_truth_table
use AutomaticUploadPolicy::{Enable, Forbid, Neither};
assert!(!automatic_upload_allowed([]));
assert!(!automatic_upload_allowed([Neither, Neither]));
assert!(automatic_upload_allowed([Neither, Enable]));
assert!(!automatic_upload_allowed([Enable, Forbid]));
```

`remove_membership` contains surviving affected contacts only; `mark_deleted` contains contact identities for deletion, never merely untagging. Use 100 contacts, 30 also in another tag: LeaveAll marks 0 deleted; DeleteAll marks 100; DeleteUnique marks 70 and preserves/untags the shared 30. Membership removal must not mutate unrelated facts or infer external deletion. In `loaded_baseline_distinguishes_local_and_saved_changes`, test neither changed, local station-only, saved profile-only and both changed; operator/power changes do not set either flag. A newer saved profile is reported without auto-loading it.

- [x] **Step 2:** Run `cargo +1.99.1 test -p clacks-model --test defaults_and_tags`; expect missing copy/classification operations.
- [x] **Step 3:** Implement the interfaces. Require callsign in saved profiles through StationSnapshot's value type. Keep configuration display names/IDs out of QSO EquipmentValues. Do not store synchronization baselines in QSO facts, auto-reload current defaults, implement a session lifecycle or perform tag deletion against storage. Pure results hand off to the accepted reference/conflict/transaction rules in future storage/sync tasks.
- [x] **Step 4:** Run this test and contacts; all assertions pass. Confirm subsequent edits to returned/source values cannot mutate historical copies by aliasing.
- [x] **Step 5:** Commit with this subject and rationale, adjusted for the actual delivered change:

```text
feat(model): implement profile snapshots and tag policy semantics

Loading a profile must replace the previous station setup without leaking
absent values, while a missing equipment default intentionally preserves the
current equipment. Retain a load baseline so local adjustments can be
distinguished from later changes to the saved profile.

Tag removal and contact deletion have different meanings. Keep their effects
explicit and let Forbid override automatic-upload grants.
```

## Task 6: Add supplied group definitions and separate catalogs

**Files:** Create `src/{group,catalog}.rs`, `src/export.rs`, `src/export/rules.rs` (serialized rule types only), `tests/groups_and_catalogs.rs`, `tests/fixtures/definitions.json`; modify exports and `tests/support/mod.rs`.

**Interfaces:**

- `GroupDefinition`/`CatalogBinding`/`MetadataDisplayBinding`/`ReferenceCatalog`/`SourceInformation` match spec section 9. `ReferenceCatalog` has **no** version field. `SourceInformation.obtained_at` uses UtcTimestamp; URL (Uniform Resource Locator) provenance is optional text validated by the acquiring adapter, not a new network dependency here.
- `MetadataValue` uses ordinary `serde_json::Value` for non-authoritative catalog attributes; reject nonfinite numeric inputs through normal JSON decoding. Compiled state and catalog metadata do not enter Qso equality.
- `DefinitionSet::new(Vec<GroupDefinition>) -> Result<Self, DefinitionError>`, `get(GroupId) -> Option<&GroupDefinition>`; duplicate IDs with competing definitions error, names may repeat.
- `normalize_reference(association: &ReferenceAssociation, definition: Option<&GroupDefinition>) -> Result<ReferenceAssociation, DefinitionError>` applies the explicit entry/edit operation; a missing definition returns the supplied association unchanged. `interpret_reference<'a>(association: &ReferenceAssociation, definitions: &'a DefinitionSet) -> Option<&'a GroupDefinition>` returns only the definition with matching ID; it never normalizes recorded values.
- `lookup_reference(association: &ReferenceAssociation, definitions: &DefinitionSet, catalogs: &CatalogSet) -> CatalogFinding` distinguishes `NotConfigured`, `Unavailable`, `NotListed` and `Found`. These are lookup findings, not new persistent validity states or automatic rejection policies. `CatalogSet = BTreeMap<CatalogId, ReferenceCatalog>`. `CatalogFinding::{NotConfigured, Unavailable { catalog_id: CatalogId }, NoLookupKey, InvalidBinding { reason: String }, NotListed { key: String }, Found { key: String, entry: CatalogEntry }}` contains copied display data. `MetadataField` is a nonblank string key, `MetadataDisplayBinding` follows the spec fields, and `CatalogEntry { metadata: BTreeMap<String, serde_json::Value> }`.

- `TextNormalization::{Trim, Uppercase, Lowercase}` applies in listed order. `IdentifierDefinition { required: bool, normalization: Vec<TextNormalization>, syntax: Option<String> }` fills `GroupDefinition.identifier`. Add `DefinitionError::{DuplicateGroupId { id: GroupId }, MissingIdentifier { group_id: GroupId }, InvalidIdentifier { group_id: GroupId, value: String }, InvalidDefinition { group_id: GroupId, path: String, reason: String }}`.
- Introduce the complete serialized mapping/expression/profile types listed in Task 8 now, because GroupDefinition and CatalogBinding contain them. Task 8 adds compilation, not a second representation. Types may refer to each other across Rust modules; none depend on compiled state. `ProfileSet = BTreeMap<OutputProfileId, OutputProfile>` is also declared here.

- [x] **Step 1:** Add tests for required versus group-only identifiers, explicit trim/uppercase and full-match syntax, unknown associations preserved exactly, and metadata refresh not changing serialized Qso values. A pattern accepting `US-1234` must reject `junkUS-1234junk`; duplicate names with distinct group IDs remain distinct.

```rust
// a_catalog_refresh_is_not_a_qso_edit
let (contact, association, definitions, mut catalogs, catalog_id) = support::catalog_case();
let before = serde_json::to_value(&contact)?;
catalogs.get_mut(&catalog_id).unwrap().entries.get_mut("US-1234").unwrap()
    .metadata.insert("name".into(), serde_json::json!("Revised park name"));
let finding = lookup_reference(&association, &definitions, &catalogs);
assert!(matches!(finding, CatalogFinding::Found { .. }));
assert_eq!(serde_json::to_value(&contact)?, before);
```

Add the deterministic `support::catalog_case() -> (Qso, ReferenceAssociation, DefinitionSet, CatalogSet, CatalogId)` fixture containing a US-1234 association and catalog entry. Test source provenance round trips without a separate version, name/status/location display metadata and missing catalog availability without rewriting a reference or importing a park center into a QSO.

- [x] **Step 2:** Run `cargo +1.99.1 test -p clacks-model --test groups_and_catalogs`; expect missing definition/catalog behavior.
- [x] **Step 3:** Implement the interfaces. Keep optional catalog lookup separate from normalization/syntax. The fixture contains a POTA-like required-ID/location-bearing group and a participation-only CQ World Wide Single Sideband (CQ-WW-SSB) contest group with fixed IDs; it is illustrative supplied data, not a privileged built-in registry. Definitions arrive as records; no program-name switch, group editor, seeding side effect or network fetch. Account creation will persist defaults later. Compile regexes once per compiled definition in Task 8; this task uses a private regex-validation helper; Task 8 promotes it into the shared compiler. Catalog lookup accepts only expressions evaluable from the current association (literal, current identifier and capture; joins require a collection and QSO paths/expansion bindings are unavailable in this context). Invalid contexts return `CatalogFinding::InvalidBinding { reason: String }`, not NotListed. The catalog lookup initially uses that helper; the compiler/evaluator reuse this behavior rather than introducing a second expression dialect.
- [x] **Step 4:** Run group/catalog and contacts tests. Confirm a second equivalent group with another ID/name has identical generic capabilities and that merely interpreting an existing identifier does not run entry normalization.
- [x] **Step 5:** Commit with this subject and rationale, adjusted for the actual delivered change:

```text
feat(model): define data-driven groups and separate reference catalogs

Default programs and future user-defined groups need the same capabilities;
hardcoded program branches would prevent that parity. Store interpretation
rules as supplied account data and preserve unknown associations unchanged.

Catalog descriptions are lookup information, not recorded station facts.
Refreshing them must not move a contact or create a contact edit.
```

## Task 7: Implement pure source selection without provider record invention

**Files:** Create `src/effective.rs`, `tests/effective_values.rs`; modify group/location exports and test support.

**Interfaces:**

- `InformationSource::{Recorded, Lotw, Qrz, Directory}`; `Sourced<T> { value: T, source: InformationSource }`.
- `EligibleStationFacts<'a> { lotw: Option<&'a StationFacts>, qrz: Option<&'a StationFacts>, directory: Option<&'a StationFacts> }`, where `StationFacts { name: Option<String>, location: Option<RecordedLocation> }` carries only read-side facts. This is a read projection of already eligible provider information, not the service-evidence storage schema.
- `select_location(recorded: &StationSnapshot, eligible: &EligibleStationFacts<'_>, definitions: &DefinitionSet) -> Option<Sourced<Option<RecordedLocation>>>`: a recorded location-bearing reference can select recorded provenance with no geographic details. Do not collapse that to “no selected source, try directory.”
- `select_name(recorded: Option<&str>, eligible: &EligibleStationFacts<'_>) -> Option<Sourced<String>>`.
- `directory_key_matches(current: &str, snapshot_key: &str) -> bool` compares already-normalized lookup keys; full portable-call derivation/request correlation are downstream directory work.

- [x] **Step 1:** Add `whole_location_never_patches_lower_source`, `location_reference_blocks_home_location`, `catalog_point_is_not_station_point` and `old_directory_key_cannot_supply_fallback`. A recorded state-only location blocks directory county/coordinates; a POTA-like association without a position selects recorded/unknown geographic details; with no recorded information, LoTW precedes QRZ then directory. Unknown group properties are not guessed.

```rust
// location_reference_blocks_home_location
let chosen = select_location(&station_with_location_reference, &eligible, &definitions).unwrap();
assert_eq!(chosen.source, InformationSource::Recorded);
assert_eq!(chosen.value, None);
```

Also test recorded name over provider names and empty aggregates as absence. Test the adapter composition excluding directory facts whose normalized key differs, while same-key suffix changes retain them. The selector never changes the input snapshots.

- [x] **Step 2:** Run `cargo +1.99.1 test -p clacks-model --test effective_values`; expect missing selection behavior.
- [x] **Step 3:** Implement the interfaces and document preconditions: callers supply current-key/eligible LoTW/QRZ/directory observations; issue #3/provider adapters establish eligibility. No direct provider query, snapshot arbitration record, lookup ID, causal metadata or submission history is invented here. Latest-successful-directory arbitration and canceled/late request processing are downstream integration obligations, not silently approximated in this selector.
- [x] **Step 4:** Run effective-values and groups/catalog tests. Verify all results carry truthful provenance and no selector mutates recorded information.
- [x] **Step 5:** Commit with this subject and rationale, adjusted for the actual delivered change:

```text
feat(model): select effective station values without overwriting facts

Provider locations can describe a home station rather than the operating
position. Select a whole eligible source instead of filling individual gaps
from lower-priority sources, including when a location-bearing reference
leaves geographic details unknown.

Keep provider eligibility and request arbitration outside this pure model
operation so it cannot invent service-observation guarantees.
```

## Task 8: Compile bounded mapping rules

**Files:** Modify `src/export.rs`, `src/export/rules.rs`; create `tests/export_rules.rs`; expand `src/group.rs`, definition fixtures and exports.

**Interfaces:** The following serialized types are declared in Task 6; this task consumes them and produces compiled validation state.

- `ExportContext { output_profile_id: OutputProfileId, target_group_id: Option<GroupId> }`.
- `GroupExportMappings { ordinary_interchange: Vec<FieldBinding>, targeted: BTreeMap<OutputProfileId, GroupExportPlan> }`; `GroupExportPlan { fields: Vec<FieldBinding>, file_partition: Vec<PartitionRule>, entry_expansion: Vec<ExpansionRule> }`.
- `ValueExpression::{Literal(String), QsoField(QsoFieldPath), CurrentIdentifier, Identifiers(StationSide), BoundIdentifier(String), Capture { value: Box<ValueExpression>, pattern: String, name: String }, Join { values: Box<ValueExpression>, separator: String }}`. Iteration/identifier lookup is scoped to the current group ID. Captures are data expressions, not additional saved reference fields.
- `QsoFieldPath::{OwnCallsign, ContactedCallsign, OwnPrimarySubdivision, ContactedPrimarySubdivision}` initially exposes the native paths needed by reference mappings. Common ADIF date/mode formatting is supplied by the later format adapter, not ad-hoc dynamic reflection.
- `FieldBinding { field: String, value: ValueExpression, when: BindingCondition }`; conditions are `Always`, `BoundPresent(String)` or `AssociationPresent(StationSide)`.
- `PartitionRule::{ByValue(ValueExpression), EachReference { side: StationSide, binding: String }}`; `ExpansionRule { side: StationSide, binding: String, keep_one_if_empty: bool }` declares whether to retain one entry if empty; POTA-compatible data sets it true. A required file-partition reference that is absent reports output readiness, not silent filtering.
- `compile_definition(&GroupDefinition) -> Result<CompiledGroupDefinition, DefinitionError>` validates regexes, full-match semantics, declared paths and binding/capture names. `CompiledGroupDefinition` retains the original GroupDefinition plus reusable compiled patterns keyed by definition path; it does not hold borrowed input lifetimes. `CompiledGroupDefinition` is nonserialized derived state. `OutputProfile { id: OutputProfileId, name: String, purpose: OutputPurpose, contextual_fields: BTreeSet<String> }` with `OutputPurpose::{OrdinaryInterchange, Targeted}` supplies the reference-planning interface; ordinary ADIF data lists `CONTEST_ID` as a contextual field.

- Expression evaluation uses `ExpressionValue::{Absent, Scalar(String), Collection(Vec<String>)}`. Literals/fields/current or bound identifiers yield scalar/absence; Identifiers yields an ordered collection for the current group/side; Capture maps scalar or collection values without altering source identifiers; Join converts a collection to a scalar and omits an empty collection. FieldBinding must yield scalar/absence; an unjoined collection is an InvalidDefinition diagnostic. A capture name absent from the regex is a definition error; a declared capture absent from an optional match yields absence. File partition/entry bindings are scoped and bound before evaluating fields.

- [x] **Step 1:** Add tests `definition_errors_are_not_record_errors`, `full_match_and_named_capture`, `bindings_are_group_scoped` and `compiled_state_is_not_serialized`. Malformed regex, missing capture `park` and undeclared bound variable return explicit definition diagnostics. An unsupported QSO path in supplied JSON fails typed decoding. For pattern `(?P<park>[A-Z0-9]{1,4}-[0-9]{4,5})(?:@(?P<location>[A-Z]{2}-[A-Z0-9]{1,3}))?`, assert capture `park` of `US-1234@US-WY` is `US-1234`; the association itself remains unchanged. This is a representative extraction test, not a claim of a complete POTA submission transform.
- [x] **Step 2:** Run `cargo +1.99.1 test -p clacks-model --test export_rules`; expect missing rule compilation.
- [x] **Step 3:** Use the typed Serde enums introduced in Task 6 and compile regexes into reusable state; wrap patterns with full-match boundaries. Interpret no arbitrary Rust, JavaScript, executable path or network source. Resolve duplicate field assignments only through explicit context/profile logic in Task 9; conflicting assignments inside one selected definition error before producing an output plan. Catch statically demonstrable conflicts at compilation and data-dependent conflicts during selection/evaluation. Replace Task 6's temporary regex compilation with this shared compile path.
- [x] **Step 4:** Run export-rules and groups/catalog tests; expect valid supplied definitions to compile and errors to identify the definition and failing binding without changing a QSO.
- [x] **Step 5:** Commit with this subject and rationale, adjusted for the actual delivered change:

```text
feat(model): compile declarative reference mapping definitions

Reference groups must express extraction and output behavior without scripts
or privileged application knowledge of program names. Validate the bounded
expression language before producing output so malformed definitions remain
configuration errors rather than invalid contact records.

Keep compiled regex state derived and outside serialized account data.
```

## Task 9: Implement ordinary and targeted mapping selection

**Files:** Create `src/export/select.rs`, `tests/export_context.rs`; modify export types and fixtures.

**Interfaces:**

- `select_mappings(qso: &Qso, context: &ExportContext, definitions: &DefinitionSet, profiles: &ProfileSet, resolution: Option<&ContextResolution>) -> Result<MappingSelection, ExportError>`; `ProfileSet` is keyed by OutputProfileId.
- `MappingSelection { groups: Vec<SelectedGroupMapping>, decisions: Vec<ExportDecision>, warnings: Vec<ExportWarning> }`; `SelectedGroupMapping { group_id: GroupId, plan: GroupExportPlan, sides: BTreeSet<StationSide> }`. `ExportDecision::AmbiguousContextField { qso_id, field, candidates: BTreeMap<GroupId, String> }` records unresolved ambiguity, not a silently chosen value.
- `ContextResolution::{ChooseTarget(GroupId), OmitReported}` is supplied to `select_mappings`; choosing a target reselects with that context and omission removes the ambiguous binding with `ExportWarning::OmittedContextField { qso_id: QsoId, field: String, candidates: BTreeMap<GroupId, String> }`. `ExportError::{UnknownProfile { id: OutputProfileId }, UnknownTarget { id: GroupId }, InvalidResolution { qso_id: QsoId, reason: String }, ConflictingBindings { group_id: GroupId, field: String }, Definition(DefinitionError), NotReady { qso_id: QsoId, reason: String }}` distinguishes configuration/readiness failures. Unknown associated groups are retained and reported as unsupported by a downstream adapter, not rejected as invalid native QSOs.

- [x] **Step 1:** Add a fixture with contest groups A and B contributing different literals to `CONTEST_ID`, plus a POTA-like group with list mappings. In `per_contact_context_is_unambiguous_or_explicit`, assert A-only and B-only contacts each emit their own value with no decision; one contact associated with both produces exactly one explicit ambiguity. Identical candidate values coalesce. In `target_does_not_invent_participation`, selecting A for a B-only QSO must not add A membership or fabricate A participation. In `targeted_mapping_excludes_competing_context`, selecting A on an A+B contact selects A's context field while preserving ordinary supported reference mappings.
- [x] **Step 2:** Run `cargo +1.99.1 test -p clacks-model --test export_context`; expect missing selection/resolution.
- [x] **Step 3:** Implement selection by profile purpose, optional target and actual associations. Context-sensitive field names are declared by profile data, not program names. For an OrdinaryInterchange profile, ordinary mappings run per recorded group; no partitions/expansions are activated. An optional target governs contextual fields without suppressing supported ordinary reference mappings from other groups. A Targeted profile requires a target and that group's plan keyed by OutputProfileId; a missing target/plan returns NotReady rather than assuming a program. Only its declared partition/expansion rules activate. Resolve unique values per QSO; return decisions for distinct competing context candidates. Selected-target mappings control contextual fields; conflicts within that definition are errors, not ambiguity to resolve by discarding it. No generic per-binding omit switch is added. Do not decide import-side CONTEST_ID station ownership here.
- [x] **Step 4:** Re-run the test and export-rules tests. Assert serialized QSO values/tag/reference sets and the selected QsoId set are identical before/after all selection and resolution calls.
- [x] **Step 5:** Commit with this subject and rationale, adjusted for the actual delivered change:

```text
feat(model): select reference mappings from explicit export context

Ordinary interchange can include several program associations on one contact,
but a single contextual output field cannot silently choose among competing
values. Require a selected context or explicitly reported omission.

Choosing an export context must not fabricate participation or silently filter
the caller-selected contact set.
```

## Task 10: Evaluate reference output partitions and expansions

**Files:** Create `src/export/evaluate.rs`, `tests/reference_export.rs`; modify export types and fixtures.

**Interfaces:**

- `plan_reference_export(qsos: &[Qso], context: &ExportContext, definitions: &DefinitionSet, profiles: &ProfileSet, resolutions: &BTreeMap<QsoId, ContextResolution>) -> Result<ReferenceExportPlan, ExportError>`.
- `ReferenceExportPlan { files: Vec<ReferenceExportFile>, decisions: Vec<ExportDecision>, warnings: Vec<ExportWarning>, source_contact_count: usize }`; `ReferenceExportFile { partition_key: Vec<String>, entries: Vec<ReferenceExportEntry> }` groups by profile/target plus declared partition values. `ReferenceExportEntry { qso_id: QsoId, reference_fields: BTreeMap<String, String> }` references the original contact, not a modified QSO or fully encoded ADI text.
- Duplicate identical input identities are represented once; conflicting versions supplied under one QsoId return NotReady rather than selecting one arbitrarily. Unresolved context decisions remain visible and do not publish fields from an arbitrary choice. `ExportWarning::OmittedContextField { qso_id, field, candidates }` accompanies explicit omission. A required partition source absent from a targeted plan returns `ExportError::NotReady { qso_id, reason }`, preserving the contact set for reporting.

- [x] **Step 1:** Add tests `ordinary_export_does_not_expand`, `two_by_three_expands_only_target_group`, `hunter_contact_retains_one_entry`, `omission_is_explicit_and_reported`, `no_program_name_switch`, and `missing_partition_is_not_silent_filtering`. Assert ordinary POTA lists preserve both/three references in one QSO entry, while a targeted own-2/contacted-3 plan yields two partitions of three entries. A fourth unrelated contest/reference group does not multiply output. With no contacted parks, retain one entry per own park. All entries retain the original QsoId/start/callsigns through the source record reference.

Use whitespace-free `US-0001`, `US-0002`, `US-0003` fixture identifiers and a copied equivalent group with a different name/ID to prove generic behavior. The literals `POTA`, fields `MY_SIG`/`SIG` and mappings belong to fixture data, never an interpreter switch.

- [x] **Step 2:** Run `cargo +1.99.1 test -p clacks-model --test reference_export`; expect missing evaluation.
- [x] **Step 3:** Implement expression evaluation, deterministic set iteration, partitions and expansion within the selected group. Stable ordering aids fixtures but carries no new contact ordering semantics. Empty optional fields/absent contacted references omit their corresponding fields without dropping the entry. Missing capture/partition/readiness diagnostics are distinguished from invalid native contacts. Return an in-memory plan only: no filenames, file I/O, external submission, snapshot transaction or destination replacement. Profile-specific full ADIF validation and qualified park/state extraction fixtures are added by the interchange implementation, using these operations.
- [x] **Step 4:** Run reference-export, export-context and export-rules tests; assert input serialization is unchanged and source count differs appropriately from expanded entry/file counts. The fixture demonstrates POTA's known mapping distinction without claiming live pota.app acceptance.
- [x] **Step 5:** Commit with this subject and rationale, adjusted for the actual delivered change:

```text
feat(model): evaluate group-scoped reference export plans

Submission-oriented formats can require several output entries for one native
contact. Keep that expansion in output planning so multi-park submissions do
not create duplicate native contacts or multiply unrelated reference groups.

Return an in-memory plan; file publication and complete format validation
belong to the later interchange capability.
```

## Task 11: Verify the cross-component contract and hand off evidence

**Files:** Create `tests/compatibility.rs`; modify `tests/support/mod.rs`, the data README, the spec/status notes, `docs/handoff.md`, `docs/github-backlog.md` and `README.md` only as needed to report actual results.

**Interfaces:** Consume the public interfaces above without adding new behavior. Test-only `legacy_view_round_trip(&str, &RecognitionCatalog) -> Result<String, Box<dyn std::error::Error>>` uses the old-recognition fixture and the same preserved mode/band identities; it must not simply parse every value using the current recognition catalog. No public schema version or actor protocol is invented.

- [ ] **Step 1:** Add `unrelated_edit_survives_old_recognition_round_trip`, `source_mutation_cannot_rewrite_recorded_snapshots`, `empty_optional_aggregates_are_not_location_evidence`, `whole_record_comparison_includes_disjoint_tag_edits`, and `invalid_deserialized_values_are_rejected_before_save`. Use the stored-values fixture to retain unknown submode/optional parent text and band labels exactly, change only a comment, and compare both native identities and serialized JSON values. Test that equal center coordinates do not erase area/point distinction. Tests exercise real boundaries rather than mirror a struct's field list.
- [ ] **Step 2:** Run `cargo +1.99.1 test -p clacks-model --test compatibility`; expect any uncovered integration gap to fail before fixes. If it passes immediately because preceding tasks already implement the complete contract, record that truthfully instead of introducing a deliberate defect.
- [ ] **Step 3:** Fix only demonstrated contract gaps, then document the implementation/library boundary, source data provenance, actual dependency graph and unimplemented downstream adapters. All comparison fixtures use immutable copied input values. Record the unresolved import CONTEST_ID ownership boundary without implementing a guessed answer. Do not mark any roadmap phase complete or close issue #1 merely because planning documents exist; evaluate its capability acceptance against actual delivered evidence.
- [ ] **Step 4:** Run the full verification set below. Document real results and any unavailable target checks; do not report an unrun Windows check as passing. Review the changed source for item spacing as well as rustfmt output. Inspect `cargo tree` for unexpected native/system dependencies and ensure the crate can be tested offline after dependency acquisition.
- [ ] **Step 5:** Commit with this subject and rationale, adjusted for the actual delivered change:

```text
test(model): verify preservation and cross-component native contracts

Individual type tests cannot establish that an unrelated edit survives an
older recognition view or that later source changes leave copied facts intact.
Exercise those boundaries through the public interfaces before downstream
storage and synchronization rely on them.

Record actual verification evidence and the remaining adapter boundaries;
model delivery does not certify provider interoperability or deployed recovery.
```

## Final verification and scope evidence

Run from the repository root after the implementation tasks:

```bash
cargo +1.99.1 fmt --all -- --check
cargo +1.99.1 test --workspace --all-targets --locked
cargo +1.99.1 clippy --workspace --all-targets --locked -- -D warnings
cargo +1.99.1 test --doc --workspace --locked
cargo +1.99.1 tree --workspace --locked
cargo +1.99.1 test --workspace --all-targets --locked --offline
git diff --check
```

Expected: tests/doc tests pass, formatter/clippy/diff checks exit zero, offline tests make no data/network requests, and the dependency graph matches the four ordinary Rust libraries plus their necessary Rust transitive dependencies. On a host with the target installed, also run `cargo +1.99.1 check --workspace --target x86_64-pc-windows-msvc --locked`. Installing that standard-library target during authorized execution is acceptable. A cross-check is foundation evidence, not a Windows installer/runtime certification; the latter remains a later milestone.

### Spec coverage map

| Spec section | Deliverable/tasks |
| --- | --- |
| 1–3: scope, engineering choices, identity/ownership | Task 1 plus explicit downstream ownership/durability contracts; no application scaffold. |
| 4–5: contacts/scalars/validation | Tasks 1 and 4; imports/entry validate before saving in their adapters. |
| 6: point/area, precision, geography | Task 2; source selection in Task 7; actual datasets/lookup remain their capability. |
| 7: band/frequency/mode and preservation | Tasks 3–4 and old/new compatibility in Task 11. |
| 8: station-side reference associations | Tasks 4 and 6. |
| 9: group/catalog schemas and generic interpretation | Tasks 6 and 8; seed persistence/catalog acquisition remain downstream. |
| 10: export context and reference transforms | Tasks 8–10; full format adapters and file publication remain downstream. |
| 11: profiles/equipment/tags | Task 5; missing/deleted references feed downstream resolution rather than silent repair. |
| 12: sources, equality, component boundaries | Tasks 4, 7 and 11; provider eligibility/arbitration and causal synchronization remain their capabilities. |
| 13: acceptance cases | Named owning-task tests above and final Task 11 verification. |
| 14: review/execution boundary | This plan remains reviewable and unexecuted until the user selects execution. |

### Self-review and execution handoff

Self-review checked the plan against all specification sections, traced the five Review Focus cases to owning tests, and corrected mapping-type dependencies, context-resolution inputs and profile-baseline coverage. The coverage map identifies integration behavior deliberately owned by later capabilities; this plan does not claim to deliver those adapters. No execution method has been chosen. After user review, use the selected execution workflow in an isolated worktree; do not create one or start code while merely preparing this plan.

Native execution is recommended because the tasks share a tightly coupled value/schema contract and one library; a single implementer can carry that contract through the chain with a final independent review. Subagent-driven execution remains available if the user prefers a fresh implementer/reviewer per task. In either case, read the spec and this plan together and do not infer additional product requirements during implementation.

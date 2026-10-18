# Directory lookup and geographic reference data Milestone Outline

> **Status: milestone outline, not an executable implementation plan.** This document maps scope, dependencies and acceptance goals. Proposed files, interfaces and verification targets are provisional. Before coding, prepare a concrete plan for the selected milestone covering its data model, behavior, algorithms, edge cases and tests. Do not infer those choices while executing this outline.

A radio contact is a **QSO**; minimum viable product is **MVP**. DX Century Club is **DXCC**; International Telecommunication Union is **ITU**; CQ zones are amateur-radio operating zones. LoTW means Logbook of The World; Hypertext Transfer Protocol is **HTTP**; user interface is **UI**. Point versus grid area describes location meaning, not just the display of center coordinates.

**Goal:** Deliver direct-client QRZ/HamQTH lookup, synchronized source-separated snapshots and bundled offline geographic autofill.

**Architecture:** Directory requests use only the account-selected provider and protected client credentials. Geographic lookup queries a versioned local bundle; explicit coordinates mean a point and a grid-derived center retains its grid-area meaning.

**Tech Stack:** Rust directory adapters, local geographic geometry/indexes, reproducible reference-data build tooling.

**Spec:** [callsign-directory.md](../../../references/callsign-directory.md), [location-lookup.md](../../../references/location-lookup.md), [geographic-data-audit.json](../../../references/geographic-data-audit.json), [product.md](../../../docs/product.md). Read these with the [coordinating plan](2026-10-17-clacks-log.md).

## Global Constraints

This is a planned file/interface layout, not existing code or authorization to execute. All tasks also inherit the coordinating plan’s constraints and file/dependency contracts. Native records and protocol encodings are pinned by F1/S1 before downstream code; do not invent competing representations in an isolated task. No implementation checks below have run yet.

- No provider merging, automatic fallback or persistent reusable callsign-result cache in MVP.
- U.S. state/county coverage is required; worldwide DXCC/CQ/ITU coverage is required at the accepted lower-fidelity boundary.
- Reference data is independently versioned and bundled with releases; a separate updater is post-MVP.

## Review Focus

- A call changes while a lookup is pending: G3 never shows or uses an old-key result.
- A provider refresh races another refresh: G1 selects latest successful snapshot without user conflict.
- A grid straddles boundaries: G2 fills only unambiguous values and exposes alternatives.
- A point lies at a date line/invalid polygon: G2 tests documented geometry fixes, not guessed precision.
- Cancellation or provider failure: G3 preserves existing snapshots and completed bulk results.

## File structure

The Files entries below use `lib/<crate-name>` for libraries and `bin/<application-name>` for thin executable wrappers. Behavior, configuration/runtime composition and tests belong to libraries; binaries only delegate to the library entry point. The Files entries map the new implementation units and their tests. Brace groups expand to individual files in that directory. Crate manifests/module exports and required schema migrations belong to the task that first needs them; keep related source/tests together. No production file exists at these proposed paths yet.

---

### G1: Lookup keys, snapshots and source eligibility

**Depends on:** F1.

**Files:**

- Create/modify: `lib/clacks-directory/src/{lib,key,snapshot}.rs`
- Create/modify: `lib/clacks-domain/src/effective.rs`
- Create/modify: `lib/clacks-directory/tests/snapshot_rules.rs`
- Create/modify: `docs/engineering/directory-contract.md`

**Interfaces:**

- Consumes: F1 recorded facts and source-aware effective_contact contract.
- Produces: normalize_lookup_key(call: &str) -> Result<LookupKey, LookupKeyError>; choose_snapshot(a: &DirectorySnapshot, b: &DirectorySnapshot) -> DirectorySnapshot; DirectorySnapshot { qso_id, lookup_key, provider, successful_at, tie_break_id, fields }; snapshot_eligible(snapshot: &DirectorySnapshot, qso: &QsoRecord) -> bool. EffectiveSources selection uses recorded → eligible LoTW → eligible QRZ → eligible directory.

**Acceptance scenarios to specify:**

snapshot_rules::portable_call_retains_recorded_call; old_key_ineligible; equal_time_deterministic; recognition_no_fact_write; whole_location_no_mixing. Document normalization for supported portable/compound forms rather than stripping every slash expression.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert_eq!(normalize_lookup_key("OK2CQR/P").unwrap().as_str(), "OK2CQR");
assert!(!old_key_snapshot_is_eligible);
assert_eq!(choose_snapshot(&a, &b), choose_snapshot(&b, &a));
```

**Required outcomes:**

Keep provider information, including names, separate from recorded contact facts. Retain snapshots when the normalized key is unchanged and stop using them immediately on a key change. Latest successful client lookup timestamp plus stable tie-breaker wins; retries/propagation do not refresh it. Failed/not-found results do not replace snapshots. Apply whole-location precedence even to partial recorded locations. Define eligibility against current QSO identity for confirmation-provider evidence without relabeling our uploaded values as counterpart information.

**Candidate verification targets:**

```text
cargo test -p clacks-directory --test snapshot_rules
```

**Expected:** all named tests/checks pass; Source eligibility, deterministic arbitration and fallback preserve recorded facts and station identity.


### G2: Build and validate offline geographic bundle

**Depends on:** F1.

**Files:**

- Create/modify: `lib/clacks-geography-build/Cargo.toml`
- Create/modify: `bin/clacks-log-geography/{Cargo.toml,src/main.rs}`
- Create/modify: `data/geography/sources.json`
- Create/modify: `lib/clacks-geography-build/src/{lib,cli,build,validate}.rs`
- Create/modify: `lib/clacks-geography-build/tests/build_validation.rs`
- Create/modify: `lib/clacks-geography/src/{lib,bundle,query}.rs`
- Create/modify: `lib/clacks-geography/tests/coverage.rs`
- Create/modify: `data/geography/{README.md,dxcc-mapping.json}`
- Create/modify: `docs/verification/geographic-bundle.md`

**Interfaces:**

- Consumes: F1 Location basis; selected Census, HA8TKS and Natural Earth sources and candidate audit. Resolve exact source versions and checksums at build execution.
- Produces: clacks_geography_build::main_entry() -> std::process::ExitCode; GeoBundle::open(path: &Path) -> Result<GeoBundle, GeoError>; GeoBundle::lookup(location: &Location) -> Result<GeoCandidates, GeoError>; GeoCandidates reports independent unambiguous values and ambiguous alternatives with dataset provenance.

**Acceptance scenarios to specify:**

build_validation::argument_errors_and_manifest_validation_without_process_exit; coverage::us_county_state; cq_itu_worldwide; dxcc_mapping_and_exceptions; grid_crossing_boundary; explicit_point; date_line; six_itu_repairs; corrupt_bundle_rejected. Select representative real reference locations from pinned datasets and record expected provenance.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert!(cross_boundary_county.requires_selection());
assert_eq!(unambiguous_state, expected_state);
assert_eq!(grid_query.basis, LocationBasis::GridArea);
assert!(corrupt_bundle_open.is_err());
```

**Required outcomes:**

Use `clacks-geography-build` as the builder library package and `clacks-log-geography` as the thin executable package. Argument parsing, configuration, build/validation and substantive error handling live in the library; the binary delegates to its entry point. Build a reproducible independently versioned artifact with exact upstream versions/hashes, redistribution basis and notices. Compile the explicit DXCC mapping/exception layer; review the six ITU polygon repairs, coordinate reference/datum treatment and date-line normalization. Use U.S. states/counties as defined by the selected source, not invented radio county-equivalent semantics. Query the whole grid area for grid-derived positions and the point for explicit coordinates. Populate unambiguous fields, offer manual selection for ambiguous ones and allow corrections until location changes under the existing rules. Validate before bundling; do not claim worldwide fidelity beyond the accepted source limits.

**Candidate verification targets:**

```text
cargo check -p clacks-log-geography; cargo test -p clacks-geography-build; cargo test -p clacks-geography --test coverage; cargo run -p clacks-log-geography -- validate --manifest data/geography/sources.json
```

**Expected:** all named tests/checks pass; The release bundle is reproducible, redistributable and passes recorded U.S./worldwide and ambiguity fixtures.


### G3: Provider adapters, live debounce and bulk lookup

**Depends on:** G1, A3, F2, D1.

**Files:**

- Create/modify: `lib/clacks-directory/src/{provider,qrz,hamqth}.rs`
- Create/modify: `lib/clacks-client/src/lookup/{mod,controller,bulk}.rs`
- Create/modify: `lib/clacks-client/tests/lookup_flow.rs`
- Create/modify: `lib/clacks-directory/tests/provider_responses.rs`

**Interfaces:**

- Consumes: G1 key/snapshot rules; A3 directory credential updates; D1 entry identity; F2 snapshot persistence. D3 later supplies WS call events to this controller.
- Produces: DirectoryProvider::lookup(key: &LookupKey, credentials: &DirectoryCredentials) -> Result<LookupOutcome, LookupError>; LookupController::call_changed(entry: EntryId, value: String, now: Instant) -> Vec<LookupEffect>; bulk_lookup(ids: &[QsoId], cancel: &CancelToken) -> BulkLookupReport. Lookup correlation records entry ID, eventual QSO ID and requested normalized key.

**Acceptance scenarios to specify:**

lookup_flow::five_hundred_ms_debounce; repeated_unchanged_status_no_reset; empty_call_no_request; late_result_original_qso; server_down_provider_up; key_change_discards_old_display; successful_refresh_replaces; failure_not_found_preserve; bulk_partial_cancel; provider_auth_pause. Use a fake clock and local HTTP response fixtures.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert_eq!(requests_before_500_ms, 0);
assert_eq!(requests_at_500_ms, 1);
assert_eq!(late_result.qso_id, completed_original_id);
assert_eq!(failed_refresh.snapshot, previous_snapshot);
```

**Required outcomes:**

Implement both provider adapters, session reuse where appropriate and account-selected-provider-only operation. Keep credentials in protected local storage and allow lookup during sync-server outages. Do not delay contact save for a lookup. Bind late results to the original saved QSO/key and prevent deleted-QSO or next-entry contamination. Bulk selected/filtered IDs deduplicate identities, apply snapshots as successes arrive and report unchanged/not-found/failure/unprocessed outcomes. Keep completed updates on cancellation; pause only affected-provider authentication failures until credential update or explicit retry. No automatic reconnect bulk action.

**Candidate verification targets:**

```text
cargo test -p clacks-directory --test provider_responses; cargo test -p clacks-client --test lookup_flow
```

**Expected:** all named tests/checks pass; Live/manual/WS call changes and bulk refresh preserve source separation under latency, cancellation and outages.

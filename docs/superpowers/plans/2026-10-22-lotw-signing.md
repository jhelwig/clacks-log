# LoTW Certificate and Signing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for native execution, or superpowers:subagent-driven-development if the user selects that method. Implement task-by-task using the checkboxes. Read the specification and this plan before execution.

**Goal:** Deliver the production certificate import, eligibility and signed-batch library required by issue #4, with independently checked compatibility evidence.

**Architecture:** Add `lib/clacks-lotw`, depending on `clacks-model` without changing its frozen submission semantics. Certificate operations return checked credentials and public metadata; a batch serializer consumes prepared submissions and explicit credentials through `std::io::Write`. The later server adapters own persistence, routing, compression, dispatch and upload outcomes.

**Tech stack:** Rust 1.99.1; `p12-keystore` 0.4.1 with `pbes1`; `x509-parser` 0.18.1 without verification features; RustCrypto `rsa` 0.9.10; `sha1` 0.10.7 with `oid`; existing Serde, anyhow, thiserror, time and tracing. Serde provides Rust serialization. PKCS means Public-Key Cryptography Standards; PKCS#12 is the imported archive format. RSA is Rivest–Shamir–Adleman; SHA-1 is Secure Hash Algorithm 1. DER means Distinguished Encoding Rules, the binary certificate/key encoding. TQSL is TrustedQSL; LoTW is Logbook of The World. ADIF means Amateur Data Interchange Format. GAbbI is TQSL's signed interchange format. DXCC means DX Century Club; UTC is Coordinated Universal Time; OIDs are object identifiers.

**Spec:** [LoTW certificate import and signed batches](../../lotw-signing.md). The [source findings](../../../references/lotw-cryptography.md), [source map](../../../references/trustedqsl/source-map.md), [service-evidence contract](../../service-evidence.md) and [observability contract](../../observability.md) accompany it. This plan does not authorize execution or claim compatibility results.

## Global constraints

- Project-owned code, executable maintenance/test tooling and non-trivial fixture construction use Rust. Do not use Python for fixture generation, even externally or through uncommitted scripts. All fixture bytes must be reproducible with the locked Rust tooling.
- Rust **1.99.1**, edition **2024**, workspace resolver **3**; inherit package metadata/dependencies with `workspace = true`; preserve Elastic License **2.0** and applicable upstream notices.
- Production library in `lib/clacks-lotw`; Rust maintenance commands live in `lib/clacks-reference-data` with a thin `bin/clacks-lotw-data` entry point. No product application, database stand-in, alternate signer or empty future adapter scaffold. No `mod.rs` outside tests. No facade/glob re-exports.
- The supplied TQSL sources are read-only reference material: never compile, link, execute or wrap them, including in tests, fixture generation, external tooling or containers. Do not retain outputs obtained from executing them. No C++ harness or TQSL/OpenSSL project build dependency.
- Source pin: TQSL **2.8.6**, commit **`6ce824363e27da455c6cf6fd440b9c401945bfcb`**, configuration **11.34**. Data-driven signature definitions select **LoTW 2.0** and **`SIGN_LOTW_V2.0`**.
- No certificate-chain acceptance, root trust installation, authentication, requests/renewals, private-key export, persistent key management, HTTP, compression implementation or upload scheduling. HTTP is Hypertext Transfer Protocol.
- Keep the model's frozen `LotwSubmission` and service-equality rules unchanged. Formatting conversion is not normalization. No crypto dependency leaks into model/synchronization/browser/desktop consumers.
- Require both archive integrity and actual certificate/private-key matching. Preserve original certificate DER, full serial and issuer. No trust inferred from those checks.
- Signing is digest-aware RSA PKCS#1 version 1.5 with SHA-1 and explicit blinding. No unblinded fallback. Retain a version-specific assessment of `RUSTSEC-2023-0071`, not a blanket ignore.
- Complete the artifact and finish any caller-owned compression before dispatch. Any construction failure makes the partial artifact unusable; never silently skip a contact.
- Keep ordinary error propagation/control flow, checked access, public rustdoc and substantive commit-message bodies. No production `expect()`/`unwrap()` or direct container indexing.
- Follow the observability specification in each operation as implemented: focused annotations, module-path targets, safe domain events and no lifecycle/timing wrappers or new library metric families.

## Review focus

1. A certificate with duplicate required attributes/extensions or a misleading alias must not select an arbitrary station/key; Tasks 1–2 test this through actual import.
2. A restored/replaced key, changed signing identity or inconsistent frozen contact key must not sign another semantic contact; Tasks 2, 3 and 5 test these boundaries.
3. Locale/Unicode case handling and compact timestamps must not change signature input silently; Tasks 4–5 pin supported text and exact bytes.
4. A short write, interrupted write or failure after one valid contact must not be returned as a successful smaller batch; Task 5 checks writer failures and membership.
5. Attacker-chosen derivation parameters and accidentally logged test credentials must not bypass bounded-work checks or leak secrets; Tasks 2 and 6 test actual import/reporting boundaries.

## File and interface map

Paths below are relative to `lib/clacks-lotw/` unless explicitly rooted elsewhere.

| Files | Responsibility |
| --- | --- |
| `Cargo.toml`, `src/lib.rs`, `README.md` | Production crate, module declarations and caller integration guide. |
| `src/error.rs` | Concrete domain errors with thiserror; combined operations use anyhow and preserve sources. |
| `src/certificate.rs` | Public metadata, opaque credential, import entry point. |
| `src/certificate/metadata.rs`, `archive.rs`, `eligibility.rs` | Certificate interpretation, archive checks/key matching, explicit eligibility. |
| `src/signing.rs`, `src/signing/definition.rs`, `input.rs`, `wire.rs` | Public batch operation/types, definition loading, private signing input and record writing. |
| `data/signatures-11.34.json`, `data/README.md` | Derived ordered definitions with provenance and extraction instructions. |
| `tests/certificates.rs`, `archives.rs`, `eligibility.rs`, `batches.rs`, `projection.rs`, `observability.rs` | Production integration tests. Private input/definition helpers also have local unit tests. |
| `tests/support/mod.rs`, `telemetry.rs`, `tests/fixtures/` | Test-only constructors, observation collection and committed compatibility fixtures. |
| `tests/fixtures/README.md`, `tests/support/` | Independent fixture construction, source-derived expected values and provenance; no execution of the supplied sources. |
| `lib/clacks-reference-data` and thin `bin/clacks-lotw-data` | Deterministic extraction/check of signature definitions from the pinned XML. |

Use further private subdivision only when actual code size warrants it. Do not add
public input/signature/writer helpers just for tests. Existing unrelated Rust files
need no restructuring.

Shared public contract to implement in the owning modules:

```rust
// certificate.rs; public metadata contains no private-key library types.
pub struct CertificateMetadata {
    pub identity: PublicSigningIdentity,
    pub issuer_der: Vec<u8>,
    pub serial: Vec<u8>,
    pub not_before: time::OffsetDateTime,
    pub not_after: time::OffsetDateTime,
    pub qso_not_before: time::Date,
    pub qso_not_after: time::Date,
}

pub struct SigningCredential { /* private metadata, DER certificate and signing key */ }
pub fn inspect_certificate(der: &[u8]) -> anyhow::Result<CertificateMetadata>;
pub fn import_pkcs12(archive: &[u8], password: &str) -> anyhow::Result<SigningCredential>;
// SigningCredential exposes metadata() -> &CertificateMetadata and
// certificate_der() -> &[u8]; no Clone, Serialize, raw-key getter or secret Debug.

// certificate/eligibility.rs
pub enum CredentialState { Available, Superseded, Removed }
pub enum EligibilityIssue {
    CallsignMismatch, EntityMismatch, BeforeCoverage, AfterCoverage,
    Expired, Superseded, KeyUnavailable,
}
pub fn eligibility(
    certificate: &CertificateMetadata,
    identity: &PublicSigningIdentity,
    qso_date: time::Date,
    state: CredentialState,
    now: time::OffsetDateTime,
) -> Vec<EligibilityIssue>;

// signing/definition.rs; fields/selected definition remain private.
pub struct SignatureDefinitions { /* validated rows and mapping provenance */ }
impl SignatureDefinitions {
    pub fn from_json(json: &str) -> anyhow::Result<Self>;
    pub fn bundled() -> anyhow::Result<Self>;
}

// signing.rs
pub struct BatchItem<'a> {
    pub submission: &'a LotwSubmission,
    pub credential: &'a SigningCredential,
    pub state: CredentialState,
}
pub struct ProducerInformation<'a> {
    pub application: &'a str,
    pub version: &'a str,
}
pub struct BatchSummary {
    pub certificates: usize,
    pub stations: usize,
    pub contacts: usize,
    pub bytes_written: u64,
}
pub fn write_batch<W: std::io::Write>(
    writer: &mut W,
    items: &[BatchItem<'_>],
    definitions: &SignatureDefinitions,
    producer: &ProducerInformation<'_>,
    now: time::OffsetDateTime,
) -> anyhow::Result<BatchSummary>;
```

Imports in this interface block refer to the existing model types at
`clacks_model::service::lotw::{PublicSigningIdentity, LotwSubmission}`. Derive
ordinary equality/debug on payload-free eligibility enums and public metadata as
appropriate; do not apply those derives to secret-bearing credentials. Eligibility
returns all applicable reasons in enum declaration order, with no duplicates.
`inspect_certificate` serves persisted public metadata/display and does not assert
key possession; `import_pkcs12` establishes possession and matching. Private-key
persistence APIs belong to #27 rather than exposing a test-driven export now.

## Task 1: Certificate interpretation with independently constructed credentials

**Files:** Root `Cargo.toml`, `Cargo.lock`; crate manifest/root, `error.rs`,
`certificate.rs`, `certificate/metadata.rs`, `tests/certificates.rs`,
`tests/support/mod.rs`, `tests/fixtures/`, crate `README.md`.

**Consumes:** Pinned source OIDs, public model signing identity, selected parsers.
**Produces:** `CertificateMetadata`, `inspect_certificate` and the reference
certificate/key inputs used by later tasks. The crate is introduced with real metadata
behavior, not a standalone scaffolding commit.

- [x] **1. Construct controlled certificate/key inputs independently.** Use synthetic 1024/2048/4096-bit keys and certificates containing the documented `AROcallsign`, raw coverage/entity extensions and a full-width serial. Record fixed dates and explicit expected metadata. No user credentials, network submission or execution of the supplied sources.
- [x] **2. Record fixture provenance.** Keep independent certificate/key inputs and metadata-negative cases in `tests/fixtures/`, with tool versions and an exact field/construction description in its README. Provide the dedicated `clacks-lotw-fixtures` Rust command to reconstruct all fixtures from fixed test-only inputs, using `--check` for exact comparison and `--write` for regeneration. Keep recipes under the optional fixture feature in `clacks-reference-data`; never call the production importer, signer or serializer to generate expectations. Source inspection supplies protocol facts, not executable fixture code. Do not add or retain a C++ harness, reference-build container, source-executing generator or its outputs.
- [x] **3. Write failing metadata tests** with these exact assertions:

```rust
let metadata = inspect_certificate(&fixture("station-1024.der"))?;
assert_eq!(metadata.identity.callsign.as_str(), "N0CALL");
assert_eq!(metadata.identity.dxcc, "291");
assert_eq!(metadata.serial, expected.serial);
assert_eq!(metadata.qso_not_before, expected.qso_not_before);
assert_eq!(metadata.qso_not_after, expected.qso_not_after);
assert!(inspect_certificate(&fixture("common-name-only.der")).is_err());
assert!(inspect_certificate(&fixture("duplicate-callsign.der")).is_err());
assert!(inspect_certificate(&fixture("duplicate-coverage.der")).is_err());
assert!(inspect_certificate(&fixture("reversed-coverage.der")).is_err());
```

  Also cover missing/malformed raw extensions, invalid calendar dates, malformed entity text, unsupported public-key algorithm, trailing certificate garbage, and preservation of issuer/serial without integer narrowing. `fixture(name)` and `expected` are test-support loaders for committed bytes and expected JSON, not production APIs.
- [x] **4. Verify red:** `cargo +1.99.1 test -p clacks-lotw --test certificates` initially fails because the crate/API does not exist. Add workspace membership and inherited manifests as part of implementing the behavior, then record an assertion failure before the relevant parsing fix where applicable.
- [x] **5. Implement metadata.** Use `x509-parser` without verification features. Check full input consumption and duplicate OIDs; decode callsign from the subject and extension bytes directly. Parse real calendar dates and decimal entity text without guessing. Preserve exact certificate bytes outside metadata in the eventual credential. No issuer-chain or current-time acceptance here. Define actionable domain errors with thiserror; propagate parser errors through anyhow with stage context rather than forwarding-only variants.
- [x] **6. Verify green:** `cargo +1.99.1 test -p clacks-lotw --test certificates --locked --offline`. Inspect `cargo +1.99.1 tree -p clacks-lotw -e features --locked` and confirm model/sync manifests have no crypto additions. Record any prerequisite cache/network setup separately from offline verification.
- [x] **7. Commit** the production metadata capability, independent inputs/provenance and manifest/lock changes. Synthetic metadata fixtures do not establish live LoTW acceptance.

Dependency additions are workspace-owned. Introduce each when its task uses it:
`p12-keystore =0.4.1` (explicit `pbes1`), `x509-parser =0.18.1` (no verification),
`rsa =0.9.10`, `sha1 =0.10.7` (`oid`), `base64` 0.22, `zeroize` 1 and
`rand` 0.8 (`std`, `std_rng`). Add `der` 0.8 and `pkcs12` 0.2.0-pre.0 only for
typed archive framing/MAC-presence checks, not a second decoder. RSA's older
dependency family and the archive's newer family coexist through bytes. Use
`StdRng::from_rng(OsRng)?` to seed a standard cryptographic generator fallibly
before signing, avoiding an infallible operating-system-randomness call inside
the RSA operation. Do not add a public random-generator injection API solely for tests.

## Task 2: Checked archive import and opaque signing credentials

**Files:** `certificate.rs`, `certificate/archive.rs`, `error.rs`,
`tests/archives.rs`, fixture expectations and README limits/dependency notes.

**Consumes:** Task 1's metadata parser and independently constructed certificate/key inputs. Construct normal/legacy-layout test archives without using the supplied TQSL sources.
**Produces:** `SigningCredential`, its two read-only public accessors and
`import_pkcs12`. The credential privately owns a digest-aware
`rsa::pkcs1v15::SigningKey<sha1::Sha1>` and the original certificate DER.

- [x] **1. Write failing archive tests.** Import both `normal.p12` and `legacy.p12`; assert identical expected public metadata/certificate bytes, independent of aliases and bag order. Inspect fixture framing for modern SHA-256 and legacy SHA-1 integrity and exercise certificate-first as well as key-first safe order. Require errors for wrong password, corrupted bytes, missing MAC, unmatched key, multiple distinct station/key candidates, no station certificate and invalid required metadata. Test that friendly-name/local-key-ID collisions do not bypass modulus/exponent matching. Deduplicate repeated byte-identical certificate/key bags after validation, without hiding distinct candidates. All failures leave an already imported usable credential usable.
- [x] **2. Run red:** `cargo +1.99.1 test -p clacks-lotw --test archives` and confirm missing behavior rather than an environmental fixture failure.
- [x] **3. Implement `import_pkcs12`.** Inspect typed outer framing to require MAC presence; let `Pkcs12Archive` perform integrity checking/decryption. Reject passwords outside Unicode's Basic Multilingual Plane (BMP) explicitly in this unsupported dependency path without rewriting them; do not classify every MAC failure as a definitely wrong password. Use actual RSA keys for matching and reject unmatched/ambiguous candidates. Typed RSA private-key decoding already validates private components; retain the early encoding-size checks without repeating validation on the unchanged decoded key. Prove invalid bounded private components fail through the real importer. Ignore unrelated supporting certificates as credentials. Keep decrypted temporary byte copies in available zeroizing containers; do not derive secret-bearing Debug/Clone/Serialize or retain passwords/whole archives. Acknowledge dependency-owned allocation limits truthfully.
- [x] **4. Pin bounded-work failures.** Add fixtures with declared iteration counts **1,000,001** for MAC, modern password derivation and legacy password derivation; each must fail before expensive work. Exercise the selected dependency's excessive scrypt memory/parallelism rejections with tiny encrypted fixtures, without allocating the requested memory. Characterize zero/negative/overflowed parameters and truncated encodings through the actual importer. Legacy encryption declarations 0 and -1 over deliberately one-round ciphertext are accepted by the locked decoder; test both key and certificate declarations under valid 2048-round integrity, retain a positive-one control and assert wrong-password rejection. Nonpositive MAC declarations remain errors. Keep the library's existing limits, not a custom KDF implementation or server request-budget framework. KDF means key derivation function. Before constructing/validating RSA big integers, inspect key encodings with the existing typed PKCS#8/PKCS#1 parsers and bound modulus/private-component sizes to the supported **4096-bit** range; also apply RSA's public-key validation/bound. Do not rely on `from_pkcs8_der` to perform an early size check: it constructs/validates private components first. Test 1024, 2048 and 4096-bit import plus over-limit public/private encodings. This is the tested implementation range, not a LoTW service guarantee.
- [x] **5. Verify green:** `cargo +1.99.1 test -p clacks-lotw --test archives --locked --offline`. Include empty, ordinary non-ASCII BMP and non-BMP passwords, and duplicate/reordered bag cases. Check concrete causes remain downcastable/source-linked; do not assert dependency-specific prose for every error.
- [x] **6. Commit** with the required-MAC/key-match boundary, actual dependency safeguards, password limits and secret-memory limitations explained in the body.

## Task 3: Explicit certificate eligibility

**Files:** `certificate/eligibility.rs`, `certificate.rs`, `tests/eligibility.rs`.

**Consumes:** Task 1 metadata; prepared `PublicSigningIdentity`; caller state/time.
**Produces:** The `CredentialState`, `EligibilityIssue` and `eligibility` contract
defined above. No credential pool, routing preference or inferred supersession.

- [x] **1. Write failing tests:** eligible at both QSO coverage endpoints; `BeforeCoverage`/`AfterCoverage` immediately outside them. With certificate expiry date **2030-06-15**, `2030-06-15T23:59:59Z` is not expired and `2030-06-16T00:00:00Z` is expired. An evaluation timestamp with a nonzero offset must use its UTC date. Assert callsign/entity mismatches, explicit supersession/removal and deterministic multiple reasons. A future `notBefore` does not introduce the unselected full-time X.509 gate.
- [x] **2. Run red:** `cargo +1.99.1 test -p clacks-lotw --test eligibility`.
- [x] **3. Implement** pure comparisons with supplied time, inclusive QSO dates and UTC-day expiry. Keep imports of expired credentials successful. Give variants standalone documentation; use Debug for payload-free diagnostic reasons rather than manual spellings.
- [x] **4. Verify green:** `cargo +1.99.1 test -p clacks-lotw --test eligibility --locked --offline`. Assert no clock reads, profile defaults, issuer caches or callsign suffix stripping are necessary.
- [x] **5. Commit** with the distinction between certificate expiration and historical QSO coverage.

## Task 4: Data-driven signature definitions and exact private signing input

**Files:** `signing.rs`, `signing/definition.rs`, `signing/input.rs`,
`data/signatures-11.34.json`, `data/README.md`, Rust maintenance library/application; local unit tests in the owning Rust files.

**Consumes:** Existing frozen `LotwSubmission`, selected certificate metadata,
Task 3 eligibility and the pinned XML `sigspecs` section.
**Produces:** Public `SignatureDefinitions::{from_json,bundled}` and a private
`prepare_input(submission, metadata, definitions) -> anyhow::Result<PreparedInput>`.
`PreparedInput` privately contains formatted contact/station fields, selected
signature-field name and exact signing bytes; it is not a new synchronized model.

- [x] **1. Add extractor and failing definition tests.** Preserve source order, required flags, version/deprecation and mapping provenance; `--check` compares the generated file without rewriting it. Assert bundled selection is `SIGN_LOTW_V2.0`; test reordered definition rows, deprecated newer rows, numeric version comparison, duplicate name/version, malformed definitions, unknown selected version, unknown field and incompatible mapping provenance. An unsupported highest nondeprecated version must fail, not quietly select 2.0. Private selection needs no public test helper.
- [x] **2. Add failing input tests** comparing hand-reviewed, source-derived literal signing bytes independent of the production implementation. Assert station-then-contact definition order, no separators, final ASCII uppercasing, absent optional values contributing nothing and required values checked after trimming. Cover every station signing field with reference-compatible per-entity cases, rather than assuming all fields coexist in one location. Read `tqsl_getGABBItCONTACTData`, which uppercases, rather than infer the upload behavior from `tqsl_signQSORecord`. Neither function may be executed.
- [x] **3. Run red:** `cargo +1.99.1 test -p clacks-lotw --lib`. Record which assertions fail before implementing the definitions/input behavior.
- [x] **4. Implement definition validation/selection and `prepare_input`.** `SignatureDefinitions` loads the data with Serde; use numeric version components, not floating-point version ordering. Build formatted date/time from validated compact fields, e.g. **`20261022` → `2026-10-22`**, **`123456` → `12:34:56Z`**. Keep **`14.074000`** unchanged. Station `CALL`/`DXCC` come from frozen identity checked against metadata. Validate consistency with the frozen external key (callsigns/date/time/band/mode/receive interpretation) without remapping that key or normalizing the original QSO. Reject extra fields that cannot be represented rather than omit them.
- [x] **5. Pin the supported text boundary.** Pinned station enumeration values are ASCII. For this implementation, trim ASCII whitespace at value boundaries, require remaining protocol text to be printable ASCII, then uppercase signing bytes. Test non-ASCII text and embedded NUL/newlines/control bytes as explicit errors, not transliteration. Printable delimiter characters in producer values must remain length-framed data and cannot create extra records. Explain this tested support limit in the crate guide rather than claiming that LoTW universally prohibits Unicode. Keep field values and XML order separate from map ordering.
- [x] **6. Verify green:** `cargo +1.99.1 run --bin clacks-lotw-data --locked --offline -- --check` and `cargo +1.99.1 test -p clacks-lotw --lib --locked --offline`. Both derived JSON files must remain byte-identical to the pinned source extraction.
- [x] **7. Commit** with exact input/wire formatting, supported text, selection behavior and provenance contracts explained.

## Task 5: Blinded signing and the standard Write batch serializer

**Files:** `signing.rs`, `signing/wire.rs`, `tests/batches.rs`,
reference expectations and `tests/support/mod.rs` writer fixtures.

**Consumes:** Tasks 2–4, including private prepared input and the credential key.
**Produces:** `BatchItem`, `ProducerInformation`, `BatchSummary`, `write_batch`.
The signature helper remains private and uses the same production path in tests.

- [x] **1. Write failing batch tests:** two contacts sharing credential/location produce **1 certificate, 1 station, 2 contacts**; changed location produces **1, 2, 2**; two credentials produce **2, 2, 2**. Separate imported objects with identical certificate bytes deduplicate by certificate identity, not pointer address. Check explicit ineligible items fail rather than reduce contact count. Assert certificate DER is unchanged and all station references resolve in the emitted batch.
- [x] **2. Add exact signing/wire checks.** Check independently constructed cryptographic known-answer vectors and hand-reviewed exact signing inputs; independent verification must fail when input or signature is changed. Test expected source-derived full-record output with a test-only field parser, checking byte lengths, typed signature field, base64/newlines, `SIGNDATA`, station `CALL`/`DXCC` and truthful producer/configuration header. Match reference layout where defined; do not require Clacks producer text to impersonate TQSL.
- [x] **3. Run red:** `cargo +1.99.1 test -p clacks-lotw --test batches`.
- [x] **4. Implement `write_batch`.** Validate batch inputs/eligibility/producer before output where possible; seed `StdRng` fallibly and use `RandomizedSigner::try_sign_with_rng` with digest-aware `SigningKey<Sha1>`. Assign output-local certificate/station IDs in first-use order, starting at **1**, using checked increments. Deduplicate certificates by exact DER and stations by certificate plus prepared station values. Emit `TQSL_IDENT`, records and exact signature data through ordinary `write_all` calls. Account for serialized byte lengths directly; no counting-writer/timing/completion abstraction. Return summary only after the last successful write; do not flush/finalize the caller's compression wrapper or claim publication.
- [x] **5. Pin interruption behavior.** Test a writer that accepts only a few bytes per write, one returning `Interrupted`, one returning zero and one failing after the first full contact. Assert short writes complete, retryable interrupted writes work through standard facilities, and actual failures preserve the I/O source and produce no `BatchSummary`/batch-success event. Compare the original submissions before/after. A caller-owned staging test discards failed bytes and never invokes its test dispatch callback; do not add a production uploader merely for this assertion. Specify an empty input as an error before writing any header.
- [x] **6. Verify all supported key sizes independently.** Test 1024/2048/4096-bit import and dynamic signature/wire lengths using independently constructed keys and cryptographic vectors. Compare source-derived literal input/record expectations separately from independent cryptographic verification. Do not execute `tqsl_signDataBlock` or any other supplied source function. Do not claim executed TQSL interoperability or live acceptance. Only a private helper's unit tests may supply deterministic randomness.
- [x] **7. Verify green:** `cargo +1.99.1 test -p clacks-lotw --test batches --locked --offline`. Expected protocol values must not come from Clacks's own signing/serialization helpers; independent cryptographic verification must cover more than a same-implementation round trip.
- [x] **8. Commit** with complete-before-dispatch, standard writer failure semantics and exact artifact membership documented.

## Task 6: Production projection integration, telemetry and completion evidence

**Files:** `tests/projection.rs`, `tests/observability.rs`,
`tests/support/telemetry.rs`, production operation annotations/events,
`README.md`, `data/README.md`, `tests/fixtures/README.md`, root `README.md`,
`docs/lotw.md`, `references/lotw-cryptography.md`, `references/licensing.md`.
Change existing model/sync code only to correct a demonstrated contract mismatch,
with a regression and explanation, not to accommodate a second normalization model.

**Consumes:** All production APIs above plus the real model projection.
**Produces:** End-to-end local compatibility proof, safe instrumentation and the
crate's caller/maintenance guide. No new public API is necessary for this task.

- [x] **1. Write a failing production-boundary test.** Construct a native QSO/destination/definitions using the existing model APIs, call `clacks_model::service::lotw::project`, retain its returned `LotwSubmission`, then call `write_batch` with the matching credential. Verify exact wire date/time and mode; serialization must leave the projection and external key unchanged. Cover a band-only primary pair, receive override, station-reference mapping and a mismatch between public signing identity/credential. Serialize/reload the projection and repeat without remapping it. Do not import another crate's test support or expose helpers in production to construct fixtures.
- [x] **2. Write failing telemetry tests** through import, eligibility and batch operations. Collect trace fields using a small test-only tracing layer based on the existing pattern, without extracting a production telemetry framework. Use sentinel passwords, private-key encodings, callsigns and raw contact values; none may appear in emitted selected fields. Check actual certificate/station/contact/output-byte counts after success, safe failure/readiness reasons, no success event after writer failure, normal inherited request context and behavior with operation spans filtered. Compare one representative unconfigured run where it protects a concrete error-path risk; do not duplicate the entire functional suite. Assert no synthetic duration/operation/component fields, no library metrics or upload-success claims.
- [x] **3. Run red:** `cargo +1.99.1 test -p clacks-lotw --test projection --test observability`.
- [x] **4. Add focused instrumentation.** Annotate import as `lotw.certificate.import` and batch writing as `lotw.batch.write`, both `debug`, with `skip_all` and default module targets. Domain events `certificate.import.checked`, `certificate.import.rejected`, `certificate.eligibility.unavailable`, `signing.input.rejected` and `signing.batch.constructed` carry only relevant bounded reasons/counts; use existing typed reasons through Debug. Do not intercept every propagated error for a second log. Successful batch construction reports actual output counts/bytes; import reports checked metadata/key matching, not issuer trust. Native account/job correlation comes from callers. No manual formatting logic in annotations, lifecycle guards, wall-clock metrics or success counters pretending to count durable effects.
- [x] **5. Verify green:** `cargo +1.99.1 test -p clacks-lotw --test projection --test observability --locked --offline`. Check that ordinary Rust test runs neither invoke reference tools nor install subscribers/recorders globally.
- [x] **6. Complete durable integration documentation.** The crate guide describes exact APIs, error/source behavior, tested archive/key/password/text limits, key-memory caveats, the caller's latest-state/routing obligation and complete/finalized artifact before dispatch. Record the actual locked dependency families/features/licenses and signing-path advisory assessment, preserving historical upstream notices. Explain that synthetic fixtures do not prove live LoTW acceptance. Update overview implementation-status text only for delivered behavior; don't add a review-history or backlog mirror. Operational import/upload metrics and server admission/concurrency budgets remain their later adapters' responsibility.
- [x] **7. Run final verification**, recording actual commands/results and any unavailable target/runtime distinctly:

```sh
cargo +1.99.1 run --bin clacks-lotw-data --locked --offline -- --check
cargo +1.99.1 fmt --all --check
cargo +1.99.1 test --workspace --all-targets --locked --offline
cargo +1.99.1 test --workspace --doc --locked --offline
cargo +1.99.1 clippy --workspace --all-targets --locked --offline -- -D warnings
cargo +1.99.1 check --workspace --all-targets --target x86_64-pc-windows-msvc --locked --offline
cargo +1.99.1 check -p clacks-lotw --target x86_64-unknown-linux-gnu --locked --offline
RUSTDOCFLAGS='-D warnings' cargo +1.99.1 doc --workspace --no-deps --document-private-items --locked --offline
cargo +1.99.1 tree -p clacks-lotw -e features --locked
git diff --check
```

  Run the Rust compatibility suite natively on Linux as well as the development host; a cross-check is not runtime evidence. Provision the ordinary Rust toolchain/target/cache when needed. Inspect the dependency graph for native crypto and feature leakage; assess advisories with available tooling without suppressing the known RSA result. Review independently constructed fixtures and literal protocol expectations against the source/documentation without executing the supplied sources. Record the precise verification method and limits in fixture/crate documentation, not a chronological review report.
- [x] **8. Commit** production integration/observability and maintained usage/verification evidence with a substantive body. Request the selected execution workflow's final code review; address confirmed findings and rerun affected checks before integration. Do not close #4 until independent compatibility evidence and production deliverables are present; do not push, create a PR or start #27/#30 unless authorized.

## Execution handoff

Tasks are sequential because they share credentials, normalized input and wire
contracts. Native execution is recommended: implement them in this session with
the prescribed red/green checks and commits, then perform the execution workflow's
independent whole-branch review. Read and approve the plan before implementation;
the user's previous native-execution choices are not a request to start this plan
automatically.

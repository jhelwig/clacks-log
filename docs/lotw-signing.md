# LoTW certificate import and signed batches

This specification consolidates the design for
[issue #4](https://github.com/jhelwig/clacks-log/issues/4). It defines production
components to be implemented in `lib/clacks-lotw`, not a compatibility prototype.
The [LoTW workflow](lotw.md), [service-evidence contract](service-evidence.md) and
[observability contract](observability.md) remain applicable. This specification
does not claim the implementation or interoperability checks already exist.

Logbook of The World is abbreviated LoTW. TrustedQSL (TQSL) is its reference
signing application and library. Amateur Data Interchange Format is ADIF; GAbbI
is TQSL's signed interchange format. Coordinated Universal Time is UTC. DX Century
Club is DXCC. Public-Key Cryptography Standards are PKCS; PKCS#12 is the archive
format normally saved as `.p12`. Rivest–Shamir–Adleman is RSA; Secure Hash Algorithm
1 is SHA-1. Distinguished Encoding Rules are DER, a binary encoding used for
certificates and keys. Privacy-Enhanced Mail (PEM) supplies the text envelope
around base64-encoded DER.

## 1. Outcome and scope

Deliver reusable pure-Rust import, certificate interpretation, eligibility,
signing and batch serialization. Issues #27 and #30 integrate and extend these
components for credential management and publication. Test tools exercise this
production code rather than supply an alternate signer for later replacement.

Included:

- Current and legacy TQSL exports containing one issued station certificate,
  its private key and supporting certificates.
- Archive integrity/decryption, certificate/key matching, public metadata and
  explicit eligibility results.
- Source-derived signature definitions, exact signing input, signatures and
  linked GAbbI certificate/station/contact records.
- Independently checked fixtures, dependency/security/license assessment and
  documented supported cases and limits.

Not included: certificate requests/renewals, private-key export, account
authentication, database/encrypted credential storage, certificate-management UI,
HTTP submission, compression implementation, scheduling or live acceptance.
HTTP means Hypertext Transfer Protocol. These belong to later integration;
certificate removal and explicit supersession remain MVP requirements there.

## 2. Production ownership and dependencies

```text
clacks-lotw -> clacks-model
clacks-sync -> clacks-model
later server library -> clacks-lotw + clacks-sync + clacks-model
```

| Module | Responsibility |
| --- | --- |
| `clacks-lotw::certificate` | Decode archives/keys, interpret public certificate metadata and evaluate signing eligibility. |
| `clacks-lotw::signing` | Load/validate signature definitions, construct signing input, perform cryptography and serialize batches. |
| `clacks-model::service::lotw` | Existing normalization, frozen `LotwSubmission`, remote identity and semantic comparison. |
| Later server credential/provider adapters | Account isolation, encryption, credential routing/state, staging, durable dispatch and service outcomes. |

No dependency from model or synchronization to signing. No certificate/private-key
library types in model, synchronization payloads or browser/desktop consumers.
No process-global credential store, configuration cache, mutable error state or
application runtime in this library. Callers supply credentials, definitions,
evaluation time and prepared submissions explicitly.

Use Rust 1.99.1 and workspace-inherited metadata/dependencies. Public items live
in owning modules, with named module files rather than `mod.rs`. Document their
caller-visible purpose, guarantees and errors. No public APIs solely for tests,
runtime-pluggable backend framework or custom writer framework is needed.

Selected cryptographic composition:

| Dependency | Use and constraint |
| --- | --- |
| `p12-keystore` 0.4.1 | Archive-level `Pkcs12Archive`; enable legacy `pbes1` support without adopting the wrapper's alias/trust policy. |
| `x509-parser` 0.18.1 | Public metadata/raw extensions; no `verify` or `verify-aws` features. |
| RustCrypto `rsa` 0.9.10 | Digest-aware PKCS#1 version 1.5 signing through an explicitly randomized/blinded interface. |
| `sha1` 0.10.7 | SHA-1 with the digest object identifier needed by digest-aware RSA signing. |

Use standard encoded bytes between incompatible dependency type families.
Inspect and lock the actual transitive graph/features, including archive-format
prereleases and the randomness dependency compatible with RSA's signing interface.
Use a cryptographically secure random source; randomness failure must not select
an unblinded fallback. No production OpenSSL dependency or TQSL foreign-function
calls are allowed. Applicable upstream source notices still apply to ported code.

## 3. Trust boundary and archive import

LoTW decides issuer-chain acceptance when receiving uploads. Clacks neither
installs archive roots as trusted nor maintains an ARRL trust store or chain
verifier for this capability. ARRL is the American Radio Relay League. Imported
certificate claims confer no Clacks account identity or permissions.

Import receives archive bytes and the supplied password and either returns a
complete checked credential or an error. It performs no persistence and leaves
existing credentials untouched on failure. An expired issued credential may be
imported for display/history; eligibility is evaluated separately.

The import checks are:

1. Decode the archive and require its message authentication code (MAC). Verify
   it before accepting the contents. Reject non-positive declared MAC iteration
   counts before delegating to archive decoding. The selected library accepts absent MACs,
   so its default acceptance alone is insufficient.
2. Decode the supported certificate/private-key bags and decrypt the key. A
   normal TQSL export contains one station credential; supporting certificates
   are not independent station credentials. No multi-credential partial-import
   workflow is needed. Reject missing or ambiguous station/key selection rather
   than silently choosing an unrelated bag.
3. Match the actual RSA public modulus/exponent derived from the private key to
   the station certificate. Aliases, friendly names and `localKeyId` are hints,
   not proof of a match. Validate the private key using the established library.
4. Interpret the required public metadata from the issued certificate, retaining
   its original DER bytes. Do not substitute key-bag claims or a common name for
   the certificate's station callsign.

Reject malformed or ambiguous required metadata, invalid date ranges, unsupported
algorithms and unusable keys with contextual errors. Do not guess missing coverage
or silently accept an unsupported archive as an empty successful import. Unknown
unrelated bag types do not create additional product scope.

PKCS#12 MAC failure does not prove whether the password was wrong or the archive
was corrupted. Preserve that diagnostic uncertainty. An unsupported password
encoding is different from verified bad integrity. Do not retain the export
password as part of the returned credential or public metadata.

## 4. Public metadata and eligibility

Object identifiers (OIDs) locate the LoTW metadata independently of parser names:

| Metadata | OID/source |
| --- | --- |
| Station callsign | Subject `AROcallsign`, `1.3.6.1.4.1.12348.1.1`. |
| Earliest covered QSO date | Extension `1.3.6.1.4.1.12348.1.2`. |
| Latest covered QSO date | Extension `1.3.6.1.4.1.12348.1.3`. |
| DXCC entity | Extension `1.3.6.1.4.1.12348.1.4`. |
| Certificate validity | X.509 `notBefore` and `notAfter`. |
| Public identity/encoding | Issuer, full serial, public key and original DER certificate. |

The raw certificate callsign must satisfy the supported ASCII/control-text
boundary before native trimming and uppercase normalization, so Unicode case
expansion cannot silently change the certificate claim.

The coverage/entity extension contents are raw date/decimal text, not an assumed
additional nested string/integer encoding. Parse required values strictly; do not
copy upstream integer narrowing or permissive malformed-number conversion.

Eligibility uses the prepared signing identity, QSO date, explicit evaluation time
and current credential state. It must explain failure rather than return an
undifferentiated false result:

- Station callsign and DXCC must match the selected certificate. Do not strip
  portable components or change the frozen identity to make a match.
- The QSO date must lie within both certificate coverage dates, inclusively.
  Missing/malformed bounds do not mean unrestricted coverage.
- Match TQSL's UTC-day expiry rule: `notAfter`'s date remains eligible through
  that day and expires the following day. Preserve `notBefore` metadata without
  adding a full current-time X.509 validity/chain gate absent from that selection
  behavior.
- Expired, explicitly superseded or key-removed credentials cannot sign. The
  later credential-management adapter supplies authoritative current state;
  this library does not invent a cache or database to own it.

Missing or ambiguous routing pauses only affected uploads. Do not choose the
newest or first certificate merely to resolve overlap. Supersession is explicit,
with coverage-gap preview in the later management workflow. Local key removal
and issuer revocation remain distinct.

Support the 1024-bit RSA keys generated by the pinned TQSL request path as import
inputs. Do not impose a 2048-bit minimum that excludes ordinary existing exports.
Use key-sized signature storage rather than copying TQSL's fixed 129-byte buffer.
List additional tested key sizes precisely without asserting universal LoTW
acceptance. Key generation remains outside scope.

## 5. Frozen submissions and signature definitions

The signer consumes existing `LotwSubmission` contact/station fields, mapping
provenance and public signing identity, plus an explicitly selected credential.
It does not read the original QSO, reload profiles, select location fallbacks,
remap modes/bands or recalculate the external-record key.

Validate that the credential, supported mapping/configuration and signature
definition can represent those prepared values. Fail rather than silently drop
or reinterpret a field. Station `CALL` and `DXCC` come from the frozen
`signing_identity`; the current `station_fields` map holds location values and
is insufficient by itself to construct a station record.

Keep signature definitions as source-derived data in this library, separate
from the model's normalization data. Definitions retain ordered station/contact
fields, required flags, name, version and deprecation status. Follow TQSL's
highest-version nondeprecated selection. The pinned configuration selects LoTW
2.0 and `SIGN_LOTW_V2.0`. Reject an unsupported selected definition rather than
silently falling back to an older one. There is no user-facing version selector.

Definition selection and the model's configuration provenance must be compatible;
do not reinterpret historical prepared submissions using silently updated data.
Bundled data records upstream commit/configuration and has a reproducible
extraction check. Signature-construction tests are distinct from extraction tests.

## 6. Exact signing and record construction

Use the upload path `tqsl_getGABBItCONTACTData`, not the similarly named standalone
`tqsl_signQSORecord`, as the signing-input reference:

1. Take present trimmed station values in the signature definition's station
   order, followed by present trimmed contact values in its contact order.
2. Concatenate with no separators, field names or record headers. Missing optional
   values add no bytes. Reject missing required values.
3. Uppercase the complete signing input as the upload path does.
4. Sign those exact bytes using digest-aware RSA PKCS#1 version 1.5 with SHA-1
   and explicit random blinding. Raw/unprefixed RSA signing is not equivalent.

The pinned contact order is `BAND`, `BAND_RX`, `CALL`, `FREQ`, `FREQ_RX`, `MODE`,
`PROP_MODE`, `QSO_DATE`, `QSO_TIME`, `SAT_NAME`. Required fields are `BAND`, `CALL`,
`MODE`, `QSO_DATE`, `QSO_TIME`. Optional satellite/propagation fields in the
reference do not add missing native-model features to this issue.

Convert compact frozen ADIF dates/times losslessly to `YYYY-MM-DD` and
`HH:MM:SSZ` for both signing and wire output, retaining whole-second precision.
Preserve prepared frequency spelling, including trailing zeroes. Formatting
conversion does not mutate or replace retained semantic evidence.

Protocol text handling must match the supported reference cases. The C helpers
are byte/locale based, not evidence for Unicode normalization or transliteration.
Do not silently transliterate prepared signing values. Declare unsupported text
precisely and cover the supported domain with independently constructed fixtures.

Output contains:

- Producer/configuration identification in `TQSL_IDENT`, truthfully identifying
  Clacks rather than claiming the TQSL executable's version.
- `tCERT` with an output-local `CERT_UID` and base64 of the selected station
  certificate, without PEM delimiters. Supporting chain certificates do not
  automatically become additional `tCERT` records.
- `tSTATION` with `STATION_UID`, `CERT_UID`, station callsign/entity and prepared
  location values.
- `tCONTACT` with `STATION_UID`, contact values, the selected signature field
  with ADIF type `6`, and `SIGNDATA` containing the actual uppercased signing input.

Byte lengths include actual value bytes, including base64 wrapping/newlines where
present. Field/header order, line endings, delimiters and base64 encoding must be
checked against independently authored, source-derived wire expectations. Output record order is not signing field order.
Signature bytes, record references and producer text do not change the existing
service semantic-equality rules.

## 7. Batch writing and dispatch integration

Serialize prepared submissions and explicitly selected credentials through
`std::io::Write`. Callers may supply a buffer, file or compression writer. Emit
each certificate once per batch and share a station record for the same
certificate and normalized station values. Different locations require separate
station records. Contacts reference their station records. References exist only
within that output batch, never as synchronization/evidence identities.

Use normal short-write/error handling. A signing or writing failure stops batch
construction; partial bytes are not a valid reduced batch. The caller discards the
incomplete artifact. Arbitrary `Write` sinks cannot provide rollback, and the
library does not promise it or introduce an atomic-writer abstraction.

Complete the entire signed artifact before network dispatch. The caller must
successfully finalize compression before dispatch; flushing an arbitrary writer
does not guarantee compression finalization. The later provider adapter owns
staging storage/cleanup, durable intent and dispatch transitions, exact attempt
membership, retries and uncertain delivery. The serializer neither records a
successful upload nor starts HTTP. Changing membership requires explicit batch
reconstruction and matching evidence, not silently skipping a failing contact.

Later integration rechecks credential state at its signing boundary and preserves
the agreed removal/affected-upload behavior. It retains actual dispatched semantic
values and public signing identity through the existing service-evidence contract.
A later retry uses the current QSO under that contract; this serializer does not
create a policy of forever resending the first attempt's version.

## 8. Errors, secrets and bounded work

Use concrete `thiserror` errors for actionable domain failures and `anyhow` where
operations combine underlying errors. Preserve sources; avoid forwarding-only
unit variants and stringifying away causes. Context should identify the failed
stage, safe batch/contact position, definition or field when relevant, without
embedding passwords, decrypted key bytes or whole archive/submission dumps.
Do not expose production `expect()`/`unwrap()` or unchecked indexing as error handling.

Passwords and decrypted keys are not ordinary serializable/debuggable domain
objects. Minimize copies/lifetimes, avoid secret-bearing `Debug`, and use available
secret-erasure facilities without claiming that every dependency allocation is
zeroized. Persistent protection, account-key wrapping and backup handling remain
the server credential adapter's responsibility.

Enforce parser/cryptographic work bounds and document the resolved dependencies'
actual limits and failure behavior. A small archive alone does not bound its
declared key-derivation work. Do not add an unbounded fallback on limit failure.
With the locked decoder, nonpositive legacy encryption iteration counts are
interpreted as one derivation round. Keep explicit characterization with valid
positive integrity counts and wrong-password rejection; this does not override
the separate requirement to reject nonpositive MAC counts.
Server request-size, concurrency and admission budgets remain adapter policy;
the implementation plan must identify how this library avoids bypassing the
dependency safeguards on any supported path.

The selected PKCS#12 password-derivation path uses `BmpString` and cannot claim
arbitrary Unicode password support. Characterize Basic Multilingual Plane (BMP) and
non-BMP cases against the documented/source-described password semantics and report unsupported encoding explicitly; do not
rewrite passwords. RSA advisory `RUSTSEC-2023-0071` remains unresolved in the
selected version. Assess the actual blinded signing path, record the rationale
and maintenance obligation, and do not blanket-ignore the advisory or describe
blinding as proof it is fixed.

## 9. Observability

Follow the existing observability contract with focused
`#[tracing::instrument(skip_all, ...)]` annotations, ordinary module-path targets
and framework duration/lifecycle support. No completion wrapper, custom lifecycle
machinery, redundant `operation`/`component` fields or telemetry-only formatting
helpers. Do not enable automatic returned-value/error logging.

Archive import and signed-batch construction are meaningful operation spans at
`debug`; include safe sizes/counts and available correlation inherited from the
caller. A successful batch event reports the actual certificate/station/contact
counts and output size, not external success. Useful diagnostic branches explain
unsupported format/definition, failed local checks and incomplete construction
through bounded categories, without raw supplied values. Expected eligibility
reasons remain `debug`. Later management adapters own `info` security-state-change
events; the upload adapter owns dispatch/failure events and severity.

The later credential/import and provider workflows own unsampled operation rates,
error rates and latency histograms, including unsuccessful attempts. Keep archive
import and signed-batch construction as separate measurements there; batch
construction is not an upload. This library exposes diagnostic timing through
ordinary spans and work counts through domain events. No additional library metric
family is justified merely by adding functions, and no custom timing/completion
guard is required. Do not infer unsampled operational totals from filtered traces.
The library installs no subscribers, recorders or exporters and creates no live
resource gauges. Integration metrics must use bounded outcomes, not identities,
paths, callsigns, error strings or arbitrary field values as labels.

Test actual operation outcomes/counts, safe selected fields with recognizable
fixture secrets, and preservation of results/errors when telemetry is unconfigured
or spans are filtered. Do not test tracing's own span lifecycle implementation.

## 10. Compatibility evidence and acceptance

Use pinned TQSL 2.8.6 commit
`6ce824363e27da455c6cf6fd440b9c401945bfcb` and configuration 11.34. The
[source map](../references/trustedqsl/source-map.md) and
[compatibility findings](../references/lotw-cryptography.md) identify functions,
dependency constraints and notices. Additional reference source must have the
same provenance and applicable notices.

Use controlled, non-production certificates with the actual LoTW subject and
extensions. The supplied TQSL sources are strictly read-only reference material:
never compile, link, execute, wrap or otherwise use them directly, including in
tests, fixture generation, external tools or containers. Do not retain outputs
obtained by executing those sources as a substitute for this boundary.

Author expected signing inputs and records independently from the documented
protocol and source inspection, with worked examples reviewed separately from
the Rust implementation under test. Use independently constructed synthetic
archives covering the source-described export algorithms and layout. Verify
cryptographic results against independently constructed known answers and an
independent verifier; a sign/verify round trip through the same implementation
alone is insufficient. Document which evidence checks protocol construction,
archive handling and cryptographic correctness separately. Do not describe
source-derived tests as executed TQSL interoperability results.

Keep known test private keys in test support and constructors in the dedicated
Rust fixture tool, isolated from application dependencies. Non-trivial fixtures
must be reproducible with the project's locked Rust tooling; Python generation
is prohibited even when its scripts are not checked in. Use
`cargo +1.99.1 run --release --bin clacks-lotw-fixtures --locked --offline -- --check`
to reconstruct and compare all committed bytes; `--write` regenerates them. Record
source commit/configuration, construction procedure/tool versions, explicit
inputs, password/algorithm cases, expected metadata, exact signing input/signature
and parsed record expectations. Expected values must not be generated by the
production implementation under test. Synthetic credentials do not establish
ARRL issuance or live acceptance and must not be submitted to LoTW.

Required coverage:

| Area | Cases and proof |
| --- | --- |
| Archives | Normal AES-256 with SHA-256 integrity and legacy RC2/Triple DES with SHA-1 integrity; certificate-first and key-first safe order; nonpositive legacy encryption counts under valid positive integrity counts; correct, incorrect, empty and non-ASCII passwords; characterize non-BMP limits; absent/bad MAC, truncation, unsupported encryption and excessive declared work. AES is Advanced Encryption Standard; DES is Data Encryption Standard; RC2 is a historical cipher name. |
| Credentials | Matching/mismatched RSA keys, missing/ambiguous station credential, missing/malformed/duplicate required metadata, full serials, unrelated supporting certificates and 1024-bit/larger supported keys. |
| Eligibility | Inclusive coverage endpoints, just-outside dates, current UTC expiry day and following day, callsign/entity mismatch, removed/superseded state, explicit evaluation time. |
| Definitions | Reproducible extraction, selection/deprecation/version behavior, ordered fields, required/optional fields, unsupported definitions and incompatible configuration provenance. |
| Signing | Hand-reviewed source-derived exact input/records, independent cryptographic vectors and verification, whitespace/case, every supported station field, frequency spelling, receive overrides and compact-to-wire timestamps. |
| Records | Correct byte lengths, base64, signature type, `SIGNDATA`, producer header, linked certificate/station references, shared/different locations and multiple certificates/contacts. |
| Failure | Short writes and failure after a prefix; no successful artifact or silently reduced membership; unchanged frozen submissions and normal error propagation. |
| Integration boundary | Existing production model projections feed the actual signer; no normalization fork or cryptographic types in shared models; server-only dependency graph. |

Before issue completion, run workspace verification and supported server-target
checks; inspect the locked features, licenses, historical notices, advisory and
packaging graph. Record tested combinations and unsupported cases accurately.
The crate guide owns public usage and adapter obligations once implemented.
Live authenticated upload, retained service values and network-outcome recovery
are later provider-integration acceptance checks. This issue stays open until
production components and their independent compatibility evidence are delivered;
this specification and a subsequent plan do not complete it.

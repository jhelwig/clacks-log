# LoTW cryptographic dependencies

Checked 22 October 2026 against released crate sources and the pinned TrustedQSL
2.8.6 implementation. D224 selects pure Rust. The dependency combination and
narrower trust boundary below are accepted for issue #4; they are not a claim of
live interoperability. The production library and locked dependency combination
are implemented and exercised with independent synthetic inputs as described
below; no private user certificates were inspected or live submissions made.

## Selected combination

| Responsibility | Selected dependency | Required use |
| --- | --- | --- |
| Certificate archive decoding | `p12-keystore` 0.4.1 with `pbes1` support | Use `Pkcs12Archive`, not the alias/trust-store import policy. Support normal and legacy TQSL exports. Require and verify archive integrity. |
| Certificate interpretation | `x509-parser` 0.18.1 without `verify` or `verify-aws` | Read required LoTW subject attributes/extensions, public key, issuer/serial and validity. Preserve original certificate bytes. |
| Submission signing | RustCrypto `rsa` 0.9.10 and `sha1` 0.10.7 with digest-identifier support | Use the digest-aware Public-Key Cryptography Standards #1 (PKCS#1) version 1.5 construction and explicit randomized/blinded signing interface. |

These are server-side dependencies. Shared domain, synchronization and browser/
desktop consumers do not acquire certificate/private-key or signing-library types.
The signer consumes the frozen normalized `LotwSubmission` already produced by
`clacks-model`; it does not remap modes or reinterpret the original QSO.

## Certificate usability and LoTW acceptance

Clacks checks password-based archive integrity, successful decoding, the actual
public-key match between the private key and station certificate, and required
metadata. It checks local signing eligibility using the recorded station identity,
QSO dates, certificate validity and explicit credential state. A friendly name or
local key ID can assist discovery but does not establish a key match.

**Issuer-chain acceptance is LoTW's responsibility when receiving uploads.**
Clacks does not provision an ARRL trust-root bundle, add uploaded roots to a trust
store, or require local issuer-chain validation before import/signing. Certificate
claims do not authenticate the Clacks account or grant account permissions.
Archive integrity is not proof of the certificate issuer's identity.

The reference importer first imports root and intermediate certificates from the
archive, then checks the callsign certificate against its local certificate store.
Its application callback can prompt for a new root; the inspected callback defaults
that prompt off. This is observed TQSL credential-management behavior, not a Clacks
requirement. See the pinned [importer and certificate handlers](trustedqsl/upstream/src/openssl_cert.cpp)
and [application callback](https://sourceforge.net/p/trustedqsl/tqsl/ci/6ce824363e27da455c6cf6fd440b9c401945bfcb/tree/apps/loadcertwiz.cpp).

LoTW's [confirmation-query interface](https://lotw.arrl.org/lotw-help/developer-query-qsos-qsls/)
returns ordinary Amateur Data Interchange Format (ADIF) records over HTTPS. Its
authenticity relies on the HTTPS client authenticating LoTW's server. The interface
does not specify per-confirmation signatures or verification using imported
callsign certificates. HTTPS server-certificate verification is a separate network
boundary, not a reason to add a certificate-chain verifier to this subsystem.

## Actual TQSL export coverage

Public-Key Cryptography Standards #12 (PKCS#12) is the archive format normally
saved as `.p12` or `.pfx`. The pinned `tqsl_exportPKCS12` exports one selected
station certificate and private key, plus its supporting certificate chain. A
multi-station-credential partial-import workflow is not required merely because
the container format permits it. Certificate requests/renewals remain deferred.

- The normal export path selects Advanced Encryption Standard (AES)-256 encryption.
- The optional `weakCrypto` path selects 40-bit RC2 certificate encryption, Triple
  Data Encryption Standard (Triple DES) private-key encryption and Secure Hash
  Algorithm 1 (SHA-1) archive integrity. RC2 is a historical cipher name.
- TQSL writes a message authentication code (MAC), and its importer verifies it.
- `tqsl_signDataBlock` uses SHA-1 with the private key. For the Rivest–Shamir–Adleman
  (RSA) keys used by TQSL, the compatible construction is digest-aware PKCS#1
  version 1.5 signing; raw/unprefixed signing is not equivalent.

These findings come from the pinned [certificate/export/signing source](trustedqsl/upstream/src/openssl_cert.cpp).
They describe protocol compatibility, not algorithms for Clacks credential storage.

## Archive and parser integration findings

The released `Pkcs12Archive` retains decoded certificate/key bags before alias
assignment or certificate linking. Modern and legacy decryption are implemented;
the legacy path uses a five-byte RC2 key and a 24-byte Triple DES key. The enum
name `PbeWithShaAnd40BitRc4Cbc` is misleading: the inspected implementation uses
RC2 for that identifier. [Archive source](https://github.com/ancwrd1/p12-keystore/blob/21a427cacf79b10186aa1fc6f953516d95e0379d/src/archive.rs),
[legacy cipher source](https://github.com/ancwrd1/p12-keystore/blob/21a427cacf79b10186aa1fc6f953516d95e0379d/src/pbes1.rs).

The archive API verifies a MAC when present but accepts archives without one.
Clacks must explicitly require its presence for the supported TQSL exports using
an existing format parser; do not silently equate the library's default acceptance
with the intended integrity requirement. Unknown bag attributes are not fully
exposed by this API. Required issued-certificate metadata comes from the actual
certificate; request-only key attributes do not add certificate-request scope.

`x509-parser` preserves access to subject values and raw extensions, including
fields not named by a built-in catalog. Its default feature set is empty;
`verify` enables `ring` and `verify-aws` enables AWS's native cryptographic backend.
Neither is selected. No separate X.509 signature/chain-verification dependency is
needed. [Parser documentation](https://docs.rs/x509-parser/0.18.1/x509_parser/).

## Signing security and maintenance

In `rsa` 0.9.10, `try_sign_with_rng` supplies random blinding to the private
operation. Ordinary `try_sign`, digest signing and prehashed signing do not all
supply blinding. Use the explicitly randomized interface with a cryptographically
secure random source. Blinding randomizes the private computation without changing
the resulting protocol signature. [Released signing implementation](https://github.com/RustCrypto/RSA/blob/v0.9.10/src/pkcs1v15/signing_key.rs).

[RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071.html) remains
unpatched. The [upstream tracking issue](https://github.com/RustCrypto/RSA/issues/626)
focuses on padding/decryption timing; this signing subsystem does not offer RSA
ciphertext decryption. That distinction and explicit blinding are relevant to a
version-specific assessment, not grounds to declare the advisory resolved or
suppress it without explanation. The inspected 0.10.0-rc.19 source also retains
an explicit randomized signing interface; a prerelease version number alone does
not establish that the concern is fixed.

The top-level archive release is stable, but its manifest includes prerelease
`pkcs12` and `cms` dependencies. It also uses a newer digest/randomness dependency
family than `rsa` 0.9.10. Use standard encoded key/certificate bytes at the boundary,
not an assumption that their Rust library types are interchangeable. A locked,
resolved dependency/feature inventory and build remain required. The inspected
direct dependencies declare MIT/Apache-2.0 licenses; preserve their notices and
review the final transitive set. [Archive manifest](https://github.com/ancwrd1/p12-keystore/blob/21a427cacf79b10186aa1fc6f953516d95e0379d/Cargo.toml).

## Other researched options

| Option | Assessment for the accepted scope |
| --- | --- |
| Picky 7.0.0-rc.27 | Broader archive/certificate toolkit with additional raw attributes. Its manifest pins RSA 0.10.0-rc.18 and brings additional cryptographic families. It does not independently resolve the RSA advisory, and no demonstrated requirement justifies replacing the selected smaller composition. [Manifest](https://github.com/Devolutions/picky-rs/blob/59487be05e6db1f01f5faa72fc926eb5fb4751b1/picky/Cargo.toml). |
| `p12` 0.6.3 | Inspected algorithm dispatch handles legacy SHA-1/RC2/Triple DES, without the normal TQSL export's modern encryption path. Unsuitable as the sole reader. [Source](https://docs.rs/crate/p12/0.6.3/source/src/lib.rs). |

OpenSSL, mixed native signing and a runtime-pluggable cryptographic framework are
not selected. Keeping issuer-chain acceptance at LoTW also removes the proposed
local trust-bundle and chain-verifier work.

## Signature and wire compatibility

These findings use the pinned TrustedQSL 2.8.6 source and configuration 11.34,
not live LoTW acceptance. The accepted serializer and dispatch contracts are in
[LoTW signing](../docs/lotw.md#batch-signed-output-serialization).

- `location.cpp::make_sign_data` selects the highest nondeprecated signature
  definition. The pinned selection is LoTW 2.0. Preserve definition field order
  as data; neither alphabetical map iteration nor emitted field order supplies
  the signing order.
- Build the station portion from the trimmed, present values in the definition's
  `tSTATION` order. Append trimmed contact values in its `tCONTACT` order without
  separators or field names. The upload path, `tqsl_getGABBItCONTACTData`, uppercases
  the complete concatenation before signing. The separate `tqsl_signQSORecord`
  helper does not perform that final uppercasing; it is not the complete upload
  oracle.
- The ordered contact fields are `BAND`, `BAND_RX`, `CALL`, `FREQ`, `FREQ_RX`,
  `MODE`, `PROP_MODE`, `QSO_DATE`, `QSO_TIME`, `SAT_NAME`. Required fields are
  `BAND`, `CALL`, `MODE`, `QSO_DATE` and `QSO_TIME`. Unsupported native satellite
  inputs remain outside this issue; absent optional values contribute no bytes.
- The model's frozen fields use compact ADIF dates/times. TQSL's
  `tqsllib.cpp::tqsl_convertDateToText` and `tqsl_convertTimeToText` produce
  `YYYY-MM-DD` and `HH:MM:SSZ` for signing and GAbbI output. The serializer must
  perform this lossless syntax conversion, retaining whole-second precision and
  the original frozen semantic projection. Do not sign the compact strings or
  rerun QSO normalization. Preserve the prepared frequency spelling, including
  trailing zeroes; the reference converter's `fix_freq` does not canonicalize
  numeric precision.
- For RSA credentials, `openssl_cert.cpp::tqsl_signDataBlock` uses digest-aware
  PKCS#1 version 1.5 with SHA-1. Use the selected blinded Rust interface. Blinding
  does not change the signature bytes for the same key and input, allowing exact
  reference comparisons.
- `tqsl_getGABBItCERT` emits the selected station certificate's base64 content,
  without PEM delimiters. Supporting chain certificates from import are not
  additional signed station credentials or automatically emitted `tCERT` records.
  Preserve the original certificate encoding.
- `tqsl_getGABBItSTATION` emits station values plus `STATION_UID` and `CERT_UID`;
  `tqsl_getGABBItCONTACTData` emits the station reference, contact fields,
  `SIGN_LOTW_V2.0` with ADIF type `6`, and `SIGNDATA` containing the actual signing
  input. Reference numbers, field headers and record delimiters are not part of
  that input. Serialized lengths count bytes, as `adif.cpp::tqsl_adifMakeField`
  does. Base64 wrapping and newlines are included in the relevant field lengths.
  Emit station `CALL` and `DXCC` from the frozen `signing_identity`, checked
  against the selected credential. The model's `station_fields` contains location
  values, not those two identity fields; copying that map alone is insufficient.
- `tqslconvert.cpp::get_ident_rec` adds `TQSL_IDENT` producer/configuration
  information before records. Clacks output should identify its actual producer
  and compatibility configuration, rather than claim to be the TQSL executable.
  This header is separate from signing input and service semantic equality.

The source's uppercasing/whitespace helpers operate on bytes using C character
classification. They are not evidence for Unicode normalization or transliteration.
Fixtures must establish the supported protocol text domain; reject unrepresentable
prepared values with context rather than silently changing their meaning.

Source owners: [record construction](trustedqsl/upstream/src/location.cpp),
[definitions](trustedqsl/upstream/src/config.xml),
[primitive and certificate handling](trustedqsl/upstream/src/openssl_cert.cpp),
[date/time/base64 helpers](trustedqsl/upstream/src/tqsllib.cpp),
[field encoding](trustedqsl/upstream/src/adif.cpp) and
[conversion](trustedqsl/upstream/src/tqslconvert.cpp).

## Certificate interpretation and eligibility

Object identifiers (OIDs) identify the following metadata independently of a
parser's built-in names. `tqsllib.cpp` registers them; `openssl_cert.cpp` consumes
them through the public certificate getters and `tqsl_get_cert_ext`.

| Metadata | OID | Encoding used by the reference |
| --- | --- | --- |
| Station callsign | `1.3.6.1.4.1.12348.1.1` | Subject attribute `AROcallsign`, not the common name. |
| Earliest covered QSO date | `1.3.6.1.4.1.12348.1.2` | Date text in the extension's raw octet-string contents. |
| Latest covered QSO date | `1.3.6.1.4.1.12348.1.3` | Date text in the extension's raw octet-string contents. |
| DXCC entity | `1.3.6.1.4.1.12348.1.4` | Decimal text in the extension's raw octet-string contents. |

Do not assume these extension contents contain an additional nested encoded
string or integer. Match the certificate's RSA public modulus/exponent against
the private key; archive aliases and `localKeyId` attributes cannot replace that
check. Bag attributes are not authoritative substitutes for issued-certificate
metadata. Preserve issuer, full serial and original certificate bytes for public
metadata/identity without narrowing serials to TQSL's older integer APIs.

QSO coverage and certificate expiration are different checks:

- `tqsl_filter_cert_list` and the converter's `find_matching_cert` match station
  callsign/entity and compare the QSO's date inclusively against both coverage
  dates. Missing/malformed coverage is not evidence for an unrestricted range.
- `tqsl_isCertificateExpired` compares the certificate's `notAfter` **date** with
  the current UTC date: it is expired starting the following day. It also consults
  TQSL's local status cache. The inspected selection path does not enforce the
  X.509 `notBefore` instant as a separate signing gate. Preserve both certificate
  validity values as metadata; do not describe the reference as doing full
  current-time X.509 validity or chain verification.
- Clacks's explicit supersession, key removal and ambiguous-routing rules still
  apply. Do not import TQSL's status cache, automatic supersession or first-match
  selection policy. Pass evaluation time explicitly for reproducible eligibility
  checks rather than hide a clock read in a getter.

TQSL generates 1024-bit RSA keys in the pinned request path. Although requests
are deferred, importing those existing keys is necessary compatibility coverage;
do not impose a 2048-bit minimum that rejects ordinary exports. Its fixed
129-byte signature buffer is an implementation limit, not justification for a
Clacks fixed-size buffer. Verify larger supported RSA keys separately without
claiming that every key size has been accepted by LoTW.

## Verification boundary

Compatibility acceptance needs independently constructed normal/legacy-layout archives, wrong/
empty/non-ASCII passwords, missing or failed archive integrity, key mismatch,
required metadata and validity/date boundaries, precise signature-input bytes,
independent signature verification, and correctly linked signed records. Use
production components, with controlled/reference fixtures and recorded provenance.
No private user export is required for initial fixtures. Live LoTW acceptance
belongs to later provider integration; it has not been demonstrated by this research.

The supplied TQSL sources are reference material only. Never compile, link or
execute them, including for fixtures or external validation tools. Author test
inputs and expected protocol bytes independently from source inspection and
published documentation. Use independent cryptographic known-answer vectors and
verification, keeping those checks distinct from protocol-byte expectations.
No executed-TQSL oracle or derived outputs belong in the test suite.

Retain source commit/configuration, independent fixture construction procedure,
explicit inputs, archive algorithm/password cases, expected metadata, signing
bytes and parsed record expectations. Constructors belong in test support and
must not obtain expected values from Clacks's production code. Synthetic
credentials do not demonstrate ARRL issuance or live acceptance.

The fixture matrix must cover:

- Current AES and legacy RC2/Triple DES archives; correct, incorrect, empty and
  non-ASCII passwords; missing/invalid MAC; corrupt input; unmatched keys and
  missing/malformed/duplicate required metadata. A MAC failure alone cannot
  distinguish a wrong password from corruption.
- Certificate expiry-day boundaries and inclusive QSO coverage boundaries;
  callsign/entity mismatches; explicit unavailable credential states; 1024-bit
  and larger RSA keys. Evaluate each with an explicit time.
- Signature definition selection/order, absent optional fields, whitespace and
  case, all supported station fields, compact-to-wire time conversion, frequency
  spelling and receive overrides. Check exact signing input and signature, then
  independently verify the signature and parse field lengths/references.
- Shared/distinct certificate and station records, multiple contacts, and a
  writer that accepts short writes or fails after a prefix. Failed construction
  must never produce a dispatchable artifact or silently reduced membership.

The selected archive library's password derivation uses `BmpString`, restricting
that path to Unicode's Basic Multilingual Plane. Non-BMP password compatibility
must be checked against documented/source-described semantics and reported explicitly; do not claim arbitrary
Unicode password support. Input-size and key-derivation work bounds also need
concrete limits in the specification; dependency defaults alone are not a server
resource policy.

The implemented `clacks-lotw` library combines these dependencies without native
cryptography. Its static synthetic archives and independent RSA known answers
exercise the real importer and signing/serialization path. Literal signing input
and wire expectations come from reading the pinned sources, never executing them.
See the [caller and verification guide](../lib/clacks-lotw/README.md). This evidence
does not establish executed-TQSL interoperability, ARRL issuance or live acceptance.

Historical notices embedded in adapted TrustedQSL source still apply regardless
of runtime dependencies. See the [source map](trustedqsl/source-map.md#licensing)
and [license review](licensing.md#projected-dependency-compatibility).

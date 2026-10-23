# Logbook of The World (LoTW) signing and certificate management

The accepted direction is a source-guided Rust port of relevant tqsllib behavior. The [certificate/signing specification](lotw-signing.md) consolidates the production contract for issue #4. Preserve LoTW interoperability while using explicit tenant context, managed credential storage, safe key access, durable publication state within D213’s single Clacks server application. Horizontal scaling is not an additional accepted deployment requirement.

## Scope of the port

Use the available reference implementation and configuration definitions to preserve certificate interpretation, signing eligibility, normalization, signature construction, and submission serialization. This is a source-guided port, not an attempt to infer an undocumented black box from outputs.

The supplied TQSL source files are read-only reference material. Never compile, link or execute them, including for tests or fixture generation.

Do not reproduce filesystem stores, process-global errors/caches, graphical user interface (GUI) dependencies, incidental import return conventions, or automatic certificate deletion simply because upstream implements them that way.

Cryptographic primitives should come from established libraries. Most application code should be Rust. D224 selects pure-Rust libraries for certificate import, interpretation and signing. The selected combination is `p12-keystore` 0.4.1 with legacy `pbes1` support, `x509-parser` 0.18.1 without verification features, and RustCrypto `rsa` 0.9.10 with `sha1` 0.10.7 using the explicitly randomized/blinded signing interface. The [dependency assessment](../references/lotw-cryptography.md) records source findings and verification limits.

## Minimum viable product (MVP) certificate workflow

- Any connected client can initiate import of an existing certificate/private-key export, normally `.p12`.
- Import requires a connection to the server; offline certificate import is out of scope.
- Collect the export password when required. The server checks archive integrity, parses the certificate metadata, and verifies that the private key matches the certificate public key, then protects stored key material with server-managed secrets. These local checks do not establish issuer trust or prove LoTW acceptance.
- The export password and ongoing credential protection are separate concerns. No accepted design requires retaining the user's export password.
- Display callsign, DX Century Club (DXCC), certificate validity, eligible radio contact (QSO) dates, and signing readiness.
- Keep expired and superseded certificates visible with status; exclude them from signing.
- D229 makes supersession explicit. Importing a validated credential does not supersede an older certificate merely because its callsign matches or its expiration is later. Show resulting coverage gaps before applying explicit supersession; preserve public metadata and submission history. Supersession excludes signing but does not itself select private-key removal under D228.
- D228 includes private-key credential removal in MVP. Prevent further signing with removed credentials, retaining public certificate metadata and submission history. Pause only uploads left without an eligible credential. Local removal is distinct from revocation with the certificate issuer.
- Reusable certificate/private-key export from Clacks is post-MVP under D228. Documented native backup/recovery remains required; backup is separate from this export feature.
- Certificate requests and renewals are later scope. Importing a replacement issued elsewhere remains part of import management.

D236 selects independent account encryption keys protected by a deployment wrapping key supplied outside PostgreSQL; encrypted keys and credentials reside in PostgreSQL. Exact encryption tooling/encoding, rotation, key-version retention, authentication/authorization, audit policy, key-access boundary, removal/in-flight mechanics, backup-retention effects and memory handling remain engineering work. Credential-removal scope and certificate/private-key export timing are settled in D228. Server-managed secrets do not imply a plaintext file store or one global shared tenant secret.

## Local checks and service acceptance

Certificate-chain acceptance belongs to LoTW when receiving a submission. Clacks
does not build an ARRL root store, bootstrap trust from uploaded roots, or validate
the issuer chain as an import/signing prerequisite. Imported certificate claims
do not authenticate a Clacks account or grant account permissions.

Local checks establish that the archive can be decoded with its integrity check
verified, that the private key matches the station certificate, and that required
LoTW metadata can be interpreted. Apply the agreed callsign/entity/date and
credential-state eligibility checks before signing. A password-based archive
integrity check is distinct from authenticating the certificate issuer. Preserve
original certificate bytes for submission rather than replacing or reissuing them.

TQSL's normal export contains one selected station certificate/private key and
its supporting certificate chain. Supporting chain certificates are not independent
station credentials. The broader archive format does not require a multi-credential
partial-import workflow for #4. Current and legacy TQSL export compatibility is the
relevant boundary; certificate requests/renewals remain deferred.

LoTW's [confirmation-query interface](https://lotw.arrl.org/lotw-help/developer-query-qsos-qsls/)
returns ordinary ADIF over HTTPS. The network client authenticates LoTW through
normal HTTPS server-certificate verification; the response has no separate
callsign-certificate-chain or per-confirmation signature-verification step. That
network trust path is separate from imported upload-signing credentials.

## Import behaviors to design deliberately

The tqsllib reference identifies duplicate certificates by issuer and serial. Public-Key Cryptography Standards #12 (PKCS#12) import writes a key before all certificate processing completes and may report a duplicate using a nonzero return. Its older-certificate check compares expiration for the same callsign without checking DXCC/QSO coverage. Key replacement matches public keys, not merely callsigns. Desktop cleanup separately removes expired/superseded certificates.

These are compatibility findings, not behavior we must copy into the product. D229 rejects inferred supersession: import adds the validated credential and supersession requires an explicit action with coverage gaps shown beforehand. Same callsign or later expiration is insufficient. Multiple eligible credentials follow D53’s affected-upload pause when routing remains ambiguous. Harmless duplicate import after checking usability, clear import outcomes, no wholesale deletion by callsign, and rejected imports leaving existing usable credentials intact remain proposed engineering behavior. Exact eligibility, coverage calculation, overlap configuration and import outcomes still need specification.

## Operating locations

The logger should provide its own station-location management. TrustedQSL (TQSL) signing tool's configuration describes country/entity-dependent fields and allowed values; a GUI wizard is not a protocol requirement.

Each QSO captures its operating location as a snapshot at logging time. Saved profiles synchronize, while current logging defaults are independent per client. Profile/default edits do not change historical signing inputs; correcting existing QSOs requires explicit editing. Profiles are optional for one-off locations, and no operating-session start/end lifecycle is required. Native station/location values and validation are defined in the [implemented model specification](native-model.md). The model library implements source-derived signing-input mapping and validation; the server-side `clacks-lotw` library implements checked certificate handling and signed protocol serialization.

## What tqsllib supplies

LoTW submissions are digitally signed, not encrypted QSO payloads. Certificates use standard X.509 and key exports use PKCS#12. The inspected 2.8.6 signing function uses standard OpenSSL signature operations with Secure Hash Algorithm 1 (SHA-1); key generation uses Rivest–Shamir–Adleman (RSA) public-key algorithm. This is a protocol compatibility observation, not a choice to use these primitives for our own credential encryption.

The domain-specific work includes LoTW certificate extensions, station/contact signing field definitions, normalization, GAbbI record references and signature encoding, mode/band mappings, geographic data, certificate selection, and correction/duplicate semantics.

The released configuration includes LoTW 1.0 and 2.0 signature definitions. The published 2002 GAbbI specification is background; the pinned reference source and current configuration matter for current behavior.

Keep signature definitions as source-derived data in `clacks-lotw`. Select the
highest-version nondeprecated supported definition, following TQSL; the pinned
configuration selects LoTW 2.0 and the `SIGN_LOTW_V2.0` output field. Definitions
specify signing order and required fields. Reject an unsupported definition or a
missing required signing value rather than silently using an older definition.
There is no user-facing signature-version selector. This protocol construction
does not repeat mode normalization or modify frozen semantic submission values.

## Prior library investigation

Unmodified tqsllib uses a filesystem-backed certificate/key store. Base64 import still populates that store. Directory, errors, and configuration/certificate caches include process-global state. Password callbacks supply passwords, not external signatures or replacement key-store implementations. No public custom-store/delegated-signing application programming interface (API) was found in the inspected release.

Direct foreign function interface (FFI) with one tenant per process and temporary stores materialized from managed credentials was explored. It was considered feasible for the existing library but is superseded by the Rust-port direction. It should not silently return as the selected architecture.

## Source and verification plan

Issue #4 implements the production pure-Rust certificate/archive import,
interpretation and signing/serialization components, with compatibility tests.
Later credential management and live submission integrate and extend that code.
A compatibility probe or alternate signing implementation intended for later
replacement does not satisfy the issue.

The [compatibility findings](../references/lotw-cryptography.md#signature-and-wire-compatibility)
identify exact signing order, protocol date/time formatting, certificate metadata
and eligibility boundaries, and the fixture matrix needed for implementation.
The [source map](../references/trustedqsl/source-map.md) identifies the owning
upstream functions. Transport and live service acceptance remain provider-integration
work.

Future compatibility verification should check signing bytes and parsed records against independently authored expectations derived from source inspection, verify signatures independently, and check actual LoTW acceptance and retained values. Gzip metadata or incidental formatting need not match byte-for-byte when protocol semantics do. Cover the supported native input fields, multiple callsigns/entities, historical dates, portable locations, band-only values, receive overrides, and invalid credentials. Reference protocol support for satellite fields does not add those currently unrepresented native inputs to MVP. Live acceptance, corrections and ambiguous upload outcomes are later provider-integration checks.

The model library implements source-derived LoTW normalization and shared semantic submission values. The server-side `clacks-lotw` library implements checked archive import, public metadata, eligibility, blinded signing and standard-writer serialization. Its [caller guide](../lib/clacks-lotw/README.md) records tested limits and compatibility evidence. Live end-to-end upload verification remains provider-integration work.

## Pure-Rust cryptographic backend (D224)

D224 selects pure-Rust certificate import, certificate/key interpretation and signing, rejecting the OpenSSL packaging/maintenance tradeoff. The [backend comparison](../references/lotw-cryptography.md) retains source findings and now follows that choice. No OpenSSL fallback or mixed native signing backend is proposed.

Use the selected released dependency combination above. `Pkcs12Archive` supplies
archive-level decoding without adopting the keystore wrapper's alias or trust
policy; `x509-parser` supplies metadata interpretation without issuer-chain
verification. RustCrypto supplies the required digest-aware RSA signature using
explicit blinding. Neither parser trust decisions nor signing types belong in
shared client/domain models.

Require the integrity check present in TQSL exports; `p12-keystore` verifies it
when present but also accepts archives without it, so library defaults alone do
not enforce that requirement. Preserve public-key matching and required metadata.
Its archive-format dependencies include prereleases; lock and inspect the resolved
feature/dependency graph rather than treating the stable top-level version as an
all-stable dependency graph.

The RSA timing advisory requires a version-specific assessment of the actual
signing path and continued maintenance. Explicit blinding is a mitigation, not a
claim that the advisory is resolved. Import and signing remain server-side;
client/browser/model/synchronization dependencies do not inherit these libraries.

The locked feature graph, synthetic archive/password fixtures and independent
signature vectors are checked by the library tests. Authenticated LoTW acceptance
remains unverified. Existing upstream notices apply to adapted source regardless of
runtime backend. No OpenSSL fallback, runtime-pluggable cryptographic framework,
or local issuer-chain acceptance is selected.

## Shared normalized signing input

`clacks-model::service::lotw::project` prepares `LotwSubmission` from native QSO
values, the public signing identity, reference mappings and pinned configuration
11.34. Source-derived configuration data includes mode mappings/groups, band
ranges, station-field dependencies and entity-specific subdivision enumerations.
The deterministic extraction check verifies the committed derived data against
the vendored source. This does not replace signature/wire compatibility testing.

The future signer consumes the frozen contact/station fields and signing identity
in `LotwSubmission`; it must not normalize the mode a second time. Both signing
and publication coverage depend on the model library, while synchronization
carries evidence and accepted contexts without becoming a cryptographic
dependency. Certificates/private keys are absent from these shared types.

## Batch signed-output serialization

The [certificate/signing specification](lotw-signing.md) defines the production
contract for issue #4. The batch serializer consumes frozen submissions and
explicit credentials through `std::io::Write`, sharing output-local certificate
and station records. Complete and finalize the artifact before network dispatch;
a failed construction is discarded without silently reducing membership. The
later provider adapter owns staging, compression, durable dispatch and outcomes.

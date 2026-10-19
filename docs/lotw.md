# Logbook of The World (LoTW) signing and certificate management

The accepted direction is a source-guided Rust port of relevant tqsllib behavior. Preserve LoTW interoperability while using explicit tenant context, managed credential storage, safe key access, durable publication state within D213’s single Clacks server application. Horizontal scaling is not an additional accepted deployment requirement.

## Scope of the port

Use the available reference implementation and configuration definitions to preserve certificate interpretation, signing eligibility, normalization, signature construction, and submission serialization. This is a source-guided port, not an attempt to infer an undocumented black box from outputs.

Do not reproduce filesystem stores, process-global errors/caches, graphical user interface (GUI) dependencies, incidental import return conventions, or automatic certificate deletion simply because upstream implements them that way.

Cryptographic primitives should come from established libraries. Most application code should be Rust. D224 selects pure-Rust libraries for certificate import, interpretation and signing. The OpenSSL-backed proposal is rejected; exact Rust crate versions/features remain engineering work.

## Minimum viable product (MVP) certificate workflow

- Any connected client can initiate import of an existing certificate/private-key export, normally `.p12`.
- Import requires a connection to the server; offline certificate import is out of scope.
- Collect the export password when required. The server validates certificate identity and corresponding private key, then protects stored key material with server-managed secrets.
- The export password and ongoing credential protection are separate concerns. No accepted design requires retaining the user's export password.
- Display callsign, DX Century Club (DXCC), certificate validity, eligible radio contact (QSO) dates, and signing readiness.
- Keep expired and superseded certificates visible with status; exclude them from signing.
- D229 makes supersession explicit. Importing a validated credential does not supersede an older certificate merely because its callsign matches or its expiration is later. Show resulting coverage gaps before applying explicit supersession; preserve public metadata and submission history. Supersession excludes signing but does not itself select private-key removal under D228.
- D228 includes private-key credential removal in MVP. Prevent further signing with removed credentials, retaining public certificate metadata and submission history. Pause only uploads left without an eligible credential. Local removal is distinct from revocation with the certificate issuer.
- Reusable certificate/private-key export from Clacks is post-MVP under D228. Documented native backup/recovery remains required; backup is separate from this export feature.
- Certificate requests and renewals are later scope. Importing a replacement issued elsewhere remains part of import management.

D236 selects independent account encryption keys protected by a deployment wrapping key supplied outside PostgreSQL; encrypted keys and credentials reside in PostgreSQL. Exact encryption tooling/encoding, rotation, key-version retention, authentication/authorization, audit policy, key-access boundary, removal/in-flight mechanics, backup-retention effects and memory handling remain engineering work. Credential-removal scope and certificate/private-key export timing are settled in D228. Server-managed secrets do not imply a plaintext file store or one global shared tenant secret.

## Import behaviors to design deliberately

The tqsllib reference identifies duplicate certificates by issuer and serial. Public-Key Cryptography Standards #12 (PKCS#12) import writes a key before all certificate processing completes and may report a duplicate using a nonzero return. Its older-certificate check compares expiration for the same callsign without checking DXCC/QSO coverage. Key replacement matches public keys, not merely callsigns. Desktop cleanup separately removes expired/superseded certificates.

These are compatibility findings, not behavior we must copy into the product. D229 rejects inferred supersession: import adds the validated credential and supersession requires an explicit action with coverage gaps shown beforehand. Same callsign or later expiration is insufficient. Multiple eligible credentials follow D53’s affected-upload pause when routing remains ambiguous. Harmless duplicate import after checking usability, clear import outcomes, no wholesale deletion by callsign, and rejected imports leaving existing usable credentials intact remain proposed engineering behavior. Exact eligibility, coverage calculation, overlap configuration and import outcomes still need specification.

## Operating locations

The logger should provide its own station-location management. TrustedQSL (TQSL) signing tool's configuration describes country/entity-dependent fields and allowed values; a GUI wizard is not a protocol requirement.

Each QSO captures its operating location as a snapshot at logging time. Saved profiles synchronize, while current logging defaults are independent per client. Profile/default edits do not change historical signing inputs; correcting existing QSOs requires explicit editing. Profiles are optional for one-off locations, and no operating-session start/end lifecycle is required. Native station/location values and validation are defined in the [implemented model specification](superpowers/specs/2026-10-18-native-model-design.md). LoTW-specific signing-input mapping and validation remain unimplemented.

## What tqsllib supplies

LoTW submissions are digitally signed, not encrypted QSO payloads. Certificates use standard X.509 and key exports use PKCS#12. The inspected 2.8.6 signing function uses standard OpenSSL signature operations with Secure Hash Algorithm 1 (SHA-1); key generation uses Rivest–Shamir–Adleman (RSA) public-key algorithm. This is a protocol compatibility observation, not a choice to use these primitives for our own credential encryption.

The domain-specific work includes LoTW certificate extensions, station/contact signing field definitions, normalization, GAbbI record references and signature encoding, mode/band mappings, geographic data, certificate selection, and correction/duplicate semantics.

The released configuration includes LoTW 1.0 and 2.0 signature definitions. The published 2002 GAbbI specification is background; the pinned reference source and current configuration matter for current behavior.

## Prior library investigation

Unmodified tqsllib uses a filesystem-backed certificate/key store. Base64 import still populates that store. Directory, errors, and configuration/certificate caches include process-global state. Password callbacks supply passwords, not external signatures or replacement key-store implementations. No public custom-store/delegated-signing application programming interface (API) was found in the inspected release.

Direct foreign function interface (FFI) with one tenant per process and temporary stores materialized from managed credentials was explored. It was considered feasible for the existing library but is superseded by the Rust-port direction. It should not silently return as the selected architecture.

## Source and verification plan

Issue #4 implements the production pure-Rust certificate/archive import,
interpretation and signing/serialization components, with compatibility tests.
Later credential management and live submission integrate and extend that code.
A compatibility probe or alternate signing implementation intended for later
replacement does not satisfy the issue.

Remaining engineering preparation includes inventorying the reference functions and configuration sections needed for certificate import/interpretation, signing-input construction, signature generation, serialization, and transport. See the [initial source map](../references/trustedqsl/source-map.md).

Future compatibility verification should compare signing bytes and parsed records against the reference, verify signatures independently, and check actual LoTW acceptance and retained values. Gzip metadata or incidental formatting need not match byte-for-byte when protocol semantics do. Include corrections, ambiguous upload outcomes, multiple callsigns/entities, historical dates, portable locations, satellite and cross-band contacts, and invalid credentials.

No Rust LoTW implementation or live end-to-end signing/upload test has been performed. The previous speculative effort estimate was withdrawn; source-level scope must support any future estimate.

## Pure-Rust cryptographic backend (D224)

D224 selects pure-Rust certificate import, certificate/key interpretation and signing, rejecting the OpenSSL packaging/maintenance tradeoff. The [backend comparison](../references/lotw-cryptography.md) retains source findings and now follows that choice. No OpenSSL fallback or mixed native signing backend is proposed.

The working library composition is p12-keystore archive decoding, a pure-Rust certificate parser for required LoTW fields, and RustCrypto RSA/SHA-1 signing using the explicit blinded interface. This is an engineering starting direction, not a claim that exact crate versions/features were separately approved or that interoperability has been tested. Picky remains a researched library alternative for a demonstrated metadata/integration advantage within the pure-Rust boundary.

Pure-Rust source paths support modern and legacy archives. Preserve archive integrity checks, key/certificate matching, raw required metadata and protocol signature semantics. The RSA timing advisory remains subject to a documented, version-specific signing-path assessment; its decryption-padding focus is not proof that LoTW signing is exploitable or that it is automatically safe.

Import, certificate interpretation and signing remain server-side. Shared client/browser/domain code does not inherit server cryptography dependencies. Review optional/transitive features to avoid native verification backends within the selected LoTW subsystem. Unrelated client networking choices remain separate.

Exact library releases, feature compatibility and fixtures are engineering work; no Clacks LoTW implementation, private certificate inspection or LoTW compatibility test has started. Existing historical upstream notices apply to adapted source regardless of the runtime backend.

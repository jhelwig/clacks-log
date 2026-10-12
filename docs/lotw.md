# LoTW signing and certificate management

The accepted direction is a source-guided Rust port of relevant tqsllib behavior. Preserve LoTW interoperability while using explicit tenant context, managed credential storage, safe key access, durable publication state, and horizontally scalable execution.

## Scope of the port

Use the available reference implementation and configuration definitions to preserve certificate interpretation, signing eligibility, normalization, signature construction, and submission serialization. This is a source-guided port, not an attempt to infer an undocumented black box from outputs.

Do not reproduce filesystem stores, process-global errors/caches, GUI dependencies, incidental import return conventions, or automatic certificate deletion simply because upstream implements them that way.

Cryptographic primitives should come from established libraries. Most application code should be Rust. A Rust protocol implementation using OpenSSL remains a possible dependency choice; pure Rust cryptographic dependencies are not selected or ruled out.

## MVP certificate workflow

- Any connected client can initiate import of an existing certificate/private-key export, normally `.p12`.
- Import requires a connection to the server; offline certificate import is out of scope.
- Collect the export password when required. The server validates certificate identity and corresponding private key, then protects stored key material with server-managed secrets.
- The export password and ongoing credential protection are separate concerns. No accepted design requires retaining the user's export password.
- Display callsign, DXCC, certificate validity, eligible QSO dates, and signing readiness.
- Keep expired and superseded certificates visible with status; exclude them from signing.
- Certificate requests and renewals are later scope. Importing a replacement issued elsewhere remains part of import management.

The exact secret store, encryption/key hierarchy, authentication/authorization, audit policy, key-access boundary, export/deletion controls, and memory handling remain open. Server-managed secrets do not imply a plaintext file store or one global shared tenant secret.

## Import behaviors to design deliberately

The tqsllib reference identifies duplicate certificates by issuer and serial. PKCS#12 import writes a key before all certificate processing completes and may report a duplicate using a nonzero return. Its older-certificate check compares expiration for the same callsign without checking DXCC/QSO coverage. Key replacement matches public keys, not merely callsigns. Desktop cleanup separately removes expired/superseded certificates.

These are compatibility findings, not behavior we must copy into the product. The proposed product rules are harmless duplicate import after checking usability, clear addition/supersession results, no wholesale deletion by callsign, and rejected imports leaving existing usable credentials intact. Exact outcomes and overlap-selection rules need further design.

## Operating locations

The logger should provide its own station-location management. TQSL's configuration describes country/entity-dependent fields and allowed values; a GUI wizard is not a protocol requirement.

Each QSO captures its operating location as a snapshot at logging time. Saved profiles synchronize, while current logging defaults are independent per client. Profile/default edits do not change historical signing inputs; correcting existing QSOs requires explicit editing. Profiles are optional for one-off locations, and no operating-session start/end lifecycle is required. Canonical representation and field validation remain open.

## What tqsllib supplies

LoTW submissions are digitally signed, not encrypted QSO payloads. Certificates use standard X.509 and key exports use PKCS#12. The inspected 2.8.6 signing function uses standard OpenSSL signature operations with SHA-1; key generation uses RSA. This is a protocol compatibility observation, not a choice to use these primitives for our own credential encryption.

The domain-specific work includes LoTW certificate extensions, station/contact signing field definitions, normalization, GAbbI record references and signature encoding, mode/band mappings, geographic data, certificate selection, and correction/duplicate semantics.

The released configuration includes LoTW 1.0 and 2.0 signature definitions. The published 2002 GAbbI specification is background; the pinned reference source and current configuration matter for current behavior.

## Prior library investigation

Unmodified tqsllib uses a filesystem-backed certificate/key store. Base64 import still populates that store. Directory, errors, and configuration/certificate caches include process-global state. Password callbacks supply passwords, not external signatures or replacement key-store implementations. No public custom-store/delegated-signing API was found in the inspected release.

Direct FFI with one tenant per process and temporary stores materialized from managed credentials was explored. It was considered feasible for the existing library but is superseded by the Rust-port direction. It should not silently return as the selected architecture.

## Source and verification plan

The next planning task is to inventory the reference functions and configuration sections needed for certificate import/interpretation, signing-input construction, signature generation, serialization, and transport. See the [initial source map](../references/trustedqsl/source-map.md).

Future compatibility verification should compare signing bytes and parsed records against the reference, verify signatures independently, and check actual LoTW acceptance and retained values. Gzip metadata or incidental formatting need not match byte-for-byte when protocol semantics do. Include corrections, ambiguous upload outcomes, multiple callsigns/entities, historical dates, portable locations, satellite and cross-band contacts, and invalid credentials.

No Rust implementation or live end-to-end signing/upload test has been performed. The previous speculative effort estimate was withdrawn; source-level scope must support any future estimate.

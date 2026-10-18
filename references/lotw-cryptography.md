# LoTW cryptographic backend comparison

Checked 17 October 2026. This compares pure-Rust and OpenSSL-backed certificate import/inspection/signing against the accepted source-guided Rust port. D224 selects the pure-Rust direction and rejects the OpenSSL cost tradeoff. The modular library composition below is the engineering starting recommendation; exact crates/releases/features are not recorded as separate user-approved choices. No application code, dependency installation, private certificate inspection, runtime build, fixture execution or live provider submission was performed.

## Selected direction and working library composition (D224)

Use **pure-Rust libraries for the LoTW certificate/import/signing subsystem**. D224 rejects OpenSSL-backed Rust for this role because its native packaging and maintenance costs are not acceptable for the preferred direction. No OpenSSL fallback or mixed signing backend is proposed.

The working composition is `p12-keystore` for modern/legacy archive decoding, a pure-Rust certificate parser for LoTW-specific fields, and RustCrypto `rsa`/`sha1` for the required blinded signature operation. Use archive-level access and explicit public-key association rather than importing the keystore wrapper's alias/trust policy. Picky remains a researched alternative if a concrete certificate/attribute requirement favors its higher-level facilities; this is ordinary library selection, not permission to reopen the pure-Rust boundary.

Pure-Rust source paths support the needed legacy algorithms. Confirm released version compatibility, raw metadata access and signing-path security for the chosen set; no new cryptographic primitive or archive codec should be invented merely to keep application code in Rust. The research below distinguishes concrete capability findings from fixture results that have not been obtained.

These operations remain server-side. Keep native platform implementations out of this subsystem's dependency graph, including optional certificate-verification features that call native backends. This is scoped to the LoTW choice; client network-encryption and other unrelated library choices remain separate.

## What compatibility actually requires

The pinned [TrustedQSL certificate implementation](trustedqsl/upstream/src/openssl_cert.cpp) is the reference at commit `6ce824363e27da455c6cf6fd440b9c401945bfcb`:

- Its normal export path starts with Advanced Encryption Standard (AES)-256 encryption; its optional `weakCrypto` path uses SHA-1 with 40-bit RC2 certificate encryption and Triple Data Encryption Standard (Triple DES) private-key encryption. RC2 is a historical cipher name. Do not assume every export is legacy or that a legacy-only reader suffices.
- Import needs archive integrity verification, supported password encodings, private keys and certificates, bag identities/attributes, and correct public-key matching. An archive's friendly name or local key ID is not proof of a key match or trusted certificate.
- Certificate interpretation needs issuer/serial, validity, station callsign, DX Century Club (DXCC) entity and eligible radio-contact dates, including application-specific subject attributes/extensions. Generic certificate parsing does not implement LoTW eligibility.
- `tqsl_signDataBlock` uses Secure Hash Algorithm 1 (SHA-1) with the certificate's private key; the Rivest–Shamir–Adleman (RSA) key path requires the corresponding Public-Key Cryptography Standards #1 (PKCS#1) version 1.5 signature construction and digest identifier. Exact normalized input bytes and encoding remain the Rust port's responsibility.

Public-Key Cryptography Standards #12 (PKCS#12) is the archive format normally saved as `.p12`/`.pfx`. It uses symmetric password-based encryption; importing it does **not** require offering RSA ciphertext decryption. Signing and certificate-signature verification are also separate operations. These distinctions matter when assessing security advisories and selecting libraries.

## Candidate comparison

Capabilities in this table come from inspected source/documentation, not successful Clacks interoperability tests.

| Candidate | Import coverage | Inspection and signing | Packaging and maintenance judgment |
| --- | --- | --- | --- |
| **OpenSSL + Rust `openssl`** | Modern archives and legacy RC2/Triple DES with the appropriate provider; established password/format interoperability behavior. | Integrated X.509, private keys, trust/signature primitives and RSA/SHA-1 signing. LoTW rules still ours. | Native headers/libraries and provider configuration. Excluded from the selected LoTW direction under D224. |
| **`p12-keystore` + certificate parser + RustCrypto signer** | Source supports modern password encryption plus legacy RC2/Triple DES and archive integrity verification. Archive-level access avoids imposing its Java-style alias/chain policy. | Keeps certificate/key bytes available; separate parsing, trust/eligibility and signing integration. | Best modular pure-Rust option; simpler target packaging, but several crate families/versions and signer assurance to maintain. |
| **Picky with PKCS#12 support** | Modern/legacy encryption, nested bags, integrity checks and custom attribute access. | A broader Rust certificate/key/signature toolkit; currently depends on RustCrypto RSA. | Less low-level assembly than bare formats crates; broader dependency surface and release-candidate coordination in the inspected development line. Strong pure-Rust alternative, not an independent signer-security answer. |
| **`p12` + separate parser/signer** | Inspected reader handles SHA-1/RC2/Triple DES; its dispatch has no modern AES/password-scheme path. | Returns archive data; certificate interpretation and signing need other libraries. | Useful legacy-focused reader, unsuitable as Clacks's sole import path for current plus legacy exports. |
| **RustCrypto format/primitives composition** | `pkcs12`, `pkcs5`, certificate formats and cipher crates provide the required building blocks. | Explicit parsing, key linking, metadata and signing composition. | Flexible and native-library-free, but makes Clacks maintain more archive orchestration already supplied by the wrappers above. No case for starting at this level. |
| **Mixed Rust import + OpenSSL signing** | Rust reader handles imports; native backend signs. | Feasible standard key/certificate encodings connect them. | Retains OpenSSL packaging while maintaining two stacks. Not recommended without a demonstrated isolation, compatibility or other benefit. |

### Pure-Rust import: concrete findings

`p12-keystore`'s inspected upstream `pbes1.rs` dispatches the RC2 archive identifier to a five-byte RC2 key and Triple DES to a 24-byte key. Its confusing `PbeWithShaAnd40BitRc4Cbc` public name does not mean it implements RC4 for that path. The actual implementation settles the earlier documentation ambiguity. Its codec also supports modern password encryption and integrity checks. [Legacy implementation](https://raw.githubusercontent.com/ancwrd1/p12-keystore/master/src/pbes1.rs), [archive codec](https://raw.githubusercontent.com/ancwrd1/p12-keystore/master/src/codec.rs)

The `Pkcs12Archive` interface preserves decoded duplicate/unrelated certificate bags and leaves linking/trust decisions to the caller. Its comments explicitly say it does not verify certificate signatures, validity or key matching. Supported archive access is better suited to Clacks than automatically accepting its keystore alias/chain decisions. Custom bag attributes outside those exposed by this interface need separate handling if the reference import inventory establishes that they are required. [Archive interface](https://raw.githubusercontent.com/ancwrd1/p12-keystore/master/src/archive.rs)

Picky exposes custom attributes and underlying archive structures. Its source includes tests for RC2/Triple DES/SHA-1 and AES/SHA-256 combinations, with a Windows certificate-tool interoperability check when run on Windows. Those are upstream tests read during research, not tests executed here. This is substantive evidence of a pure-Rust compatibility path, not proof of every TrustedQSL export variant. [Picky archive model and tests](https://raw.githubusercontent.com/Devolutions/picky-rs/master/picky/src/pkcs12/mod.rs)

By contrast, the inspected `p12` algorithm dispatch implements the older schemes and returns no decryption result for other identifiers. This is a specific coverage limitation, not a criticism inferred from the crate's age or size. [p12 reader source](https://raw.githubusercontent.com/hjiayz/p12/master/src/lib.rs)

### Certificate interpretation is not automatic in either approach

Preserve raw certificate data and decode the required application-specific object identifiers. An object identifier identifies a certificate field or algorithm; it need not be pre-registered in a cryptographic library to be parsed by numeric value. Do not reject unfamiliar LoTW fields merely because a general library does not name them.

`x509-parser` provides pure-Rust certificate parsing, but its optional verification features use `ring` or AWS's cryptographic library, which are not a purely Rust implementation chain. A strict pure-Rust composition should use the parser without those features and explicitly select Rust verification primitives, or use suitable RustCrypto/Picky certificate facilities. Parsing is not equivalent to trust validation. [Parser feature boundaries](https://docs.rs/x509-parser/latest/x509_parser/), [RustCrypto generic extensions](https://raw.githubusercontent.com/RustCrypto/formats/master/x509-cert/src/ext.rs)

Both paths must use LoTW's intended trust/eligibility rules rather than web-server certificate rules or roots supplied uncritically by an uploaded archive. Both must compare public keys, handle duplicates/expired/superseded certificates, and keep malformed imports from replacing usable credentials. These are common costs, not extra work unique to pure Rust.

## Pure-Rust signing and the RSA advisory

RustCrypto `rsa` supports PKCS#1 v1.5 signatures, and its SHA-1 digest identifier can supply the required prefixed digest format. Use the normal digest-aware signing construction, not the unprefixed/raw signature operation. Its randomized signing interface supplies blinding: randomization of the private computation while preserving the protocol's resulting deterministic signature. Ordinary signing and prehashed signing calls in the inspected stable source do not all request blinding automatically. [Signature format and blinding](https://raw.githubusercontent.com/RustCrypto/RSA/v0.9.10/src/pkcs1v15.rs), [signing entry points](https://raw.githubusercontent.com/RustCrypto/RSA/v0.9.10/src/pkcs1v15/signing_key.rs)

RustSec still marks RUSTSEC-2023-0071 unpatched and warns against attacker-observable private-key timings. This is a real maintenance/assurance concern, not proof that Clacks's signing operation is exploitable. The current upstream tracking issue focuses on decryption padding, and the LoTW signing path does not call that depadding operation. Blinded signing is relevant mitigation, but neither that observation nor a closed modular-arithmetic issue establishes that the advisory is fully resolved for a chosen version. [Current advisory](https://rustsec.org/advisories/RUSTSEC-2023-0071.html), [maintainer tracking](https://github.com/RustCrypto/RSA/issues/626)

A pure-Rust signing selection would therefore require a documented, version-specific assessment of the signing path and observable timing, use of the blinded interface, and continuing review of the advisory. A blanket advisory suppression or the assertion that “signing only means safe” is not supported. Conversely, forcing the application to support RSA decryption to demonstrate a vulnerability would misrepresent the requirements. The inspected Picky development manifest pins the same RSA family, so it does not avoid this question. [Picky dependency selection](https://raw.githubusercontent.com/Devolutions/picky-rs/master/picky/Cargo.toml)

This concern requires a documented assessment and continued maintenance for the selected pure-Rust signing path; it does not reopen the rejected OpenSSL recommendation. OpenSSL is also subject to security fixes and must remain updated; using it does not certify Clacks's whole implementation as secure.

## Packaging and maintenance costs

| Cost | Pure-Rust composition | OpenSSL-backed Rust |
| --- | --- | --- |
| Client runtime | No native crypto library from this subsystem. | Also none: import/signing stays on the server. Other client dependencies need a separate feature audit. |
| Server build | Cargo/browser-independent Rust crypto build; no OpenSSL headers, discovery or native crypto linking. | Requires the selected native headers/library. Cross builds need matching target libraries or architecture-native container builds. |
| Developer platforms | Lower setup friction for local server tests on Windows/macOS/Linux. | Local server builds need platform OpenSSL setup; containerized server work can avoid host installation but is a workflow cost, not zero cost. Client-only builds must not inherit it through shared code/default dependency features. |
| Container runtime | Fewer native-library/provider artifacts. | Package matching runtime library/provider files and configuration; verify legacy imports in the final image. |
| Patching | Update affected Rust crates, rebuild and redeploy; monitor upstream composition and advisory status. | Update library/bindings as applicable and rebuild/redeploy the image. Dynamic linking does not update an already deployed immutable container automatically. |
| Version coordination | Parser/cipher/encoding/signature crate compatibility, optional features, and possible prereleases. | Bindings plus native version/provider compatibility and distribution support; not eliminated by Cargo. |
| Integration ownership | More explicit key association, certificate parsing and signer wiring; wrappers reduce this. | More integrated library surface, but wrapper coverage and LoTW-specific logic still need inspection. |

The Rust OpenSSL build documentation confirms that vendoring builds and statically links OpenSSL and requires a C compiler, Perl and Make; the vendored OpenSSL version can change without a breaking Rust-crate release. Do not characterize vendoring as removing native maintenance or assume it preserves an intended long-term-support branch. Those costs explain the rejected native path; no OpenSSL package/build mode is proposed for LoTW under D224. Static versus dynamic linking is a packaging choice, not a compatibility guarantee. [Build and discovery documentation](https://docs.rs/openssl/latest/openssl/)

OpenSSL's published policy lists 3.5 as a long-term-support line through 8 April 2030. It also lists older 3.0 as unsupported without extended support. That support lifecycle does not override D224’s pure-Rust choice. Distribution-specific security support can differ and must be checked for the actual image. [Release/support policy](https://openssl-library.org/policies/releasestrat/)

OpenSSL's legacy-provider support for RC2 archives is explicit, but it must be packaged/enabled deliberately. Archive integrity checks must stay enabled. Older password encodings and format quirks may have compatibility behavior not duplicated by strict Rust decoders. These are specific fixture cases to verify, not grounds to assume every malformed archive should be accepted. [Archive compatibility and provider documentation](https://docs.openssl.org/3.5/man1/openssl-pkcs12/)

### Version and licensing evidence limits

Inspected sources include `p12-keystore` docs labeled 0.3.2 and an upstream manifest labeled 0.4.1, Picky's 7.0.0-rc.27 development manifest (the unversioned docs page resolves to older 6.3.0), `p12` docs 0.6.3, RSA stable-tag 0.9.10 plus development sources, and current Rust OpenSSL docs 0.10.81. Do not mix features from these snapshots into a claim that one released locked set has been built and verified. The source-level comparison is sufficient to choose a direction; selecting a mutually compatible release set is implementation work.

The inspected core options use permissive MIT/Apache terms, compatible in principle with Elastic License 2.0 subject to upstream notices. Final transitive/feature inventories remain necessary. Existing historical OpenSSL notices embedded in adapted TrustedQSL source are independent of which runtime backend is used. See the [license review](licensing.md#projected-dependency-compatibility). `ring`, AWS-backed libraries and a system cryptographic API are not counted as pure-Rust alternatives merely because they have Rust interfaces; they retain native/assembly or platform dependencies and have not been selected as an additional backend.

## Decision and validation boundary

D224 selects pure Rust. Proceed with the modular composition described above, choosing actual released versions/features during engineering work. Use the explicit blinded signing interface and document the version-specific advisory assessment. Keep standard certificate/private-key encodings and framework-independent Rust values at subsystem boundaries. Do not build a runtime-pluggable crypto framework or an OpenSSL fallback.

Remaining validation is implementation acceptance: current/legacy exports; wrong/empty/non-ASCII passwords; archive integrity failures; duplicate and multiple bags; key/certificate mismatch; LoTW subject/extensions and validity; known signing bytes and independent verification; native-free resolved dependencies; supported server build targets; and actual LoTW acceptance. No private user export is needed for initial synthetic/reference fixtures. No estimated days, measured performance or tested interoperability is claimed by this research.

# TrustedQSL reference source map

The source-guided Rust port uses official TrustedQSL release **2.8.6**, tag `v2.8.6`, commit `6ce824363e27da455c6cf6fd440b9c401945bfcb`, as the initial reference. Selected upstream files are preserved unchanged under [upstream](upstream/).

Upstream repository: https://git.code.sf.net/p/trustedqsl/tqsl

Upstream browser: https://sourceforge.net/p/trustedqsl/tqsl/ci/6ce824363e27da455c6cf6fd440b9c401945bfcb/tree/

Reference capture date: 11 October 2026. This is a pinned baseline, not a promise that upstream rules will never change. Bundled `config.xml` is the configuration in this release, not necessarily the latest downloadable configuration.

GAbbI is TrustedQSL’s signed record format, containing linked certificate, station, and contact records. It is a named interchange format; the source function names below are literal identifiers.

## Initial behavioral map

| Responsibility | Reference location | Port boundary |
| --- | --- | --- |
| Certificate import | `src/openssl_cert.cpp`: `tqsl_importPKCS12File`, internal Public-Key Cryptography Standards #12 (PKCS#12) import logic | Decode TQSL exports, check archive integrity and match certificate/private keys. Replace file writes with explicit credential objects; leave issuer-chain acceptance to LoTW. |
| Certificate trust | `src/openssl_cert.cpp`: `tqsl_handle_root_cert`, `tqsl_handle_ca_cert`, `tqsl_handle_user_cert`; `apps/loadcertwiz.cpp`: `notifyImport` | Reference behavior only: TQSL installs archive roots and verifies chains against its local store. Clacks does not port this trust-store/chain-acceptance policy. |
| Certificate eligibility | `src/openssl_cert.cpp`: `tqsl_selectCertificates`, `tqsl_filter_cert_list`, `tqsl_isCertificateExpired`, `tqsl_isCertificateSuperceded` | Preserve protocol eligibility; design explicit status and selection results. |
| Import duplication and age checks | `src/openssl_cert.cpp`: `tqsl_store_cert`, `tqsl_replace_key` | Study identity/key matching; do not copy filesystem transactions, nonzero duplicate-result conventions, or callsign-only replacement assumptions blindly. |
| Station input and validation | `src/location.cpp`; `src/config.xml` field/page definitions | Extract validation and protocol mappings independently of the location-capture user interface (UI) sequence. |
| Signing definition selection | `src/location.cpp`: `make_sign_data`; `src/config.xml`: `sigspecs` | Source-derived ordered definitions, selecting the highest nondeprecated supported version; pinned selection is LoTW 2.0. Keep model normalization separate. |
| Signing primitive | `src/openssl_cert.cpp`: `tqsl_signDataBlock` | Implement compatible signing using an established crypto library and explicit key access. |
| GAbbI records | `src/location.cpp`: `tqsl_getGABBItCERT`, `tqsl_getGABBItSTATION`, `tqsl_getGABBItCONTACTData` | Preserve certificate/station/contact references and signature serialization. |
| Amateur Data Interchange Format (ADIF) field serialization | `src/adif.cpp` | Identify formatting needed for GAbbI; a general-purpose import parser is a separate scope question. |
| Mode and radio contact (QSO) conversion | `src/tqslconvert.cpp`; `src/config.xml` | Separate relevant normalization from file parsing and local duplicate-store assumptions. |
| Packaging and upload | `apps/tqsl.cpp`: `ConvertLogFile`, `UploadLogFile`, `UploadFile` | Extract compression, Hypertext Transfer Protocol (HTTP) request, response, and outcome behavior; use our own publication jobs. |
| Desktop certificate cleanup | `apps/tqsl.cpp`: `cert_cleanup` | Reference only; do not copy automatic deletion of expired/superseded certificates. |

This is an initial map, not a complete dependency inventory or implementation plan. Additional upstream files remain available at the pinned commit.

## Supporting sources

- [American Radio Relay League (ARRL) developer submission guidance](https://lotw.arrl.org/lotw-help/developer-submit-qsos/)
- [ARRL confirmation and acceptance queries](https://lotw.arrl.org/lotw-help/developer-query-qsos-qsls/)
- [GAbbI specification](https://www.rickmurphy.net/trustedqsl.org/Trusted_GAbbI.pdf), dated 2002; historical specification, not the sole source for current behavior.
- [Certificate management](https://lotw.arrl.org/lotw-help/managing-callsign-certificates/)
- [Selected pure-Rust dependencies and trust boundary](../lotw-cryptography.md).
- [Signing, wire and certificate compatibility details](../lotw-cryptography.md#signature-and-wire-compatibility).

## Licensing

Included files retain their original notices. [Upstream root license](upstream/LICENSE.txt), [library license](upstream/src/LICENSE), and [upstream contributors](upstream/AUTHORS.txt) accompany the snapshot. The upstream license applies to these files; D221 selects Elastic License 2.0 for Clacks’s own application code; that does not replace upstream terms. Porting source requires preserving applicable notices and attribution. In particular, `upstream/src/openssl_cert.cpp` contains copied historical OpenSSL application code with additional OpenSSL/SSLeay notices and attribution/advertising conditions; the main TrustedQSL license does not replace them. See the [projected dependency compatibility review](../licensing.md#projected-dependency-compatibility). A modern OpenSSL library license does not retroactively replace those historical source terms.

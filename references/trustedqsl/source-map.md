# TrustedQSL reference source map

The source-guided Rust port uses official TrustedQSL release **2.8.6**, tag `v2.8.6`, commit `6ce824363e27da455c6cf6fd440b9c401945bfcb`, as the initial reference. Selected upstream files are preserved unchanged under [upstream](upstream/).

Upstream repository: https://git.code.sf.net/p/trustedqsl/tqsl

Upstream browser: https://sourceforge.net/p/trustedqsl/tqsl/ci/6ce824363e27da455c6cf6fd440b9c401945bfcb/tree/

Reference capture date: 11 October 2026. This is a pinned baseline, not a promise that upstream rules will never change. Bundled `config.xml` is the configuration in this release, not necessarily the latest downloadable configuration.

## Initial behavioral map

| Responsibility | Reference location | Port boundary |
| --- | --- | --- |
| Certificate import | `src/openssl_cert.cpp`: `tqsl_importPKCS12File`, internal PKCS#12 import logic | Parse exports and match identities/keys; replace file writes with explicit credential objects and managed storage. |
| Certificate eligibility | `src/openssl_cert.cpp`: `tqsl_selectCertificates`, `tqsl_filter_cert_list`, `tqsl_isCertificateExpired`, `tqsl_isCertificateSuperceded` | Preserve protocol eligibility; design explicit status and selection results. |
| Import duplication and age checks | `src/openssl_cert.cpp`: `tqsl_store_cert`, `tqsl_replace_key` | Study identity/key matching; do not copy filesystem transactions, nonzero duplicate-result conventions, or callsign-only replacement assumptions blindly. |
| Station input and validation | `src/location.cpp`; `src/config.xml` field/page definitions | Extract validation and protocol mappings independently of the location-capture UI sequence. |
| Signing definition selection | `src/location.cpp`: `make_sign_data`; `src/config.xml`: `sigspecs` | Preserve supported signing definitions and normalization. |
| Signing primitive | `src/openssl_cert.cpp`: `tqsl_signDataBlock` | Implement compatible signing using an established crypto library and explicit key access. |
| GAbbI records | `src/location.cpp`: `tqsl_getGABBItCERT`, `tqsl_getGABBItSTATION`, `tqsl_getGABBItCONTACTData` | Preserve certificate/station/contact references and signature serialization. |
| ADIF field serialization | `src/adif.cpp` | Identify formatting needed for GAbbI; a general-purpose import parser is a separate scope question. |
| Mode and QSO conversion | `src/tqslconvert.cpp`; `src/config.xml` | Separate relevant normalization from file parsing and local duplicate-store assumptions. |
| Packaging and upload | `apps/tqsl.cpp`: `ConvertLogFile`, `UploadLogFile`, `UploadFile` | Extract compression, HTTP request, response, and outcome behavior; use our own publication jobs. |
| Desktop certificate cleanup | `apps/tqsl.cpp`: `cert_cleanup` | Reference only; do not copy automatic deletion of expired/superseded certificates. |

This is an initial map, not a complete dependency inventory or implementation plan. Additional upstream files remain available at the pinned commit.

## Supporting sources

- [ARRL developer submission guidance](https://lotw.arrl.org/lotw-help/developer-submit-qsos/)
- [ARRL confirmation and acceptance queries](https://lotw.arrl.org/lotw-help/developer-query-qsos-qsls/)
- [GAbbI specification](https://www.rickmurphy.net/trustedqsl.org/Trusted_GAbbI.pdf), dated 2002; historical specification, not the sole source for current behavior.
- [Certificate management](https://lotw.arrl.org/lotw-help/managing-callsign-certificates/)
- [Rust OpenSSL PKCS#12 API](https://docs.rs/openssl/latest/openssl/pkcs12/struct.Pkcs12.html), one potential dependency, not selected.

## Licensing

Included files retain their original notices. [Upstream root license](upstream/LICENSE.txt), [library license](upstream/src/LICENSE), and [upstream contributors](upstream/AUTHORS.txt) accompany the snapshot. The upstream license applies to these files; the project license remains unselected. Porting source requires preserving applicable notices and attribution.

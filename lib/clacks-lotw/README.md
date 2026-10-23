# LoTW signing library

Production server-side certificate and signing components for the
[LoTW specification](../../docs/lotw-signing.md). Logbook of The World is LoTW.

`certificate::inspect_certificate` interprets public station metadata from the
original DER certificate without asserting issuer trust or private-key possession.
DER is Distinguished Encoding Rules. Coverage dates, certificate validity, issuer
and the complete serial are retained; duplicate/missing required metadata fails.

[Independent fixture provenance](tests/fixtures/README.md) records synthetic inputs.
The supplied TQSL sources are read-only references and are never compiled, linked
or executed by this project or its test tooling. Test certificates are synthetic; they do not establish ARRL
issuance or live acceptance. Public metadata types do not expose private keys.

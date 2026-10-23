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

`certificate::import_pkcs12` requires archive integrity and one unambiguous issued
station certificate with a validated, actually matching RSA private key. Supporting
CA certificates grant no trust. Friendly names and local key IDs cannot override
key matching. Repeated identical bags are deduplicated. Expired certificates may
be imported; eligibility is evaluated separately.

The tested RSA range is 1024–4096 bits. Integer encodings are bounded before
private-key decoding, which validates the private components. The archive
dependency caps MAC/derivation iterations at
1,000,000 and scrypt memory/parallelism at its documented limits; server request
budgets and concurrency limits belong to the later adapter. Empty and BMP Unicode
passwords are supported; non-BMP passwords are rejected without rewriting them.
An integrity failure alone does not distinguish a wrong password from corruption.
For legacy encryption, the locked decoder interprets zero and negative declared
iteration counts as one derivation round. Regression cases retain valid positive
integrity counts and check that a wrong password still fails; this does not
relax the integrity-count requirement.

Credentials expose no cloning, serialization or raw-key export. RSA erases private
components on drop; the archive dependency owns temporary decrypted byte allocations
without a complete erasure guarantee. This library does not claim all copies are
zeroized, and never retains the archive password. Persistent encryption is separate.

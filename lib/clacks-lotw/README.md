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

Raw certificate callsigns are checked before native normalization: non-ASCII
and embedded controls fail; ASCII case and boundary whitespace are normalized.
Non-positive integrity iteration counts are rejected even with a matching MAC.
For legacy encryption, the locked decoder interprets zero and negative declared
iteration counts as one derivation round. Regression cases retain valid positive
integrity counts and check that a wrong password still fails; this does not
relax the integrity-count requirement.

The tested RSA range is 1024–4096 bits. Integer encodings are bounded before
private-key decoding, which validates the private components. The archive
dependency caps MAC/derivation iterations at
1,000,000 and scrypt memory/parallelism at its documented limits; server request
budgets and concurrency limits belong to the later adapter. Empty and BMP Unicode
passwords are supported; non-BMP passwords are rejected without rewriting them.
An integrity failure alone does not distinguish a wrong password from corruption.

Credentials expose no cloning, serialization or raw-key export. RSA erases private
components on drop; the archive dependency owns temporary decrypted byte allocations
without a complete erasure guarantee. This library does not claim all copies are
zeroized, and never retains the archive password. Persistent encryption is separate.

## Preparing an artifact

Call `certificate::eligibility::eligibility` with the selected public identity,
QSO date, explicit credential state and current time. Coverage dates are inclusive;
expiration is evaluated by UTC day. The library does not choose among certificates
or infer supersession. Later storage/routing adapters supply current availability.

Prepare `LotwSubmission` through `clacks_model::service::lotw::project` with the
current QSO, destination, account definitions and public signing identity. Retain
that frozen projection and external key for submission evidence. Pass it and the
selected credential to `signing::write_batch` with
`signing::definition::SignatureDefinitions::bundled()`, truthful
`ProducerInformation` and an explicit evaluation time. `BatchItem` carries each
submission/credential/state; `BatchSummary` reports certificate, station, contact
and byte counts only after all writes succeed. The caller must recheck latest
state and routing before dispatching; construction does not claim service success.

Use a caller-owned `Vec<u8>` or staging file as the standard `Write` sink. A
compression wrapper is also supported, but must be finalized successfully after
`write_batch` returns. Only dispatch the complete finalized artifact. Discard all
staged bytes on error. The serializer uses ordinary `write_all`, supports short and
interrupted writes, and neither flushes nor closes the sink. Failures preserve
concrete I/O and cryptographic causes through `anyhow::Error`; local validation
errors are downcastable types in `error`. No upload, durable acceptance or service
evidence mutation occurs in this library.

The signing definition is data-driven, choosing the highest nondeprecated numeric
version. Unsupported selected versions and incompatible mapping provenance fail;
there is no downgrade. Signing input concatenates ordered station values followed
by contact values, trims boundary ASCII whitespace and uppercases the complete
input. Printable ASCII is the tested text boundary; other Unicode and embedded
controls fail explicitly. This is a library support limit, not a claim that LoTW
universally prohibits Unicode. Wire fields are length-framed, ordinary value
fields have stable lexical ordering, and signature order comes from definitions.
Date/time are formatted without losing seconds; frequency spelling is retained.
Certificate DER is unchanged, base64 is wrapped at 64 columns, and certificate and
station references are local to each output batch.

## Instrumentation and maintenance

Debug operation spans use `#[instrument(skip_all)]` with default module targets.
Focused events describe checked material, local rejection/readiness reasons and
constructed artifact counts. They exclude credentials, certificates, callsigns and
contact values. Callers supply request/job context. Libraries install no global
subscriber, add no workflow metrics and make no upload-success claim; runtime
adapters own durable management and submission rates, latency and operational
budgets. Events remain useful when operation spans are filtered.

The locked dependency/feature and license assessment is in
[licensing](../../references/licensing.md#locked-lotw-library-dependencies).
RSA 0.9.10 remains affected by RUSTSEC-2023-0071. The signing path uses digest-aware
PKCS#1 v1.5 with SHA-1 and explicit randomized RSA blinding, seeded fallibly from
operating-system entropy; it provides no ciphertext decryption operation. Blinding
is mitigation, not resolution of the advisory. The archive decoder also performs
private-key validation on supplied material. Import admission/concurrency limits
and restricted server-side access remain adapter obligations. Maintain the pinned
versions and review security updates rather than suppressing the advisory.

Tests consume committed independently constructed archives/certificates and
independent cryptographic known answers. Archive coverage includes SHA-256 and
legacy SHA-1 integrity, both certificate/key safe orders, and bounded invalid
private components. They check literal source-derived
signing bytes, wire fields, producer framing, all three tested key sizes, batch
membership, failure behavior and the real native-model projection. The dedicated Rust `clacks-lotw-fixtures` command reconstructs every input from
fixed public test recipes and verifies exact committed bytes. It does not call
this library's importer, signer or serializer. A separate big-integer backend
checks signatures against explicit padding; standard cryptographic primitives
are shared. The `clacks-lotw-data` maintenance command only reads supplied TQSL
reference text. These
checks establish
local compatibility evidence; live LoTW acceptance requires later provider tests.

The compatibility suite runs with Rust 1.99.1 on macOS ARM64 and native Linux
ARM64 in a Rust-only container. Windows x86-64 and Linux x86-64 compile checks
cover the library, with Windows also checking workspace test targets; they do
not establish runtime/installer behavior on those targets. The workspace suite,
doctests, warning-denied Clippy and rustdoc checks run on the development host.
The advisory scan reports RUSTSEC-2023-0071 without an exception or suppression.

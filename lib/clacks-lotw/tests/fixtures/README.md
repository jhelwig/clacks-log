# Independent synthetic certificate inputs

These 66 committed certificate, archive, signature and wire inputs are constructed
by the dedicated `clacks-lotw-fixtures` Rust command. Construction starts with
fixed public test recipes, never existing fixture bytes, and does not call
Clacks's production importer, signer or serializer. It needs no external programs.
None is a real user credential. All keys, passwords, seeds, salts and initialization
vectors are public test data and must never be used outside tests.

From the checkout, using Rust 1.99.1 and the committed lockfile:

```sh
cargo +1.99.1 run --release --bin clacks-lotw-fixtures --locked --offline -- --help
cargo +1.99.1 run --release --bin clacks-lotw-fixtures --locked --offline -- --check
cargo +1.99.1 run --release --bin clacks-lotw-fixtures --locked --offline -- --write
```

No arguments displays help. Only `--write` changes files. `--check` reconstructs
all bytes and rejects missing, changed or unowned fixture data; this README is
excluded. The optimized build keeps 4096-bit key generation practical. Dependencies
must be cached for offline use. Recipes live in
[`clacks-reference-data::fixtures`](../../../clacks-reference-data/src/fixtures.rs),
behind the `fixtures` feature enabled by the dedicated binary. Exact crate
versions are recorded in the workspace lockfile. Non-trivial fixture generation
must use these Rust tools; external or uncommitted Python generators are prohibited.

The supplied TrustedQSL (TQSL) sources are read-only reference material. They must
never be compiled, linked, executed or wrapped, including for test generation.
These fixtures are not outputs obtained by running those sources.

## Certificate construction

- Station RSA keys: 1024, 2048 and 4096 bits; exponent 65537. RSA is Rivest–Shamir–Adleman.
  RustCrypto key generation uses ChaCha20 seeds `[11; 32]`, `[12; 32]` and
  `[13; 32]`; the separate 1024-bit issuer uses `[10; 32]`. Fixed blinding seeds
  are `[201; 32]` for contact signatures and `[202; 32]` for certificate signatures.
- Subject organization: `Clacks Synthetic Test CA`; unit: `Non-production fixtures`;
  email: `test@example.invalid`; common name: `Not the station callsign`.
- Station subject attribute `1.3.6.1.4.1.12348.1.1`: `N0CALL`.
- Raw extension bytes at `.1.2`, `.1.3`, `.1.4` under the same enterprise prefix:
  `2000-01-01`, `2029-12-31`, `291`, respectively. They are not nested encoded strings.
- Certificate validity: `2020-01-01T00:00:00Z` through `2030-06-15T12:00:00Z`.
- Full serial: `1234567890abcdef1234567890abcdef` (hexadecimal).
- Certificates are signed with a separate synthetic root; issuer trust is outside
  metadata inspection. The elliptic-curve input exercises an unsupported key type.

Metadata-negative cases omit the callsign or coverage, duplicate the callsign or
coverage, reverse coverage dates, supply February 30 or append junk to the entity.
Every generated certificate carries a synthetic issuer signature, including
malformed metadata cases; local parsing does not establish issuer-chain trust. Expected metadata is literal
fixture input, not derived using the production parser.

Raw callsign archive cases encode `N0CAß`, `N0CAØ`, `N0CALL\nINJECT` and
` \t n0call\r\n` before native normalization. Duplicate required metadata is also
wrapped in archives for actual-import coverage.

## Archive construction

Normal archives use AES-256-CBC encryption for both key and certificate material,
with 2048-round password derivation, the fixed salt below and `[4; 16]` as the
initialization vector.
Legacy archives use SHA-1-based password derivation with triple-DES for keys and
40-bit RC2 for certificates, plus SHA-1 integrity. Modern archives use SHA-256
integrity. Normal and legacy key-size fixtures put the encrypted certificate
safe before the key safe, matching the reference export order.
`normal-key-first.p12` and `legacy-key-first.p12` reverse that order to exercise
selection independently of bag placement. SHA-1 and SHA-256 are Secure Hash
Algorithm digest variants. AES is Advanced Encryption Standard; DES is Data
Encryption Standard; RC2 is the algorithm's conventional name. Standard archive
parameters are encoded in each committed input rather than inferred from aliases.
The default password is `fixture-password`; empty/BMP cases use `""` and
`test-café`. BMP means Basic Multilingual Plane. Separate encrypted empty/BMP
archives exercise decryption as well as password-dependent integrity checks.

Plain-bag fixtures contain a PKCS#8 key bag and X.509 certificate bags in an
AuthenticatedSafe data object, with an SHA-256 integrity MAC, 2048 iterations and
salt `independent-clacks-fixture`. PKCS means Public-Key Cryptography Standards;
MAC means message authentication code. The password is encoded as UTF-16
big-endian with a terminating zero code unit for PKCS#12 derivation. Aliases and
local key IDs are deliberately misleading in their named cases. Byte-identical
bags are repeated in the duplicate case. Missing, mismatched and ambiguous
material have their corresponding bag omitted, replaced or appended.

Boundary cases declare 1,000,001 integrity/modern/legacy derivation iterations;
request scrypt cost 2^21, block size 8, parallelism 1 (excessive memory), or cost
16, block size 8, parallelism 17; and encode a modulus/private exponent of 2^4096
before RSA construction. The iteration-zero/negative mismatch cases retain their
original 2048-round integrity digest. Separate `mac-one-round-*` archives contain
a correct one-round digest and declarations 1, 0 or -1, isolating malformed
parameter validation from MAC mismatch.
`legacy-one-round-*` archives declare encryption counts 1, 0 or -1 for both the
key and certificate safe, over ciphertext explicitly constructed with one round;
their SHA-1 integrity MACs declare and use 2048 rounds. The locked importer accepts
all three, characterizing its one-round treatment of nonpositive legacy
encryption counts without relaxing positive-MAC enforcement.
`invalid-private-exponent.p12` encodes the bounded private exponent 1 with an
otherwise unchanged key and a valid integrity MAC; private-key decoding must
reject the invalid components. `supporting-ca.p12` includes a certificate
with BasicConstraints CA=true. Unsupported encryption uses algorithm identifier
`1.2.3.4.5` with a recomputed valid archive MAC.

## Independent signing and wire evidence

`signing-input.txt` is the hand-authored literal station-then-contact input:

```text
5FN318HARTFORDCT20MK1ABC14.074000FT82026-10-2212:34:56Z
```

It contains no terminating newline. Each `known-signature.bin` is constructed with
RustCrypto's PKCS#1 v1.5/SHA-1 primitive over those exact bytes, using its directory's
synthetic key. Cryptographic primitives come from the same trusted dependencies
as application signing. Fixture construction does not call Clacks's importer,
signer or serializer; the independent expectations describe the protocol input,
key identity and wire records. Tests verify Clacks-produced signatures against
the expected message and public key through the RSA dependency's public API.
There is no second implementation of RSA exponentiation or padding validation.

`single-contact.gabbi` pins an entire independently authored batch: literal
Clacks producer header, lexical value-field order, exact lengths, base64 wrapped
at 64 columns with final newlines, certificate/station/contact records and
boundaries. Its certificate/signature bytes come from the independent inputs.
Multi-record tests separately check each contact-to-station and
station-to-certificate relationship. The test-only length parser and real native
projection checks do not introduce a second production normalization path.

Tests validate these inputs with Rust alone. They establish local compatibility
cases, not ARRL issuance, executed-TQSL interoperability or live LoTW acceptance.
Adding or changing fixtures requires updating the Rust recipes and separately
reviewing literal expectations against the documented protocol. Source extraction
is maintained by the separate `clacks-lotw-data` command; it never generates
cryptographic fixtures.

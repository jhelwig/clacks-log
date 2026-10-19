# Clacks synchronization core

Production metadata and conditional transitions for the account-wide synchronization
contract. The library consumes `clacks-model` native values and returns rows and
preconditions for real database transactions. It contains no database, transport,
simulated network or alternative record schema.

- `client`: durable working/original values, frozen versions, retries and review.
- `acceptance`: client admission, server-origin acceptance, native reference checks,
  journal rows and atomic write conditions.
- `transfer`: immutable full/incremental manifests, bounded installation progress
  and complete checkpoint validation.
- `membership` and `compaction`: explicit revocation, oldest-authorized acknowledgment,
  original-value witnesses and closed-actor projection.
- `lineage`, `compare` and `recovery`: position-dependent ancestry, baseline-aware
  comparison and retained accepted-state recovery through the server.

Use the owning modules directly. Composite rows are validated at transition
boundaries; scalar containers validate during deserialization. Persist ready
client counters and proposals together before transmission. Persist accepted
records, local comparison evidence and progress together before advancing a
checkpoint. Send acknowledgment only from a checkpoint reloaded after commit.

[Transaction and integration contract](../../docs/synchronization-core.md) describes
the adapter obligations and recovery limits. Rust 1.99.1 is required. Run
`cargo test --workspace --locked --offline` from the repository root.

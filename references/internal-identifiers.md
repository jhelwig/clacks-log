# Internal identifier direction

D119 allows duplicate logbook names with independent internal identity and distinguishing user interface (UI) context. D120 records the user's recommendation of identifiers such as ULIDs throughout the system. Exact format/library is not selected.

[Canonical Universally Unique Lexicographically Sortable Identifier (ULID) specification](https://github.com/ulid/spec), checked 2026-10-12: ULIDs contain a 48-bit millisecond timestamp and 80 bits of randomness, with a canonical 26-character encoding and lexical sortability. The format can be generated on an offline client without a central sequence allocator (design inference from its construction). Local monotonic generation is an option; it does not establish distributed causal ordering.

Entity identifiers and synchronization revision evidence have separate jobs. A radio contact (QSO)/logbook/profile/equipment identity remains stable through edits; revisions must still identify accepted state and conflicting descendants under D04/D58-D60. Device time embedded in an identifier does not select last-writer-wins, replace ancestry/common-state evidence, or determine QSO contact time. External provider IDs remain provider-scoped associations rather than replacements for local entity identity.

Pending implementation choices: exact format/encoding, Rust implementation, secure randomness, clock behavior, and revision identity/history representation. No library has been installed, and no application code is implemented.

# Open questions

These questions remain unselected as of 12 October 2026. They are planning topics, not authorization to start implementation or scope promises for the first release. The most recent discussion checkpoint is in [the planning handoff](handoff.md).

## Product and release scope

- Choose source-available license terms and how the optional hosted subscription is packaged.
- Define ordinary logger MVP functions, migration from LOG4OM, ADIF preservation, import duplicate handling, backup/recovery, and export requirements.
- Specify the Worked Before interaction and how GridTracker selections reach it.
- Verify GridTracker 2 external wanted/award-data ingestion before considering that future integration.
- Decide later eQSL scope and cross-platform intentions beyond the initial Windows environment.

## Client and server architecture

- Select native desktop tooling, client persistence, server persistence, identifiers, revisions, and synchronization transport.
- Define enrollment and authorization for arbitrary client counts and hosted tenants.
- Specify initial sync, conflict comparison data, field-resolution UX, deletions/tombstone retention, duplicate QSOs, and disaster recovery.
- Define how service-originated field changes participate in record conflicts.
- Define remaining saved-profile fields, operator persistence, and profile-edit conflicts, plus the always-visible state indication and explicit reload interaction using current defaults, last-loaded profile values, and latest saved profile values. Station identity is part of the profile; exact editor layout is deferred.
- Define empty-versus-absent incoming fields and routing. Supplied fields take precedence when present; defaults fill missing operating context. Discrepancy notices are omitted for now, and automatic missed-contact catch-up is outside MVP.
- Define observability and recovery for pending local edits and integration failures.

## External services

- Verify exact LoTW/QRZ correction, query, matching, and unmatched-download semantics.
- Separate our publication history from counterpart confirmation/contact data and local record enrichment.
- Define enrichment replacement policy and whether/how users may create unmatched local records.
- Define upload field sets, accepted/confirmed status representation, per-service revision tracking, retries/backoff, and ambiguous submission outcomes.
- Specify scheduling at startup and coordinated provider rate limits across tenants. Recheck provider recommendations without inventing numeric mandates.

## Rust LoTW port

- Complete the source-level behavioral inventory before implementation estimates.
- Identify required certificate extensions, trusted issuers, eligibility/status checks, and overlapping certificate selection rules.
- Choose cryptographic/PKCS#12 parsing dependencies and compatibility policy for older exports.
- Define the canonical representation and validation of per-QSO operating-location snapshots and saved profiles.
- Define official configuration ingestion, versioning, updates, validation, and unsupported-definition handling.
- Design credential encryption, key access, authorization, retention, deletion/export, audit, and memory handling independently of tqsllib.
- Define the boundary between protocol generation, signing, secret storage, and network submission.
- Prepare representative fixtures and a conformance strategy using the reference implementation; live validation will require suitable operator credentials and real intended contacts.

## Discussion boundaries

Remain in planning until implementation is requested. Finish feasibility and design work without prematurely jumping to a prototype. Preserve meaningful decisions and corrections as they occur; ask focused questions when a choice is needed rather than introducing unstated assumptions.

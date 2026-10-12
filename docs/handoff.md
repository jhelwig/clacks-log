# Resume Clacks Log planning

Planning checkpoint: **12 October 2026**. Project owner: Jacob Helwig (`jhelwig`). Repository: https://github.com/jhelwig/clacks-log . The working name is **Clacks Log**.

The project remains in interactive product and architecture planning. No Rust application, database schema, desktop toolkit, sync protocol, cryptographic dependency, implementation plan, or production migration has been selected or started. Reference C++ files are upstream research material, not Clacks Log implementation.

## Read first

1. [Product and workflows](product.md): actual operator routine and scope.
2. [Decision register](decisions.md): D01-D30 and superseded approaches.
3. [Synchronization](synchronization.md), [integrations](integrations.md), and [LoTW](lotw.md).
4. [Open questions](open-questions.md): unresolved topics without implied release commitments.
5. [Research provenance](../references/README.md): original documents and source findings.

All paths here are repository-relative. The repository contains the supplied originals, an extracted research brief with hyperlinks, and a pinned TrustedQSL source subset with licenses. Continuation does not require the previous agent's temporary checkout, Downloads directory, memory files, or access to the original conversation share.

## Essential established direction

Clacks Log is a focused native graphical logger, preserving specialist operating applications and useful desktop context. Rust is preferred where practical. Source-available licensing is being considered; no exact license is selected. Sync must be self-hostable, with an optional hosted subscription offering.

Any number of clients keep independently writable offline logs. Server conflict detection is record-level; conflicting pushes are rejected and remain pending locally. Resolution is field-level, with deletion treated as an edit. Do not invent a server-side collection of unresolved competing records or silent last-writer-wins behavior.

The server owns LoTW and QRZ jobs; both integrations are MVP. Periodic uploads use a five-minute cooldown after edits, with configurable upload interval and a manual action that bypasses cooldown. Download interval defaults to a configurable 30 minutes with a manual trigger. Publication acceptance and remote confirmation are distinct, as are our submission history and counterpart confirmation data. Enrichment/replacement policy is not settled.

Each QSO captures its operating context at logging time. Later default/profile changes do not rewrite it. Bulk editing is the intended way to apply repeated corrections; detailed MVP scope is not yet selected. Location profiles are optional, synchronize, and contain station identity. Current defaults are client-local, with one operating context per client. Multiple operators use separate clients. Operator identity must be easy to set/clear; whether saved profiles include an operator remains unspecified.

There is no required operating-session lifecycle.  The editor should not permanently consume desktop space, but profile synchronization/edited state must always be visible. Reload is explicit. Compare defaults with the latest saved profile and retain the last-loaded revision to distinguish local adjustments from saved-profile changes. Exact layout is deferred.

LoTW is a source-guided port of relevant tqsllib behavior, using established cryptographic primitives and our own security/storage model. The chosen direction supersedes direct FFI and temporary filesystem-store proposals. MVP imports existing `.p12` credentials from any connected client; no offline import, certificate requests, or renewals. Stored keys use server-managed protection. Expired/superseded certificates remain visible and are excluded from signing. The earlier 4-8 person-week estimate was withdrawn; no replacement estimate is accepted.

Worked Before uses a UTC calendar-day indicator but retains earlier history and available grid/POTA references. Heard-station views and award hunting/tracking, including ARRL/JARL interests, are later scope. eQSL is relevant but not committed to MVP. LOG4OM's existing service schedules are observations, not requirements.

## Exact conversation checkpoint

The latest discussion concerned WS Suite completed-QSO capture:

- Prefer supplied fields when present; current defaults fill missing operating context.
- Do nothing for now about discrepancies between supplied fields and defaults. No notices or mandatory reconciliation workflow.
- **Automatic catch-up from WS's ADIF file is outside MVP.** This does not weaken durability/sync requirements for contacts already captured by Clacks Log.
- Manual ADIF import/migration scope is still undecided.
- Primary UDP type 5 and type 12 represent one completed contact. Type 12 wraps ADIF in the primary binary protocol. The secondary ADIF checkbox enables a separate plain-ADIF broadcast.
- The disk log and UDP ADIF share the generated record; UDP does not inherently supply extra enrichment absent from it.
- Logged power is optional text from Log QSO, not CAT telemetry in the inspected WS build. Jacob's current file contains core contact fields, station callsign, own grid, and sometimes remote STATE, rather than the full optional-field set.

See [WS Suite source findings](../references/ws-suite-udp.md). WS 3.2.1 build 261010 source was inspected; no live packets or installed settings were checked. Type 12 as the preferred ingestion message remains a proposal, not a decision. Empty-versus-absent field semantics, routing, repeated-message deduplication, and WS/GT2 selection integration remain open.

## Continuing the conversation

The last explicit scope decision was excluding automatic catch-up. **No next brainstorming topic was selected.** Resume with that checkpoint and one substantive planning question; do not assume agreement to implementation, manual-import scope, or a proposed UI. The open-question list offers topics such as basic logger/migration scope, service-data reconciliation, and the detailed source-guided port inventory.

Jacob wants evidence-backed planning with stated assumptions. Avoid repetitive questions about incidental UI details, conflating observations with requirements, fixed two-client wording, and premature prototype suggestions. Answer a direct question before introducing another topic. Documents are a maintained handoff, not a verbatim transcript or a claim that all design is finished.

# Decision register

This register records explicit decisions from the planning conversation through 12 October 2026. Unselected implementation details and historical alternatives remain separate from commitments.

## Accepted decisions

| ID | Decision |
| --- | --- |
| D01 | Working project name is Clacks Log. |
| D02 | Support an arbitrary number of independently writable clients, including offline operation.  |
| D03 | Client reconciliation uses a self-hostable synchronization server. An optional hosted subscription service is part of the product direction. |
| D04 | Detect conflicts at the record level. Reject conflicting pushes until resolved; retain the pending edit locally. Resolve conflicts at the field level. |
| D05 | Treat deletion as an edit for conflict detection and resolution. |
| D06 | Preserve specialist application workflows and substantial operating context. Avoid an all-in-one shack application and a simplified web-app-in-a-desktop-window interface. |
| D07 | Prefer Rust wherever practical. |
| D08 | LoTW and QRZ are MVP integrations; heard-station views and award tracking/hunting are later scope. |
| D09 | Both services publish periodically after a cooldown. Working defaults recorded in discussion are a one-minute upload check and five minutes since the last edit; upload interval is configurable. An edit resets the cooldown. |
| D10 | Upload pending now bypasses the cooldown. It still operates on accepted server state. |
| D11 | Downloads default to a configurable 30-minute interval and support a manual trigger. |
| D12 | The synchronization server owns external service upload/download execution. |
| D13 | LoTW behavior will be ported to Rust using tqsllib source as the reference. Preserve protocol compatibility, not its filesystem/global-state/GUI assumptions. |
| D14 | MVP supports importing existing certificate/private-key exports. Certificate requests and renewals are deferred. |
| D15 | Any connected client can initiate certificate import. Offline certificate import is unnecessary. |
| D16 | Protect stored private keys with server-managed secrets. This does not select a particular secret store, permanent file layout, hosting provider, or key-access boundary. |
| D17 | Keep expired and superseded certificates visible with status, while excluding them from signing. |
| D18 | Worked Before uses the UTC calendar day for today, retains earlier history, and shows available grid/POTA information. |
| D19 | Each QSO captures its operating context at logging time. Profile/default changes do not rewrite prior QSOs; corrections use explicit edits, including bulk edits when appropriate. |
| D20 | Saved location profiles are optional and synchronize across clients. One-off locations do not require profile creation. |
| D21 | Current logging defaults are independent for each client. |
| D22 | Logging does not require distinct operating-session start/end times. Continuous application use with intermittent operating is a first-class workflow. |
| D23 | Loading a saved profile copies values into client-local defaults. Subsequent profile edits do not change those defaults until explicitly reloaded. Clearly indicate differences between current defaults and the associated saved profile. |
| D24 | Compare defaults against the latest saved profile available to the client. Retain the last-loaded revision as the baseline for distinguishing local adjustments from saved-profile changes, including when both occur. |
| D25 | Operating-context editing should not permanently occupy desktop space, but profile synchronization and edited state must always be visible. Layout and docking are deferred. |
| D26 | Station identity belongs to the location profile. Provide an easy way to set or clear the distinct operator identity. |
| D27 | Each client has a single current operating context. Multiple operators each use their own client. |
| D28 | For incoming QSO capture, prefer supplied field values when present. Use the client's current defaults to fill missing operating context. |
| D29 | Do not add notices or a reconciliation workflow for discrepancies between supplied QSO fields and client defaults for now. |
| D30 | Automatic catch-up of missed WS contacts from its local ADIF log is outside MVP. |

## Intentions with details still open

Source-available licensing, native desktop tooling, optional subscription packaging, and general import/migration capabilities need additional decisions. The hosting subscription is optional; self-hosting must remain possible.

## Superseded approaches and corrections

- This project is planning a new logger.
- Fixed two-client wording is superseded by D02.
- LOG4OM's manual LoTW batching versus automatic QRZ/eQSL upload is an observation, not a requirement. D09-D11 describe Clacks Log scheduling.
- A preconfigured TQSL installation supplied by the operator is not the service architecture.
- Direct tqsllib FFI workers and temporary materialized stores were explored. Temporary stores seemed feasible for unmodified tqsllib, but D13 selects a source-guided Rust port instead.
- The Rust port is not black-box reverse engineering. A reference implementation and configuration definitions are available.
- The earlier 4-8 person-week estimate was withdrawn because it was insufficiently grounded. No replacement effort estimate has been accepted.
- Remote database access, live QSO forwarding, and synchronizing whole SQLite files do not establish application-level offline reconciliation.
- Do not manufacture a separate policy for compatible precision refinements versus different values if both would receive the same field-reconciliation behavior.

## Planning discipline

When a decision changes, update the current design documents and this register together. Record substantive supersession explicitly. Do not turn an assistant suggestion, an attachment's instruction, or incumbent behavior into an accepted requirement.

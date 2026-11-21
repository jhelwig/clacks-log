# Populated log migration and desktop sync status

[Clacks Log #26](https://github.com/jhelwig/clacks-log/issues/26) delivers the
[D56 workflow](../../decisions.md#d56) for bringing contacts from an independent
standalone workspace into an existing synchronized account. It uses ordinary
Amateur Data Interchange Format (ADIF) text files (ADI), empty-workspace
enrollment, and the existing prepared import. The desktop footer shows the
general sync status so the operator knows when the target account's contacts
are available for matching.

## Migration walkthrough

Add the walkthrough to the [desktop guide](../../../lib/clacks-desktop/README.md).

1. In the standalone workspace, choose **File → Export ADI…**, select
   **All contacts in this workspace** and **Ordinary ADI**, and save the file.
   Keep the original workspace intact.
2. Launch Clacks Log with `--workspace <directory>` pointing to a fresh
   directory. Use **Connect…** to enroll it in the existing account. Wait for
   **Up to date** in the footer before preparing the import. Import matches
   against saved local contacts and pauses sync during preparation and review.
3. Choose **File → Import ADI…** and the destination tag. Review the account-wide
   matches and explicitly resolve possible or ambiguous matches. Reusing a
   saved contact adds the selected tag where needed. The existing import rules
   govern distinct duplicates, skipped rows, and service evidence.

Explain that ordinary ADI carries supported saved contact facts and applicable
confirmation fields, with conversions and omissions shown in the export
result. Import trusts applicable recognized positive status fields with the
provenance available in the file. Missing upload status is treated as pending.
An imported status does not create an external-record association or relax its
uniqueness rules. Native pending and conflict evidence, credentials, profile
definitions, and complete service history stay outside the transfer. This is
contact interchange for initial enrollment of an independent log. Existing
clients reauthorize or return from offline use through their normal flows.
[Export design](2026-11-20-adi-export-design.md) · [Import design](2026-11-19-adi-import-design.md) · [Account boundaries](../../accounts.md#accepted-boundaries) · [Backup and recovery](../../backup-recovery.md#recovery-needs)

## Sync footer

The sync worker owns and publishes the live sync status through the existing
status channel. The session forwards it and the footer displays one label.
Workspace lifecycle labels, including standalone, continue to come from the
session's workspace state.

| Condition | Footer |
| --- | --- |
| The worker is stopped, or has yet to start. | **Server disconnected** |
| A connection or retry attempt is underway. | **Connecting to server…** |
| The worker is negotiating or running an exchange. | **Syncing…** |
| The worker has an exchange to run and is waiting on the pause gate. | **Sync paused** |
| The exchange has settled and saved local work remains outstanding, including changes awaiting review. | **Unsynchronized changes** |
| The exchange has settled and no saved local work remains outstanding. | **Up to date** |
| A connection or exchange failed. | **Synchronization unavailable**, with the existing error details. |

Publish the settled status at the existing idle boundary, after the worker has
committed the inbound checkpoint, processed eligible outgoing changes, and
checked for another exchange. Use the existing durable pending-work view to
distinguish the two idle outcomes, including work the publisher skips pending
review. **Up to date** describes the completed exchange through the received
checkpoint. A local save or server change notification starts another exchange.
Each connection uses the same status transitions. Its saved checkpoint supplies
the resume position.

The worker reports **Sync paused** only when the gate delays an exchange. On
acquisition it resumes **Syncing…**. The existing gate waits for the final
pause holder to release, so release of an individual handle requires no footer
notification or separate pause count. When idle with no exchange requested,
the worker keeps its settled status and the import dialog describes the pause.

Use the existing connection and exchange spans, structured wait, receipt,
idle and failure events, and phase duration metrics for diagnostics. Their
existing account, actor and exchange context remains sufficient for tracing the
status boundary. Render status from the worker channel without reading logs or
emitting telemetry per repaint. The change needs no new persisted state,
protocol messages, metrics, or contact data in telemetry.

## Verification

Add one regression scenario to the existing desktop synchronization coverage.
Drive the production worker against a fresh enrolled workspace with a real
local store. Control delivery of the first inbound transfer so the connection
is established and an inbound page is installed while transfer completion is
still withheld. Observe the same status channel the desktop consumes and
assert **Syncing…**. Deliver the final message and assert **Up to date** only
after the corresponding inbound checkpoint is committed and the exchange
settles. Use an explicit protocol barrier so the check is deterministic.

This catches a Clacks regression where the worker publishes readiness on
connection or partial receipt. Reuse the existing enrollment, export, matching,
imported-status and association coverage. No separate manual migration replay
or test matrix is required for the new status boundary.

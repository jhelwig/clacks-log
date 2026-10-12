# Client synchronization and conflicts

Every client keeps an independently writable local log. The server stores accepted shared state and coordinates external integrations. Offline clients must continue to record and edit contacts; connectivity enables reconciliation rather than ordinary operation.

## Accepted conflict semantics

Conflict detection is at the record level. A push based on an out-of-date record conflicts with a newer accepted edit, even when the edits concern different fields. The server rejects that conflicting update until it is resolved. The pending edit stays on the originating client.

Resolution is at the field level. The user must be able to reconcile individual values rather than choose one entire record. The resolution UI, comparison data, and resubmission protocol are not yet designed.

Deletion is an edit for conflict purposes. Delete-versus-edit and delete-versus-delete behavior must follow an explicit resolution model; the exact representation and deletion retention policy remain open.

There is no agreed server-side collection of unresolved competing versions and no last-writer-wins merge policy. Accepted state, locally pending changes, and any historical/audit storage have different roles.

## Integration interaction

Only accepted server revisions are available to publication jobs. A rejected client edit does not become a publishable server revision. Conversely, the server cannot know about an edit still on an offline client: an earlier accepted revision may be published before that edit synchronizes. Later accepted corrections must be handled through the relevant service's correction behavior.

Remote-service data may enrich or correct QSO fields. Its effect on accepted record revisions and collisions with pending client edits needs explicit design; do not silently exempt confirmation downloads from conflict handling.

## Durability and convergence

The system must preserve changes across disconnection, restart, retry, and repeated transfers. New contacts, edits, deletion intent, relevant metadata, and service outcomes need to reach connected clients without silent loss or unintended duplication.

Stable identifiers, revision representation, change journals, transport, authentication, initial synchronization, duplicate detection, and retention are not selected. Device clocks must not be assumed to establish conflict order.

## Planning scenarios

- A client logs contacts offline and later reconnects.
- Multiple clients edit different records concurrently.
- Multiple clients edit the same record, including different fields.
- A pending edit conflicts with an accepted deletion or service-originated correction.
- A client remains disconnected for an extended period.
- Transfers or server jobs restart midway and are retried.
- A new client joins an existing log.
- Distinct QSOs that resemble one another are preserved; duplicate imports are reconciled deliberately.

These are acceptance scenarios for future design and implementation, not claims that a particular protocol has passed them.

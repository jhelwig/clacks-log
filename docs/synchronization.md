# Client synchronization and conflicts

Every client keeps an independently writable local log. The server stores accepted shared state and coordinates external integrations. Offline clients must continue to record and edit contacts; connectivity enables reconciliation rather than ordinary operation.

## Working changes and symmetric synchronization (D59)

Use the same state/history negotiation and transfer mechanism for client-to-server and server-to-client synchronization. A restored server can request missing synchronized state from clients just as a behind client can request it from the server. The server's coordination of accepted state and external service jobs does not make its stored history infallible.

Local edits are durable working changes. Synchronization can coalesce accumulated edits into the state to transfer, without requiring a retained event for every intermediate edit. This does not mean waiting for synchronization to save local work. Previously synchronized state and newer working changes must remain distinguishable across retry/restart. The user's commit-and-push analogy does not establish a logbook branch, radio contact (QSO) files within that branch, or one atomic commit containing the whole logbook.

Exact revision representation, synchronization boundaries for server-originated changes, and retention of synchronization evidence remain open. No Git storage format, logbook-wide commit model, or full per-edit event log is selected. D137 permits reconciliation without a conflict flow for one-sided record changes and equal resulting records; no general merge algorithm or interdependent-field conflict model is selected. D237 supersedes D36's confirmation-to-recorded-fact replacement behavior. D32 publication-attempt snapshots remain separate from recorded facts and provider evidence. Implementation batching must preserve the accepted record-level conflict semantics.

## Vector-based direction and revocation cutoff (D151–D152)

Continue design using vector-based causal tracking; exact representation and implementation remain open. D152 closes automatic acceptance from an actor when its client is revoked. This supplies a policy boundary: unsynchronized edits not accepted before revocation do not remain eligible for automatic submission. A lost installation need not cooperate or report its final local counter. Recovering such work later requires explicit manual reconciliation; reauthorization does not reopen automatic acceptance from the old actor.

This is closure of the admissible update history, not a claim that every locally generated update was physically received. Already accepted shared records and evidence are not deleted or rejected because their originating actor was later revoked. Other authorized clients can still recover them into an older restored server. An enrolled client that is merely offline remains eligible indefinitely. Retirement knowledge and causal compaction must preserve these distinctions; exact metadata is not selected. D143 still applies if a backup predates the only surviving revocation record.

## Selected timeline-level ancestry (D154)

Accepted shared state uses timeline/revision references with shared lineage metadata at exceptional recovery/fork/coverage boundaries. This records ordering without storing a version-history node for each contact edit. A boundary may establish that `(T2,205)` incorporates `(T1,200)`; this establishes coverage through T1:200, not T1:201, and must not retroactively attribute that coverage to earlier T2 positions. The original restored point and subsequently recovered coverage may differ. Normal server starts keep the timeline; exact detection and encoding remain open.


### Synchronization scope (D248)

Use one synchronization scope per account. Account-owned QSOs, including their tag sets, tag definitions/lifecycle, reference-group definitions seeded into the account under D254, and synchronized shared configuration use the same account timeline, ancestry, client checkpoints and D155 collection boundary. D156 requires durable local installation before acknowledgment. Each authorized enrolled client participates in that account's retention boundary; an indefinitely offline client remains eligible and may hold collection back until it catches up or is revoked. D161 still requires full-account replication to every enrolled client in MVP; selective synchronization remains post-MVP.

A QSO's identity and scope do not change when its tags change. Current display selection affects views/default tagging, not synchronization participation. Account isolation, server-only credential restrictions and client-local operating defaults remain unchanged. Account scopes are independent; their revision numbers are not directly comparable.

Records and bounded transaction batches can be accepted independently. One account scope does not require one atomic account-wide commit or change whole-record conflict detection. Tag/reference validity and deletion effects must still be enforced; D250 requires explicit removal/replacement of deleted-tag references, preserving pending QSOs locally as detailed in [tag lifecycle](product.md#membership-and-tag-lifecycle).

### Service evidence in the account scope (D248)

Contact-specific publication attempts/outcomes, actual sent-value context, associated remote observations, directory snapshots and per-contact processing decisions share the account scope with QSOs. Account-wide download coordination/progress, unresolved observations and durable rejection dispositions also live there. Preserve D140 recovery synchronization and existing credential-distribution restrictions.

D251 permits at most one local QSO associated with an external record within an account and service destination. Ambiguous candidates require explicit selection of at most one QSO; possible matches still require selection even when unique. Repeated observations of that record retain the established association. Distinct QSO identities may share matching fields, but cannot both claim that external record. Enforce association uniqueness atomically with acceptance and publication effects; a competing outgoing QSO must require resolution rather than silently replace the associated record. Tag deletion does not erase evidence required for surviving contacts, deletion suppression or duplicate-safe processing.

Recording a processing decision and its resulting accepted contact edit remains atomic. Download progress must not advance beyond durably retained incoming work. Exact representation, transaction batches and retention encoding remain engineering work.

### Actor retirement and garbage collection

The selected direction separates author identity from accepted-state ordering. Once an actor's admitted contributions are represented in accepted state and valid timeline/coverage metadata, their ordering need not depend on keeping that actor's vector component in every record. D152 revocation closes automatic admission, so retirement need not wait for an unavailable device's local tail or every other client's acknowledgment. Already accepted records, deletion knowledge, and service facts survive retirement. Source-actor provenance, if retained, is not a reason to keep an active clock component.

Retirement can collect active actor clock/progress/submission bookkeeping only where no remaining admission, retry, or recovery obligation depends on it. That is not permission to discard external outcomes, processed observations, or other replay-safety evidence. Exact authentication and deduplication retention mechanisms remain unselected. Retired identities must not be reused as fresh writers. D143's older-backup authorization limits still apply.

An unrevoked enrolled client retains normal eligibility indefinitely. Do not classify it as retired because its cursor is old or because it has been absent for a long time. Its local original/working values remain durable. Receiving a client's state or reporting a high revision does not establish complete recovery coverage by itself.

### Compact transfer history without expiring clients

Incremental transfer journals can be bounded independently of client eligibility. Keep current accepted records and the required deletion/service evidence; when a returning client's cursor predates retained transfers, use fuller-state reconciliation under D144 rather than forcing re-enrollment or discarding pending work. Retained lineage can establish the relation to the older timeline even when incremental payloads are gone. Compare ordinary pending work with its original values under D136–D138.

Keep the small lineage structure separately from per-edit transfers. Its growth follows exceptional recovery boundaries, not every contact or actor edit. D155 selects the ordering-evidence retention boundary below; exact baseline and lineage encoding remain open. An expired incremental journal does not by itself authorize discarding ordering evidence still required by that boundary.

Deletion and service evidence have separate safety obligations: an old client must not resurrect deleted contacts or replay completed enrichment merely because an author or journal entry was collected. Preserve those facts or a demonstrably equivalent representation. Timeline ancestry proves relationships; it does not reconstruct missing payloads.


### Ordering-evidence retention boundary (D155)

Within each account’s D248 synchronization scope, retain detailed ordering evidence from the oldest authorized client’s durably acknowledged synchronization point onward. Compact earlier history into a shared baseline that preserves the comparisons still required by surviving references. “Oldest” means common covered history established through timeline ancestry, not the lowest numeric revision across unrelated timelines.

For example, clients acknowledged through revisions 900, 700, and 300 on the same timeline hold the collection boundary at 300. The third client may stay offline indefinitely without losing eligibility or blocking other synchronization. Catch-up can advance its acknowledgment; revocation removes it from the participant set and may let the boundary advance to 700. No inactivity timeout substitutes for revocation.

An acknowledgment must represent durably incorporated state, not the highest revision received. D156 requires preserving pending comparison evidence before advancing that acknowledgment. Pending work can keep old comparison information locally while unrelated records synchronize. If the client has not durably preserved what is needed, its acknowledgment cannot advance; the existing D155 boundary already handles that case. No concrete normal-operation need for a separate server-side collection pin for pending edits has been established. Exact evidence representation and baseline installation remain to be designed.

Compaction must preserve the interpretation of current records that still refer to old positions. Retain or summarize the relationships needed by those references; do not simply delete all earlier timeline boundaries. Newly enrolled clients must initialize against an appropriate baseline before their acknowledgment can count toward collection.

An older restored backup can predate the collected baseline. Use surviving client evidence where available and the accepted explained manual-reconciliation fallback where ordering cannot be established. Do not treat absent old evidence as proof of succession or absence of changes. D155 preserves ordinary indefinite offline return; it does not guarantee automatic ordering for arbitrary old backups. Deletion memory used to suppress future external-download add prompts and external-service processing evidence remain governed by their separate safety obligations.

The published common-knowledge collection principle is discussed in [Peer-to-Peer Replication in WinFS, section 5.3.1](https://www.microsoft.com/en-us/research/wp-content/uploads/2006/06/tr-2006-78.pdf): metadata can become collectible once known to all participants, and disconnected members can delay collection without blocking updates. Clacks applies this principle to its authorized participant set and selected timeline ancestry; this is not selection of that software or its complete replication model.


### Durable checkpoint acknowledgment (D156)

Before acknowledging a checkpoint to the server, atomically save the received shared state, the checkpoint, and the comparison evidence required by pending local work. After a crash, either that checkpoint is durable or synchronization resumes from an older checkpoint with safe repeat transfer. Acknowledging receipt before this local commit would let collection advance past information the client could lose.

Unresolved conflicts remain local and pending while unrelated synchronization progresses. Their comparison evidence must survive checkpoint advancement. This does not imply that publishing their eventual resolution can bypass D146's revalidation against current shared state. Exact dependency factoring and storage representation remain open.


### Proposed pending comparison evidence — representation not yet selected

A durable pending comparison snapshot would identify the record and its account scope, including the QSO’s complete tag set where applicable; preserve the original accepted values, current local working values, and latest observed accepted shared values, including explicit deletion states; and retain the accepted-version references and causal context needed to interpret those values. Causal context means what accepted history the comparison inputs include, not merely what the client has subsequently received. Completing unrelated synchronization must not relabel an edit based on revision 200 as an edit based on revision 900.

For collection, the context must remain interpretable through retained timeline ancestry or a sufficient local summary of the particular ordering relationships being removed. A reference to metadata that is about to be deleted is not self-contained evidence. Known comparison outcomes may be cached, but they do not establish how a future shared version relates to the saved inputs; eventual submission still revalidates current shared state under D146.

For an ordinary same-timeline conflict, original value A at revision 200, local value B based on A, and shared value C at revision 600 preserve the three-way comparison without retaining intervening payloads. If the client later acknowledges revision 900, it must still preserve the pending edit's actual basis and enough context to compare against subsequent shared state. This requires no complete per-edit history graph.

Before advancing an acknowledgment, the client must preserve the needed comparison values and context, including any necessary summary of older ordering relationships. Failure to save that evidence leaves the earlier acknowledgment in effect; it is not a new pending-edit retention rule. The earlier suggestion of an additional global collection pin was not supported by a concrete requirement and is withdrawn as a recommendation. The exact representation still needs validation, so this is not a proof that every possible metadata encoding permits local summarization. Shared baseline obligations for surviving references and deletion/service evidence remain independent. An arbitrary older-backup recovery outside retained coverage can still use the accepted explained manual fallback; local snapshots do not guarantee automatic ordering against every future recovered branch.

This distinction has a published analogue in [Peer-to-Peer Replication in WinFS, sections 5.1.4 and 5.2.3](https://www.microsoft.com/en-us/research/wp-content/uploads/2006/06/tr-2006-78.pdf). Its “made-with knowledge” records the causal context of a version; unresolved versions retain explicit context even as replica-wide knowledge advances. Clacks's original/local/shared value semantics and server-star topology remain its own agreed requirements; no complete WinFS model is selected by this analogy.

### Proposed baseline installation mechanics — not yet a separate decision

Build a compacted baseline from a consistent view of the D155 boundary, authorization membership, and surviving reference dependencies. Durably install the equivalent baseline before removing the detailed evidence it replaces. Concurrent synchronization or enrollment must not invalidate the collection proof. Physical cleanup can be incremental and restartable: interruption before installation leaves the old evidence authoritative; interruption afterward may leave redundant old evidence to remove later. This is a proposed durability invariant, not a selected storage algorithm or requirement for a new all-client acknowledgment round.

A baseline must preserve required ordering relationships rather than replace every old record position with a newer position. Compaction is not a contact edit or a new service-relevant value; it must not trigger external uploads or erase pending conflict evidence. Backup recovery still negotiates surviving state/evidence under D153–D155; baseline installation does not make old backups rollback-proof.

## Replication research before representation selection (D135)

Stable entity identity is distinct from the unresolved means of recognizing shared, newer, stale, or divergent state. D120's Universally Unique Lexicographically Sortable Identifier (ULID)-like identity preference does not select causal metadata.

Research distributed data replication broadly. conflict-free replicated data types (CRDTs) and vector clocks are illustrative starting points, not an exhaustive list or a preferred shortlist. Examine optimistic replication, conflict/reconciliation policies, causal tracking, anti-entropy, state/log/delta transfer, membership and actor retirement, garbage collection, rollback recovery, and other relevant published approaches. Learn from their assumptions and failure cases even where their full model would not fit Clacks Log.

An individual account is expected to have a modest number of server/client replicas, without imposing a fixed client limit. Cumulative clock/actor identities still need a viable garbage-collection story. Compare that need with indefinite offline enrollment, explicit revocation, deletion memory, and restored-server recovery rather than assuming a particular metadata representation solves it.

The [replication research](../references/replication-research/README.md) will separate source findings, application requirements, candidate designs, and unresolved questions. No algorithm, revision graph, clock scheme, storage engine, or transport is selected by this research direction.

## Replication approaches and data-model research (D150)

Survey published replication approaches, abstract data models, and data structures. Compare representation, reconciliation/convergence, causal evidence, and communication/failure/garbage collection (GC) assumptions against agreed behavior. Software systems may supply case-study evidence; implementation selection is a later question.

The [replication approaches survey](../references/replication-research/approaches-survey.md) is the current analysis. No algorithm, clock ownership, storage engine, or implementation is selected. Superseded recommendations and software-fit reports have been removed.

## Accepted conflict semantics

The account owns each QSO, including its complete tag set (D243). Tags identify views and do not own independent QSO copies. A membership change is a QSO-record edit: concurrent edits to different tags, or to tags and contact facts, can require the same resolution as other differing two-sided record edits. Do not automatically merge tag sets or split synchronized membership records to avoid these accepted MVP conflicts. Exact physical storage is independent of this logical comparison boundary.

Conflict detection and resolution run on the clients (D141). D137 applies the resolution-flow trigger at the whole-record level using originally observed, local, and shared values. If only one side changed the record, use that side's result without a conflict flow. If both sides reach the same record, no conflict flow is needed. If both changed and their resulting records differ, require field-level user resolution, even when their changes concern disjoint fields. Keep the unresolved record update pending on its originating client. No last-writer-wins rule is selected.

Resolution is at the field level. The user must be able to reconcile individual values rather than choose one entire record. D40 also uses this field-level conflict-resolution interface for updates of matched QSOs during Amateur Data Interchange Format (ADIF) import, with the user choosing which differing fields to apply. D136 requires sufficient originally observed values for an effective three-way field comparison. Distinguish a field that differs because shared state changed while local state did not, a field that differs because local state changed while shared state did not, and a field where both changed. Original values need not be shown directly. This selects comparison capability, not a particular base-revision representation, permanent full history, or user interface (UI) layout. D137 selects whole-record cases that trigger user resolution; the field classification helps explain differences within that flow. A broader model of interdependent-field conflicts is after the minimum viable product (MVP) release; existing data validation and specified calculated-field behavior remain requirements. Exact field equality, validation interactions, and resubmission mechanics remain open. For an import without an originally observed shared state, do not fabricate a third comparison input.

Deletion is an edit for conflict purposes. D138 treats deleted as a possible comparison state: deletion versus an unchanged record proceeds without a conflict flow; two deletions agree without a flow; deletion versus a changed record requires resolution. This does not select tombstone encoding or retention. Tag deletion applies the selected preserve/delete-all/delete-unique effects; a QSO referencing a deleted tag remains pending locally until the user explicitly removes or replaces that membership. Removing its last tag can leave the QSO active and untagged. Do not recreate the tag or silently rewrite memberships. Unrelated synchronization continues. A pending edit to a QSO itself marked deleted still follows ordinary QSO deletion/edit comparison. Enforce reference validity at shared acceptance, so a concurrent deletion cannot be bypassed by a previously checked reference. D54 requires remembering deleted QSOs and their external associations to suppress later add prompts for those contacts; external copies remain untouched in MVP.

There is no agreed server-side collection of unresolved competing versions and no last-writer-wins merge policy. Accepted state, locally pending changes, and any historical/audit storage have different roles.

## Comparison-to-submission race (D146)

A client comparison can become stale before its result reaches shared storage. D146 requires atomic conditional acceptance: the relevant shared state must still match what the client compared when its submission is accepted. Otherwise the client receives newer state and repeats its whole-record comparison, entering user resolution only under D137/D138. The server's storage precondition check does not interpret business differences, resolve them, or retain unresolved competing business records.

The condition's encoding, representation of compared state, aggregate transaction scope, durable retry handling, and response protocol remain open. No revision/clock representation is selected.

## Offline conflict resolution (D139)

Resolving a known conflict offline from the last received shared values is acceptable but not required for MVP, with later scope uncommitted. If implemented, the selected result is durable local work and undergoes the same whole-record comparison when reconnecting. A changed shared record can require resolution again; offline selection does not authorize overwriting intervening changes. Required offline logging/editing and pending-work durability remain separate from this optional resolution capability.

## Integration interaction

Only accepted server revisions are available to publication jobs. A rejected client edit does not become a publishable server revision. Conversely, the server cannot know about an edit still on an offline client: an earlier accepted revision may be published before that edit synchronizes. Later accepted corrections must be handled through the relevant service's correction behavior.

D237/D238 keep confirmation-provider information separate from recorded QSO facts. Downloads do not automatically overwrite the on-air exchange or our station's operating context. Effective values use recorded QSO information first, then eligible LoTW information, eligible QRZ information, and the eligible directory snapshot. Select location as a whole from the first available source rather than filling gaps from other locations.

Changes to provider evidence synchronize under their evidence model; they are not edits to recorded contact facts and must not trigger the recorded-QSO conflict flow solely because that separate evidence changed. D209 still arbitrates directory snapshots automatically. Actual recorded-contact edits retain D04/D136/D137 whole-record comparison. A provider update can change effective display/export/upload values without changing recorded facts; service-relevant effective-output comparison determines publication need. Exact evidence encoding and provider-specific ordering remain engineering work.

Repeated delivery of the same remote observation has no second effect: it must not create a recorded-contact edit, replay a processing decision, or undo later work. Distinguish new provider updates from duplicate transfer. D237 supersedes the former D33–D36 overwrite/enrichment-revision model; current source separation does not require preserving that model's mutation steps.

D140 includes recovery-relevant service evidence in MVP synchronization: publication outcomes with the D32 actual sent values/destination context, remote observations, and processing decisions including matching, rejection, and enrichment. Surviving clients can supply missing evidence to an older restored server through normal synchronization. A recovered success covers its recorded publication, not later changed service-relevant values; recovered processing decisions must not replay completed enrichment. Exact representation, retention, and coordination with accepted QSO state remain open.

## Restoration scope (D55)

Explicit restoration of deleted QSOs is post-MVP. The earlier MVP restore-action proposal was declined. D54 still requires retaining deletion information and external associations for downloaded-add suppression. D05 deletion-versus-edit conflict handling remains required; excluding a dedicated restoration feature does not silently resolve those conflicts. Exact retention and any future restoration design remain open.

## Durability and convergence

The system must preserve changes across disconnection, restart, retry, and repeated transfers. New contacts, edits, deletion intent, relevant metadata, and service outcomes need to reach connected clients without silent loss or unintended duplication.

Detailed stable identifiers, revision representation, change journals, initial synchronization mechanics, duplicate detection and retention remain specification work under the later decisions. D235 selects encrypted WebSocket transport; account authentication follows D190–D193/D231–D233. D56 selects empty-log enrollment with ADIF migration for preexisting independent contacts. Device clocks must not be assumed to establish conflict order.

## Indefinite offline operation (D60)

An enrolled client can reconnect through normal synchronization after an indefinite offline period. Elapsed offline time alone must not require ADIF export/re-import or re-enrollment. Pending contacts/edits remain durable, and the representation/retention of synchronized state must permit negotiation with an older client. This does not require retaining each intermediate edit as a history event.

D56 governs initial enrollment of an independently populated log, not the normal return of an enrolled client. Exact retention, compaction, and device authorization policies remain to be designed without introducing an automatic offline-expiry rule.

## Initial connection and migration (D56)

MVP enrollment into an existing synchronized log uses an empty local log, which receives the shared server log. For preexisting independent contacts, export to ADIF before joining and re-import afterward using D38-D43's import workflow. Direct merging of independently populated logs during enrollment is outside MVP. This does not authorize silently clearing preexisting local data.

Once enrolled, the client remains independently writable offline, and contacts/edits accumulated offline synchronize normally. Shared application identifiers/revisions participate in the sync protocol; repeated transfer of an existing record is not another ADIF import. Initial download mechanics and recovery still need design.

## Rollback resilience (D58)

Normal synchronization must recover from an older restored server by negotiating shared/missing state with existing clients. Recover client-held synchronized data and preserve pending work; do not require manual reconciliation or ADIF migration merely because the server is behind. The server coordinates accepted shared state and external jobs; clients detect and resolve conflicts. Divergent record edits still require the established field-level resolution; rollback recovery does not select silent merging or last-writer-wins.

Revision identities/history evidence must distinguish a restored older history from a newer or divergent one. Server counters and transfer cursors may optimize transport but cannot alone establish that two revisions are identical. Exact history representation, recovery of service metadata, and retention remain open.

D141 makes restore publication an operator-controlled workflow: pause all uploads during rollback/restore, synchronize with available sources to become as current as possible, then explicitly resume uploads. A client may never resolve or submit a detected conflict; notifying the server must not create a publication dependency on that action. This does not suspend normal synchronization, expire offline clients, or require every enrolled client to return. Exact pause scope/control, safe restore startup, and protocol-level race protection remain open.

D147 permits explicit two-way field resolution when recovery establishes genuine divergence but no available copy retains the common original values. Explain that change attribution is unavailable rather than inventing three-way classifications. D136's original-value retention for ordinary pending work remains required. Missing historical payload and unavailable causal ordering are separate problems. D148 permits conservative explicit reconciliation for differing states when ordering cannot be established, clearly explaining that missing ordering evidence is the reason. This may prompt where complete metadata would have proved a one-sided change; do not call uncertainty proven divergence or silently select a winner. Exact retention/GC remains open.

D153 explicitly permits detecting rollback and requiring manual client reconciliation. A storage quorum or uninterrupted atomic-register recovery is not a requirement. The proposed comparison between advertised overall server revision and retained client knowledge can detect a regression, but is not a selected complete detector. Preserve data and ordinary pending originals; exact handling of invalid comparison metadata and repair scope remain open. An older server that is merely behind may still catch up automatically when relationships are established.

## Catch-up and streaming (D61)

On connection or reconnection, batch-negotiate and transfer missing synchronized state in both directions. After catch-up, stream updates as available while connected. Interrupted streams reconnect through catch-up, including cases where a server has restored older state. Logging/edits remain locally durable throughout; conflicts remain subject to D04.

The protocol needs a gap-free handoff between catch-up and live updates. Changes made during catch-up must be included or queued for subsequent delivery. Retry/overlap must have no second effect, and acknowledgments must not discard work that the peer has not durably accepted. D235 selects encrypted WebSocket transport. Exact snapshot boundaries, subscription/acknowledgment design and backpressure remain specification work. Streaming notifications do not imply retaining a permanent history event for every intermediate edit.

D144 permits automatic fuller state reconciliation when incremental transfer history is unavailable. It remains normal, bidirectional synchronization, preserving enrollment, surviving synchronized evidence, and pending work. A larger comparison or transfer is acceptable. Exact snapshot/manifest mechanics remain open, and this fallback alone does not justify collecting causal identities, deletion knowledge, or service evidence.

External-service upload cooldown has no role in this client synchronization behavior. No manual sync-trigger requirement was accepted in this discussion.

## Client revocation (D62)

MVP enrollment gives each client a distinct authorized identity, and the owner can revoke that client's server access without deleting shared contacts or revoking unrelated clients. An explicitly revoked client cannot synchronize until authorized again. Its disconnected local copy is not remotely erased; offline local editing and server authorization are different concerns. D60 prohibits expiry based solely on offline duration, not explicit access revocation.

D145/D152 preserve a reauthorized client’s retained synchronized data and pending offline work without D56 migration. Previously unsynchronized work excluded by revocation requires explicit manual reconciliation; reauthorization alone does not submit it automatically or reopen the old actor. Credential/actor identity mapping, retirement representation, and manual recovery workflow remain open.

D63 requires account isolation for log data, profiles, clients, and service credentials. Negotiation, catch-up, streaming, and revocation operate within the authorized account boundary. Exact enrollment/sign-in mechanism, administration interface, reauthorization, and permissions remain open. No token type or exact administration UI is selected. Enforcement must cover existing synchronization streams as well as subsequent requests; exact mechanics remain open.

## Planning scenarios

- A client logs contacts offline and later reconnects.
- Multiple clients edit different records concurrently.
- Multiple clients edit the same record, including different fields.
- A pending recorded-contact edit conflicts with another accepted recorded-contact edit or deletion; separate provider-evidence updates do not manufacture recorded-contact conflicts.
- A client remains disconnected indefinitely and reconnects through normal sync without export or re-enrollment.
- Transfers or server jobs restart midway and are retried.
- A server is restored from an older backup while enrolled clients retain newer accepted state and pending edits.
- A restored server and disconnected clients develop genuinely divergent edits before reconnecting.
- A new client joins an existing log.
- Distinct QSOs that resemble one another are preserved; duplicate imports are reconciled deliberately.
- One QSO with Main and Activity tags appears once per filtered view, sharing facts and service evidence; changing tags does not change its synchronization identity.
- Concurrent tag/fact or different-tag changes use the ordinary whole-record conflict trigger.
- Removing the last tag preserves an active QSO; marking the QSO deleted excludes it from every normal view.
- An offline QSO referencing a deleted tag remains pending until explicit membership removal/replacement; unrelated records continue synchronizing.
- Competing associations to one external record are rejected for resolution without discarding pending work or silently overwriting the associated external record.

These are acceptance scenarios for future design and implementation, not claims that a particular protocol has passed them.

## Mode interpretation across client versions (D178)

Clients preserve the same underlying mode value and supplied context despite different recognition tables. A typed known variant on one client and OtherMode on another are interpretations of that value, not competing edits. Preserve it through unrelated contact edits and upgrades. Recognition updates alone must not advance contact revisions or create conflicts. Use ordinary Serde serialization/deserialization under D216, with stable representation and equality handled as routine schema implementation rather than a bespoke architecture. D178 does not select general perpetual compatibility for all future schemas/protocols.

## Incompatible client versions (D179)

Schema or protocol incompatibility may require a client upgrade before synchronization resumes. Preserve enrollment, local data, pending working values, and comparison evidence through the upgrade. Indefinite offline eligibility does not promise perpetual protocol compatibility for old executables, but version incompatibility must not force re-enrollment or ADIF export/import. Compatible recognition differences use D178’s stable underlying mode representation and are not themselves grounds to require an upgrade. Exact compatibility negotiation, migration mechanisms, and supported-version policy remain open.

## Local working values in contact views (D180)

History and Worked Before show this client’s saved working version with pending/conflict indicators, one entry per QSO identity. New offline contacts and pending saved edits remain visible; unresolved proposals are not published merely to make views consistent across clients. Shared values remain available in conflict resolution. This does not duplicate a contact for each version or collapse distinct contact IDs. D181 hides pending local deletions from normal history/Worked Before immediately, including offline, while retaining evidence and access through pending/conflict review. A conflict does not automatically restore normal-view visibility. This preserves existing resolution rules and does not add accepted-deletion restoration.

## Account-wide local review (D182)

Pending/conflict review spans this client’s work throughout the account, filterable by logbook and record type. It includes hidden pending deletions, contact changes with deleted-tag references, and account-shared profile/equipment conflicts. The review remains a view of local pending state, not a server-owned set of unresolved proposals or a publication dependency. Normal history and Worked Before remain scoped to the current book.

## Missing equipment references (D183)

A pending profile edit that references deleted equipment remains local for explicit reassignment or removal of the optional default in MVP. If a new reference is accepted first, configuration deletion must refresh the affected-profile set and handle it under D73; enforce consistent acceptance rather than creating dangling accepted references. Unrelated sync continues, and copied operating values/QSO snapshots remain unchanged. Post-MVP, add explicit restore-original or clone-to-new-configuration resolution options. Exact future retention/content availability and identity/transaction mechanics remain open. This equipment-reference policy does not change D244/D250 tag-deletion behavior or general contact restoration scope.

## Provider-supplied directory snapshots (D209)

Directory snapshots use automatic arbitration rather than the user-resolution policy for recorded QSO facts. Select independently for each QSO and normalized directory lookup key: greatest successful lookup time recorded by the fetching client, with a stable deterministic tie-breaker for equal times. Preserve the ordering metadata through retries and recovery. Neither server receipt nor re-propagation creates a newer lookup. The winning successful result is one complete snapshot; failed/not-found attempts do not replace it.

This is last-writer-wins register arbitration for provider data, not silent last-writer-wins treatment of recorded contact facts. Independent snapshot refreshes do not trigger QSO conflict resolution, and competing directory refreshes need no user resolution. Clock skew affects the freshness of the chosen result, not convergence. See [the directory model and primary research](../references/callsign-directory.md). Exact timestamp/tie-breaker encoding and physical record layout remain open.

The snapshot's QSO/key association remains a validity condition under D207. An old-key refresh cannot displace a new-key snapshot; display/export/upload fallback uses only the current key's eligible snapshot. Keep snapshots in the account synchronization scope under D248. QSO deletion dominates its snapshot; a snapshot cannot resurrect a deleted QSO or tag. Existing retention/acknowledgment/recovery obligations still apply; D209 requires no permanent lookup history or additional timelines.

## Synchronization transport (D235)

Use encrypted WebSocket connections in the existing Axum server application for MVP synchronization. Carry bounded catch-up batches and ongoing changes, acceptance results and acknowledgments in both directions over the same connection. See the [transport comparison](../references/synchronization-transport.md).

Keep the researched vector/timeline model, full-account scope participation and record-level reconciliation unchanged. Reconnect from durably installed checkpoints, using fuller-state reconciliation where needed. Frame delivery is not durable installation or proof of shared acceptance. Exact message encoding/correlation, backpressure, live authorization/revocation and gap-free catch-up/stream transition remain specification work. No additional server service, automatic alternative-transport fallback or browser offline-sync feature is selected.

# Continuous contact tables

Approved design: issue #47 and the user's approval in this chat.

History and both Worked Before presentations show one scrolling display of all
matching contacts. Remove Previous, Next and page numbers. Retain match and
selection counts, filtering, sorting, row activation and keyboard navigation.
Future tables follow the same presentation contract. Internal paging remains
invisible. No schema, synchronization or dependency changes are needed.

Use egui_extras 0.37's `TableBody::rows` with the logical result count. Its
implementation already computes visible indices and reserves offscreen space;
`scroll_to_row` supplies keyboard scrolling. Clacks supplies row data for those
indices through existing 100-row asynchronous store reads. Retain only pages
needed by the current viewport and pending keyboard target. Worked Before has
one logical query and shared data, with independent tab/window viewport demand.
Do not read every preceding page when the scrollbar jumps far down the list.

Loading rows occupy their final positions, with readable loading/error feedback
and retry for failures. Requests remain bound to the complete query and existing
reply channel. Changed filters, prefixes, scopes, ordering and committed data
invalidate applicable reads; obsolete responses cannot repopulate current rows.
Selection remains keyed by QSO ID. Counts in Show selected use the selected set
as the displayed row count, retaining the applied-filter counts separately.

The alternatives were fetching every matching QSO (unnecessary unbounded memory)
and appending batches at the bottom (scrollbar does not represent the complete
result set). Neither satisfies the approved full-result bounded-read design as
well as the existing toolkit's virtual table.

Existing store/runtime tracing and query metrics cover background reads. Add
focused diagnostic events for stale reply discard; never record query text or
contact payloads, and avoid per-frame/per-row instrumentation. No new metric
family or telemetry abstraction is needed.

Acceptance: semantic component tests exercise requests beyond row 100, direct
jumps, keyboard crossing and activation, selected-only extent, stale replies,
refresh and failure recovery, plus independent Worked Before viewports. Preserve
existing selection/query and column behavior tests. Inspect the assembled macOS
production application using explicit isolated workspace/config/log paths across
empty/populated/loading states and resizing. Automated checks cannot establish
native Windows visual acceptance; report that limitation explicitly.

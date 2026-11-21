# Unified desktop Settings

Design for review. Delivery is tracked in [Clacks Log #48][issue].

## Intent

Replace the collection of commands in the desktop Settings menu with one
Settings destination. An operator can make related changes across sections
and apply them from one screen. Settings owns one draft and one Apply and
Cancel pair.

Settings occupies the main window while it is open. The workbench is
unavailable for editing during that time. Returning to it restores the
unfinished contact, current view, and prior focus when available. The workbench
continues to own **Operating settings…**, immediate logbook selection, sync
status, and direct access to **Unsynchronized changes**. Reusable station and
equipment records retain their existing [relationship to active operation][operating].
The Operating settings workflow retains its existing nested actions for
saving reusable records from the current operating values.

## Approaches considered

A separate Settings window would keep the logging context visible and require
coordinating focus and input between two windows. A main-window Settings view
gives the editor the workbench's space and keeps keyboard navigation within
the active task. Use the main-window view because workbench editing is
unavailable during a Settings session.

Section-level saves would preserve the old editors' independent save
lifecycles and require applying changes as you navigate. The selected
approach is one shared draft and one Apply. A single Apply can report
individual outcomes because the settings cover distinct operations.

## Sections

List the sections in navigation along the left side of the Settings screen.

| Section | Contents |
| --- | --- |
| Logbooks | Create, rename, and delete named logbooks. Configure their automatic-upload policies. |
| Station profiles | Manage reusable station profiles. |
| Equipment | Manage reusable equipment configurations. |
| Connection | Show the account and server connection. Start browser sign-in with Connect or Reauthorize. |
| WS Suite | Configure reception from WSJT-X and related applications, including the listening interface and port. |
| Appearance | Edit the theme, UI scale, and fonts, with a live preview. |

Lists and their create and edit forms belong inside the relevant section.
Section navigation preserves draft changes. Related changes can reference
one another within the draft. For example, a station profile can select an
equipment configuration created in the same draft. Keep confirmation dialogs
for actions that need explicit confirmation, including logbook deletion and
equipment deletion with profile repairs. A confirmation puts the reviewed
operation into the draft. The review accounts for draft changes that affect
the deletion's impact. Apply revalidates the reviewed effects before
committing them. See the [logbook deletion contract][logbooks] and
[equipment deletion contract][equipment].

Open Settings from the Clacks Log menu or Command–Comma on macOS,
from Tools → Options on Windows, and from Edit → Preferences in the Linux
fallback. Existing task-specific entry points
open the same screen at the relevant section or record. This includes the
workbench shortcut from a WS Suite error and the station-profile repair
action from Unsynchronized changes. Preserve access to Appearance while the
workspace is opening or unavailable. Workspace-dependent sections become
editable when the workspace and its settings are ready.

## Draft, Apply, and dismissal

Show validation messages at the affected inputs as the operator edits.
Apply processes the entire draft and saves every change it can. An invalid
field or failed write affects the change that depends on it. Independent
changes still proceed. Evaluate dependent changes against the outcomes of
their prerequisites and preserve the existing domain rules and operation
boundaries.
For example, a reviewed equipment deletion and its required profile repairs
commit together. Separate edits elsewhere in Settings can still succeed if
that operation fails.

While Apply is running, show progress and prevent a second Apply or edits to
the submitted draft. When it settles, keep Settings open at the current
section and clear the draft. Display the resulting saved values. A failed
change shows the current confirmed value, which may differ from the value
shown when editing began. A successful configuration write followed by
failed WS Suite reception shows the saved configuration and the reception
failure at WS Suite. Apply is disabled until there is a new edit. The
dismissal action reads **Close** when the draft is empty.

After an Apply with failures, show a status icon on Apply and on each affected
section and setting. The Apply icon describes the last result even while the
button is disabled. Put the diagnostic next to the affected setting or record.
Identify the attempted change where it helps explain the result. Give icons
accessible text equivalents. For example, if a logbook rename saves and a
WS Suite port value fails validation, the logbook shows its saved name, WS
Suite shows the current port with its diagnostic, and Apply and WS Suite carry
failure indicators. Both changes have finished processing.

If a deletion review becomes stale, show that result at the affected record
and offer to review the updated impact. Opening that review starts a new edit
and can prefill choices that still apply. Explicit confirmation is required
to add the new deletion to the draft.

Cancel discards the whole unapplied draft, restores any appearance preview,
and returns to the workbench. Backing out of a deletion confirmation keeps
the draft intact. Follow [D295][dismissal] for editor dismissal by Escape.
Application shutdown continues through its existing lifecycle.

Connect and Reauthorize are explicit Connection actions that launch the
existing browser sign-in flow. They use the Connection inputs and leave
unrelated draft edits intact. Keep sign-in progress and feedback in Connection.
Wait for sign-in and any workspace reopen to settle before enabling Apply,
so it acts on the resulting workspace. Revalidate workspace-dependent edits
against a reopened workspace before applying them. Dismissing Settings
during sign-in follows the [existing local cancellation rules][connection].
After dismissal, resume sync when sign-in has settled and the workspace is ready.

## Automatic synchronization pause

When Settings opens, pause sync for the local workspace. Let any in-flight
local synchronization transaction settle at a safe boundary before loading the
editable workspace draft. From that point until the screen closes, the sync
driver must neither install incoming changes in that workspace nor publish
local changes. The pause applies to the workspace on this desktop. Other
clients can continue to sync.

Apply saves locally while Settings is open. Closing Settings, including after
Cancel, automatically resumes normal sync for an enrolled workspace. That
includes a workspace enrolled through Connect while Settings was open. Preserve
the pause across Connection-driven workspace reopen. If the application is
shutting down, its existing shutdown path takes precedence over resuming sync.
Offline and unenrolled workspaces remain usable under their normal rules.

When sync resumes, use the existing checkpoints, pending work, and conflict
handling. A competing edit from another client is reconciled through normal
sync, with unresolved conflicts available through **Unsynchronized changes**.
Keep the existing [durable receipt and resume rules][sync]. The implementation
plan should choose the pause boundary and driver integration based on the
existing session and sync lifecycle.

## Automatic view recovery

Remove the manual **Refresh settings** command and the corresponding prompts
to refresh or retry loading. The application owns recovery in both Settings
and workbench flows that use the same settings snapshot. Publish an
acknowledged save independently of any follow-up read. Recover the current
view automatically if the read fails. If a write result is unknown, check the
stored state before deciding what action is safe. Keep the last verified state
and explain a persistent failure where it affects the operator. A read failure
never reverses a confirmed save. An automatic read also keeps any edits that
the operator has made since the completed Apply.

## Verification

- Verify one Settings screen, cross-section draft retention, and the shared
  action boundary through the production desktop entry points. Exercise the
  existing direct-entry shortcuts and the return to an unfinished contact.
- Verify no save on Cancel, restoration of the appearance preview, and
  confirmation before staging destructive changes. Verify existing stale
  deletion reviews still guard the operation at Apply.
- Exercise Apply with both successful and failed changes. Check the saved
  state, cleared draft, indicators and local diagnostics, and distinguish a
  saved WS Suite configuration from failure to start reception.
- Exercise opening Settings during active sync, applying only to local
  storage while open, automatic resume on close and Cancel, and enrollment or
  reauthorization while Settings remains open. Verify that a competing remote
  edit is handled by normal reconciliation after resume and that shutdown
  settles ongoing work through the existing lifecycle.
- Check automatic recovery when a read fails after a successful save. Confirm
  the action result stays accurate and the application does not repeat the
  write as part of refreshing the view.
- Inspect presentation, focus, and relevant menu behavior on supported
  platforms using the [desktop UI completion gates][ui].

[issue]: https://github.com/jhelwig/clacks-log/issues/48
[operating]: ../../operating-settings.md#2-selected-interaction-model
[logbooks]: ../../logbooks.md#6-tag-deletion-review-and-commit
[equipment]: ../../operating-settings.md#62-referenced-equipment-deletion
[dismissal]: ../../decisions.md#d295
[connection]: 2026-11-03-connection-workflow-design.md
[sync]: ../../synchronization.md
[ui]: ../../desktop-ui.md#ui-change-completion-gates

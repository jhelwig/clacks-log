# Connection dialog and workflow cleanup

## Intent and scope

Accepted direction: Connect owns the complete connection operation. Users supply
the server and installation label; they never select credential recovery steps.
The demonstrated defect is that a failed token exchange disables Connect and
exposes Resume saved attempt, Discard saved attempt and separate browser recovery.

This design covers the existing desktop dialog, enrollment worker, first-install
eligibility check, session integration and owning documentation. It adds no startup
policy, automatic background retry service, account switcher, workspace picker,
protocol, dependency, migration or general recovery framework.

## Existing requirements and evidence

- D56 prohibits automatically merging an independently populated standalone log.
- D145/D152 preserve enrolled data and pending work during reauthorization; a new
  writer identity does not authorize automatic submission of old pending edits.
- Browser sign-in and approval use Huskarl 0.11.9. Native credentials use the
  existing keyring adapter; SQLite holds nonsecret references and completion state.
- Current `enrollment/first.rs` installs the verified account in the current empty
  workspace, replacing only unchanged bundled definitions and retaining local
  operating facts/preferences. It performs the population check transactionally.
- `enrollment/worker.rs` already refreshes a saved credential or opens browser
  authorization, but UI commands make the user select the entry path.
- The session already serializes enrollment, joins the worker before publishing
  terminal status, and reopens account-qualified services after installation.
- Section 6 and the workspace row in `client-enrollment-proposal.md` own the
  corrected in-place enrollment and internal recovery requirements.

## Approaches

The accepted approach consolidates connection orchestration in the existing
worker and makes the dialog submit user intent. This fixes both the visible
controls and the behavior behind them while retaining existing persistence guards.
A label-only cleanup would leave the UI responsible for recovery selection and
server locking. A new recovery manager or retry queue has no demonstrated gap to
fill and is excluded. These are rejected approaches, not additional user choices.

## Dialog behavior

Default a new connection to `https://clacks.w7hex.com` and the existing
“Desktop” client label so hosted users can submit without additional input.
A saved attempt or installed binding supplies its own server; first connections
remain editable for self-hosted servers.

Keep Server and Client label. Explain the label with “Identifies this installation
in your account's client list.” Keep Connect and Cancel for first enrollment and
the existing Reauthorize action for an enrolled workspace. Both use one connection
request; an enrolled request without pending completion starts the existing browser
reauthorization flow. The enrolled server remains fixed to its installed binding.

Remove Resume saved attempt, Discard saved attempt, Authorize again in browser,
key-removal copy and any destination controls. An unfinished attempt alone does
not lock Server or Client label or disable a valid connection request.

While working, prevent duplicate submission and display “Connecting…” or
“Complete sign-in in your browser.” Retain Open browser only during browser
approval. It reopens the current authorization URL. Cancel, Escape and window close
stop the local operation and dismiss the dialog; they do not promise remote rollback.
Retain the existing rule that another operation cannot start until the worker exits.

On failure retain inputs, enable the primary action after the worker exits and show
a concise explanation with sanitized expandable technical details. For a server
failure use “The server could not complete sign-in. Try connecting again.” Native
credential access failure must describe that access problem, not demand sign-in.
Account mismatch must preserve the installed binding and explain that the existing
account is required. Do not dump error chains, tokens, keys, authorization URLs or
arbitrary response bodies into user-visible details or logs.

On successful installation close the dialog. Synchronization remains represented
by the existing connection-status display; enrollment does not assert synchronization
completion. Remove the completion path display. Preserve actionable cleanup failures
without claiming the already committed account connection was rolled back.

## Internal connection handling

One `Connect { server, label }` command plus Cancel replaces Begin, Resume,
AuthorizeAgain and Discard at the UI/session boundary. Keep recovery phases private
to the worker and existing store. Re-read actual binding/native state rather than
inferring completed effects solely from a saved phase.

1. Validate inputs and the installed server/account boundary. For first enrollment,
   check existing population eligibility before creating a key or opening a browser.
   Reuse the installation predicate; retain its transaction-time check to catch
   intervening changes. Explain an ineligible log before requesting authorization.
2. With no saved attempt, start the existing authorization flow. With only a usable
   saved signing key, start a fresh authorization-code flow using that key. Never
   replay a previously consumed authorization code.
3. With a saved refresh credential, refresh and verify identity, then finish the
   guarded local installation. Do not replace valid credentials merely to retry.
4. Missing native material or a definite refresh credential rejection permits fresh
   browser authorization within the same Connect operation. Use structured library
   errors, including the OAuth `invalid_grant` verdict, not display-string matching.
   A network failure, HTTP 500, denied native-store access, protocol mismatch is not proof that credentials should be replaced. Return an
   appropriate failure. A verified saved credential for the wrong account is
   unsuitable for this workspace: use the existing account-constrained browser
   flow once, preserving the installed key/binding. If that new approval still
   selects the wrong account, report the mismatch without another browser loop. Permit at most one transition to fresh browser approval per
   submitted operation; a failed new authorization returns to the dialog.
5. Correcting Server on an unbound workspace supersedes the uninstalled attempt.
   Verify no installed binding references its key, remove that obsolete native entry,
   and clear its exact saved attempt before creating the new one. This may include
   saved refresh material belonging solely to the abandoned, uninstalled connection.
   A native read/removal failure stops replacement and preserves recoverable state.
   An already absent entry permits clearing the stale reference. Do not revoke any
   server client implicitly. The existing account client list owns revocation.
6. An installed or uncertain binding must be inspected and preserved; it is never
   treated as an abandoned first connection. Reauthorization keeps its account and
   writer hint. Existing exact-binding and pending-work rules remain authoritative.
7. If installation already committed, recognize its exact binding and retry only
   outstanding completion/cleanup work where identity is already established by
   that durable binding. Do not allocate another writer or repeat browser approval
   solely because obsolete-key removal or attempt clearing previously failed.

Browser authorization, callback validation, token refresh, proofs and HTTP errors
remain library responsibilities. Choosing when to invoke them, preserving account
boundaries and mapping outcomes to useful UI messages are application wiring.
No new persistent state is required. Existing unused destination correction code
may be removed if made unreachable; do not build compatibility for old dev builds.

## Observability and verification

Retain meaningful enrollment spans and session correlation. Record bounded outcomes
for refreshed authorization, browser authorization, installed connection, cancellation
and failure; distinguish installed connection from cleanup failure. Follow the
existing observability contract for metrics at the owning workflow boundary, without
new wrappers, payload dumps or duplicate library timing instrumentation.

Extend existing suites to prove: retry after a server error; saved-credential
completion without browser approval; definite credential rejection versus transport
failure; correction of an uninstalled server; populated-workspace rejection before
authorization and again at installation; cancellation and admission; account mismatch;
and cleanup retry after committed installation. Assert actual Clacks decisions,
not library internals or artificial database failure hooks.

Inspect the assembled production dialog in idle, authorizing, failed, reauthorizing
and successful states, including resizing, keyboard dismissal and expandable details.
Read the machine-local development inventory before any server/desktop launch;
use explicit isolated workspace/config/log paths and verify the process before UI
attachment. Automated tests use temporary workspaces/disposable databases. Report
Windows visual acceptance separately if unavailable.

## Implementation discretion

Helper names, error-summary wording beyond the examples above and bounded metric
names follow existing conventions. These are engineering details. No additional
product decision is required to implement this design.

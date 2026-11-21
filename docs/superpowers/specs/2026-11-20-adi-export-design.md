# Desktop ADI export MVP

[Clacks Log #25](https://github.com/jhelwig/clacks-log/issues/25) delivers an
offline desktop workflow for exporting saved contacts as Amateur Data
Interchange Format (ADIF) text files (ADI). Ordinary ADI is the default.
Parks on the Air (POTA) compatibility is an explicit file export format.
The [product contract](../../product.md#export-of-current-client-state-d210)
and [native mapping contract](../../native-model.md#101-ordinary-adif) own the
saved-state and mapping rules.

## Export setup

Open **Export ADI…** from the native **File** menu next to **Import ADI…**.
Use one setup window with **Which contacts?** and **Export format**. The scopes
use the history state at launch:

| Scope | Contacts included |
| --- | --- |
| **Selected contacts** | Active saved contacts in the explicit selection, including contacts hidden by the filter. Show the total and how many are outside the filter. |
| **Contacts matching the current view and filter** | Every active saved match across all pages of the current named logbook, **All contacts**, or **Untagged**, using the applied expression. Show the view, applied filter when present, and count. |
| **All contacts in this workspace** | Every active saved contact in the account, regardless of view, filter, or tag membership. Show the count. |

Default to **Selected contacts** when available and to the current view and
filter otherwise. An in-progress **Select all** cannot supply a partial
selection. Unapplied filter edits and the **Show selected** inspection mode do
not change the matching scope. Disable **Continue…** with an explanation when
the current scope is empty or preparation is incomplete.

The format choices are **Ordinary ADI** and **POTA-compatible**. When POTA is
selected, show its readiness under **POTA-compatible** in the **Export format**
section. For a mixed set, show the affected count and the consequence before
**Continue…** is available. For example, “3 of 24 QSOs are missing required POTA
information. Continuing will skip those 3.” **Show affected QSOs** expands to
identify each contact and what it needs. **Continue…** authorizes skipping
those QSOs and proceeds with the eligible set. When every QSO is ineligible,
show the same details and disable **Continue…**. Changing scope or format
invalidates outdated preparation and skip decisions.

## Preparation and mapping

Each prepared selection uses one consistent point-in-time local snapshot for
membership, recorded QSOs, applicable service evidence, eligible directory
snapshots, and output definitions. Include unsynchronized saves and the local
working version of conflicted contacts. Exclude pending deletions and emit a
working identity once before applying any POTA expansion. Use recorded facts
and the existing source precedence and whole-location rules. Preserve native
contacts, tag membership and synchronization state. The reviewed readiness,
file preview and output all use the same prepared facts while logging and
synchronization can continue.

- **Ordinary ADI** emits one entry per QSO and uses ordinary reference lists.
  Preserve supported historical station and confirmation information. When a
  QSO has multiple applicable contest identifiers, offer a contest context or
  allow the operator to omit the ambiguous field and report the omission.
- **POTA-compatible** groups files by own activation park and applicable
  location and expands eligible entries by contacted park. A QSO with two own
  parks and three contacted parks produces two files with three entries each.
  A QSO with no contacted park still produces an entry in each applicable file.
  Clacks requires an own park for file grouping. The receiver treats the park
  fields as optional and can ask for the park and location at upload. An
  unknown location stays separate from known locations. Assess supplied location
  evidence in its own code system before deciding whether it conflicts. An
  absent contacted park or an optional field that can be omitted does not make
  a QSO ineligible. Follow the [POTA logging rules](https://docs.pota.app/docs/rules.html#logging-requirements)
  for self-contacts and the `SUBMM` exception and the
  [POTA technical reference](https://docs.pota.app/docs/activator_reference/ADIF_for_POTA_reference.html)
  for field conventions. Mapping conflicts and read failures are export errors.
- Transliterate human-readable text best effort and report conversion or
  omission. Validate identity fields using their own rules. Preserve the
  recorded values, full callsigns and timestamps, and omit unsupported optional
  fields with a report when information is lost.

## Destination and results

For ordinary ADI, resolve required choices before opening the parented native
**Save** dialog. Stage the complete file beside its destination and publish it
only after successful preparation and writing. Use the exact path returned by
the native Save dialog. If that path already exists, confirm the exact
replacement in Clacks before publication. The picker returns only a path, so
it cannot distinguish confirmed replacement from a file that arrived after
selection. Cancellation and failure preserve the previous file.

For POTA, show planned filenames and the entry count per file after
**Continue…**. Open the parented native folder picker, including its **New
Folder** action. Write directly into the selected folder. Each filename
includes the activation park, applicable location, and a shared Coordinated
Universal Time (UTC) timestamp for the export, for example
`US-0011-US-WA_20261120T235942Z.adi`. Stage all files before publication and use
no-overwrite publication for each file. Report collisions with the option to
choose another folder. If publication fails partway through, report the files
this attempt already published. Leave completed files at their destination
paths, where another process may already have moved or replaced them. The
final publication step finishes or reports failure once it starts.

After the export ends, **replace setup with a results view in the same Export
ADI window**. Show the destination, distinct QSOs exported and skipped,
entries and files written, and conversion or omission details. For a failure,
show the actionable cause and any files published before the failure. Retain
results until **Done**. Cancelling a native picker returns to the preceding
setup or file review view with the prepared data intact.

## Implementation approach and verification

Reuse the local read snapshot, saved history scope, effective-source
selection, account output profiles, and the existing reference planner.
qso-io 0.3.0 owns ADI serialization. Clacks owns field mapping, readiness,
user choices, staging, and destination protection. The native save and folder
pickers express the different destination shapes. Direct folder publication
supports the operator's chosen folder and requires per-file protection.

Trace preparation and publication as related phases of the same export. Use
aggregate counts and bounded format/outcome categories to distinguish planned,
skipped, and published work. Keep paths, callsigns, free text, and contact
contents out of telemetry. The user-facing result can show paths and affected
contacts. Summarize work per phase and avoid per-QSO telemetry.

Verify the native menu and picker flows, the readiness message at the POTA
format option and skip behavior, and the persistent result in the same window.
Exercise saved-state and fallback rules, mapped ADI output, the multi-park
case, cancellation and existing-file protection, and the existing ADI import
workflow after the qso-io upgrade.

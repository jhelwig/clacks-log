# Product scope and operating workflows

Clacks Log should replace the everyday shack logger while supporting independently writable offline logs on any number of clients. Fixed and portable operation use the same product model.

## Product direction

- Native, graphical desktop operation; Windows is the initial operating environment.
- Rust wherever practical. No desktop toolkit, local database, server database, wire protocol, or cryptographic library has been selected.
- Dense, useful operating context and efficient use of desktop space. Avoid interfaces that resemble a simplified mobile application inside a desktop window.
- Integrate with specialist applications rather than becoming an all-in-one operating suite.
- Self-hostable synchronization and integration services, with an optional hosted subscription offering.
- Source-available licensing is under consideration; exact terms remain open.
- Working name: **Clacks Log**, a Discworld reference. The name is provisional.

## Live contact capture


Hardware and application details in the original handoff are historical context. They do not require installing the synchronization service on a particular existing machine or fixing the deployment topology to this inventory.

## Current FT8 workflow

WS Suite is configured to log completed contacts automatically through its UDP interface. LOG4OM ingests contacts as they are published. GridTracker 2 supports the operator's station-selection workflow.

Clacks Log's agreed scheduling is in [External logbook services](integrations.md).

GridTracker's Generate Messages action reaches Clacks through WS status updates; D115 defines the resulting callsign, directory-lookup and Worked Before behavior.

## Logging defaults and saved location profiles

Current logging defaults belong to each client independently. Saved location profiles synchronize across clients. A shack client can keep Home defaults while another client operates from a park; changing one client's current defaults must not change another client's defaults.

Each client has one current operating context. Multiple operators each use their own client; managing multiple simultaneous operating contexts within a client is outside the design.

Profiles are optional conveniences for recurring locations. The operator can enter a one-off location without first creating a profile. Repeated locations may include multiple POTA references, such as a 2-fer.

Station identity belongs to the location profile. Operator identity is distinct and must be easy to set or clear in the current operating context. Whether an operator value is saved within a profile remains unspecified; do not infer it from the station callsign.

QSOs capture their operating context as snapshots at logging time. Editing a saved profile or current defaults does not change previously recorded contacts. Corrections to existing QSOs are explicit edits; bulk editing is the intended interface for applying the same correction to multiple records, with release scope still to be defined.

Logging does not require an operating-session start/end lifecycle. Current defaults can remain active between intermittent logging activity. Session-oriented organization must not become a prerequisite for logging.

Loading a profile copies its values into that client's current defaults. Later profile edits, including synchronized edits, do not refresh those defaults automatically; the user must explicitly reload the profile. The interface must clearly indicate when current defaults differ from the associated profile, whether because of local adjustments or a newer saved profile revision. The exact indication and reload interaction remain to be designed.

Compare current defaults against the latest saved profile values. Retain the last-loaded profile revision and its values as the comparison baseline: defaults versus that baseline identify local adjustments, while the latest saved profile versus that baseline identifies changes to the profile. Both kinds of change can exist simultaneously. When offline, latest means the latest saved profile revision available to that client; unseen server updates cannot be reflected until synchronization.

The operating-context editor should not permanently occupy desktop space. Profile synchronization and edited state must remain visible even when the editor is closed. The OrcaSlicer saved-preset versus current-values model is a useful interaction reference, but exact layout and docking are deferred.

The remaining profile fields and profile-edit conflict handling remain open.

## Worked Before

- A today indicator uses the **UTC calendar day**, not a rolling 24-hour window.
- Prior contact information must not be restricted to today.
- Show available grid and POTA reference information so multiple park activations by the same operator remain distinguishable.
- Selection-driven access from the existing operating workflow matters. The exact GridTracker-to-logger interface and UI layout remain to be specified.

## MVP boundaries

Established MVP scope includes independent offline QSO use, reconciliation, WS Suite logging integration, and LoTW and QRZ publication/confirmation workflows. The Worked Before workflow is a central operating requirement; its detailed first-release interaction design is not complete.

Certificate import is in MVP; certificate requests and renewals are later scope. A server connection is required for certificate import.

Automatic catch-up of contacts missed while Clacks Log was not receiving WS UDP messages is outside MVP. This does not settle manual ADIF import scope or change the requirement for durable client-to-server synchronization of contacts already captured by Clacks Log.

eQSL is relevant to the existing workflow but has not been committed to MVP. General migration/import/export requirements, bulk editing, backup/export UX, and other ordinary logger functions still need explicit scoping rather than assuming every incumbent capability belongs in the first release.

## Later interests

Heard-station views and award tracking/hunting are explicitly **not MVP**. Future interests include wanted-station handling similar in clarity to GridTracker 2, ARRL award opportunities, and JARL district/prefix tracking. Do not assume GridTracker 2 can ingest external award-interest information; that capability remains to be verified.

## Research use

The attached qualitative research brief supports product discovery. Its examples are not a representative ranking of demand or a product specification. Preserve the distinctions between actual user workflows, other operators' accounts, documented software behavior, and proposed Clacks Log features.

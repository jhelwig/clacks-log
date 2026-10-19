# Issue #1 native-model consolidation index

The issue #1 design discussion has been consolidated into the
[written native-model specification](../superpowers/specs/2026-10-18-native-model-design.md),
dated 18 October 2026. The user then requested the
[implementation plan](../superpowers/plans/2026-10-18-native-model.md), now
executed with [verification evidence](../native-model-execution.md). This file
remains an index, not a parallel specification. The initial implementation review and its fixes are recorded in the evidence; the later D256 correction is now included in [PR #39](https://github.com/jhelwig/clacks-log/pull/39). The [adversarial panel review](../adversarial-native-model-review.md) records the user-authorized resolution of its three P2 findings.

A radio contact is a QSO. Minimum viable product is MVP. Amateur Data
Interchange Format is ADIF. Parks on the Air is POTA.

## Accepted decisions carried into the specification

- [Contact inventory and required values](../superpowers/specs/2026-10-18-native-model-design.md#4-contact-schema-and-saved-record-invariants):
  both station callsigns; whole-second start/optional end; mode; frequency or
  band; optional receive override; optional reports, operator information,
  milliwatt power and copied equipment; the complete tag set. An absent receive
  override uses the primary pair. A band-only override never borrows frequency.
- [Scalar normalization](../superpowers/specs/2026-10-18-native-model-design.md#5-scalar-values-normalization-and-limits):
  callsign trim/uppercase with slash components preserved; optional blank text
  absent and other supplied text preserved; omitted import fields remain
  distinct from selected explicit clears. Incomplete entry/import preparation
  creates no saved contact before validation; this is established workflow.
- [Location representation](../superpowers/specs/2026-10-18-native-model-design.md#6-positions-and-geographic-details):
  grid area or explicit point, coordinates in integer millionths of degrees,
  locator lengths 2 through 12 in pairs, imported two-character areas retained,
  four-character minimum for own operating-location entry. Preserve point/area
  meaning, imported coordinate authority, corrections and whole-location fallback.
- [Mode and band recognition](../superpowers/specs/2026-10-18-native-model-design.md#7-bandfrequency-and-mode-interpretation):
  complete ADIF 3.1.8 catalogs, deprecated mode labels for historical data,
  generic mode hierarchy and preserved labels, with data-driven mode/submode/band interpretation (D256).
  Recognition alone does not change recorded values or synchronization equality.
- [References and groups](../superpowers/specs/2026-10-18-native-model-design.md#8-participationreference-associations):
  own/contacted unordered sets, each association group ID plus optional identifier.
  Each contest/event is a peer group. Group-defined normalization, syntax and
  identifier requiredness; preserved unknown associations. No contact-wide
  scope, named-field reference map or contest-exchange fields.
- [Group/catalog capabilities](../superpowers/specs/2026-10-18-native-model-design.md#9-group-definitions-and-optional-catalogs):
  seed default definitions into accounts and synchronize as ordinary shared data;
  full default/user-defined parity with no group management in MVP. Catalog
  metadata remains separate from QSOs. Stable catalog ID, obtained_at and optional
  upstream_version; no separate catalog version scheme.
- [Export context](../superpowers/specs/2026-10-18-native-model-design.md#10-export-context-and-mapping-contracts):
  output profile plus optional target group, independent of contact selection;
  ordinary ADIF versus POTA submission mappings; explicit per-QSO contest
  ambiguity handling and definition errors under D255. POTA expansion changes
  output only and does not multiply unrelated references.
- [Profiles/equipment/tags](../superpowers/specs/2026-10-18-native-model-design.md#11-profiles-equipment-and-logbook-tags):
  profile loads replace the whole station snapshot and clear absent values;
  operator/power stay unchanged. No equipment default leaves current equipment
  unchanged. QSO copies equipment values without its display name. Preserve
  stable account-owned QSOs, tag memberships, tri-state service policies,
  explicit deletion choices and whole-record reconciliation.

## Review and remaining boundaries

The specification labels engineering choices implemented through the authorized
plan, including Universally Unique Lexicographically Sortable Identifier (ULID)
selection, coordinate tie-rounding/validation order and declarative rules. They
are not backdated as individually approved product decisions. Implementation
and review evidence distinguishes library behavior from future adapters.

Import-side station assignment for ADIF CONTEST_ID has not been approved; it
belongs to import mapping work. Catalog acquisition/update/distribution, complete
provider/ADIF adapters, physical storage and synchronization mechanics have their
own specifications and backlog items. Dedicated contest exchanges and serial
assistance remain post-MVP. Do not generate another product-decision queue from
ordinary implementation details.

The [organizer-rule survey](../../references/contest-exchanges.md) and
[logger-behavior survey](../../references/contest-logger-data.md) remain research
evidence. They do not expand the accepted MVP scope or prescribe physical storage.

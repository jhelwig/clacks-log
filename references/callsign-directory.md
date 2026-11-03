# Callsign-directory research

D88 includes directory lookup during manual radio contact (QSO) entry in minimum viable product (MVP). D89 selects QRZ and HamQTH with an account-level user-selected provider (D196). The [account contract](../docs/accounts.md#client-direct-directory-lookup) and [desktop guide](../lib/clacks-desktop/README.md#direct-directory-lookup) own the implemented field/lifecycle behavior. The research below records primary documentation checked 2026-10-12; it does not establish authenticated live-provider interoperability or comparative coverage.

## QRZ

[Extensible Markup Language (XML) interface specification](https://www.qrz.com/docs/xml/current_spec.html) documents callsign-directory access authenticated with QRZ username/password and a session key. Most features require an active XML Logbook Data subscription; non-subscriber data is limited. [Logbook API](https://www.qrz.com/docs/logbook/QRZLogbookAPI.html) instead uses a logbook-specific API access key. Supporting QRZ uploads does not by itself establish directory authentication/access.

## HamQTH

[frequently asked questions (FAQ)](https://www.hamqth.com/faq) describes a free callbook. [Developer documentation](https://www.hamqth.com/developers) documents authenticated directory lookup, including name/station location (QTH), station DX Century Club (DXCC), zones, location/grid, and administrative-region fields. Current documentation recommends API v2 for new development; legacy XML remains supported. Portable callsigns without their own record may fall back to the home record, so directory location must not automatically be interpreted as the actual QSO location. Postal-address DXCC and station DXCC can differ; privacy-hidden fields may be unavailable.

## Accepted lookup model

D204 keeps all supported directory information separate from recorded QSO facts, including names. Retain/synchronize the separate per-QSO directory snapshot with provider and lookup time under D203/D204. Use recorded QSO information when present, otherwise directory information, including for supported export/upload fields. For location, D201 uses the selected source as a whole rather than filling gaps from another location. This supersedes D90/D95/D99's former directory-to-QSO fill-empty behavior.

D91 triggers lookup before WS Suite completion, with D94's 500 ms debounce. D92 defers outbound WS DX-call control; local Clacks edits drive lookup only in MVP. D95/D204 allow late results to update the original contact's separate directory snapshot, without changing recorded QSO information. Preserve original-contact/request correlation and D116 stale-result suppression. D197 selects one provider in MVP, defers automatic fallback, and excludes provider-result merging. D195 governs rejected credentials. Manual-entry implementation supports returned identity, name, location/precision metadata, entity/subdivisions/zones, IOTA and locality. QRZ station DXCC is distinct from postal DXCC; HamQTH uses its station DXCC. Account definitions normalize new reference values when available. Bulk lookup and WS Suite integration retain their separate delivery scope.

Directory data is not confirmation of a specific QSO. Provider contact confirmation (QSL) preferences/participation indicators do not constitute a positive QSO confirmation under D52.

## Credentials and cache checkpoint

D97 accepts account-level web configuration and secure credential provisioning to clients for protected local storage. D98 excludes persistent local caching of queried callsign results from MVP without a future commitment, and includes bulk lookup/population for contacts captured without provider access. QRZ XML documentation checked 2026-10-12 explicitly recommends caching/reusing authentication session keys; that passage does not establish callsign-result retention terms. A future cache, if proposed, would need its own scope and provider-term review; it is not an accepted backlog commitment. D99 settles user-triggered selected/filtered bulk lookup; exact snapshot application and review mechanics remain open.

D99 selects user-triggered bulk lookup on selected/filtered QSOs with partial-failure reporting. D204 applies results to separate directory snapshots, not recorded QSO fields. D205 applies results as they arrive and reports outcomes afterward, without a preview/approval stage. D206 selects whole-snapshot replacement on success, preservation on failure/not found, and completed-update retention on cancellation. Exact supported fields and report presentation remain open. This does not select automatic bulk recovery on reconnect or persistent directory caching.

## Directory authentication failures (D195)

Rejected credentials pause lookup for the captured current material with a clear diagnostic. Updated credentials or explicit retry resume lookup. Session expiry renews once and does not itself pause credentials; late rejection of replaced material cannot pause its replacement. Network/provider failure preserves successful facts, and provider retry timing gates new requests without scheduling automatic retries. Logging and synchronization continue normally. D99 bulk lookup remains separately scoped; authentication recovery does not trigger automatic bulk catch-up.

## Provider-selection scope (D196)

The provider choice is account-level and shared by enrolled clients. Lookup requests remain client-direct. A client without synchronization uses its last received selection until it receives the account change on reconnect. D197 defers automatic provider fallback to post-MVP and excludes provider-result merging. The web account page owns configuration; changes control newly admitted requests. Already-started responses keep their original contact/key eligibility, with no provider-switch-specific display rule.

## Provider fallback and merging (D197)

MVP queries only the account-selected provider, including when it fails or has no callsign result. Automatic fallback to another provider is post-MVP. Merging results from multiple providers is excluded from the product direction, not a later-release feature. D204’s separate snapshots/logical fallback and contact-specific confirmation enrichment are distinct from combining directory-provider results. Transient-failure retry details remain open.

## Geographic precision finding — checked 16 October 2026

The [QRZ interface specification, geographic-coordinate discussion](https://www.qrz.com/docs/xml/current_spec.html) says returned coordinates may be user-supplied, derived from a supplied grid, geocoded from a postal address, or approximated from a postal code, state, or DX Century Club entity center. The `geoloc` field distinguishes these sources. A returned numeric coordinate therefore does not by itself establish an explicitly supplied station position. D198 keeps all such directory location information separate from QSO location, rather than using precision/source categories to decide automatic QSO population. D184 area-versus-point semantics do not make a directory location evidence of the contact location.

The [HamQTH developer documentation](https://www.hamqth.com/developers) also documents portable-call lookup returning a home record when no separate portable record exists. Provider record identity and supplied-contact location must remain distinct. D198 now establishes the separate directory-location boundary. No authenticated lookup was performed.

## Directory location versus contact location (D198)

Directory location describes the directory record, not inherently the station's location during a particular contact. Keep it separate from the QSO's contacted-station location, including grid, coordinates, locality, and location-derived administrative/entity/zone details. Do not automatically copy these into QSO location fields during entry, late lookup completion, or user-triggered bulk population, even when those fields are empty. Neither user-supplied directory coordinates nor greater precision changes that separation.

D204 extends separate storage and logical fallback to all directory information, including names; the earlier directory-to-QSO fill-empty behavior no longer applies. Contact-specific confirmation enrichment under D33-D36 is a separate source/workflow. D199 selects a logical fallback instead of explicit copying: use QSO location when present, otherwise available directory location. D200 includes exports and external uploads in fallback use. D201 selects whole-location precedence, including partial QSO locations, without filling gaps from directory fields. D203 selects retaining/synchronizing a separate directory-location snapshot with each QSO. Exact presentation and calculation details remain open; D98 still excludes a reusable persistent lookup cache for future contacts.

## Logical location fallback (D199)

Keep the QSO's recorded location and directory-provided location separate. Where location is needed, use the QSO location when it specifies where the contact happened; otherwise use directory location when available. This is a logical fallback, not copying into the QSO's recorded location, and does not need an explicit user-copy action. Keep the source distinction available. Provider-result merging remains excluded under D197.

D201 settles partially specified QSO location: use it as a whole without filling gaps from directory data. D203 selects retained/synchronized per-QSO directory-location snapshots, distinct from a reusable lookup cache under D98. Exact geographic-calculation details remain open.

## Export and external-upload fallback (D200)

Exports and external-service uploads use the same logical location fallback: QSO location when present, otherwise available directory location. This is an output transformation, not copying into native QSO location. Keep native sources separate; ordinary interchange/provider output may not retain that distinction under D185. Service-specific supported fields and validation still apply. D201 selects whole-location precedence for partial locations as well. D203 retains/synchronizes directory-location snapshots with their QSO; consequences of changed effective output for upload scheduling remain open.

## Whole-location source selection (D201)

Use the QSO's location as a whole whenever present, including a partially specified location. Do not fill missing grid, coordinates, locality, or administrative/entity/zone fields from directory location. Derive geographic details from the selected QSO location where existing precision/ambiguity rules permit; otherwise leave gaps unknown. Only when QSO location is absent does the separate directory location become the fallback. Apply the same source selection to exports and external uploads. D203 selects retained/synchronized per-QSO directory-location snapshots; a reusable persistent lookup cache remains excluded under D98.

## Directory lookup callsign normalization (D202)

Normalize the directory request key separately from the QSO's recorded callsign. For example, recorded `OK2CQR/P` is queried as `OK2CQR`; the recorded worked callsign stays unchanged. The fact that a provider accepts a portable form does not require Clacks to query it in that form. Never replace the recorded worked callsign with a returned directory identity.

Keep lookup responses associated with their original contact/request and retain existing stale-result protection even when different recorded callsigns normalize to the same request key. Exact normalization for other forms and any alias-provenance retention remain open; do not assume every slash-separated component can be discarded. A separate returned-callsign snapshot field is not required by the portable-suffix example. D203 accepts the per-QSO directory-location snapshot including provider and lookup time, without requiring a returned-callsign field.

## Retained directory-location snapshot (D203)

Retain a separate directory-location snapshot with each QSO, including provider and lookup time, and synchronize it with the contact. This makes fallback available offline and to the server-owned upload process without another provider request. Keep the snapshot distinct from recorded QSO location and apply whole-location precedence under D201. Later changes at the provider do not silently update existing snapshots.

D98 still excludes a reusable persistent callsign-result cache for future lookups. A retained snapshot is part of its associated contact data, not a cache entry used to satisfy unrelated lookup requests. A provider-returned callsign is not a required snapshot field under this decision. D206 selects snapshot replacement/failure/cancellation behavior. Exact encoding and consequent upload scheduling remain open.

## All directory information remains separate (D204)

The source separation applies to names and other supported directory information, not just location. Extend the synchronized per-QSO snapshot under D203 accordingly, retaining provider and lookup time. Recorded QSO information takes precedence when present; directory information supplies a logical fallback without being copied into recorded QSO fields. D201 whole-location selection remains unchanged.

Bulk lookup retrieves separate directory information for selected/filtered contacts. Recorded manual, captured, imported, or contact-specific information is not a target of this operation. D205 applies results as they arrive, without preview/approval, and reports outcomes afterward. D206 settles snapshot replacement/failure/cancellation behavior. D98 still excludes a reusable persistent lookup cache, and D197 still excludes merging directory-provider results.

## Bulk lookup application and reporting (D205)

For selected/filtered contacts under D99, apply directory results to their separate snapshots as results arrive. There is no preview/approval stage. Recorded QSO facts remain untouched under D204; D209 synchronizes snapshots with automatic latest-successful-lookup arbitration, independently from user conflict resolution for recorded QSO facts.

Provide a clear report afterward distinguishing new/updated snapshots, materially unchanged results, no directory record, failures with reasons, and contacts not processed. D206 settles whole-snapshot replacement on success, preservation on failure/not found, and keeping completed updates on cancellation. Exact implementation and report presentation remain open.

## Snapshot replacement and cancellation (D206)

Successful lookup replaces the separate directory snapshot as a whole, including provider and lookup time. Fields absent from the new successful result are absent from the replacement; do not keep stale fields from the prior result. Failure or callsign not found leaves any existing snapshot unchanged and is reported. Cancellation preserves completed updates, stops further application at a safe boundary, and reports remaining contacts unprocessed. Recorded QSO facts remain untouched, and snapshot updates follow normal synchronization/conflict rules. Exact transaction/cancellation implementation and report presentation remain open.

## Callsign changes and snapshot association (D207)

Retain a QSO's snapshot when its recorded callsign changes but its normalized directory lookup key does not. If the key changes, stop using the old snapshot immediately and look up the new station. A failure/not-found result for the new station does not make the old snapshot usable again. D206's preservation rule applies only to snapshots associated with the current key.

Validate original-QSO/request correlation and current normalized-key association before applying a delayed result. A response for a previous key cannot restore obsolete fallback, including in exports and server-owned uploads. The lookup association must remain available with synchronized snapshot data; it is the normalized request key, not a requirement to store the provider-returned callsign. Exact representation, invalid-snapshot retention, and request lifecycle mechanisms remain open.

## Clearing recorded information and fallback (D208)

There is no explicit “leave this unknown despite directory information” control in MVP. Clearing recorded QSO information exposes the applicable directory fallback under D204. D201 still selects location as a whole. A future suppression control is undecided and would be post-MVP if ever added; it is not a committed backlog feature.

## Latest successful lookup arbitration (D209)

For each QSO and normalized directory lookup key, retain the complete successful snapshot with the greatest ordering pair: successful lookup time recorded by the fetching client, followed by a stable unique tie-break identity when times are equal. Preserve this metadata across replication and retries; server arrival time does not establish freshness. A retransmitted snapshot is the same lookup, not a newly timestamped result. The exact time representation and tie-break encoding are unselected.

This is the published last-writer-wins register model: one complete value is selected by a shared total order, without user resolution or field merging. See [Preguiça, Conflict-free Replicated Data Types: An Overview, sections 2.1 and 2.1.1](https://arxiv.org/html/1806.10254#S2.SS1.SSS1). A register here means one replaceable stored value. In Clacks, this arbitration is scoped to provider-supplied directory snapshots, not recorded QSO facts. Clock skew can select a lookup that was not actually the freshest; it does not prevent all replicas selecting the same winner after receiving the same snapshots. The user accepts this freshness tradeoff for directory data; no new clock-synchronization service is required by D209.

Failure/not-found does not compete as a replacement value. Whole-snapshot replacement follows D206, including absent fields in the winning successful result. QSO/key association follows D207: old-key snapshots cannot supply current fallback or overwrite the new-key snapshot. Snapshot selection is independent of recorded-QSO conflict detection, so a report edit and directory refresh alone do not require resolution. Parent QSO/logbook deletion still takes precedence; a snapshot cannot restore a deleted parent. This adds no synchronization scope or permanent per-refresh history requirement.

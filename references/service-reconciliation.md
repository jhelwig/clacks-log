# Downloaded contact-detail reconciliation

Research checkpoint: 12 October 2026. This is a focused start to the incumbent-behavior survey, not a comprehensive survey or selected Clacks Log policy. No installed logger was exercised.

## DXKeeper: documented choices

[Official contact confirmation (QSL) configuration help](https://www.dxlabsuite.com/dxkeeper/Help/QSLConfiguration.htm), Logbook of The World (LoTW) tab, documents configurable handling of differences in country code, CQ magazine zones and International Telecommunication Union (ITU) zones, grid, Islands on the Air (IOTA), state, and county. Options replace local values, preserve them, preserve and report differences, or present both values for operator selection. A separate subsquare option exists in DXKeeper; this does not establish a need for such a distinction in Clacks Log.

This demonstrates established configurable reconciliation rather than one universal rule that downloaded confirmation details always replace local values. It does not establish which rule users generally prefer.

## LOG4OM: limited evidence

Jacob has observed updates to local county, DX Century Club (DXCC), and ITU information from downloaded confirmations and an option to add unmatched incoming contacts. These remain user observations; exact settings, field replacement behavior, and precedence have not been verified in the installed version.

The [LOG4OM 2 manual](https://www.log4om.com/l4ong/usermanual/Log4OMNG_ENU.pdf) indexed documentation describes confirmation downloads and optional creation of unmatched radio contacts (QSOs). The complete Portable Document Format (PDF) could not be fetched by the web reader because it exceeded its size limit. No exact automatic replacement policy is claimed from that limited documentation access. Older forum discussion and LOG4OM 1 documentation do not settle current behavior.

## Current Clacks Log direction (D237)

D32 separates remote observations from recorded radio contact (QSO) facts. D237 extends the directory source-separation principle to confirmation downloads: preserve manually recorded information and retain provider details separately. It supersedes D33–D36's automatic replacement of recorded values and the treatment of that replacement as a QSO edit. This applies across fields; Islands on the Air (IOTA) references are not a special replacement exception.

Logbook of The World (LoTW) remains above QRZ for provider fallback. D238 selects logical fallback in this order: recorded QSO information, eligible LoTW information, eligible QRZ information, then the eligible directory snapshot. Select location as a whole from the first available source, including partial recorded locations; do not mix location fields across sources or change recorded QSO facts. Apply this to display and supported export/upload fields, retaining provenance and existing association/applicability requirements. Exact field coverage and representation remain engineering work. Do not assume receipt of a confirmation validates every supplied detail for a separate award programme. Our station's operating context remains protected.

Retain duplicate-safe provider observations and associations. Repeated delivery must not become a new event or undo later work. Exact provider update identity and evidence reconciliation remain engineering work; provider evidence is distinct from user-edited contact facts.

## Unmatched downloads (D37)

Require explicit user confirmation to add the QSO locally or reject it locally. Do not automatically create a local QSO or delete the external record. Under D218, rejection applies to the version the user reviewed. Identical repeat delivery stays suppressed; a changed downloaded version receives normal matching/review and, if still unmatched, the explicit add/reject choice again. This does not automatically add a contact or override D54 deletion memory. Exact provider update identification/comparison and any explicit reversal of an unchanged rejection remain separate specification/scope matters.

## Download association matching (D227)

Established associations take precedence. For an unassociated LoTW or QRZ download, search all account QSOs, including untagged contacts and those without automatic-upload eligibility. Reuse D41’s precision-aware confident matching and D225/D226’s compatible incomplete-identity and 30-minute-either-side possible matches. A unique confident match can be applied automatically; possible matches require explicit selection even when there is only one. Explain missing information and time differences. Multiple matching contacts remain ambiguous: explicitly select at most one QSO under D251 for the external record within that account/service destination. A QSO may separately associate with records in other services/destinations. Do not fan out observations to local duplicates.

If no candidate is selected, retain explicit add/reject under D37/D105, with selection of the tag to apply when adding. D218 scopes rejection to the reviewed version, and D54 known-deletion suppression remains applicable. Distinct contacts may share matching fields; matching fields are not a uniqueness constraint on the QSO store. D251 separately requires uniqueness of the external-record association per account/service destination; a duplicate cannot inherit an association already held by another QSO or silently publish over its external record. Exact compatibility tables, provider identity normalization and precision-aware boundary handling remain specification work. Association to a local contact is distinct from the provider’s matching of the two operators’ reports and from D124’s applicability of historical confirmation evidence after a local correction.

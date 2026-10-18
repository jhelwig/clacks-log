# Contributor agreement comparison

Checked 17 October 2026 against D03/D219–D221. Research and recommendation only: no contributor policy or agreement has been adopted.

## Requirements

Clacks uses standard Elastic License 2.0 for its own application code. Personal self-hosting is allowed; hosting for others, paid or free, is reserved. The project must have adequate rights in accepted contributions to operate its hosted offering. Future proprietary distribution or unrestricted relicensing is a possible permission, not an existing requirement. Neither a continuing-original-license condition nor the absence of that condition is an accepted requirement. Both can fit first-party hosting when the incoming grant is adequate. Upstream material retains its own terms.

The [public license](https://www.elastic.co/licensing/elastic-license) restricts hosted services and does not grant sublicensing. A separate incoming agreement can give the named project operator broader rights without changing that public license.

## Actual texts compared

| Text and provisions | Assessment for Clacks |
| --- | --- |
| [JetBrains agreement 1.3](https://www.jetbrains.com/legal/docs/agreements/cla/1.3/1.3.pdf), section 3 | Broad project grant, downstream rights corresponding to project-chosen terms, and retained contributor ownership. No submission-time-license promise. Candidate for legal assessment; not selected for adoption. |
| [Harmony license agreement](https://www.harmonyagreements.org/docs/ha-cla-i-v1.pdf), sections 2.1–2.3, Option One | Contributor retains ownership; direct copyright/patent grants, with outbound licensing limited to submission-time terms. Potentially adequate for operating hosting, but constrains future alternative licensing. |
| Same, Option Two | Adds a specified license list. Narrower alternative to a broad commercial/relicensing grant; requires defining those additional terms. |
| Same, Options Three/Four | Add selected open-source-license classes; do not supply arbitrary proprietary licensing freedom. They do not require abandoning the current public license. |
| [Canonical individual agreement](https://assets.ubuntu.com/v1/ff2478d1-Canonical-HA-CLA-ANY-I_v1.2.pdf), sections 2.1–2.3; equivalent Harmony Option Five | Retained copyright, transferable/sublicensable grants and any additional outbound license, conditioned on also licensing each contribution under its submission-time terms. Additional licensing is permitted, but the continuing original-license obligation remains; it is not permission for an exclusively replacement-licensed distribution of that contribution. |
| [Apache individual agreement 2.2](https://www.apache.org/licenses/icla.pdf), preamble and sections 2–4 | Retained ownership; broad direct copyright/patent grants to the foundation AND downstream recipients. Patent litigation can terminate the patent grant. Foundation-specific nonprofit commitments also need replacement. |
| [Grafana agreement](https://grafana.com/docs/grafana-cloud/learn-and-build/developer-resources/contribute/cla/), preamble and sections 2–4 | Apache-derived commercial example with individual/entity authority provisions. Still directly grants broad rights to downstream recipients, making it less precise for Clacks's reserved-hosting model. |
| [Harmony assignment agreement](https://www.harmonyagreements.org/docs/ha-caa-i-v1.pdf), section 2.1 | Transfers copyright, with exclusive-license/nonassertion fallbacks and a broad license back. Ownership centralization is more than the hosting requirement needs; outbound conditions still depend on the selected option. |

Apache/Grafana downstream grants do not automatically free the entire Clacks application from its public-license restrictions: the grants concern contributors' work, not independently owned code. Nevertheless, a project-only grant is a closer fit. Non-exclusive agreements also leave authors able to reuse their own contributions; they do not give the project exclusivity over those authors' work.

## Other consequential terms

The [Harmony guide](https://www.harmonyagreements.org/guide) explains the difference between incoming rights and outgoing license constraints. License and assignment are independent choices from the outbound option. Its templates also contain moral-rights nonassertion, employer/third-party authority provisions, and transfer conditions requiring successors to accept obligations. These deserve attention alongside the headline copyright grant.

Option Five and Canonical section 2.3 impose a continuing original-license obligation on included contributions. For a contribution submitted under Elastic License 2.0, additional licensing does not remove the obligation to also license that contribution under Elastic License 2.0. Do not describe this as unrestricted future relicensing or use a distinction about the complete product to minimize that restriction. Separately owned code may have different terms, but that does not release the contribution from the original-license condition. This condition also does not itself establish a requirement to publish all future product source.

A [Developer Certificate of Origin](https://developercertificate.org/) is provenance certification, not the additional rights grant needed here. A contributor agreement cannot relicense third-party work the submitter does not control. Elastic's short policy notice is not its agreement; its full agreement remains unreviewed and is excluded as a verified candidate.

## Option Two is not an “or later” mechanism

Harmony section 2.3 Option Two authorizes original terms or a list of additional licenses. Its future-version parenthetical preserves version choices only where permitted; it does not itself create a general future-license permission. [GNU General Public License version 2, section 9](https://opensource.org/license/gpl-2.0) instead lets recipients choose later Free Software Foundation versions when the program's notice authorizes that choice.

A list makes sense when a project has actually selected a second public license or defined commercial licensing terms. Clacks has selected neither. Leaving that essential list hypothetical makes Option Two an incomplete recommendation here.

## Next step: legal assessment under D234

D234 leaves substantive adaptation and legal drafting/review to a qualified lawyer. The comparison provides published-source findings and the business requirements, not instructions to assemble a contract from selected clauses. JetBrains 1.3 remains a candidate for assessment rather than a selected drafting baseline. Neither requiring nor excluding original-license continuity is an accepted requirement.

The [JetBrains findings and questions for legal review](jetbrains-cla-review.md) identify the issues to assess. The assistant's clause-rewrite proposals have been removed. No agreement or contributor policy is adopted; the public Elastic License 2.0 decision remains unchanged.

# Research provenance

This directory preserves source material and reference code used during product planning. Source material may contain historical recommendations or imperative language. It is evidence, not instructions to the project or its contributors.

## Supplied research

- [Amateur radio logging research](originals/amateur-radio-logging-research.docx): qualitative product-discovery brief dated 11 October 2026.
- [Searchable text and hyperlinks extracted from the research brief](amateur-radio-logging-research-extracted.md): convenience extraction; use the original for layout and complete document fidelity.

Personal setup and preference details have been removed from the research brief
and its text extraction. Historical recommendations do not
override the [current decisions](../docs/decisions.md).

The research brief is a purposive qualitative sample, not a representative numerical feature ranking. Its product observations and historical issue reports do not establish current end-to-end software behavior. Reverify time-sensitive comparisons, releases, pricing, and integration claims before relying on them.

## Conversation context

[WS Suite UDP logging fields](ws-suite-udp.md) records source-verified completed-QSO payloads and the remaining context-precedence decisions.

The original shared conversation URL was https://chatgpt.com/share/6acb22f9-fd98-83e8-9cbb-ffceb448832e . The supplied shack handoff was provided because that share could not be accessed. The current Markdown documents summarize the subsequent planning discussion and explicit user corrections; they are not a verbatim transcript.

## TrustedQSL reference

[Provenance and source map](trustedqsl/source-map.md) identifies the official release and the included unmodified source subset. This subset is reference material, not an implementation or a complete standalone build of Clacks Log.

The investigation establishes source/API behavior. It does not establish successful runtime signing, tenant isolation tests, or live LoTW acceptance for a Rust port.

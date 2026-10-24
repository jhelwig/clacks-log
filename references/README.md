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

[WS Suite (the WSJT-X family of weak-signal radio applications) User Datagram Protocol (UDP) logging fields](ws-suite-udp.md) records source-verified payloads for completed radio contacts (QSOs) and the remaining context-precedence decisions.

The current Markdown documents summarize planning decisions; they are not a
verbatim conversation transcript.

## Amateur Data Interchange Format (ADIF) import evidence

[LOG4OM ADIF sample findings](log4om-adif-import.md) inventories the supplied 12 October export and distinguishes standard fields, useful private metadata, and unresolved import policy. The personal export itself is not stored in this repository.

## TrustedQSL reference

[Provenance and source map](trustedqsl/source-map.md) identifies the official release and the included unmodified source subset. This subset is reference material, not an implementation or a complete standalone build of Clacks Log.

The investigation establishes source/application programming interface (API) behavior. It does not establish successful runtime signing, tenant isolation tests, or live Logbook of The World (LoTW) acceptance for a Rust port.

## Distributed replication

[Comparative replication research](replication-research/README.md) surveys causal metadata/membership/garbage collection (GC), conflict models, and reconciliation/recovery under D135. conflict-free replicated data types (CRDTs)/vector clocks are examples, not an exhaustive research boundary. The previously proposed revision IDs/parent links/base representation is set aside; research findings do not select an implementation.

D150 redirects this work to the [replication approaches survey](replication-research/approaches-survey.md): data models, structures, reconciliation/convergence, and failure/retention assumptions, with software only as case-study evidence. Superseded research/recommendations have been removed; the current reports retain primary-source citations.

## Server persistence comparison

[Server persistence comparison and recommendation](server-persistence.md), checked 16 October 2026, compares embedded relational, relational-service, transactional key-value, document, persistent data-structure, distributed transactional, and file/event-log approaches against D123 and the accepted transaction, concurrency, deployment, backup and older-server recovery requirements. MySQL and MariaDB are outside the selected storage stack. It recommends PostgreSQL with database-backed durable work and documented recovery; D213 accepts PostgreSQL for the server and embedded SQLite for the client. Detailed mechanisms remain proposals, not a replication-model replacement or implementation plan. Primary database documentation supports capability findings; no workload benchmark, database setup or live restore test was performed.

## Desktop toolkit comparison

[Desktop toolkit comparison and recommendation](desktop-toolkit.md), checked 17 October 2026, compares Tauri, Rust-authored web alternatives, Qt Widgets/Quick, Slint, Iced, egui and GTK 4 against the accepted desktop workflows and D213 embedded SQLite direction, with Electron/Flutter/Avalonia screening. The user prefers no TypeScript and as much native Rust as practical, and D214 excludes GPL distribution. Slint is not selected. The completed Clacks-specific comparison recommends egui/eframe with egui_extras, accounting for the project's Rust direction and widget capabilities. It covers community components and explicit native-library packaging tradeoffs without carrying over CAD rendering judgments. D215 accepts egui, with eframe and egui_extras as the starting integration direction. Exact dependency versions, renderer, optional components and interface design remain open. The report distinguishes source findings, engineering judgments and proposed feasibility checks; no prototype, benchmark or installation was performed.

## Application licensing

[Licensing comparison and recommendation](licensing.md), checked 17 October 2026, compares hosted-service restrictions, broader noncompete/noncommercial terms, delayed open-source conversion, permissive licensing, service-source disclosure and custom terms against D03/D214/D219–D221. It recommends Elastic License 2.0 under the clarified policy that hosting for others competes even when free, with individual personal self-hosting allowed. The earlier proposed community-hosting exception is removed; D221 selects standard Elastic License 2.0 without a community-hosting exception.

The [projected dependency compatibility review](licensing.md#projected-dependency-compatibility) finds no identified direct-component blocker for Elastic License 2.0 in the selected stack. It records historical OpenSSL notices inside TrustedQSL, limited-copyleft conditions, separate font/data rights and the remaining version/feature-specific dependency inventory. There is no application dependency lockfile yet; this is not a complete transitive or release audit.

## Web interface architecture

[Web architecture comparison](web-interface-architecture.md), checked 17 October 2026, evaluates server templates/hypermedia, Leptos, Dioxus, Yew, a JavaScript frontend and browser egui against D222’s broader web direction. It recommends Leptos with Axum and server rendering followed by Rust/WebAssembly browser hydration, with concrete table/reporting leads and deployment/license tradeoffs. D223 accepts Leptos/Axum and that rendering direction. Exact versions and optional components remain open; no prototype or browser-offline requirement is implied.

## LoTW cryptographic backend

[Cryptographic dependency and compatibility findings](lotw-cryptography.md), checked
22 October 2026, records the selected archive/parser/blinded-RustCrypto composition,
exact dependency versions, the narrower local checks that leave certificate-chain
acceptance to LoTW, and pinned-source signing/metadata behavior. The
[certificate/signing specification](../docs/lotw-signing.md) consolidates the #4
contract. The unresolved RSA advisory, password limitations, transitive dependency
assessment and independent fixture requirements remain explicit. Source inspection
does not establish a combined build, runtime interoperability or live acceptance.

## Contributor agreement research

[Actual agreement comparison](contributor-agreements.md) records published terms against the hosting/licensing requirements. [JetBrains 1.3 findings and questions](jetbrains-cla-review.md) are material for counsel, not an adaptation recipe. D234 leaves substantive modifications and legal drafting/review to a qualified lawyer. No contributor agreement has been selected.

## Synchronization transport

[Transport comparison](synchronization-transport.md) compares WebSockets, HTTP requests with server-sent events, and gRPC against the accepted catch-up/streaming model. D235 selects encrypted WebSockets; replication semantics remain unchanged.

## Server credential protection

[Deployment-option comparison](server-credential-protection.md) records D236: independent account encryption keys protected by a deployment wrapping key supplied outside PostgreSQL, with documented key/database recovery. External key services are compared, not mandatory dependencies.

## Geographic reference data

The [geographic-pipeline specification](../docs/geographic-pipeline.md) consolidates D239 and D257–D268 for issue #5, with concrete engineering contracts prepared for review. The [source comparison](location-lookup.md), checked 23 October 2026, retains source selection, provenance, redistribution, accuracy and maintenance evidence. The [coordinate investigation](geographic-coordinate-operations.md) records inspected declarations, regional parameters, selected no-shift handling and accuracy limits. The [historical candidate audit](geographic-data-audit.json) retains source pins/digests, defects and sample results; its historical tool provenance is not a project-tooling dependency or a production fixture generator. These documents do not establish a shipping dataset or executed Rust interoperability. The specification defines the production contract; the research supports its source choices.

## Native-model reference and exchange research

The [contest exchange survey](contest-exchanges.md) uses organizer rules to
identify exchange data beyond ordinary contact fields. The
[logger-behavior survey](contest-logger-data.md) distinguishes serial operating
assistance, saved exchanges and participation/reference identity. They informed
the accepted separation of exchanges from the simple native reference
association; dedicated contest exchanges remain post-MVP. They do not prescribe
native fields or expand scope. The [native-model specification](../docs/native-model.md)
defines the maintained native contracts and component boundaries.

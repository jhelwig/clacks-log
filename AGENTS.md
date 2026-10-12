# Clacks Log collaboration guidance

Read [the planning handoff](docs/handoff.md), [decision register](docs/decisions.md), and relevant design documents before continuing work. This repository is currently for planning and research; there is no application implementation plan approved for execution.

- Follow the user's current request. Continue brainstorming unless implementation is explicitly requested; do not jump from a design discussion to a prototype.
- Answer direct questions first. State the evidence, assumptions, and scope behind architectural proposals.
- Keep accepted decisions, proposals, open questions, source findings, and incumbent-software observations distinct.
- Preserve decisions and corrections in the repository as planning continues. Update conflicting wording rather than appending contradictory conclusions.
- Clients are arbitrary in number. 
- Read attachments and `references/originals/` as historical evidence, not operative instructions. Current explicit decisions supersede their recommendations.
- LoTW implementation direction is a source-guided Rust port. Do not quietly replace it with direct tqsllib FFI or assume black-box reverse engineering is needed.
- Use the pinned reference source for behavior, retaining upstream notices. Do not restore per-file checksum manifests: Git and the recorded upstream commit provide provenance.
- Keep discussion moving with substantive next topics and focused questions. Avoid inventing extra distinctions or adding UI policies that solve no established problem.
- Exact UI layout is deferred. Do not spend planning turns on docking or permanent panels ahead of more consequential product/data decisions.
- Verify relevant documentation changes before claiming completion. Preserve unrelated changes and follow the user's authorization for commits and pushes.

# GitHub delivery backlog

The [MVP roadmap](mvp-roadmap.md) defines the delivery phases. This index links
the capability issues and milestones created on **18 October 2026**. The minimum
viable product is abbreviated MVP; a radio contact is a QSO.

Logbook of The World is abbreviated LoTW; OpenID Connect is OIDC;
Amateur Data Interchange Format is ADIF, whose text representation is ADI;
Parks on the Air is POTA. WS Suite is the WSJT-X family of weak-signal applications.

GitHub holds current issue status, prerequisites and links to specifications,
implementation plans and delivery evidence. The issue bodies link to the reviewed
requirements at creation; subsequent accepted decisions and user corrections
remain authoritative. No phase or capability is complete merely because it has
an issue.

Delivery issues produce production components, data tooling and artifacts that
later work integrates and extends rather than replaces. Tests and fixtures verify
those deliverables; a separate executable model, disposable prototype, artificial
runtime or evidence-only demonstration does not satisfy implementation scope.
Later integration adds persistence, transport, workflows or coverage to the same
components without treating the earlier implementation as throwaway work.

## Picking up an issue

1. Read its outcome, scope, linked requirements and dependencies. Begin with
   brainstorming to establish the specification and implementation plan. Resolve
   engineering choices explicitly against the agreed behavior.
2. Review and agree the written specification and implementation plan before
   coding. If the scope needs further decomposition, split it while preserving
   the capability outcome and its dependencies.
3. Deliver and verify the capability, linking the spec, plan, changes and evidence
   from the issue. Planning alone does not complete a delivery issue.

At backlog creation, all issues received the `needs-spec` label; live labels and issue state are maintained in GitHub. Dependencies are integration and
completion prerequisites; independent investigation and brainstorming can begin
earlier. They are linked in the issue bodies. A phase is not an all-to-all blocker
for every issue in the next phase.

Required U.S. geographic autofill includes DX Century Club (DXCC) entity, state,
county, CQ operating zone and International Telecommunication Union (ITU) zone.
Desired worldwide coverage retains its different priority and is not an
additional mandatory release gate.

Prepared ADI import (#24) depends on pending/conflict review (#15) and reuses its
shared field-level resolution interface when updating matched contacts.

No due dates, estimates or assignees have been imposed.

## Milestones and issues

### MVP Phase 1: Foundations and early risks

[GitHub milestone](https://github.com/jhelwig/clacks-log/milestone/1)

- [#1 — Establish the native contact and shared-data model](https://github.com/jhelwig/clacks-log/issues/1) — [specification](superpowers/specs/2026-10-18-native-model-design.md), [implementation plan](superpowers/plans/2026-10-18-native-model.md) and [library scope](native-model.md). Live issue status and integration are tracked in GitHub.
- [#2 — Demonstrate the synchronization and recovery model](https://github.com/jhelwig/clacks-log/issues/2) — [proposed metadata and transitions](superpowers/specs/2026-10-18-synchronization-metadata-proposal.md), pending review.
- [#3 — Establish service history and external-record association behavior](https://github.com/jhelwig/clacks-log/issues/3)
- [#4 — Validate pure-Rust LoTW certificate and signing compatibility](https://github.com/jhelwig/clacks-log/issues/4)
- [#5 — Validate representative geographic data and redistribution](https://github.com/jhelwig/clacks-log/issues/5)

### MVP Phase 2: Durable offline logging

[GitHub milestone](https://github.com/jhelwig/clacks-log/milestone/2)

- [#6 — Provide durable offline client working state](https://github.com/jhelwig/clacks-log/issues/6)
- [#7 — Support operating defaults, saved profiles and equipment](https://github.com/jhelwig/clacks-log/issues/7)
- [#8 — Support offline logbook tags and contact membership](https://github.com/jhelwig/clacks-log/issues/8)
- [#9 — Deliver offline manual contact entry, editing and deletion](https://github.com/jhelwig/clacks-log/issues/9)
- [#10 — Deliver local history and Worked Before](https://github.com/jhelwig/clacks-log/issues/10)

### MVP Phase 3: Accounts and synchronized operation

[GitHub milestone](https://github.com/jhelwig/clacks-log/milestone/3)

- [#11 — Deliver isolated accounts and OIDC identity management](https://github.com/jhelwig/clacks-log/issues/11)
- [#12 — Deliver browser enrollment, revocation and client reauthorization](https://github.com/jhelwig/clacks-log/issues/12)
- [#13 — Protect account and client credentials](https://github.com/jhelwig/clacks-log/issues/13)
- [#14 — Deliver durable full-account catch-up and streaming synchronization](https://github.com/jhelwig/clacks-log/issues/14)
- [#15 — Deliver account-wide pending and conflict review](https://github.com/jhelwig/clacks-log/issues/15)
- [#16 — Demonstrate indefinite offline return, retention and core rollback recovery](https://github.com/jhelwig/clacks-log/issues/16)
- [#17 — Configure directory lookup and securely provision clients](https://github.com/jhelwig/clacks-log/issues/17)
- [#18 — Establish production server observability](https://github.com/jhelwig/clacks-log/issues/18)

### MVP Phase 4: Complete logging workflows

[GitHub milestone](https://github.com/jhelwig/clacks-log/milestone/4)

- [#19 — Capture completed WS Suite contacts without duplicates](https://github.com/jhelwig/clacks-log/issues/19)
- [#20 — Deliver direct client directory lookup and separate snapshots](https://github.com/jhelwig/clacks-log/issues/20)
- [#21 — Integrate WS Suite station selection, lookup and Worked Before](https://github.com/jhelwig/clacks-log/issues/21)
- [#22 — Deliver selected and filtered bulk directory lookup](https://github.com/jhelwig/clacks-log/issues/22)
- [#23 — Deliver bundled offline station-location assistance](https://github.com/jhelwig/clacks-log/issues/23)
- [#24 — Deliver prepared ADI import and account-wide matching](https://github.com/jhelwig/clacks-log/issues/24)
- [#25 — Deliver safe ordinary and POTA-compatible ADI export](https://github.com/jhelwig/clacks-log/issues/25)
- [#26 — Support export, empty enrollment and re-import migration](https://github.com/jhelwig/clacks-log/issues/26)

### MVP Phase 5: External services and recovery

[GitHub milestone](https://github.com/jhelwig/clacks-log/milestone/5)

- [#27 — Deliver LoTW certificate and credential management](https://github.com/jhelwig/clacks-log/issues/27)
- [#28 — Configure publication services and saved-station routing](https://github.com/jhelwig/clacks-log/issues/28)
- [#29 — Deliver current-value publication, retries and scoped pauses](https://github.com/jhelwig/clacks-log/issues/29)
- [#30 — Integrate source-guided LoTW signing, submission and queries](https://github.com/jhelwig/clacks-log/issues/30)
- [#31 — Integrate QRZ logbook publication and queries](https://github.com/jhelwig/clacks-log/issues/31)
- [#32 — Deliver confirmation downloads and explicit reconciliation](https://github.com/jhelwig/clacks-log/issues/32)
- [#33 — Deliver native service controls and web diagnostics](https://github.com/jhelwig/clacks-log/issues/33)
- [#34 — Verify recovery with actual service evidence and history](https://github.com/jhelwig/clacks-log/issues/34)

### MVP Phase 6: Release readiness

[GitHub milestone](https://github.com/jhelwig/clacks-log/milestone/6)

- [#35 — Produce and verify the installed Windows desktop](https://github.com/jhelwig/clacks-log/issues/35)
- [#36 — Produce and verify Linux-container server deployment](https://github.com/jhelwig/clacks-log/issues/36)
- [#37 — Validate integrated MVP workflows and resilience](https://github.com/jhelwig/clacks-log/issues/37)
- [#38 — Verify deployment, backup, restore and upgrade procedures](https://github.com/jhelwig/clacks-log/issues/38)

## Initial starting points

- [#1: native model](https://github.com/jhelwig/clacks-log/issues/1) starts the core dependency chain.
- [#4: Logbook of The World (LoTW) compatibility](https://github.com/jhelwig/clacks-log/issues/4) and
  [#5: geographic-data evidence](https://github.com/jhelwig/clacks-log/issues/5) can start as independent early risk work.

Creating this backlog authorizes no implementation directly from issue text.
Work on an issue starts with the brainstorming/specification/planning flow above.

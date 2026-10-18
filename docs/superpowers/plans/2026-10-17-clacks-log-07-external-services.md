# External services and signing Milestone Outline

> **Status: milestone outline, not an executable implementation plan.** This document maps scope, dependencies and acceptance goals. Proposed files, interfaces and verification targets are provisional. Before coding, prepare a concrete plan for the selected milestone covering its data model, behavior, algorithms, edge cases and tests. Do not infer those choices while executing this outline.

A radio contact is a **QSO**; Logbook of The World is **LoTW**; minimum viable product is **MVP**; Public-Key Cryptography Standards #12 is **PKCS#12**; foreign function interface is **FFI**. DX denotes the contacted station and DE our station. QRZ and TrustedQSL are named service/tool products, not Clacks ownership identities.

**Goal:** Deliver server-owned QRZ and LoTW publication/download workflows with safe credentials, evidence and unique external associations.

**Architecture:** Provider adapters run inside the existing server and share durable jobs, account transactions and source-separated evidence. The LoTW implementation ports the pinned TrustedQSL behavior using established pure-Rust cryptography, not library FFI or runtime OpenSSL.

**Tech Stack:** Rust, PostgreSQL durable work, QRZ/LoTW protocol adapters; pure-Rust PKCS#12, certificate and signing libraries selected/verified during execution.

**Spec:** [integrations.md](../../../docs/integrations.md), [lotw.md](../../../docs/lotw.md), [source-map.md](../../../references/trustedqsl/source-map.md), [lotw-cryptography.md](../../../references/lotw-cryptography.md), [service-state-model.md](../../../references/service-state-model.md), [publication-recovery.md](../../../references/publication-recovery.md), [service-routing.md](../../../references/service-routing.md). Read these with the [coordinating plan](2026-10-17-clacks-log.md).

## Global Constraints

This is a planned file/interface layout, not existing code or authorization to execute. All tasks also inherit the coordinating plan’s constraints and file/dependency contracts. Native records and protocol encodings are pinned by F1/S1 before downstream code; do not invent competing representations in an isolated task. No implementation checks below have run yet.

- Only accepted server state is publishable; every new attempt uses current effective service values.
- Retry unless explicitly successful; clearly non-transient problems pause only affected uploads until remedy and explicit resume.
- At most one local QSO per external record/account/service destination; credentials are not the provider’s record identity.

## Review Focus

- Two local duplicates target one record before either response: E1/E2 reserve/validate provider identity, not just returned remote IDs.
- A retry occurs after a QSO edit: E2 records current dispatched values and attributes an old response only to its attempt.
- Our uploaded fields return with confirmation: E3 does not relabel them as counterpart-provided fallback.
- An old certificate archive uses legacy algorithms: L1/L2 demonstrate supported compatibility without OpenSSL fallback.
- Restore loses an upload outcome: E2/S4 retain uncertainty and follow the selected retry policy, not exactly-once claims.

## File structure

The Files entries below use `lib/<crate-name>` for libraries and `bin/<application-name>` for thin executable wrappers. Behavior, configuration/runtime composition and tests belong to libraries; binaries only delegate to the library entry point. The Files entries map the new implementation units and their tests. Brace groups expand to individual files in that directory. Crate manifests/module exports and required schema migrations belong to the task that first needs them; keep related source/tests together. No production file exists at these proposed paths yet.

---

### E1: Provider contracts, evidence and external association identity

**Depends on:** F2, I1, G1, A1.

**Files:**

- Create/modify: `lib/clacks-server/src/services/{mod,model,payload,matching,association,routing}.rs`
- Create/modify: `lib/clacks-server/migrations/0005_services.sql`
- Create/modify: `lib/clacks-server/tests/service_contracts.rs`
- Create/modify: `docs/engineering/service-contracts.md`

**Interfaces:**

- Consumes: F1 effective/native values; I1 candidate matcher; F2 account transaction boundary. Reads existing API/source research before defining provider-specific keys.
- Produces: ProviderDestination, ExternalRecordKey, PublicationAttempt, SentPayload, RemoteObservation, ImportedServiceClaim; effective_payload(qso: &QsoRecord, sources: &EligibleSources, service: Service) -> Result<ServicePayload, PayloadError>; resolve_route(qso: &QsoRecord, configured: &[ProviderDestination]) -> RouteDecision; associate(ctx: &AccountContext, external: ExternalRecordKey, qso: QsoId) -> Result<(), AssociationError>. Define canonical service/destination identity independent of credential rotation. Expose automatic_eligible(policies: impl IntoIterator<Item = UploadPolicy>) -> bool for the shared tri-state rule.

**Acceptance scenarios to specify:**

service_contracts::tri_state_forbid_precedence; untagged_no_grant; overlapping_tags_one_payload; occupied_association_rejected; distinct_destinations_independent; imported_positive_suppresses; missing_status_pending_independently; effective_change_not_metadata_change; route_saved_station_and_date.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert!(!automatic_eligible([Enable, Forbid, Neither]));
assert!(!automatic_eligible([Neither]));
assert!(second_qso_same_external_key.is_err());
assert!(metadata_only_refresh.does_not_need_upload());
```

**Required outcomes:**

Write exact provider identity/normalization, effective payload field sets, confirmation applicability, imported flags/dates and routing contracts before dispatch code. For provider records without opaque IDs, derive identity from documented service behavior and verify it against fixtures; do not use local destination-config IDs to evade uniqueness for the same actual provider target. Retain trusted imported statuses with provenance without fabricated payload history. Prevent duplicates inheriting occupied external associations. Multi-certificate/QRZ routing uses saved station callsign/date; missing/ambiguous routes pause affected uploads only. Publish this contract for I2/S4/E2/E3.

**Candidate verification targets:**

```text
cargo test -p clacks-server --test service_contracts
```

**Expected:** all named tests/checks pass; Known provider identity rules and tests support uniqueness before/after writes; unsupported ambiguities remain blocked, not silently overwritten.


### E2: Durable publication jobs and outcome handling

**Depends on:** E1, A3.

**Files:**

- Create/modify: `lib/clacks-server/src/services/{scheduler,jobs,attempts,outcomes,diagnostics}.rs`
- Create/modify: `lib/clacks-server/tests/publication_jobs.rs`

**Interfaces:**

- Consumes: E1 payload/route/identity contracts; A3 credential lifecycle; PostgreSQL transaction/job claims.
- Produces: plan_attempt(ctx: &AccountContext, qso: QsoId, service: Service, trigger: UploadTrigger) -> Result<AttemptId, DispatchBlocked>; record_outcome(attempt: AttemptId, outcome: ProviderOutcome) -> Result<(), ServiceError>; UploadTrigger distinguishes automatic/manual; ProviderAdapter::submit(attempt: &PreparedAttempt) -> Result<ProviderOutcome, ProviderError>. PreparedAttempt freezes actual sent values, destination and credential version.

**Acceptance scenarios to specify:**

publication_jobs::current_values_on_retry; old_response_does_not_cover_new_correction; concurrent_duplicate_target_blocked; lost_response_retry_diagnostic; explicit_success_no_retry; non_transient_affected_only_pause; manual_bypasses_cooldown_not_pause; removal_prevents_new_signing; restore_startup_pause. Inject crashes before dispatch, after provider success and before local outcome commit.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert_eq!(retry.sent_payload, current_effective_payload);
assert!(old_success.newer_correction_still_pending());
assert!(unrelated_destination_continues);
assert!(restore_mode.blocks_upload_until_explicit_resume());
```

**Required outcomes:**

Implement durable job claim/outcome work using ordinary transactions, bounded workers and provider-aware throttling. Do not hold database locks during network calls. Evaluate all tags, route and association constraints at dispatch; record exact values before external effects. Implement configurable scheduling with the recorded working defaults: upload checks every 60 seconds, cooldown 300 seconds since the last edit, downloads every 1800 seconds. These are application defaults, not provider mandates. Add injected-clock tests of the boundaries, manual controls, retries/backoff and actionable diagnostics. Retain ambiguous outcomes truthfully; do not claim exactly-once provider writes. Coordinate duplicate-target reservations and in-flight attempts under E1’s identity contract. Define races with credential removal, deletion and policy changes as dispatch semantics, preserving truthful outcomes for already-started calls.

**Candidate verification targets:**

```text
cargo test -p clacks-server --test publication_jobs
```

**Expected:** all named tests/checks pass; Failure/retry/race scenarios preserve sent-value coverage and only affected work pauses.


### L1: Pure-Rust certificate import and eligibility

**Depends on:** F1; persist/integrate after A3.

**Files:**

- Create/modify: `lib/clacks-lotw/src/{lib,archive,certificate,eligibility,config}.rs`
- Create/modify: `lib/clacks-lotw/tests/certificate_compatibility.rs`
- Create/modify: `tests/fixtures/lotw/README.md`
- Create/modify: `docs/engineering/lotw-compatibility.md`

**Interfaces:**

- Consumes: Pinned TrustedQSL 2.8.6 source/configuration and cryptographic comparison; synthetic/test-only certificate material.
- Produces: decode_archive(bytes: &[u8], password: &SecretString) -> Result<ImportedCredential, CredentialError>; eligible_certificates(request: &SigningIdentity, candidates: &[CertificateMetadata]) -> EligibilityResult; ImportedCredential separates secret key material from retained public metadata. Server-side storage uses A3; explicit supersession validates coverage gaps.

**Acceptance scenarios to specify:**

certificate_compatibility::correct_password; wrong_password; key_certificate_mismatch; duplicate_import; date_callsign_eligibility; expired_and_superseded; explicit_supersession_gap_preview; removal_preserves_public_metadata. Generate supported test archives with independently checked tooling and record format/algorithm provenance.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert!(wrong_password_import.is_err());
assert!(mismatched_key_import.is_err());
assert!(!supersession_is_inferred_from_newer_expiration);
assert_eq!(coverage_gap_preview, expected_uncovered_dates);
```

**Required outcomes:**

Evaluate released pure-Rust archive/certificate/signing dependencies against actual supported archive algorithms, certificate extensions and source rules. Record exact features, versions, trust validation and advisory assessment. Port certificate semantics using the source map, not upstream filesystem stores or global error state. Import requires a server connection; support multiple credentials and explicit supersession. Do not add certificate requests/renewals or reusable private-key export. No runtime OpenSSL or direct tqsllib FFI fallback. Any unsupported required compatibility becomes a documented engineering blocker with failing fixture, not an unannounced backend substitution.

**Candidate verification targets:**

```text
cargo test -p clacks-lotw --test certificate_compatibility; cargo tree -p clacks-lotw -e features
```

**Expected:** all named tests/checks pass; The supported archive/certificate matrix passes, secret handling is explicit and no OpenSSL runtime dependency is introduced.


### L2: Source-guided station normalization, signing and serialization

**Depends on:** L1, E1.

**Files:**

- Create/modify: `lib/clacks-lotw/src/{station,normalize,sign,gabbi,submission}.rs`
- Create/modify: `lib/clacks-lotw/tests/signing_vectors.rs`
- Create/modify: `docs/verification/lotw-signatures.md`

**Interfaces:**

- Consumes: L1 eligible explicit credential access; E1 current effective payload; pinned signature definitions and configuration.
- Produces: prepare_submission(payloads: &[ServicePayload], station: &SigningStation, credential: &SigningCredential) -> Result<SignedSubmission, SigningError>; SignedSubmission retains actual canonical values plus compatible certificate/station/contact records. GAbbI is TrustedQSL’s linked signed-record format.

**Acceptance scenarios to specify:**

signing_vectors::station_field_normalization; mode_band_date_rules; linked_record_references; canonical_signature_bytes; independent_signature_verification; unsupported_station_fields_diagnostic. Use independently verified vectors and reference output, not only sign/verify with the same implementation.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert_eq!(canonical_input, expected_reference_input);
assert!(independent_verifier.accepts(signature));
assert_eq!(submission.sent_values, payload_values);
```

**Required outcomes:**

Port normalization, signature-definition selection and GAbbI serialization from the pinned source/configuration; preserve relevant notices, including copied-source notices where applicable. Use established pure-Rust primitives with the chosen signing-path mitigations/advisory assessment. Independently inspect signatures and packaged requests. Preserve station identity and reject unrepresentable signing inputs diagnostically. Do not reinterpret GridTracker/directory information as own-station context. Runtime library builds/signing tests are necessary but do not prove live service acceptance; R2 owns that gate.

**Candidate verification targets:**

```text
cargo test -p clacks-lotw --test signing_vectors
```

**Expected:** all named tests/checks pass; Pinned reference behavior and independent checks establish a defensible signed-submission implementation before live provider claims.


### E3: QRZ/LoTW adapters and durable download processing

**Depends on:** E2, L2, A4, I1.

**Files:**

- Create/modify: `lib/clacks-server/src/services/{qrz,lotw,downloads,processing,confirmation}.rs`
- Create/modify: `lib/clacks-server/tests/provider_adapters.rs`
- Create/modify: `lib/clacks-server/tests/download_reconciliation.rs`
- Create/modify: `lib/clacks-web/src/services.rs`

**Interfaces:**

- Consumes: E1 service contracts; E2 jobs; L2 signing; I1 parsing/matching; account-isolated setup/diagnostics from A4.
- Produces: ProviderAdapter implementations submit/query using provider-specific response semantics; ingest_page(ctx: &AccountContext, page: ProviderPage) -> Result<IngestedPage, ServiceError>; process_observation(id: ObservationId, choice: ProcessingChoice) -> Result<ProcessingResult, ServiceError>. Cursor progress reflects durable ingestion, not completion of every user decision.

**Acceptance scenarios to specify:**

provider_adapters::explicit_submission_success; per_record_acceptance_distinct_confirmation; qrz_own_report_not_counterpart; correction_status_history. download_reconciliation::overlapping_windows_duplicate_safe; changed_remote_version_new_work; unique_confident_auto; possible_single_requires_choice; ambiguous_at_most_one; unmatched_explicit_add_reject; rejected_identical_suppressed_changed_reprocessed; deleted_contact_not_reoffered; cursor_never_ahead_of_retained_work.

Assertion excerpts below use values arranged by those named fixtures; they pin behavior, not a complete test implementation.

```rust
assert_eq!(repeated_observation.effect_count, 1);
assert_eq!(ambiguous_selected_qso_count, 1);
assert!(retained_page_coverage.covers(&advanced_cursor));
assert!(own_uploaded_location.is_not_counterpart_fallback());
```

**Required outcomes:**

Implement current provider requests/responses from verified official specifications and controlled fixtures; distinguish upload acceptance, per-record processing and counterpart confirmation. Retain provider observations separately; never overwrite recorded on-air facts or DE context. Track page/window coverage and atomically retain observation/disposition/effect associations. Pending ambiguity must not block unrelated processing. Confirmed status applies only while evidence matches the current QSO; retain historical positives when applicability changes or a newer explicit unconfirmed observation arrives. Finish web configuration for multiple QRZ destinations/certificates, credential removal, supersession gaps and diagnostics. Do not clear status on absent fields or empty windows.

**Candidate verification targets:**

```text
cargo test -p clacks-server --test provider_adapters; cargo test -p clacks-server --test download_reconciliation
```

**Expected:** all named tests/checks pass; Both providers pass submission/query fixtures, source-provenance and duplicate-safe processing tests; live interoperability remains R2.

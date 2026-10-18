# Publication recovery design

Planning proposal, 12 October 2026. This develops D32's publication-history boundary. D46-D49 establish current-value submission, automatic retries unless explicitly successful, pause/remedy/resume for clearly non-transient failures, and detailed diagnostics. Exact recovery mechanics remain unselected; no implementation has started.

## Intended behavior

D46 requires each new submission attempt, including retries, to use the current accepted server radio contact (QSO) values at dispatch. Do not replay a stale payload from the first attempt. The server retains the service destination, actual dispatched revision/values, and attempt information as publication history. A response must be applied to the attempt it describes, even if a newer QSO revision has since arrived. Success for an older submission does not clear a pending newer correction.

Track successful submission, provider-reported individual acceptance, and confirmation according to each provider's evidence. Logbook of The World (LoTW) immediately returns an upload-level accepted/rejected result. This establishes the upload outcome when received; do not treat a normal accepted response as an unknown result. Subsequent per-QSO processing/reporting and counterpart confirmation are separate. D47 requires automatic retries unless the upload was explicitly successful. Clearly non-transient failures pause attempts until the user remedies the situation and explicitly resumes. D48 requires as much diagnostic information as possible. Backoff timing remains open; D49 scopes pauses to affected uploads.

A lost response leaves an uncertain submission outcome and, under D47, is retried automatically. Provider queries may assist diagnosis or recovery, but the lack of an explicit success does not introduce a manual approval gate. Do not interpret an immediately absent LoTW query result as proof of failure while its processing may still be pending. QRZ's INSERT OPTION=REPLACE specifically overwrites an existing duplicate and may temporarily remove its confirmation. That behavior concerns updating the remote record with supplied data, including current QSO data; it does not imply resending a stale local version. No QRZ write/correction algorithm has been selected. Provider response interpretation and correction mechanisms require further service-specific research; the automatic retry policy is accepted.

Normal accepted/rejected responses provide usable outcomes; an explicit success stops retries for the values actually submitted. Missing/unusable responses remain retryable. An obviously non-transient rejection pauses attempts until remedy and explicit resume. A later local edit does not automatically resume a pause. The earlier manual-retry question is superseded by D47.

## Verified upload-response distinction

[American Radio Relay League (ARRL)'s upload interface](https://lotw.arrl.org/lotw-help/developer-submit-qsos/) specifies `.UPL.` accepted/rejected and `.UPLMESSAGE.` text. [ARRL's TrustedQSL (TQSL) signing tool command-line examples](https://lotw.arrl.org/lotw-help/cmdline/?lang=en) show upload success with a queued-for-processing message. The pinned `tqsl.cpp` source around lines 3207-3268 parses the returned status and reports success immediately on accepted. This confirms the distinction between immediate upload acceptance and subsequent processing, not an absence of an immediate success response.

## Diagnostic information (D48)

Retain enough information to explain what happened: attempt time, destination, QSO/revision and values actually submitted, failure phase, transport status/error, provider response and parsed outcome, retry history, next planned retry, and pause reason. Diagnostics must avoid recording credentials or private keys. Presentation and retention remain to be designed.

## External-record association and retries (D251)

Within an account/service destination, an external record may associate with at most one local QSO. An explicit local duplicate does not inherit another QSO's association. Validate association/collision constraints when preparing a current-value attempt; a competing QSO requires resolution instead of silently replacing the external record. Repeated results for the same attempt/record retain the existing association. Tag membership does not duplicate attempts or grant separate remote identities. Provider-specific identity checks and handling of in-flight races remain engineering work; the database cannot guarantee atomicity with the external service.

## Accepted pause scope (D49)

Pause only uploads affected by the non-transient problem. A credential failure affects uploads using that destination credential; a certificate failure affects contacts requiring that certificate; a contact-specific validation error affects that QSO/service. Other eligible uploads continue. Resume remains explicit after remedy. Downloads are not automatically paused by a signing-only or QSO-only upload error. Exact dependency scoping remains to be designed.

## Evidence and limits

[LoTW query documentation](https://lotw.arrl.org/lotw-help/developer-query-qsos-qsls/) distinguishes receipt/acceptance and confirmation and supplies receipt/update timestamps. [QRZ's application programming interface (API) guide](https://www.qrz.com/docs/logbook/QRZLogbookAPI.html) returns insertion/replacement outcomes and remote record identifiers; replacement can affect confirmation. Neither source establishes a common client-selected idempotency key or one universal safe retry algorithm.

Exact backoff timings, provider completion/failure evidence, user-facing queue presentation, recovery actions, and attempt retention remain open. No live provider operations have been tested.

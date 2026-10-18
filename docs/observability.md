# Server observability

D230 requires full production-grade server observability for the minimum viable product (MVP): tracing, structured logs and metrics for server operators. This applies to the one Clacks server application selected in D213/D223, including its web, synchronization and background-work paths.

## Operator and account-user needs

Operator telemetry and account-user diagnostics serve different audiences. Operator observability must include security events such as client enrollment/revocation, service-credential changes, certificate import and explicit supersession. Exclude passwords, tokens and private keys from telemetry. Exact event fields and access/retention arrangements remain specification work.

A retained account-scoped security-event history view in the web interface is post-MVP under D230. Existing account-isolated upload diagnostics and client synchronization status remain MVP under D66/D142. Deferring that history view does not reduce operator observability to basic diagnostic logs.

Operational telemetry is distinct from authoritative submission history, remote observations and synchronization/recovery evidence. Existing retention and recovery requirements for those application records still apply independently.

## Remaining specification work

Define instrumentation and correlation across requests, synchronization sessions and background jobs; structured event fields and secret redaction; metrics, label cardinality and trace sampling; export/collection integration, access and retention; and documented deployment configuration. These are engineering decisions against the accepted production-service requirement. No telemetry library, protocol, backend or deployment bundle has yet been selected.

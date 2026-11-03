//! Real provider exchanges distinguish fresh authentication, reuse and session renewal in traces.

use crate::directory::{
    providers::{client, hamqth::HamQthSession, qrz::QrzSession},
    test_support::{fixture, key, material, response},
};

/// Exercise fresh login, cached lookup and expiry renewal through both concrete adapters.
pub(super) async fn run() -> anyhow::Result<()> {
    let qrz = fixture(vec![
        response(
            200,
            "<QRZDatabase><Session><Key>LOGIN_TRACE_QRZ_TOKEN</Key></Session></QRZDatabase>",
        ),
        response(
            200,
            "<QRZDatabase><Session><Error>Not found: unknown</Error></Session></QRZDatabase>",
        ),
        response(
            200,
            "<QRZDatabase><Session><Error>Session Timeout</Error></Session></QRZDatabase>",
        ),
        response(
            200,
            "<QRZDatabase><Session><Key>LOGIN_TRACE_QRZ_RENEWED</Key></Session></QRZDatabase>",
        ),
        response(
            200,
            "<QRZDatabase><Session><Error>Not found: unknown</Error></Session></QRZDatabase>",
        ),
        response(
            200,
            "<QRZDatabase><Session><Error>Not found: unknown</Error></Session></QRZDatabase>",
        ),
    ])
    .await?;
    let session = QrzSession::new(client()?, qrz.endpoint.clone(), material());
    for _ in 0..3 {
        session.lookup(&key()).await?;
    }

    let hamqth = fixture(vec![
        response(200, r#"{"token":"LOGIN_TRACE_HAMQTH_TOKEN"}"#),
        response(200, r#"{"callsign":"W7TEST"}"#),
        response(401, "{}"),
        response(200, r#"{"token":"LOGIN_TRACE_HAMQTH_RENEWED"}"#),
        response(200, r#"{"callsign":"W7TEST"}"#),
        response(200, r#"{"callsign":"W7TEST"}"#),
    ])
    .await?;
    let session = HamQthSession::new(client()?, hamqth.endpoint.clone(), material());
    for _ in 0..3 {
        session.lookup(&key()).await?;
    }
    Ok(())
}

/// Credentials, returned tokens, query calls and raw provider responses never enter a sink.
pub(super) fn exclude_secrets(bytes: &[u8]) {
    for secret in [
        "fixture-user",
        "secret&+ sentinel",
        "LOGIN_TRACE_",
        "EA8/W7TEST",
        "W7TEST",
        "<QRZDatabase>",
    ] {
        assert!(
            !bytes
                .windows(secret.len())
                .any(|window| window == secret.as_bytes()),
            "provider data entered telemetry: {secret}"
        );
    }
}

/// Each provider emits one login span for initial authentication and one for renewal, not reuse.
pub(super) fn verify(requests: &[super::collector::Request]) -> anyhow::Result<()> {
    use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
    use prost::Message;
    let mut counts = std::collections::BTreeMap::new();
    for request in requests {
        exclude_secrets(&request.body);
        if request.path == "/v1/traces" {
            let export = ExportTraceServiceRequest::decode(request.body.as_slice())?;
            for span in export
                .resource_spans
                .into_iter()
                .flat_map(|resource| resource.scope_spans)
                .flat_map(|scope| scope.spans)
            {
                *counts.entry(span.name).or_insert(0) += 1;
            }
        }
    }
    for provider in ["qrz", "hamqth"] {
        assert_eq!(
            counts.get(&format!("desktop.directory.{provider}.login")),
            Some(&2)
        );
        assert_eq!(
            counts.get(&format!("desktop.directory.{provider}.lookup")),
            Some(&3)
        );
    }
    Ok(())
}

use crate::support;

use clacks_lotw::{
    certificate::{eligibility::CredentialState, import_pkcs12},
    signing::{BatchItem, ProducerInformation, definition::SignatureDefinitions, write_batch},
};
use support::telemetry::{Observations, capture};

fn event<'a>(observations: &'a Observations, name: &str) -> &'a support::telemetry::Observation {
    observations
        .events
        .iter()
        .find(|e| e.fields.get("event").is_some_and(|v| v == name))
        .expect("domain event")
}

#[test]
fn records_safe_domain_outcomes_with_inherited_context_and_filtered_spans() -> anyhow::Result<()> {
    let defs = SignatureDefinitions::bundled()?;
    let a = support::submission();
    for spans in [true, false] {
        let (result, observed) = capture(
            || -> anyhow::Result<_> {
                let parent = tracing::debug_span!("request", job_id = "test-job");
                let _entered = parent.enter();
                let c = import_pkcs12(&support::fixture("1024/normal.p12"), "fixture-password")?;
                let mut bytes = Vec::new();
                let summary = write_batch(
                    &mut bytes,
                    &[BatchItem {
                        submission: &a,
                        credential: &c,
                        state: CredentialState::Available,
                    }],
                    &defs,
                    &ProducerInformation {
                        application: "Clacks",
                        version: "test",
                    },
                    support::now(),
                )?;
                Ok((summary, bytes.len()))
            },
            spans,
        );
        let (summary, size) = result?;
        assert_eq!(summary.bytes_written as usize, size);
        let constructed = event(&observed, "signing.batch.constructed");
        for (key, value) in [
            ("certificate_count", 1),
            ("station_count", 1),
            ("contact_count", 1),
            ("output_bytes", size),
        ] {
            assert_eq!(constructed.fields[key].parse::<usize>()?, value);
        }
        event(&observed, "certificate.import.checked");
        if spans {
            let batch = observed
                .spans
                .iter()
                .find(|s| s.name == "lotw.batch.write")
                .unwrap();
            let request = observed.spans.iter().find(|s| s.name == "request").unwrap();
            assert_eq!(batch.parent, request.id);
            assert_eq!(constructed.parent, batch.id);
        }
        let text = format!("{observed:?}");
        for secret in [
            "fixture-password",
            "N0CALL",
            "K1ABC",
            "14.074000",
            "PRIVATE KEY",
            "duration_seconds",
            "component",
            "operation",
        ] {
            assert!(!text.contains(secret), "{secret}");
        }
        assert!(
            observed
                .events
                .iter()
                .all(|e| e.target.starts_with("clacks_lotw"))
        );
    }
    Ok(())
}

#[test]
fn explains_local_rejections_without_logging_payloads_or_claiming_success() -> anyhow::Result<()> {
    let c = import_pkcs12(&support::fixture("1024/normal.p12"), "fixture-password")?;
    let defs = SignatureDefinitions::bundled()?;
    let producer = ProducerInformation {
        application: "Clacks",
        version: "test",
    };
    let mut a = support::submission();
    let (result, observed) = capture(
        || import_pkcs12(&support::fixture("no-mac.p12"), "sentinel-password"),
        true,
    );
    assert!(result.is_err());
    event(&observed, "certificate.import.rejected");
    a.signing_identity.dxcc = "1".into();
    let (result, observed) = capture(
        || {
            write_batch(
                &mut Vec::new(),
                &[BatchItem {
                    submission: &a,
                    credential: &c,
                    state: CredentialState::Available,
                }],
                &defs,
                &producer,
                support::now(),
            )
        },
        true,
    );
    assert!(result.is_err());
    event(&observed, "signing.input.rejected");
    let a = support::submission();
    let (result, observed) = capture(
        || {
            write_batch(
                &mut Vec::new(),
                &[BatchItem {
                    submission: &a,
                    credential: &c,
                    state: CredentialState::Removed,
                }],
                &defs,
                &producer,
                support::now(),
            )
        },
        true,
    );
    assert!(result.is_err());
    event(&observed, "certificate.eligibility.unavailable");
    let run = || {
        write_batch(
            &mut support::LimitedWriter {
                bytes: Vec::new(),
                chunk: 0,
                fail_at: None,
            },
            &[BatchItem {
                submission: &a,
                credential: &c,
                state: CredentialState::Available,
            }],
            &defs,
            &producer,
            support::now(),
        )
    };
    let (result, observed) = capture(run, false);
    assert_eq!(
        result
            .unwrap_err()
            .downcast_ref::<std::io::Error>()
            .unwrap()
            .kind(),
        std::io::ErrorKind::WriteZero
    );
    assert!(!observed.events.iter().any(|e| {
        e.fields
            .get("event")
            .is_some_and(|v| v == "signing.batch.constructed")
    }));
    Ok(())
}

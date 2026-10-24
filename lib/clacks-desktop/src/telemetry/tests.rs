//! Composition checks use isolated processes to keep global framework state separate.

mod collector;
mod operations;
mod output;
mod session;

use super::*;
use crate::{
    config::{Cli, Environment},
    workspace::{AppDirectories, WorkspacePaths},
};
use clap::Parser;
use std::process::{Command, Stdio};

/// Execute one real composition case in an isolated global-subscriber process.
fn child_case(case: &str) -> anyhow::Result<()> {
    let mut command = Command::new(std::env::current_exe()?);
    for (key, _) in std::env::vars_os() {
        if key
            .to_str()
            .is_some_and(|key| key.starts_with("OTEL_") || key == "RUST_LOG")
        {
            command.env_remove(key);
        }
    }
    command.env("NO_PROXY", "*").env("no_proxy", "*");
    let output = command
        .args([
            "--exact",
            "telemetry::tests::child_entry",
            "--ignored",
            "--test-threads=1",
        ])
        .env("CLACKS_TELEMETRY_CASE", case)
        .stdin(Stdio::null())
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "telemetry child failed: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn export_disabled_sends_nothing() -> anyhow::Result<()> {
    child_case("disabled")
}

#[test]
fn log_output_failure_is_degraded_operation() -> anyhow::Result<()> {
    child_case("unwritable")
}

#[test]
#[ignore = "isolated process fixture, invoked by composition tests"]
fn child_entry() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let case = std::env::var("CLACKS_TELEMETRY_CASE")?;
    let collector = (case != "unavailable" && case != "unwritable")
        .then(|| {
            collector::Collector::start(match case.as_str() {
                "secrets" => collector::Reply::RejectSecret,
                "partial" => collector::Reply::PartialSecret,
                "slow" => collector::Reply::Delayed,
                _ => collector::Reply::Accept,
            })
        })
        .transpose()?;
    let mut env = Environment {
        cwd: directory.path().into(),
        values: Default::default(),
        directories: AppDirectories {
            config: directory.path().join("config"),
            data: directory.path().join("data"),
        },
    };
    let endpoint = collector
        .as_ref()
        .map(|collector| format!("http://{}", collector.address))
        .unwrap_or_else(|| "http://127.0.0.1:0".into());
    let exporting = matches!(
        case.as_str(),
        "enabled"
            | "unavailable"
            | "secrets"
            | "slow"
            | "session"
            | "partial"
            | "headers-file"
            | "headers-env"
    );
    let mut arguments = vec![
        "clacks-log".to_owned(),
        "--otlp".into(),
        exporting.to_string(),
        "--otlp-timeout-ms".into(),
        "100".into(),
        "--otlp-endpoint".into(),
        if case == "secrets" {
            format!("{endpoint}/?auth=SECRET_SENTINEL")
        } else {
            endpoint
        },
    ];
    if case == "secrets" {
        arguments.extend(["--log-filter".into(), "trace".into()]);
        env.values.insert(
            "OTEL_EXPORTER_OTLP_HEADERS".into(),
            "authorization=Bearer%20SECRET_SENTINEL".into(),
        );
    }
    if case == "headers-file" {
        std::fs::create_dir_all(&env.directories.config)?;
        std::fs::write(
            env.directories.config.join("config.toml"),
            "[telemetry]\nheaders = { authorization = \"Bearer abc%2Fdef\" }\n",
        )?;
    }
    if case == "headers-env" {
        env.values.insert(
            "OTEL_EXPORTER_OTLP_HEADERS".into(),
            "authorization=Bearer%20abc%252Fdef".into(),
        );
    }
    let config = crate::config::resolve(Cli::try_parse_from(arguments)?, &env)?;
    let mut paths = WorkspacePaths::resolve(&config)?;
    if case == "unwritable" {
        paths.log_directory = directory.path().join("regular-file");
        std::fs::write(&paths.log_directory, "preserve")?;
    }
    let telemetry = initialize(&config, &paths);
    assert_eq!(!telemetry.diagnostics().is_empty(), case == "unwritable");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let account = if case == "session" {
        session::complete(&runtime, &paths, telemetry)?
    } else {
        let account = runtime.block_on(operations::save_contact(&paths))?;
        let shutdown = telemetry.shutdown();
        if !matches!(case.as_str(), "unavailable" | "secrets" | "slow") {
            shutdown?;
        }
        account
    };
    assert!(paths.database.exists());
    if case != "unwritable" {
        let contents = std::fs::read_dir(&paths.log_directory)?
            .map(|entry| std::fs::read_to_string(entry?.path()))
            .collect::<Result<Vec<_>, _>>()?
            .join("");
        assert!(contents.contains("workspace.created"));
        assert!(contents.contains(&account.to_string()));
        assert!(!contents.contains("SECRET_SENTINEL"));
        if case == "secrets" {
            assert!(contents.contains("status code: 401"));
        }
        if case == "filtered" {
            assert!(contents.contains("schema.upgraded"));
            assert!(!contents.contains("client_store.schema.migrate"));
        }
    }
    if let Some(collector) = &collector {
        let requests = collector.requests.lock().unwrap();
        if !exporting {
            assert!(requests.is_empty());
        }
        for request in requests.iter() {
            assert!(
                !request
                    .body
                    .windows(b"SECRET_SENTINEL".len())
                    .any(|bytes| bytes == b"SECRET_SENTINEL")
            );
        }
        if matches!(case.as_str(), "headers-file" | "headers-env") {
            assert_eq!(requests.len(), 3);
            for request in requests.iter() {
                assert_eq!(request.authorization.as_deref(), Some("Bearer abc%2Fdef"));
            }
        }
        if case == "session" {
            session::assert_wire(&requests)?;
        }
        if case == "enabled" {
            assert_signals(&requests)?;
        }
    }
    Ok(())
}

#[test]
fn all_three_signals_reach_receiver() -> anyhow::Result<()> {
    child_case("enabled")
}

#[test]
fn unavailable_collector_preserves_store_work_and_local_logs() -> anyhow::Result<()> {
    child_case("unavailable")
}

#[test]
fn filtered_spans_preserve_milestone_context() -> anyhow::Result<()> {
    child_case("filtered")
}

#[test]
fn secret_configuration_stays_out_of_telemetry() -> anyhow::Result<()> {
    child_case("secrets")
}

/// Verify Clacks's actual semantic output through the real wire encodings.
fn assert_signals(requests: &[collector::Request]) -> anyhow::Result<()> {
    use opentelemetry_proto::tonic::collector::{
        logs::v1::ExportLogsServiceRequest, metrics::v1::ExportMetricsServiceRequest,
        trace::v1::ExportTraceServiceRequest,
    };
    use prost::Message;
    let trace = requests
        .iter()
        .find(|request| request.path == "/v1/traces")
        .expect("trace export");
    let logs = requests
        .iter()
        .find(|request| request.path == "/v1/logs")
        .expect("log export");
    let metrics = requests
        .iter()
        .find(|request| request.path == "/v1/metrics")
        .expect("metric export");
    let trace = ExportTraceServiceRequest::decode(trace.body.as_slice())?;
    assert!(
        trace
            .resource_spans
            .iter()
            .flat_map(|resource| &resource.scope_spans)
            .flat_map(|scope| &scope.spans)
            .any(|span| span.name == "client_store.create_with_definitions")
    );
    let logs = ExportLogsServiceRequest::decode(logs.body.as_slice())?;
    assert!(
        logs.resource_logs
            .iter()
            .flat_map(|resource| &resource.scope_logs)
            .flat_map(|scope| &scope.log_records)
            .any(|record| record
                .attributes
                .iter()
                .any(|attribute| attribute.key == "event"))
    );
    let metrics = ExportMetricsServiceRequest::decode(metrics.body.as_slice())?;
    assert!(
        metrics
            .resource_metrics
            .iter()
            .flat_map(|resource| &resource.scope_metrics)
            .flat_map(|scope| &scope.metrics)
            .any(|metric| metric.name == "clacks_client_qso_mutations_total")
    );
    Ok(())
}

#[test]
fn slow_collector_does_not_prevent_committed_local_work() -> anyhow::Result<()> {
    child_case("slow")
}

#[test]
fn constructor_diagnostics_exclude_sdk_payloads() {
    let message = diagnostics::exporter(
        "Trace",
        opentelemetry_otlp::ExporterBuildError::InternalFailure("SECRET_SENTINEL".into()),
    );
    assert!(!message.contains("SECRET_SENTINEL"));
    assert!(message.contains("initialization failed"));
}

#[test]
fn session_reports_startup_once_and_drains_before_telemetry_shutdown() -> anyhow::Result<()> {
    child_case("session")
}

#[test]
fn collector_partial_success_does_not_disclose_response_messages() -> anyhow::Result<()> {
    child_case("partial")
}

#[test]
fn literal_file_headers_and_decoded_environment_headers_reach_the_collector_unchanged()
-> anyhow::Result<()> {
    child_case("headers-file")?;
    child_case("headers-env")
}

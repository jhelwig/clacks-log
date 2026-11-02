//! Composition checks use isolated processes to keep global framework state separate.

mod collector;
mod membership;
mod operations;
mod output;
mod session;

use super::*;
use crate::{
    config::{Cli, StartupContext},
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
    if case == "enabled" {
        // Valid SDK configuration keeps the milestone check independent of batching.
        command.env("OTEL_BLRP_MAX_EXPORT_BATCH_SIZE", "1");
    }

    let output = command
        .args([
            "--exact",
            "telemetry::tests::child_entry",
            "--ignored",
            "--nocapture",
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
    if case == "disabled" {
        anyhow::ensure!(
            output.stderr.is_empty(),
            "first launch unexpectedly wrote to stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

#[test]
fn first_launch_keeps_diagnostics_in_local_logs() -> anyhow::Result<()> {
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
    let collector = (!matches!(case.as_str(), "unavailable" | "unwritable" | "membership"))
        .then(|| {
            collector::Collector::start(match case.as_str() {
                "secrets" => collector::Reply::RejectSecret,
                "partial" => collector::Reply::PartialSecret,
                "slow" => collector::Reply::Delayed,
                _ => collector::Reply::Accept,
            })
        })
        .transpose()?;

    if case == "session"
        && let Some(collector) = &collector
    {
        // Export cancellation can close a connection before any HTTP request arrives.
        drop(std::net::TcpStream::connect(collector.address)?);
    }

    let mut values = ::config::Map::new();
    let mut env = StartupContext {
        cwd: directory.path().into(),
        environment: ::config::Environment::default(),
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
        format!("--otlp={exporting}"),
        "--otlp-endpoint".into(),
        if case == "secrets" {
            format!("{endpoint}/?auth=SECRET_SENTINEL")
        } else {
            endpoint
        },
    ];

    // Successful composition uses normal production deadlines; only the cases
    // exercising collector unavailability or delay need an abbreviated budget.
    if matches!(case.as_str(), "slow" | "unavailable") {
        arguments.extend(["--otlp-timeout-ms".into(), "100".into()]);
    }

    if case == "filtered" {
        arguments.extend([
            "--log-filter".into(),
            "warn,clacks_client_store::migration=info,clacks_client_store::store::creation=info"
                .into(),
        ]);
    }

    if case == "secrets" {
        arguments.extend(["--log-filter".into(), "trace".into()]);
        values.insert(
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
        values.insert(
            "OTEL_EXPORTER_OTLP_HEADERS".into(),
            "authorization=Bearer%20abc%252Fdef".into(),
        );
    }

    env.environment = ::config::Environment::default().source(Some(values));

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
    if case == "session" {
        session::complete(&runtime, &paths, telemetry)?;
    } else {
        match case.as_str() {
            "retry" => runtime.block_on(operations::retry_contact(&paths))?,
            "membership" => runtime.block_on(membership::run(&paths))?,
            _ => runtime.block_on(operations::save_contact(&paths))?,
        }
        let shutdown = telemetry.shutdown();
        if !matches!(case.as_str(), "unavailable" | "secrets" | "slow") {
            shutdown?;
        }
    }

    assert!(paths.database.exists());
    if matches!(case.as_str(), "slow" | "partial") {
        let receiver = collector.as_ref().unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let requests = receiver.requests.lock().unwrap();
            let witnessed = if case == "slow" {
                requests
                    .iter()
                    .any(|request| request.path == "/v1/traces" && request.delayed)
            } else {
                ["/v1/traces", "/v1/metrics", "/v1/logs"]
                    .iter()
                    .all(|path| {
                        requests
                            .iter()
                            .any(|request| request.path == *path && request.response_sent)
                    })
            };
            drop(requests);
            if witnessed {
                break;
            }
            anyhow::ensure!(
                std::time::Instant::now() < deadline,
                "collector response was never delivered"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    if case != "unwritable" {
        let contents = std::fs::read_dir(&paths.log_directory)?
            .map(|entry| std::fs::read_to_string(entry?.path()))
            .collect::<Result<Vec<_>, _>>()?
            .join("");

        assert!(!contents.contains("SECRET_SENTINEL"));
        if case == "retry" {
            for callsign in ["W7HEX", "K1ABC"] {
                assert!(
                    !contents.contains(callsign),
                    "recorded callsigns must not enter telemetry"
                );
            }
        }
        if case == "membership" {
            assert!(!contents.contains("newer fact"));
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
            for path in ["/v1/traces", "/v1/metrics", "/v1/logs"] {
                assert!(requests.iter().any(|request| request.path == path));
            }
            for request in requests.iter() {
                assert_eq!(request.authorization.as_deref(), Some("Bearer abc%2Fdef"));
            }
        }
    }
    Ok(())
}

#[test]
fn enabled_export_preserves_committed_store_work() -> anyhow::Result<()> {
    child_case("enabled")
}

#[test]
fn unavailable_collector_preserves_store_work_and_local_logs() -> anyhow::Result<()> {
    child_case("unavailable")
}

#[test]
fn configured_filters_preserve_committed_store_work() -> anyhow::Result<()> {
    child_case("filtered")
}

#[test]
fn secret_configuration_stays_out_of_telemetry() -> anyhow::Result<()> {
    child_case("secrets")
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
fn session_closes_reopens_workspace_and_releases_ownership() -> anyhow::Result<()> {
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

#[test]
fn retry_acknowledgment_preserves_work_without_disclosing_contact_facts() -> anyhow::Result<()> {
    child_case("retry")
}

#[test]
fn membership_handles_stale_bases_and_cancellation() -> anyhow::Result<()> {
    child_case("membership")
}

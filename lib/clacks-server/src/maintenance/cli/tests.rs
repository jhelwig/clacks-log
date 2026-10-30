//! Operator reporting through the actual command driver and process exporters.

use super::*;
use anyhow::Context;
use axum::{Router, body::Bytes, extract::State, routing::post};
use opentelemetry_proto::tonic::collector::{
    logs::v1::ExportLogsServiceRequest, metrics::v1::ExportMetricsServiceRequest,
};
use prost::Message;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Exports {
    logs: Vec<ExportLogsServiceRequest>,
    metrics: Vec<ExportMetricsServiceRequest>,
}

async fn logs(State(exports): State<Arc<Mutex<Exports>>>, body: Bytes) {
    exports
        .lock()
        .unwrap()
        .logs
        .push(ExportLogsServiceRequest::decode(body).unwrap());
}

async fn metrics(State(exports): State<Arc<Mutex<Exports>>>, body: Bytes) {
    exports
        .lock()
        .unwrap()
        .metrics
        .push(ExportMetricsServiceRequest::decode(body).unwrap());
}

#[tokio::test]
async fn operator_exports_committed_rotation_and_flushes_failure() -> anyhow::Result<()> {
    let _serial = crate::maintenance::keys::tests::SERIAL.lock().await;
    for outcome in ["complete", "failed", "partial_rewrap"] {
        let exports = Arc::new(Mutex::new(Exports::default()));
        let app = Router::new()
            .route("/v1/logs", post(logs))
            .route("/v1/metrics", post(metrics))
            .route("/v1/traces", post(|| async {}))
            .with_state(exports.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let collector = tokio::spawn(async move { axum::serve(listener, app).await });

        // Global subscribers and recorders are process-owned, as in clacks-admin.
        let mut command = tokio::process::Command::new(std::env::current_exe()?);
        command.args([
            "--exact",
            "maintenance::cli::tests::operator_process",
            "--ignored",
            "--nocapture",
        ]);
        for (name, _) in
            std::env::vars().filter(|(name, _)| name.starts_with("OTEL_") || name == "RUST_LOG")
        {
            command.env_remove(name);
        }
        let output = command
            .env("CLACKS_OPERATOR_TEST_OUTCOME", outcome)
            .env("OTEL_EXPORTER_OTLP_ENDPOINT", endpoint)
            .env("OTEL_METRIC_EXPORT_INTERVAL", "600000")
            .env("OTEL_BLRP_SCHEDULE_DELAY", "600000")
            .env("NO_PROXY", "127.0.0.1")
            .env("no_proxy", "127.0.0.1")
            .output()
            .await?;
        collector.abort();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!String::from_utf8_lossy(&output.stderr).contains("operator-provider-secret"));
        let exports = exports.lock().unwrap();
        let logs = format!("{:?}", exports.logs);
        assert!(!logs.contains("operator-provider-secret"));
        if outcome == "complete" {
            use opentelemetry_proto::tonic::metrics::v1::{metric::Data, number_data_point::Value};
            assert!(
                exports
                    .metrics
                    .iter()
                    .flat_map(|batch| &batch.resource_metrics)
                    .flat_map(|resource| &resource.scope_metrics)
                    .flat_map(|scope| &scope.metrics)
                    .any(
                        |metric| metric.name == "clacks_secret_values_reencrypted_total"
                            && matches!(&metric.data, Some(Data::Sum(sum)) if sum.data_points.iter()
                        .any(|point| point.value == Some(Value::AsInt(1))))
                    )
            );
            assert!(logs.contains("account.key.rotation"));
        } else if outcome == "failed" {
            let events: Vec<_> = exports.logs.iter().flat_map(|batch| &batch.resource_logs)
                .flat_map(|resource| &resource.scope_logs).flat_map(|scope| &scope.log_records)
                .filter(|record| record.attributes.iter().any(|attribute|
                    attribute.key == "event" && attribute.value.as_ref().is_some_and(|value|
                        matches!(&value.value, Some(opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue(value)) if value == "account.key.rotation.failed"))))
                .collect();
            assert_eq!(events.len(), 1);
            for field in ["credentials", "receipts"] {
                assert!(events.iter().any(|event| event.attributes.iter().any(|attribute|
                    attribute.key == field && attribute.value.as_ref().is_some_and(|value|
                        matches!(value.value, Some(opentelemetry_proto::tonic::common::v1::any_value::Value::IntValue(0)))))));
            }
        } else {
            use opentelemetry_proto::tonic::common::v1::any_value::Value;
            let events: Vec<_> = exports.logs.iter().flat_map(|batch| &batch.resource_logs)
                .flat_map(|resource| &resource.scope_logs).flat_map(|scope| &scope.log_records)
                .filter(|record| record.attributes.iter().any(|attribute|
                    attribute.key == "event" && attribute.value.as_ref().is_some_and(|value|
                        matches!(&value.value, Some(Value::StringValue(value)) if value == "deployment.keys.rewrap.failed"))))
                .collect();
            assert_eq!(events.len(), 1);
            assert!(
                events.iter().any(
                    |event| event.attributes.iter().any(|attribute| attribute.key
                        == "migrated_account_keys"
                        && attribute
                            .value
                            .as_ref()
                            .is_some_and(|value| matches!(value.value, Some(Value::IntValue(1)))))
                )
            );
        }
    }
    Ok(())
}

#[tokio::test]
#[ignore = "automated child process for operator exporter isolation"]
async fn operator_process() -> anyhow::Result<()> {
    use crate::{
        auth::oidc::VerifiedIdentity,
        secrets::{
            credentials::{self, CredentialIdentity, CredentialPurpose, SecretMaterial},
            keys::WrappingKeys,
        },
        store::AccountStore,
    };
    use clacks_model::ids::CredentialId;
    let pool = sqlx::PgPool::connect(&std::env::var("CLACKS_TEST_DATABASE_URL")?).await?;
    crate::maintenance::migrate::run(&pool).await?;
    let store = AccountStore::new(pool.clone());
    let login = store
        .provision_login(
            VerifiedIdentity {
                issuer: "https://operator.example".into(),
                subject: ulid::Ulid::generate().to_string(),
                display_name: None,
            },
            time::Duration::days(14),
        )
        .await?;
    let account = store
        .authenticate(&login, time::Duration::days(14))
        .await?
        .context("missing login")?
        .account_id;
    let directory = tempfile::tempdir()?;
    let outcome = std::env::var("CLACKS_OPERATOR_TEST_OUTCOME")?;
    let from = format!("{}-W1", ulid::Ulid::generate());
    let source = directory.path().join("W1");
    let target = directory.path().join("W2");
    std::fs::write(&source, [61u8; 32])?;
    std::fs::write(&target, [62u8; 32])?;
    let keys = WrappingKeys::load(&crate::config::WrappingKeyConfig {
        active: from.clone(),
        files: [(from.clone(), source.clone())].into(),
    })?;
    let mut tx = pool.begin().await?;
    sqlx::query_file!("sql/account_lock.sql", account.to_string())
        .fetch_one(&mut *tx)
        .await?;
    credentials::put_in(
        &mut tx,
        &keys,
        CredentialIdentity {
            account,
            credential: CredentialId::new(),
            purpose: CredentialPurpose::QrzDirectory,
        },
        None,
        SecretMaterial {
            payload_format: 1,
            bytes: zeroize::Zeroizing::new(b"operator-provider-secret".to_vec()),
        },
    )
    .await?;
    tx.commit().await?;
    let failed = outcome == "failed";
    if failed {
        std::fs::write(&source, [63u8; 32])?;
    }
    if outcome == "partial_rewrap" {
        // Model two instances configured with different bytes under one key ID.
        // Rewrap commits the first account before reaching the unreadable second.
        let login = store
            .provision_login(
                VerifiedIdentity {
                    issuer: "https://operator.example".into(),
                    subject: ulid::Ulid::generate().to_string(),
                    display_name: None,
                },
                time::Duration::days(14),
            )
            .await?;
        let later = store
            .authenticate(&login, time::Duration::days(14))
            .await?
            .context("missing second login")?
            .account_id;
        let wrong_file = directory.path().join("wrong-key");
        std::fs::write(&wrong_file, [64u8; 32])?;
        let wrong = WrappingKeys::load(&crate::config::WrappingKeyConfig {
            active: from.clone(),
            files: [(from.clone(), wrong_file)].into(),
        })?;
        let mut tx = pool.begin().await?;
        sqlx::query_file!("sql/account_lock.sql", later.to_string())
            .fetch_one(&mut *tx)
            .await?;
        crate::secrets::keys::create_in(&mut tx, &wrong, later, 1).await?;
        tx.commit().await?;
        if later < account {
            std::fs::write(&source, [64u8; 32])?;
        }
    }
    let config = directory.path().join("keys.toml");
    std::fs::write(
        &config,
        format!(
            "[wrapping_keys]\nactive = 'W2'\n[wrapping_keys.files]\n'{from}' = '{}'\nW2 = '{}'\n",
            source.display(),
            target.display()
        ),
    )?;
    let url = directory.path().join("url");
    std::fs::write(&url, std::env::var("CLACKS_TEST_DATABASE_URL")?)?;
    let args = if outcome == "partial_rewrap" {
        Arguments::parse_from([
            "clacks-admin",
            "--otlp",
            "keys",
            "rewrap",
            "--config",
            config.to_str().context("config path")?,
            "--database-url-file",
            url.to_str().context("URL file path")?,
            "--from",
            &from,
            "--to",
            "W2",
            "--batch-size",
            "1",
        ])
    } else {
        Arguments::parse_from([
            "clacks-admin",
            "--otlp",
            "keys",
            "rotate-account",
            "--config",
            config.to_str().context("config path")?,
            "--database-url-file",
            url.to_str().context("URL file path")?,
            "--account",
            &account.to_string(),
            "--from-version",
            "1",
            "--to-version",
            "2",
        ])
    };
    let result = args.execute().await;
    if failed {
        let error = result.err().context("bad wrapping material accepted")?;
        assert!(
            error
                .downcast_ref::<crate::maintenance::keys::report::AccountRotationError>()
                .is_some()
        );
    } else if outcome == "partial_rewrap" {
        let error = result.err().context("unreadable second account accepted")?;
        let progress = error
            .downcast_ref::<crate::maintenance::keys::report::RewrapError>()
            .context("missing partial rewrap progress")?;
        assert_eq!(progress.migrated_account_keys, 1);
    } else {
        result?;
    }
    pool.close().await;
    Ok(())
}
